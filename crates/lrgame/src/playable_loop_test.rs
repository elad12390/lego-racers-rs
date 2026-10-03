//! Normal launch future with synthetic Bevy InputPlugin messages, NOT physical
//! keyboard/user play, audible output, visual acceptance, or full-game parity.
use crate::{
  editor_gpu_test::{poll, send},
  platform,
};
use bevy::{input::ButtonState, prelude::*};
use serde::Serialize;
use std::{
  cell::RefCell,
  future::Future,
  io::Write,
  pin::Pin,
  task::Poll,
  time::{Duration, Instant},
};

#[derive(Clone, Serialize)]
pub(crate) struct Snapshot {
  pub frame: u32,
  pub table: String,
  pub scripted: bool,
  pub position: [f32; 3],
  pub start: [f32; 3],
  pub speed: f32,
  pub heading: f32,
  pub throttle: f32,
  pub steer: f32,
  pub dt: f32,
  pub paused: bool,
  pub released: bool,
  pub elapsed: f64,
  pub laps: u32,
  pub finished: bool,
  pub collisions: u32,
  pub racer_contacts: u32,
  pub rivals: Vec<([f32; 3], f64, u32)>,
  pub all_rivals_finished: bool,
  pub ranks: Vec<u32>,
}
thread_local! {
  static RACE: RefCell<Option<Snapshot>> = const { RefCell::new(None) };
  static MENU: RefCell<&'static str> = const { RefCell::new("") };
  static TRACE: RefCell<Option<std::fs::File>> = const { RefCell::new(None) };
  static PILOT: RefCell<Option<(lrsim::vehicle::Vehicle, Vec<[f32; 3]>)>> = const { RefCell::new(None) };
  static RESULT: RefCell<Option<serde_json::Value>> = const { RefCell::new(None) };
}
// Copies only: the keyboard pilot never receives mutable game state.
pub(crate) fn observe_vehicle(car: &lrsim::vehicle::Vehicle, line: &[[f32; 3]]) {
  PILOT.with(|p| {
    let mut p = p.borrow_mut();
    if let Some((copy, _)) = p.as_mut() {
      *copy = car.clone();
    } else {
      *p = Some((car.clone(), line.to_vec()));
    }
  });
}
pub(crate) fn observe_result(result: &crate::driving::RaceResult) {
  RESULT.with(|r| *r.borrow_mut() = Some(serde_json::to_value(result).unwrap()));
}
pub(crate) fn observe_race(snapshot: Snapshot) {
  TRACE.with(|trace| {
    if let Some(file) = trace.borrow_mut().as_mut() {
      serde_json::to_writer(&mut *file, &snapshot).unwrap();
      writeln!(file).unwrap();
    }
  });
  RACE.with(|s| *s.borrow_mut() = Some(snapshot));
}
pub(crate) fn observe_menu(screen: &'static str) {
  MENU.with(|s| *s.borrow_mut() = screen);
}
fn snapshot() -> Snapshot {
  RACE.with(|s| {
    s.borrow()
      .clone()
      .expect("normal menu did not launch a race")
  })
}
fn menu() -> &'static str {
  MENU.with(|s| *s.borrow())
}
fn pending(future: Pin<&mut impl Future<Output = Result<(), String>>>) {
  match poll(future) {
    Poll::Pending => {}
    Poll::Ready(result) => panic!("normal application exited unexpectedly: {result:?}"),
  }
}
fn frames(
  app: &mut App,
  mut future: Pin<&mut impl Future<Output = Result<(), String>>>,
  count: usize,
) {
  for _ in 0..count {
    app.update();
    platform::sync(app.world_mut());
    pending(future.as_mut());
  }
}
fn key(
  app: &mut App,
  window: Entity,
  future: Pin<&mut impl Future<Output = Result<(), String>>>,
  code: KeyCode,
  state: ButtonState,
) {
  send(app, window, code, state, None);
  pending(future);
}
fn tap(
  app: &mut App,
  window: Entity,
  mut future: Pin<&mut impl Future<Output = Result<(), String>>>,
  code: KeyCode,
) {
  key(app, window, future.as_mut(), code, ButtonState::Pressed);
  key(app, window, future, code, ButtonState::Released);
}
fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
  lrsim::contact::length(lrsim::contact::sub(a, b))
}

