use lrformats::cmb::Chassis;
use lrformats::world::Vec3;

use crate::ground::Ground;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Controls {
  /// -1 (reverse/brake) ..= 1 (full throttle).
  pub throttle: f32,
  /// -1 (right) ..= 1 (left), counter-clockwise positive like the map view.
  pub steer: f32,
}

/// Tuning for the arcade model. These are derived from chassis data but the mapping is an
/// approximation: the original's exact handling code has not been reverse engineered yet.
#[derive(Debug, Clone, PartialEq)]
pub struct Tuning {
  pub max_speed: f32,
  pub accel: f32,
  pub brake: f32,
  pub drag: f32,
  pub turn_rate: f32,
  pub grip: f32,
  pub ride_height: f32,
}

impl Tuning {
  pub fn from_chassis(chassis: &Chassis) -> Self {
    let speed = f32::from(chassis.rating_a.max(1)) / 50.0;
    let accel = f32::from(chassis.rating_b.max(1)) / 80.0;
    let handling = f32::from(chassis.rating_c.max(1)) / 50.0;
    Tuning {
      max_speed: 90.0 * speed.clamp(0.6, 1.6),
      accel: 60.0 * accel.clamp(0.6, 1.6),
      brake: 140.0,
      drag: 0.35,
      turn_rate: 1.9 * handling.clamp(0.6, 1.5),
      grip: chassis.grip.clamp(0.3, 0.99),
      ride_height: 0.0,
    }
  }
}

impl Default for Tuning {
  fn default() -> Self {
    Tuning {
      max_speed: 90.0,
      accel: 60.0,
      brake: 140.0,
      drag: 0.35,
      turn_rate: 1.9,
      grip: 0.9,
      ride_height: 0.0,
    }
  }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Car {
  pub position: Vec3,
  /// Heading in radians, counter-clockwise from +X.
  pub heading: f32,
  pub velocity: [f32; 2],
  pub tuning: Tuning,
  /// Surface under the car after the last step, if any.
  pub surface: Option<u32>,
}

impl Car {
  pub fn new(position: Vec3, forward: Vec3, tuning: Tuning) -> Self {
    Car {
      position,
      heading: forward[1].atan2(forward[0]),
      velocity: [0.0; 2],
      tuning,
      surface: None,
    }
  }

  pub fn forward(&self) -> [f32; 2] {
    [self.heading.cos(), self.heading.sin()]
  }

  pub fn speed(&self) -> f32 {
    let f = self.forward();
    self.velocity[0] * f[0] + self.velocity[1] * f[1]
  }

