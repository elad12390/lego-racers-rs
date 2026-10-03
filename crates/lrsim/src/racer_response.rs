//! Original ordinary free-racer/unowned-body contact response (00438560).
//! Owned-racer effects, on-route displacement and locked body branches remain
//! separate. The caller must collision-check the proposed displacement.
use crate::contact::{cross, dot, normalized, sub};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct Body {
  pub position: [f32; 3],
  pub velocity: [f32; 3],
  pub inverse_mass: f32,
  pub inverse_inertia_world: [f32; 9],
}
#[derive(Debug, Deserialize)]
pub struct Case {
  pub a: Body,
  pub b: Body,
  pub point: [f32; 3],
  pub normal: [f32; 3],
  pub penetration: f32,
}
#[derive(Serialize)]
pub struct Response {
  pub position: [f32; 3],
  pub velocity: [f32; 3],
}

fn matrix_vector(m: [f32; 9], v: [f32; 3]) -> [f32; 3] {
  std::array::from_fn(|i| (0..3).map(|j| m[i + 3 * j] * v[j]).sum())
}

pub fn unowned_contact(c: &Case) -> Response {
  let position = std::array::from_fn(|i| c.a.position[i] + c.normal[i] * c.penetration);
  let impulse = impulse(c);
  Response {
    position,
    velocity: std::array::from_fn(|i| c.a.velocity[i] + c.normal[i] * impulse * c.a.inverse_mass),
  }
}

/// Scalar has velocity*mass units; native callers use world units/second.
/// Divide by1000before passing to an original on-route speed response.
pub fn impulse(c: &Case) -> f32 {
  // TryDisplace changes both transforms but does not resync the stored COM
  // before HandleEvent computes its normalized contact directions.
  let ra = normalized(sub(c.a.position, c.point));
  let rb = normalized(sub(c.b.position, c.point));
  let angular = |r| {
    cross(
      matrix_vector(c.a.inverse_inertia_world, cross(r, c.normal)),
      r,
    )
  };
  // Original uses receiver inertia for both normalized contact-direction
  // terms. Keep this observed branch, not a generic rigid-body solver.
  let x = angular(ra);
  let y = angular(rb);
  let denominator = dot(c.normal, std::array::from_fn(|i| x[i] + y[i]))
    + (c.a.inverse_mass + c.b.inverse_mass) * dot(c.normal, c.normal);
  let impulse = -1.75 * dot(c.normal, sub(c.a.velocity, c.b.velocity)) / denominator;
  assert!(
    impulse.is_finite(),
    "nonfinite racer impulse: denominator={denominator}, ra={ra:?}, rb={rb:?}, case={c:?}"
  );
  impulse
}
