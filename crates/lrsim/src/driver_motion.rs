//! Normal driver clip selection from0043ec10, excluding reaction/idle/finish paths.
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct Motion {
  pub speed: f32,
  pub throttle: f32,
  pub steer: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Clip {
  Default,
  SteerRight,
  SteerLeft,
  Reverse,
}

impl Clip {
  pub fn index(self) -> u16 {
    match self {
      Self::Default => 9,
      Self::SteerRight => 5,
      Self::SteerLeft => 6,
      Self::Reverse => 2,
    }
  }
  pub fn name(self) -> &'static str {
    match self {
      Self::Default => "default",
      Self::SteerRight => "steer-r",
      Self::SteerLeft => "steer-l",
      Self::Reverse => "reverse",
    }
  }
}

pub fn select(motion: &Motion) -> Clip {
  if motion.speed < 0.0 && motion.throttle < 0.0 {
    Clip::Reverse
  } else if motion.steer < 0.0 {
    Clip::SteerRight
  } else if motion.steer > 0.0 {
    Clip::SteerLeft
  } else {
    Clip::Default
  }
}
