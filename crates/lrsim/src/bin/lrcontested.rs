//! Coupled real-asset race benchmark, never human-play or whole-game acceptance.
use lrformats::library::Library;
use lrsim::{
  diagnostic_driver::DiagnosticDriver, race::Race, race_contacts, race_data::RaceData, rival_data,
  rivals::Rival, vehicle::Vehicle,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let args: Vec<_> = std::env::args().skip(1).collect();
  let [jam, table] = args.as_slice() else {
    return Err("usage: lrcontested LEGO.JAM RACE_TABLE".into());
  };
  let library = Library::open(jam)?;
  let data = RaceData::load(&library, table, "bkchas0")?;
  let definitions = rival_data::load(&library, table, [0, 1, 2, 3, 4])?;
  let mut rivals = definitions
    .iter()
    .map(Rival::new)
    .collect::<Result<Vec<_>, _>>()?;
  let mut car = Vehicle::spawn(&data.start, &data.chassis, &data.contacts);
  let mut driver = DiagnosticDriver::new(data.diagnostic_line);
  let mut race = Race::new(3)?;
  let mut pair_contacts = 0;
  let mut trajectory = Vec::new();
  let mut positions =
    lrsim::race_positions::RacePositions::new(&lrsim::race_positions::poses(&car, &rivals));
  let world = lrsim::world_dispatch::ChassisWorld::new(
    data
      .contacts
      .primary_collider()
      .ok_or("missing primary query")?
      .clone(),
    &data.checkpoints,
  );
  for tick in 0..36000 {
    let dt = 1.0 / 60.0;
    let old = car.position;
    let actions = driver.actions(&car);
    let before = serde_json::json!({"position":car.position,"velocity":car.velocity(),"basis":car.basis(),"wheels":car.supported_wheels,"throttle":actions.throttle,"steer":actions.steer});
    if !race.finished {
      let hits = car.step_dispatched(
        actions,
        &data.contacts,
        dt as f32,
        &world,
        positions.state_mut(0),
      );
      positions.add_contacts(hits);
    }
    let after_step = serde_json::json!({"position":car.position,"velocity":car.velocity(),"basis":car.basis(),"wheels":car.supported_wheels});
    for (index, rival) in rivals.iter_mut().enumerate() {
      let hits = rival.advance_dispatched(
        dt,
        &data.lap_zones,
        &data.contacts,
        &world,
        positions.state_mut(index + 1),
      );
      positions.add_contacts(hits);
    }
    let (pairs, hits) = race_contacts::resolve_dispatched(
      &mut car,
      &data.chassis,
      &data.contacts,
      &mut rivals,
      &world,
      positions.state_mut(0),
    );
    pair_contacts += pairs;
    positions.add_contacts(hits);
    positions.rank_dispatched(
      &lrsim::race_positions::poses(&car, &rivals),
      &data.checkpoints,
    );
    race.advance(old, car.position, &data.lap_zones, dt);
    positions.finish(&lrsim::race_positions::finished(&race, &rivals));
    if car
      .position
      .iter()
      .chain(&car.velocity())
      .any(|x| !x.is_finite())
      || rivals.iter().any(|r| {
        r.motion
          .position
          .iter()
          .chain(&r.motion.velocity)
          .any(|x| !x.is_finite())
      })
    {
      return Err(format!("nonfinite coupled state at tick{tick}; before={before}; after_step={after_step}; after_contacts={}",serde_json::json!({"position":car.position,"velocity":car.velocity(),"basis":car.basis(),"rivals":rivals.iter().map(Rival::report).collect::<Vec<_>>()})).into());
    }
    if tick % 60 == 0 {
      trajectory.push(serde_json::json!({"time":tick as f32/60.0,"player_position":car.position,"player_speed":car.speed(),"player_laps":race.laps.completed_laps,"pair_contacts":pair_contacts,"rivals":rivals.iter().map(Rival::report).collect::<Vec<_>>()}));
    }
    if race.finished && rivals.iter().all(|r| r.race.finished) {
      break;
    }
  }
  println!(
    "{}",
    serde_json::json!({"mode":"coupled_headless_diagnostic_not_full_opponents_or_gameplay","player_finished":race.finished,"player_laps":race.laps.completed_laps,"player_time":race.elapsed,"unresolved_contacts":car.unresolved_contacts,"secondary_wheel_contacts":car.secondary_wheel_contacts,"rejected_empty_manifolds":lrsim::racer_box::rejected_empty_manifolds(),"pair_contacts":pair_contacts,"rivals":rivals.iter().map(Rival::report).collect::<Vec<_>>(),"positions":positions.report(),"trajectory":trajectory})
  );
  Ok(())
}
