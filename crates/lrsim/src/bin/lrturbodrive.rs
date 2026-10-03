//! Probe the live native turbo control helpers, not a second implementation.
use std::io::{self, Read};
use serde::{Deserialize, Serialize};
#[derive(Deserialize)]
struct Case {acceleration_scale: f32, speed_scale: f32, contact: bool}
#[derive(Serialize)]
struct Output {throttle: f32, reference_speed: f32}
fn main() -> Result<(), Box<dyn std::error::Error>> {
  let mut input = String::new(); io::stdin().read_to_string(&mut input)?;
  if std::env::args().nth(1).as_deref() == Some("--uv") {
    let cases: Vec<lrsim::uv_scroll::Case> = serde_json::from_str(&input)?;
    println!("{}", serde_json::to_string(&cases.iter().map(lrsim::uv_scroll::sequence).collect::<Vec<_>>())?);
    return Ok(());
  }
  if std::env::args().nth(1).as_deref() == Some("--mount") {
    let cases: Vec<lrsim::turbo_mount::Case> = serde_json::from_str(&input)?;
    let rules = lrsim::powerups::Rules::load()?;
    let result = cases.iter().map(|c| lrsim::turbo_mount::solve(c, rules.turbo_mount_offset)).collect::<Vec<_>>();
    println!("{}", serde_json::to_string(&result)?);
    return Ok(());
  }
  let cases: Vec<Case> = serde_json::from_str(&input)?;
  let rules = lrsim::powerups::Rules::load()?;
  let result = cases.into_iter().map(|case| Output {
    throttle: lrsim::turbo_drive::throttle(case.acceleration_scale, case.contact,
      rules.turbo_drive_strength, rules.turbo_contact_scale),
    reference_speed: lrsim::turbo_drive::reference_speed(case.speed_scale, rules.turbo_speed_limit),
  }).collect::<Vec<_>>();
  println!("{}", serde_json::to_string(&result)?);
  Ok(())
}
