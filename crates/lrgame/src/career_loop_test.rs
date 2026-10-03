//! Isolated offscreen normal application future. Synthetic menu keyboard and
//! existing diagnostic route pilot: NOT human driving or audible acceptance.
use crate::{
  editor_gpu_test::{capture_editor_frame, poll, send},
  platform,
  profile::Profile,
};
use bevy::{input::ButtonState, prelude::*};
use std::{cell::RefCell, future::Future, pin::Pin, task::Poll, time::Duration};

thread_local! {
  static STATE: RefCell<Option<(&'static str, Profile, Option<(usize, [u32; 6])>)>> = const { RefCell::new(None) };
}
pub(super) fn observe(screen: &'static str, profile: &Profile, run: Option<(usize, [u32; 6])>) {
  STATE.with(|s| *s.borrow_mut() = Some((screen, profile.clone(), run)));
}
fn state() -> (&'static str, Profile, Option<(usize, [u32; 6])>) {
  STATE.with(|s| s.borrow().clone().unwrap())
}
fn advance(app: &mut App, mut future: Pin<&mut impl Future<Output = Result<(), String>>>) {
  app.update();
  platform::sync(app.world_mut());
  assert!(matches!(poll(future.as_mut()), Poll::Pending));
}
fn tap(
  app: &mut App,
  window: Entity,
  mut future: Pin<&mut impl Future<Output = Result<(), String>>>,
  key: KeyCode,
) {
  for button in [ButtonState::Pressed, ButtonState::Released] {
    send(app, window, key, button, None);
    assert!(matches!(poll(future.as_mut()), Poll::Pending));
  }
}
fn enter_career(
  app: &mut App,
  window: Entity,
  mut future: Pin<&mut impl Future<Output = Result<(), String>>>,
) {
  assert_eq!(state().0, "main");
  tap(app, window, future.as_mut(), KeyCode::ArrowDown);
  tap(app, window, future.as_mut(), KeyCode::Enter);
  assert_eq!(state().0, "circuits");
}

#[test]
#[ignore = "private JAM, offscreen Bevy; synthetic menu keyboard + diagnostic route pilot"]
fn career_four_races_save_resume_standings_award() {
  let jam = std::env::var("LR_TEST_JAM").expect("private LR_TEST_JAM");
  let output =
    std::path::PathBuf::from(std::env::var("LR_TEST_OUTPUT").expect("isolated LR_TEST_OUTPUT"));
  std::fs::create_dir(&output).expect("use a new isolated output directory");
  let library = lrformats::library::Library::open(&jam).unwrap();
  let catalog = crate::game_catalog::Catalog::load(&library).unwrap();
  let names = super::racer_labels(&library).unwrap();
  assert_eq!(names.get("AD").map(String::as_str), Some("ANN DROID"));
  assert_eq!(
    names.get("GB").map(String::as_str),
    Some("GOVERNOR BROADSIDE")
  );
  let races = catalog.circuit_races(&catalog.circuits[0].name);
  assert_eq!(races.len(), 4, "first original CRB circuit is four races");
  assert_eq!(catalog.rules.finish_points, [30, 20, 10, 3, 2, 1]);
  let save = output.join("profile.json");
  let mut options = crate::options::Options::parse([
    "--play".into(),
    jam,
    "--profile".into(),
    save.to_string_lossy().into_owned(),
  ])
  .unwrap();
  // Real races and placements; only driving uses the existing diagnostic pilot.
  options.scripted_route = true;
  options.frames = Some(36000);
  let mut quiet = Profile::default();
  quiet.music = false;
  quiet.sound = false;
  quiet.save(&save).unwrap();
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
  platform::rand::srand(0x2026_1003);
  let mut first = Box::pin(crate::application::run(options.clone()));
  assert!(poll(first.as_mut()).is_pending());
  enter_career(&mut app, window, first.as_mut());
  crate::capture::save_frame(
    &output.join("career-selector.png"),
    &capture_editor_frame(&mut app),
  )
  .unwrap();
  tap(&mut app, window, first.as_mut(), KeyCode::Enter);
  assert_eq!(state().0, "racer_select");
  tap(&mut app, window, first.as_mut(), KeyCode::Enter);
  for _ in 0..36000 {
    if state().0 == "results" {
      break;
    }
    advance(&mut app, first.as_mut());
  }
  assert_eq!(state().0, "results", "first race failed to reach standings");
  let first_scores = state().2.unwrap().1;
  assert_eq!(first_scores.iter().sum::<u32>(), 66);
  assert_eq!(Profile::load(&save).unwrap().career.unwrap().completed, 1);
  crate::capture::save_frame(
    &output.join("career-standings.png"),
    &capture_editor_frame(&mut app),
  )
  .unwrap();
  // Leave via the normal results return, then destroy/recreate the application
  // future so resume genuinely reads the disk save, not an in-memory shortcut.
  tap(&mut app, window, first.as_mut(), KeyCode::Escape);
  assert_eq!(state().0, "main");
  send(
    &mut app,
    window,
    KeyCode::Escape,
    ButtonState::Pressed,
    None,
  );
  assert!(matches!(poll(first.as_mut()), Poll::Ready(Ok(()))));
  drop(first);
  send(
    &mut app,
    window,
    KeyCode::Escape,
    ButtonState::Released,
    None,
  );
  let mut future = Box::pin(crate::application::run(options));
  assert!(poll(future.as_mut()).is_pending());
  enter_career(&mut app, window, future.as_mut());
  tap(&mut app, window, future.as_mut(), KeyCode::Enter);
  assert_eq!(state().2, Some((1, first_scores)));
  tap(&mut app, window, future.as_mut(), KeyCode::Enter);
  for race in 1..4 {
    let expected_points = 66 * (race as u32 + 1);
    for _ in 0..36000 {
      if state().0 == "results" && state().2.unwrap().1.iter().sum::<u32>() == expected_points {
        break;
      }
      advance(&mut app, future.as_mut());
    }
    assert_eq!(state().0, "results");
    let scores = state().2.unwrap().1;
    assert_eq!(
      scores.iter().sum::<u32>(),
      expected_points,
      "missing/duplicate circuit awards"
    );
    if race < 3 {
      assert_eq!(
        Profile::load(&save).unwrap().career.unwrap().completed,
        race + 1
      );
      tap(&mut app, window, future.as_mut(), KeyCode::Enter);
    }
  }
  let scores = state().2.unwrap().1;
  let rank = 1 + scores[1..].iter().filter(|v| **v > scores[0]).count() as u32;
  assert!(
    rank <= 2,
    "diagnostic pilot did not earn next-circuit unlock: place {rank}"
  );
  let restored = Profile::load(&save).unwrap();
  assert_eq!(
    restored.circuit_medals.get(&catalog.circuits[0].name),
    Some(&rank)
  );
  assert!(restored.career.is_none());
  assert_eq!(restored.unlocked_circuit, if rank <= 2 { 1 } else { 0 });
  assert_eq!(restored.driver_part_allowed(3, &catalog), rank == 1);
  crate::capture::save_frame(
    &output.join("career-complete.png"),
    &capture_editor_frame(&mut app),
  )
  .unwrap();
  tap(&mut app, window, future.as_mut(), KeyCode::Enter);
  // Skip/continue actual award and any first-gold reward through ordinary keys.
  for _ in 0..12 {
    if state().0 == "circuits" {
      break;
    }
    tap(&mut app, window, future.as_mut(), KeyCode::Enter);
    advance(&mut app, future.as_mut());
  }
  assert_eq!(
    state().0,
    "circuits",
    "award did not return to circuit selection"
  );
  std::fs::write(output.join("career-evidence.json"), serde_json::to_vec_pretty(&serde_json::json!({
    "input": "synthetic_menu_keyboard_and_existing_route_pilot_not_human_driving_or_audible_acceptance",
    "races": races.iter().map(|i| &catalog.races[*i].name).collect::<Vec<_>>(),
    "scores": scores, "rank": rank, "unlocked_circuit": restored.unlocked_circuit,
    "disk_resume_after_race_1": true, "final_award_return": true,
  })).unwrap()).unwrap();
}
