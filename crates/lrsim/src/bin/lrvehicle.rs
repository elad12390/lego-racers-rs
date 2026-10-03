//! Headless real-asset vehicle benchmark, not playable-game evidence.
use lrformats::library::Library;
use lrsim::{
  contact::{length, sub},
  diagnostic_driver::DiagnosticDriver,
  race_data::RaceData,
  vehicle::Vehicle,
};
use std::process::ExitCode;

fn run() -> Result<(), String> {
  let args: Vec<_> = std::env::args().skip(1).collect();
  let [jam, table, seconds] = args.as_slice() else {
    return Err("usage: lrvehicle LEGO.JAM RACE_TABLE SECONDS".into());
  };
  let seconds: f32 = seconds.parse().map_err(|_| "invalid duration")?;
  if !seconds.is_finite() || !(0.0..=600.0).contains(&seconds) {
    return Err("duration out of bounds".into());
  }
  let library = Library::open(jam).map_err(|e| e.to_string())?;
  let data = RaceData::load(&library, table, "bkchas0")?;
  let mut car = Vehicle::spawn(&data.start, &data.chassis, &data.contacts);
  let mut driver = DiagnosticDriver::new(data.diagnostic_line);
  let mut race = lrsim::race::Race::new(3)?;
  let mut distance = 0.0;
  let mut grounded = 0;
  let mut trajectory = Vec::new();
  for tick in 0..(seconds * 120.0) as usize {
    let old = car.position;
    let actions = driver.actions(&car);
    car.step(actions, &data.contacts, 1.0 / 120.0);
    race.advance(old, car.position, &data.lap_zones, 1.0 / 120.0);
    distance += length(sub(car.position, old));
    if car.grounded {
      grounded += 1;
    }
    if car
      .position
      .iter()
      .chain(&car.velocity())
      .any(|v| !v.is_finite())
    {
      return Err("nonfinite vehicle state".into());
    }
    if tick % 120 == 0 {
      trajectory.push(serde_json::json!({"time":tick as f32/120.0,"position":car.position,"speed":car.speed(),"line_index":driver.index,"steer":actions.steer,"throttle":actions.throttle,"grounded":car.grounded,"heading":car.heading,"up":car.up,"wheels":car.supported_wheels,"collisions":car.collisions}));
    }
    if race.finished {
      break;
    }
  }
  println!(
    "{}",
    serde_json::json!({"mode":"headless_diagnostic_not_gameplay","distance":distance,"position":car.position,
        "speed":car.speed(),"supported_ticks":grounded,"collisions":car.collisions,"unresolved_contacts":car.unresolved_contacts,
        "line_samples_reached":driver.index,"line_samples_total":driver.points.len(),"trajectory":trajectory,
        "lap_state":race.laps,"finished":race.finished,"elapsed":race.elapsed,"lap_times":race.lap_times,"race_events":race.events})
  );
  Ok(())
}

fn main() -> ExitCode {
  match run() {
    Ok(()) => ExitCode::SUCCESS,
    Err(e) => {
      eprintln!("{e}");
      ExitCode::FAILURE
    }
  }
}
