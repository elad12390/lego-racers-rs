//! Diagnostic camera, not the original race camera or a gameplay system.
use crate::platform::prelude::*;

use crate::gpu::TrackGpu;

pub struct PreviewCamera {
  pub azimuth: f32,
  pub elevation: f32,
  pub distance_scale: f32,
}

impl Default for PreviewCamera {
  fn default() -> Self {
    Self {
      azimuth: 0.0,
      elevation: 75.0_f32.to_radians(),
      distance_scale: 1.5,
    }
  }
}

impl PreviewCamera {
  pub fn apply(&mut self, yaw: f32, tilt: f32, zoom: f32) {
    self.azimuth += yaw;
    self.elevation = (self.elevation + tilt).clamp(10.0_f32.to_radians(), 85.0_f32.to_radians());
    self.distance_scale = (self.distance_scale * zoom.exp()).clamp(0.35, 3.0);
  }

  pub fn native(&self, track: &TrackGpu) -> Camera3D {
    let distance = track.radius * self.distance_scale;
    let horizontal = distance * self.elevation.cos();
    let offset = vec3(
      horizontal * self.azimuth.sin(),
      distance * self.elevation.sin(),
      horizontal * self.azimuth.cos(),
    );
    Camera3D {
      position: track.center + offset,
      target: track.center,
      up: Vec3::Y,
      fovy: 60.0_f32.to_radians(),
      z_near: 0.1,
      z_far: track.radius * 10.0,
      ..Default::default()
    }
  }
}
