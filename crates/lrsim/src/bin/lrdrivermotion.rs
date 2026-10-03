//! Normal driver-selection oracle adapter; not opponent AI.
use lrsim::driver_motion::{self, Motion};
use std::io::{self, Read};

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let mut text = String::new();
  io::stdin().read_to_string(&mut text)?;
  let cases: Vec<Motion> = serde_json::from_str(&text)?;
  let output: Vec<_> = cases
    .iter()
    .map(|c| driver_motion::select(c).index())
    .collect();
  println!("{}", serde_json::to_string(&output)?);
  Ok(())
}
