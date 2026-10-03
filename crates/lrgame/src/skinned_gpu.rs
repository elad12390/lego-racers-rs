//! Repose original rigid-jointed geometry without rebuilding textures each frame.
use crate::gpu::{world_position, TrackGpu};
use crate::platform::prelude::*;
use lrformats::model::Model;

struct BoundVertex {
  joint: usize,
  point: Vec3,
  normal: Option<Vec3>,
}
struct Binding {
  points: Vec<BoundVertex>,
}
pub struct SkinnedGpu {
  gpu: TrackGpu,
  bindings: Vec<Binding>,
}

impl SkinnedGpu {
  pub fn set_uv_offset(&mut self, offset: [f32; 2]) {
    self.gpu.set_uv_offset(offset);
  }
  pub fn set_scene_opacity(&mut self, opacity: u8) {
    self.gpu.set_scene_opacity(opacity);
  }
  pub fn enable_scene_render(&mut self, model: &Model) -> Result<(), String> {
    self.gpu.enable_scene_render(model)
  }
  pub fn set_scene_surface(&mut self, name: &str, surface: &crate::cinematic_material::Surface) {
    self.gpu.set_scene_surface(name, surface)
  }
  pub fn set_scene_lighting(
    &mut self,
    light: &lrformats::cinematic_lighting::Lighting,
    transform: Mat4,
  ) {
    self.gpu.set_scene_lighting(light, transform)
  }
  pub fn upload(model: &Model, joints: &[Mat4]) -> Result<Self, String> {
    let gpu = TrackGpu::upload_skinned(model, joints)?;
    let bindings = model
      .surfaces
      .iter()
      .flat_map(|surface| {
        surface
          .triangles
          .chunks(1400)
          .enumerate()
          .map(|(batch, triangles)| Binding {
            points: triangles
              .iter()
              .enumerate()
              .flat_map(|(row, t)| {
                t.iter()
                  .enumerate()
                  .map(move |(corner, index)| BoundVertex {
                    joint: surface.joints[batch * 1400 + row][corner] as usize,
                    point: Vec3::from_array(model.mesh.vertices[*index as usize].position)
                      * model.mesh.scale,
                    normal: model
                      .mesh
                      .normals
                      .get(*index as usize)
                      .copied()
                      .map(Vec3::from_array),
                  })
              })
              .collect(),
          })
          .collect::<Vec<_>>()
      })
      .collect();
    Ok(Self { gpu, bindings })
  }

  pub fn set_pose(&mut self, joints: &[Mat4]) -> Result<(), String> {
    for (mesh, binding) in self.gpu.meshes.iter_mut().zip(&self.bindings) {
      for (vertex, binding) in mesh.vertices.iter_mut().zip(&binding.points) {
        let joint = joints
          .get(binding.joint)
          .ok_or("GDB animation joint absent in SDB")?;
        vertex.position = world_position(joint.transform_point3(binding.point).to_array(), 1.0);
        if let Some(normal) = binding.normal {
          vertex.normal =
            world_position(joint.transform_vector3(normal).to_array(), 1.0).extend(1.0);
        }
      }
    }
    Ok(())
  }

  pub fn draw_at(&self, transform: Mat4) {
    self.gpu.draw_at(transform);
  }
  pub fn bounds_at(&self, transform: Mat4) -> (Vec3, f32) {
    let mut lo = Vec3::splat(f32::INFINITY);
    let mut hi = Vec3::splat(f32::NEG_INFINITY);
    for mesh in &self.gpu.meshes {
      for vertex in &mesh.vertices {
        let point = transform.transform_point3(vertex.position);
        lo = lo.min(point);
        hi = hi.max(point);
      }
    }
    ((lo + hi) * 0.5, (hi - lo).length() * 0.5)
  }
  pub fn tint(&mut self, tint: Color) {
    self.gpu.tint(tint);
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn original_menu_driver_lighting_follows_animated_rotated_normals_without_accumulating() {
    let library = lrformats::library::Library::open(
      std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../extracted/Program_Files_Group/LEGO.JAM"),
    )
    .unwrap();
    let data = crate::custom_driver::Data::load(&library).unwrap();
    let build = crate::custom_driver::Build::default();
    let model = data.menu_model(&library, &build).unwrap();
    let skeleton = crate::skeleton::Skeleton::load(
      library
        .find_at(
          &format!("{}.SDB", data.menu_template(&build)),
          "MENUDATA",
          "MENUPART",
        )
        .unwrap(),
      model.mesh.scale,
    )
    .unwrap();
    let animation =
      lrformats::animation::parse(library.find_in("CBANIM.ADB", "MENUDATA").unwrap()).unwrap();
    let poses =
      [0.0, 12.0, 24.0].map(|frame| skeleton.pose(Some((&animation, "breath1", frame))).unwrap());
    let mut gpu = SkinnedGpu::upload(&model, &poses[0]).unwrap();
    let full_bright = lrformats::cinematic_lighting::Lighting {
      ambient: [255; 3],
      directional: vec![],
    };
    gpu.set_scene_lighting(&full_bright, Mat4::IDENTITY);
    let source: Vec<_> = gpu
      .gpu
      .meshes
      .iter()
      .flat_map(|m| m.vertices.iter().map(|v| v.color))
      .collect();
    let rules = lrsim::brick_build::Rules::load().unwrap();
    for pose in &poses {
      gpu.set_pose(pose).unwrap();
      for rotation in [0.0, std::f32::consts::FRAC_PI_2, std::f32::consts::PI] {
        let transform = Mat4::from_rotation_y(rotation);
        for light in [
          crate::garage_lighting::driver(&rules),
          crate::garage_lighting::license(&rules),
        ] {
          gpu.set_scene_lighting(&light, transform);
          let mut shaded = 0;
          for (vertex, base) in gpu.gpu.meshes.iter().flat_map(|m| &m.vertices).zip(&source) {
            let shade = if vertex.normal.w == 0.0 {
              [255; 3]
            } else {
              let normal = transform
                .transform_vector3(vertex.normal.truncate())
                .normalize_or_zero();
              light.shade([normal.x, -normal.z, normal.y])
            };
            let expected: [u8; 3] =
              std::array::from_fn(|i| (u16::from(base[i]) * u16::from(shade[i]) / 255) as u8);
            assert_eq!(&vertex.color[..3], &expected);
            assert_eq!(vertex.color[3], base[3]);
            shaded += usize::from(shade != [255; 3]);
          }
          assert!(
            shaded > 100,
            "original driver must retain real lighting normals"
          );
          let once: Vec<_> = gpu
            .gpu
            .meshes
            .iter()
            .flat_map(|m| m.vertices.iter().map(|v| v.color))
            .collect();
          gpu.set_scene_lighting(&light, transform);
          let twice: Vec<_> = gpu
            .gpu
            .meshes
            .iter()
            .flat_map(|m| m.vertices.iter().map(|v| v.color))
            .collect();
          assert_eq!(
            once, twice,
            "per-frame relighting must not progressively darken the driver"
          );
        }
      }
    }
  }
}
