//! Scripted actual editor input, timed Metal images and draft-preservation checks.
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
  assert_eq!(ui.driver_scroll_ms, 400);
  let data = Data::load(library).unwrap();
  let original = profile.custom_driver.clone().unwrap_or_default();
  let choices = data
    .row(0)
    .iter()
    .enumerate()
    .filter(|(i, p)| profile.driver_part_allowed(p.unlock, catalog) || *i == original.hat)
    .map(|(i, _)| i)
    .collect::<Vec<_>>();
  let position = choices.iter().position(|i| *i == original.hat).unwrap();
  let before = serde_json::to_vec(profile).unwrap();
  let mut editor = std::pin::pin!(crate::driver_editor::run(
    library, profile, catalog, ui, None
  ));
  assert!(poll(editor.as_mut()).is_pending());
  send(app, window, KeyCode::ArrowRight, ButtonState::Pressed, None);
  platform::test_state(|s| {
    s.dt = 0.0;
    s.audio_commands.clear();
  });
  assert!(poll(editor.as_mut()).is_pending());
  assert_eq!(
    platform::test_state(|s| std::mem::take(&mut s.audio_commands).len()),
    1
  );
  let image = capture_editor_frame(app);
  crate::capture::save_frame(&output.join("driver-scroll-000ms.png"), &image).unwrap();
  send(
    app,
    window,
    KeyCode::ArrowRight,
    ButtonState::Released,
    None,
  );
  platform::test_state(|s| s.dt = 0.0);
  assert!(poll(editor.as_mut()).is_pending());
  // Both new press directions during translation must be rejected and silent.
  for key in [KeyCode::ArrowRight, KeyCode::ArrowLeft] {
    for state in [ButtonState::Pressed, ButtonState::Released] {
      send(app, window, key, state, None);
      platform::test_state(|s| s.dt = 0.0);
      assert!(poll(editor.as_mut()).is_pending());
      assert!(platform::test_state(|s| s.audio_commands.is_empty()));
    }
  }
  let mut hashes = Vec::new();
  for ms in [100, 200, 300, 400] {
    app.update();
    platform::sync(app.world_mut());
    platform::test_state(|s| s.dt = 0.1);
    assert!(poll(editor.as_mut()).is_pending());
    let image = capture_editor_frame(app);
    crate::capture::save_frame(&output.join(format!("driver-scroll-{ms:03}ms.png")), &image)
      .unwrap();
    // Hash only the Hat viewport, not the unrelated animated character.
    let mut hash = 0xcbf29ce484222325u64;
    for y in 28..92 {
      for x in 48..248 {
        for byte in crate::editor_gpu_test::logical_pixel(&image, Vec2::new(x as f32, y as f32)) {
          hash = (hash ^ byte as u64).wrapping_mul(0x100000001b3);
        }
      }
    }
    hashes.push(hash);
    assert_eq!(
      crate::editor_gpu_test::logical_pixel(&image, Vec2::new(320.0, 90.0)),
      [0, 0, 55, 255]
    );
  }
  assert!(
    hashes.windows(2).all(|h| h[0] != h[1]),
    "native timed frames must actually move"
  );
  send(app, window, KeyCode::ArrowRight, ButtonState::Pressed, None);
  platform::test_state(|s| s.dt = 0.0);
  assert!(poll(editor.as_mut()).is_pending());
  assert!(
    platform::test_state(|s| s.audio_commands.is_empty()),
    "zero remaining still gated on entry"
  );
  send(
    app,
    window,
    KeyCode::ArrowRight,
    ButtonState::Released,
    None,
  );
  platform::test_state(|s| s.dt = 0.0);
  assert!(poll(editor.as_mut()).is_pending());
  send(app, window, KeyCode::ArrowRight, ButtonState::Pressed, None);
  platform::test_state(|s| s.dt = 0.0);
  assert!(poll(editor.as_mut()).is_pending());
  assert_eq!(
    platform::test_state(|s| std::mem::take(&mut s.audio_commands).len()),
    1
  );
  // Slow frames clamp at the target, then a separate update clears the gate.
  send(
    app,
    window,
    KeyCode::ArrowRight,
    ButtonState::Released,
    None,
  );
  platform::test_state(|s| s.dt = 0.75);
  assert!(poll(editor.as_mut()).is_pending());
  app.update();
  platform::sync(app.world_mut());
  platform::test_state(|s| s.dt = 0.0);
  assert!(poll(editor.as_mut()).is_pending());
  send(app, window, KeyCode::ArrowLeft, ButtonState::Pressed, None);
  platform::test_state(|s| s.dt = 0.0);
  assert!(poll(editor.as_mut()).is_pending());
  assert_eq!(
    platform::test_state(|s| std::mem::take(&mut s.audio_commands).len()),
    1
  );
  send(app, window, KeyCode::ArrowLeft, ButtonState::Released, None);
  platform::test_state(|s| s.dt = 0.0);
  assert!(poll(editor.as_mut()).is_pending());
  for ms in [100, 400] {
    app.update();
    platform::sync(app.world_mut());
    platform::test_state(|s| s.dt = if ms == 100 { 0.1 } else { 0.3 });
    assert!(poll(editor.as_mut()).is_pending());
    let image = capture_editor_frame(app);
    crate::capture::save_frame(
      &output.join(format!("driver-scroll-reverse-{ms:03}ms.png")),
      &image,
    )
    .unwrap();
  }
  send(app, window, KeyCode::Enter, ButtonState::Pressed, None);
  let Poll::Ready(Ok(Some(actual))) = poll(editor.as_mut()) else {
    panic!("scroll draft did not commit")
  };
  let mut expected = original;
  expected.hat = choices[(position + 1) % choices.len()];
  assert_eq!(
    actual, expected,
    "blocked presses must not change draft; settled scroll must accept next"
  );
  assert_eq!(serde_json::to_vec(profile).unwrap(), before);
  send(
    app,
    window,
    KeyCode::ArrowRight,
    ButtonState::Released,
    None,
  );
  send(app, window, KeyCode::Enter, ButtonState::Released, None);
  export_positions(library, ui, output);
  std::fs::write(
    output.join("driver-scroll.json"),
    serde_json::to_vec_pretty(&serde_json::json!({
    "mode":"scripted_offscreen_native_editor_not_physical_or_audible","duration_ms":400,
    "timed_native_frames_ms":[0,100,200,300,400],"both_directions_gated":true,
    "blocked_press_silent":true,"zero_remaining_gated_until_next_update":true,
    "reverse_timed_frames_ms":[100,400],"slow_frame_clamped":true,
    "profile_unchanged":true,"returned_parts":actual}))
    .unwrap(),
  )
  .unwrap();
}

