//! Suppress stale held keys after focus loss, pause and restart.
use crate::platform::prelude::*;
use std::collections::HashSet;

pub struct InputGate {
  active: bool,
  blocked: HashSet<KeyCode>,
}
impl Default for InputGate {
  fn default() -> Self {
    Self {
      active: true,
      blocked: HashSet::new(),
    }
  }
}
impl InputGate {
  fn update(
    &mut self,
    active: bool,
    held: HashSet<KeyCode>,
    fresh: HashSet<KeyCode>,
    released: HashSet<KeyCode>,
  ) -> bool {
    let lost = self.active && !active;
    self.active = active;
    if active {
      self
        .blocked
        .retain(|key| !fresh.contains(key) && !released.contains(key));
    } else {
      self.blocked.extend(held);
    }
    lost
  }
  /// Diagnostic playback is deliberately independent of the user's active app.
  pub fn poll(&mut self, diagnostic: bool) -> bool {
    self.update(
      diagnostic || crate::native_window::focused(),
      get_keys_down(),
      get_keys_pressed(),
      get_keys_released(),
    )
  }
  pub fn suspend(&mut self) {
    self.blocked.extend(get_keys_down());
    clear_input_queue();
  }
  pub fn active(&self) -> bool {
    self.active
  }
  pub fn down(&self, key: KeyCode) -> bool {
    self.active && !self.blocked.contains(&key) && is_key_down(key)
  }
  pub fn pressed(&self, key: KeyCode) -> bool {
    self.active && !self.blocked.contains(&key) && is_key_pressed(key)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn focus_loss_latches_once_and_held_keys_require_a_fresh_press_or_release() {
    let mut gate = InputGate::default();
    let held = HashSet::from([KeyCode::Up, KeyCode::Space]);
    assert!(gate.update(false, held.clone(), HashSet::new(), HashSet::new()));
    assert!(!gate.update(false, held.clone(), held.clone(), HashSet::new()));
    assert!(!gate.update(true, held.clone(), HashSet::new(), HashSet::new()));
    assert_eq!(gate.blocked, held);
    gate.update(
      true,
      held,
      HashSet::from([KeyCode::Up]),
      HashSet::from([KeyCode::Space]),
    );
    assert!(gate.blocked.is_empty());
  }
}
