//! Scripted native keyboard activation with real application-owned GPU photographs.
use crate::editor_gpu_test::{capture_editor_frame, logical_pixel, poll, send};
use crate::{license_snapshot_test::finish_readback, menu_ui::MenuUi, platform, profile::Profile};
use bevy::{input::ButtonState, prelude::*};
use lrformats::library::Library;
use std::{future::Future, path::Path, pin::Pin, task::Poll};

fn input(
  app: &mut App,
  window: Entity,
  editor: Pin<&mut impl Future<Output = Result<bool, String>>>,
  key: KeyCode,
  state: ButtonState,
  text: Option<&str>,
) -> Poll<Result<bool, String>> {
  send(app, window, key, state, text);
  platform::test_state(|s| s.dt = 0.0);
  poll(editor)
}

fn navigate(
  app: &mut App,
  window: Entity,
  mut editor: Pin<&mut impl Future<Output = Result<bool, String>>>,
  key: KeyCode,
) {
  for state in [ButtonState::Pressed, ButtonState::Released] {
    assert!(input(app, window, editor.as_mut(), key, state, None).is_pending());
  }
}

fn no_photo() {
  assert!(!platform::test_state(
    |s| s.capture_requested || s.capture_inflight
  ));
}

fn one_activation() {
  let commands = platform::test_state(|s| std::mem::take(&mut s.audio_commands));
  assert!(matches!(commands.as_slice(),
    [platform::audio::AudioCommand::Play(_, false, gain)] if *gain > 0.0));
}

fn pixels(image: &platform::Image) -> Vec<u8> {
  (0..139)
    .flat_map(|y| {
      (0..143).flat_map(move |x| logical_pixel(image, Vec2::new(375.0 + x as f32, 82.0 + y as f32)))
    })
    .collect()
}

