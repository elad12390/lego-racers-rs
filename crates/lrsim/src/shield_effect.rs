//! FileTimedEffect 0045c2a0 keeps protection through a final 1000 ms fade.
use serde::Serialize;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub enum Phase {
  Running,
  Fading,
  #[default]
  Finished,
}
#[derive(Clone, Default, Serialize)]
pub struct Playback {
  pub phase: Phase,
  pub remaining_ms: u32,
  pub elapsed_ms: u32,
}
impl Playback {
  pub fn start(&mut self, running_ms: u32) {
    *self = Self {phase: Phase::Running, remaining_ms: running_ms, elapsed_ms: 0};
  }
  pub fn active(&self) -> bool { self.phase != Phase::Finished }
  pub fn advance(&mut self, ticks: u32, fade_ms: u32) {
    if !self.active() || ticks == 0 { return; }
    self.elapsed_ms = self.elapsed_ms.wrapping_add(ticks);
    if ticks < self.remaining_ms { self.remaining_ms -= ticks; return; }
    (self.phase, self.remaining_ms) = match self.phase {
      Phase::Running => (Phase::Fading, fade_ms),
      _ => (Phase::Finished, 0),
    };
  }
  pub fn inner_alpha(&self) -> u8 {
    match self.phase {
      Phase::Running => 255,
      Phase::Fading => (f64::from(self.remaining_ms) * f64::from(0.001f32) * 255.0) as u8,
      Phase::Finished => 0,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn source_duration_enters_fade_before_releasing_protection() {
    for duration in [4000, 6000, 8000, 10000] {
      let mut effect = Playback::default();
      effect.start(duration);
      effect.advance(duration, 1000);
      assert_eq!(effect.phase, Phase::Fading);
      assert_eq!(effect.inner_alpha(), 255);
      assert!(effect.active());
      effect.advance(500, 1000);
      assert_eq!(effect.inner_alpha(), 127);
      assert!(effect.active());
      effect.advance(499, 1000);
      assert_eq!(effect.inner_alpha(), 0);
      assert!(effect.active());
      effect.advance(1, 1000);
      assert!(!effect.active());
    }
  }
  #[test]
  fn excess_elapsed_is_discarded_and_retrigger_restarts_both_children() {
    let mut effect = Playback::default();
    effect.start(4000);
    effect.advance(20000, 1000);
    assert_eq!((effect.phase, effect.remaining_ms), (Phase::Fading, 1000));
    effect.start(6000);
    assert_eq!((effect.phase, effect.remaining_ms, effect.elapsed_ms), (Phase::Running, 6000, 0));
  }
}
