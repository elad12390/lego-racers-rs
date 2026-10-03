//! Real editor mouse paths; no private selector state or simulated acceptance.
use crate::{
  custom_driver::{Build, Data},
  editor_gpu_test::{capture_editor_frame, logical_pixel, poll, send, send_mouse},
  game_catalog::Catalog,
  menu_ui::MenuUi,
  platform,
  profile::Profile,
};
use bevy::{input::ButtonState, prelude::*};
use lrformats::library::Library;
use std::{future::Future, path::Path, pin::Pin, task::Poll};

fn tick<F: Future>(app: &mut App, editor: Pin<&mut F>, dt: f32) {
  app.update();
  platform::sync(app.world_mut());
  platform::test_state(|s| s.dt = dt);
  assert!(poll(editor).is_pending());
}

fn mouse<F: Future>(
  app: &mut App,
  window: Entity,
  editor: Pin<&mut F>,
  point: Vec2,
  state: ButtonState,
) {
  send_mouse(app, window, point, state);
  platform::test_state(|s| s.dt = 0.0);
  assert!(poll(editor).is_pending());
}

fn move_cursor(app: &mut App, window: Entity, point: Vec2) {
  app
    .world_mut()
    .get_mut::<Window>(window)
    .unwrap()
    .set_cursor_position(Some(MenuUi::origin() + point * MenuUi::scale()));
}

fn commit<F: Future<Output = Result<Option<Build>, String>>>(
  app: &mut App,
  window: Entity,
  editor: Pin<&mut F>,
) -> Build {
  send(app, window, KeyCode::Enter, ButtonState::Pressed, None);
  platform::test_state(|s| s.dt = 0.0);
  let Poll::Ready(Ok(Some(build))) = poll(editor) else {
    panic!("pointer-edited draft did not commit")
  };
  send(app, window, KeyCode::Enter, ButtonState::Released, None);
  platform::test_state(|s| s.dt = 0.0);
  build
}

fn sounds() -> usize {
  platform::test_state(|s| std::mem::take(&mut s.audio_commands).len())
}

fn row_pixels(image: &platform::Image, rect: crate::platform::prelude::Rect) -> Vec<[u8; 4]> {
  let mut pixels = Vec::new();
  for y in rect.y.ceil() as i32..(rect.y + rect.h).floor() as i32 {
    for x in rect.x.ceil() as i32..(rect.x + rect.w).floor() as i32 {
      pixels.push(logical_pixel(image, Vec2::new(x as f32, y as f32)));
    }
  }
  pixels
}

