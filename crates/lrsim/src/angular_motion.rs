//! Original ordinary rigid-body momentum/inertia coupling in millisecond units.
//! Physical pitch/roll impulses retain their independent alignment locks.
use crate::{
  attitude::{self, Basis},
  collider_transform::ColliderTransform,
  contact::{cross, normalized, sub},
};

#[derive(Clone)]
pub struct AngularMotion {
  momentum: [f32; 3],
  inertia: [f32; 3],
  inverse: [f32; 3],
  yaw_hold_ms: f32,
  roll_hold_ms: f32,
  pitch_hold_ms: f32,
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f64 {
  f64::from(a[0]) * f64::from(b[0])
    + f64::from(a[1]) * f64::from(b[1])
    + f64::from(a[2]) * f64::from(b[2])
}

impl AngularMotion {
  pub fn new(mass: f32) -> Self {
    let extents = [8.0f32, 5.0, 6.2];
    let squares = extents.map(|v| v * v);
    let inertia = std::array::from_fn(|i| {
      let other = (0..3)
        .filter(|j| *j != i)
        .map(|j| f64::from(squares[j]))
        .sum::<f64>();
      (other * (f64::from(mass) / f64::from(extents[i])) * f64::from(0.083333336f32)) as f32
    });
    Self {
      momentum: [0.0; 3],
      inertia,
      inverse: inertia.map(|v| 1.0 / v),
      yaw_hold_ms: 0.0,
      roll_hold_ms: 0.0,
      pitch_hold_ms: 0.0,
    }
  }

  fn remove(&mut self, axis: [f32; 3], positive_only: bool) {
    let projection = dot(self.momentum, axis);
    if !positive_only || projection >= 0.0 {
      for i in 0..3 {
        self.momentum[i] = (f64::from(self.momentum[i]) - f64::from(axis[i]) * projection) as f32;
      }
    }
  }

  pub fn set_yaw(&mut self, basis: Basis, yaw_rate_ms: f32) {
    self.remove(basis.up, false);
    self.yaw_hold_ms = 200.0;
    self.add_angular_velocity(basis, basis.up, yaw_rate_ms);
  }

  fn add_angular_velocity(&mut self, basis: Basis, axis: [f32; 3], rate_ms: f32) {
    let transform = ColliderTransform {
      origin: [0.0; 3],
      axes: [basis.forward, basis.left, basis.up],
    };
    let impulse = axis.map(|v| v * rate_ms);
    let body = transform.inverse_vector(impulse);
    let world = transform.rotate(std::array::from_fn(|i| body[i] * self.inertia[i]));
    for i in 0..3 {
      self.momentum[i] += world[i];
    }
  }

  ///00445bb0: an active forward-axis impulse blocks this left-axis impulse.
  pub fn apply_pitch(&mut self, basis: Basis, rate_ms: f32, hold_ms: u32) -> bool {
    if self.roll_hold_ms != 0.0 {
      return false;
    }
    self.pitch_hold_ms = hold_ms as f32;
    self.remove(basis.left, false);
    self.add_angular_velocity(basis, basis.left, rate_ms);
    true
  }

  ///00445b30: the opposing left-axis lock blocks a forward-axis impulse.
  pub fn apply_roll(&mut self, basis: Basis, rate_ms: f32, hold_ms: u32) -> bool {
    if self.pitch_hold_ms != 0.0 {
      return false;
    }
    self.roll_hold_ms = hold_ms as f32;
    self.remove(basis.forward, false);
    self.add_angular_velocity(basis, basis.forward, rate_ms);
    true
  }

  pub fn alignment_locked(&self) -> bool {
    self.roll_hold_ms != 0.0 || self.pitch_hold_ms != 0.0
  }

  pub fn momentum(&self) -> [f32; 3] { self.momentum }
  pub fn pitch_hold_ms(&self) -> f32 { self.pitch_hold_ms }

