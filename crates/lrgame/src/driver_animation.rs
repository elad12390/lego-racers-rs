//! Original PELVIS clips, source-selected reverse/reaction/finish chains.
//! Idle glances, reaction dispatch and full playback/mid-blend parity remain open.
use crate::platform::prelude::*;
use crate::{skeleton::Skeleton, skinned_gpu::SkinnedGpu};
use lrformats::{
  animation::{self, Animation},
  library::Library,
  model::Model,
};

struct ClipTime {
  index: u16,
  time: f32,
  looped: bool,
}
struct Transition {
  from: ClipTime,
  elapsed: f32,
}

struct Playback {
  current: ClipTime,
  transition: Option<Transition>,
  next: Option<u16>,
}

impl Default for Playback {
  fn default() -> Self {
    Self {
      current: ClipTime {
        index: 9,
        time: 0.0,
        looped: true,
      },
      transition: None,
      next: None,
    }
  }
}

impl Playback {
  fn update(&mut self, animation: &Animation, speed: f32, throttle: f32, steer: f32, dt: f32) {
    self.update_race(animation, speed, throttle, steer, None, 0, 0, dt);
  }

  fn update_race(
    &mut self,
    animation: &Animation,
    speed: f32,
    throttle: f32,
    steer: f32,
    finish_rank: Option<u32>,
    pending: u32,
    noise: u32,
    dt: f32,
  ) {
    if !dt.is_finite() || dt < 0.0 {
      return;
    }
    let clip = &animation.clips[usize::from(self.current.index)];
    self.current.time += dt * clip.frames_per_second;
    if !self.current.looped && self.current.time >= f32::from(clip.duration.saturating_sub(1)) {
      if let Some(next) = self.next.take() {
        self.current = ClipTime {
          index: next,
          time: 0.0,
          looped: true,
        };
        self.transition = None;
      } else {
        self.current.time = f32::from(clip.duration.saturating_sub(1));
      }
    } else if self.current.looped && clip.loop_duration > 0 {
      self.current.time = self.current.time.rem_euclid(f32::from(clip.loop_duration));
    }
    if let Some(transition) = &mut self.transition {
      transition.elapsed += dt;
      let clip = &animation.clips[usize::from(transition.from.index)];
      transition.from.time += dt * clip.frames_per_second;
      if transition.elapsed >= 0.3 {
        self.transition = None;
      }
    }
    let input = lrsim::driver_selection::Input {
      motion: lrsim::driver_motion::Motion {
        speed,
        throttle,
        steer,
      },
      current: self.current.index,
      finished: finish_rank.is_some(),
      rank: finish_rank.unwrap_or(0),
      noise,
      pending,
    };
    if let Some(decision) = lrsim::driver_selection::select(&input) {
      let from = std::mem::replace(
        &mut self.current,
        ClipTime {
          index: decision.clip,
          time: 0.0,
          looped: decision.next.is_none(),
        },
      );
      self.next = decision.next;
      self.transition = if decision.blend_ms > 0 {
        Some(Transition { from, elapsed: 0.0 })
      } else {
        None
      };
    }
  }
}

pub struct DriverAnimation {
  gpu: SkinnedGpu,
  skeleton: Skeleton,
  animation: Animation,
  playback: Playback,
  finish_noise: u32,
  pending: u32,
}

impl DriverAnimation {
  pub fn set_scene_lighting(
    &mut self,
    light: &lrformats::cinematic_lighting::Lighting,
    transform: Mat4,
  ) {
    self.gpu.set_scene_lighting(light, transform);
  }
  pub fn load(library: &Library, model: &Model) -> Result<Self, String> {
    let read = |name| {
      library
        .find_in(name, "COMMON")
        .ok_or_else(|| format!("missing original driver{name}"))
    };
    let skeleton = Skeleton::load(read("PELVIS.SDB")?, model.mesh.scale)?;
    let animation = animation::parse(read("PELVIS.ADB")?)?;
    for (index, required) in [
      "velostop", "boost", "reverse", "rev-hold", "rev-back", "steer-r", "steer-l", "s-lean-r",
      "l-lean-r", "default", "power1", "loser", "loser_lp", "winner1", "winner2", "win2_lp",
      "s-lean-l", "l-lean-l",
    ]
    .into_iter()
    .enumerate()
    {
      if animation
        .clips
        .get(index)
        .is_none_or(|c| c.name != required)
      {
        return Err(format!(
          "missing/reordered original driver clip{index}:{required}"
        ));
      }
    }
    let gpu = SkinnedGpu::upload(
      model,
      &skeleton.pose_clip(Some((&animation, "default", 0.0)), false)?,
    )?;
    Ok(Self {
      gpu,
      skeleton,
      animation,
      playback: Playback::default(),
      finish_noise: rand::gen_range(0, u32::MAX),
      pending: 0,
    })
  }

