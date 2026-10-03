//! Native projection defaults from App0042b9d0 and GolDP1001bfc0.
use crate::platform::prelude::*;

// GolDP multiplies the angle by pi/360 before tan, then width by aspect.
// The original angle is therefore vertical degrees, not horizontal or radians.
pub const FOV_DEGREES: f32 = 65.0;
pub const NEAR: f32 = 5.0;
pub const FAR: f32 = 800.0;

/// Small modern presentation-only suspension kick. Never feeds back into the
/// source-derived chase rig or vehicle physics, and leaves original FOV intact.
#[derive(Default)]
pub struct SuspensionKick {
  remaining: f32,
  strength: f32,
}
impl SuspensionKick {
  pub fn advance(&mut self, dt: f32, impulse: f32) {
    if !dt.is_finite() || dt <= 0.0 {
      return;
    }
    self.remaining = (self.remaining - dt).max(0.0);
    if impulse > 0.0 && self.remaining == 0.0 {
      self.remaining = 0.25;
      self.strength = impulse.clamp(0.0, 1.0);
    }
  }
  pub fn apply(&self, view: &mut Camera3D) {
    let phase = (0.25 - self.remaining) / 0.25;
    let kick = (phase * std::f32::consts::TAU * 1.5).sin()
      * (1.0 - phase).max(0.0).powi(2)
      * self.strength
      * 0.8;
    // Translate eye and target together: no horizon roll or changed steering.
    view.position += view.up * kick;
    view.target += view.up * kick;
  }
}

/// Shared by native rendering and the original-frustum projection adapter.
pub fn view(eye: [f32; 3], forward: [f32; 3], up: [f32; 3], aspect: f32) -> Camera3D {
  let native = |[x, y, z]: [f32; 3]| vec3(x, z, -y);
  let position = native(eye);
  Camera3D {
    position,
    target: position + native(forward),
    up: native(up),
    aspect: Some(aspect),
    fovy: FOV_DEGREES.to_radians(),
    z_near: NEAR,
    z_far: FAR,
    ..Default::default()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn suspension_kick_preserves_optical_direction_and_settles() {
    let original = view([0.0, 0.0, 15.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], 1.5);
    let mut kick = SuspensionKick::default();
    kick.advance(0.016, 1.0);
    kick.advance(0.016, 0.0);
    let mut bumped = original.clone();
    kick.apply(&mut bumped);
    assert!(bumped.position.distance(original.position) > 0.0);
    assert!(
      (bumped.target - bumped.position - (original.target - original.position)).length() < 0.00001
    );
    kick.advance(0.25, 0.0);
    let mut settled = original.clone();
    kick.apply(&mut settled);
    assert_eq!(settled.position, original.position);
  }
}
