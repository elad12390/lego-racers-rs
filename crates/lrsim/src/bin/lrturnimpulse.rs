//! Physical left-axis impulse oracle adapter; not native driving acceptance.
use lrsim::{angular_motion::AngularMotion, attitude::Basis};
use serde::Deserialize;
use std::io::{self, Read};

#[derive(Deserialize)]
struct Case {
  mass: f32,
  basis: [f32; 9],
  rate_ms: f32,
  hold_ms: u32,
  roll_hold_ms: u32,
  ticks: u32,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
  let mut input = String::new();
  io::stdin().read_to_string(&mut input)?;
  let cases: Vec<Case> = serde_json::from_str(&input)?;
  let mut results = Vec::new();
  for case in cases {
    let basis = Basis { forward: case.basis[..3].try_into()?, left: case.basis[3..6].try_into()?, up: case.basis[6..].try_into()? };
    let mut angular = AngularMotion::new(case.mass);
    if case.roll_hold_ms > 0 { angular.apply_roll(basis, 0.0, case.roll_hold_ms); }
    let accepted = angular.apply_pitch(basis, case.rate_ms, case.hold_ms);
    let impulse = angular.momentum();
    let result = if case.ticks == 0 { basis } else {
      angular.advance(basis, [0.0;3], [[0.0;3];4], [false;4], 0, basis.up, 0.0, None, case.ticks as f32)
    };
    let result_basis = [result.forward,result.left,result.up].concat();
    results.push(serde_json::json!({"accepted":accepted,"impulse":impulse,"momentum":angular.momentum(),"basis":result_basis,"pitch_hold_ms":angular.pitch_hold_ms()}));
  }
  println!("{}", serde_json::to_string(&results)?);
  Ok(())
}
