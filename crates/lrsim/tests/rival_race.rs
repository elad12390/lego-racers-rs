use lrformats::library::Library;
use lrsim::{race_data::RaceData, rival_data, rivals::Rival};
use std::path::PathBuf;

#[test]
fn real_original_rosters_routes_independently_complete_and_restart_on_all_tracks() {
  let library = Library::open(
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../extracted/Program_Files_Group/LEGO.JAM"),
  )
  .unwrap();
  for table in library.tables().filter(|t| t.starts_with("RACEC")) {
    let data = RaceData::load(&library, table, "bkchas0").unwrap();
    let definitions = rival_data::load(&library, table, [0, 1, 2, 3, 4]).unwrap();
    assert_eq!(definitions.len(), 5);
    for definition in &definitions {
      let mut rival = Rival::new(definition).unwrap();
      let start = rival.motion.position;
      rival.advance(0.0, &data.lap_zones);
      assert_eq!(rival.motion.position, start);
      assert_eq!(rival.race.elapsed, 0.0);
      for _ in 0..36000 {
        rival.advance(1.0 / 60.0, &data.lap_zones);
        if rival.race.finished {
          break;
        }
      }
      assert!(
        rival.race.finished,
        "{table}/{}never completed original spatial laps; events{:?}",
        definition.route_name,
        rival
          .race
          .events
          .iter()
          .map(|e| e.event.id)
          .collect::<Vec<_>>()
      );
      assert_eq!(rival.race.laps.completed_laps, 3);
      assert_eq!(rival.race.lap_times.len(), 3);
      assert!(rival.race.events.iter().any(|e| e.event.id == 101));
      let finished = rival.motion.position;
      let clock = rival.race.elapsed;
      rival.advance(1.0, &data.lap_zones);
      assert_ne!(rival.motion.position, finished);
      assert_eq!(rival.race.elapsed, clock);
      let reset = Rival::new(definition).unwrap();
      assert_eq!(reset.motion.position, start);
      assert_eq!(reset.race.laps.completed_laps, 0);
      assert!(reset.race.events.is_empty());
    }
  }
  assert!(rival_data::load(&library, "MISSING", [0; 5]).is_err());
}

#[test]
fn overlapping_original_grid_cars_separate_and_transfer_speed_without_teleporting_to_route_samples()
{
  let library = Library::open(
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../extracted/Program_Files_Group/LEGO.JAM"),
  )
  .unwrap();
  let data = RaceData::load(&library, "RACEC0R0", "bkchas0").unwrap();
  let definitions = rival_data::load(&library, "RACEC0R0", [0, 1, 2, 3, 4]).unwrap();
  let mut rivals = vec![Rival::new(&definitions[0]).unwrap()];
  let mut player = lrsim::vehicle::Vehicle::spawn(&data.start, &data.chassis, &data.contacts);
  // Exercise genuine car contact at a shipped grid position, not fake bodies.
  player.position = rivals[0].motion.position;
  player.set_velocity(player.forward().map(|v| v * 100.0));
  let initial = player.position;
  let speed = rivals[0].motion.cursor.speed;
  let contacts =
    lrsim::race_contacts::resolve(&mut player, &data.chassis, &data.contacts, &mut rivals);
  assert!(contacts > 0);
  assert_ne!(player.position, initial);
  assert_ne!(rivals[0].motion.cursor.speed, speed);
  assert!(lrsim::contact::length(lrsim::contact::sub(player.position, initial)) < 10.0);
  assert!(player
    .position
    .iter()
    .chain(&player.velocity())
    .all(|x| x.is_finite()));
  assert_eq!(player.unresolved_contacts, 0);
}
