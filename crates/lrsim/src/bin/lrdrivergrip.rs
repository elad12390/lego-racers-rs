//! Probe the actual native Driver grip helper.
use std::io::{self, Read};
fn main() -> Result<(), Box<dyn std::error::Error>> {
  let mut input = String::new();
  io::stdin().read_to_string(&mut input)?;
  let cases: Vec<lrsim::driver_grip::Case> = serde_json::from_str(&input)?;
  let outputs = cases.iter().map(lrsim::driver_grip::solve).collect::<Vec<_>>();
  println!("{}", serde_json::to_string(&outputs)?);
  Ok(())
}
