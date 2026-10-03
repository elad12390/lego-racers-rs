//! Native free-racer/unowned-contact oracle adapter.
use lrsim::racer_response::{self, Case};
use std::io::{self, Read};
fn main() -> Result<(), Box<dyn std::error::Error>> {
  let mut input = String::new();
  io::stdin().read_to_string(&mut input)?;
  let cases: Vec<Case> = serde_json::from_str(&input)?;
  let output: Vec<_> = cases.iter().map(racer_response::unowned_contact).collect();
  println!("{}", serde_json::to_string(&output)?);
  Ok(())
}
