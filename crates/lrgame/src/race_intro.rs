//! RAB-selected original WDB camera rig, nonlooping first ADB clip and handoff.
use crate::platform::prelude::*;
use crate::skeleton::Skeleton;
use lrformats::{animation::Animation, library::Library, world_camera};

pub struct Intro {
  camera: world_camera::Camera,
  rig: Option<(Skeleton, Animation, Mat4)>,
  bind: Mat4,
  elapsed: f64,
  duration: f64,
  settling: f64,
  handed_off: bool,
}
fn placement(instance: &lrformats::world::Instance) -> Mat4 {
  let forward = Vec3::from_array(instance.forward).normalize();
  let up = Vec3::from_array(instance.up);
  let up = (up - forward * up.dot(forward)).normalize();
  Mat4::from_cols(
    forward.extend(0.0),
    up.cross(forward).extend(0.0),
    up.extend(0.0),
    Vec3::from_array(instance.position).extend(1.0),
  )
}
impl Intro {
  pub fn load(library: &Library, table: &str) -> Result<Option<Self>, String> {
    let owner = library
      .jam()
      .tables
      .iter()
      .find(|t| t.name.eq_ignore_ascii_case(table))
      .ok_or("missing race camera owner")?;
    let archive = owner
      .entries
      .iter()
      .find(|e| e.name.to_ascii_uppercase().ends_with(".RAB"))
      .ok_or("missing race camera archive")?;
    let Some(name) = lrformats::race_archive::intro_camera(
      library.jam().bytes(archive).map_err(|e| e.to_string())?,
    )?
    else {
      return Ok(None);
    };
    let mut cameras = Vec::new();
    for entry in &owner.entries {
      if !entry.name.to_ascii_uppercase().ends_with(".WDB") {
        continue;
      }
      cameras.extend(
        world_camera::parse(library.jam().bytes(entry).map_err(|e| e.to_string())?)?
          .into_iter()
          .filter(|c| c.placement.model.eq_ignore_ascii_case(&name)),
      );
    }
    if cameras.len() != 1 {
      return Err(format!(
        "{table}/{name}: missing or ambiguous original intro camera"
      ));
    }
    let camera = cameras.remove(0);
    let bind = placement(&camera.placement);
    let mut duration = 0.0;
    let rig = if let Some(source) = &camera.rig {
      let read = |name: &str, extension: &str| {
        library
          .find_in(&format!("{name}.{extension}"), table)
          .ok_or_else(|| format!("missing original camera {table}/{name}.{extension}"))
      };
      let skeleton = Skeleton::load(read(&source.skeleton, "SDB")?, 1.0)?;
      if source.joint >= skeleton.pose(None)?.len() {
        return Err("race intro joint outside source rig".into());
      }
      let animation = lrformats::animation::parse(read(&source.animation, "ADB")?)?;
      let clip = animation
        .clips
        .first()
        .ok_or("race intro rig lacks first clip")?;
      if !clip.frames_per_second.is_finite() || clip.frames_per_second <= 0.0 || clip.duration == 0
      {
        return Err("invalid race intro clip timing".into());
      }
      duration = f64::from(clip.duration) / f64::from(clip.frames_per_second);
      Some((skeleton, animation, placement(&source.placement)))
    } else {
      None
    };
    Ok(Some(Self {
      camera,
      rig,
      bind,
      elapsed: 0.0,
      duration,
      settling: 0.0,
      handed_off: false,
    }))
  }
  pub fn reset(&mut self) {
    self.elapsed = 0.0;
    self.settling = 0.0;
    self.handed_off = false;
  }
  pub fn showing(&self) -> bool {
    !self.handed_off
  }
  pub fn ready(&self) -> bool {
    self.handed_off && self.settling >= 2.0
  }
  pub fn pose(&self) -> Result<([f32; 3], [f32; 3], [f32; 3]), String> {
    let transform = if let Some((skeleton, animation, placement)) = &self.rig {
      let clip = &animation.clips[0];
      let matrices = skeleton.pose_clip(
        Some((
          animation,
          &clip.name,
          (self.elapsed * f64::from(clip.frames_per_second)).min(f64::from(clip.duration)) as f32,
        )),
        false,
      )?;
      *placement * matrices[self.camera.rig.as_ref().unwrap().joint] * self.bind
    } else {
      self.bind
    };
    Ok((
      transform.w_axis.truncate().to_array(),
      transform.x_axis.truncate().to_array(),
      transform.z_axis.truncate().to_array(),
    ))
  }
  pub fn projection(&self) -> (f32, f32, f32) {
    (
      self.camera.fov_degrees.to_radians(),
      self.camera.near,
      self.camera.far,
    )
  }
  /// Returns a one-time pose to seed the chase rig. Excess clip time is not
  /// leaked into the two-second stationary handoff or the three-second gate.
  pub fn advance(
    &mut self,
    dt: f32,
    skip: bool,
  ) -> Result<Option<([f32; 3], [f32; 3], [f32; 3])>, String> {
    if !dt.is_finite() || dt <= 0.0 {
      return Ok(None);
    }
    if self.handed_off {
      self.settling = (self.settling + f64::from(dt)).min(2.0);
      return Ok(None);
    }
    self.elapsed = (self.elapsed + f64::from(dt)).min(self.duration);
    if skip || self.elapsed >= self.duration {
      self.handed_off = true;
      return self.pose().map(Some);
    }
    Ok(None)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn every_playable_rab_intro_resolves_its_exact_world_object_and_joint() {
    let library = Library::open(
      std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../extracted/Program_Files_Group/LEGO.JAM"),
    )
    .unwrap();
    let mut count = 0;
    let mut missing = Vec::new();
    for table in library
      .jam()
      .tables
      .iter()
      .filter(|t| t.group == "GAMEDATA" && t.name.starts_with("RACEC"))
    {
      let Some(mut intro) = Intro::load(&library, &table.name).unwrap() else {
        println!("{} has no authored RAB intro camera", table.name);
        missing.push(table.name.as_str());
        continue;
      };
      let first = intro.pose().unwrap();
      if table.name == "RACEC0R0" {
        assert!((intro.duration - 8.0 / 30.0).abs() < 1e-6);
        assert_eq!(intro.camera.rig.as_ref().unwrap().joint, 1);
        println!("RACEC0R0 duration {}", intro.duration);
      }
      assert!(first.0.into_iter().all(f32::is_finite), "{}", table.name);
      assert!(!intro.ready());
      let handoff = intro
        .advance((intro.duration + 0.1) as f32, false)
        .unwrap()
        .unwrap();
      assert!(Vec3::from_array(handoff.1).length() > 0.99);
      assert!(
        Vec3::from_array(handoff.2)
          .dot(Vec3::from_array(handoff.1))
          .abs()
          < 1e-4
      );
      assert!(!intro.showing());
      assert!(!intro.ready());
      assert!(intro.advance(1.999, false).unwrap().is_none());
      assert!(!intro.ready());
      intro.advance(0.0011, false).unwrap();
      assert!(intro.ready());
      intro.reset();
      assert_eq!(intro.pose().unwrap(), first);
      count += 1;
    }
    assert_eq!(count, 10);
    assert_eq!(missing, ["RACEC0R2", "RACEC1R2", "RACEC3R0"]);
    println!("{count} original intro camera bindings resolved");
  }
}
