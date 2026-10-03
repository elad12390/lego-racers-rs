//! Chase/cockpit rig from 004281b0/004283f0/00428540/004280a0.
//! Source-derived target following plus x86-checked preset/basis/smoothing.
//! Rear-view output follows 00427d30 without mutating the forward chase state.
//! Cinematic/impact modes and full trajectory calibration remain open.
use bevy::math::{vec2, vec3, Mat3, Quat, Vec2, Vec3};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
  #[default]
  Chase,
  Far,
  Near,
  Cockpit,
}

impl Mode {
  /// Racer::NextHandling (0043a390) cycles all four original player views.
  pub fn next(self) -> Self {
    match self {
      Self::Chase => Self::Far,
      Self::Far => Self::Near,
      Self::Near => Self::Cockpit,
      Self::Cockpit => Self::Chase,
    }
  }

  pub fn label(self) -> &'static str {
    match self {
      Self::Chase => "Chase",
      Self::Far => "Far",
      Self::Near => "Near",
      Self::Cockpit => "Cockpit",
    }
  }

  fn preset(self) -> Preset {
    match self {
      Self::Chase => PRESETS[0],
      Self::Far => PRESETS[1],
      Self::Near => PRESETS[2],
      // Original 004b0328 record 3: mode 2, zero offsets, same ratios as 0.
      Self::Cockpit => Preset {
        pitch_degrees: 0.0,
        lift_degrees: 0.0,
        distance: 0.0,
        position_ratio: 0.1,
        orientation_ratio: 0.25,
      },
    }
  }
}

///00428f40chooses the smaller component before the original CRT acos.
fn heading_angle(x: f32, y: f32) -> f32 {
  if y.abs() < x.abs() {
    if x > 0.0 {
      std::f32::consts::FRAC_PI_2 - y.clamp(-1.0, 1.0).acos()
    } else {
      std::f32::consts::FRAC_PI_2 + y.clamp(-1.0, 1.0).acos()
    }
  } else if y > 0.0 {
    x.clamp(-1.0, 1.0).acos()
  } else {
    -x.clamp(-1.0, 1.0).acos()
  }
}

#[derive(Clone, Copy, Deserialize, Serialize)]
pub struct Preset {
  pub pitch_degrees: f32,
  pub lift_degrees: f32,
  pub distance: f32,
  pub position_ratio: f32,
  pub orientation_ratio: f32,
}

// Original004b0328 records0..2. These are camera settings despite the legacy
// decompiler's HandlingPresets name; they do not change vehicle handling.
pub const PRESETS: [Preset; 3] = [
  Preset {
    pitch_degrees: 5.0,
    lift_degrees: 35.0,
    distance: 20.0,
    position_ratio: 0.1,
    orientation_ratio: 0.25,
  },
  Preset {
    pitch_degrees: 8.0,
    lift_degrees: 25.0,
    distance: 30.0,
    position_ratio: 0.1,
    orientation_ratio: 0.25,
  },
  Preset {
    pitch_degrees: 8.0,
    lift_degrees: 45.0,
    distance: 10.0,
    position_ratio: 0.05,
    orientation_ratio: 0.25,
  },
];

pub fn target(preset: Preset, owner: Vec3, heading: Vec2) -> (Vec3, Mat3) {
  let heading = heading.normalize_or_zero();
  let (pitch, cosine) = preset.pitch_degrees.to_radians().sin_cos();
  let forward = vec3(heading.x * cosine, heading.y * cosine, -pitch);
  let eye = owner - forward * preset.distance
    + Vec3::Z * preset.lift_degrees.to_radians().sin() * preset.distance;
  (eye, basis(forward, -Vec3::Z))
}

pub fn basis(forward: Vec3, down: Vec3) -> Mat3 {
  let forward = forward.normalize();
  let down = (down - forward * forward.dot(down)).normalize();
  Mat3::from_cols(down.cross(forward), down, forward)
}

/// Complete004280a0's normal blend, before optional held-object output.
/// Original quaternions are conjugated relative to glam; conjugating both ends
/// commutes with shortest-arc normalized linear interpolation.
pub fn smooth(
  preset: Preset,
  previous: (Vec3, Mat3),
  desired: (Vec3, Mat3),
  dt_ms: f32,
) -> (Vec3, Mat3) {
  let coefficient = |ratio: f32| (1.0 - ratio) / (ratio * 250.0);
  let retained = 1.0 / (coefficient(preset.position_ratio) * dt_ms + 1.0);
  let position = desired.0 + (previous.0 - desired.0) * retained;
  let retained = 1.0 / (coefficient(preset.orientation_ratio) * dt_ms + 1.0);
  let a = Quat::from_mat3(&desired.1);
  let mut b = Quat::from_mat3(&previous.1);
  if a.dot(b) <= 0.0 {
    b = -b;
  }
  (position, Mat3::from_quat(a.lerp(b, retained).normalize()))
}

