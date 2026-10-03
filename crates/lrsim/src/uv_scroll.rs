//! Original low-16-bit UV phase and signed rate quantization, not float drift.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy)]
pub struct Scroll { rate: [i32; 2] }
impl Scroll {
  pub fn new(rate: [f32; 2]) -> Self {
    Self {rate: rate.map(|v| (f64::from(v) * f64::from(65.536f32)).trunc() as i32)}
  }
  pub fn sample(&self, elapsed_ms: u32) -> [f32; 2] {
    self.rate.map(|rate| {
      f32::from((rate as u32).wrapping_mul(elapsed_ms) as u16) / 65536.0
    })
  }
}

/// Rendering-frame seconds accumulate into integer ticks once, not float UVs.
pub struct Clock { scroll: Scroll, elapsed: u32, fractional_ms: f64 }
impl Clock {
  pub fn new(rate: [f32; 2]) -> Self { Self {scroll: Scroll::new(rate), elapsed: 0, fractional_ms: 0.0} }
  pub fn reset(&mut self) { self.elapsed = 0; self.fractional_ms = 0.0; }
  pub fn advance(&mut self, seconds: f32) -> [f32; 2] {
    if seconds.is_finite() && seconds > 0.0 {
      self.fractional_ms += f64::from(seconds)*1000.0;
      let ticks = self.fractional_ms.floor() as u32;
      self.fractional_ms -= f64::from(ticks);
      self.elapsed = self.elapsed.wrapping_add(ticks);
    }
    self.scroll.sample(self.elapsed)
  }
}
#[derive(Deserialize)]
pub struct Case {pub rate: [f32; 2], pub ticks: Vec<u32>}
#[derive(Serialize)]
pub struct Sample {pub offset: [f32; 2]}
pub fn sequence(case: &Case) -> Vec<Sample> {
  let scroll = Scroll::new(case.rate);
  let mut elapsed = 0u32;
  case.ticks.iter().map(|ticks| { elapsed = elapsed.wrapping_add(*ticks);
    Sample {offset: scroll.sample(elapsed)} }).collect()
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn uv_scroll_quantizes_and_wraps_positive_and_negative_original_rates() {
    let scroll = Scroll::new([0.3,-4.0]);
    assert_eq!(scroll.sample(0),[0.0,0.0]);
    assert_eq!(scroll.sample(1000),[19000.0/65536.0,144.0/65536.0]);
    assert_eq!(scroll.sample(65536),[0.0,0.0]);
    assert_eq!(scroll.sample(65537),scroll.sample(1));
  }
  #[test]
  fn uv_scroll_clock_retains_fractional_milliseconds_and_resets_without_drift() {
    let mut clock = Clock::new([0.3,4.0]);
    let mut last = [0.0;2];
    for _ in 0..30 { last = clock.advance(1.0/30.0); }
    assert_eq!(last,Scroll::new([0.3,4.0]).sample(1000));
    assert_eq!(clock.advance(f32::NAN),last);
    assert_eq!(clock.advance(-1.0),last);
    clock.reset();
    assert_eq!(clock.advance(0.0),[0.0,0.0]);
  }
}
