//! Racer00437740sets absolute frames/ms=1.8*abs(forward speed in units/ms).
//! This overrides ADB FPS, and starts the other clip at0when direction changes.
use serde::Serialize;

#[derive(Default, Serialize)]
pub struct WheelPlayback {
  pub reversed: bool,
  pub time: f32,
}

impl WheelPlayback {
  pub fn advance(&mut self, speed: f32, dt: f32, forward_period: u16, reverse_period: u16) {
    if !speed.is_finite() || !dt.is_finite() || dt < 0.0 {
      return;
    }
    let moving = if speed.abs() < 0.1 { 0.0 } else { speed };
    let reversed = moving < 0.0;
    if self.reversed != reversed {
      self.time = 0.0;
      self.reversed = reversed;
    }
    let period = f32::from(if reversed {
      reverse_period
    } else {
      forward_period
    });
    if period > 0.0 {
      self.time = (self.time + moving.abs() * 1.8 * dt).rem_euclid(period);
    }
  }

  pub fn clip(&self) -> &'static str {
    if self.reversed {
      "rvrse"
    } else {
      "frwrd"
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn forward_reverse_and_stopped_motion_use_original_rate_and_reset() {
    let mut playback = WheelPlayback::default();
    playback.advance(120.0, 0.1, 35, 31);
    assert!((playback.time - 21.6).abs() < 0.00001);
    playback.advance(120.0, 0.1, 35, 31);
    assert!((playback.time - 8.2).abs() < 0.00001);
    playback.advance(-30.0, 0.1, 35, 31);
    assert_eq!(playback.clip(), "rvrse");
    assert!((playback.time - 5.4).abs() < 0.00001);
    playback.advance(-0.05, 0.1, 35, 31);
    assert_eq!(playback.clip(), "frwrd");
    assert_eq!(playback.time, 0.0);
    playback.advance(0.05, 1.0, 35, 31);
    assert_eq!(playback.time, 0.0);
  }
}
