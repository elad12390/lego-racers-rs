use super::state;
#[derive(Clone)]
pub struct Sound(pub(crate) usize);
pub struct PlaySoundParams {
  pub looped: bool,
  pub volume: f32,
}
pub enum AudioCommand {
  Play(usize, bool, f32),
  Stop(usize),
  Volume(usize, f32),
}
pub async fn load_sound_from_bytes(bytes: &[u8]) -> Result<Sound, String> {
  if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
    return Err("expected original decoded PCM WAV".into());
  }
  let key = state::fingerprint(bytes);
  Ok(Sound(state::with(|s| {
    if let Some(ids) = s.audio_keys.get(&key) {
      if let Some(id) = ids.iter().find(|id| s.audio_bytes[**id] == bytes) {
        return *id;
      }
    }
    let id = s.audio_bytes.len();
    s.audio_bytes.push(bytes.to_vec());
    s.audio_keys.entry(key).or_default().push(id);
    id
  })))
}
pub fn play_sound(sound: &Sound, params: PlaySoundParams) {
  state::with(|s| {
    s.audio_commands
      .push(AudioCommand::Play(sound.0, params.looped, params.volume))
  });
}
pub fn stop_sound(sound: &Sound) {
  state::with(|s| s.audio_commands.push(AudioCommand::Stop(sound.0)));
}
pub fn set_sound_volume(sound: &Sound, volume: f32) {
  state::with(|s| s.audio_commands.push(AudioCommand::Volume(sound.0, volume)));
}

use bevy::{
  audio::{AudioSink, AudioSinkPlayback, Volume},
  prelude::*,
};
#[derive(Resource, Default)]
pub struct Backend {
  sounds: Vec<Handle<AudioSource>>,
}
#[derive(Component)]
struct SoundId(usize);
pub fn flush(world: &mut World) {
  world.resource_scope(|world, mut backend: Mut<Backend>| {
    let commands = state::with(|s| {
      for bytes in s.audio_bytes.iter().skip(backend.sounds.len()) {
        backend.sounds.push(
          world
            .resource_mut::<Assets<AudioSource>>()
            .add(AudioSource {
              bytes: bytes.clone().into(),
            }),
        );
      }
      std::mem::take(&mut s.audio_commands)
    });
    for command in commands {
      match command {
        AudioCommand::Play(id, looped, volume) => {
          world.spawn((
            AudioPlayer::new(backend.sounds[id].clone()),
            if looped {
              PlaybackSettings::LOOP
            } else {
              PlaybackSettings::DESPAWN
            }
            .with_volume(Volume::Linear(volume)),
            SoundId(id),
          ));
        }
        AudioCommand::Stop(id) => {
          let entities = world
            .query::<(Entity, &SoundId)>()
            .iter(world)
            .filter(|(_, sound)| sound.0 == id)
            .map(|(e, _)| e)
            .collect::<Vec<_>>();
          for entity in entities {
            world.despawn(entity);
          }
        }
        AudioCommand::Volume(id, volume) => {
          for (sound, sink, settings) in world
            .query::<(&SoundId, Option<&mut AudioSink>, &mut PlaybackSettings)>()
            .iter_mut(world)
          {
            if sound.0 == id {
              if let Some(mut sink) = sink {
                sink.set_volume(Volume::Linear(volume));
              } else {
                let mut settings = settings;
                settings.volume = Volume::Linear(volume);
              }
            }
          }
        }
      }
    }
  });
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  #[ignore = "requires LR_TEST_JAM and a real native audio output; all playback is muted"]
  fn bevy_native_original_audio_sink_restart_volume_and_cleanup() {
    let library = lrformats::library::Library::open(std::path::Path::new(
      &std::env::var("LR_TEST_JAM").expect("LR_TEST_JAM"),
    ))
    .unwrap();
    let decoded = lrformats::pcm::decode(library.find_in("321.PCM", "COMMON").unwrap()).unwrap();
    let wav = lrformats::wav::encode_mono_16(decoded.sample_rate, &decoded.samples);
    let mut future = std::pin::pin!(load_sound_from_bytes(&wav));
    let waker = std::task::Waker::noop();
    let sound = match std::future::Future::poll(
      future.as_mut(),
      &mut std::task::Context::from_waker(waker),
    ) {
      std::task::Poll::Ready(Ok(sound)) => sound,
      _ => panic!("decoded original sound must load synchronously"),
    };
    let mut app = App::new();
    app.add_plugins((
      MinimalPlugins,
      bevy::asset::AssetPlugin::default(),
      bevy::audio::AudioPlugin::default(),
    ));
    app.init_resource::<Backend>();
    app.finish();
    app.cleanup();
    play_sound(
      &sound,
      PlaySoundParams {
        looped: true,
        volume: 0.0,
      },
    );
    flush(app.world_mut());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
      app.update();
      if app
        .world_mut()
        .query::<&AudioSink>()
        .iter(app.world())
        .count()
        == 1
      {
        break;
      }
      assert!(
        std::time::Instant::now() < deadline,
        "real native audio sink did not start"
      );
      std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let entity = app
      .world_mut()
      .query::<(Entity, &SoundId)>()
      .iter(app.world())
      .find(|(_, id)| id.0 == sound.0)
      .unwrap()
      .0;
    let sink = app.world().get::<AudioSink>(entity).unwrap();
    assert_eq!(sink.volume(), Volume::Linear(0.0));
    sink.pause();
    assert!(sink.is_paused());
    sink.play();
    assert!(!sink.is_paused());
    set_sound_volume(&sound, 0.0);
    flush(app.world_mut());
    assert_eq!(
      app.world().get::<AudioSink>(entity).unwrap().volume(),
      Volume::Linear(0.0)
    );
    stop_sound(&sound);
    flush(app.world_mut());
    assert!(app.world().get_entity(entity).is_err());
    // A same-frame restart must remove the old voice but retain the new one,
    // even before Bevy has prepared its sink.
    play_sound(
      &sound,
      PlaySoundParams {
        looped: true,
        volume: 0.0,
      },
    );
    stop_sound(&sound);
    play_sound(
      &sound,
      PlaySoundParams {
        looped: true,
        volume: 0.0,
      },
    );
    flush(app.world_mut());
    app.update();
    assert_eq!(
      app
        .world_mut()
        .query::<&SoundId>()
        .iter(app.world())
        .count(),
      1
    );
    assert_eq!(
      app
        .world_mut()
        .query::<&AudioSink>()
        .iter(app.world())
        .count(),
      1
    );
    stop_sound(&sound);
    flush(app.world_mut());
    app.update();
    assert_eq!(
      app
        .world_mut()
        .query::<&AudioSink>()
        .iter(app.world())
        .count(),
      0
    );
  }
}
