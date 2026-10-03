//! Original00440e10 angular step and00410900 orthonormalization, in seconds.
use crate::contact::{cross, dot, length, normalized};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Deserialize, Serialize)]
pub struct Basis {
  pub forward: [f32; 3],
  pub left: [f32; 3],
  pub up: [f32; 3],
}

///00410a00 projection retains its dot, spills X/Y products before subtracting,
/// and subtracts Z's product without spilling. Used by contact alignment.
pub fn perpendicular(primary: [f32; 3], secondary: [f32; 3]) -> [f32; 3] {
  let product = |i: usize| f64::from(primary[i]) * f64::from(secondary[i]);
  //0041091f/00410a1f evaluate (Y + Z) + X, not (X + Y) + Z.
  let projection = (product(1) + product(2)) + product(0);
  let projected = std::array::from_fn(|i| {
    let product = f64::from(primary[i]) * projection;
    if i < 2 {
      secondary[i] - (product as f32)
    } else {
      (f64::from(secondary[i]) - product) as f32
    }
  });
  normalized(projected)
}

pub fn integrate(basis: Basis, velocity: [f32; 3], dt: f32) -> Basis {
  let step = velocity.map(|v| v * dt);
  integrate_step(basis, step)
}

pub fn integrate_step(basis: Basis, step: [f32; 3]) -> Basis {
  let advance = |v: [f32; 3]| {
    let v = v.map(f64::from);
    let s = step.map(f64::from);
    //00440fbd..00441059spills only complete advanced components, not
    // a separately rounded cross-product before adding the basis column.
    [
      (v[2] * s[1] + (v[0] - v[1] * s[2])) as f32,
      ((v[0] * s[2] + v[1]) - v[2] * s[0]) as f32,
      (v[1] * s[0] + (v[2] - v[0] * s[1])) as f32,
    ]
  };
  let forward = normalized(advance(basis.forward));
  let up = advance(basis.up);
  let up = perpendicular(forward, up);
  Basis {
    forward,
    left: cross(up, forward),
    up,
  }
}

pub fn sleeps(velocity: [f32; 3], torque: [f32; 3], dt: f32) -> bool {
  torque == [0.0; 3] && dot(velocity, velocity) * dt * dt < 0.0006
}

/// Modern rigid-axis rotation avoids first-order angular drift. The original
/// normalized-Euler step above remains the reference, not an ABI requirement.
/// Their discrepancy is measured by the unchanged-executable comparator.
pub fn integrate_rotation(basis: Basis, velocity: [f32; 3], dt: f32) -> Basis {
  let speed = length(velocity);
  if speed == 0.0 {
    return basis;
  }
  let axis = velocity.map(|v| v / speed);
  let angle = speed * dt;
  let rotate = |v: [f32; 3]| {
    let turn = cross(axis, v);
    let projection = dot(axis, v);
    normalized(std::array::from_fn::<_, 3, _>(|i| {
      v[i] * angle.cos() + turn[i] * angle.sin() + axis[i] * projection * (1.0 - angle.cos())
    }))
  };
  Basis {
    forward: rotate(basis.forward),
    left: rotate(basis.left),
    up: rotate(basis.up),
  }
}

///00445d10limits the physical third column before querying wheel support.
/// Keep its horizontal tilt direction and reproject forward onto the new up.
pub fn clamp_tilt(basis: Basis) -> Basis {
  if basis.up[2] >= std::f32::consts::FRAC_1_SQRT_2 {
    return basis;
  }
  let length = basis.up[0].hypot(basis.up[1]);
  let horizontal = if length == 0.0 {
    [0.0; 3]
  } else {
    [basis.up[0] / length, basis.up[1] / length, 0.0]
  };
  let (sine, cosine) = std::f32::consts::FRAC_PI_4.sin_cos();
  let up = normalized([horizontal[0] * sine, horizontal[1] * sine, cosine]);
  let projection = dot(basis.forward, up);
  let forward = normalized(std::array::from_fn::<_, 3, _>(|i| {
    basis.forward[i] - up[i] * projection
  }));
  Basis {
    forward,
    left: cross(up, forward),
    up,
  }
}
