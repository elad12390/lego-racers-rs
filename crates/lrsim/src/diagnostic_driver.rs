//! Test-only steering driver over an original recorded line, NOT opponent AI.
//! Feeds the same vehicle actions as keyboard control; never sets car positions.
use crate::{
  contact::{dot, length, sub},
  vehicle::{Actions, Vehicle},
};

pub struct DiagnosticDriver {
  pub points: Vec<[f32; 3]>,
  pub index: usize,
  pub distance_on_line: f32,
}

impl DiagnosticDriver {
  pub fn new(points: Vec<[f32; 3]>) -> Self {
    Self {
      points,
      index: 0,
      distance_on_line: 0.0,
    }
  }

  pub fn actions(&mut self, car: &Vehicle) -> Actions {
    if self.points.len() < 2 {
      return Actions::default();
    }
    // Only search forward locally: no shortcut, teleport, or global snap.
    let end = (self.index + 80).min(self.points.len() - 1);
    let nearest = (self.index..=end)
      .min_by(|a, b| {
        length(sub(self.points[*a], car.position))
          .total_cmp(&length(sub(self.points[*b], car.position)))
      })
      .unwrap();
    for i in self.index..nearest {
      self.distance_on_line += length(sub(self.points[i + 1], self.points[i]));
    }
    self.index = nearest;
    let lookahead = (car.speed().abs() * 0.45).clamp(15.0, 45.0);
    let mut ahead = nearest;
    let mut distance = 0.0;
    while ahead + 1 < self.points.len() && distance < lookahead {
      distance += length(sub(self.points[ahead + 1], self.points[ahead]));
      ahead += 1;
    }
    let target = sub(self.points[ahead], car.position);
    let forward = car.forward();
    let left = [-forward[1], forward[0], 0.0];
    let lateral = dot(target, left);
    let longitudinal = dot(target, forward);
    let range2 = target[0] * target[0] + target[1] * target[1];
    let curvature = if range2 > 0.01 {
      2.0 * lateral / range2
    } else {
      0.0
    };
    let steer = if curvature.abs() < 0.00025 {
      0.0
    } else {
      curvature.signum() * (curvature.abs() - 0.00025) / 0.02475 / car.handling.steering_scale
    };
    let turn = lateral.atan2(longitudinal).abs();
    let throttle = if turn > 1.1 && car.speed() > 25.0 {
      -0.2
    } else if turn > 0.6 {
      0.3
    } else {
      1.0
    };
    Actions {
      throttle,
      steer: steer.clamp(-1.0, 1.0),
    }
  }
}
