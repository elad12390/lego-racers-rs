//! Explicit row clicks must cancel a keyboard hold in the formerly focused row.
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
  let data = Data::load(library).unwrap();
  let advance = |row, index| {
    let choices = data
      .row(row)
      .iter()
      .enumerate()
      .filter(|(i, p)| profile.driver_part_allowed(p.unlock, catalog) || *i == index)
      .map(|(i, _)| i)
      .collect::<Vec<_>>();
    let start = choices.iter().position(|i| *i == index).unwrap();
    choices[(start + 1) % choices.len()]
  };
  let before = serde_json::to_vec(profile).unwrap();
  let saved = std::fs::read(output.join("profile.json")).unwrap();
  for click_thumbnail in [false, true] {
    for key in [
      KeyCode::Enter,
      KeyCode::Escape,
      KeyCode::ArrowRight,
      KeyCode::ArrowDown,
    ] {
      send(app, window, key, ButtonState::Released, None);
    }
    send_mouse(app, window, Vec2::new(630.0, 470.0), ButtonState::Released);
    platform::test_state(|s| s.dt = 0.0);
    let mut editor = std::pin::pin!(crate::driver_editor::run(
      library, profile, catalog, ui, None
    ));
    assert!(poll(editor.as_mut()).is_pending());
    for state in [ButtonState::Pressed, ButtonState::Released] {
      send(app, window, KeyCode::ArrowDown, state, None);
      platform::test_state(|s| s.dt = 0.0);
      assert!(poll(editor.as_mut()).is_pending());
    }
    send(app, window, KeyCode::ArrowRight, ButtonState::Pressed, None);
    platform::test_state(|s| s.dt = 0.0);
    assert!(poll(editor.as_mut()).is_pending());
    if !click_thumbnail {
      crate::capture::save_frame(
        &output.join("driver-row-transfer-face-held.png"),
        &capture_editor_frame(app),
      )
      .unwrap();
    }
    let point = if click_thumbnail {
      let (selector, viewport) = ui.driver_selector(2).unwrap();
      let slot = selector.slots[selector.selected_slot + 1];
      Vec2::new(
        viewport.x + (slot[0] + slot[2]) as f32 * 0.5,
        viewport.y + (slot[1] + slot[3]) as f32 * 0.5,
      )
    } else {
      let rect = ui.driver_row_arrow_bounds(2, true).unwrap();
      Vec2::new(rect.x + rect.w * 0.5, rect.y + rect.h * 0.5)
    };
    send_mouse(app, window, point, ButtonState::Pressed);
    platform::test_state(|s| s.dt = 0.0);
    assert!(poll(editor.as_mut()).is_pending());
    send_mouse(app, window, Vec2::new(630.0, 470.0), ButtonState::Released);
    platform::test_state(|s| s.dt = 0.0);
    assert!(poll(editor.as_mut()).is_pending());
    // Keep the former Face key physically down through several repeat gates.
    for dt in [0.4, 0.0, 0.0, 0.4, 0.0, 0.0] {
      app.update();
      platform::sync(app.world_mut());
      platform::test_state(|s| s.dt = dt);
      assert!(poll(editor.as_mut()).is_pending());
    }
    if !click_thumbnail {
      crate::capture::save_frame(
        &output.join("driver-row-transfer-torso-settled.png"),
        &capture_editor_frame(app),
      )
      .unwrap();
    }
    send(
      app,
      window,
      KeyCode::ArrowRight,
      ButtonState::Released,
      None,
    );
    platform::test_state(|s| s.dt = 0.0);
    assert!(poll(editor.as_mut()).is_pending());
    send(app, window, KeyCode::Enter, ButtonState::Pressed, None);
    let Poll::Ready(Ok(Some(build))) = poll(editor.as_mut()) else {
      panic!("row transfer draft did not commit")
    };
    let mut expected = original.clone();
    expected.face = advance(1, original.face);
    expected.torso = advance(2, original.torso);
    assert_eq!(
      build, expected,
      "explicit row click must cancel the former row hold before later panel updates"
    );
    assert_eq!(serde_json::to_vec(profile).unwrap(), before);
    assert_eq!(std::fs::read(output.join("profile.json")).unwrap(), saved);
  }
  send(app, window, KeyCode::Enter, ButtonState::Released, None);
}
