//! Real driver-editor keyboard paths; called by the parent's native GPU harness.
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

fn input<F: Future<Output = Result<Option<Build>, String>>>(
  app: &mut App,
  window: Entity,
  editor: Pin<&mut F>,
  key: KeyCode,
  state: ButtonState,
) -> Poll<Result<Option<Build>, String>> {
  send(app, window, key, state, None);
  platform::test_state(|s| s.dt = 0.0);
  poll(editor)
}

fn navigate<F: Future<Output = Result<Option<Build>, String>>>(
  app: &mut App,
  window: Entity,
  mut editor: Pin<&mut F>,
  key: KeyCode,
) {
  for state in [ButtonState::Pressed, ButtonState::Released] {
    assert!(input(app, window, editor.as_mut(), key, state).is_pending());
  }
}

fn silent() {
  assert!(platform::test_state(|s| s.audio_commands.is_empty()));
}

fn activation() {
  let commands = platform::test_state(|s| std::mem::take(&mut s.audio_commands));
  assert!(matches!(commands.as_slice(),
    [platform::audio::AudioCommand::Play(_, false, gain)] if *gain > 0.0));
}

fn no_snapshot() {
  assert!(!platform::test_state(
    |s| s.capture_requested || s.capture_inflight
  ));
}

fn reset(app: &mut App, window: Entity) {
  for key in [
    KeyCode::Enter,
    KeyCode::Space,
    KeyCode::Tab,
    KeyCode::ArrowDown,
    KeyCode::ArrowUp,
    KeyCode::Escape,
  ] {
    send(app, window, key, ButtonState::Released, None);
  }
  send_mouse(app, window, Vec2::new(630.0, 470.0), ButtonState::Released);
  platform::test_state(|s| {
    s.dt = 0.0;
    s.audio_commands.clear();
  });
}

