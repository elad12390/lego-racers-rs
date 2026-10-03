//! Proposed native regressions for unchanged-original focus/capture semantics.
//! Parent harness owns registration and GPU execution.
use crate::{
  custom_driver::Data,
  editor_gpu_test::{poll, send, send_mouse},
  game_catalog::Catalog,
  menu_ui::MenuUi,
  platform,
  profile::Profile,
};
use bevy::{input::ButtonState, prelude::*};
use lrformats::library::Library;
use std::{future::Future, pin::Pin, task::Poll};

fn tick<F: Future>(app: &mut App, editor: Pin<&mut F>, dt: f32) {
  app.update();
  platform::sync(app.world_mut());
  platform::test_state(|s| s.dt = dt);
  assert!(poll(editor).is_pending());
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
  let original = profile.custom_driver.clone().unwrap_or_default();
  let data = Data::load(library).unwrap();
  let choices = data
    .row(1)
    .iter()
    .enumerate()
    .filter(|(i, part)| profile.driver_part_allowed(part.unlock, catalog) || *i == original.face)
    .map(|(i, _)| i)
    .collect::<Vec<_>>();
  assert!(choices.len() > 2);
  let current = choices.iter().position(|i| *i == original.face).unwrap();
  let mut expected = original.clone();
  expected.face = choices[(current + 1) % choices.len()];
  let before = serde_json::to_vec(profile).unwrap();
  let save = output.join("driver-capture-focus-profile.json");
  profile.save(&save).unwrap();
  let saved = std::fs::read(&save).unwrap();
  let arrow = ui.driver_row_arrow_bounds(1, true).unwrap();
  let point = Vec2::new(arrow.x + arrow.w * 0.5, arrow.y + arrow.h * 0.5);
  let outside = Vec2::new(630.0, 470.0);

  // Do not release the mouse until AFTER the old row has had ample opportunity
  // to repeat. These keys really change focus; no mocked cancellation call.
  for key in [KeyCode::Tab, KeyCode::ArrowUp, KeyCode::ArrowDown] {
    send_mouse(app, window, outside, ButtonState::Released);
    platform::test_state(|s| s.dt = 0.0);
    let mut editor = std::pin::pin!(crate::driver_editor::run(
      library, profile, catalog, ui, None
    ));
    assert!(poll(editor.as_mut()).is_pending());
    send_mouse(app, window, point, ButtonState::Pressed);
    platform::test_state(|s| s.dt = 0.0);
    assert!(poll(editor.as_mut()).is_pending());
    for state in [ButtonState::Pressed, ButtonState::Released] {
      send(app, window, key, state, None);
      platform::test_state(|s| s.dt = 0.0);
      assert!(poll(editor.as_mut()).is_pending());
    }
    for dt in [0.4, 0.0, 0.0, 0.4, 0.0, 0.0] {
      tick(app, editor.as_mut(), dt);
    }
    send_mouse(app, window, outside, ButtonState::Released);
    platform::test_state(|s| s.dt = 0.0);
    assert!(poll(editor.as_mut()).is_pending());
    send(app, window, KeyCode::Enter, ButtonState::Pressed, None);
    platform::test_state(|s| s.dt = 0.0);
    let Poll::Ready(Ok(Some(actual))) = poll(editor.as_mut()) else {
      panic!("focus-cancellation draft did not commit after {key:?}")
    };
    assert_eq!(
      actual, expected,
      "{key:?} must unhighlight/deactivate the old row: only the initial mouse move survives"
    );
    send(app, window, KeyCode::Enter, ButtonState::Released, None);
    platform::test_state(|s| s.dt = 0.0);
  }
  assert_eq!(serde_json::to_vec(profile).unwrap(), before);
  assert_eq!(std::fs::read(save).unwrap(), saved);
}
