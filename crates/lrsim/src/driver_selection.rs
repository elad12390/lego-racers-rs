//! RacerVisual0043ec10 selection with idle-neighbour search excluded.
//! Decisions preserve one-shot/hold chains; playback and audio are separate.
use crate::driver_motion::{self, Motion};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct Input {
  pub motion: Motion,
  pub current: u16,
  pub finished: bool,
  pub rank: u32,
  pub noise: u32,
  pub pending: u32,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Decision {
  pub clip: u16,
  pub blend_ms: u32,
  pub next: Option<u16>,
}

pub fn select(input: &Input) -> Option<Decision> {
  let start = |clip, next| {
    Some(Decision {
      clip,
      blend_ms: 0,
      next,
    })
  };
  let current = input.current;
  if input.finished {
    if input.rank != 1 {
      return if [11, 12].contains(&current) {
        None
      } else {
        start(11, Some(12))
      };
    }
    if [13, 14, 15].contains(&current) {
      return None;
    }
    return if input.noise & 1 == 0 {
      start(13, None)
    } else {
      start(14, Some(15))
    };
  }
  if input.pending & 1 != 0 {
    return start(1, Some(9));
  }
  if input.pending & 2 != 0 {
    return start(10, Some(9));
  }
  if [0, 1, 10, 13, 15, 12].contains(&current) {
    return None;
  }
  let reversing = input.motion.speed < 0.0 && input.motion.throttle < 0.0;
  if reversing {
    return if [2, 3].contains(&current) {
      None
    } else {
      start(2, Some(3))
    };
  }
  if current == 3 {
    return start(4, Some(9));
  }
  if [7, 16, 8, 17].contains(&current) {
    return None;
  }
  let clip = driver_motion::select(&input.motion).index();
  if clip == current {
    None
  } else {
    Some(Decision {
      clip,
      blend_ms: 300,
      next: None,
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  fn input(current: u16) -> Input {
    Input {
      motion: Motion {
        speed: 30.0,
        throttle: 1.0,
        steer: 0.0,
      },
      current,
      finished: false,
      rank: 0,
      noise: 0,
      pending: 0,
    }
  }
  #[test]
  fn finish_precedes_pending_reaction_and_normal_steering() {
    let mut i = input(5);
    i.finished = true;
    i.rank = 2;
    i.pending = 3;
    assert_eq!(
      select(&i),
      Some(Decision {
        clip: 11,
        blend_ms: 0,
        next: Some(12)
      })
    );
    i.current = 12;
    assert!(select(&i).is_none());
    i.rank = 1;
    i.current = 9;
    assert_eq!(select(&i).unwrap().clip, 13);
    i.noise = 1;
    assert_eq!(
      select(&i),
      Some(Decision {
        clip: 14,
        blend_ms: 0,
        next: Some(15)
      })
    );
    i.current = 14;
    assert!(select(&i).is_none());
  }
  #[test]
  fn pending_priority_and_reverse_hold_chain_protect_transient_clips() {
    let mut i = input(9);
    i.pending = 3;
    assert_eq!(select(&i).unwrap().clip, 1);
    i.pending = 0;
    i.current = 10;
    assert!(select(&i).is_none());
    i.current = 9;
    i.motion.speed = -10.0;
    i.motion.throttle = -1.0;
    assert_eq!(select(&i).unwrap().next, Some(3));
    i.current = 3;
    assert!(select(&i).is_none());
    i.motion.throttle = 1.0;
    assert_eq!(select(&i).unwrap().clip, 4);
  }
}