  pub fn advance(&mut self, speed: f32, throttle: f32, steer: f32, dt: f32) -> Result<(), String> {
    self
      .playback
      .update(&self.animation, speed, throttle, steer, dt);
    self.upload_pose()
  }

  pub fn advance_race(
    &mut self,
    speed: f32,
    throttle: f32,
    steer: f32,
    finish_rank: Option<u32>,
    dt: f32,
  ) -> Result<(), String> {
    let pending = if dt > 0.0 {
      std::mem::take(&mut self.pending)
    } else {
      0
    };
    self.playback.update_race(
      &self.animation,
      speed,
      throttle,
      steer,
      finish_rank,
      pending,
      self.finish_noise,
      dt,
    );
    self.upload_pose()
  }

  fn upload_pose(&mut self) -> Result<(), String> {
    let current = &self.playback.current;
    let current_name = &self.animation.clips[usize::from(current.index)].name;
    let pose = if let Some(transition) = &self.playback.transition {
      let from_name = &self.animation.clips[usize::from(transition.from.index)].name;
      self.skeleton.blend_clips(
        &self.animation,
        (from_name, transition.from.time),
        (current_name, current.time),
        transition.elapsed / 0.3,
      )?
    } else {
      self.skeleton.pose_clip(
        Some((&self.animation, current_name, current.time)),
        current.looped,
      )?
    };
    self.gpu.set_pose(&pose)
  }

  pub fn clip_name(&self) -> &str {
    &self.animation.clips[usize::from(self.playback.current.index)].name
  }
  pub fn reset(&mut self) -> Result<(), String> {
    self.playback = Playback::default();
    self.pending = 0;
    self.finish_noise = rand::gen_range(0, u32::MAX);
    self.advance(0.0, 0.0, 0.0, 0.0)
  }
  pub fn power_reaction(&mut self, turbo: bool) {
    self.pending |= if turbo { 1 } else { 2 };
  }
  pub fn draw_at(&self, transform: Mat4) {
    self.gpu.draw_at(transform);
  }
  pub fn tint(&mut self, tint: Color) {
    self.gpu.tint(tint);
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn original_assets_steering_reverse_hold_return_and_restart_flow() {
    let library = Library::open(
      std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../extracted/Program_Files_Group/LEGO.JAM"),
    )
    .unwrap();
    let animation = animation::parse(library.find_in("PELVIS.ADB", "COMMON").unwrap()).unwrap();
    let mut playback = Playback::default();
    playback.update(&animation, 30.0, 1.0, 1.0, 0.01);
    assert_eq!(playback.current.index, 6);
    assert!(playback.transition.is_some());
    for _ in 0..31 {
      playback.update(&animation, 30.0, 1.0, 1.0, 0.01);
    }
    assert!(playback.transition.is_none());
    playback.update(&animation, 30.0, 1.0, -1.0, 0.01);
    assert_eq!(playback.current.index, 5);
    playback.update(&animation, -10.0, -1.0, 0.0, 0.01);
    assert_eq!(playback.current.index, 2);
    for _ in 0..100 {
      playback.update(&animation, -10.0, -1.0, 0.0, 0.01);
    }
    assert_eq!(playback.current.index, 3);
    playback.update(&animation, -10.0, 1.0, 0.0, 0.01);
    assert_eq!(playback.current.index, 4);
    for _ in 0..100 {
      playback.update(&animation, 10.0, 1.0, 0.0, 0.01);
    }
    assert_eq!(playback.current.index, 9);
    playback = Playback::default();
    assert_eq!(playback.current.time, 0.0);
    assert!(playback.transition.is_none());
  }

  #[test]
  fn real_pelvis_finish_and_reaction_chains_hold_correct_loops_until_restart() {
    let library = Library::open(
      std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../extracted/Program_Files_Group/LEGO.JAM"),
    )
    .unwrap();
    let animation = animation::parse(library.find_in("PELVIS.ADB", "COMMON").unwrap()).unwrap();
    for (rank, noise, start, held) in [(2, 0, 11, 12), (1, 0, 13, 13), (1, 1, 14, 15)] {
      let mut playback = Playback::default();
      playback.update_race(&animation, 30.0, 1.0, 1.0, Some(rank), 3, noise, 0.01);
      assert_eq!(playback.current.index, start);
      assert!(playback.transition.is_none());
      for _ in 0..300 {
        playback.update_race(&animation, 30.0, 1.0, -1.0, Some(rank), 0, noise, 0.01);
      }
      assert_eq!(playback.current.index, held);
      assert!(playback.current.looped);
      assert!(playback.current.time < f32::from(animation.clips[usize::from(held)].loop_duration));
    }
    let mut playback = Playback::default();
    playback.update_race(&animation, 30.0, 1.0, 0.0, None, 2, 0, 0.01);
    assert_eq!(playback.current.index, 10);
    for _ in 0..100 {
      playback.update(&animation, 30.0, 1.0, 0.0, 0.01);
    }
    assert_eq!(playback.current.index, 9);
  }
}