pub struct ChaseRig {
  pose: (Vec3, Mat3),
  owner: Vec3,
  heading: f32,
  turn: f32,
  preset: Preset,
  first: bool,
  mode: Mode,
}

impl ChaseRig {
  pub fn new(position: [f32; 3], forward: [f32; 3]) -> Self {
    Self::with_preset(position, forward, PRESETS[0])
  }

  pub fn with_preset(position: [f32; 3], forward: [f32; 3], preset: Preset) -> Self {
    let owner = Vec3::from_array(position);
    let direction = vec2(forward[0], forward[1]).normalize_or_zero();
    Self {
      pose: target(preset, owner, direction),
      owner,
      heading: direction.y.atan2(direction.x),
      turn: 0.0,
      preset,
      first: true,
      mode: Mode::Chase,
    }
  }

  pub fn mode(&self) -> Mode {
    self.mode
  }

  pub fn handoff(&mut self, eye: [f32; 3], forward: [f32; 3], up: [f32; 3]) {
    self.pose = (
      Vec3::from_array(eye),
      basis(Vec3::from_array(forward), -Vec3::from_array(up)),
    );
    self.first = false;
  }

  pub fn set_mode(&mut self, mode: Mode) {
    if self.mode != mode {
      self.mode = mode;
      self.preset = mode.preset();
      // ApplyHandlingPreset marks the rig active so a mode switch snaps,
      // rather than blending a cockpit eye through the player's body.
      self.first = true;
    }
  }

  pub fn update_view(
    &mut self,
    position: [f32; 3],
    forward: [f32; 3],
    up: [f32; 3],
    yaw_rate: f32,
    dt: f32,
  ) {
    if self.mode != Mode::Cockpit {
      self.update(position, forward, yaw_rate, dt);
      return;
    }
    if !dt.is_finite() || dt <= 0.0 {
      return;
    }
    // 00428540 mode 2 follows the actual body basis, not planar chase
    // heading, and adds 10 world-Z units (6 in the separate snap variant).
    self.owner = Vec3::from_array(position);
    let desired = (
      self.owner + Vec3::Z * 10.0,
      basis(Vec3::from_array(forward), -Vec3::from_array(up)),
    );
    self.pose = if self.first {
      self.first = false;
      desired
    } else {
      smooth(self.preset, self.pose, desired, (dt * 1000.0).min(100.0))
    };
  }

  pub fn view_pose(&self, rear: bool) -> ([f32; 3], [f32; 3], [f32; 3]) {
    if !rear {
      return self.pose();
    }
    let (eye, forward, up) = self.pose();
    if self.mode == Mode::Cockpit {
      return (eye, (-Vec3::from_array(forward)).to_array(), up);
    }
    // 00427d30 mode 1 reflects only the horizontal eye coordinates around
    // owner_pos, preserves height, and reverses horizontal optical direction.
    let eye = Vec3::from_array(eye);
    let reflect = |v: [f32; 3]| [-v[0], -v[1], v[2]];
    (
      [
        2.0 * self.owner.x - eye.x,
        2.0 * self.owner.y - eye.y,
        eye.z,
      ],
      reflect(forward),
      reflect(up),
    )
  }

