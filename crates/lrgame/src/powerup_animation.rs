//! Original POWERUP.WDB object-to-rig bindings, independent of effect physics.
use crate::platform::prelude::*;
use crate::{skeleton::Skeleton, skinned_gpu::SkinnedGpu};
use lrformats::{animation::Animation, library::Library, model::Model, scene::SceneBindings};

pub struct Asset {
  pub model: Model,
  pub skeleton: Skeleton,
  pub animation: Animation,
  pub uv: lrsim::uv_scroll::Scroll,
}
impl Asset {
  pub fn load(library: &Library, name: &str, bindings: &SceneBindings) -> Result<Self, String> {
    Self::load_material(library, name, bindings, None)
  }
  pub fn pickup(library: &Library, kind: u8, bindings: &SceneBindings) -> Result<Self, String> {
    //00458a80 /00457900 bind pbrickP/S/T/M to slot zero of gen.
    // brick1..4 are flying inventory/effect bricks, NOT roadside generators.
    let material = match kind {
      0 => return Self::load(library, "enh", bindings),
      1 => "pbrickp",
      2 => "pbricks",
      3 => "pbrickt",
      4 => "pbrickm",
      _ => return Err("invalid original pickup kind".into()),
    };
    Self::load_material(library, "gen", bindings, Some(material))
  }
  fn load_material(
    library: &Library,
    name: &str,
    bindings: &SceneBindings,
    material: Option<&str>,
  ) -> Result<Self, String> {
    let world = library
      .find_in("POWERUP.WDB", "COMMON")
      .ok_or("missing powerup world")?;
    let source = lrformats::world_animation::parse(world)?
      .into_iter()
      .find(|s| s.name.eq_ignore_ascii_case(name))
      .ok_or_else(|| format!("missing original animated powerup {name}"))?;
    let mut materials = bindings.materials.clone();
    if let Some(material) = material {
      let replacement = materials
        .get(material)
        .cloned()
        .ok_or("missing original pickup material")?;
      // The shipped pubricky/putraily geometry uses ptrailM for slot zero;
      // the original launcher replaces that slot with its brick material.
      materials.insert("ptrailm".into(), replacement);
    }
    let model =
      Model::load_with_materials(library, &source.placement.model, Some("COMMON"), &materials)
        .map_err(|e| e.to_string())?;
    let read = |name: &str, ext| {
      library
        .find_in(&format!("{name}.{ext}"), "COMMON")
        .ok_or_else(|| format!("missing original powerup {name}.{ext}"))
    };
    let skeleton = Skeleton::load(read(&source.skeleton, "SDB")?, model.mesh.scale)?;
    let animation = lrformats::animation::parse(read(&source.animation, "ADB")?)?;
    if animation.clips.is_empty() {
      return Err(format!("powerup {name} has no original clip"));
    }
    Ok(Self {
      model,
      skeleton,
      animation,
      uv: lrsim::uv_scroll::Scroll::new(lrformats::world_uv::for_object(world, name)?.rate),
    })
  }
  pub fn pose(&self, clip: usize, seconds: f32) -> Result<Vec<Mat4>, String> {
    self.pose_playback(clip, seconds, true)
  }
  fn pose_playback(&self, clip: usize, seconds: f32, looped: bool) -> Result<Vec<Mat4>, String> {
    let clip = self
      .animation
      .clips
      .get(clip)
      .ok_or("powerup clip absent in original ADB")?;
    let frame = seconds.max(0.0) * clip.frames_per_second;
    let frame = if looped && clip.loop_duration > 0 {
      frame.rem_euclid(f32::from(clip.loop_duration))
    } else {
      frame.min(f32::from(clip.duration))
    };
    self
      .skeleton
      .pose_clip(Some((&self.animation, &clip.name, frame)), looped)
  }
}

