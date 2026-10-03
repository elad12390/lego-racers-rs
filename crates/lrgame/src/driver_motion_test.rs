//! Fresh video of the actual editor's held-arrow path, not an animation mock.
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
  send_mouse(app, window, Vec2::new(600.0, 460.0), ButtonState::Released);
  let original = profile.custom_driver.clone().unwrap_or_default();
  let before = serde_json::to_vec(profile).unwrap();
  let data = Data::load(library).unwrap();
  let choices = data
    .row(1)
    .iter()
    .enumerate()
    .filter(|(i, part)| profile.driver_part_allowed(part.unlock, catalog) || *i == original.face)
    .map(|(i, _)| i)
    .collect::<Vec<_>>();
  let start = choices.iter().position(|i| *i == original.face).unwrap();
  let mut editor = std::pin::pin!(crate::driver_editor::run(
    library, profile, catalog, ui, None
  ));
  platform::test_state(|s| {
    s.dt = 0.0;
    s.audio_commands.clear();
  });
  assert!(poll(editor.as_mut()).is_pending());
  let first = capture_editor_frame(app);
  let mut video = crate::capture_video::Video::start(
    &output.join("driver-held-arrow.mp4"),
    first.width,
    first.height,
  )
  .unwrap();
  let arrow = ui.driver_row_arrow_bounds(1, true).unwrap();
  send_mouse(
    app,
    window,
    Vec2::new(arrow.x + arrow.w * 0.5, arrow.y + arrow.h * 0.5),
    ButtonState::Pressed,
  );
  platform::test_state(|s| s.dt = 0.0);
  assert!(poll(editor.as_mut()).is_pending());
  let mut moves = platform::test_state(|s| std::mem::take(&mut s.audio_commands).len());
  assert_eq!(moves, 1);
  let mut hashes = std::collections::HashSet::new();
  for frame in 0..90 {
    app.update();
    platform::sync(app.world_mut());
    platform::test_state(|s| s.dt = 1.0 / 30.0);
    assert!(poll(editor.as_mut()).is_pending());
    moves += platform::test_state(|s| std::mem::take(&mut s.audio_commands).len());
    let image = capture_editor_frame(app);
    video.frame(&image).unwrap();
    let hash = image.bytes.iter().fold(0xcbf29ce484222325u64, |h, b| {
      (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
    });
    hashes.insert(hash);
    if frame == 5 || frame == 77 {
      crate::capture::save_frame(
        &output.join(if frame == 5 {
          "driver-held-arrow-early.png"
        } else {
          "driver-held-arrow-later.png"
        }),
        &image,
      )
      .unwrap();
    }
  }
  assert_eq!(video.finish().unwrap(), 90);
  assert_eq!(
    moves, 7,
    "held selector repeats only after the original completion gate clears"
  );
  assert!(
    hashes.len() > 60,
    "clip must show actual changing native frames, not a frozen screenshot"
  );
  send_mouse(app, window, Vec2::new(600.0, 460.0), ButtonState::Released);
  platform::test_state(|s| s.dt = 0.0);
  assert!(poll(editor.as_mut()).is_pending());
  for _ in 0..15 {
    app.update();
    platform::sync(app.world_mut());
    platform::test_state(|s| s.dt = 1.0 / 30.0);
    assert!(poll(editor.as_mut()).is_pending());
    assert!(platform::test_state(|s| std::mem::take(
      &mut s.audio_commands
    )
    .is_empty()));
  }
  send(app, window, KeyCode::Enter, ButtonState::Pressed, None);
  let Poll::Ready(Ok(Some(result))) = poll(editor.as_mut()) else {
    panic!("held-arrow draft did not commit")
  };
  let mut expected = original;
  expected.face = choices[(start + moves) % choices.len()];
  assert_eq!(result, expected);
  send(app, window, KeyCode::Enter, ButtonState::Released, None);
  assert_eq!(serde_json::to_vec(profile).unwrap(), before);
  std::fs::write(
    output.join("driver-held-arrow.json"),
    serde_json::to_vec_pretty(&serde_json::json!({
      "mode":"scripted_offscreen_native_editor_not_physical_audible_or_game_acceptance",
      "frames":90,"scripted_seconds":3.0,"fps":30,"distinct_frame_hashes":hashes.len(),
      "accepted_moves":moves,"outside_release_stops":true,"returned_draft":result,
      "profile_unchanged":true
    }))
    .unwrap(),
  )
  .unwrap();
}