struct AttemptEvidence(std::path::PathBuf);
impl Drop for AttemptEvidence {
  fn drop(&mut self) {
    let latest = RACE.with(|r| r.borrow().clone());
    let result = RESULT.with(|r| r.borrow().clone());
    let _ = std::fs::write(
      self.0.join("terminal.json"),
      serde_json::to_vec_pretty(&serde_json::json!({
        "assertion_panicked": std::thread::panicking(), "last_observed_menu": menu(),
        "last_observed_race": latest, "actual_result": result,
        "input": "automatic_synthetic_keyboard_not_human_play",
      }))
      .unwrap(),
    );
    TRACE.with(|t| {
      t.borrow_mut().take();
    });
  }
}

#[test]
#[ignore = "private original JAM and offscreen Bevy; synthetic keyboard, not physical input"]
fn normal_launch_single_race_keyboard_pause_restart() {
  let began = Instant::now();
  let jam = std::env::var("LR_TEST_JAM").expect("private LR_TEST_JAM");
  let output =
    std::path::PathBuf::from(std::env::var("LR_TEST_OUTPUT").expect("isolated LR_TEST_OUTPUT"));
  std::fs::create_dir(&output).expect("output must be a new isolated directory");
  TRACE.with(|trace| {
    *trace.borrow_mut() = Some(std::fs::File::create(output.join("frames.jsonl")).unwrap())
  });
  let library = lrformats::library::Library::open(&jam).unwrap();
  let catalog = crate::game_catalog::Catalog::load(&library).unwrap();
  let expected_table = catalog.races[catalog.circuit_races(&catalog.circuits[0].name)[0]]
    .table
    .to_ascii_uppercase();
  let save = output.join("profile.json");
  let options = crate::options::Options::parse([
    "--play".to_owned(),
    jam,
    "--profile".into(),
    save.to_string_lossy().into_owned(),
  ])
  .unwrap();
  assert!(!options.scripted_route && !options.scripted_drive && !options.menu_smoke);
  let mut app = platform::offscreen_test_app();
  app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
    Duration::from_secs_f64(1.0 / 60.0),
  ));
  let window = app
    .world_mut()
    .spawn((
      Window {
        focused: true,
        ..default()
      },
      bevy::window::PrimaryWindow,
    ))
    .id();
  app.update();
  platform::sync(app.world_mut());
  platform::rand::srand(0x1_2026_1003);
  let mut future = std::pin::pin!(crate::application::run(options));
  pending(future.as_mut());
  assert_eq!(menu(), "main");
  for _ in 0..2 {
    tap(&mut app, window, future.as_mut(), KeyCode::ArrowDown);
  }
  tap(&mut app, window, future.as_mut(), KeyCode::Enter);
  assert_eq!(menu(), "tracks");
  tap(&mut app, window, future.as_mut(), KeyCode::Enter);
  assert_eq!(menu(), "racer_select");
  tap(&mut app, window, future.as_mut(), KeyCode::Enter);
  let start = snapshot();
  assert_eq!(start.table, expected_table);
  assert!(!start.scripted);
  assert_eq!(start.rivals.len(), 5);
  // Skip the real intro through Enter, then wait for the actual countdown.
  tap(&mut app, window, future.as_mut(), KeyCode::Enter);
  for _ in 0..900 {
    if snapshot().released {
      break;
    }
    frames(&mut app, future.as_mut(), 1);
  }
  let ready = snapshot();
  assert!(ready.released, "intro/countdown never released");
  assert!(
    distance(ready.position, start.position) < 0.01,
    "countdown moved player"
  );
  let mut evidence = vec![("race_ready", ready.clone())];
  key(
    &mut app,
    window,
    future.as_mut(),
    KeyCode::ArrowUp,
    ButtonState::Pressed,
  );
  frames(&mut app, future.as_mut(), 90);
  let accelerated = snapshot();
  evidence.push(("up_acceleration", accelerated.clone()));
  assert_eq!(accelerated.throttle, 1.0);
  assert!(
    accelerated.speed > 1.0 && distance(accelerated.position, ready.position) > 1.0,
    "Up failed to move actual player"
  );
  assert!(
    accelerated
      .rivals
      .iter()
      .zip(&ready.rivals)
      .any(|(a, b)| distance(a.0, b.0) > 1.0),
    "opponents did not advance"
  );
  key(
    &mut app,
    window,
    future.as_mut(),
    KeyCode::ArrowLeft,
    ButtonState::Pressed,
  );
  frames(&mut app, future.as_mut(), 20);
  let left = snapshot();
  evidence.push(("left_steering", left.clone()));
  assert_eq!(left.steer, 1.0);
  assert!(
    (left.heading - accelerated.heading).abs() > 0.001,
    "Left failed to turn actual car"
  );
  key(
    &mut app,
    window,
    future.as_mut(),
    KeyCode::ArrowLeft,
    ButtonState::Released,
  );
  key(
    &mut app,
    window,
    future.as_mut(),
    KeyCode::ArrowRight,
    ButtonState::Pressed,
  );
  frames(&mut app, future.as_mut(), 30);
  let right = snapshot();
  evidence.push(("right_steering", right.clone()));
  assert_eq!(right.steer, -1.0);
  assert!(
    (right.heading - left.heading) * (left.heading - accelerated.heading) < 0.0,
    "Right did not reverse heading response"
  );
  key(
    &mut app,
    window,
    future.as_mut(),
    KeyCode::ArrowRight,
    ButtonState::Released,
  );
  key(
    &mut app,
    window,
    future.as_mut(),
    KeyCode::ArrowUp,
    ButtonState::Released,
  );
  assert_eq!(snapshot().throttle, 0.0, "released Up left throttle active");
  evidence.push(("released_throttle", snapshot()));
  let before_brake = snapshot();
  key(
    &mut app,
    window,
    future.as_mut(),
    KeyCode::ArrowDown,
    ButtonState::Pressed,
  );
  frames(&mut app, future.as_mut(), 20);
  let braked = snapshot();
  evidence.push(("down_brake", braked.clone()));
  assert_eq!(braked.throttle, -1.0);
  assert!(
    braked.speed < before_brake.speed,
    "Down did not reduce speed"
  );
  key(
    &mut app,
    window,
    future.as_mut(),
    KeyCode::ArrowDown,
    ButtonState::Released,
  );
  key(
    &mut app,
    window,
    future.as_mut(),
    KeyCode::ArrowUp,
    ButtonState::Pressed,
  );
  frames(&mut app, future.as_mut(), 10);
  tap(&mut app, window, future.as_mut(), KeyCode::Escape);
  let paused = snapshot();
  assert!(paused.paused);
  frames(&mut app, future.as_mut(), 30);
  let frozen = snapshot();
  evidence.push(("pause_frozen", frozen.clone()));
  assert_eq!(frozen.position, paused.position);
  assert_eq!(frozen.speed, paused.speed);
  assert_eq!(frozen.elapsed, paused.elapsed);
  assert_eq!(frozen.rivals, paused.rivals, "paused opponents advanced");
  tap(&mut app, window, future.as_mut(), KeyCode::Escape);
  frames(&mut app, future.as_mut(), 5);
  let held_resume = snapshot();
  evidence.push(("resume_held_up_blocked", held_resume.clone()));
  assert!(!held_resume.paused);
  assert_eq!(held_resume.throttle, 0.0, "resume accepted stale held Up");
  key(
    &mut app,
    window,
    future.as_mut(),
    KeyCode::ArrowUp,
    ButtonState::Released,
  );
  key(
    &mut app,
    window,
    future.as_mut(),
    KeyCode::ArrowUp,
    ButtonState::Pressed,
  );
  assert_eq!(snapshot().throttle, 1.0);
  tap(&mut app, window, future.as_mut(), KeyCode::KeyR);
  let restarted = snapshot();
  evidence.push(("restart_reset", restarted.clone()));
  assert_eq!(restarted.position, start.position);
  assert_eq!(restarted.speed, 0.0);
  assert_eq!(restarted.heading, start.heading);
  assert_eq!(restarted.elapsed, 0.0);
  assert_eq!(restarted.laps, 0);
  assert_eq!(restarted.collisions, 0);
  assert!(!restarted.finished && !restarted.paused && !restarted.released);
  assert_eq!(restarted.rivals, start.rivals);
  assert_eq!(restarted.throttle, 0.0, "restart accepted stale Up");
  key(
    &mut app,
    window,
    future.as_mut(),
    KeyCode::ArrowUp,
    ButtonState::Released,
  );
  tap(&mut app, window, future.as_mut(), KeyCode::Enter);
  for _ in 0..900 {
    if snapshot().released {
      break;
    }
    frames(&mut app, future.as_mut(), 1);
  }
  assert!(snapshot().released);
  key(
    &mut app,
    window,
    future.as_mut(),
    KeyCode::ArrowUp,
    ButtonState::Pressed,
  );
  frames(&mut app, future.as_mut(), 90);
  let second = snapshot();
  evidence.push(("second_acceleration", second.clone()));
  assert!(
    second.speed > 1.0 && distance(second.position, start.position) > 1.0,
    "second acceleration failed after restart"
  );
  key(
    &mut app,
    window,
    future.as_mut(),
    KeyCode::ArrowUp,
    ButtonState::Released,
  );
  // Leave through the real pause menu, not quit-state mutation.
  tap(&mut app, window, future.as_mut(), KeyCode::Escape);
  for _ in 0..2 {
    tap(&mut app, window, future.as_mut(), KeyCode::ArrowDown);
  }
  tap(&mut app, window, future.as_mut(), KeyCode::Enter);
  frames(&mut app, future.as_mut(), 2);
  assert_eq!(menu(), "main");
  send(
    &mut app,
    window,
    KeyCode::Escape,
    ButtonState::Pressed,
    None,
  );
  assert!(matches!(poll(future.as_mut()), Poll::Ready(Ok(()))));
  std::fs::write(output.join("telemetry.json"), serde_json::to_vec_pretty(&serde_json::json!({
    "mode": "synthetic_Bevy_KeyboardInput_normal_application_future_not_physical_keyboard_or_visual_acceptance",
    "finish_results_relaunch": "not_covered_parent_scripted_menu_smoke_is_separate_autopilot_evidence",
    "runtime_seconds": began.elapsed().as_secs_f64(),
    "snapshots": evidence,
  })).unwrap()).unwrap();
  println!(
    "synthetic normal-launch race input/pause/restart telemetry: {} ({:.2}s)",
    output.display(),
    began.elapsed().as_secs_f64()
  );
}

