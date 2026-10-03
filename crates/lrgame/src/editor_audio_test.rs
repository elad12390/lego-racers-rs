//! Actual original-asset editor loops driven through Bevy's native input boundary.
use crate::{
  game_catalog::Catalog, menu_audio::MenuAudio, menu_ui::MenuUi, platform, profile::Profile,
};
use bevy::{
  input::{
    keyboard::{Key, KeyboardInput},
    ButtonState,
  },
  prelude::*,
};
use lrformats::{library::Library, pcm, wav};
use std::{
  future::Future,
  pin::Pin,
  task::{Context, Poll, Waker},
};

fn poll<T>(future: Pin<&mut impl Future<Output = T>>) -> Poll<T> {
  future.poll(&mut Context::from_waker(Waker::noop()))
}

fn send_key(app: &mut App, window: Entity, key: KeyCode, state: ButtonState) {
  app.world_mut().write_message(KeyboardInput {
    key_code: key,
    logical_key: Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified),
    state,
    text: None,
    repeat: false,
    window,
  });
  app.update();
}

struct EditorFixture {
  library: Library,
  ui: MenuUi,
  effects: Vec<platform::audio::Sound>,
  app: App,
  window: Entity,
}

fn setup() -> EditorFixture {
  let directory =
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extracted/Program_Files_Group");
  let library = Library::open(directory.join("LEGO.JAM")).unwrap();
  let mut ui = MenuUi::load(&library).unwrap();
  let mut audio = {
    let mut load = std::pin::pin!(MenuAudio::load(&library, &directory));
    let Poll::Ready(Ok(audio)) = poll(load.as_mut()) else {
      panic!("original menu audio load failed")
    };
    audio
  };
  audio.update(true, false, true);
  ui.audio = Some(audio);

  // Decode the actual menu effects, so the assertions distinguish navigation,
  // activation and cancellation rather than merely counting any audio command.
  let mut effects = Vec::new();
  for file in ["HILIGHT1.PCM", "ACTIVATE.PCM", "BACKUP.PCM"] {
    let decoded = pcm::decode(library.find_at(file, "MENUDATA", "SOUNDS").unwrap()).unwrap();
    let bytes = wav::encode_mono_16(decoded.sample_rate, &decoded.samples);
    let mut load = std::pin::pin!(platform::audio::load_sound_from_bytes(&bytes));
    let Poll::Ready(Ok(sound)) = poll(load.as_mut()) else {
      panic!("original effect load failed")
    };
    effects.push(sound);
  }
  let mut app = App::new();
  app.add_plugins((MinimalPlugins, bevy::input::InputPlugin));
  app.init_resource::<platform::InputEvents>();
  app.add_systems(Update, platform::sync);
  let window = app
    .world_mut()
    .spawn((Window::default(), bevy::window::PrimaryWindow))
    .id();
  app.update();
  platform::test_state(|s| s.audio_commands.clear());
  EditorFixture {
    library,
    ui,
    effects,
    app,
    window,
  }
}

fn expect_effect(sound: &platform::audio::Sound) {
  let commands = platform::test_state(|s| std::mem::take(&mut s.audio_commands));
  assert!(
    matches!(commands.as_slice(), [platform::audio::AudioCommand::Play(id, false, gain)] if *id == sound.0 && *gain > 0.0)
  );
}

#[test]
fn original_builder_keys_emit_sfx_once_and_respect_sound_off_through_real_editor_loop() {
  let EditorFixture {
    library,
    mut ui,
    effects,
    mut app,
    window,
  } = setup();
  let catalog = Catalog::load(&library).unwrap();
  let profile = Profile::default();
  {
    let mut editor = std::pin::pin!(crate::garage_builder::run(
      &library, &catalog, &profile, &mut ui, None
    ));
    assert!(poll(editor.as_mut()).is_pending());
    assert!(
      platform::test_state(|s| s.audio_commands.is_empty()),
      "idle editor emitted SFX"
    );
    for (key, effect) in [
      (KeyCode::KeyC, 0),
      (KeyCode::KeyR, 0),
      (KeyCode::ArrowRight, 0),
      (KeyCode::ArrowLeft, 0),
      (KeyCode::ArrowDown, 0),
      (KeyCode::ArrowUp, 0),
      (KeyCode::PageUp, 0),
      (KeyCode::PageDown, 0),
      (KeyCode::Tab, 0),
      (KeyCode::KeyV, 0),
      (KeyCode::KeyN, 0),
      (KeyCode::Enter, 1),
      (KeyCode::Backspace, 1),
    ] {
      send_key(&mut app, window, key, ButtonState::Pressed);
      assert!(poll(editor.as_mut()).is_pending());
      let commands = platform::test_state(|s| std::mem::take(&mut s.audio_commands));
      assert_eq!(commands.len(), 1, "{key:?} must emit exactly one effect");
      assert!(
        matches!(commands[0], platform::audio::AudioCommand::Play(id, false, gain) if id == effects[effect].0 && gain > 0.0)
      );
      app.update();
      assert!(poll(editor.as_mut()).is_pending());
      assert!(
        platform::test_state(|s| s.audio_commands.is_empty()),
        "held {key:?} repeats SFX"
      );
      send_key(&mut app, window, key, ButtonState::Released);
      assert!(poll(editor.as_mut()).is_pending());
    }
    send_key(&mut app, window, KeyCode::Escape, ButtonState::Pressed);
    assert!(
      matches!(poll(editor.as_mut()), Poll::Ready(Ok(None))),
      "Escape must cancel the draft"
    );
    let commands = platform::test_state(|s| std::mem::take(&mut s.audio_commands));
    assert!(
      matches!(commands.as_slice(), [platform::audio::AudioCommand::Play(id, false, gain)] if *id == effects[2].0 && *gain > 0.0)
    );
  }

  ui.audio.as_mut().unwrap().update(true, false, false);
  platform::test_state(|s| s.audio_commands.clear());
  app.update();
  let mut editor = std::pin::pin!(crate::garage_builder::run(
    &library, &catalog, &profile, &mut ui, None
  ));
  assert!(poll(editor.as_mut()).is_pending());
  send_key(&mut app, window, KeyCode::KeyS, ButtonState::Pressed);
  assert!(poll(editor.as_mut()).is_pending()); // Render/advance before the save handoff.
  app.update();
  let Poll::Ready(Ok(Some((build, _)))) = poll(editor.as_mut()) else {
    panic!("S must save the draft")
  };
  lrsim::brick_build::BuilderData::load(&library)
    .unwrap()
    .validate(&build)
    .unwrap();
  assert!(
    platform::test_state(|s| s.audio_commands.is_empty()),
    "sound-off editor emitted SFX"
  );
}