  pub fn update(&mut self, position: [f32; 3], forward: [f32; 3], yaw_rate: f32, dt: f32) {
    if !dt.is_finite() || dt <= 0.0 {
      return;
    }
    if self.first {
      self.first = false;
      self.owner = Vec3::from_array(position);
      let direction = vec2(forward[0], forward[1]).normalize_or_zero();
      self.heading = direction.y.atan2(direction.x);
      self.pose = target(self.preset, self.owner, direction);
      return;
    }
    let ms = (dt * 1000.0).min(100.0);
    let retain = |rate: f32| 1.0 / (rate * ms + 1.0);
    // Original rig anticipates80ms of angular motion, limited to0.3radians.
    let desired_turn = (yaw_rate * 0.08).clamp(-0.3, 0.3);
    self.turn = desired_turn + (self.turn - desired_turn) * retain(0.0044444444);
    let planar = vec2(forward[0], forward[1]);
    //00428540writes the anticipated X component before computing Y.
    // Preserve that observable heading rather than replacing it with an
    // ideal angle addition; the difference accumulates on curved trails.
    let (sine, cosine) = self.turn.sin_cos();
    let x = cosine * planar.x - sine * planar.y;
    let y = sine * x + cosine * planar.y;
    // The component-based angle runs BEFORE later vector normalization.
    let target_angle = heading_angle(x, y);
    let raw_difference = heading_angle(self.heading.cos(), self.heading.sin()) - target_angle;
    let difference = (raw_difference + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
      - std::f32::consts::PI;
    self.heading = if raw_difference.abs() > 0.014 {
      target_angle + difference * retain(0.016666668)
    } else {
      y.atan2(x)
    };
    let position = Vec3::from_array(position);
    self.owner = position + (self.owner - position) * retain(0.09);
    let (_, orientation) = target(
      self.preset,
      self.owner,
      vec2(self.heading.cos(), self.heading.sin()),
    );
    // The original uses the previous smoothed optical direction for eye
    // placement, not vehicle velocity or instantaneous banked chassis up.
    let eye = self.owner - self.pose.1.z_axis * self.preset.distance
      + Vec3::Z * self.preset.lift_degrees.to_radians().sin() * self.preset.distance;
    self.pose = smooth(self.preset, self.pose, (eye, orientation), ms);
  }

  pub fn pose(&self) -> ([f32; 3], [f32; 3], [f32; 3]) {
    (
      self.pose.0.to_array(),
      self.pose.1.z_axis.to_array(),
      (-self.pose.1.y_axis).to_array(),
    )
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn original_four_view_cycle_and_rear_view_preserve_follow_state() {
    let mut rig = ChaseRig::new([30.0, 20.0, 5.0], [1.0, 0.0, 0.0]);
    for mode in [Mode::Chase, Mode::Far, Mode::Near] {
      rig.set_mode(mode);
      rig.update_view(
        [30.0, 20.0, 5.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        0.0,
        0.016,
      );
      let original = rig.pose();
      let rear = rig.view_pose(true);
      assert_eq!(rear.0[2], original.0[2]);
      assert!((rear.0[0] + original.0[0] - 60.0).abs() < 0.00001);
      assert_eq!(rear.1, [-original.1[0], -original.1[1], original.1[2]]);
      assert!(Vec3::from_array(rear.1).dot(Vec3::from_array(rear.2)).abs() < 0.00001);
      assert_eq!(rig.view_pose(false), original);
    }
    let mut mode = Mode::Chase;
    for expected in [Mode::Far, Mode::Near, Mode::Cockpit, Mode::Chase] {
      mode = mode.next();
      assert_eq!(mode, expected);
    }
  }

  #[test]
  fn cockpit_follows_tilted_body_and_rear_view_keeps_eye_and_up() {
    let mut rig = ChaseRig::new([30.0, 20.0, 5.0], [1.0, 0.0, 0.0]);
    rig.set_mode(Mode::Cockpit);
    let axes = Mat3::from_rotation_y(0.3);
    rig.update_view(
      [30.0, 20.0, 5.0],
      axes.x_axis.to_array(),
      axes.z_axis.to_array(),
      0.0,
      0.016,
    );
    let front = rig.view_pose(false);
    let rear = rig.view_pose(true);
    assert_eq!(front.0, [30.0, 20.0, 15.0]);
    assert_eq!(front.0, rear.0);
    assert_eq!(front.2, rear.2);
    assert!(Vec3::from_array(front.1).distance(axes.x_axis) < 0.00001);
    assert!(Vec3::from_array(front.1).dot(Vec3::from_array(rear.1)) < -0.99999);
  }

  #[test]
  fn original_normal_preset_has_horizontal_follow_and_downward_pitch() {
    let (eye, axes) = target(PRESETS[0], Vec3::ZERO, Vec2::X);
    assert!((eye.x + 19.923893).abs() < 0.00001);
    assert!((eye.z - 13.214644).abs() < 0.00001);
    assert!(axes.z_axis.z < 0.0);
    assert!(axes.is_finite());
  }

  #[test]
  fn driving_turn_keeps_chase_pose_finite_across_heading_wrap_and_restart() {
    let mut rig = ChaseRig::new([0.0; 3], [1.0, 0.0, 0.0]);
    for i in 0..2000 {
      let angle = i as f32 * 0.01;
      rig.update(
        [i as f32, 10.0, angle.sin() * 5.0],
        [angle.cos(), angle.sin(), 0.0],
        1.0,
        0.01,
      );
      let (eye, forward, up) = rig.pose();
      assert!(eye.iter().chain(&forward).chain(&up).all(|v| v.is_finite()));
      assert!(Vec3::from_array(forward).dot(Vec3::from_array(up)).abs() < 0.00001);
    }
    let restarted = ChaseRig::new([0.0; 3], [1.0, 0.0, 0.0]);
    assert_eq!(
      restarted.pose(),
      ChaseRig::new([0.0; 3], [1.0, 0.0, 0.0]).pose()
    );
  }
}
