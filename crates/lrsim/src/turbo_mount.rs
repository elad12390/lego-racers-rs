//! TurboEffect mount: rearward/upward offset and left-facing original rig basis.
use crate::contact::{cross, normalized};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct Case {
  pub position: [f32; 3],
  pub basis: [f32; 9],
}
#[derive(Debug, Serialize)]
pub struct Mount {
  pub position: [f32; 3],
  pub forward: [f32; 3],
  pub left: [f32; 3],
  pub up: [f32; 3],
}
pub fn solve(case: &Case, offset: [f32; 2]) -> Mount {
  let position = std::array::from_fn(|i| {
    // Original x87 retains the X forward offset; Y/Z are spilled first.
    let rear = f64::from(case.basis[i]) * f64::from(offset[0]) + f64::from(case.position[i]);
    let rear = if i == 0 { rear } else { f64::from(rear as f32) };
    (rear + f64::from(case.basis[6 + i]) * f64::from(offset[1])) as f32
  });
  let forward = normalized(case.basis[3..6].try_into().unwrap());
  let left = normalized(cross(case.basis[6..9].try_into().unwrap(), forward));
  let up = normalized(cross(forward, left));
  Mount {position, forward, left, up}
}
