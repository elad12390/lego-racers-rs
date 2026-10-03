//! Original ordinary Driver::UpdateSteering numerical path (0041fee0).
//! Skid effect spawning and aggressive steering remain separate receivers.
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct Case {
  pub requested_radius: f32,
  pub speed_ms: f32,
  pub forward_speed_ms: f32,
  pub mass: f32,
  pub grounded: [bool; 4],
  pub grip: [f32; 4],
  pub grounded_scale: f32,
}

#[derive(Serialize)]
pub struct Output {
  pub radius: f32,
}

pub fn solve(case: &Case) -> Output {
  if case.requested_radius == 0.0 { return Output {radius: 0.0}; }
  let requested = if case.speed_ms < 0.04f32 {
    case.requested_radius * 0.2f32
  } else { case.requested_radius };
  let count = case.grounded.iter().filter(|v| **v).count();
  let sum = case.grip.iter().zip(case.grounded)
    .filter(|(_, supported)| *supported).map(|(value, _)| f64::from(*value)).sum::<f64>();
  let average = if count == 0 { 0.0 } else { sum / count as f64 * f64::from(case.grounded_scale) };
  let mass_scale = (f64::from(case.mass) * f64::from(0.001f32) * f64::from(0.001f32)) as f32;
  let denominator = f64::from(39.0f32) * f64::from(mass_scale) * average;
  let ratio = if denominator != 0.0 {
    (f64::from(case.forward_speed_ms).powi(2) * f64::from(case.mass) / denominator) as f32
  } else if count == 0 { 100.0 } else { 4096.0 };
  let magnitude = requested.abs();
  let radius = if ratio > magnitude {
    ((f64::from(ratio) + f64::from(magnitude)) * 0.5) as f32 * requested.signum()
  } else { requested };
  Output { radius: crate::handling::clamp_radius(radius) }
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn turbo_grip_changes_radius_not_acceleration_and_airborne_has_its_own_default() {
    let mut case = Case {requested_radius: 40.0, speed_ms: 0.15, forward_speed_ms: 0.15,
      mass: 20.0, grounded: [true;4], grip: [3.0;4], grounded_scale: 1.0};
    let ordinary = solve(&case).radius;
    case.grounded_scale = 1.5;
    let boosted = solve(&case).radius;
    assert!(ordinary > boosted && boosted > 40.0);
    case.grounded = [false;4];
    assert_eq!(solve(&case).radius,70.0);
    case.requested_radius = 0.0;
    assert_eq!(solve(&case).radius,0.0);
  }
}
