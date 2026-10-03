//! Integrated CPU acceptance with original assets and automated Actions input.
//! No physical keyboard, Bevy InputPlugin, GPU, audio or human play is claimed.
use lrformats::library::Library;
use lrsim::{
  diagnostic_driver::DiagnosticDriver, race::Race, race_data::RaceData,
  race_positions::RacePositions, rival_data, rivals::Rival, start_gate::StartGate,
  vehicle::Vehicle, world_dispatch::ChassisWorld,
};

#[test]
fn royal_knights_contested_three_lap_finish_rebuild_and_repeat() {
  let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
  let library = Library::open(root.join("extracted/Program_Files_Group/LEGO.JAM")).unwrap();
  let data = RaceData::load(&library, "RACEC0R0", "bkchas0").unwrap();
  let definitions = rival_data::load(&library, "RACEC0R0", [0, 1, 2, 3, 4]).unwrap();
  assert_eq!(definitions.len(), 5);
  let world = ChassisWorld::new(
    data.contacts.primary_collider().unwrap().clone(),
    &data.checkpoints,
  );
  for attempt in 1..=2 {
    // The native restart contract: rebuild bodies, route cursors, hit states,
    // lap clocks, countdown, checkpoint/finish standings and diagnostic input.
    let mut car = Vehicle::spawn(&data.start, &data.chassis, &data.contacts);
    let mut rivals: Vec<_> = definitions.iter().map(|d| Rival::new(d).unwrap()).collect();
    let mut race = Race::new(3).unwrap();
    let mut driver = DiagnosticDriver::new(data.diagnostic_line.clone());
    let mut gate = StartGate::default();
    let mut positions = RacePositions::new(&lrsim::race_positions::poses(&car, &rivals));
    assert_eq!(car.position, data.start.position);
    assert_eq!(car.velocity(), [0.0; 3]);
    assert_eq!(car.collisions, 0);
    assert_eq!(driver.index, 0);
    assert_eq!(driver.distance_on_line, 0.0);
    assert_eq!(gate.numeral(), Some(3));
    assert_eq!(race.elapsed, 0.0);
    assert!(race.events.is_empty() && race.lap_times.is_empty() && !race.finished);
    for rival in &rivals {
      assert_eq!(rival.motion.position, rival.data.record.start_position);
      assert_eq!(rival.motion.velocity, [0.0; 3]);
      assert_eq!(rival.world_collisions, 0);
      assert_eq!(rival.hit.flags, 0);
      assert_eq!(rival.hit.remaining_ms, 0);
      assert_eq!(rival.hit.collision_scale, 1.0);
      assert_eq!(rival.race.elapsed, 0.0);
      assert_eq!(rival.race.laps.completed_laps, 0);
      assert!(rival.race.events.is_empty() && !rival.race.finished);
    }
    assert_eq!(positions.report().finite_probe_contacts, 0);
    assert!(positions
      .report()
      .states
      .iter()
      .all(|s| s.checkpoint.is_none() && s.flags == 2 && s.contact_count == -1));
    let mut racer_contacts = 0;
    let mut contact_responses = 0;
    let mut distance = 0.0;
    let mut max_speed = 0.0f32;
    let mut min_steer = 1.0f32;
    let mut max_steer = -1.0f32;
    let mut countdown_numerals = Vec::new();
    // Match driving.rs's scripted-route interval rather than its solo test's
    // smaller timestep; the interactive frame interval is independently owned.
    const HZ: usize = 60;
    for tick in 0..183 * HZ {
      if let Some(numeral) = gate.numeral() {
        if countdown_numerals.last() != Some(&numeral) {
          countdown_numerals.push(numeral);
        }
      }
      let dt = gate.advance(1.0 / HZ as f64) as f32;
      let actions = driver.actions(&car);
      let old = car.position;
      // Same ordinary power_vehicle/owner-dispatch path as live driving.rs;
      // no active power effect is installed for this ordinary race acceptance.
      let hits = lrsim::power_vehicle::advance(
        &mut car,
        actions,
        None,
        &[],
        &data.contacts,
        &data.checkpoints,
        &world,
        positions.state_mut(0),
        dt,
      )
      .unwrap();
      positions.add_contacts(hits);
      race.advance(old, car.position, &data.lap_zones, f64::from(dt));
      for (index, rival) in rivals.iter_mut().enumerate() {
        let hits = rival.advance_dispatched(
          f64::from(dt),
          &data.lap_zones,
          &data.contacts,
          &world,
          positions.state_mut(index + 1),
        );
        positions.add_contacts(hits);
      }
      if dt > 0.0 {
        let before_contact = (car.position, car.velocity());
        let (pairs, hits) = lrsim::race_contacts::resolve_dispatched(
          &mut car,
          &data.chassis,
          &data.contacts,
          &mut rivals,
          &world,
          positions.state_mut(0),
        );
        racer_contacts += pairs;
        contact_responses +=
          u32::from(pairs > 0 && before_contact != (car.position, car.velocity()));
        positions.add_contacts(hits);
        positions.rank_dispatched(
          &lrsim::race_positions::poses(&car, &rivals),
          &data.checkpoints,
        );
        positions.finish(&lrsim::race_positions::finished(&race, &rivals));
        distance += lrsim::contact::length(lrsim::contact::sub(car.position, old));
        max_speed = max_speed.max(car.speed());
        min_steer = min_steer.min(actions.steer);
        max_steer = max_steer.max(actions.steer);
      } else {
        assert_eq!(car.position, data.start.position);
        assert_eq!(car.velocity(), [0.0; 3]);
        assert_eq!(race.elapsed, 0.0);
        assert_eq!(racer_contacts, 0);
        assert!(rivals
          .iter()
          .all(|r| r.motion.position == r.data.record.start_position && r.race.elapsed == 0.0));
      }
      assert!(
        car
          .position
          .iter()
          .chain(&car.velocity())
          .all(|v| v.is_finite()),
        "attempt {attempt} tick {tick}"
      );
      assert_eq!(car.unresolved_contacts, 0, "attempt {attempt} tick {tick}");
      if tick % (30 * HZ) == 0 || (race.finished && rivals.iter().all(|r| r.race.finished)) {
        println!("attempt {attempt} tick {tick} player_laps {} player_time {:.3} player_position {:?} route_index {} rival_laps {:?} racer_contacts {racer_contacts} checkpoint_contacts {}",
          race.laps.completed_laps, race.elapsed, car.position, driver.index,
          rivals.iter().map(|r| r.race.laps.completed_laps).collect::<Vec<_>>(),
          positions.report().finite_probe_contacts);
      }
      if race.finished && rivals.iter().all(|r| r.race.finished) {
        break;
      }
    }
    println!(
      "PLAYABLE_CPU_AUTOMATED_INPUT {}",
      serde_json::json!({
        "attempt": attempt, "countdown": countdown_numerals,
        "hz": HZ,
        "player": {"finished": race.finished, "laps": race.laps.completed_laps,
          "elapsed": race.elapsed, "lap_times": race.lap_times, "events": race.events,
          "distance": distance, "max_speed": max_speed, "steer_range": [min_steer, max_steer],
          "world_collisions": car.collisions, "unresolved_contacts": car.unresolved_contacts},
        "rivals": rivals.iter().map(Rival::report).collect::<Vec<_>>(),
        "racer_contacts": racer_contacts, "player_contact_response_ticks": contact_responses,
        "positions": positions.report(),
      })
    );
    assert_eq!(countdown_numerals, [3, 2, 1]);
    assert!(max_speed > 50.0 && min_steer < -0.1 && max_steer > 0.1);
    assert!(distance > 1000.0);
    assert!(
      racer_contacts > 0 && contact_responses > 0,
      "no natural grid/race player contact response"
    );
    assert!(positions.report().finite_probe_contacts > 0);
    assert!(race.finished, "attempt {attempt}: player never finished");
    assert_eq!(race.laps.completed_laps, 3);
    assert_eq!(race.lap_times.len(), 3);
    assert_eq!(
      race
        .events
        .iter()
        .filter(|e| e.state_changed)
        .map(|e| e.event.id)
        .collect::<Vec<_>>(),
      [101, 102, 100, 101, 102, 100, 101, 102, 100, 101]
    );
    for rival in &rivals {
      assert!(
        rival.race.finished,
        "attempt {attempt}: {} never finished",
        rival.data.route_name
      );
      assert_eq!(rival.race.laps.completed_laps, 3);
      assert_eq!(rival.race.lap_times.len(), 3);
    }
    assert!(positions
      .report()
      .states
      .iter()
      .all(|s| s.flags & 0x1000 != 0));
  }
}
