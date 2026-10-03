//! Native GPU translation of original asset geometry. Rendering parity remains open.
//! GDB float-vertex records carry unsigned baked RGBA. This does NOT assign RGB
//! semantics to the unrelated packed signed-byte runtime attributes.
use std::collections::HashMap;

use crate::platform::prelude::*;
use lrformats::model::Model;

pub struct TrackGpu {
  pub meshes: Vec<Mesh>,
  pub center: Vec3,
  pub radius: f32,
  scene_pipelines: Vec<Material>,
  scene_sources: Vec<Vec<[u8; 4]>>,
  scene_slots: Vec<Option<String>>,
  scene_raw: Vec<Vec<[u8; 4]>>,
  scene_tints: Vec<[u8; 4]>,
  source_uv: Vec<Vec<Vec2>>,
}

pub fn world_position(position: [f32; 3], scale: f32) -> Vec3 {
  // Original +Z-up becomes native +Y-up; negate oldY to retain handedness.
  vec3(
    position[0] * scale,
    position[2] * scale,
    -position[1] * scale,
  )
}
pub fn instance_transform(instance: &lrformats::world::Instance) -> Mat4 {
  // WDB instantiation calls the model's orientation slot +0x40 (GolDP
  // 10026b50 for skinned objects). It orthonormalizes these directions;
  // their lengths are NOT geometry scale. RKHAMM02 carries length 3.204:
  // retaining it enlarged both the already-sized mesh and its joint offsets.
  let forward = world_position(instance.forward, 1.0).normalize_or_zero();
  let up = world_position(instance.up, 1.0);
  let up = (up - forward * up.dot(forward)).normalize_or_zero();
  let right = forward.cross(up);
  Mat4::from_cols(
    forward.extend(0.0),
    up.extend(0.0),
    right.extend(0.0),
    world_position(instance.position, 1.0).extend(1.0),
  )
}

#[cfg(test)]
mod placement_tests {
  use super::*;

  #[test]
  fn world_orientation_lengths_do_not_resize_or_shear_geometry() {
    let instance = lrformats::world::Instance {
      model: "orientation-regression".into(),
      position: [10.0, 20.0, 30.0],
      forward: [2.0, 0.0, 0.0],
      up: [2.0, 0.0, 4.0],
    };
    let transform = instance_transform(&instance);
    // Original 10026b50 returns identity for these scaled/skewed directions.
    let point = [3.0, 4.0, 5.0];
    let expected = world_position([13.0, 24.0, 35.0], 1.0);
    assert!((transform.transform_point3(world_position(point, 1.0)) - expected).length() < 1e-6);
  }

  #[test]
  fn original_hammer_placement_matches_executed_original_orientation_setter() {
    let library = lrformats::library::Library::open(
      std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../extracted/Program_Files_Group/LEGO.JAM"),
    )
    .unwrap();
    let objects =
      lrformats::world_animation::parse(library.find_in("RRTRK.WDB", "RACEC0R0").unwrap()).unwrap();
    let hammer = objects.iter().find(|o| o.name == "rkhamm02").unwrap();
    let transform = instance_transform(&hammer.placement);
    // Columns read from the unmodified 10026b50 routine, real WDB inputs.
    let original = [
      [0.21967116, 0.65652657, -0.72160757],
      [-0.94832355, 0.31730509, -0.00000014562731],
      [0.22896966, 0.68431747, 0.69230235],
    ];
    for (axis, column) in [Vec3::X, Vec3::Y, Vec3::Z].into_iter().zip(original) {
      let actual = transform.transform_vector3(world_position(axis.to_array(), 1.0));
      assert!((actual - world_position(column, 1.0)).length() < 1e-6);
      assert!((actual.length() - 1.0).abs() < 1e-6);
    }
    assert_eq!(
      transform.transform_point3(Vec3::ZERO),
      world_position(hammer.placement.position, 1.0)
    );
    let model = lrformats::model::Model::load(&library, "rkhamm02", Some("RACEC0R0")).unwrap();
    let skeleton = crate::skeleton::Skeleton::load(
      library.find_in("RKHAMM02.SDB", "RACEC0R0").unwrap(),
      model.mesh.scale,
    )
    .unwrap();
    let animation =
      lrformats::animation::parse(library.find_in("RKHAMM02.ADB", "RACEC0R0").unwrap()).unwrap();
    for time in [0.0, 25.0, 50.0, 75.0, 100.0] {
      for joint in skeleton.pose(Some((&animation, "a0", time))).unwrap() {
        let local = joint.transform_point3(Vec3::ZERO);
        let placed = transform.transform_vector3(world_position(local.to_array(), 1.0));
        // Animation translations retain their source length at every pose.
        assert!((placed.length() - local.length()).abs() < 1e-5);
      }
    }
  }
}