  pub fn advance(
    &mut self,
    basis: Basis,
    center: [f32; 3],
    points: [[f32; 3]; 4],
    supported: [bool; 4],
    wheels: u32,
    normal: [f32; 3],
    gravity_force: f32,
    yaw_command: Option<f32>,
    elapsed_ms: f32,
  ) -> Basis {
    //00444ef0 expires the PREVIOUS hold before UpdateForces can issue a new
    // command. Straight steering does not itself cancel an unexpired hold.
    if self.yaw_hold_ms <= elapsed_ms {
      self.yaw_hold_ms = 0.0;
      self.remove(basis.up, false);
    } else {
      self.yaw_hold_ms -= elapsed_ms;
    }
    for (hold, axis) in [(&mut self.roll_hold_ms, basis.forward), (&mut self.pitch_hold_ms, basis.left)] {
      if *hold > 0.0 {
        if *hold <= elapsed_ms {
          *hold = 0.0;
          let projection = dot(self.momentum, axis);
          self.momentum = std::array::from_fn(|i| (f64::from(self.momentum[i]) - f64::from(axis[i]) * projection) as f32);
        } else {
          *hold -= elapsed_ms;
        }
      }
    }
    let mut torque = [0.0f32; 3];
    //00445500 skips contact torque/cancellation while either impulse lock is
    // active. Cancelling against grounded wheels here erases turbo's pitch.
    if wheels > 0 && !self.alignment_locked() {
      for i in 0..4 {
        if supported[i] {
          if wheels < 3 {
            let support_force = [
              0.0,
              0.0,
              (f64::from(-gravity_force) / f64::from(wheels + 8)) as f32,
            ];
            let lever = sub(points[i], center);
            let contribution = cross(lever, support_force);
            for j in 0..3 {
              torque[j] += contribution[j];
            }
          }
          self.remove(normalized(cross(sub(points[3 - i], center), normal)), true);
        }
      }
    }
    let axes = [basis.forward, basis.left, basis.up];
    //00445c30 cancels up-axis momentum, then00440c10converts the requested
    // angular velocity through the BODY inertia before rotating it back.
    if let Some(yaw_rate_ms) = yaw_command {
      self.set_yaw(basis, yaw_rate_ms);
    }
    //00410f30 spills B*inverse_body before multiplying by transpose(B).
    let intermediate: [[f32; 3]; 3] =
      std::array::from_fn(|axis| axes[axis].map(|v| v * self.inverse[axis]));
    let orders = [
      [[1, 2, 0], [1, 2, 0], [2, 1, 0]],
      [[1, 2, 0], [2, 0, 1], [0, 2, 1]],
      [[2, 1, 0], [0, 2, 1], [1, 2, 0]],
    ];
    let matrix: [[f32; 3]; 3] = std::array::from_fn(|row| {
      std::array::from_fn(|col| {
        orders[row][col]
          .into_iter()
          .map(|axis| f64::from(intermediate[axis][row]) * f64::from(axes[axis][col]))
          .sum::<f64>() as f32
      })
    });
    //00440a80 evaluates (Z + Y) + X for every world angular component.
    //00449190writes row-major products, while00440a80reads columns.
    // Do not exploit mathematical tensor symmetry: original off-diagonal
    // entries can differ by one binary32 bit after multiplication spills.
    let velocity = std::array::from_fn(|row| {
      let product = |i: usize| f64::from(matrix[i][row]) * f64::from(self.momentum[i]);
      ((product(2) + product(1)) + product(0)) as f32
    });
    let step = velocity.map(|v| v * elapsed_ms);
    let result = attitude::integrate_step(basis, step);
    for i in 0..3 {
      self.momentum[i] =
        (f64::from(torque[i]) * f64::from(elapsed_ms) + f64::from(self.momentum[i])) as f32;
    }
    if torque == [0.0; 3] && dot(step, step) < f64::from(0.0006f32) {
      self.momentum = [0.0; 3];
    }
    result
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  fn basis() -> Basis { Basis { forward: [1.0,0.0,0.0], left: [0.0,1.0,0.0], up: [0.0,0.0,1.0] } }
  #[test]
  fn turbo_pitch_has_no_linear_impulse_and_its_lock_expires_on_exact_boundary() {
    let mut angular = AngularMotion::new(1.0);
    assert!(angular.apply_pitch(basis(), -0.0025, 125));
    assert!(angular.alignment_locked());
    assert!(angular.momentum()[1] < 0.0);
    angular.advance(basis(), [0.0;3], [[0.0;3];4], [false;4], 0, [0.0,0.0,1.0], 0.0, None, 124.0);
    assert_eq!(angular.pitch_hold_ms(), 1.0);
    angular.advance(basis(), [0.0;3], [[0.0;3];4], [false;4], 0, [0.0,0.0,1.0], 0.0, None, 1.0);
    assert!(!angular.alignment_locked());
    assert_eq!(angular.momentum(), [0.0;3]);
  }
  #[test]
  fn opposing_roll_lock_blocks_pitch_without_replacing_its_timer() {
    let mut angular = AngularMotion::new(1.0);
    angular.roll_hold_ms = 20.0;
    assert!(!angular.apply_pitch(basis(), -0.0025, 250));
    assert_eq!(angular.pitch_hold_ms(), 0.0);
    assert_eq!(angular.momentum(), [0.0;3]);
  }
}
