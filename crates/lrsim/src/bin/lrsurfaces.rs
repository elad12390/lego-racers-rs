//! Original TMB effective ordinary wheel surface adapter, no GPU/effects.
use serde::Deserialize;
use std::io::{self, Read};
#[derive(Deserialize)]
struct Case {
  bytes: Vec<u8>,
  mirrored: bool,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
  let mut input = String::new();
  io::stdin().read_to_string(&mut input)?;
  let cases: Vec<Case> = serde_json::from_str(&input)?;
  let mut output = Vec::new();
  for case in cases {
    let surface = lrformats::collision_materials::surfaces(&case.bytes, case.mirrored)?;
    let entry = surface.get("test").ok_or("missing surface")?;
    output.push(serde_json::json!({"flags":entry.flags,"friction":entry.slope_friction,"drag":entry.quadratic_drag,"force":entry.force}));
  }
  println!("{}", serde_json::to_string(&output)?);
  Ok(())
}