  /// Advances the car by `dt` seconds on `ground`. Leaving the mesh stops the car at the edge.
  pub fn step(&mut self, controls: Controls, ground: &Ground, dt: f32) {
    let t = &self.tuning;
    let f = self.forward();
    let side = [-f[1], f[0]];
    let mut forward_speed = self.speed();
    let mut lateral_speed = self.velocity[0] * side[0] + self.velocity[1] * side[1];

    let throttle = controls.throttle.clamp(-1.0, 1.0);
    if throttle > 0.0 {
      forward_speed +=
        t.accel * throttle * dt * (1.0 - (forward_speed / t.max_speed).clamp(0.0, 1.0));
    } else if throttle < 0.0 {
      let braking = forward_speed > 1.0;
      forward_speed += if braking { t.brake } else { t.accel * 0.5 } * throttle * dt;
      forward_speed = forward_speed.max(-t.max_speed * 0.3);
    }
    forward_speed -= forward_speed * t.drag * dt;
    // Tires pull sideways motion back to zero.
    lateral_speed *= (1.0 - t.grip).powf(dt * 6.0);

    let steer_authority = (forward_speed.abs() / 20.0).clamp(0.0, 1.0);
    self.heading +=
      controls.steer.clamp(-1.0, 1.0) * t.turn_rate * steer_authority * dt * forward_speed.signum();

    let f = self.forward();
    let side = [-f[1], f[0]];
    self.velocity = [
      f[0] * forward_speed + side[0] * lateral_speed,
      f[1] * forward_speed + side[1] * lateral_speed,
    ];

    let next = [
      self.position[0] + self.velocity[0] * dt,
      self.position[1] + self.velocity[1] * dt,
    ];
    match ground.at(next[0], next[1], self.position[2] + 6.0) {
      Some(hit) => {
        self.position = [next[0], next[1], hit.height + t.ride_height];
        self.surface = Some(hit.surface);
      }
      None => {
        self.velocity = [0.0; 2];
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use lrformats::world::{CollisionMesh, CollisionTriangle};

  fn plane(size: f32) -> Ground {
    Ground::new(CollisionMesh {
      names: vec![],
      vertices: vec![
        [-size, -size, 0.0],
        [size, -size, 0.0],
        [-size, size, 0.0],
        [size, size, 0.0],
      ],
      triangles: vec![
        CollisionTriangle {
          indices: [0, 1, 2],
          surface: 1,
        },
        CollisionTriangle {
          indices: [1, 3, 2],
          surface: 1,
        },
      ],
    })
  }

  fn run(car: &mut Car, controls: Controls, ground: &Ground, seconds: f32) {
    for _ in 0..(seconds * 60.0) as usize {
      car.step(controls, ground, 1.0 / 60.0);
    }
  }

  #[test]
  fn accelerates_toward_but_never_past_top_speed() {
    let ground = plane(5000.0);
    let mut car = Car::new([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], Tuning::default());
    run(
      &mut car,
      Controls {
        throttle: 1.0,
        steer: 0.0,
      },
      &ground,
      3.0,
    );
    let speed = car.speed();
    assert!(
      speed > 20.0 && speed <= car.tuning.max_speed,
      "speed {speed}"
    );
    assert!(car.position[0] > 20.0 && car.position[1].abs() < 1e-3);
  }

  #[test]
  fn steering_turns_a_moving_car_but_not_a_parked_one() {
    let ground = plane(5000.0);
    let mut parked = Car::new([0.0; 3], [1.0, 0.0, 0.0], Tuning::default());
    run(
      &mut parked,
      Controls {
        throttle: 0.0,
        steer: 1.0,
      },
      &ground,
      1.0,
    );
    assert!(parked.heading.abs() < 1e-6);
    let mut moving = Car::new([0.0; 3], [1.0, 0.0, 0.0], Tuning::default());
    run(
      &mut moving,
      Controls {
        throttle: 1.0,
        steer: 1.0,
      },
      &ground,
      2.0,
    );
    assert!(moving.heading > 0.5, "heading {}", moving.heading);
  }

  #[test]
  fn braking_brings_a_fast_car_to_rest() {
    let ground = plane(5000.0);
    let mut car = Car::new([0.0; 3], [1.0, 0.0, 0.0], Tuning::default());
    run(
      &mut car,
      Controls {
        throttle: 1.0,
        steer: 0.0,
      },
      &ground,
      4.0,
    );
    assert!(car.speed() > 30.0);
    run(
      &mut car,
      Controls {
        throttle: -1.0,
        steer: 0.0,
      },
      &ground,
      1.0,
    );
    assert!(car.speed() < 5.0, "speed after braking {}", car.speed());
  }

  #[test]
  fn the_car_cannot_leave_the_ground_mesh() {
    let ground = plane(50.0);
    let mut car = Car::new([0.0; 3], [1.0, 0.0, 0.0], Tuning::default());
    run(
      &mut car,
      Controls {
        throttle: 1.0,
        steer: 0.0,
      },
      &ground,
      10.0,
    );
    // The seam tolerance lets the car sit a hair past the geometric edge.
    assert!(
      car.position[0] <= 50.1,
      "left the ground: {:?}",
      car.position
    );
    assert!(
      car.speed() < 2.0,
      "pinned against the edge, speed {}",
      car.speed()
    );
  }

  #[test]
  fn follows_the_ground_height() {
    let ground = Ground::new(CollisionMesh {
      names: vec![],
      vertices: vec![
        [0.0, -10.0, 0.0],
        [100.0, -10.0, 10.0],
        [0.0, 10.0, 0.0],
        [100.0, 10.0, 10.0],
      ],
      triangles: vec![
        CollisionTriangle {
          indices: [0, 1, 2],
          surface: 1,
        },
        CollisionTriangle {
          indices: [1, 3, 2],
          surface: 1,
        },
      ],
    });
    let mut car = Car::new([1.0, 0.0, 0.1], [1.0, 0.0, 0.0], Tuning::default());
    run(
      &mut car,
      Controls {
        throttle: 1.0,
        steer: 0.0,
      },
      &ground,
      1.5,
    );
    let expected = car.position[0] * 0.1;
    assert!(
      (car.position[2] - expected).abs() < 0.05,
      "z {} vs {}",
      car.position[2],
      expected
    );
  }
}
