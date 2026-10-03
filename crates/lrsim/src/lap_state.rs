//! Original racer lap history: 00439b70, 00439ba0, 00439c40.
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct LapState {
  pub counter: i32,
  pub completed_laps: u32,
  pub history: [u32; 3],
}

impl Default for LapState {
  fn default() -> Self {
    Self {
      counter: -1,
      completed_laps: 0,
      history: [0, 2, 1],
    }
  }
}

impl LapState {
  /// Return true only when the original maximum completed lap advances.
  pub fn event(&mut self, mode: u32) -> bool {
    if mode > 2 || self.history[0] == mode {
      return false;
    }
    let previous = self.completed_laps;
    if mode == 1 && self.history == [0, 2, 1] {
      self.counter += 1;
      if self.counter > 0 && (self.completed_laps as i32) < self.counter {
        self.completed_laps += 1;
      }
    }
    self.history = [mode, self.history[0], self.history[1]];
    previous != self.completed_laps
  }
}
