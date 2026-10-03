use super::state;
pub use bevy::input::mouse::MouseButton;
use std::{
  collections::HashSet,
  future::Future,
  pin::Pin,
  task::{Context, Poll},
};
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum KeyCode {
  Up,
  Down,
  Left,
  Right,
  W,
  A,
  S,
  D,
  Q,
  E,
  F,
  R,
  C,
  V,
  N,
  PageDown,
  PageUp,
  Enter,
  Escape,
  Space,
  Tab,
  Backspace,
  F5,
  F9,
  LeftShift,
  RightShift,
  LeftControl,
  RightControl,
  Key1,
  Key2,
  Key3,
  Key4,
  Key5,
}
pub fn is_key_down(k: KeyCode) -> bool {
  state::with(|s| s.down.contains(&k))
}
pub fn is_key_pressed(k: KeyCode) -> bool {
  state::with(|s| s.pressed.contains(&k))
}
pub fn get_keys_down() -> HashSet<KeyCode> {
  state::with(|s| s.down.clone())
}
pub fn get_keys_pressed() -> HashSet<KeyCode> {
  state::with(|s| s.pressed.clone())
}
pub fn get_keys_released() -> HashSet<KeyCode> {
  state::with(|s| s.released.clone())
}
pub fn is_mouse_button_down(k: MouseButton) -> bool {
  state::with(|s| s.mouse_down.contains(&k))
}
pub fn is_mouse_button_pressed(k: MouseButton) -> bool {
  state::with(|s| s.mouse_pressed.contains(&k))
}
pub fn is_mouse_button_released(k: MouseButton) -> bool {
  state::with(|s| s.mouse_released.contains(&k))
}
pub fn mouse_position() -> (f32, f32) {
  state::with(|s| s.mouse)
}
pub fn mouse_delta_position() -> bevy::math::Vec2 {
  state::with(|s| s.mouse_delta)
}
pub fn clear_input_queue() {
  state::with(|s| {
    s.pressed.clear();
    s.released.clear();
    s.mouse_pressed.clear();
    s.mouse_released.clear();
    s.chars.clear();
  });
}
pub fn get_char_pressed() -> Option<char> {
  state::with(|s| s.chars.pop_front())
}
pub fn get_frame_time() -> f32 {
  state::with(|s| s.dt)
}
pub fn get_time() -> f64 {
  state::with(|s| s.time)
}
pub fn screen_width() -> f32 {
  state::with(|s| s.width)
}
pub fn screen_height() -> f32 {
  state::with(|s| s.height)
}
pub fn is_quit_requested() -> bool {
  state::with(|s| s.quit)
}
pub fn focused() -> bool {
  state::with(|s| s.focused)
}
pub async fn next_frame() {
  struct Next(Option<u64>);
  impl Future for Next {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<()> {
      state::with(|s| match self.0 {
        Some(frame) if frame < s.frame => {
          s.batches.clear();
          s.transforms.truncate(1);
          s.camera = None;
          s.material = None;
          Poll::Ready(())
        }
        Some(_) => Poll::Pending,
        None => {
          self.0 = Some(s.frame);
          Poll::Pending
        }
      })
    }
  }
  Next(None).await
}

