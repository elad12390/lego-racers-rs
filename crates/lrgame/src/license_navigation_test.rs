//! Scripted actual editor/InputPlugin/Metal keyboard focus, not OS input.
use crate::editor_gpu_test::{capture_editor_frame, logical_pixel, poll, send, send_mouse};
use crate::{menu_ui::MenuUi, platform, profile::Profile};
use bevy::{input::ButtonState, prelude::*};
use lrformats::library::Library;
use std::task::Poll;

fn assert_focus(image: &platform::Image, ui: &MenuUi, expected: &str) {
  assert_eq!(
    logical_pixel(image, Vec2::new(114.0, 200.0)) == [8, 8, 115, 255],
    expected == "name",
    "name frame must follow keyboard focus"
  );
  for (widget, label, style) in [
    ("swapface", 0x3b, "nubutton"),
    ("gonext", 0xb, "buttonra"),
    ("goback", 9, "buttonla"),
  ] {
    let rect = ui.license_action_bounds(widget, label, style).unwrap();
    let yellow = (rect.y as i32..(rect.y + rect.h) as i32)
      .flat_map(|y| {
        (rect.x as i32..(rect.x + rect.w) as i32)
          .map(move |x| Vec2::new(x as f32 + 0.5, y as f32 + 0.5))
      })
      // FONT_THS glyphs are authored yellow, not white masks: the highlighted
      // untinted interior is RGB246/230/6, while idle MSB tint dims that color.
      .filter(|p| logical_pixel(image, *p) == [246, 230, 6, 255])
      .count();
    assert_eq!(
      yellow > 20,
      widget == expected,
      "wrong highlighted control {widget}"
    );
  }
}