fn export_positions(library: &Library, ui: &MenuUi, output: &std::path::Path) {
  let data = Data::load(library).unwrap();
  let light = crate::garage_lighting::driver(&lrsim::brick_build::Rules::load().unwrap());
  let source = |v: Vec3| [v.x, -v.z, v.y];
  let mut cases = Vec::new();
  for row in 0..4 {
    let choices = (0..data.row(row).len()).collect::<Vec<_>>();
    let selected = 3;
    let (selector, viewport) = ui.driver_selector(row).unwrap();
    for direction in [-1isize, 1] {
      let mut native =
        crate::driver_thumbnail::row(&data, library, ui, &choices, row, selected, &light).unwrap();
      let edge = if direction > 0 { 4 } else { 0 };
      let incoming_index = (selected as isize + direction + edge as isize - 2)
        .rem_euclid(choices.len() as isize) as usize;
      let incoming =
        crate::driver_thumbnail::load(&data, library, row, incoming_index, &light).unwrap();
      native.shift(direction, incoming, selector, viewport);
      let pieces = native
        .items
        .iter()
        .map(|item| {
          item.as_ref().map(|item| {
            serde_json::json!({
              "center":source(item.center),"radius":item.radius,"start":source(item.translation)
            })
          })
        })
        .collect::<Vec<_>>();
      let mut frames = Vec::new();
      for elapsed in [100, 100, 350, 0] {
        native.advance(elapsed);
        frames.push(serde_json::json!({"elapsed":elapsed,"busy":native.busy(),
          "positions":native.items.iter().map(|item|item.as_ref().map(|item|source(item.translation))).collect::<Vec<_>>()}));
      }
      cases.push(
        serde_json::json!({"row":row,"direction":direction,"pieces":pieces,
        "width":viewport.w,"height":viewport.h,"slots":selector.slots,"frames":frames}),
      );
    }
  }
  std::fs::write(
    output.join("driver-scroll-positions.json"),
    serde_json::to_vec(&cases).unwrap(),
  )
  .unwrap();
}
