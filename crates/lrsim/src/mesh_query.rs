//! Original full plane-tree segment order00403fa0, independent of world dispatch.
use crate::contact::{cross, sub};
use lrformats::collision_tree::{CollisionTree, PlaneNode};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Hit {
  pub point: [f32; 3],
  pub normal: [f32; 3],
  pub plane_offset: f32,
  pub fraction: f32,
  pub triangle: usize,
  pub surface: u32,
}

fn precise_dot(a: [f32; 3], b: [f32; 3]) -> f64 {
  a.into_iter()
    .zip(b)
    .map(|(x, y)| f64::from(x) * f64::from(y))
    .sum()
}

///004497f0 uses the node normal to choose projection, not triangle normal.
pub fn contains(point: [f32; 3], vertices: [[f32; 3]; 3], normal: [f32; 3]) -> bool {
  let n = normal.map(f32::abs);
  let axes = if n[0] <= n[1] {
    if n[1] < n[2] {
      [0, 1]
    } else {
      [0, 2]
    }
  } else if n[0] < n[2] {
    [0, 1]
  } else {
    [1, 2]
  };
  let [x, y] = axes;
  let [[b, a], [l, k], [j, c]] = vertices.map(|p| [p[x], p[y]]);
  let e = a - k;
  let f = l - b;
  let g = k - c;
  let h = j - l;
  let last_x = c - a;
  let last_y = b - j;
  //0044993d..004499af spills one difference with FSTP, but keeps the
  // other difference after FST while forming each binary32 edge bound.
  let bound =
    (f64::from(last_x) * f64::from(j) + (f64::from(b) - f64::from(j)) * f64::from(c)) as f32;
  let ab = (f64::from(e) * f64::from(b) + (f64::from(l) - f64::from(b)) * f64::from(a)) as f32;
  let bc = (f64::from(g) * f64::from(l) + (f64::from(j) - f64::from(l)) * f64::from(k)) as f32;
  // Point-edge comparisons reload those spilled differences; their product
  // sums stay at53-bit precision until FCOM, without another binary32 spill.
  let values = [
    f64::from(last_x) * f64::from(point[x]) + f64::from(last_y) * f64::from(point[y]),
    f64::from(e) * f64::from(point[x]) + f64::from(f) * f64::from(point[y]),
    f64::from(g) * f64::from(point[x]) + f64::from(h) * f64::from(point[y]),
  ];
  if f64::from(h) * f64::from(e) <= f64::from(g) * f64::from(f) {
    values
      .into_iter()
      .zip([bound, ab, bc])
      .all(|(v, b)| v <= f64::from(b))
  } else {
    values
      .into_iter()
      .zip([bound, ab, bc])
      .all(|(v, b)| v >= f64::from(b))
  }
}

fn crossing(
  tree: &CollisionTree,
  node: &PlaneNode,
  start: [f32; 3],
  end: [f32; 3],
  from: f32,
  to: f32,
) -> Option<Hit> {
  let fraction = (f64::from(from) / (f64::from(from) + f64::from(to))) as f32;
  let delta = sub(end, start);
  let point = std::array::from_fn(|i| start[i] + delta[i] * fraction);
  let mut fallback = None;
  for index in usize::from(node.first)..usize::from(node.first) + usize::from(node.count) {
    let triangle = &tree.mesh.triangles[index];
    let vertices = triangle.indices.map(|i| tree.mesh.vertices[i as usize]);
    if !contains(point, vertices, node.normal) {
      continue;
    }
    let normal = cross(sub(vertices[2], vertices[1]), sub(vertices[0], vertices[1]));
    // Native math does not assume the node's normal matches winding.
    let length = precise_dot(normal, normal).sqrt();
    if length == 0.0 {
      continue;
    }
    let unit = normal.map(|v| (f64::from(v) / length) as f32);
    let p = |i: usize| f64::from(unit[i]) * f64::from(vertices[0][i]);
    let hit = Hit {
      point,
      fraction,
      triangle: index,
      surface: triangle.surface,
      normal: unit,
      plane_offset: -((p(2) + p(1)) + p(0)) as f32,
    };
    if precise_dot(normal, delta) <= 0.0 {
      return Some(hit);
    }
    fallback = Some(hit);
  }
  fallback
}

/// First start-side subtree, node crossing/ordered faces, then end-side subtree.
/// It is not a nearest-triangle sort and preserves opposite-face fallback.
pub fn trace(tree: &CollisionTree, start: [f32; 3], end: [f32; 3]) -> Option<Hit> {
  trace_observed(tree, start, end, |_, _, _| {})
}

/// Passive traversal observation for original comparisons; never changes selection.
pub fn trace_observed(
  tree: &CollisionTree,
  start: [f32; 3],
  end: [f32; 3],
  mut observe: impl FnMut(usize, bool, f64),
) -> Option<Hit> {
  let mut stack = vec![(0usize, false, 0.0f32)];
  while let Some((index, phase, stored)) = stack.pop() {
    let node = &tree.planes[index];
    let reference =
      tree.mesh.vertices[tree.mesh.triangles[usize::from(node.first)].indices[0] as usize];
    if !phase {
      let distance = precise_dot(node.normal, sub(start, reference));
      observe(index, false, distance);
      stack.push((index, true, distance as f32));
      let child = node.children[usize::from(distance <= 0.0)];
      if child < 0xfffe {
        stack.push((usize::from(child), false, 0.0));
      }
      continue;
    }
    let distance = precise_dot(node.normal, sub(end, reference));
    observe(index, true, distance);
    let positive = distance > 0.0;
    if positive == (stored > 0.0) {
      continue;
    }
    let (from, to) = if positive {
      (-stored, distance as f32)
    } else {
      (stored, (-distance) as f32)
    };
    if let Some(hit) = crossing(tree, node, start, end, from, to) {
      return Some(hit);
    }
    let child = node.children[usize::from(!positive)];
    if child < 0xfffe {
      stack.push((usize::from(child), false, 0.0));
    }
  }
  None
}

#[cfg(test)]
mod tests {
  use super::*;
  use lrformats::{collision_tree, library::Library};
  #[test]
  fn real_shortcut_sliver_and_shared_wall_edge_select_original_first_face() {
    let library = Library::open(
      std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../extracted/Program_Files_Group/LEGO.JAM"),
    )
    .unwrap();
    let shortcut =
      collision_tree::parse(library.find_in("SHTCTCOL.BVB", "RACEC0R2").unwrap()).unwrap();
    let hit = trace(
      &shortcut,
      [-48.15554, 20.437181, 29.962942],
      [51.844345, 20.589558, 29.96035],
    )
    .unwrap();
    assert_eq!(hit.triangle, 12);
    assert!(hit.normal[0] > 0.99);
    assert!((hit.fraction - 0.5).abs() < 0.000001);
    let wall = collision_tree::parse(library.find_in("COLLIDE.BVB", "RACEC2R2").unwrap()).unwrap();
    let hit = trace(
      &wall,
      [-373.33334, 546.5, -47.768616],
      [-373.33334, 446.5, -47.768616],
    )
    .unwrap();
    assert_eq!(hit.triangle, 341);
    assert_eq!(hit.normal, [0.0, -1.0, 0.0]);
    assert!((hit.fraction - 0.04).abs() < 0.000001);
  }
}