/// Normal application lifecycle, driven exclusively by automatic/synthetic
/// keyboard messages. PWM approximates the recorded-line driver's analog
/// requests; no Actions, lap counters, poses, or application states are injected.
#[test]
#[ignore = "private original JAM; synthetic keyboard full three-lap Results/relaunch loop"]
fn normal_results_race_again_keyboard_loop() {
  let began = Instant::now();
  let jam = std::env::var("LR_TEST_JAM").expect("private LR_TEST_JAM");
  let output =
    std::path::PathBuf::from(std::env::var("LR_TEST_OUTPUT").expect("isolated LR_TEST_OUTPUT"));
  std::fs::create_dir(&output).expect("output must be a new isolated directory");
  let _terminal_evidence = AttemptEvidence(output.clone());
  TRACE
    .with(|t| *t.borrow_mut() = Some(std::fs::File::create(output.join("frames.jsonl")).unwrap()));
  PILOT.with(|p| *p.borrow_mut() = None);
  RESULT.with(|r| *r.borrow_mut() = None);
  let library = lrformats::library::Library::open(&jam).unwrap();
  let catalog = crate::game_catalog::Catalog::load(&library).unwrap();
  // Prefer Royal Knights only if available in an untouched fresh profile.
  // Otherwise use the real unlocked catalog default; never inject an unlock.
  let (circuit, slot, race_index) = catalog
    .circuits
    .iter()
    .enumerate()
    .filter(|(c, _)| *c as u32 <= crate::profile::Profile::default().unlocked_circuit)
    .find_map(|(c, circuit)| {
      catalog
        .circuit_races(&circuit.name)
        .iter()
        .enumerate()
        .find_map(|(s, &r)| {
          (catalog.races[r].table.eq_ignore_ascii_case("RACEC0R0") && !catalog.races[r].mirrored)
            .then_some((c, s, r))
        })
    })
    .unwrap_or((0, 0, catalog.circuit_races(&catalog.circuits[0].name)[0]));
  println!(
    "fresh-profile catalog selection: {} / {} (circuit {circuit}, slot {slot})",
    catalog.title(&catalog.races[race_index]),
    catalog.races[race_index].table
  );
  let save = output.join("profile.json");
  let options = crate::options::Options::parse([
    "--play".into(),
    jam,
    "--profile".into(),
    save.to_string_lossy().into_owned(),
  ])
  .unwrap();
  assert!(!options.scripted_route && !options.scripted_drive && !options.menu_smoke);
  let mut app = platform::offscreen_test_app();
  app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
    Duration::from_secs_f64(1.0 / 60.0),
  ));
  let window = app
    .world_mut()
    .spawn((
      Window {
        focused: true,
        ..default()
      },
      bevy::window::PrimaryWindow,
    ))
    .id();
  app.update();
  platform::sync(app.world_mut());
  platform::rand::srand(0x2_2026_1003);
  let mut future = std::pin::pin!(crate::application::run(options));
  pending(future.as_mut());
  assert_eq!(menu(), "main");
  for _ in 0..2 {
    tap(&mut app, window, future.as_mut(), KeyCode::ArrowDown);
  }
  tap(&mut app, window, future.as_mut(), KeyCode::Enter);
  assert_eq!(menu(), "tracks");
  for _ in 0..circuit {
    tap(&mut app, window, future.as_mut(), KeyCode::ArrowDown);
  }
  for _ in 0..slot {
    tap(&mut app, window, future.as_mut(), KeyCode::ArrowRight);
  }
  tap(&mut app, window, future.as_mut(), KeyCode::Enter);
  assert_eq!(menu(), "racer_select");
  tap(&mut app, window, future.as_mut(), KeyCode::Enter);
  let start = snapshot();
  assert_eq!(
    start.table,
    catalog.races[race_index].table.to_ascii_uppercase()
  );
  assert!(!start.scripted);
  assert_eq!(start.rivals.len(), 5);
  let mut evidence = vec![("first_race_initialized", start.clone())];
  // Wait through the actual intro AND countdown: no Enter skip for this test.
  for _ in 0..2400 {
    if snapshot().released {
      break;
    }
    frames(&mut app, future.as_mut(), 1);
  }
  assert!(snapshot().released, "real intro/countdown never released");
  assert!(distance(snapshot().position, start.position) < 0.01);
  evidence.push(("first_countdown_released", snapshot()));
  let mut driver = PILOT.with(|p| {
    lrsim::diagnostic_driver::DiagnosticDriver::new(p.borrow().as_ref().unwrap().1.clone())
  });
  let keys = [
    KeyCode::ArrowUp,
    KeyCode::ArrowDown,
    KeyCode::ArrowLeft,
    KeyCode::ArrowRight,
  ];
  let mut held = [false; 4];
  let mut duty = [0.0_f32; 4];
  let mut keyboard_trace = std::fs::File::create(output.join("keyboard.jsonl")).unwrap();
  let mut previous_lap = 0;
  for tick in 0..36000 {
    let observed = snapshot();
    if observed.finished && observed.all_rivals_finished {
      break;
    }
    if began.elapsed() > Duration::from_secs(480) {
      break;
    }
    let requested = PILOT.with(|p| driver.actions(&p.borrow().as_ref().unwrap().0));
    let requests = [
      requested.throttle.max(0.0),
      (-requested.throttle).max(0.0),
      requested.steer.max(0.0),
      (-requested.steer).max(0.0),
    ];
    let mut down = [false; 4];
    for i in 0..4 {
      duty[i] += requests[i];
      down[i] = duty[i] >= 1.0;
      if down[i] {
        duty[i] -= 1.0;
      }
      if down[i] != held[i] {
        app
          .world_mut()
          .write_message(bevy::input::keyboard::KeyboardInput {
            key_code: keys[i],
            state: if down[i] {
              ButtonState::Pressed
            } else {
              ButtonState::Released
            },
            text: None,
            repeat: false,
            window,
            logical_key: bevy::input::keyboard::Key::Unidentified(
              bevy::input::keyboard::NativeKey::Unidentified,
            ),
          });
      }
    }
    held = down;
    serde_json::to_writer(
      &mut keyboard_trace,
      &serde_json::json!({
        "tick": tick, "observed_frame": observed.frame, "line_index": driver.index,
        "requested_analog": [requested.throttle, requested.steer],
        "synthetic_arrow_keys_down_up_down_left_right": held,
      }),
    )
    .unwrap();
    writeln!(keyboard_trace).unwrap();
    frames(&mut app, future.as_mut(), 1);
    let actual = snapshot();
    if actual.laps != previous_lap {
      println!(
        "synthetic keyboard: lap {} at {:.2}s, line {}",
        actual.laps, actual.elapsed, driver.index
      );
      evidence.push(("actual_lap_transition", actual.clone()));
      previous_lap = actual.laps;
    }
  }
  // Persist the terminal observation before any assertion, including an
  // incomplete digital-pilot attempt. Incompleteness is NOT a game diagnosis.
  let terminal = snapshot();
  evidence.push(("first_race_terminal_observation", terminal.clone()));
  std::fs::write(
    output.join("attempt.json"),
    serde_json::to_vec_pretty(&serde_json::json!({
      "mode": "automatic_synthetic_Bevy_KeyboardInput_recorded_line_PWM_not_human_input",
      "scripted_route": false, "scripted_drive": false,
      "runtime_seconds": began.elapsed().as_secs_f64(), "line_index": driver.index,
      "line_samples": driver.points.len(), "snapshots": evidence,
      "incomplete_means": "digital_pilot_harness_limitation_not_game_broken",
    }))
    .unwrap(),
  )
  .unwrap();
  assert!(terminal.finished && terminal.all_rivals_finished && terminal.laps == 3,
    "digital keyboard pilot did not finish within budget; preserved telemetry is a harness limitation, not proof of a game blocker: {}", output.display());
  for (i, code) in keys.into_iter().enumerate() {
    if held[i] {
      key(
        &mut app,
        window,
        future.as_mut(),
        code,
        ButtonState::Released,
      );
    }
  }
  tap(&mut app, window, future.as_mut(), KeyCode::Enter);
  frames(&mut app, future.as_mut(), 3);
  assert_eq!(menu(), "results");
  let result = RESULT.with(|r| r.borrow().clone().expect("actual Results observation"));
  assert_eq!(result["laps"], 3);
  assert_eq!(result["names"].as_array().unwrap().len(), 6);
  assert_eq!(result["time"].as_f64().unwrap(), terminal.elapsed);
  let restored = crate::profile::Profile::load(&save).unwrap();
  assert_eq!(
    restored.best_times[&catalog.races[race_index].name],
    terminal.elapsed
  );
  std::fs::write(
    output.join("results.json"),
    serde_json::to_vec_pretty(&serde_json::json!({
      "actual_result": result, "restored_best_times": restored.best_times,
    }))
    .unwrap(),
  )
  .unwrap();
  // Default Results choice is the real Race Again button, not a direct launch.
  tap(&mut app, window, future.as_mut(), KeyCode::Enter);
  let second_start = snapshot();
  evidence.push(("race_again_initialized", second_start.clone()));
  assert_eq!(second_start.table, start.table);
  assert_eq!(second_start.position, start.position);
  assert_eq!(second_start.heading, start.heading);
  assert_eq!(second_start.speed, 0.0);
  assert_eq!(second_start.elapsed, 0.0);
  assert_eq!(second_start.laps, 0);
  assert_eq!(second_start.collisions, 0);
  assert_eq!(second_start.racer_contacts, 0);
  assert_eq!(second_start.rivals, start.rivals);
  assert!(
    !second_start.finished
      && !second_start.paused
      && !second_start.released
      && !second_start.scripted
  );
  for _ in 0..2400 {
    if snapshot().released {
      break;
    }
    frames(&mut app, future.as_mut(), 1);
  }
  let ready = snapshot();
  assert!(ready.released);
  key(
    &mut app,
    window,
    future.as_mut(),
    KeyCode::ArrowUp,
    ButtonState::Pressed,
  );
  frames(&mut app, future.as_mut(), 90);
  let accelerated = snapshot();
  evidence.push(("second_race_up_acceleration", accelerated.clone()));
  assert_eq!(accelerated.throttle, 1.0);
  assert!(accelerated.speed > 1.0 && distance(accelerated.position, ready.position) > 1.0);
  key(
    &mut app,
    window,
    future.as_mut(),
    KeyCode::ArrowUp,
    ButtonState::Released,
  );
  tap(&mut app, window, future.as_mut(), KeyCode::Escape);
  assert!(snapshot().paused);
  evidence.push(("second_race_pause", snapshot()));
  for _ in 0..2 {
    tap(&mut app, window, future.as_mut(), KeyCode::ArrowDown);
  }
  tap(&mut app, window, future.as_mut(), KeyCode::Enter);
  frames(&mut app, future.as_mut(), 3);
  assert_eq!(menu(), "main");
  send(
    &mut app,
    window,
    KeyCode::Escape,
    ButtonState::Pressed,
    None,
  );
  assert!(matches!(poll(future.as_mut()), Poll::Ready(Ok(()))));
  std::fs::write(output.join("telemetry.json"), serde_json::to_vec_pretty(&serde_json::json!({
    "status": "passed", "mode": "automatic_synthetic_keyboard_not_human_play_or_visual_acceptance",
    "selected_track": catalog.title(&catalog.races[race_index]),
    "loop": ["Main Menu", "unlocked catalog track", "actual three laps and five rivals finished",
      "Enter Results", "Enter Race Again", "fresh second race", "Up acceleration", "pause Exit Race", "Main Menu"],
    "runtime_seconds": began.elapsed().as_secs_f64(), "snapshots": evidence,
    "actual_result": result, "restored_best_times": restored.best_times,
    "remaining_acceptance": "human perceived GUI/input feel; entire game remains unfinished",
  })).unwrap()).unwrap();
  println!(
    "Results/relaunch synthetic-keyboard telemetry: {}",
    output.display()
  );
}
