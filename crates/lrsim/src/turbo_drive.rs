//! Original Driver boost throttle override; 1.5 is grip scale, not propulsion.
use crate::powerups::Rules;

#[derive(Clone, Default)]
pub struct State {
  acceleration_scale: f32,
  speed_scale: f32,
  activation: Option<u32>,
  contact_latched: bool,
  pub throttle: Option<f32>,
}
impl State {
  pub fn new(acceleration_scale: f32, speed_scale: f32) -> Self {
    Self {acceleration_scale, speed_scale, ..Self::default()}
  }
  pub fn clear(&mut self) { self.activation = None; self.throttle = None; }
  pub fn reference_speed(&self, limit: f32) -> f32 { reference_speed(self.speed_scale, limit) }
  pub fn update(&mut self, activation: Option<u32>, contact_hits: u32, rules: &Rules) -> bool {
    let started = activation.is_some() && activation != self.activation;
    if started {
      self.contact_latched = false;
    }
    self.activation = activation;
    if activation.is_none() { self.throttle = None; return false; }
    self.contact_latched |= contact_hits != 0;
    self.throttle = Some(throttle(self.acceleration_scale, self.contact_latched,
      rules.turbo_drive_strength, rules.turbo_contact_scale));
    started
  }
}
pub fn throttle(acceleration_scale: f32, contact_latched: bool, strength: f32, contact_scale: f32) -> f32 {
  let drive = if contact_latched { strength * contact_scale } else { strength };
  drive * acceleration_scale
}
pub fn reference_speed(speed_scale: f32, limit: f32) -> f32 {
  speed_scale * limit
}
#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn turbo_contact_half_strength_latches_until_a_new_activation() {
    let rules = Rules::load().unwrap();
    let mut state = State::new(1.0, 1.0);
    assert!(state.update(Some(1), 0, &rules));
    assert_eq!(state.throttle, Some(432.0));
    assert!(!state.update(Some(1), 1, &rules));
    assert_eq!(state.throttle, Some(216.0));
    state.update(Some(1), 0, &rules);
    assert_eq!(state.throttle, Some(216.0));
    assert!(state.update(Some(2), 0, &rules));
    assert_eq!(state.throttle, Some(432.0));
    state.update(None, 0, &rules);
    assert_eq!(state.throttle, None);
  }
}
