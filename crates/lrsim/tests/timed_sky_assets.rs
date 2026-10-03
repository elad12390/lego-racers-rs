//! Actual original TIB/EVB/SKB files exercise clock-driven sky selection.
use lrformats::{environment_events, library::Library, sky, timed_events};
use lrsim::sky_events::Player;
fn library() -> Library {
  Library::open(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("../../extracted/Program_Files_Group/LEGO.JAM")).unwrap()
}
#[test]
fn magma_moon_flash_and_return_are_authored_timer_actions_not_zone_palette_guesses() {
  let library = library(); let table = "RACEC0R3";
  let read = |name| library.find_in(name, table).unwrap();
  let sky = sky::parse(read("BACKGRND.SKB")).unwrap();
  let actions = environment_events::parse(read("EVENT.EVB")).unwrap();
  assert_eq!(actions.len(), 2);
  assert_eq!((actions[0].id, actions[0].stop, actions[0].name.as_str(), actions[0].transition_ms), (50, false, "flash", 250));
  assert_eq!((actions[1].id, actions[1].stop, actions[1].name.as_str(), actions[1].transition_ms), (50, true, "openair", 500));
  let timers = timed_events::parse(read("TIMER.TIB")).unwrap();
  let mut player = Player::new(sky, actions, timers, || 1023).unwrap();
  let ambient = player.sky.colors();
  assert!(player.advance(7160, || 1023).unwrap().is_empty());
  assert_eq!(player.sky.name(), "openair");
  assert_eq!(player.advance(1, || 1023).unwrap().len(), 1);
  assert_eq!(player.sky.name(), "flash");
  assert_eq!(player.sky.colors(), ambient); // callback does not steal prior tick
  player.advance(250, || 1023).unwrap();
  assert_ne!(player.sky.colors(), ambient);
  player.advance(773, || 1023).unwrap();
  assert_eq!(player.sky.name(), "openair");
  player.advance(501, || 1023).unwrap();
  assert_eq!(player.sky.colors(), ambient);
  player.reset(|| 1023);
  assert_eq!(player.sky.name(), "openair");
  assert_eq!(player.named_dispatches, 0);
}
#[test]
fn every_playable_world_timer_and_sky_table_is_validated_without_inventing_royal_castle_dispatch() {
  let library = library();
  let mut worlds = 0;
  let mut action_count = 0;
  for table in library.jam().tables.iter().filter(|t| t.group == "GAMEDATA" && t.name.starts_with("RACEC")) {
    let read = |name| library.find_in(name, &table.name).unwrap();
    let actions = environment_events::parse(read("EVENT.EVB")).unwrap();
    if table.name == "RACEC0R0" { assert!(actions.is_empty()); }
    action_count += actions.len();
    let timers = timed_events::parse(read("TIMER.TIB")).unwrap();
    let sky = sky::parse(read("BACKGRND.SKB")).unwrap();
    let mut player = Player::new(sky, actions, timers, || 512).unwrap();
    for _ in 0..2000 { player.advance(16, || 512).unwrap(); }
    worlds += 1;
  }
  assert_eq!(worlds, 13); assert_eq!(action_count, 4);
}

#[test]
fn alien_rally_empty_name_stop_hides_sky_and_start_restores_without_palette_change() {
  let library = library(); let table = "RACEC2R2";
  let read = |name| library.find_in(name, table).unwrap();
  let sky = sky::parse(read("BACKGRND.SKB")).unwrap();
  let actions = environment_events::parse(read("EVENT.EVB")).unwrap();
  let mut player = Player::new(sky, actions, vec![], || 0).unwrap();
  let colors = player.sky.colors();
  player.dispatch(lrsim::timed_events::Event {id: 10, stop: true}).unwrap();
  assert!(!player.shell_visible());
  assert!(!player.children_visible());
  assert_eq!(player.sky.colors(), colors);
  player.advance(1000, || 0).unwrap();
  assert!(!player.shell_visible());
  player.dispatch(lrsim::timed_events::Event {id: 10, stop: false}).unwrap();
  assert!(player.shell_visible());
  assert!(player.children_visible());
  assert_eq!(player.sky.colors(), colors);
  assert_eq!(player.flag_dispatches, 2);
  player.reset(|| 0);
  assert!(player.shell_visible());
  assert_eq!(player.flag_dispatches, 0);
}
