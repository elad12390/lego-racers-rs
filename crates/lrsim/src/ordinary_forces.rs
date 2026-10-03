//! Ordinary00445500world-space force accumulation in original millisecond units.
//! Power/hold flags and external forces remain separate, not silently approximated.
use crate::contact::normalized;
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
pub struct Case {
  pub velocity: [f32; 3],
  pub direction: [f32; 3],
  pub forward: [f32; 3],
  pub mass: f32,
  pub throttle: f32,
  pub reference_speed: f32,
  pub radius: f32,
  pub wheels: u32,
  pub slope_force: [f32; 3],
  pub friction: f32,
  pub surface_drag: f32,
  pub surface_force: [f32; 3],
}

#[derive(Serialize)]
pub struct Result {
  pub force: [f32; 3],
  pub yaw_rate: f32,
  pub turn_rate: f32,
}

fn precise_dot(a: [f32; 3], b: [f32; 3]) -> f64 {
  //00447330 sums Z/Y first, then X, retaining all products.
  (f64::from(a[2]) * f64::from(b[2]) + f64::from(a[1]) * f64::from(b[1]))
    + f64::from(a[0]) * f64::from(b[0])
}

pub fn solve(c: &Case) -> Result {
  let scale = (f64::from(c.mass) * f64::from(0.001f32) * f64::from(0.001f32)) as f32;
  let gravity = -39.0 * scale;
  let speed = precise_dot(c.velocity, c.velocity).sqrt() as f32;
  let longitudinal = precise_dot(c.velocity, c.direction);
  let forward_speed = longitudinal as f32;
  let parallel = c.direction.map(|d| (f64::from(d) * longitudinal) as f32);
  let transverse = std::array::from_fn::<_, 3, _>(|i| {
    let product = f64::from(c.direction[i]) * longitudinal;
    if i == 0 {
      (f64::from(c.velocity[i]) - product) as f32
    } else {
      c.velocity[i] - product as f32
    }
  });
  let mut force = [0.0; 3];
  let mut add = |value: [f32; 3]| {
    for i in 0..3 {
      force[i] += value[i];
    }
  };
  if c.wheels == 0 {
    add([0.0, 0.0, gravity * 4.0]);
    let strength = f64::from(c.throttle) * f64::from(scale);
    let mut propulsion = c.direction.map(|d| (f64::from(d) * strength) as f32);
    propulsion[2] = propulsion[2].min(-gravity);
    add(propulsion);
  } else {
    let slope_length = precise_dot(c.slope_force, c.slope_force).sqrt();
    if f64::from(39.0) * f64::from(scale) * f64::from(c.friction) < slope_length {
      add(c.slope_force);
    }
    add(transverse.map(|v| (f64::from(v) * f64::from(c.mass) * f64::from(-0.01f32)) as f32));
    if c.throttle == 0.0 {
      add(parallel.map(|v| (f64::from(v) * f64::from(c.mass) * f64::from(-0.001f32)) as f32));
    } else {
      let direction = if c.wheels >= 3 {
        c.forward
      } else {
        c.direction
      };
      add(direction.map(|d| (f64::from(d) * f64::from(c.throttle) * f64::from(scale)) as f32));
    }
    if c.radius != 0.0 {
      let centripetal = f64::from(c.mass) * f64::from(forward_speed) * f64::from(forward_speed)
        / f64::from(c.radius);
      add(
        normalized([-c.direction[1], c.direction[0], 0.0])
          .map(|d| (f64::from(d) * centripetal) as f32),
      );
    }
  }
  add(c.surface_force.map(|f| f * (scale * 0.25)));
  let coefficient = (f64::from(c.mass) * f64::from(c.throttle.abs())
    / (f64::from(c.reference_speed) * f64::from(c.reference_speed))) as f32;
  let drag = -((f64::from(c.surface_drag) + f64::from(coefficient)) * f64::from(speed));
  add(c.velocity.map(|v| (f64::from(v) * drag) as f32));
  let yaw_speed = if c.wheels > 0 && forward_speed > 0.0005 && forward_speed < 0.03 {
    0.03
  } else {
    forward_speed
  };
  let yaw_rate = if c.radius == 0.0 {
    0.0
  } else {
    yaw_speed / c.radius
  };
  Result {
    force,
    yaw_rate,
    turn_rate: if c.wheels > 0 { yaw_rate } else { 0.0 },
  }
}