impl TrackGpu {
  pub fn tint(&mut self, tint: Color) {
    let rgba = tint.to_vec().to_array();
    for mesh in &mut self.meshes {
      for vertex in &mut mesh.vertices {
        for i in 0..4 {
          vertex.color[i] = (vertex.color[i] as f32 * rgba[i]) as u8;
        }
      }
    }
  }
  pub fn upload(model: &Model) -> Result<Self, String> {
    Self::upload_skinned(model, &[])
  }

  pub fn upload_skinned(model: &Model, joints: &[Mat4]) -> Result<Self, String> {
    let mut textures = HashMap::new();
    for (name, image) in &model.images {
      if image.width == 0 || image.height == 0 {
        return Err(format!("original image {name} has zero dimensions"));
      }
      if image
        .indices
        .iter()
        .any(|i| *i as usize >= image.palette.len())
      {
        return Err(format!("original image {name} has invalid palette indices"));
      }
      let mut rgba = image.to_rgba();
      if let Some(key) = model.surfaces.iter().find_map(|s| {
        s.texture
          .as_deref()
          .filter(|t| t.eq_ignore_ascii_case(name))
          .and(s.color_key)
      }) {
        for p in rgba.chunks_exact_mut(4) {
          if p[..3] == key {
            p[3] = 0;
          }
        }
      }
      let texture = Texture2D::from_rgba8(image.width, image.height, &rgba);
      texture.set_filter(FilterMode::Nearest);
      // The Bevy image sampler repeats both original UV axes.
      textures.insert(name.clone(), texture);
    }
    let mut lo = Vec3::splat(f32::INFINITY);
    let mut hi = Vec3::splat(f32::NEG_INFINITY);
    for vertex in &model.mesh.vertices {
      let position = world_position(vertex.position, model.mesh.scale);
      lo = lo.min(position);
      hi = hi.max(position);
    }
    let mut meshes = Vec::new();
    let mut scene_sources = Vec::new();
    let mut scene_slots = Vec::new();
    let mut scene_raw = Vec::new();
    let mut scene_tints = Vec::new();
    for surface in &model.surfaces {
      let texture = surface
        .texture
        .as_ref()
        .and_then(|name| textures.get(&name.to_ascii_lowercase()))
        .cloned();
      // Bound batches under the native renderer's default draw capacities.
      for (batch, triangles) in surface.triangles.chunks(1400).enumerate() {
        let mut vertices = Vec::with_capacity(triangles.len() * 3);
        let mut sources = Vec::with_capacity(triangles.len() * 3);
        let mut raw = Vec::with_capacity(triangles.len() * 3);
        for (row, triangle) in triangles.iter().enumerate() {
          for (corner, &index) in triangle.iter().enumerate() {
            let joint = surface
              .joints
              .get(batch * 1400 + row)
              .ok_or("missing per-corner GDB joint bindings")?[corner];
            let source = model
              .mesh
              .vertices
              .get(index as usize)
              .ok_or_else(|| format!("invalid original vertex index{index}"))?;
            let mut original = Vec3::from_array(source.position) * model.mesh.scale;
            if let Some(joint) = joints.get(usize::from(joint)) {
              original = joint.transform_point3(original);
            }
            let position = world_position(original.to_array(), 1.0);
            let base = if texture.is_some() {
              [255; 4]
            } else {
              surface.color
            };
            let mut uncolored = source.rgba;
            if let Some(normal) = model.mesh.normals.get(index as usize) {
              let normal = Vec3::from_array(*normal);
              let normal = joints
                .get(joint as usize)
                .map_or(normal, |m| m.transform_vector3(normal));
              // Provisional menu light, explicitly separate from baked
              // RGBA assets. Retain the source normal for later shaders.
              let gain = 0.55 + 0.45 * normal.dot(vec3(0.5, -0.3, 0.8).normalize()).max(0.0);
              for channel in &mut uncolored[..3] {
                *channel = (*channel as f32 * gain) as u8;
              }
            }
            sources.push(uncolored);
            raw.push(source.rgba);
            let rgba = std::array::from_fn::<_, 4, _>(|i| {
              (u16::from(base[i]) * u16::from(uncolored[i]) / 255) as u8
            });
            let color = Color::from_rgba(rgba[0], rgba[1], rgba[2], rgba[3]);
            // Both baked and normal GDB vertices use bottom-origin V.
            // BMP decoding supplies top-origin pixels. Keeping baked V
            // unflipped mapped KK tire treads onto the atlas's white trim.
            let v = 1.0 - source.uv[1];
            let mut vertex =
              Vertex::new(position.x, position.y, position.z, source.uv[0], v, color);
            if let Some(normal) = model.mesh.normals.get(index as usize) {
              let normal = Vec3::from_array(*normal);
              let normal = joints
                .get(joint as usize)
                .map_or(normal, |m| m.transform_vector3(normal));
              vertex.normal = world_position(normal.to_array(), 1.0).extend(1.0);
            }
            vertices.push(vertex);
          }
        }
        let indices = (0..vertices.len()).map(|index| index as u16).collect();
        meshes.push(Mesh {
          vertices,
          indices,
          texture: texture.clone(),
        });
        scene_sources.push(sources);
        scene_slots.push(surface.material.clone());
        scene_raw.push(raw);
        scene_tints.push(if texture.is_some() {
          [255; 4]
        } else {
          surface.color
        });
      }
    }
    let source_uv = meshes
      .iter()
      .map(|m| m.vertices.iter().map(|v| v.uv).collect())
      .collect();
    Ok(Self {
      meshes,
      center: (lo + hi) * 0.5,
      radius: (hi - lo).length().max(1.0) * 0.5,
      scene_pipelines: Vec::new(),
      scene_sources,
      scene_slots,
      scene_raw,
      scene_tints,
      source_uv,
    })
  }

