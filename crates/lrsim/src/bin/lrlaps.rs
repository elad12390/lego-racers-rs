//! Native lap-history probe, used for unchanged-original comparisons.
use lrsim::lap_state::LapState;
use std::io::{self, Read};

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let mut input = String::new();
  io::stdin().read_to_string(&mut input)?;
  let sequences: Vec<Vec<u32>> = serde_json::from_str(&input)?;
  let results: Vec<_> = sequences
    .into_iter()
    .map(|sequence| {
      let mut state = LapState::default();
      sequence
        .into_iter()
        .map(|mode| {
          state.event(mode);
          state.clone()
        })
        .collect::<Vec<_>>()
    })
    .collect();
  println!("{}", serde_json::to_string(&results)?);
  Ok(())
}
