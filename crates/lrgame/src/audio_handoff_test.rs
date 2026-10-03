//! Game-level audible integration; not compiled into standalone camera tools.
use crate::platform::audio;
use bevy::{
  audio::{AudioSink, AudioSinkPlayback},
  prelude::*,
};
use std::time::{Duration, Instant};

fn ready<T>(future: impl std::future::Future<Output = Result<T, String>>) -> T {
  let mut future = std::pin::pin!(future);
  match std::future::Future::poll(
    future.as_mut(),
    &mut std::task::Context::from_waker(std::task::Waker::noop()),
  ) {
    std::task::Poll::Ready(Ok(value)) => value,
    std::task::Poll::Ready(Err(error)) => panic!("original audio load failed: {error}"),
    std::task::Poll::Pending => panic!("original audio decoding unexpectedly yielded"),
  }
}
fn audible_voices(app: &mut App) -> Vec<AssetId<AudioSource>> {
  app
    .world_mut()
    .query::<(&AudioPlayer<AudioSource>, &AudioSink)>()
    .iter(app.world())
    .filter(|(_, sink)| !sink.empty() && !sink.is_paused() && sink.volume().to_linear() > 0.0)
    .map(|(source, _)| source.0.id())
    .collect()
}
fn wait_for_audio(app: &mut App) -> Vec<AssetId<AudioSource>> {
  let deadline = Instant::now() + Duration::from_secs(3);
  loop {
    app.update();
    let voices = audible_voices(app);
    if !voices.is_empty() {
      return voices;
    }
    assert!(
      Instant::now() < deadline,
      "real non-muted native audio sink did not start"
    );
    std::thread::sleep(Duration::from_millis(10));
  }
}
fn play_for(app: &mut App, milliseconds: u64) {
  let deadline = Instant::now() + Duration::from_millis(milliseconds);
  while Instant::now() < deadline {
    app.update();
    std::thread::sleep(Duration::from_millis(10));
  }
}

// Deliberately audible, explicit opt-in; this does not prove human hearing.
#[test]
#[ignore = "requires LR_TEST_JAM, LR_TEST_AUDIBLE=1 and a real native audio output"]
fn bevy_native_menu_builder_race_audio_handoff() {
  assert_eq!(
    std::env::var("LR_TEST_AUDIBLE").as_deref(),
    Ok("1"),
    "explicit audible opt-in required"
  );
  let jam = std::path::PathBuf::from(std::env::var("LR_TEST_JAM").expect("LR_TEST_JAM"));
  let library = lrformats::library::Library::open(&jam).unwrap();
  let directory = jam.parent().unwrap();
  let mut menu = ready(crate::menu_audio::MenuAudio::load(&library, directory));
  let mut race = ready(crate::race_audio::RaceAudio::load_settings(
    &library, "RACEC0R0", directory, true, true,
  ));
  let mut app = App::new();
  app.add_plugins((
    MinimalPlugins,
    bevy::asset::AssetPlugin::default(),
    bevy::audio::AudioPlugin::default(),
  ));
  app.init_resource::<audio::Backend>();
  app.finish();
  app.cleanup();

  menu.update(false, true, true);
  audio::flush(app.world_mut());
  let theme = wait_for_audio(&mut app);
  assert_eq!(theme.len(), 1);
  play_for(&mut app, 600);
  menu.moved();
  menu.activated();
  audio::flush(app.world_mut());
  play_for(&mut app, 200);
  menu.update(true, true, true);
  audio::flush(app.world_mut());
  let builder = wait_for_audio(&mut app);
  assert_eq!(builder.len(), 1, "menu theme overlaps builder music");
  assert_ne!(theme[0], builder[0]);
  play_for(&mut app, 600);
  menu.update(true, false, true);
  audio::flush(app.world_mut());
  app.update();
  assert!(
    audible_voices(&mut app).is_empty(),
    "music switch failed to stop builder tune"
  );
  menu.cancelled();
  audio::flush(app.world_mut());
  play_for(&mut app, 200);
  menu.update(false, true, false);
  audio::flush(app.world_mut());
  wait_for_audio(&mut app);
  menu.stop_music();
  race.start_sequence(true);
  audio::flush(app.world_mut());
  wait_for_audio(&mut app);
  assert!(
    app
      .world_mut()
      .query::<&AudioPlayer<AudioSource>>()
      .iter(app.world())
      .all(|source| source.0.id() != theme[0] && source.0.id() != builder[0]),
    "menu music survived the race handoff"
  );
  play_for(&mut app, 400);
  race.go();
  race.engine(100.0, 0.3, false);
  audio::flush(app.world_mut());
  assert!(
    wait_for_audio(&mut app).len() >= 2,
    "race music and engine did not mix"
  );
  play_for(&mut app, 600);
  race.engine(100.0, 0.3, true);
  audio::flush(app.world_mut());
  app.update();
  assert!(
    audible_voices(&mut app).is_empty(),
    "paused race still emits audio"
  );
  race.engine(100.0, 0.3, false);
  audio::flush(app.world_mut());
  wait_for_audio(&mut app);
  race.pickup(None);
  race.effect(3, 0);
  race.contact();
  audio::flush(app.world_mut());
  play_for(&mut app, 600);
  drop(race);
  drop(menu);
  audio::flush(app.world_mut());
  app.update();
  assert_eq!(
    app
      .world_mut()
      .query::<&AudioSink>()
      .iter(app.world())
      .count(),
    0,
    "audio survives leaving game"
  );
  println!("Actual native non-muted original menu/builder/race sinks: transition, toggle, mixed engine/SFX, pause/resume and cleanup passed; human-heard output remains unverified.");
}
