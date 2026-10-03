//! Native isolated recovery adapter on real route files; not full opponent AI.
use lrsim::recovery_driver::{Pose, RecoveryDriver, State};
use serde::Deserialize;
use std::io::{self, Read};
#[derive(Deserialize)]
struct Case {
  path: String,
  state: State,
  pose: Pose,
  elapsed: u32,
  #[serde(default = "one")]
  frames: usize,
  #[serde(default)]
  follow_target: bool,
  #[serde(default)]
  begin: bool,
}
fn one() -> usize {
  1
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
  let mut input = String::new();
  io::stdin().read_to_string(&mut input)?;
  let cases: Vec<Case> = serde_json::from_str(&input)?;
  let mut output = Vec::new();
  for case in cases {
    let record = lrformats::route::RouteRecord::load(&std::fs::read(&case.path)?, false)?;
    let mut driver = RecoveryDriver::new(&record, case.state);
    let mut trace = Vec::new();
    if case.begin {
      driver.begin();
    }
    let initial_target = driver.state.target;
    for _ in 0..case.frames {
      let mut pose = case.pose.clone();
      if case.follow_target {
        pose.position =
          std::array::from_fn(|i| driver.state.target[i] + (pose.position[i] - initial_target[i]));
      }
      let command = driver.tick(&pose, case.elapsed)?;
      trace.push(serde_json::json!({"state":driver.state,"command":command}));
    }
    output.push(trace);
  }
  println!("{}", serde_json::to_string(&output)?);
  Ok(())
}
