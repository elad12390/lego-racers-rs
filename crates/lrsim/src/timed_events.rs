//! Original ChannelEffect and EventScheduler once-per-update start/stop pulses.
//! Noise samples are caller-supplied 10-bit values; the proprietary table is not copied.
use lrformats::timed_events::{Period, Timer};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Event {pub id: i32, pub stop: bool}
struct State {
  timer: Timer,
  delay_ms: u32,
  period_ms: u32,
  elapsed_ms: u32,
  active: bool,
}
pub struct Player {states: Vec<State>}
fn sample(period: &Period, noise: &mut impl FnMut() -> u16) -> u32 {
  if !period.random { return period.milliseconds; }
  let value = u32::from(noise() & 1023);
  if period.milliseconds < 1024 { value % period.milliseconds }
  else { value * (period.milliseconds / 1023) }
}
impl Player {
  pub fn new(timers: Vec<Timer>, mut noise: impl FnMut() -> u16) -> Self {
    Self {states: timers.into_iter().map(|timer| {
      // StartRandomEffect samples inactive range even when delayed; delayed
      // activation later schedules the active range but still begins inactive.
      let period_ms = sample(&timer.inactive, &mut noise);
      State {delay_ms: timer.delay_ms, timer, period_ms, elapsed_ms: 0, active: false}
    }).collect()}
  }
  pub fn reset(&mut self, mut noise: impl FnMut() -> u16) {
    for state in &mut self.states {
      state.delay_ms = state.timer.delay_ms;
      state.period_ms = sample(&state.timer.inactive, &mut noise);
      state.elapsed_ms = 0;
      state.active = false;
    }
  }
  pub fn advance(&mut self, ticks: u32, mut noise: impl FnMut() -> u16) -> Vec<Event> {
    let mut events = Vec::new();
    if ticks == 0 { return events; }
    // Original scheduler walks newest allocated node first.
    for state in self.states.iter_mut().rev() {
      if state.delay_ms != 0 {
        if state.delay_ms > ticks { state.delay_ms -= ticks; continue; }
        state.delay_ms = 0;
        state.period_ms = sample(&state.timer.active, &mut noise);
      }
      state.elapsed_ms = state.elapsed_ms.wrapping_add(ticks);
      if state.elapsed_ms < state.period_ms { continue; }
      events.push(Event {id: state.timer.id, stop: state.active});
      let (previous, next) = if state.active {(&state.timer.active, &state.timer.inactive)}
        else {(&state.timer.inactive, &state.timer.active)};
      let excess = state.elapsed_ms.wrapping_sub(previous.milliseconds);
      let remaining = if excess < next.milliseconds {next.milliseconds - excess}
        else {next.milliseconds};
      state.period_ms = sample(&Period {milliseconds: remaining, random: next.random}, &mut noise);
      state.elapsed_ms = 0;
      state.active = !state.active;
    }
    events
  }
}
#[cfg(test)]
mod tests {
  use super::*;
  fn fixed() -> Timer {Timer {id: 50, active: Period {milliseconds: 1200, random: false},
    inactive: Period {milliseconds: 8000, random: false}, delay_ms: 0}}
  #[test]
  fn exact_boundary_alternates_once_and_overshoot_shortens_next_period() {
    let mut player = Player::new(vec![fixed()], || 0);
    assert!(player.advance(7999, || 0).is_empty());
    assert_eq!(player.advance(2, || 0), [Event {id: 50, stop: false}]);
    assert!(player.advance(1198, || 0).is_empty());
    assert_eq!(player.advance(1, || 0), [Event {id: 50, stop: true}]);
    assert_eq!(player.advance(20000, || 0).len(), 1);
  }
  #[test]
  fn original_random_range_scaling_not_uniform_full_millisecond_range() {
    let mut noise = || 1023;
    assert_eq!(sample(&Period {milliseconds: 8000, random: true}, &mut noise), 7161);
    assert_eq!(sample(&Period {milliseconds: 1200, random: true}, &mut noise), 1023);
    assert_eq!(sample(&Period {milliseconds: 500, random: true}, &mut noise), 23);
    let mut timer = fixed(); timer.inactive.random = true;
    let mut player = Player::new(vec![timer], noise);
    assert!(player.advance(7160, noise).is_empty());
    assert_eq!(player.advance(1, noise), [Event {id: 50, stop: false}]);
    player.reset(noise);
    assert!(player.advance(7160, noise).is_empty());
  }
}
