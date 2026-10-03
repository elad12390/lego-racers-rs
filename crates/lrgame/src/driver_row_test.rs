//! Actual Bevy/Metal driver row arrows at source-authored parent-relative bounds.
use crate::{
  custom_driver::{Build, Data},
  editor_gpu_test::{capture_editor_frame, poll, send_mouse},
  game_catalog::Catalog,
  menu_ui::MenuUi,
  platform,
  profile::Profile,
};
use bevy::{input::ButtonState, prelude::*};
use lrformats::library::Library;
use std::task::Poll;

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
  let data = Data::load(library).unwrap();
  let mut expected = profile.custom_driver.clone().unwrap_or_default();
  let mut indices = [expected.hat, expected.face, expected.torso, expected.legs];
  for (row, index) in indices.iter_mut().enumerate() {
    let allowed = data
      .row(row)
      .iter()
      .enumerate()
      .filter(|(i, p)| profile.driver_part_allowed(p.unlock, catalog) || *i == *index)
      .map(|(i, _)| i)
      .collect::<Vec<_>>();
    let current = allowed.iter().position(|i| i == index).unwrap();
    *index = allowed[(current + 1) % allowed.len()];
  }
  expected = Build {
    hat: indices[0],
    face: indices[1],
    torso: indices[2],
    legs: indices[3],
  };
  let actual = {
    let mut editor = std::pin::pin!(crate::driver_editor::run(
      library, profile, catalog, ui, None
    ));
    assert!(poll(editor.as_mut()).is_pending());
    for (row, y) in [44.0, 124.0, 196.0, 276.0].into_iter().enumerate() {
      let left = ui.driver_row_arrow_bounds(row, false).unwrap();
      let right = ui.driver_row_arrow_bounds(row, true).unwrap();
      assert_eq!((left.x, left.y, left.w, left.h), (16.0, y, 32.0, 32.0));
      assert_eq!((right.x, right.y, right.w, right.h), (248.0, y, 32.0, 32.0));
      // Just beyond the former provisional 40px height must not change parts.
      platform::test_state(|s| s.audio_commands.clear());
      let outside = Vec2::new(right.x + right.w * 0.5, right.y + right.h + 1.0);
      send_mouse(app, window, outside, ButtonState::Pressed);
      assert!(poll(editor.as_mut()).is_pending());
      assert!(platform::test_state(|s| s.audio_commands.is_empty()));
      send_mouse(app, window, outside, ButtonState::Released);
      assert!(poll(editor.as_mut()).is_pending());
      let corner = Vec2::new(right.x + right.w, right.y + right.h);
      send_mouse(app, window, corner, ButtonState::Pressed);
      assert!(poll(editor.as_mut()).is_pending());
      assert_eq!(
        platform::test_state(|s| std::mem::take(&mut s.audio_commands).len()),
        1
      );
      app.update();
      platform::sync(app.world_mut());
      assert!(poll(editor.as_mut()).is_pending());
      assert!(
        platform::test_state(|s| s.audio_commands.is_empty()),
        "held row arrow repeats"
      );
      let image = capture_editor_frame(app);
      if row == 0 || row == 3 {
        crate::capture::save_frame(
          &output.join(if row == 0 {
            "driver-authored-hat-arrow.png"
          } else {
            "driver-authored-legs-arrow.png"
          }),
          &image,
        )
        .unwrap();
      }
      send_mouse(app, window, corner, ButtonState::Released);
      assert!(poll(editor.as_mut()).is_pending());
    }
    let commit = ui.driver_action_bounds("gonext", 10, "buttonra").unwrap();
    let point = Vec2::new(commit.x + 1.0, commit.y + commit.h * 0.5);
    send_mouse(app, window, point, ButtonState::Pressed);
    assert!(poll(editor.as_mut()).is_pending());
    send_mouse(app, window, point, ButtonState::Released);
    let Poll::Ready(Ok(Some(build))) = poll(editor.as_mut()) else {
      panic!("row draft did not commit")
    };
    build
  };
  assert_eq!(
    actual, expected,
    "each authored corner must change exactly one unlocked part"
  );
  data.validate(&actual).unwrap();
}