use bevy::{prelude::*, window::PrimaryWindow};
#[derive(Resource, Default)]
pub struct InputEvents {
  keyboard: bevy::ecs::message::MessageCursor<bevy::input::keyboard::KeyboardInput>,
}
fn keys(key: bevy::input::keyboard::KeyCode) -> Option<KeyCode> {
  use self::KeyCode as K;
  use bevy::input::keyboard::KeyCode as B;
  Some(match key {
    B::ArrowUp => K::Up,
    B::ArrowDown => K::Down,
    B::ArrowLeft => K::Left,
    B::ArrowRight => K::Right,
    B::KeyW => K::W,
    B::KeyA => K::A,
    B::KeyS => K::S,
    B::KeyD => K::D,
    B::KeyQ => K::Q,
    B::KeyE => K::E,
    B::KeyF => K::F,
    B::KeyR => K::R,
    B::KeyC => K::C,
    B::KeyV => K::V,
    B::KeyN => K::N,
    B::PageDown => K::PageDown,
    B::PageUp => K::PageUp,
    B::Enter => K::Enter,
    B::Escape => K::Escape,
    B::Space => K::Space,
    B::Tab => K::Tab,
    B::Backspace => K::Backspace,
    B::F5 => K::F5,
    B::F9 => K::F9,
    B::ShiftLeft => K::LeftShift,
    B::ShiftRight => K::RightShift,
    B::ControlLeft => K::LeftControl,
    B::ControlRight => K::RightControl,
    B::Digit1 => K::Key1,
    B::Digit2 => K::Key2,
    B::Digit3 => K::Key3,
    B::Digit4 => K::Key4,
    B::Digit5 => K::Key5,
    _ => return None,
  })
}
pub fn sync(world: &mut World) {
  let input = world.resource::<ButtonInput<bevy::input::keyboard::KeyCode>>();
  let (down, pressed, released) = (
    input.get_pressed().filter_map(|k| keys(*k)).collect(),
    input.get_just_pressed().filter_map(|k| keys(*k)).collect(),
    input.get_just_released().filter_map(|k| keys(*k)).collect(),
  );
  let mouse = world.resource::<ButtonInput<MouseButton>>();
  let (md, mp, mr) = (
    mouse.get_pressed().copied().collect(),
    mouse.get_just_pressed().copied().collect(),
    mouse.get_just_released().copied().collect(),
  );
  let time = world.resource::<Time>();
  let (dt, elapsed) = (time.delta_secs(), time.elapsed_secs_f64());
  let window = world
    .query_filtered::<&Window, With<PrimaryWindow>>()
    .single(world)
    .ok();
  let window = window.map(|w| {
    (
      w.width(),
      w.height(),
      w.resolution.scale_factor(),
      w.focused,
      w.cursor_position(),
    )
  });
  state::with(|s| {
    s.down = down;
    s.pressed = pressed;
    s.released = released;
    s.mouse_down = md;
    s.mouse_pressed = mp;
    s.mouse_released = mr;
    s.dt = dt;
    s.time = elapsed;
    s.frame += 1;
    s.mouse_delta = Vec2::ZERO;
    if let Some((w, h, scale, focused, cursor)) = window {
      s.width = w;
      s.height = h;
      s.scale = scale;
      s.focused = focused;
      if let Some(cursor) = cursor {
        s.mouse_delta = cursor - Vec2::from(s.mouse);
        s.mouse = (cursor.x, cursor.y);
      }
    }
  });
  world.resource_scope(|world, mut events: Mut<InputEvents>| {
    for message in events
      .keyboard
      .read(world.resource::<Messages<bevy::input::keyboard::KeyboardInput>>())
    {
      if message.state.is_pressed() {
        if let Some(text) = &message.text {
          state::with(|s| s.chars.extend(text.chars()));
        }
      }
    }
  });
}

#[cfg(test)]
mod tests {
  use super::*;
  use bevy::input::{
    keyboard::{Key, KeyCode as NativeKey, KeyboardFocusLost, KeyboardInput},
    ButtonState,
  };
  #[test]
  fn bevy_boundary_native_input_messages_preserve_text_retina_and_focus_release() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::input::InputPlugin));
    app.init_resource::<InputEvents>();
    app.add_systems(Update, sync);
    let mut window = Window {
      focused: true,
      ..default()
    };
    window.resolution.set_scale_factor_override(Some(2.0));
    window.resolution.set(1000.0, 760.0);
    window.set_cursor_position(Some(Vec2::new(100.0, 200.0)));
    let entity = app.world_mut().spawn((window, PrimaryWindow)).id();
    state::with(|s| {
      s.chars.clear();
      s.mouse = (0.0, 0.0);
    });
    let press = KeyboardInput {
      key_code: NativeKey::KeyW,
      logical_key: Key::Character("é".into()),
      state: ButtonState::Pressed,
      text: Some("é".into()),
      repeat: false,
      window: entity,
    };
    app.world_mut().write_message(press.clone());
    app.update();
    assert!(is_key_down(super::KeyCode::W));
    assert!(is_key_pressed(super::KeyCode::W));
    assert_eq!(get_char_pressed(), Some('é'));
    assert_eq!((screen_width(), screen_height()), (1000.0, 760.0));
    assert_eq!(mouse_position(), (100.0, 200.0));
    assert_eq!(state::with(|s| s.scale), 2.0);
    assert!(focused());
    app.world_mut().get_mut::<Window>(entity).unwrap().focused = false;
    app.world_mut().write_message(KeyboardFocusLost);
    app.update();
    assert!(!focused());
    assert!(!is_key_down(super::KeyCode::W));
    app.world_mut().get_mut::<Window>(entity).unwrap().focused = true;
    app.update();
    assert!(focused());
    assert!(!is_key_down(super::KeyCode::W));
    app.world_mut().write_message(press);
    app.update();
    assert!(is_key_down(super::KeyCode::W));
    app.world_mut().write_message(KeyboardInput {
      key_code: NativeKey::KeyW,
      logical_key: Key::Character("é".into()),
      state: ButtonState::Released,
      text: None,
      repeat: false,
      window: entity,
    });
    app.update();
    assert!(!is_key_down(super::KeyCode::W));
    assert!(get_keys_released().contains(&super::KeyCode::W));
  }
}
