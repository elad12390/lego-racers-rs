use lrsim::ghost_run::{Pose, Recorder, Run};
#[test]
fn original_ghost_moves_without_becoming_a_race_collider_and_native_recording_reopens() {
  let library = lrformats::library::Library::open(
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
      .join("../../extracted/Program_Files_Group/LEGO.JAM"),
  )
  .unwrap();
  let source =
    lrformats::ghost::Ghost::load(library.find_in("GHOST.GHB", "RACEC0R0").unwrap(), false)
      .unwrap();
  assert_eq!(source.lap_ms, [32266, 30015, 33066]);
  assert_eq!(source.samples.len(), 382);
  let run = Run::original(source);
  run.validate().unwrap();
  let a = run.sample(0.0).unwrap();
  let b = run.sample(0.25).unwrap();
  let mid = run.sample(0.125).unwrap();
  assert_ne!(a.position, b.position);
  for i in 0..3 {
    assert!((mid.position[i] - (a.position[i] + b.position[i]) * 0.5).abs() < 0.0001);
  }
  assert!(run.sample(240.0).is_none());
  let mut recorder = Recorder::new(a);
  for frame in 1..60 * 3 {
    let time = frame as f64 / 60.0;
    recorder.observe(time, run.sample(time).unwrap());
  }
  let saved = recorder.finish(vec![1.0; 3]).unwrap();
  let reopened: Run = serde_json::from_slice(&serde_json::to_vec(&saved).unwrap()).unwrap();
  reopened.validate().unwrap();
  assert_eq!(
    reopened.sample(1.0).unwrap().position,
    run.sample(1.0).unwrap().position
  );
  let mut bad = reopened;
  bad.samples[0].rotation = [0.0; 4];
  assert!(bad.validate().is_err());
  let _pose: Pose = mid;
}

#[test]
fn native_slow_race_keeps_recording_and_replays_past_four_minutes() {
  let library = lrformats::library::Library::open(
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
      .join("../../extracted/Program_Files_Group/LEGO.JAM"),
  )
  .unwrap();
  let original = Run::original(
    lrformats::ghost::Ghost::load(library.find_in("GHOST.GHB", "RACEC0R0").unwrap(), false)
      .unwrap(),
  );
  let mut recorder = Recorder::new(original.sample(0.0).unwrap());
  // Feed the actual original trajectory at a slow practice-race cadence.
  // No GPU race/contact/other-mode replay is necessary for recorder storage.
  for frame in 1..=1200 {
    let time = frame as f64 * 0.25;
    recorder.observe(time, original.sample(time / 4.0).unwrap());
  }
  let run = recorder.finish(vec![100.0; 3]).unwrap();
  assert_eq!(run.samples.len(), 1202);
  let restored: Run = serde_json::from_slice(&serde_json::to_vec(&run).unwrap()).unwrap();
  restored.validate().unwrap();
  for time in [239.75, 240.0, 280.0, 300.0] {
    assert_eq!(
      restored.sample(time).unwrap().position,
      original.sample(time / 4.0).unwrap().position
    );
  }
  assert!(restored.sample(300.25).is_none());
  assert!(restored.sample(f64::MAX).is_none());
}
