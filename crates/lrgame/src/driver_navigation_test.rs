//! Real editor/InputPlugin/Metal focus checks; scripted, not physical input.
use crate::{
  custom_driver::Build,
  editor_gpu_test::{capture_editor_frame, logical_pixel, poll, send, send_mouse},
  game_catalog::Catalog,
  menu_ui::MenuUi,
  platform,
  profile::Profile,
};
use bevy::{input::ButtonState, prelude::*};
use lrformats::{bmp, library::Library};
use std::task::Poll;

fn assert_focus(image: &platform::Image, ui: &MenuUi, expected: usize, library: &Library) {
  assert_eq!(
    logical_pixel(image, Vec2::new(320.0, 90.0)),
    [0, 0, 55, 255],
    "focus navigation must preserve preview panel"
  );
  assert_eq!(
    logical_pixel(image, Vec2::new(382.0, 398.0)),
    [255, 255, 255, 255],
    "focus navigation must preserve preview stand"
  );
  let bitmap = bmp::decode(library.find_in("arrowls.BMP", "MENUDATA").unwrap()).unwrap();
  let bytes = bitmap.to_rgba();
  let (index, pixel) = bytes
    .chunks_exact(4)
    .enumerate()
    .find(|(_, p)| p[0] > 200 && p[1] > 150 && p[2] < 30)
    .unwrap();
  for row in 0..4 {
    let rect = ui.driver_row_arrow_bounds(row, false).unwrap();
    let point = Vec2::new(
      rect.x + (index % bitmap.width as usize) as f32 + 0.5,
      rect.y + (index / bitmap.width as usize) as f32 + 0.5,
    );
    assert_eq!(
      logical_pixel(image, point) == pixel,
      expected == row,
      "wrong highlighted driver row {row}"
    );
  }
  for (index, widget, label, style) in [
    (4, "mix", 0x38, "nubutton"),
    (5, "gonext", 10, "buttonra"),
    (6, "goback", 31, "buttonca"),
  ] {
    let rect = ui.driver_action_bounds(widget, label, style).unwrap();
    let count = (rect.y as i32..(rect.y + rect.h) as i32)
      .flat_map(|y| {
        (rect.x as i32..(rect.x + rect.w) as i32)
          .map(move |x| Vec2::new(x as f32 + 0.5, y as f32 + 0.5))
      })
      .filter(|p| logical_pixel(image, *p) == [246, 230, 6, 255])
      .count();
    assert_eq!(
      count > 20,
      index == expected,
      "wrong highlighted driver action {widget}"
    );
  }
}

pub fn verify(
  app: &mut App,
  window: Entity,
  library: &Library,
  catalog: &Catalog,
  ui: &MenuUi,
  profile: &Profile,
  output: &std::path::Path,
) {
  send_mouse(app, window, Vec2::new(600.0, 460.0), ButtonState::Released);
  let before = serde_json::to_vec(profile).unwrap();
  let actual = {
    let mut editor = std::pin::pin!(crate::driver_editor::run(
      library, profile, catalog, ui, None
    ));
    assert!(poll(editor.as_mut()).is_pending());
    let initial = capture_editor_frame(app);
    assert_focus(&initial, ui, 0, library);
    crate::capture::save_frame(&output.join("driver-initial-hat-focus.png"), &initial).unwrap();
    for (key, shift, order) in [
      (KeyCode::Tab, None, [1, 2, 3, 4, 5, 6, 0]),
      (
        KeyCode::Tab,
        Some(KeyCode::ShiftLeft),
        [6, 5, 4, 3, 2, 1, 0],
      ),
      (
        KeyCode::Tab,
        Some(KeyCode::ShiftRight),
        [6, 5, 4, 3, 2, 1, 0],
      ),
      (KeyCode::ArrowDown, None, [1, 2, 3, 4, 5, 6, 0]),
      (KeyCode::ArrowUp, None, [6, 5, 4, 3, 2, 1, 0]),
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
          1
        );
        let image = capture_editor_frame(app);
        crate::capture::save_frame(
          &output.join(format!(
            "driver-navigation-{key:?}-{shift:?}-{expected}.png"
          )),
          &image,
        )
        .unwrap();
        assert_focus(&image, ui, expected, library);
        if key == KeyCode::Tab && shift.is_none() && expected == 4 {
          crate::capture::save_frame(&output.join("driver-tab-mix-focus.png"), &image).unwrap();
        }
        app.update();
        platform::sync(app.world_mut());
        assert!(poll(editor.as_mut()).is_pending());
        assert!(
          platform::test_state(|s| s.audio_commands.is_empty()),
          "held navigation repeated"
        );
        send(app, window, key, ButtonState::Released, None);
        assert!(poll(editor.as_mut()).is_pending());
        if expected >= 4 {
          // Bottom actions do not inherit the last row's left/right ownership.
          send(app, window, KeyCode::ArrowRight, ButtonState::Pressed, None);
          assert!(poll(editor.as_mut()).is_pending());
          assert!(platform::test_state(|s| s.audio_commands.is_empty()));
          send(
            app,
            window,
            KeyCode::ArrowRight,
            ButtonState::Released,
            None,
          );
          assert!(poll(editor.as_mut()).is_pending());
        }
      }
      if let Some(shift) = shift {
        send(app, window, shift, ButtonState::Released, None);
        assert!(poll(editor.as_mut()).is_pending());
      }
    }
    let mix = ui.driver_action_bounds("mix", 0x38, "nubutton").unwrap();
    let point = Vec2::new(mix.x + 1.0, mix.y + mix.h * 0.5);
    send_mouse(app, window, point, ButtonState::Pressed);
    assert!(poll(editor.as_mut()).is_pending());
    send(app, window, KeyCode::ArrowDown, ButtonState::Pressed, None);
    assert!(poll(editor.as_mut()).is_pending());
    assert_focus(&capture_editor_frame(app), ui, 5, library);
    send(app, window, KeyCode::ArrowDown, ButtonState::Released, None);
    assert!(poll(editor.as_mut()).is_pending());
    send_mouse(app, window, point, ButtonState::Released);
    assert!(poll(editor.as_mut()).is_pending());
    send(app, window, KeyCode::Enter, ButtonState::Pressed, None);
    assert!(
      poll(editor.as_mut()).is_pending(),
      "focused MakeLicense arms on down"
    );
    send(app, window, KeyCode::Enter, ButtonState::Released, None);
    let Poll::Ready(Ok(Some(build))) = poll(editor.as_mut()) else {
      panic!("focused MakeLicense did not commit on matching release")
    };
    build
  };
  assert_eq!(
    actual,
    profile.custom_driver.clone().unwrap_or_else(Build::default),
    "focus navigation, bottom Right or cancelled Mix changed parts"
  );
  assert_eq!(serde_json::to_vec(profile).unwrap(), before);
  send(app, window, KeyCode::Enter, ButtonState::Released, None);
}