pub fn verify(
  app: &mut App,
  window: Entity,
  library: &Library,
  ui: &MenuUi,
  profile: &Profile,
  output: &Path,
) {
  let animation = lrformats::animation::parse(
    library
      .find_at("CMAMAN.ADB", "MENUDATA", "MENUDATA")
      .unwrap(),
  )
  .unwrap();
  let clip = &animation.clips[0];
  // The live editor clamps dt to 0.1, so one huge dt cannot settle the clip.
  let settle = (f32::from(clip.duration) / clip.frames_per_second / 0.1).ceil() as usize + 2;
  assert!(settle < 600);
  for key in [KeyCode::Enter, KeyCode::Space] {
    // Release all keys used by this unit before opening each independent session.
    for release in [
      KeyCode::Enter,
      KeyCode::Space,
      KeyCode::Tab,
      KeyCode::ArrowDown,
      KeyCode::ArrowUp,
      KeyCode::KeyA,
      KeyCode::F5,
      KeyCode::Escape,
    ] {
      send(app, window, release, ButtonState::Released, None);
    }
    let mut edited = profile.clone();
    edited.name = "KEY".into();
    edited.license_expression = 5;
    edited.license_photo = None;
    let photograph = {
      let mut editor = std::pin::pin!(crate::license_editor::run(library, &mut edited, ui, None));
      assert!(poll(editor.as_mut()).is_pending());
      // Space in Name remains text; no assumptions about Name's Enter behavior.
      assert!(input(
        app,
        window,
        editor.as_mut(),
        KeyCode::Space,
        ButtonState::Pressed,
        Some(" ")
      )
      .is_pending());
      no_photo();
      assert!(input(
        app,
        window,
        editor.as_mut(),
        KeyCode::Space,
        ButtonState::Released,
        None
      )
      .is_pending());
      assert!(input(
        app,
        window,
        editor.as_mut(),
        KeyCode::KeyA,
        ButtonState::Pressed,
        Some("a")
      )
      .is_pending());
      assert!(input(
        app,
        window,
        editor.as_mut(),
        KeyCode::KeyA,
        ButtonState::Released,
        None
      )
      .is_pending());
      navigate(app, window, editor.as_mut(), KeyCode::Tab); // Name -> Snapshot
      for _ in 0..settle {
        app.update();
        platform::sync(app.world_mut());
        platform::test_state(|s| s.dt = 0.1);
        assert!(poll(editor.as_mut()).is_pending());
      }
      platform::test_state(|s| {
        s.dt = 0.0;
        s.audio_commands.clear();
      });
      assert!(input(
        app,
        window,
        editor.as_mut(),
        key,
        ButtonState::Pressed,
        None
      )
      .is_pending());
      no_photo();
      one_activation();
      // Duplicate key-down and an ordinary held frame must not re-activate.
      assert!(input(
        app,
        window,
        editor.as_mut(),
        key,
        ButtonState::Pressed,
        None
      )
      .is_pending());
      app.update();
      platform::sync(app.world_mut());
      platform::test_state(|s| s.dt = 0.0);
      assert!(poll(editor.as_mut()).is_pending());
      assert!(platform::test_state(|s| s.audio_commands.is_empty()));
      no_photo();
      let other = if key == KeyCode::Enter {
        KeyCode::Space
      } else {
        KeyCode::Enter
      };
      assert!(input(
        app,
        window,
        editor.as_mut(),
        other,
        ButtonState::Pressed,
        None
      )
      .is_pending());
      assert!(input(
        app,
        window,
        editor.as_mut(),
        other,
        ButtonState::Released,
        None
      )
      .is_pending());
      assert!(platform::test_state(|s| s.audio_commands.is_empty()));
      no_photo();
      // Navigation cancels the armed Snapshot even after returning to it.
      navigate(app, window, editor.as_mut(), KeyCode::ArrowDown);
      navigate(app, window, editor.as_mut(), KeyCode::ArrowUp);
      assert!(input(
        app,
        window,
        editor.as_mut(),
        key,
        ButtonState::Released,
        None
      )
      .is_pending());
      no_photo();
      platform::test_state(|s| s.audio_commands.clear());
      assert!(input(
        app,
        window,
        editor.as_mut(),
        key,
        ButtonState::Pressed,
        None
      )
      .is_pending());
      one_activation();
      no_photo();
      let before = capture_editor_frame(app);
      if key == KeyCode::Enter {
        crate::capture::save_frame(&output.join("license-keyboard-before-release.png"), &before)
          .unwrap();
      }
      // Accepted keyboard down captures pointer motion just like a mouse down:
      // hovering Name cannot steal Snapshot focus or swallow matching release.
      let name_bounds = ui.license_name_bounds().unwrap();
      crate::editor_gpu_test::move_mouse(
        app,
        window,
        Vec2::new(name_bounds.x + 1.0, name_bounds.y + 1.0),
      );
      platform::test_state(|s| s.dt = 0.0);
      assert!(poll(editor.as_mut()).is_pending());
      no_photo();
      assert!(input(
        app,
        window,
        editor.as_mut(),
        key,
        ButtonState::Released,
        None
      )
      .is_pending());
      assert!(platform::test_state(
        |s| s.capture_requested || s.capture_inflight
      ));
      let after = finish_readback(app, editor.as_mut());
      assert_ne!(
        pixels(&before),
        pixels(&after),
        "matching release must change the photographed expression"
      );
      assert!(platform::test_state(|s| s.audio_commands.is_empty()));
      if key == KeyCode::Enter {
        crate::capture::save_frame(&output.join("license-keyboard-after-release.png"), &after)
          .unwrap();
      }
      navigate(app, window, editor.as_mut(), KeyCode::Tab); // Snapshot -> BuildCar
      platform::test_state(|s| s.audio_commands.clear());
      assert!(input(
        app,
        window,
        editor.as_mut(),
        KeyCode::Enter,
        ButtonState::Pressed,
        None
      )
      .is_pending());
      one_activation();
      no_photo();
      assert!(matches!(
        input(
          app,
          window,
          editor.as_mut(),
          KeyCode::Enter,
          ButtonState::Released,
          None
        ),
        Poll::Ready(Ok(true))
      ));
      after
    };
    assert_eq!(edited.name, "KEY A");
    assert_eq!(
      edited.license_expression, 0,
      "only the successful Snapshot release advances 5 -> 0"
    );
    let photo = edited.license_photo.as_ref().unwrap();
    assert_eq!((photo.width, photo.height), (143, 139));
    assert_eq!(photo.rgba, pixels(&photograph));
    let save = output.join(if key == KeyCode::Enter {
      "license-keyboard-enter-profile.json"
    } else {
      "license-keyboard-space-profile.json"
    });
    // Saving syncs the active garage slot on a clone; normalize the test's
    // in-memory baseline too before comparing the full restored profile.
    edited.sync_racer();
    edited.save(&save).unwrap();
    let baseline = serde_json::to_vec(&edited).unwrap();
    let file_before = std::fs::read(&save).unwrap();
    assert_eq!(
      serde_json::to_vec(&Profile::load(&save).unwrap()).unwrap(),
      baseline
    );
    {
      let mut editor = std::pin::pin!(crate::license_editor::run(library, &mut edited, ui, None));
      assert!(poll(editor.as_mut()).is_pending());
      assert!(input(
        app,
        window,
        editor.as_mut(),
        KeyCode::KeyA,
        ButtonState::Pressed,
        Some("z")
      )
      .is_pending());
      assert!(input(
        app,
        window,
        editor.as_mut(),
        KeyCode::KeyA,
        ButtonState::Released,
        None
      )
      .is_pending());
      navigate(app, window, editor.as_mut(), KeyCode::Tab);
      assert!(input(
        app,
        window,
        editor.as_mut(),
        key,
        ButtonState::Pressed,
        None
      )
      .is_pending());
      assert!(input(
        app,
        window,
        editor.as_mut(),
        key,
        ButtonState::Released,
        None
      )
      .is_pending());
      let _dirty_photo = finish_readback(app, editor.as_mut());
      navigate(app, window, editor.as_mut(), KeyCode::Tab);
      navigate(app, window, editor.as_mut(), KeyCode::Tab); // BuildDriver
      platform::test_state(|s| s.audio_commands.clear());
      assert!(input(
        app,
        window,
        editor.as_mut(),
        key,
        ButtonState::Pressed,
        None
      )
      .is_pending());
      one_activation();
      assert!(matches!(
        input(
          app,
          window,
          editor.as_mut(),
          key,
          ButtonState::Released,
          None
        ),
        Poll::Ready(Ok(false))
      ));
    }
    assert_eq!(
      serde_json::to_vec(&edited).unwrap(),
      baseline,
      "BuildDriver must discard pending name, expression and photograph"
    );
    assert_eq!(std::fs::read(&save).unwrap(), file_before);
  }
}
