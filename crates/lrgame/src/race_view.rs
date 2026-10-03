//! Mirrored races use canonical simulation coordinates and a reflected world.
//! Cars retain right-handed local geometry, rather than reversing skin lettering.
use crate::platform::prelude::*;

#[derive(Clone, Copy)]
pub struct RaceView {
  pub mirrored: bool,
}
impl RaceView {
  pub fn original(&self, mut value: [f32; 3]) -> [f32; 3] {
    if self.mirrored {
      value[1] = -value[1];
    }
    value
  }
  pub fn native(&self, value: [f32; 3]) -> Vec3 {
    crate::gpu::world_position(self.original(value), 1.0)
  }
  pub fn world_matrix(&self) -> Mat4 {
    Mat4::from_scale(vec3(1.0, 1.0, if self.mirrored { -1.0 } else { 1.0 }))
  }
  pub fn car(&self, position: [f32; 3], forward: [f32; 3], up: [f32; 3]) -> Mat4 {
    let forward = self.native(forward);
    let up = self.native(up);
    Mat4::from_cols(
      forward.extend(0.0),
      up.extend(0.0),
      forward.cross(up).extend(0.0),
      self.native(position).extend(1.0),
    )
  }
}