fn region(image: &platform::Image, rect: platform::prelude::Rect) -> Vec<[u8; 4]> {
  let mut result = Vec::new();
  for y in rect.y.ceil() as i32..(rect.y + rect.h).floor() as i32 {
    for x in rect.x.ceil() as i32..(rect.x + rect.w).floor() as i32 {
      result.push(logical_pixel(image, Vec2::new(x as f32, y as f32)));
    }
  }
  result
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
  assert!(ui.audio.is_some());
  let baseline = serde_json::to_vec(profile).unwrap();
  let save = output.join("driver-keyboard-profile.json");
  profile.save(&save).unwrap();
  let saved = std::fs::read(&save).unwrap();
  let original = profile.custom_driver.clone().unwrap_or_default();
  let data = Data::load(library).unwrap();
  let original_indices = [original.hat, original.face, original.torso, original.legs];
  let choices: Vec<Vec<usize>> = (0..4)
    .map(|row| {
      data
        .row(row)
        .iter()
        .enumerate()
        .filter(|(i, part)| {
          profile.driver_part_allowed(part.unlock, catalog) || *i == original_indices[row]
        })
        .map(|(i, _)| i)
        .collect()
    })
    .collect();
  // Select a deterministic seed that changes Face. Do not assume every random
  // draw must differ: the editor is allowed to choose its existing parts.
  assert!(choices[1].len() > 1);
  let (seed, expected) = (1..=4096u64)
    .find_map(|seed| {
      platform::rand::srand(seed);
      let pick = |r: usize| choices[r][platform::rand::gen_range(0usize, choices[r].len())];
      let build = Build {
        hat: pick(0),
        face: pick(1),
        torso: pick(2),
        legs: pick(3),
      };
      (build.face != original.face).then_some((seed, build))
    })
    .expect("no deterministic face-changing Mix seed");
  let face_rect = ui.driver_selector(1).unwrap().1;
  let mix_rect = ui.driver_action_bounds("mix", 0x38, "nubutton").unwrap();

  for key in [KeyCode::Enter, KeyCode::Space] {
    let other = if key == KeyCode::Enter {
      KeyCode::Space
    } else {
      KeyCode::Enter
    };
    reset(app, window);
    {
      let mut editor = std::pin::pin!(crate::driver_editor::run(
        library, profile, catalog, ui, None
      ));
      assert!(poll(editor.as_mut()).is_pending());
      for _ in 0..4 {
        navigate(app, window, editor.as_mut(), KeyCode::Tab);
      }
      platform::test_state(|s| s.audio_commands.clear());
      platform::rand::srand(seed);
      assert!(input(app, window, editor.as_mut(), key, ButtonState::Pressed).is_pending());
      activation();
      for _ in 0..2 {
        assert!(input(app, window, editor.as_mut(), key, ButtonState::Pressed).is_pending());
        silent();
      }
      assert!(input(app, window, editor.as_mut(), other, ButtonState::Pressed).is_pending());
      assert!(input(app, window, editor.as_mut(), other, ButtonState::Released).is_pending());
      silent();
      no_snapshot();
      navigate(app, window, editor.as_mut(), KeyCode::ArrowDown);
      navigate(app, window, editor.as_mut(), KeyCode::ArrowUp);
      platform::test_state(|s| s.audio_commands.clear());
      assert!(input(app, window, editor.as_mut(), key, ButtonState::Released).is_pending());
      silent();
      // Commit the untouched draft: catches premature Mix on down/repeat or
      // a stale release after leaving and returning to the same button.
      navigate(app, window, editor.as_mut(), KeyCode::Tab);
      platform::test_state(|s| s.audio_commands.clear());
      assert!(input(app, window, editor.as_mut(), key, ButtonState::Pressed).is_pending());
      activation();
      assert!(input(app, window, editor.as_mut(), other, ButtonState::Released).is_pending());
      silent();
      let Poll::Ready(Ok(Some(build))) =
        input(app, window, editor.as_mut(), key, ButtonState::Released)
      else {
        panic!("Make License did not commit on matching release")
      };
      assert_eq!(build, original);
      data.validate(&build).unwrap();
      silent();
      no_snapshot();
    }

    reset(app, window);
    {
      let mut editor = std::pin::pin!(crate::driver_editor::run(
        library, profile, catalog, ui, None
      ));
      assert!(poll(editor.as_mut()).is_pending());
      for _ in 0..4 {
        navigate(app, window, editor.as_mut(), KeyCode::Tab);
      }
      platform::test_state(|s| s.audio_commands.clear());
      platform::rand::srand(seed);
      assert!(input(app, window, editor.as_mut(), key, ButtonState::Pressed).is_pending());
      activation();
      no_snapshot();
      let armed = capture_editor_frame(app);
      if key == KeyCode::Enter {
        crate::capture::save_frame(&output.join("driver-keyboard-mix-armed.png"), &armed).unwrap();
      }
      assert!(input(app, window, editor.as_mut(), key, ButtonState::Released).is_pending());
      silent();
      no_snapshot();
      // Mix mutates after drawing; the following zero-time frame presents the
      // released style and rebuilt draft without advancing preview animation.
      app.update();
      platform::sync(app.world_mut());
      platform::test_state(|s| s.dt = 0.0);
      assert!(poll(editor.as_mut()).is_pending());
      silent();
      let released = capture_editor_frame(app);
      assert_ne!(
        region(&armed, face_rect),
        region(&released, face_rect),
        "matching Mix release must rebuild the real face row"
      );
      assert_ne!(
        region(&armed, mix_rect),
        region(&released, mix_rect),
        "armed and released Mix styles must visibly differ"
      );
      if key == KeyCode::Enter {
        crate::capture::save_frame(&output.join("driver-keyboard-mix-released.png"), &released)
          .unwrap();
      }
      navigate(app, window, editor.as_mut(), KeyCode::Tab);
      platform::test_state(|s| s.audio_commands.clear());
      assert!(input(app, window, editor.as_mut(), key, ButtonState::Pressed).is_pending());
      activation();
      let Poll::Ready(Ok(Some(build))) =
        input(app, window, editor.as_mut(), key, ButtonState::Released)
      else {
        panic!("mixed draft did not commit")
      };
      data.validate(&build).unwrap();
      assert_eq!(
        build, expected,
        "Mix must consume exactly one four-part random draw"
      );
      silent();
    }

    reset(app, window);
    {
      let mut editor = std::pin::pin!(crate::driver_editor::run(
        library, profile, catalog, ui, None
      ));
      assert!(poll(editor.as_mut()).is_pending());
      for _ in 0..4 {
        navigate(app, window, editor.as_mut(), KeyCode::Tab);
      }
      platform::rand::srand(seed);
      assert!(input(app, window, editor.as_mut(), key, ButtonState::Pressed).is_pending());
      assert!(input(app, window, editor.as_mut(), key, ButtonState::Released).is_pending());
      // Mix -> Make License -> Cancel: six Tabs total from initial Hat.
      for _ in 0..2 {
        navigate(app, window, editor.as_mut(), KeyCode::Tab);
      }
      platform::test_state(|s| s.audio_commands.clear());
      assert!(input(app, window, editor.as_mut(), key, ButtonState::Pressed).is_pending());
      activation();
      assert!(input(app, window, editor.as_mut(), key, ButtonState::Pressed).is_pending());
      assert!(input(app, window, editor.as_mut(), other, ButtonState::Released).is_pending());
      silent();
      assert!(
        input(app, window, editor.as_mut(), key, ButtonState::Released).is_pending(),
        "changed parts must open the discard warning, not exit immediately"
      );
      app.update();
      platform::sync(app.world_mut());
      platform::test_state(|s| s.dt = 0.0);
      assert!(poll(editor.as_mut()).is_pending());
      // The warning defaults to CANCEL: cancel the discard, keep the edited
      // draft, then prove it through a normal MakeLicense commit.
      assert!(input(app, window, editor.as_mut(), key, ButtonState::Pressed).is_pending());
      activation();
      assert!(input(app, window, editor.as_mut(), key, ButtonState::Released).is_pending());
      navigate(app, window, editor.as_mut(), KeyCode::ArrowUp);
      platform::test_state(|s| s.audio_commands.clear());
      assert!(input(app, window, editor.as_mut(), key, ButtonState::Pressed).is_pending());
      activation();
      let Poll::Ready(Ok(Some(build))) =
        input(app, window, editor.as_mut(), key, ButtonState::Released)
      else {
        panic!("kept draft did not commit after cancelling the discard")
      };
      assert_eq!(build, expected);
      silent();
      no_snapshot();
    }
    assert_eq!(serde_json::to_vec(profile).unwrap(), baseline);
    assert_eq!(std::fs::read(&save).unwrap(), saved);
  }
  reset(app, window);
}
