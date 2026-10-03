//! Real native hover focus, capture ownership and accepted keyboard release.
use crate::{
  custom_driver::{Build, Data},
  editor_gpu_test::{capture_editor_frame, move_mouse, poll, send, send_mouse},
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
  let before = serde_json::to_vec(profile).unwrap();
  let saved = std::fs::read(output.join("profile.json")).unwrap();
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
  let torso = Vec2::new(150.0, 210.0);
  let mix_rect = ui.driver_action_bounds("mix", 0x38, "nubutton").unwrap();
  let mix = Vec2::new(mix_rect.x + 1.0, mix_rect.y + 16.0);
  // Idle torso hover must select Torso without a click; captured Face must
  // retain selection across both Torso and Mix until matching release.
  for capture in [0, 1, 2, 3, 4] {
    let mouse_capture = capture == 2 || capture == 4;
    let recover_hover = capture >= 3;
    for key in [
      KeyCode::Enter,
      KeyCode::Space,
      KeyCode::ArrowDown,
      KeyCode::ArrowRight,
    ] {
      send(app, window, key, ButtonState::Released, None);
    }
    send_mouse(app, window, Vec2::new(630.0, 470.0), ButtonState::Released);
    let mut editor = std::pin::pin!(crate::driver_editor::run(
      library, profile, catalog, ui, None
    ));
    platform::test_state(|s| s.dt = 0.0);
    assert!(poll(editor.as_mut()).is_pending());
    if capture == 1 || capture == 3 {
      for state in [ButtonState::Pressed, ButtonState::Released] {
        send(app, window, KeyCode::ArrowDown, state, None);
        platform::test_state(|s| s.dt = 0.0);
        assert!(poll(editor.as_mut()).is_pending());
      }
      send(app, window, KeyCode::ArrowRight, ButtonState::Pressed, None);
    } else if mouse_capture {
      let arrow = ui.driver_row_arrow_bounds(1, true).unwrap();
      send_mouse(
        app,
        window,
        Vec2::new(arrow.x + 16.0, arrow.y + 16.0),
        ButtonState::Pressed,
      );
    }
    platform::test_state(|s| s.dt = 0.0);
    assert!(poll(editor.as_mut()).is_pending());
    move_mouse(app, window, torso);
    platform::test_state(|s| s.dt = 0.0);
    assert!(poll(editor.as_mut()).is_pending());
    if capture < 2 {
      crate::capture::save_frame(
        &output.join(if capture == 0 {
          "driver-idle-hover-torso.png"
        } else {
          "driver-captured-face-hover-torso.png"
        }),
        &capture_editor_frame(app),
      )
      .unwrap();
    }
    if capture == 3 {
      crate::capture::save_frame(
        &output.join("driver-hover-recovery-face-held.png"),
        &capture_editor_frame(app),
      )
      .unwrap();
    }
    if capture == 0 {
      send(app, window, KeyCode::ArrowRight, ButtonState::Pressed, None);
      platform::test_state(|s| s.dt = 0.0);
      assert!(poll(editor.as_mut()).is_pending());
    } else {
      move_mouse(app, window, mix);
      platform::test_state(|s| s.dt = 0.0);
      assert!(poll(editor.as_mut()).is_pending());
    }
    if mouse_capture {
      // Matching mouse release anywhere ends capture without another move.
      send_mouse(app, window, mix, ButtonState::Released);
    } else {
      send(
        app,
        window,
        KeyCode::ArrowRight,
        ButtonState::Released,
        None,
      );
    }
    platform::test_state(|s| s.dt = 0.0);
    assert!(poll(editor.as_mut()).is_pending());
    if recover_hover {
      // Release must restore the outer hover route without a new mouse click.
      move_mouse(app, window, torso);
      platform::test_state(|s| s.dt = 0.0);
      assert!(poll(editor.as_mut()).is_pending());
      for state in [ButtonState::Pressed, ButtonState::Released] {
        send(app, window, KeyCode::ArrowRight, state, None);
        platform::test_state(|s| s.dt = 0.0);
        assert!(poll(editor.as_mut()).is_pending());
      }
      // Settled image must show both selected edits and the recovered focus.
      for dt in [0.4, 0.0, 0.0] {
        app.update();
        platform::sync(app.world_mut());
        platform::test_state(|s| s.dt = dt);
        assert!(poll(editor.as_mut()).is_pending());
      }
      if capture == 3 {
        crate::capture::save_frame(
          &output.join("driver-hover-recovery-torso-selected.png"),
          &capture_editor_frame(app),
        )
        .unwrap();
      }
    }
    send(app, window, KeyCode::Enter, ButtonState::Pressed, None);
    platform::test_state(|s| s.dt = 0.0);
    let Poll::Ready(Ok(Some(build))) = poll(editor.as_mut()) else {
      panic!("hover/capture row commit")
    };
    let mut expected = original.clone();
    if capture == 0 {
      expected.torso = advance(2, expected.torso)
    } else {
      expected.face = advance(1, expected.face)
    }
    if recover_hover {
      expected.torso = advance(2, expected.torso)
    }
    assert_eq!(
      build, expected,
      "hover must not edit a row or steal active capture"
    );
  }
  // The child original-router proof specifically covered Mix, not only the
  // generic bottom-button class. Verify its actual native four-part operation
  // survives pointer-only Torso hover for both accepted keys.
  let original_indices = [original.hat, original.face, original.torso, original.legs];
  let choices = (0..4)
    .map(|row| {
      data
        .row(row)
        .iter()
        .enumerate()
        .filter(|(i, part)| {
          profile.driver_part_allowed(part.unlock, catalog) || *i == original_indices[row]
        })
        .map(|(i, _)| i)
        .collect::<Vec<_>>()
    })
    .collect::<Vec<_>>();
  let (seed, mixed) = (1..=4096u64)
    .find_map(|seed| {
      platform::rand::srand(seed);
      let pick = |row: usize| choices[row][platform::rand::gen_range(0, choices[row].len())];
      let build = Build {
        hat: pick(0),
        face: pick(1),
        torso: pick(2),
        legs: pick(3),
      };
      (build.face != original.face).then_some((seed, build))
    })
    .unwrap();
  for key in [KeyCode::Enter, KeyCode::Space] {
    for release in [
      KeyCode::Enter,
      KeyCode::Space,
      KeyCode::ArrowRight,
      KeyCode::ArrowDown,
    ] {
      send(app, window, release, ButtonState::Released, None);
    }
    send_mouse(app, window, Vec2::new(630.0, 470.0), ButtonState::Released);
    platform::test_state(|s| s.dt = 0.0);
    let mut editor = std::pin::pin!(crate::driver_editor::run(
      library, profile, catalog, ui, None
    ));
    assert!(poll(editor.as_mut()).is_pending());
    move_mouse(app, window, mix);
    platform::test_state(|s| s.dt = 0.0);
    assert!(poll(editor.as_mut()).is_pending());
    platform::rand::srand(seed);
    send(app, window, key, ButtonState::Pressed, None);
    platform::test_state(|s| s.dt = 0.0);
    assert!(poll(editor.as_mut()).is_pending());
    move_mouse(app, window, torso);
    platform::test_state(|s| s.dt = 0.0);
    assert!(poll(editor.as_mut()).is_pending());
    // Crossing Torso is not permission to route its arrow keys or release Mix.
    for state in [ButtonState::Pressed, ButtonState::Released] {
      send(app, window, KeyCode::ArrowRight, state, None);
      platform::test_state(|s| s.dt = 0.0);
      assert!(poll(editor.as_mut()).is_pending());
    }
    let other = if key == KeyCode::Enter {
      KeyCode::Space
    } else {
      KeyCode::Enter
    };
    send(app, window, other, ButtonState::Released, None);
    platform::test_state(|s| s.dt = 0.0);
    assert!(poll(editor.as_mut()).is_pending());
    send(app, window, key, ButtonState::Released, None);
    platform::test_state(|s| s.dt = 0.0);
    assert!(poll(editor.as_mut()).is_pending());
    // A new movement after release restores Torso focus; Enter then commits
    // the exact one-operation seeded draft rather than arming Mix a second time.
    move_mouse(app, window, Vec2::new(torso.x + 1.0, torso.y));
    platform::test_state(|s| s.dt = 0.0);
    assert!(poll(editor.as_mut()).is_pending());
    send(app, window, KeyCode::Enter, ButtonState::Pressed, None);
    let Poll::Ready(Ok(Some(build))) = poll(editor.as_mut()) else {
      panic!("Mix release did not restore idle row hover")
    };
    assert_eq!(
      build, mixed,
      "captured hover must perform exactly one Mix and no Torso edit"
    );
  }
  for key in [KeyCode::Enter, KeyCode::Space] {
    for release in [
      KeyCode::Enter,
      KeyCode::Space,
      KeyCode::ArrowRight,
      KeyCode::ArrowDown,
    ] {
      send(app, window, release, ButtonState::Released, None);
    }
    send_mouse(app, window, Vec2::new(630.0, 470.0), ButtonState::Released);
    let mut editor = std::pin::pin!(crate::driver_editor::run(
      library, profile, catalog, ui, None
    ));
    assert!(poll(editor.as_mut()).is_pending());
    let commit = ui.driver_action_bounds("gonext", 10, "buttonra").unwrap();
    move_mouse(app, window, Vec2::new(commit.x + 1.0, commit.y + 16.0));
    assert!(poll(editor.as_mut()).is_pending());
    send(app, window, key, ButtonState::Pressed, None);
    assert!(poll(editor.as_mut()).is_pending());
    move_mouse(app, window, torso);
    assert!(poll(editor.as_mut()).is_pending());
    let other = if key == KeyCode::Enter {
      KeyCode::Space
    } else {
      KeyCode::Enter
    };
    send(app, window, other, ButtonState::Released, None);
    assert!(
      poll(editor.as_mut()).is_pending(),
      "wrong release must retain bottom capture"
    );
    send(app, window, key, ButtonState::Released, None);
    let Poll::Ready(Ok(Some(build))) = poll(editor.as_mut()) else {
      panic!("hover stole armed Make License")
    };
    assert_eq!(build, original);
  }
  assert_eq!(serde_json::to_vec(profile).unwrap(), before);
  assert_eq!(std::fs::read(output.join("profile.json")).unwrap(), saved);
  let report = serde_json::json!({"mode":"scripted_offscreen_native_not_physical_or_audible",
    "native_cases":9,"idle_row_hover":true,"keyboard_and_mouse_arrow_capture":true,
    "matching_release_restores_pointer_only_row_hover":true,"mix_enter_and_space_hover_capture":true,
    "make_license_enter_and_space_hover_capture":true,"exact_returned_drafts":true,
    "profile_and_file_unchanged":true});
  std::fs::write(
    output.join("driver-hover-acceptance.json"),
    serde_json::to_vec_pretty(&report).unwrap(),
  )
  .unwrap();
  send(app, window, KeyCode::Enter, ButtonState::Released, None);
  send(app, window, KeyCode::Space, ButtonState::Released, None);
}