pub fn verify(
  app: &mut App,
  window: Entity,
  library: &Library,
  ui: &MenuUi,
  profile: &Profile,
  output: &std::path::Path,
) {
  send(app, window, KeyCode::Escape, ButtonState::Released, None);
  let mut draft = profile.clone();
  draft.name = "KEYBOARD".into();
  let before = serde_json::to_vec(&draft).unwrap();
  {
    let mut editor = std::pin::pin!(crate::license_editor::run(library, &mut draft, ui, None));
    assert!(poll(editor.as_mut()).is_pending());
    for (key, shift, order) in [
      (KeyCode::Tab, None, ["swapface", "gonext", "goback", "name"]),
      (
        KeyCode::Tab,
        Some(KeyCode::ShiftLeft),
        ["goback", "gonext", "swapface", "name"],
      ),
      (
        KeyCode::Tab,
        Some(KeyCode::ShiftRight),
        ["goback", "gonext", "swapface", "name"],
      ),
      (
        KeyCode::ArrowDown,
        None,
        ["swapface", "gonext", "goback", "name"],
      ),
      (
        KeyCode::ArrowUp,
        None,
        ["goback", "gonext", "swapface", "name"],
      ),
    ] {
      if let Some(shift) = shift {
        send(app, window, shift, ButtonState::Pressed, None);
        assert!(poll(editor.as_mut()).is_pending());
      }
      for expected in order {
        platform::test_state(|s| s.audio_commands.clear());
        send(app, window, key, ButtonState::Pressed, None);
        assert!(poll(editor.as_mut()).is_pending());
        assert_eq!(
          platform::test_state(|s| std::mem::take(&mut s.audio_commands).len()),
          1,
          "navigation must emit one enabled movement effect"
        );
        let image = capture_editor_frame(app);
        crate::capture::save_frame(
          &output.join(format!(
            "license-navigation-{key:?}-{shift:?}-{expected}.png"
          )),
          &image,
        )
        .unwrap();
        assert_focus(&image, ui, expected);
        if key == KeyCode::Tab && shift.is_none() && expected == "swapface" {
          crate::capture::save_frame(&output.join("license-tab-snapshot.png"), &image).unwrap();
        }
        if key == KeyCode::Tab && shift == Some(KeyCode::ShiftLeft) && expected == "goback" {
          crate::capture::save_frame(&output.join("license-shift-tab-driver.png"), &image).unwrap();
        }
        app.update();
        platform::sync(app.world_mut());
        assert!(poll(editor.as_mut()).is_pending());
        assert!(
          platform::test_state(|s| s.audio_commands.is_empty()),
          "held navigation repeated"
        );
        if expected != "name" {
          send(
            app,
            window,
            KeyCode::KeyZ,
            ButtonState::Pressed,
            Some("lost"),
          );
          assert!(poll(editor.as_mut()).is_pending());
          send(app, window, KeyCode::KeyZ, ButtonState::Released, None);
          assert!(poll(editor.as_mut()).is_pending());
          send(app, window, KeyCode::Backspace, ButtonState::Pressed, None);
          assert!(poll(editor.as_mut()).is_pending());
          send(app, window, KeyCode::Backspace, ButtonState::Released, None);
          assert!(poll(editor.as_mut()).is_pending());
        }
        send(app, window, key, ButtonState::Released, None);
        assert!(poll(editor.as_mut()).is_pending());
      }
      if let Some(shift) = shift {
        send(app, window, shift, ButtonState::Released, None);
        assert!(poll(editor.as_mut()).is_pending());
      }
    }
    // Changing focus cancels a held operation: releasing the old button cannot
    // photograph a face after keyboard navigation unhighlighted it.
    send_mouse(app, window, Vec2::new(4.0, 350.0), ButtonState::Pressed);
    assert!(poll(editor.as_mut()).is_pending());
    send(app, window, KeyCode::ArrowDown, ButtonState::Pressed, None);
    assert!(poll(editor.as_mut()).is_pending());
    assert_focus(&capture_editor_frame(app), ui, "gonext");
    send(app, window, KeyCode::ArrowDown, ButtonState::Released, None);
    assert!(poll(editor.as_mut()).is_pending());
    send_mouse(app, window, Vec2::new(4.0, 350.0), ButtonState::Released);
    assert!(poll(editor.as_mut()).is_pending());
    assert!(!platform::test_state(
      |s| s.capture_requested || s.capture_inflight
    ));
    send(app, window, KeyCode::Escape, ButtonState::Pressed, None);
    assert!(matches!(poll(editor.as_mut()), Poll::Ready(Ok(false))));
  }
  assert_eq!(
    serde_json::to_vec(&draft).unwrap(),
    before,
    "keyboard cancel changed draft/photo"
  );
  send(app, window, KeyCode::Escape, ButtonState::Released, None);
  // Confirm from the name field verifies ignored off-field text/backspace did
  // not silently corrupt the committed name, independently of cancellation.
  {
    let mut editor = std::pin::pin!(crate::license_editor::run(library, &mut draft, ui, None));
    assert!(poll(editor.as_mut()).is_pending());
    send(app, window, KeyCode::ArrowDown, ButtonState::Pressed, None);
    assert!(poll(editor.as_mut()).is_pending());
    send(app, window, KeyCode::ArrowDown, ButtonState::Released, None);
    assert!(poll(editor.as_mut()).is_pending());
    send(
      app,
      window,
      KeyCode::KeyZ,
      ButtonState::Pressed,
      Some("lost"),
    );
    assert!(poll(editor.as_mut()).is_pending());
    send(app, window, KeyCode::KeyZ, ButtonState::Released, None);
    assert!(poll(editor.as_mut()).is_pending());
    send(app, window, KeyCode::Backspace, ButtonState::Pressed, None);
    assert!(poll(editor.as_mut()).is_pending());
    send(app, window, KeyCode::Backspace, ButtonState::Released, None);
    assert!(poll(editor.as_mut()).is_pending());
    send(app, window, KeyCode::ArrowUp, ButtonState::Pressed, None);
    assert!(poll(editor.as_mut()).is_pending());
    send(app, window, KeyCode::ArrowUp, ButtonState::Released, None);
    assert!(poll(editor.as_mut()).is_pending());
    send(app, window, KeyCode::KeyX, ButtonState::Pressed, Some("x"));
    assert!(poll(editor.as_mut()).is_pending());
    send(app, window, KeyCode::KeyX, ButtonState::Released, None);
    assert!(poll(editor.as_mut()).is_pending());
    send(app, window, KeyCode::Enter, ButtonState::Pressed, None);
    assert!(matches!(poll(editor.as_mut()), Poll::Ready(Ok(true))));
  }
  assert_eq!(draft.name, "KEYBOARDX");
  assert!(draft.license_photo == profile.license_photo);
  assert_eq!(draft.license_expression, profile.license_expression);
  send(app, window, KeyCode::Enter, ButtonState::Released, None);
}
