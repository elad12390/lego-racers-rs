//! Original selector minifigure template, CBANIM idle and CMAMAN license pose.
use crate::{
  custom_driver::{Build, Data},
  skeleton::Skeleton,
  skinned_gpu::SkinnedGpu,
};
use bevy::math::Mat4;
use lrformats::{
  animation::{self, Animation},
  library::Library,
};
pub struct MenuDriver {
  gpu: SkinnedGpu,
  skeleton: Skeleton,
  animation: Animation,
  clip: usize,
  looped: bool,
  time: f32,
}
impl MenuDriver {
  pub fn load(library: &Library, build: &Build) -> Result<Self, String> {
    let rules = lrsim::brick_build::Rules::load()?;
    Self::load_animation(
      library,
      build,
      &rules.driver_animation_file,
      Some(&rules.driver_idle_clip),
      true,
      "dflt",
    )
  }
  /// CameraManAnimation::Load0047b470 starts the first CMAMAN clip without repeat.
  pub fn load_license(library: &Library, build: &Build) -> Result<Self, String> {
    Self::load_license_expression(library, build, 0)
  }
  pub fn load_license_expression(
    library: &Library,
    build: &Build,
    expression: u8,
  ) -> Result<Self, String> {
    let rules = lrsim::brick_build::Rules::load()?;
    let suffix = rules
      .license_expressions
      .get(usize::from(expression))
      .ok_or("invalid license expression")?;
    Self::load_animation(
      library,
      build,
      &rules.license_animation_file,
      None,
      false,
      suffix,
    )
  }
  fn load_animation(
    library: &Library,
    build: &Build,
    file: &str,
    clip_name: Option<&str>,
    looped: bool,
    expression: &str,
  ) -> Result<Self, String> {
    let data = Data::load(library)?;
    let model = data.menu_model_expression(library, build, expression)?;
    let name = data.menu_template(build);
    let read = |ext| {
      library
        .find_at(&format!("{name}.{ext}"), "MENUDATA", "MENUPART")
        .ok_or("missing original selector animation/rig")
    };
    let skeleton = Skeleton::load(read("SDB")?, model.mesh.scale)?;
    // RR/HR/RP/HP a0 is an empty export clip, not either screen's animation.
    // Both original screen-owned tables animate the selector's 29-joint rig.
    let animation = animation::parse(
      library
        .find_in(file, "MENUDATA")
        .ok_or_else(|| format!("missing original menu animation {file}"))?,
    )?;
    let clip = animation
      .clips
      .iter()
      .position(|c| clip_name.is_none_or(|name| c.name == name))
      .ok_or("missing original menu animation clip")?;
    let gpu = SkinnedGpu::upload(
      &model,
      &skeleton.pose_clip(Some((&animation, &animation.clips[clip].name, 0.0)), looped)?,
    )?;
    Ok(Self {
      gpu,
      skeleton,
      animation,
      clip,
      looped,
      time: 0.0,
    })
  }
  pub fn advance(&mut self, seconds: f32) -> Result<(), String> {
    let clip = &self.animation.clips[self.clip];
    self.time += seconds.max(0.0) * clip.frames_per_second;
    if !self.looped {
      self.time = self.time.min(f32::from(clip.duration));
    }
    self.gpu.set_pose(
      &self
        .skeleton
        .pose_clip(Some((&self.animation, &clip.name, self.time)), self.looped)?,
    )
  }
  /// CameraManAnimation0047b850 refreshes face material, not playback state.
  /// Keep the original CMAMAN clock/pose when replacing the uploaded face model.
  pub fn set_license_expression(
    &mut self,
    library: &Library,
    build: &Build,
    expression: u8,
  ) -> Result<(), String> {
    if self.looped {
      return Err("license expression cannot replace builder animation".into());
    }
    let mut replacement = Self::load_license_expression(library, build, expression)?;
    replacement.time = self.time;
    replacement.advance(0.0)?;
    *self = replacement;
    Ok(())
  }
  pub fn draw_at(&self, transform: Mat4) {
    self.gpu.draw_at(transform);
  }
  pub fn set_scene_lighting(
    &mut self,
    light: &lrformats::cinematic_lighting::Lighting,
    transform: Mat4,
  ) {
    self.gpu.set_scene_lighting(light, transform);
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::platform;

  fn positions(driver: &MenuDriver) -> Vec<bevy::math::Vec3> {
    platform::test_state(|s| s.batches.clear());
    driver.draw_at(Mat4::IDENTITY);
    platform::test_state(|s| {
      s.batches
        .iter()
        .flat_map(|b| b.mesh.vertices.iter().map(|v| v.position))
        .collect()
    })
  }

  #[test]
  fn original_license_snapshot_changes_face_without_restarting_cmaman_playback() {
    let library = Library::open(
      std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../extracted/Program_Files_Group/LEGO.JAM"),
    )
    .unwrap();
    let build = Build {
      hat: 15,
      face: 22,
      torso: 22,
      legs: 11,
    };
    let mut driver = MenuDriver::load_license(&library, &build).unwrap();
    driver.advance(0.2).unwrap();
    let before = positions(&driver);
    let time = driver.time;
    for expression in [1, 2, 3, 4, 5, 0] {
      driver
        .set_license_expression(&library, &build, expression)
        .unwrap();
      assert_eq!(
        positions(&driver),
        before,
        "Snapshot expression must not rewind any joint"
      );
      assert_eq!(driver.time, time);
    }
    driver.advance(100.0).unwrap();
    let end = positions(&driver);
    driver.set_license_expression(&library, &build, 1).unwrap();
    driver.advance(0.1).unwrap();
    assert_eq!(
      positions(&driver),
      end,
      "held last pose survives retaking a photograph"
    );
    let mut builder = MenuDriver::load(&library, &build).unwrap();
    assert!(builder.set_license_expression(&library, &build, 1).is_err());
  }

  #[test]
  fn original_license_preview_draws_cmaman_first_clip_and_holds_its_end_instead_of_builder_idle() {
    let library = Library::open(
      std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../extracted/Program_Files_Group/LEGO.JAM"),
    )
    .unwrap();
    let data = Data::load(&library).unwrap();
    let original = animation::parse(
      library
        .find_at("CMAMAN.ADB", "MENUDATA", "MENUDATA")
        .unwrap(),
    )
    .unwrap();
    assert_eq!(original.clips.len(), 1);
    assert_eq!(original.channels.len(), 29);
    let clip = &original.clips[0];
    for build in [
      Build::default(),
      Build {
        hat: 15,
        face: 22,
        torso: 22,
        legs: 11,
      },
    ] {
      let model = data.menu_model(&library, &build).unwrap();
      let skeleton = Skeleton::load(
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
      let mut license = MenuDriver::load_license(&library, &build).unwrap();
      let expected = SkinnedGpu::upload(
        &model,
        &skeleton
          .pose_clip(Some((&original, &clip.name, 0.0)), false)
          .unwrap(),
      )
      .unwrap();
      platform::test_state(|s| s.batches.clear());
      expected.draw_at(Mat4::IDENTITY);
      let expected_start: Vec<_> = platform::test_state(|s| {
        s.batches
          .iter()
          .flat_map(|b| b.mesh.vertices.iter().map(|v| v.position))
          .collect()
      });
      let start = positions(&license);
      assert_eq!(
        start, expected_start,
        "live license preview uses the original CMAMAN pose"
      );
      assert_ne!(
        start,
        positions(&MenuDriver::load(&library, &build).unwrap()),
        "license must not silently reuse the builder breathing pose"
      );
      license.advance(100.0).unwrap();
      let end = positions(&license);
      license.advance(0.25).unwrap();
      assert_eq!(
        end,
        positions(&license),
        "nonrepeat license preview must hold, not wrap into a new cycle"
      );
      let expected = SkinnedGpu::upload(
        &model,
        &skeleton
          .pose_clip(
            Some((&original, &clip.name, f32::from(clip.duration))),
            false,
          )
          .unwrap(),
      )
      .unwrap();
      platform::test_state(|s| s.batches.clear());
      expected.draw_at(Mat4::IDENTITY);
      let expected_end: Vec<_> = platform::test_state(|s| {
        s.batches
          .iter()
          .flat_map(|b| b.mesh.vertices.iter().map(|v| v.position))
          .collect()
      });
      assert_eq!(end, expected_end);
    }
  }
}
