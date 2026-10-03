//! Original TurboEffect startup/run/tail timers and clip selection.
//! Physical boost forces, smoke and external effect-slot exhaustion are separate.
use serde::Serialize;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub enum Phase {
  Starting,
  Running,
  Tail,
  #[default]
  Finished,
}
#[derive(Clone, Default, Serialize)]
pub struct Playback {
  pub activation: u32,
  pub elapsed_ms: u32,
  pub phase: Phase,
  pub remaining_ms: u32,
  pub clip_ms: u32,
}
impl Playback {
  pub fn start(&mut self, startup_ms: u32) {
    self.activation = self.activation.wrapping_add(1);
    self.phase = Phase::Starting;
    self.remaining_ms = startup_ms;
    self.clip_ms = 0;
    self.elapsed_ms = 0;
  }
  pub fn active(&self) -> bool {
    self.phase != Phase::Finished
  }
  pub fn cancel(&mut self) {
    *self = Self {activation: self.activation, ..Self::default()};
  }
  pub fn clip(&self) -> Option<usize> {
    match self.phase {
      Phase::Starting => Some(0),
      Phase::Running => Some(1),
      Phase::Tail => Some(2),
      Phase::Finished => None,
    }
  }
  pub fn advance(&mut self, elapsed: u32, running_ms: u32, tail_ms: u32, repeat_running: bool) {
    if !self.active() || elapsed == 0 {
      return;
    }
    // TimedEffect countdown <= calls exactly one transition, discarding excess.
    self.elapsed_ms = self.elapsed_ms.wrapping_add(elapsed);
    // Children step AFTER transition, so the new clip consumes the whole tick.
    if self.remaining_ms <= elapsed {
      if self.phase == Phase::Running && repeat_running {
        self.remaining_ms = running_ms;
        self.clip_ms = self.clip_ms.wrapping_add(elapsed);
        return;
      }
      self.clip_ms = 0;
      (self.phase, self.remaining_ms) = match self.phase {
        Phase::Starting => (Phase::Running, running_ms),
        Phase::Running => (Phase::Tail, tail_ms),
        _ => (Phase::Finished, 0),
      };
    } else {
      self.remaining_ms -= elapsed;
    }
    self.clip_ms = self.clip_ms.wrapping_add(elapsed);
  }
  pub fn flame_alpha(&self) -> u8 {
    match self.phase {
      Phase::Starting => {
        if self.remaining_ms < 51 {
          255
        } else {
          0
        }
      }
      Phase::Tail => ((self.remaining_ms.saturating_sub(350) as f64 / 350.0) * 255.0) as u8,
      Phase::Running => 255,
      Phase::Finished => 0,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn startup_exact_boundary_running_and_tail_select_all_three_clips() {
    let mut effect = Playback::default();
    effect.start(400);
    assert_eq!(effect.clip(), Some(0));
    effect.advance(349, 1500, 700, false);
    assert_eq!(effect.flame_alpha(), 0);
    effect.advance(1, 1500, 700, false);
    assert_eq!(effect.flame_alpha(), 255);
    effect.advance(50, 1500, 700, false);
    assert_eq!(effect.clip(), Some(1));
    assert_eq!((effect.remaining_ms, effect.clip_ms), (1500, 50));
    effect.advance(1500, 1500, 700, false);
    assert_eq!(effect.clip(), Some(2));
    effect.advance(175, 1500, 700, false);
    assert_eq!(effect.flame_alpha(), 127);
    effect.advance(175, 1500, 700, false);
    assert_eq!(effect.flame_alpha(), 0);
    assert!(effect.active());
    effect.advance(350, 1500, 700, false);
    assert!(!effect.active());
  }
  #[test]
  fn oversized_tick_transitions_once_and_retrigger_restarts_startup() {
    let mut effect = Playback::default();
    effect.start(400);
    effect.advance(10_000, 5000, 700, false);
    assert_eq!((effect.phase, effect.remaining_ms), (Phase::Running, 5000));
    effect.start(400);
    assert_eq!(
      (effect.phase, effect.remaining_ms, effect.clip_ms),
      (Phase::Starting, 400, 0)
    );
    effect.cancel();
    assert_eq!(effect.clip(), None);
  }
  #[test]
  fn sustained_tier_three_reloads_timer_without_restarting_its_running_clip() {
    let mut effect = Playback::default();
    effect.start(400);
    effect.advance(400, 5000, 700, true);
    effect.advance(5000, 5000, 700, true);
    assert_eq!(effect.phase, Phase::Running);
    assert_eq!((effect.remaining_ms, effect.clip_ms), (5000, 5400));
  }
  #[test]
  fn turbo_uv_clock_survives_clip_transitions_but_restarts_on_activation() {
    let mut effect = Playback::default();
    effect.start(400);
    effect.advance(400,1000,700,false);
    assert_eq!(effect.elapsed_ms,400);
    effect.advance(1000,1000,700,false);
    assert_eq!((effect.elapsed_ms,effect.clip_ms),(1400,1000));
    effect.start(400);
    assert_eq!(effect.elapsed_ms,0);
  }
}