#[test]
fn original_driver_and_license_keyboard_handoffs_emit_sfx_without_repeating_or_mutating_cancelled_drafts(
) {
  let EditorFixture {
    library,
    mut ui,
    effects,
    mut app,
    window,
  } = setup();
  let catalog = Catalog::load(&library).unwrap();
  let mut profile = Profile::default();
  let driver = {
    let mut editor = std::pin::pin!(crate::driver_editor::run(
      &library, &profile, &catalog, &ui, None
    ));
    assert!(poll(editor.as_mut()).is_pending());
    assert!(platform::test_state(|s| s.audio_commands.is_empty()));
    for key in [
      KeyCode::ArrowDown,
      KeyCode::ArrowRight,
      KeyCode::ArrowUp,
      KeyCode::ArrowLeft,
    ] {
      send_key(&mut app, window, key, ButtonState::Pressed);
      assert!(poll(editor.as_mut()).is_pending());
      expect_effect(&effects[0]);
      app.update();
      assert!(poll(editor.as_mut()).is_pending());
      assert!(
        platform::test_state(|s| s.audio_commands.is_empty()),
        "held driver selection repeats SFX"
      );
      send_key(&mut app, window, key, ButtonState::Released);
      assert!(poll(editor.as_mut()).is_pending());
    }
    send_key(&mut app, window, KeyCode::Enter, ButtonState::Pressed);
    let Poll::Ready(Ok(Some(build))) = poll(editor.as_mut()) else {
      panic!("Enter must accept the driver draft")
    };
    expect_effect(&effects[1]);
    crate::custom_driver::Data::load(&library)
      .unwrap()
      .validate(&build)
      .unwrap();
    build
  };
  send_key(&mut app, window, KeyCode::Enter, ButtonState::Released);
  profile.custom_driver = Some(driver);
  let saved = serde_json::to_vec(&profile).unwrap();
  {
    let mut editor = std::pin::pin!(crate::license_editor::run(
      &library,
      &mut profile,
      &ui,
      None
    ));
    assert!(poll(editor.as_mut()).is_pending());
    send_key(&mut app, window, KeyCode::Escape, ButtonState::Pressed);
    assert!(matches!(poll(editor.as_mut()), Poll::Ready(Ok(false))));
    expect_effect(&effects[2]);
  }
  assert_eq!(
    serde_json::to_vec(&profile).unwrap(),
    saved,
    "cancelled license mutated the profile"
  );
  send_key(&mut app, window, KeyCode::Escape, ButtonState::Released);
  ui.audio.as_mut().unwrap().update(true, false, false);
  platform::test_state(|s| s.audio_commands.clear());
  {
    let mut editor = std::pin::pin!(crate::license_editor::run(
      &library,
      &mut profile,
      &ui,
      None
    ));
    assert!(poll(editor.as_mut()).is_pending());
    send_key(&mut app, window, KeyCode::Enter, ButtonState::Pressed);
    assert!(matches!(poll(editor.as_mut()), Poll::Ready(Ok(true))));
    assert!(
      platform::test_state(|s| s.audio_commands.is_empty()),
      "Sound Off license emitted SFX"
    );
  }
  send_key(&mut app, window, KeyCode::Enter, ButtonState::Released);
  {
    let mut editor = std::pin::pin!(crate::driver_editor::run(
      &library, &profile, &catalog, &ui, None
    ));
    assert!(poll(editor.as_mut()).is_pending());
    send_key(&mut app, window, KeyCode::Escape, ButtonState::Pressed);
    assert!(matches!(poll(editor.as_mut()), Poll::Ready(Ok(None))));
    assert!(
      platform::test_state(|s| s.audio_commands.is_empty()),
      "Sound Off driver emitted SFX"
    );
  }
}
