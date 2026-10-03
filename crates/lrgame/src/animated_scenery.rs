//! WDB placed skinned scenery, original SDB/ADB and declared initial loop.
use crate::platform::prelude::*;
use crate::{
  gpu::{instance_transform, world_position},
  race_view::RaceView,
  skeleton::Skeleton,
  skinned_gpu::SkinnedGpu,
};
use lrformats::{
  library::Library,
  model::Model,
  scene::SceneBindings,
  world_animation::{self, Clip},
};

pub struct Object {
  pub name: String,
  gpu: SkinnedGpu,
  skeleton: Skeleton,
  animation: lrformats::animation::Animation,
  clip: Option<usize>,
  frame: f32,
  transform: Mat4,
  range: f32,
  materials: crate::world_material::Player,
  time: f32,
  uv: lrsim::uv_scroll::Clock,
}
impl Object {
  pub fn load(
    library: &Library,
    table: &str,
    bytes: &[u8],
    bindings: &SceneBindings,
  ) -> Result<Vec<Self>, String> {
    let mut out = Vec::new();
    let material_objects = lrformats::world_material::parse(bytes)?;
    for source in world_animation::parse(bytes)? {
      let model = Model::load_with_materials(
        library,
        &source.placement.model,
        Some(table),
        &bindings.materials,
      )
      .map_err(|e| e.to_string())?;
      let read = |name: &str, ext| {
        library
          .find_in(&format!("{name}.{ext}"), table)
          .or_else(|| library.find_in(&format!("{name}.{ext}"), "COMMON"))
          .ok_or_else(|| format!("missing original scenery {name}.{ext}"))
      };
      let skeleton = Skeleton::load(read(&source.skeleton, "SDB")?, model.mesh.scale)?;
      let animation = lrformats::animation::parse(read(&source.animation, "ADB")?)?;
      let clip = match source.clip {
        Some(Clip::Index(index)) => {
          if index >= animation.clips.len() {
            return Err("WDB initial clip outside declared ADB".into());
          }
          Some(index)
        }
        Some(Clip::Name(name)) => Some(
          animation
            .clips
            .iter()
            .position(|c| c.name.eq_ignore_ascii_case(&name))
            .ok_or("WDB named clip absent in ADB")?,
        ),
        None => None,
      };
      let pose =
        skeleton.pose(clip.map(|i| (&animation, animation.clips[i].name.as_str(), 0.0)))?;
      let mut gpu = SkinnedGpu::upload(&model, &pose)?;
      gpu.enable_scene_render(&model)?;
      let references = material_objects
        .iter()
        .find(|o| o.name.eq_ignore_ascii_case(&source.name))
        .map_or(&[][..], |o| o.references.as_slice());
      let mut materials =
        crate::world_material::Player::load(library, table, bytes, &model, references, bindings)?;
      materials.update(0.0, |name, surface| gpu.set_scene_surface(name, surface))?;
      let uv =
        lrsim::uv_scroll::Clock::new(lrformats::world_uv::for_object(bytes, &source.name)?.rate);
      out.push(Self {
        name: source.name,
        gpu,
        skeleton,
        animation,
        clip,
        frame: 0.0,
        transform: instance_transform(&source.placement),
        range: source.range,
        materials,
        time: 0.0,
        uv,
      });
    }
    Ok(out)
  }
  pub fn reset(&mut self) -> Result<(), String> {
    self.frame = 0.0;
    self.time = 0.0;
    self.uv.reset();
    self.materials.reset();
    self.update(0.0)
  }
  pub fn bounds(&self) -> (Vec3, f32) {
    self.gpu.bounds_at(self.transform)
  }
  pub fn update(&mut self, dt: f32) -> Result<(), String> {
    self.time += dt.max(0.0);
    self.gpu.set_uv_offset(self.uv.advance(dt));
    self.materials.update(self.time, |name, surface| {
      self.gpu.set_scene_surface(name, surface)
    })?;
    let Some(index) = self.clip else {
      return Ok(());
    };
    let clip = &self.animation.clips[index];
    self.frame += dt.max(0.0) * clip.frames_per_second;
    if clip.loop_duration > 0 {
      self.frame = self.frame.rem_euclid(f32::from(clip.loop_duration));
    }
    self.gpu.set_pose(
      &self
        .skeleton
        .pose(Some((&self.animation, &clip.name, self.frame)))?,
    )
  }
  pub fn draw(&self, eye: [f32; 3], view: RaceView) {
    let center = self.transform.w_axis.truncate();
    if self.range >= 0.0 && (center - world_position(eye, 1.0)).length() > self.range {
      return;
    }
    self.gpu.draw_at(view.world_matrix() * self.transform);
    gl_use_default_material();
  }
  pub fn draw_sky(&self, camera_relative_position: Vec3) {
    let mut transform = self.transform;
    transform.w_axis = camera_relative_position.extend(1.0);
    self.gpu.draw_at(transform);
    gl_use_default_material();
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn original_hammer_rig_changes_pose_and_returns_after_its_declared_loop() {
    let library = Library::open(
      std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../extracted/Program_Files_Group/LEGO.JAM"),
    )
    .unwrap();
    let animation =
      lrformats::animation::parse(library.find_in("RKHAMM02.ADB", "RACEC0R0").unwrap()).unwrap();
    let skeleton =
      Skeleton::load(library.find_in("RKHAMM02.SDB", "RACEC0R0").unwrap(), 1.0).unwrap();
    let clip = &animation.clips[0];
    let start = skeleton.pose(Some((&animation, &clip.name, 0.0))).unwrap();
    let moving = skeleton
      .pose(Some((
        &animation,
        &clip.name,
        f32::from(clip.loop_duration) * 0.25,
      )))
      .unwrap();
    let looped = skeleton
      .pose(Some((
        &animation,
        &clip.name,
        f32::from(clip.loop_duration),
      )))
      .unwrap();
    assert_ne!(start, moving);
    assert_eq!(start, looped);
    assert!(moving.iter().all(|m| m.is_finite()));
  }
  #[test]
  fn all_original_placed_world_rigs_sample_their_declared_initial_clip() {
    let library = Library::open(
      std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../extracted/Program_Files_Group/LEGO.JAM"),
    )
    .unwrap();
    let mut sampled = 0;
    for table in library
      .jam()
      .tables
      .iter()
      .filter(|t| t.group.eq_ignore_ascii_case("GAMEDATA") && t.name.starts_with("RACEC"))
    {
      for entry in table
        .entries
        .iter()
        .filter(|e| e.name.to_ascii_uppercase().ends_with(".WDB"))
      {
        let bytes = library.jam().bytes(entry).unwrap();
        let sources = world_animation::parse(bytes).unwrap();
        if sources.is_empty() {
          continue;
        }
        let bindings = SceneBindings::load(&library, &table.name, bytes).unwrap();
        for source in sources {
          let model = Model::load_with_materials(
            &library,
            &source.placement.model,
            Some(&table.name),
            &bindings.materials,
          )
          .unwrap_or_else(|e| panic!("{}/{}: {e}", table.name, source.name));
          let read = |name: &str, ext| {
            library
              .find_in(&format!("{name}.{ext}"), &table.name)
              .or_else(|| library.find_in(&format!("{name}.{ext}"), "COMMON"))
              .unwrap()
          };
          let skeleton = Skeleton::load(read(&source.skeleton, "SDB"), model.mesh.scale).unwrap();
          let animation = lrformats::animation::parse(read(&source.animation, "ADB")).unwrap();
          let clip = match source.clip {
            Some(Clip::Index(index)) => Some(&animation.clips[index]),
            Some(Clip::Name(name)) => animation
              .clips
              .iter()
              .find(|c| c.name.eq_ignore_ascii_case(&name)),
            None => None,
          };
          for fraction in [0.0, 0.25, 0.75, 1.0] {
            let pose = skeleton
              .pose(clip.map(|clip| {
                (
                  &animation,
                  clip.name.as_str(),
                  fraction * f32::from(clip.loop_duration),
                )
              }))
              .unwrap_or_else(|e| panic!("{}/{}: {e}", table.name, source.name));
            assert!(pose.iter().all(|m| m.is_finite()));
            for surface in &model.surfaces {
              assert!(surface
                .joints
                .iter()
                .flatten()
                .all(|joint| usize::from(*joint) < pose.len()));
            }
          }
          sampled += 1;
        }
      }
    }
    assert_eq!(sampled, 67);
  }
}