pub fn verify(
  app: &mut App,
  window: Entity,
  library: &Library,
  catalog: &Catalog,
  ui: &MenuUi,
  profile: &Profile,
  output: &Path,
) {
  assert_eq!(ui.driver_scroll_ms, 400);
  assert!(
    ui.audio.is_some(),
    "move acceptance checks require actual menu audio commands"
  );
  let before = serde_json::to_vec(profile).unwrap();
  let save = output.join("driver-pointer-profile.json");
  profile.save(&save).unwrap();
  let saved = std::fs::read(&save).unwrap();
  let original = profile.custom_driver.clone().unwrap_or_default();
  let data = Data::load(library).unwrap();
  // Face has no optional empty-hat mesh, so the before/after pictures are useful.
  let row = 1;
  let choices = data
    .row(row)
    .iter()
    .enumerate()
    .filter(|(i, part)| profile.driver_part_allowed(part.unlock, catalog) || *i == original.face)
    .map(|(i, _)| i)
    .collect::<Vec<_>>();
  assert!(choices.len() > 2);
  let start = choices.iter().position(|i| *i == original.face).unwrap();
  let expected = |offset: usize| {
    let mut build = original.clone();
    build.face = choices[(start + offset) % choices.len()];
    build
  };
  let center = |r: crate::platform::prelude::Rect| Vec2::new(r.x + r.w * 0.5, r.y + r.h * 0.5);
  let right = center(ui.driver_row_arrow_bounds(row, true).unwrap());
  let left = center(ui.driver_row_arrow_bounds(row, false).unwrap());
  let outside = Vec2::new(630.0, 470.0);
  let (selector, viewport) = ui.driver_selector(row).unwrap();
  let clicked_slot = selector.selected_slot + 1;
  let bounds = selector.slots[clicked_slot];
  let lo = Vec2::new((bounds[0] as f32).max(0.0), (bounds[1] as f32).max(0.0));
  let hi = Vec2::new(
    (bounds[2] as f32).min(viewport.w),
    (bounds[3] as f32).min(viewport.h),
  );
  assert!(
    hi.x > lo.x && hi.y > lo.y,
    "neighbor authored slot must intersect row clip"
  );
  let thumbnail = Vec2::new(viewport.x, viewport.y) + (lo + hi) * 0.5;
  send_mouse(app, window, outside, ButtonState::Released);
  platform::test_state(|s| s.dt = 0.0);

  // Held right: exactly two accepted moves, despite opposite/outside cursor.
  {
    let mut editor = std::pin::pin!(crate::driver_editor::run(
      library, profile, catalog, ui, None
    ));
    assert!(poll(editor.as_mut()).is_pending());
    sounds();
    mouse(app, window, editor.as_mut(), right, ButtonState::Pressed);
    assert_eq!(sounds(), 1, "fresh right arrow should move once");
    move_cursor(app, window, left);
    tick(app, editor.as_mut(), 0.4);
    assert_eq!(
      sounds(),
      0,
      "400ms reaches zero but does not clear entry gate"
    );
    move_cursor(app, window, outside);
    tick(app, editor.as_mut(), 0.0);
    assert_eq!(sounds(), 0, "next update clears gate after attempted move");
    tick(app, editor.as_mut(), 0.0);
    assert_eq!(
      sounds(),
      1,
      "held right repeats after completion, without another press"
    );
    mouse(app, window, editor.as_mut(), outside, ButtonState::Released);
    tick(app, editor.as_mut(), 0.4);
    tick(app, editor.as_mut(), 0.0);
    tick(app, editor.as_mut(), 0.0);
    assert_eq!(sounds(), 0, "outside release must stop future repeats");
    assert_eq!(
      commit(app, window, editor.as_mut()),
      expected(2),
      "only face advances twice"
    );
  }

  // A fresh opposite press while busy captures, rather than disappearing.
  {
    let mut editor = std::pin::pin!(crate::driver_editor::run(
      library, profile, catalog, ui, None
    ));
    assert!(poll(editor.as_mut()).is_pending());
    sounds();
    mouse(app, window, editor.as_mut(), right, ButtonState::Pressed);
    assert_eq!(sounds(), 1);
    mouse(app, window, editor.as_mut(), outside, ButtonState::Released);
    mouse(app, window, editor.as_mut(), left, ButtonState::Pressed);
    assert_eq!(sounds(), 0, "busy opposite press must not move immediately");
    tick(app, editor.as_mut(), 0.4);
    tick(app, editor.as_mut(), 0.0);
    assert_eq!(sounds(), 0);
    tick(app, editor.as_mut(), 0.0);
    assert_eq!(sounds(), 1, "busy press must retain opposite capture");
    mouse(app, window, editor.as_mut(), outside, ButtonState::Released);
    assert_eq!(commit(app, window, editor.as_mut()), original);
  }

  // Idle authored thumbnail click changes the actual native draft and picture.
  {
    let mut editor = std::pin::pin!(crate::driver_editor::run(
      library, profile, catalog, ui, None
    ));
    assert!(poll(editor.as_mut()).is_pending());
    let image_before = capture_editor_frame(app);
    crate::capture::save_frame(
      &output.join("driver-thumbnail-click-before.png"),
      &image_before,
    )
    .unwrap();
    mouse(
      app,
      window,
      editor.as_mut(),
      thumbnail,
      ButtonState::Pressed,
    );
    mouse(app, window, editor.as_mut(), outside, ButtonState::Released);
    let image_after = capture_editor_frame(app);
    crate::capture::save_frame(
      &output.join("driver-thumbnail-click-after.png"),
      &image_after,
    )
    .unwrap();
    assert_ne!(
      row_pixels(&image_before, viewport),
      row_pixels(&image_after, viewport),
      "thumbnail click must visibly rebuild the actual row"
    );
    assert_eq!(
      commit(app, window, editor.as_mut()),
      expected(1),
      "authored neighbor click must preserve hat, torso, and legs"
    );
  }

  // Busy click changes the draft, not its in-flight thumbnail records or timer.
  {
    let mut editor = std::pin::pin!(crate::driver_editor::run(
      library, profile, catalog, ui, None
    ));
    assert!(poll(editor.as_mut()).is_pending());
    mouse(app, window, editor.as_mut(), right, ButtonState::Pressed);
    mouse(app, window, editor.as_mut(), outside, ButtonState::Released);
    tick(app, editor.as_mut(), 0.1);
    let moving_before = capture_editor_frame(app);
    mouse(
      app,
      window,
      editor.as_mut(),
      thumbnail,
      ButtonState::Pressed,
    );
    let moving_after = capture_editor_frame(app);
    assert_eq!(
      row_pixels(&moving_before, viewport),
      row_pixels(&moving_after, viewport),
      "busy click must preserve every rendered thumbnail pixel at the same timestep"
    );
    mouse(app, window, editor.as_mut(), outside, ButtonState::Released);
    sounds();
    tick(app, editor.as_mut(), 0.3);
    let moving_finished = capture_editor_frame(app);
    assert_ne!(
      row_pixels(&moving_after, viewport),
      row_pixels(&moving_finished, viewport),
      "preserved busy thumbnail records must continue their actual motion"
    );
    // Press now: still gated on entry, then completion clears it.
    mouse(app, window, editor.as_mut(), right, ButtonState::Pressed);
    assert_eq!(
      sounds(),
      0,
      "busy click must neither clear nor prematurely finish scroll"
    );
    tick(app, editor.as_mut(), 0.0);
    assert_eq!(
      sounds(),
      1,
      "busy click must not restart the 400ms countdown"
    );
    mouse(app, window, editor.as_mut(), outside, ButtonState::Released);
    assert_eq!(
      commit(app, window, editor.as_mut()),
      expected(3),
      "busy thumbnail selection must contribute its own immediate draft change"
    );
  }

  // Cancel a genuinely mouse-edited draft and verify both persistence surfaces.
  {
    let mut editor = std::pin::pin!(crate::driver_editor::run(
      library, profile, catalog, ui, None
    ));
    assert!(poll(editor.as_mut()).is_pending());
    mouse(
      app,
      window,
      editor.as_mut(),
      thumbnail,
      ButtonState::Pressed,
    );
    mouse(app, window, editor.as_mut(), outside, ButtonState::Released);
    send(app, window, KeyCode::Escape, ButtonState::Pressed, None);
    platform::test_state(|s| s.dt = 0.0);
    assert!(matches!(poll(editor.as_mut()), Poll::Ready(Ok(None))));
    send(app, window, KeyCode::Escape, ButtonState::Released, None);
  }
  assert_eq!(
    serde_json::to_vec(profile).unwrap(),
    before,
    "commit/cancel mutated source profile"
  );
  assert_eq!(
    std::fs::read(save).unwrap(),
    saved,
    "commit/cancel changed saved profile"
  );
  platform::test_state(|s| s.dt = 0.0);
}