  pub fn enable_scene_render(&mut self, model: &Model) -> Result<(), String> {
    let mut cache: HashMap<Option<[u8; 2]>, Material> = HashMap::new();
    let mut pipelines = Vec::new();
    for surface in &model.surfaces {
      // Color-key cutouts still own depth. Only explicit MDB blend
      // factors place a surface in the non-depth-writing pass.
      let blend = surface.blend;
      let material = if let Some(material) = cache.get(&blend) {
        material.clone()
      } else {
        let material = crate::scene_material::load_blend(blend)?;
        cache.insert(blend, material.clone());
        material
      };
      pipelines.extend(std::iter::repeat_n(
        material,
        surface.triangles.len().div_ceil(1400),
      ));
    }
    self.scene_pipelines = pipelines;
    self.scene_sources.clone_from(&self.scene_raw);
    Ok(())
  }
  pub fn set_scene_surface(&mut self, name: &str, surface: &crate::cinematic_material::Surface) {
    for i in 0..self.meshes.len() {
      if !self.scene_slots[i]
        .as_deref()
        .is_some_and(|slot| slot.eq_ignore_ascii_case(name))
      {
        continue;
      }
      let mesh = &mut self.meshes[i];
      mesh.texture = surface.texture.clone();
      self.scene_pipelines[i] = surface.pipeline.clone();
      self.scene_tints[i] = surface.color;
      for (vertex, source) in mesh.vertices.iter_mut().zip(&self.scene_sources[i]) {
        vertex.color =
          std::array::from_fn(|i| (u16::from(source[i]) * u16::from(surface.color[i]) / 255) as u8);
      }
    }
  }

  pub fn set_scene_opacity(&mut self, opacity: u8) {
    for (i, mesh) in self.meshes.iter_mut().enumerate() {
      for (j, vertex) in mesh.vertices.iter_mut().enumerate() {
        vertex.color[3] = (u32::from(self.scene_sources[i][j][3])
          * u32::from(self.scene_tints[i][3])
          * u32::from(opacity)
          / (255 * 255)) as u8;
      }
    }
  }

  pub fn set_scene_lighting(
    &mut self,
    light: &lrformats::cinematic_lighting::Lighting,
    transform: Mat4,
  ) {
    for (i, mesh) in self.meshes.iter_mut().enumerate() {
      for (j, vertex) in mesh.vertices.iter_mut().enumerate() {
        let mut source = self.scene_raw[i][j];
        if vertex.normal.w != 0.0 {
          let normal = transform
            .transform_vector3(vertex.normal.truncate())
            .normalize_or_zero();
          let shade = light.shade([normal.x, -normal.z, normal.y]);
          for channel in 0..3 {
            source[channel] = (u16::from(source[channel]) * u16::from(shade[channel]) / 255) as u8;
          }
        }
        self.scene_sources[i][j] = source;
        let tint = self.scene_tints[i];
        vertex.color = std::array::from_fn(|channel| {
          (u16::from(source[channel]) * u16::from(tint[channel]) / 255) as u8
        });
      }
    }
  }

  pub fn draw(&self) {
    for (i, mesh) in self.meshes.iter().enumerate() {
      if let Some(material) = self.scene_pipelines.get(i) {
        gl_use_material(material);
      }
      draw_mesh(mesh);
    }
  }
  pub fn set_uv_offset(&mut self, [u, v]: [f32; 2]) {
    let offset = vec2(u, -v); // Upload already flips original V.
    for (mesh, source) in self.meshes.iter_mut().zip(&self.source_uv) {
      for (vertex, original) in mesh.vertices.iter_mut().zip(source) {
        vertex.uv = *original + offset;
      }
    }
  }

  pub fn draw_at(&self, transform: Mat4) {
    push_model_matrix(transform);
    self.draw();
    pop_model_matrix();
  }
}
