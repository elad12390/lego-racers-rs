//! Actual editor input paths for original panel repeat and focus-loss cancellation.
use crate::{
  custom_driver::Data,
  editor_gpu_test::{capture_editor_frame, poll, send, send_mouse},
  game_catalog::Catalog,
  menu_ui::MenuUi,
  platform,
  profile::Profile,
};
use bevy::{input::ButtonState, prelude::*};
use lrformats::library::Library;
use std::{path::Path, task::Poll};

pub fn verify(
  app: &mut App,
  window: Entity,
  library: &Library,
  catalog: &Catalog,
  ui: &MenuUi,
  profile: &Profile,
  output: &Path,
) {
  let original = profile.custom_driver.clone().unwrap_or_default();
  let before = serde_json::to_vec(profile).unwrap();
  let saved = std::fs::read(output.join("profile.json")).unwrap();
  let data = Data::load(library).unwrap();
  let choices = data
    .row(1)
    .iter()
    .enumerate()
    .filter(|(i, p)| profile.driver_part_allowed(p.unlock, catalog) || *i == original.face)
    .map(|(i, _)| i)
    .collect::<Vec<_>>();
  let start = choices.iter().position(|i| *i == original.face).unwrap();
  let arrow = ui.driver_row_arrow_bounds(1, true).unwrap();
  let mut cases = Vec::new();
  for mouse in [false, true] {
    for cancel in [false, true] {
      send_mouse(app, window, Vec2::new(600.0, 460.0), ButtonState::Released);
      for key in [
        KeyCode::ArrowLeft,
        KeyCode::ArrowRight,
        KeyCode::ArrowDown,
        KeyCode::Enter,
        KeyCode::Escape,
      ] {
        send(app, window, key, ButtonState::Released, None);
      }
      platform::test_state(|s| {
        s.dt = 0.0;
        s.audio_commands.clear();
      });
      let mut editor = std::pin::pin!(crate::driver_editor::run(
        library, profile, catalog, ui, None
      ));
      assert!(poll(editor.as_mut()).is_pending());
      send(app, window, KeyCode::ArrowDown, ButtonState::Pressed, None);
      platform::test_state(|s| s.dt = 0.0);
      assert!(poll(editor.as_mut()).is_pending());
      send(app, window, KeyCode::ArrowDown, ButtonState::Released, None);
      platform::test_state(|s| {
        s.dt = 0.0;
        s.audio_commands.clear();
      });
      assert!(poll(editor.as_mut()).is_pending());
      if mouse {
        send_mouse(
          app,
          window,
          Vec2::new(arrow.x + arrow.w * 0.5, arrow.y + arrow.h * 0.5),
          ButtonState::Pressed,
        );
      } else {
        send(app, window, KeyCode::ArrowRight, ButtonState::Pressed, None);
      }
      platform::test_state(|s| s.dt = 0.0);
      assert!(poll(editor.as_mut()).is_pending());
      assert_eq!(
        platform::test_state(|s| std::mem::take(&mut s.audio_commands).len()),
        1
      );
      if !mouse && !cancel {
        crate::capture::save_frame(
          &output.join("driver-key-repeat-before-focus.png"),
          &capture_editor_frame(app),
        )
        .unwrap();
      }
      // Mouse capture owns the direction: a simultaneously pressed Left must
      // not replace its pending Right, even when the current animation is busy.
      if mouse {
        send(app, window, KeyCode::ArrowLeft, ButtonState::Pressed, None);
        platform::test_state(|s| s.dt = 0.0);
        assert!(poll(editor.as_mut()).is_pending());
        assert!(platform::test_state(|s| s.audio_commands.is_empty()));
      }
      for (dt, sounds) in [(0.4, 0), (0.0, 0), (0.0, 1)] {
        app.update();
        platform::sync(app.world_mut());
        platform::test_state(|s| s.dt = dt);
        assert!(poll(editor.as_mut()).is_pending());
        assert_eq!(
          platform::test_state(|s| std::mem::take(&mut s.audio_commands).len()),
          sounds,
          "mouse and keyboard capture must both repeat after the same completion gate"
        );
      }
      // Move focus while the second scroll is active and the arrow is held.
      send(app, window, KeyCode::ArrowDown, ButtonState::Pressed, None);
      platform::test_state(|s| s.dt = 0.0);
      assert!(poll(editor.as_mut()).is_pending());
      assert_eq!(
        platform::test_state(|s| std::mem::take(&mut s.audio_commands).len()),
        1
      );
      send(app, window, KeyCode::ArrowDown, ButtonState::Released, None);
      platform::test_state(|s| s.dt = 0.0);
      assert!(poll(editor.as_mut()).is_pending());
      for dt in [0.4, 0.0, 0.0, 0.4, 0.0, 0.0] {
        app.update();
        platform::sync(app.world_mut());
        platform::test_state(|s| s.dt = dt);
        assert!(poll(editor.as_mut()).is_pending());
        assert!(
          platform::test_state(|s| s.audio_commands.is_empty()),
          "focus loss must stop arrow repeats"
        );
      }
      if !mouse && !cancel {
        crate::capture::save_frame(
          &output.join("driver-key-repeat-after-focus.png"),
          &capture_editor_frame(app),
        )
        .unwrap();
      }
      if mouse {
        send_mouse(app, window, Vec2::new(600.0, 460.0), ButtonState::Released);
        platform::test_state(|s| s.dt = 0.0);
        assert!(poll(editor.as_mut()).is_pending());
      }
      send(
        app,
        window,
        if mouse {
          KeyCode::ArrowLeft
        } else {
          KeyCode::ArrowRight
        },
        ButtonState::Released,
        None,
      );
      platform::test_state(|s| s.dt = 0.0);
      assert!(poll(editor.as_mut()).is_pending());
      send(
        app,
        window,
        if cancel {
          KeyCode::Escape
        } else {
          KeyCode::Enter
        },
        ButtonState::Pressed,
        None,
      );
      let Poll::Ready(Ok(result)) = poll(editor.as_mut()) else {
        panic!("capture/focus editor did not exit")
      };
      let mut expected = original.clone();
      expected.face = choices[(start + 2) % choices.len()];
      assert_eq!(result, if cancel { None } else { Some(expected) });
      cases.push(serde_json::json!({"mouse_capture":mouse,"cancel":cancel,"accepted_moves":2,"draft":result}));
      assert_eq!(serde_json::to_vec(profile).unwrap(), before);
      assert_eq!(std::fs::read(output.join("profile.json")).unwrap(), saved);
    }
  }
  for key in [KeyCode::Enter, KeyCode::Escape] {
    send(app, window, key, ButtonState::Released, None);
  }
  std::fs::write(
    output.join("driver-capture-focus.json"),
    serde_json::to_vec_pretty(&serde_json::json!({
    "mode":"scripted_offscreen_native_editor_not_physical_audible_or_game_acceptance",
    "keyboard_and_mouse_repeat":true,"focus_loss_cancels":true,"cases":cases}))
    .unwrap(),
  )
  .unwrap();
}
