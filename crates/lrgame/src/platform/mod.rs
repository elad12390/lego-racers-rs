//! Game-facing presentation boundary backed exclusively by Bevy.
//! The original application state machines submit batches; Bevy owns the window,
//! GPU resources, cameras, input, sound entities and asynchronous readback.
pub mod audio;
mod backend;
mod draw;
mod input;
mod material;
mod mesh_cache;
mod performance;
pub mod rand;
mod readback;
mod renderer;
mod state;
mod target;
mod types;

pub use backend::run;
#[cfg(test)]
pub(crate) use backend::{offscreen_test_app, submit_test_frame};
pub use input::*;
#[cfg(test)]
pub(crate) use state::with as test_state;
pub use types::*;

pub mod prelude {
  pub use super::{draw::*, input::*, rand, types::*};
  pub use bevy::math::{vec2, vec3, vec4, Mat3, Mat4, Quat, Vec2, Vec3, Vec4};
}

pub async fn screen_data() -> Image {
  struct Readback;
  impl std::future::Future for Readback {
    type Output = Image;
    fn poll(
      self: std::pin::Pin<&mut Self>,
      _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Image> {
      state::with(|s| {
        if let Some(image) = s.readback.take() {
          return std::task::Poll::Ready(image);
        }
        s.capture_requested = true;
        std::task::Poll::Pending
      })
    }
  }
  Readback.await
}
