//! Ordinary00447cf0chassis response in original units per millisecond.
//! Attached-effect detachment and collision flag800 reactions are caller-owned.
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct Case {
  pub velocity: [f32; 3],
  pub normal: [f32; 3],
  pub forward: [f32; 3],
  pub left: [f32; 3],
  pub spin_hold: bool,
}
#[derive(Serialize)]
pub struct Response {
  pub velocity: [f32; 3],
  pub yaw_rate: Option<f32>,
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f64 {
  let p = |i: usize| f64::from(a[i]) * f64::from(b[i]);
  (p(1) + p(2)) + p(0)
}

pub fn solve(c: &Case) -> Response {
  let mut velocity = c.velocity;
  let incoming = dot(velocity, c.normal) as f32;
  if incoming < 0.0 {
    for i in 0..3 {
      velocity[i] = (f64::from(velocity[i]) - f64::from(incoming) * f64::from(c.normal[i])) as f32;
    }
  }
  let yaw_rate = if !c.spin_hold && dot(c.forward, c.normal) < 0.0 {
    let side = dot(c.left, c.normal) as f32;
    Some(if side >= 0.0 {
      (((1.0 - f64::from(side)) * 0.5 + 0.5) * f64::from(0.004f32)) as f32
    } else {
      (-f64::from(0.004f32) * ((f64::from(side) + 1.0) * 0.5 + 0.5)) as f32
    })
  } else {
    None
  };
  if incoming < 0.0 {
    let rebound = 0.3f32 * incoming;
    for i in 0..2 {
      velocity[i] = (f64::from(velocity[i]) - f64::from(c.normal[i]) * f64::from(rebound)) as f32;
    }
    velocity[2] = (f64::from(velocity[2])
      - f64::from(0.15f32) * f64::from(incoming) * f64::from(c.normal[2])) as f32;
  }
  velocity[0] = (f64::from(c.normal[0]) * f64::from(0.004f32) + f64::from(velocity[0])) as f32;
  for i in 1..3 {
    velocity[i] += c.normal[i] * 0.004;
  }
  velocity[2] = velocity[2].min(0.3);
  Response { velocity, yaw_rate }
}
