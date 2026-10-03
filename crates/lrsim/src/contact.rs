//! Swept contact queries over original BVB triangles, not invented boundaries.
//! Current car probes are an approximation; full original suspension remains open.
use crate::ground::Ground;
use lrformats::collision_materials::{blocks_chassis, supports_wheels, Surface};
use lrformats::world::CollisionMesh;

pub struct Contacts {
  pub ground: Ground,
  walls: Vec<[[f32; 3]; 3]>,
  surfaces: Vec<Surface>,
  primary: Option<(crate::chassis_dispatch::Collider, Vec<Surface>)>,
}

impl Contacts {
  pub fn new(mesh: CollisionMesh) -> Self {
    Self::with_surface_flags(mesh, &[])
  }

  pub fn with_surface_flags(mesh: CollisionMesh, flags: &[u32]) -> Self {
    Self::with_surfaces(
      mesh,
      &flags
        .iter()
        .map(|flags| Surface {
          flags: *flags,
          ..Default::default()
        })
        .collect::<Vec<_>>(),
    )
  }

  pub fn surface(&self, index: u32) -> Surface {
    self
      .surfaces
      .get(index as usize)
      .copied()
      .unwrap_or_default()
  }

  pub fn with_surfaces(mut mesh: CollisionMesh, surfaces: &[Surface]) -> Self {
    let walls = mesh
      .triangles
      .iter()
      .filter_map(|tri| {
        if !blocks_chassis(
          surfaces
            .get(tri.surface as usize)
            .copied()
            .unwrap_or_default()
            .flags,
        ) {
          return None;
        }
        let [a, b, c] = tri.indices.map(|i| mesh.vertices[i as usize]);
        let n = cross(sub(b, a), sub(c, a));
        (n[2].abs() < length(n) * 0.45).then_some([a, b, c])
      })
      .collect();
    mesh.triangles.retain(|tri| {
      supports_wheels(
        surfaces
          .get(tri.surface as usize)
          .copied()
          .unwrap_or_default()
          .flags,
      )
    });
    Self {
      ground: Ground::new(mesh),
      walls,
      surfaces: surfaces.to_vec(),
      primary: None,
    }
  }

  /// The primary query uses world coordinates directly in004478b0.
  /// Do not merge/filter/reorder its triangles or substitute nearest hits.
  pub fn set_primary(
    &mut self,
    tree: lrformats::collision_tree::CollisionTree,
    surfaces: Vec<Surface>,
  ) -> Result<(), String> {
    if tree.mesh.names.len() != surfaces.len() {
      return Err("primary surface count mismatch".into());
    }
    let tree = std::sync::Arc::new(tree);
    //00445dc0 retains the selected contact regardless of material20000.
    // The original surface callbacks own coefficients/effects afterward;
    // suppressing the query here bypasses support/cache and is incorrect.
    self.ground = Ground::with_tree(tree.clone(), Vec::new());
    self.surfaces = surfaces.clone();
    self.primary = Some((
      crate::chassis_dispatch::Collider {
        tree,
        transform: crate::collider_transform::ColliderTransform {
          origin: [0.0; 3],
          axes: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        },
      },
      surfaces,
    ));
    Ok(())
  }

  pub fn primary_selection(
    &self,
    starts: [[f32; 3]; 4],
    ends: [[f32; 3]; 4],
  ) -> Option<crate::chassis_dispatch::Selection> {
    self.primary.as_ref().map(|(collider, surfaces)| {
      crate::chassis_dispatch::dispatch(
        std::slice::from_ref(collider),
        starts,
        ends,
        true,
        |_, _, hit| blocks_chassis(surfaces[hit.surface as usize].flags),
      )
    })
  }

  pub fn primary_collider(&self) -> Option<&crate::chassis_dispatch::Collider> {
    self.primary.as_ref().map(|(collider, _)| collider)
  }

  pub fn sweep(&self, start: [f32; 3], end: [f32; 3]) -> Option<(f32, [f32; 3])> {
    if let Some(selection) = self.primary_selection([start; 4], [end; 4]) {
      return (selection.improvement_count > 0).then_some((selection.fraction, selection.normal));
    }
    let direction = sub(end, start);
    let mut closest: Option<(f32, [f32; 3])> = None;
    for &[a, b, c] in &self.walls {
      if (0..3).any(|i| {
        start[i].min(end[i]) > a[i].max(b[i]).max(c[i])
          || start[i].max(end[i]) < a[i].min(b[i]).min(c[i])
      }) {
        continue;
      }
      let e1 = sub(b, a);
      let e2 = sub(c, a);
      let p = cross(direction, e2);
      let det = dot(e1, p);
      if det.abs() < 1e-7 {
        continue;
      }
      let offset = sub(start, a);
      let u = dot(offset, p) / det;
      if !(0.0..=1.0).contains(&u) {
        continue;
      }
      let q = cross(offset, e1);
      let v = dot(direction, q) / det;
      if v < 0.0 || u + v > 1.0 {
        continue;
      }
      let fraction = dot(e2, q) / det;
      if !(0.0..=1.0).contains(&fraction) {
        continue;
      }
      if closest.map_or(true, |hit| fraction < hit.0) {
        let mut normal = normalized(cross(e1, e2));
        if dot(normal, direction) > 0.0 {
          normal = normal.map(|v| -v);
        }
        closest = Some((fraction, normal));
      }
    }
    closest
  }
}

pub fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
  [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
pub fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
  a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
pub fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
  let a = a.map(f64::from);
  let b = b.map(f64::from);
  [
    (a[1] * b[2] - a[2] * b[1]) as f32,
    (a[2] * b[0] - a[0] * b[2]) as f32,
    (a[0] * b[1] - a[1] * b[0]) as f32,
  ]
}
pub fn length(a: [f32; 3]) -> f32 {
  dot(a, a).sqrt()
}
pub fn normalized(a: [f32; 3]) -> [f32; 3] {
  //004492d0 keeps sqrt and reciprocal in53-bit precision through output
  // multiplication. Spilling either to binary32 moves grazing wheel rays.
  let length = a
    .into_iter()
    .map(|v| f64::from(v) * f64::from(v))
    .sum::<f64>()
    .sqrt();
  if length == 0.0 {
    return [0.0; 3];
  }
  let inverse = 1.0 / length;
  a.map(|v| (f64::from(v) * inverse) as f32)
}
