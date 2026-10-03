//! Wheel geometry, contact band and alignment from00445dc0/00448430/00448c70.
//! Spatial queries use native original-triangle Ground, not a point-center snap.
use crate::{
  contact::{cross, normalized, sub},
  ground::{Ground, GroundHit},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
pub struct SupportCase {
  pub position: [f32; 3],
  pub forward: [f32; 3],
  pub left: [f32; 3],
  pub up: [f32; 3],
  pub base: [f32; 3],
  pub width: f32,
  pub length: f32,
  pub radius: f32,
  pub downward_movement: f32,
  pub grounded: bool,
  pub dt: f32,
}

#[derive(Debug, Serialize)]
pub struct Support {
  pub wheels: u32,
  pub position: [f32; 3],
  pub forward: [f32; 3],
  pub left: [f32; 3],
  pub up: [f32; 3],
  pub contact_normal: [f32; 3],
  pub supported: [bool; 4],
  pub surfaces: [Option<u32>; 4],
  pub colliders: [Option<usize>; 4],
}

pub fn anchors(base: [f32; 3], width: f32, length: f32) -> [[f32; 3]; 4] {
  let mut points = [base; 4];
  points[0][1] -= width;
  points[3][0] -= length;
  points[2][0] -= length;
  points[2][1] -= width;
  points
}

///00448c70 transforms wheel1 once, then derives0/3/2 by subtraction.
/// Transforming four equivalent local points independently changes rounding at
/// the grazing ray boundary (notably the original5ms support extension).
pub fn suspension_points(case: &SupportCase) -> [[f32; 3]; 4] {
  let mut points = [[0.0; 3]; 4];
  for i in 0..3 {
    let mut value = case.forward[i] * case.base[0];
    value = (f64::from(case.left[i]) * f64::from(case.base[1]) + f64::from(value)) as f32;
    value = (f64::from(case.up[i]) * f64::from(case.base[2]) + f64::from(value)) as f32;
    points[1][i] = value + case.position[i];
    let length = f64::from(case.forward[i]) * f64::from(case.length);
    let width = f64::from(case.left[i]) * f64::from(case.width);
    //00448c97..00448d7a retains X offsets until subtraction, but spills
    // Y/Z offsets to binary32. Both diagonal X subtractions reuse the
    // same retained product; do not independently re-round the offset.
    let (length, width) = if i == 0 {
      (length, width)
    } else {
      (f64::from(length as f32), f64::from(width as f32))
    };
    points[0][i] = (f64::from(points[1][i]) - width) as f32;
    points[3][i] = (f64::from(points[1][i]) - length) as f32;
    points[2][i] = (f64::from(points[0][i]) - length) as f32;
  }
  points
}

pub fn solve(case: &SupportCase, ground: &Ground) -> Support {
  solve_cached(case, ground, &mut ContactCache::default())
}

/// Original00448a50/00448a70/00448ae0: eight persistent triangle contacts,
/// oldest replacement and one preferred contact reference per wheel.
#[derive(Clone, Default)]
pub struct ContactCache {
  entries: Vec<(GroundHit, u32)>,
  wheels: [Option<usize>; 4],
}

impl ContactCache {
  fn trace(
    &mut self,
    wheel: usize,
    ground: &Ground,
    x: f32,
    y: f32,
    start: f32,
    end: f32,
  ) -> Option<GroundHit> {
    let mut order = Vec::new();
    if let Some(index) = self.wheels[wheel] {
      order.push(index);
    }
    order.extend((0..self.entries.len()).filter(|index| Some(*index) != self.wheels[wheel]));
    for index in order {
      if let Some(hit) = ground.trace_cached_hit(self.entries[index].0, x, y, start, end) {
        self.entries[index].1 = 0;
        self.wheels[wheel] = Some(index);
        return Some(hit);
      }
    }
    self.wheels[wheel] = None;
    let hit = ground.trace_vertical(x, y, start, end)?;
    let index = if self.entries.len() < 8 {
      self.entries.push((hit, 0));
      self.entries.len() - 1
    } else {
      // Strict greater-than scan keeps the first slot on age ties.
      let mut oldest = 0;
      for i in 1..8 {
        if self.entries[i].1 > self.entries[oldest].1 {
          oldest = i;
        }
      }
      self.entries[oldest] = (hit, 0);
      oldest
    };
    self.wheels[wheel] = Some(index);
    Some(hit)
  }
}

pub fn solve_cached(case: &SupportCase, ground: &Ground, cache: &mut ContactCache) -> Support {
  solve_world(case, ground, cache, &[])
}

pub fn solve_world(
  case: &SupportCase,
  ground: &Ground,
  cache: &mut ContactCache,
  secondary: &[crate::chassis_dispatch::Collider],
) -> Support {
  solve_world_locked(case, ground, cache, secondary, false)
}

pub fn solve_world_locked(
  case: &SupportCase,
  ground: &Ground,
  cache: &mut ContactCache,
  secondary: &[crate::chassis_dispatch::Collider],
  alignment_locked: bool,
) -> Support {
  for (_, age) in &mut cache.entries {
    *age = age.wrapping_add(1);
  }
  let points = suspension_points(case);
  let extension = if case.grounded { case.dt * 40.0 } else { 0.0 };
  let mut secondary_contacts = crate::wheel_query::trace(case, secondary);
  let contacts = std::array::from_fn::<_, 4, _>(|wheel| {
    if let Some(contact) = secondary_contacts[wheel].take() {
      return Some(contact);
    }
    let p = points[wheel];
    cache
      .trace(
        wheel,
        ground,
        p[0],
        p[1],
        p[2] + (case.radius + case.downward_movement.max(0.0)),
        p[2] - extension,
      )
      .map(|hit| crate::wheel_query::Contact {
        point: [p[0], p[1], hit.height],
        normal: hit.normal,
        surface: hit.surface,
        collider: 0,
        depth: (f64::from(hit.height) - f64::from(p[2] - extension)).powi(2),
      })
  });
  let mut result = Support {
    wheels: 0,
    position: case.position,
    forward: case.forward,
    left: case.left,
    up: case.up,
    contact_normal: [0.0; 3],
    supported: [false; 4],
    surfaces: [None; 4],
    colliders: [None; 4],
  };
  let depths: [f64; 4] =
    std::array::from_fn(|i| contacts[i].as_ref().map_or(f64::NEG_INFINITY, |h| h.depth));
  let penetration = depths.map(|v| v as f32);
  // The original replaces its selected wheel only for strictly greater depth.
  let deepest = (0..4)
    .filter(|i| contacts[*i].is_some())
    .fold(None, |best, i| {
      if best.is_none_or(|j| depths[i] > f64::from(penetration[j])) {
        Some(i)
      } else {
        best
      }
    });
  let Some(deepest) = deepest else {
    return result;
  };
  let threshold = (penetration[deepest].sqrt() - 0.4).max(0.0).powi(2);
  let supported =
    std::array::from_fn::<_, 4, _>(|i| contacts[i].is_some() && penetration[i] >= threshold);
  let contact_points =
    std::array::from_fn::<_, 4, _>(|i| contacts[i].as_ref().map_or(points[i], |h| h.point));
  result.wheels = supported.iter().filter(|s| **s).count() as u32;
  result.supported = supported;
  result.surfaces = std::array::from_fn(|i| contacts[i].as_ref().map(|hit| hit.surface));
  result.colliders = std::array::from_fn(|i| contacts[i].as_ref().map(|hit| hit.collider));
  let alignment_index = if result.wheels == 4 && !alignment_locked { 0 } else { deepest };
  if result.wheels > 1 && !alignment_locked {
    let along = [2, 3, 0, 1][alignment_index];
    let across = [1, 0, 3, 2][alignment_index];
    let forward = if supported[along] {
      sub(
        contact_points[alignment_index.min(along)],
        contact_points[alignment_index.max(along)],
      )
    } else {
      case.forward
    };
    let left = if supported[across] {
      sub(
        contact_points[alignment_index.max(across)],
        contact_points[alignment_index.min(across)],
      )
    } else {
      case.left
    };
    result.forward = normalized(forward);
    result.left = crate::attitude::perpendicular(result.forward, left);
    result.up = cross(result.forward, result.left);
  }
  for i in 0..3 {
    //004485ce adds the suspension offset to the hit-minus-anchor delta
    // BEFORE translating the body. Adding0.2after translation accumulates
    // a different height at grazing cached-plane crossings.
    // The hit-minus-anchor difference stays in the FPU register through
    // adding the binary32 bias. Spill the COMPLETE offset before body add.
    let offset = (f64::from(contact_points[alignment_index][i])
      - f64::from(points[alignment_index][i])
      + if i == 2 { f64::from(0.2f32) } else { 0.0 }) as f32;
    result.position[i] += offset;
  }
  result.contact_normal = if result.wheels >= 3 {
    result.wheels = 4;
    result.supported = [true; 4];
    result.up
  } else {
    let mut sum = [0.0; 3];
    for i in 0..4 {
      if supported[i] {
        for j in 0..3 {
          sum[j] += contacts[i].as_ref().unwrap().normal[j];
        }
      }
    }
    normalized(sum)
  };
  result
}

#[cfg(test)]
mod tests {
  use super::*;
  use lrformats::world::{CollisionMesh, CollisionTriangle};

  #[test]
  fn wheels_span_a_center_hole_without_inventing_ground_in_the_hole() {
    let mut mesh = CollisionMesh::default();
    for [x, y] in [[1.0, -1.0], [1.0, 1.0], [-1.0, -1.0], [-1.0, 1.0]] {
      let base = mesh.vertices.len() as u32;
      mesh.vertices.extend([
        [x - 0.1, y - 0.1, 0.0],
        [x + 0.2, y - 0.1, 0.0],
        [x - 0.1, y + 0.2, 0.0],
      ]);
      mesh.triangles.push(CollisionTriangle {
        indices: [base, base + 1, base + 2],
        surface: 0,
      });
    }
    let ground = Ground::new(mesh);
    assert!(ground.all_at(0.0, 0.0).is_empty());
    let mut case = SupportCase {
      position: [0.0, 0.0, 0.1],
      forward: [1.0, 0.0, 0.0],
      left: [0.0, 1.0, 0.0],
      up: [0.0, 0.0, 1.0],
      base: [1.0, 1.0, 0.0],
      width: 2.0,
      length: 2.0,
      radius: 3.0,
      downward_movement: 0.0,
      grounded: true,
      dt: 0.01,
    };
    let support = solve(&case, &ground);
    assert_eq!(support.wheels, 4);
    assert!((support.position[2] - 0.2).abs() < 0.00001);
    case.position[0] = 10.0;
    assert_eq!(solve(&case, &ground).wheels, 0);
  }
}