pub struct Player {
  asset: Asset,
  gpu: SkinnedGpu,
}
impl Player {
  pub fn pickup(library: &Library, kind: u8, bindings: &SceneBindings) -> Result<Self, String> {
    Self::from_asset(Asset::pickup(library, kind, bindings)?)
  }
  pub fn load(library: &Library, name: &str, bindings: &SceneBindings) -> Result<Self, String> {
    let asset = Asset::load(library, name, bindings)?;
    Self::from_asset(asset)
  }
  fn from_asset(asset: Asset) -> Result<Self, String> {
    let mut gpu = SkinnedGpu::upload(&asset.model, &asset.pose(0, 0.0)?)?;
    gpu.enable_scene_render(&asset.model)?;
    Ok(Self { asset, gpu })
  }
  pub fn draw(&mut self, transform: Mat4, seconds: f32, clip: usize) -> Result<(), String> {
    self.draw_playback(
      transform,
      seconds,
      clip,
      true,
      255,
      (seconds.max(0.0) * 1000.0) as u32,
    )
  }
  pub fn draw_playback(
    &mut self,
    transform: Mat4,
    seconds: f32,
    clip: usize,
    looped: bool,
    opacity: u8,
    uv_elapsed_ms: u32,
  ) -> Result<(), String> {
    self
      .gpu
      .set_pose(&self.asset.pose_playback(clip, seconds, looped)?)?;
    self.gpu.set_scene_opacity(opacity);
    self.gpu.set_uv_offset(self.asset.uv.sample(uv_elapsed_ms));
    self.gpu.draw_at(transform);
    gl_use_default_material();
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn roadside_pickup_rigs_stay_inside_collection_radius_and_use_original_colors() {
    let library = Library::open(
      std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../extracted/Program_Files_Group/LEGO.JAM"),
    )
    .unwrap();
    let world = library.find_in("POWERUP.WDB", "COMMON").unwrap();
    let bindings = SceneBindings::load(&library, "COMMON", world).unwrap();
    let rules = lrsim::powerups::Rules::load().unwrap();
    for (kind, texture) in ["pupwhit2", "pupred2", "pupblue2", "pupgree2", "pupyell2"]
      .iter()
      .enumerate()
    {
      let asset = Asset::pickup(&library, kind as u8, &bindings).unwrap();
      assert!(asset
        .model
        .surfaces
        .iter()
        .all(|s| s.texture.as_deref() == Some(*texture)));
      assert_eq!(
        asset.animation.clips.len(),
        1,
        "roadside idle, not flying inventory clips"
      );
      for tick in 0..180 {
        let seconds = tick as f32 / 60.0;
        let pose = asset.pose(0, seconds).unwrap();
        let mut lo = Vec3::splat(f32::INFINITY);
        let mut hi = Vec3::splat(f32::NEG_INFINITY);
        for surface in &asset.model.surfaces {
          for (triangle, joints) in surface.triangles.iter().zip(&surface.joints) {
            for (vertex, joint) in triangle.iter().zip(joints) {
              let p = pose[*joint as usize].transform_point3(
                Vec3::from_array(asset.model.mesh.vertices[*vertex as usize].position)
                  * asset.model.mesh.scale,
              );
              lo = lo.min(p);
              hi = hi.max(p);
            }
          }
        }
        let center = (lo + hi) * 0.5;
        assert!(
          center.length() < 1.5,
          "kind={kind} t={seconds} center={center:?}"
        );
        assert!(
          (hi - lo).max_element() > 4.0,
          "full-size original roadside pickup"
        );
        assert!(lo.abs().max_element().max(hi.abs().max_element()) < rules.pickup_radius);
        for mirrored in [false, true] {
          let view = crate::race_view::RaceView { mirrored };
          let source = [123.0, -456.0, 5.0];
          let transform = Mat4::from_translation(view.native(source)) * view.world_matrix();
          let drawn =
            transform.transform_point3(crate::gpu::world_position(center.to_array(), 1.0));
          let expected = view.native(std::array::from_fn(|i| source[i] + center[i]));
          assert!(
            (drawn - expected).length() < 0.0001,
            "mirror must reflect the local rig, not just its anchor"
          );
        }
      }
    }
  }
  #[test]
  fn original_shield_inner_and_outer_share_rig_but_retain_distinct_meshes() {
    let library = Library::open(
      std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../extracted/Program_Files_Group/LEGO.JAM"),
    )
    .unwrap();
    let bindings = SceneBindings::load(
      &library,
      "COMMON",
      library.find_in("POWERUP.WDB", "COMMON").unwrap(),
    )
    .unwrap();
    for tier in 0..4 {
      let outer = Asset::load(&library, &format!("shield{tier}"), &bindings).unwrap();
      let inner = Asset::load(&library, &format!("shldin{tier}"), &bindings).unwrap();
      let positions = |asset: &Asset| {
        asset
          .model
          .mesh
          .vertices
          .iter()
          .map(|v| v.position)
          .collect::<Vec<_>>()
      };
      assert_ne!(positions(&outer), positions(&inner));
      assert_eq!(outer.pose(0, 0.0).unwrap(), inner.pose(0, 0.0).unwrap());
      assert_ne!(outer.pose(0, 0.0).unwrap(), outer.pose(0, 0.5).unwrap());
      let duration = f32::from(outer.animation.clips[0].loop_duration)
        / outer.animation.clips[0].frames_per_second;
      assert_eq!(
        outer.pose(0, 0.0).unwrap(),
        outer.pose(0, duration).unwrap()
      );
    }
  }
  #[test]
  fn original_effect_rigs_and_every_declared_clip_have_finite_poses() {
    let library = Library::open(
      std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../extracted/Program_Files_Group/LEGO.JAM"),
    )
    .unwrap();
    let bindings = SceneBindings::load(
      &library,
      "COMMON",
      library.find_in("POWERUP.WDB", "COMMON").unwrap(),
    )
    .unwrap();
    for name in [
      "magnet", "dmissil", "curse", "TurboL0", "TurboL1", "TurboL2", "enh", "brick1", "brick2",
      "brick3", "brick4", "turb0f1", "turb0f2", "turb1f1", "turb1f2", "turb2f1", "turb2f2",
    ] {
      let asset = Asset::load(&library, name, &bindings).unwrap();
      if name.starts_with("TurboL") || name.starts_with("turb") {
        assert_eq!(asset.animation.clips.len(), 3, "{name} startup/run/tail");
      }
      for clip in 0..asset.animation.clips.len() {
        for seconds in [0.0, 0.2, 0.5, 1.0, 5.0] {
          assert!(
            asset
              .pose(clip, seconds)
              .unwrap()
              .iter()
              .all(|m| m.is_finite()),
            "{name}"
          );
        }
      }
    }
  }
}
