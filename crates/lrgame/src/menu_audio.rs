//! Original menu/builder music and original MENUDATA/SOUNDS button effects.
use crate::platform::audio::{self, PlaySoundParams, Sound};
use lrformats::{library::Library, pcm, wav};
use serde::Deserialize;
#[derive(Deserialize)]
struct Rules {
  menu_music_file: String,
  builder_music_file: String,
  menu_move_file: String,
  menu_activate_file: String,
  menu_cancel_file: String,
  menu_music_gain: f32,
  menu_sfx_gain: f32,
}
pub struct MenuAudio {
  music: [Sound; 2],
  move_sound: Sound,
  activate_sound: Sound,
  cancel_sound: Sound,
  active: Option<usize>,
  music_gain: f32,
  sfx_gain: f32,
  sfx_enabled: bool,
}
impl MenuAudio {
  pub async fn load(library: &Library, directory: &std::path::Path) -> Result<Self, String> {
    let rules: Rules = serde_json::from_str(include_str!("../../../assets/native/audio.json"))
      .map_err(|e| e.to_string())?;
    if [rules.menu_music_gain, rules.menu_sfx_gain]
      .iter()
      .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
    {
      return Err("invalid menu audio gain".into());
    }
    async fn effect(library: &Library, name: &str) -> Result<Sound, String> {
      let decoded = pcm::decode(
        library
          .find_at(name, "MENUDATA", "SOUNDS")
          .ok_or_else(|| format!("missing original menu effect {name}"))?,
      )
      .map_err(|e| e.to_string())?;
      audio::load_sound_from_bytes(&wav::encode_mono_16(decoded.sample_rate, &decoded.samples))
        .await
    }
    Ok(Self {
      music: [
        crate::audio_assets::tune(directory, &rules.menu_music_file).await?,
        crate::audio_assets::tune(directory, &rules.builder_music_file).await?,
      ],
      move_sound: effect(library, &rules.menu_move_file).await?,
      activate_sound: effect(library, &rules.menu_activate_file).await?,
      cancel_sound: effect(library, &rules.menu_cancel_file).await?,
      active: None,
      music_gain: rules.menu_music_gain,
      sfx_gain: rules.menu_sfx_gain,
      sfx_enabled: false,
    })
  }
  pub fn update(&mut self, builder: bool, music_enabled: bool, sfx_enabled: bool) {
    if self.sfx_enabled && !sfx_enabled {
      for sound in [&self.move_sound, &self.activate_sound, &self.cancel_sound] {
        audio::stop_sound(sound);
      }
    }
    self.sfx_enabled = sfx_enabled;
    let next = music_enabled.then_some(usize::from(builder));
    if self.active != next {
      self.stop_music();
      if let Some(index) = next {
        audio::play_sound(
          &self.music[index],
          PlaySoundParams {
            looped: true,
            volume: self.music_gain,
          },
        );
      }
      self.active = next;
    }
  }
  pub fn stop_music(&mut self) {
    if let Some(index) = self.active.take() {
      audio::stop_sound(&self.music[index]);
    }
  }
  fn effect(&self, sound: &Sound) {
    if self.sfx_enabled {
      audio::play_sound(
        sound,
        PlaySoundParams {
          looped: false,
          volume: self.sfx_gain,
        },
      );
    }
  }
  pub fn moved(&self) {
    self.effect(&self.move_sound);
  }
  pub fn activated(&self) {
    self.effect(&self.activate_sound);
  }
  pub fn cancelled(&self) {
    self.effect(&self.cancel_sound);
  }
}
impl Drop for MenuAudio {
  fn drop(&mut self) {
    self.stop_music();
    for sound in [&self.move_sound, &self.activate_sound, &self.cancel_sound] {
      audio::stop_sound(sound);
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::platform::{self as input, test_state};
  use bevy::prelude::*;

  fn load() -> (Library, MenuAudio) {
    let directory =
      std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extracted/Program_Files_Group");
    let library = Library::open(directory.join("LEGO.JAM")).unwrap();
    let audio = {
      let mut future = std::pin::pin!(MenuAudio::load(&library, &directory));
      match std::future::Future::poll(
        future.as_mut(),
        &mut std::task::Context::from_waker(std::task::Waker::noop()),
      ) {
        std::task::Poll::Ready(Ok(audio)) => audio,
        _ => panic!("original menu audio must decode synchronously"),
      }
    };
    test_state(|s| s.audio_commands.clear());
    (library, audio)
  }

  #[test]
  fn menu_audio_original_music_handoff_and_independent_toggles() {
    let (_, mut audio) = load();
    test_state(|s| {
      for sound in audio.music.iter().chain([
        &audio.move_sound,
        &audio.activate_sound,
        &audio.cancel_sound,
      ]) {
        let bytes = &s.audio_bytes[sound.0];
        assert!(
          bytes[44..]
            .chunks_exact(2)
            .any(|v| i16::from_le_bytes([v[0], v[1]]) != 0),
          "original audio decoded to silence"
        );
      }
    });
    let mut world = World::new();
    world.init_resource::<audio::Backend>();
    world.init_resource::<Assets<AudioSource>>();
    audio.update(false, true, false);
    audio::flush(&mut world);
    assert_eq!(
      world
        .query::<&AudioPlayer<AudioSource>>()
        .iter(&world)
        .count(),
      1
    );
    assert_eq!(
      world
        .query::<&PlaybackSettings>()
        .single(&world)
        .unwrap()
        .volume,
      bevy::audio::Volume::Linear(audio.music_gain)
    );
    audio.update(false, true, false);
    audio::flush(&mut world);
    assert_eq!(
      world
        .query::<&AudioPlayer<AudioSource>>()
        .iter(&world)
        .count(),
      1,
      "menu music restarted or overlapped every update"
    );
    audio.update(true, true, false);
    audio::flush(&mut world);
    assert_eq!(
      world
        .query::<&AudioPlayer<AudioSource>>()
        .iter(&world)
        .count(),
      1,
      "builder transition must replace menu music"
    );
    audio.update(true, false, true);
    audio::flush(&mut world);
    assert_eq!(
      world
        .query::<&AudioPlayer<AudioSource>>()
        .iter(&world)
        .count(),
      0
    );
    audio.moved();
    audio::flush(&mut world);
    assert_eq!(
      world
        .query::<&PlaybackSettings>()
        .single(&world)
        .unwrap()
        .volume,
      bevy::audio::Volume::Linear(audio.sfx_gain),
      "SFX stays enabled with music off"
    );
    audio.update(false, true, false);
    audio::flush(&mut world);
    audio.activated();
    assert!(
      test_state(|s| s.audio_commands.is_empty()),
      "SFX toggle is independent of music"
    );
    audio.stop_music();
    audio::flush(&mut world);
    assert_eq!(
      world
        .query::<&AudioPlayer<AudioSource>>()
        .iter(&world)
        .count(),
      0,
      "disabling SFX must stop already playing menu effects"
    );
    drop(audio);
    audio::flush(&mut world);
    assert_eq!(
      world
        .query::<&AudioPlayer<AudioSource>>()
        .iter(&world)
        .count(),
      0
    );
  }

  #[test]
  fn menu_audio_original_widgets_emit_effects_on_native_key_actions_not_idle() {
    use bevy::input::{
      keyboard::{Key, KeyCode as NativeKey, KeyboardInput},
      ButtonState,
    };
    let (library, mut audio) = load();
    audio.update(false, false, true);
    let highlight = audio.move_sound.0;
    let activate = audio.activate_sound.0;
    let mut ui = crate::menu_ui::MenuUi::load(&library).unwrap();
    ui.audio = Some(audio);
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::input::InputPlugin));
    app.init_resource::<input::InputEvents>();
    app.add_systems(Update, input::sync);
    let window = app
      .world_mut()
      .spawn((Window::default(), bevy::window::PrimaryWindow))
      .id();
    let buttons = [
      ("garage", "Build".into(), true),
      ("single", "Race".into(), true),
    ];
    app.update();
    assert_eq!(ui.original_buttons("mainmenu", &buttons), None);
    assert!(test_state(|s| s.audio_commands.is_empty()));
    for (key, logical, expected, sound) in [
      (NativeKey::ArrowDown, Key::ArrowDown, None, highlight),
      (NativeKey::Enter, Key::Enter, Some(1), activate),
    ] {
      app.world_mut().write_message(KeyboardInput {
        key_code: key,
        logical_key: logical,
        state: ButtonState::Pressed,
        text: None,
        repeat: false,
        window,
      });
      app.update();
      assert_eq!(ui.original_buttons("mainmenu", &buttons), expected);
      let commands = test_state(|s| std::mem::take(&mut s.audio_commands));
      assert_eq!(commands.len(), 1);
      assert!(
        matches!(commands[0], audio::AudioCommand::Play(id, false, gain) if id == sound && gain > 0.0)
      );
    }
    app.update();
    assert_eq!(ui.original_buttons("mainmenu", &buttons), None);
    assert!(
      test_state(|s| s.audio_commands.is_empty()),
      "held key repeats must not spam effects"
    );
  }
}
