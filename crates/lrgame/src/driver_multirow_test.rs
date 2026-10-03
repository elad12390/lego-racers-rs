//! Separate picker rows animate independently; cancellation remains read-only.
use crate::{
  custom_driver::Data,
  editor_gpu_test::{capture_editor_frame, poll, send},
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
  let before = serde_json::to_vec(profile).unwrap();
  let saved = std::fs::read(output.join("profile.json")).unwrap();
  let data = Data::load(library).unwrap();
  let original = profile.custom_driver.clone().unwrap_or_default();
  let next = |row, current| {
    let choices = data
      .row(row)
      .iter()
      .enumerate()
      .filter(|(i, p)| profile.driver_part_allowed(p.unlock, catalog) || *i == current)
      .map(|(i, _)| i)
      .collect::<Vec<_>>();
    choices[(choices.iter().position(|i| *i == current).unwrap() + 1) % choices.len()]
  };
  let mut expected = original.clone();
  expected.hat = next(0, original.hat);
  expected.face = next(1, original.face);
  let mut reports = Vec::new();
  for cancel in [false, true] {
    let mut editor = std::pin::pin!(crate::driver_editor::run(
      library, profile, catalog, ui, None
    ));
    platform::test_state(|s| s.dt = 0.0);
    assert!(poll(editor.as_mut()).is_pending());
    for (key, state, dt, sounds) in [
      (KeyCode::ArrowRight, ButtonState::Pressed, 0.0, 1),
      (KeyCode::ArrowRight, ButtonState::Released, 0.1, 0),
      (KeyCode::ArrowDown, ButtonState::Pressed, 0.0, 1),
      (KeyCode::ArrowDown, ButtonState::Released, 0.0, 0),
      (KeyCode::ArrowRight, ButtonState::Pressed, 0.0, 1),
      (KeyCode::ArrowRight, ButtonState::Released, 0.1, 0),
      (KeyCode::ArrowLeft, ButtonState::Pressed, 0.0, 0),
      (KeyCode::ArrowLeft, ButtonState::Released, 0.0, 0),
    ] {
      send(app, window, key, state, None);
      platform::test_state(|s| {
        s.dt = dt;
        s.audio_commands.clear();
      });
      assert!(poll(editor.as_mut()).is_pending());
      assert_eq!(
        platform::test_state(|s| std::mem::take(&mut s.audio_commands).len()),
        sounds,
        "busy Hat must not prevent Face from starting; busy Face must reject Left"
      );
    }
    if !cancel {
      let image = capture_editor_frame(app);
      crate::capture::save_frame(&output.join("driver-two-rows-mid-scroll.png"), &image).unwrap();
      app.update();
      platform::sync(app.world_mut());
      platform::test_state(|s| s.dt = 0.3);
      assert!(poll(editor.as_mut()).is_pending());
      app.update();
      platform::sync(app.world_mut());
      platform::test_state(|s| s.dt = 0.0);
      assert!(poll(editor.as_mut()).is_pending());
      let image = capture_editor_frame(app);
      assert_eq!(
        crate::editor_gpu_test::logical_pixel(&image, Vec2::new(320.0, 90.0)),
        [0, 0, 55, 255]
      );
      assert_eq!(
        crate::editor_gpu_test::logical_pixel(&image, Vec2::new(382.0, 398.0)),
        [255, 255, 255, 255]
      );
      crate::capture::save_frame(&output.join("driver-two-rows-settled.png"), &image).unwrap();
    }
    let exit = if cancel {
      KeyCode::Escape
    } else {
      KeyCode::Enter
    };
    send(app, window, exit, ButtonState::Pressed, None);
    let Poll::Ready(Ok(result)) = poll(editor.as_mut()) else {
      panic!("multirow editor must exit")
    };
    assert_eq!(result, if cancel { None } else { Some(expected.clone()) });
    send(app, window, exit, ButtonState::Released, None);
    assert_eq!(serde_json::to_vec(profile).unwrap(), before);
    assert_eq!(std::fs::read(output.join("profile.json")).unwrap(), saved);
    reports.push(
      serde_json::json!({"cancelled_mid_scroll":cancel,"returned_parts":result,
      "profile_and_saved_file_unchanged":true}),
    );
  }
  std::fs::write(
    output.join("driver-two-rows.json"),
    serde_json::to_vec_pretty(&serde_json::json!({
    "mode":"scripted_offscreen_native_editor_not_physical_or_audible",
    "independent_hat_face_gates":true,"mid_scroll_ms":{"hat":200,"face":100},"cases":reports}))
    .unwrap(),
  )
  .unwrap();
}
