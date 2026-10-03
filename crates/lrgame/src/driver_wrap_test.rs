//! Actual editor wraparound must render identically to entering its returned draft.
use crate::{
  custom_driver::{Build, Data},
  editor_gpu_test::{capture_editor_frame, poll, send},
  game_catalog::Catalog,
  menu_ui::MenuUi,
  platform,
  profile::Profile,
};
use bevy::{input::ButtonState, prelude::*};
use lrformats::library::Library;
use std::task::Poll;

fn index(build: &Build, row: usize) -> usize {
  [build.hat, build.face, build.torso, build.legs][row]
}
fn set_index(build: &mut Build, row: usize, value: usize) {
  match row {
    0 => build.hat = value,
    1 => build.face = value,
    2 => build.torso = value,
    _ => build.legs = value,
  }
}
fn pixels(image: &platform::Image, viewport: platform::Rect) -> Vec<[u8; 4]> {
  (viewport.y as i32..(viewport.y + viewport.h) as i32)
    .flat_map(|y| {
      (viewport.x as i32..(viewport.x + viewport.w) as i32)
        .map(move |x| crate::editor_gpu_test::logical_pixel(image, Vec2::new(x as f32, y as f32)))
    })
    .collect()
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
  let data = Data::load(library).unwrap();
  let original = profile.custom_driver.clone().unwrap_or_default();
  let before = serde_json::to_vec(profile).unwrap();
  let mut cases = Vec::new();
  for row in 0..4 {
    let choices = data
      .row(row)
      .iter()
      .enumerate()
      .filter(|(i, p)| {
        profile.driver_part_allowed(p.unlock, catalog) || *i == index(&original, row)
      })
      .map(|(i, _)| i)
      .collect::<Vec<_>>();
    let position = choices
      .iter()
      .position(|i| *i == index(&original, row))
      .unwrap();
    let (_, viewport) = ui.driver_selector(row).unwrap();
    for direction in [-1isize, 1] {
      let key = if direction < 0 {
        KeyCode::ArrowLeft
      } else {
        KeyCode::ArrowRight
      };
      let mut editor = std::pin::pin!(crate::driver_editor::run(
        library, profile, catalog, ui, None
      ));
      platform::test_state(|s| s.dt = 0.0);
      assert!(poll(editor.as_mut()).is_pending());
      for _ in 0..row {
        for state in [ButtonState::Pressed, ButtonState::Released] {
          send(app, window, KeyCode::ArrowDown, state, None);
          platform::test_state(|s| s.dt = 0.0);
          assert!(poll(editor.as_mut()).is_pending());
        }
      }
      for step in 0..choices.len() + 1 {
        send(app, window, key, ButtonState::Pressed, None);
        platform::test_state(|s| {
          s.dt = 0.0;
          s.audio_commands.clear();
        });
        assert!(poll(editor.as_mut()).is_pending());
        assert_eq!(
          platform::test_state(|s| std::mem::take(&mut s.audio_commands).len()),
          1,
          "every settled move must be accepted: row{row} direction{direction} step{step}"
        );
        send(app, window, key, ButtonState::Released, None);
        platform::test_state(|s| s.dt = 0.1);
        assert!(poll(editor.as_mut()).is_pending());
        if row == 1 && direction < 0 && step == 0 {
          let image = capture_editor_frame(app);
          crate::capture::save_frame(&output.join("driver-face-wrap-100ms.png"), &image).unwrap();
        }
        app.update();
        platform::sync(app.world_mut());
        platform::test_state(|s| s.dt = 0.3);
        assert!(poll(editor.as_mut()).is_pending());
        if row == 1 && direction < 0 && step == 0 {
          let image = capture_editor_frame(app);
          crate::capture::save_frame(&output.join("driver-face-wrap-400ms.png"), &image).unwrap();
        }
        app.update();
        platform::sync(app.world_mut());
        platform::test_state(|s| s.dt = 0.0);
        assert!(poll(editor.as_mut()).is_pending());
      }
      let actual_pixels = pixels(&capture_editor_frame(app), viewport);
      send(app, window, KeyCode::Enter, ButtonState::Pressed, None);
      let Poll::Ready(Ok(Some(actual))) = poll(editor.as_mut()) else {
        panic!("wrapped draft must commit")
      };
      send(app, window, KeyCode::Enter, ButtonState::Released, None);
      let mut expected = original.clone();
      set_index(
        &mut expected,
        row,
        choices[(position as isize + direction).rem_euclid(choices.len() as isize) as usize],
      );
      assert_eq!(
        actual, expected,
        "full cycle plus one must return correct neighbor, other parts unchanged"
      );
      // Compare the final actual rendered row with an independent direct-entry
      // rendering. This checks incoming edge mesh/styles, not just index math.
      let mut direct = profile.clone();
      direct.custom_driver = Some(expected.clone());
      let mut fresh = std::pin::pin!(crate::driver_editor::run(
        library, &direct, catalog, ui, None
      ));
      platform::test_state(|s| s.dt = 0.0);
      assert!(poll(fresh.as_mut()).is_pending());
      let direct_pixels = pixels(&capture_editor_frame(app), viewport);
      assert_eq!(
        actual_pixels, direct_pixels,
        "wrapped row must match direct-entry geometry/style: row{row} direction{direction}"
      );
      send(app, window, KeyCode::Escape, ButtonState::Pressed, None);
      assert!(matches!(poll(fresh.as_mut()), Poll::Ready(Ok(None))));
      send(app, window, KeyCode::Escape, ButtonState::Released, None);
      cases.push(
        serde_json::json!({"row":row,"direction":direction,"accepted_moves":choices.len()+1,
        "returned_parts":actual,"pixels_match_direct_entry":true}),
      );
    }
  }
  assert_eq!(serde_json::to_vec(profile).unwrap(), before);
  std::fs::write(output.join("driver-wrap.json"),serde_json::to_vec_pretty(&serde_json::json!({
    "mode":"scripted_offscreen_native_editor_not_physical_or_audible","profile_unchanged":true,"cases":cases})).unwrap()).unwrap();
}
