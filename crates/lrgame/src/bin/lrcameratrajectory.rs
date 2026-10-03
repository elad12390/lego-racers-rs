//! Camera following adapter; no window and no full visual-dispatch parity claim.
#[path = "../camera_rig.rs"]
#[allow(dead_code)]
mod camera_rig;
use serde::Deserialize;
use std::io::{self, Read};
#[derive(Deserialize)]
struct Frame {
  position: [f32; 3],
  heading: f32,
  turn_rate: f32,
  dt_ms: f32,
}
#[derive(Deserialize)]
struct Case {
  preset: usize,
  frames: Vec<Frame>,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
  let mut input = String::new();
  io::stdin().read_to_string(&mut input)?;
  let cases: Vec<Case> = serde_json::from_str(&input)?;
  let output: Vec<_> = cases
    .into_iter()
    .map(|case| {
      let first = &case.frames[0];
      let forward = |angle: f32| [angle.cos(), angle.sin(), 0.0];
      let mut rig = camera_rig::ChaseRig::with_preset(
        first.position,
        forward(first.heading),
        camera_rig::PRESETS[case.preset],
      );
      case
        .frames
        .into_iter()
        .map(|f| {
          rig.update(
            f.position,
            forward(f.heading),
            f.turn_rate,
            f.dt_ms / 1000.0,
          );
          let (eye, forward, up) = rig.pose();
          serde_json::json!({"eye":eye,"forward":forward,"up":up})
        })
        .collect::<Vec<_>>()
    })
    .collect();
  println!("{}", serde_json::to_string(&output)?);
  Ok(())
}
