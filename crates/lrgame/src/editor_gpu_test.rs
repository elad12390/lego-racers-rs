//! Shared real offscreen Bevy/Metal input/readback helpers for editor checks.
use crate::{menu_ui::MenuUi, platform};
use bevy::{
  input::{
    keyboard::{Key, KeyboardInput},
    ButtonState,
  },
  prelude::*,
};
use std::{
  future::Future,
  pin::Pin,
  task::{Context, Poll, Waker},
  time::{Duration, Instant},
};

pub fn poll<T>(future: Pin<&mut impl Future<Output = T>>) -> Poll<T> {
  future.poll(&mut Context::from_waker(Waker::noop()))
}
pub fn ready<T>(future: impl Future<Output = Result<T, String>>) -> T {
  let mut future = std::pin::pin!(future);
  let Poll::Ready(Ok(value)) = poll(future.as_mut()) else {
    panic!("original asset load yielded or failed")
  };
  value
}
pub fn send(
  app: &mut App,
  window: Entity,
  key_code: KeyCode,
  state: ButtonState,
  text: Option<&str>,
) {
  app.world_mut().write_message(KeyboardInput {
    key_code,
    state,
    text: text.map(Into::into),
    repeat: false,
    window,
    logical_key: Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified),
  });
  app.update();
  platform::sync(app.world_mut());
}
pub fn send_mouse(app: &mut App, window: Entity, logical: Vec2, state: ButtonState) {
  let position = MenuUi::origin() + logical * MenuUi::scale();
  app
    .world_mut()
    .get_mut::<Window>(window)
    .unwrap()
    .set_cursor_position(Some(position));
  app
    .world_mut()
    .write_message(bevy::input::mouse::MouseButtonInput {
      button: bevy::input::mouse::MouseButton::Left,
      state,
      window,
    });
  app.update();
  platform::sync(app.world_mut());
}
/// Pointer motion only: no synthetic button down/release accompanies hover.
pub fn move_mouse(app: &mut App, window: Entity, logical: Vec2) {
  let position = MenuUi::origin() + logical * MenuUi::scale();
  app
    .world_mut()
    .get_mut::<Window>(window)
    .unwrap()
    .set_cursor_position(Some(position));
  app.update();
  platform::sync(app.world_mut());
}
pub fn capture_editor_frame(app: &mut App) -> platform::Image {
  // Hold application advancement through the same real asynchronous readback.
  let mut capture = std::pin::pin!(crate::capture::screen_data());
  assert!(poll(capture.as_mut()).is_pending());
  let deadline = Instant::now() + Duration::from_secs(30);
  loop {
    platform::submit_test_frame(app);
    app.update();
    // Like the application-owned Snapshot test, consume only after the GPU
    // observer supplies the image. Re-polling an unavailable screen_data future
    // would enqueue a second capture request while the first is still in flight.
    if platform::test_state(|s| s.readback.is_some()) {
      let Poll::Ready(image) = poll(capture.as_mut()) else {
        panic!("ready GPU image was not delivered")
      };
      return image;
    }
    assert!(Instant::now() < deadline, "editor-frame readback timed out");
    std::thread::sleep(Duration::from_millis(5));
  }
}
pub fn logical_pixel(image: &platform::Image, point: Vec2) -> [u8; 4] {
  let screen = MenuUi::origin() + point * MenuUi::scale();
  let x = (screen.x * image.width as f32 / platform::prelude::screen_width()).floor() as usize;
  let y = (screen.y * image.height as f32 / platform::prelude::screen_height()).floor() as usize;
  let start = ((image.height as usize - 1 - y) * image.width as usize + x) * 4;
  image.bytes[start..start + 4].try_into().unwrap()
}
