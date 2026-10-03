//! Original checkpoint surface binding and contact-side state.
//! Full original checkpoint plane-tree dispatch; whole-world ordering remains open.
use crate::contact::{cross, dot, normalized, sub};
use lrformats::{
  checkpoints::CheckpointTable,
  collision_tree::{self, CollisionTree},
  library::Library,
  race_archive, world,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct State {
  pub checkpoint: Option<usize>,
  pub side: u32,
  pub flags: u32,
  pub contact_count: i32,
  pub lap_history: [u32; 3],
}
impl Default for State {
  fn default() -> Self {
    Self {
      checkpoint: None,
      side: 1,
      flags: 2,
      contact_count: -1,
      lap_history: [0, 2, 1],
    }
  }
}
impl State {
  /// Complete00439fc0 semantics, including same-record/same-side no-op.
  pub fn touch(
    &mut self,
    index: usize,
    progress: f32,
    checkpoint_normal: [f32; 3],
    hit_normal: [f32; 3],
  ) {
    let side = u32::from(
      checkpoint_normal
        .into_iter()
        .zip(hit_normal)
        .map(|(n, h)| f64::from(n) * f64::from(h))
        .sum::<f64>()
        >= 0.0,
    );
    if self.checkpoint == Some(index) && self.side == side {
      return;
    }
    if progress == 0.0 {
      if side == 0 {
        self.side = 0;
        self.flags |= 0x80000;
        self.lap_history = [0, 2, 2];
        self.checkpoint = Some(index);
        return;
      }
      if self.flags & 0x80000 == 0 {
        self.contact_count = self.contact_count.wrapping_add(1);
      }
    } else if self.flags & 0x80000 != 0 {
      self.contact_count = self.contact_count.wrapping_sub(1);
    }
    self.flags &= !0x80000;
    self.checkpoint = Some(index);
    self.side = side;
  }

  pub fn standing(&self, position: [f32; 3], previous_rank: u32) -> crate::standings::Racer {
    crate::standings::Racer {
      checkpoint: self.checkpoint,
      contact_count: self.contact_count,
      position,
      flags: self.flags,
      previous_rank,
    }
  }
}

struct Gate {
  checkpoint: usize,
  triangle: [[f32; 3]; 3],
  normal: [f32; 3],
}
pub struct CheckpointContacts {
  pub table: CheckpointTable,
  gates: Vec<Gate>,
  pub collider: String,
  tree: std::sync::Arc<CollisionTree>,
  surfaces: Vec<Option<usize>>,
  ordinary_surfaces: Vec<lrformats::collision_materials::Surface>,
  pub transform: crate::collider_transform::ColliderTransform,
}

#[derive(Serialize)]
pub struct ProbeFixture {
  pub checkpoint: usize,
  pub triangle: [[f32; 3]; 3],
  pub opposite: [[f32; 3]; 3],
  pub start: [f32; 3],
  pub end: [f32; 3],
  pub plane: [f32; 4],
  pub progress: f32,
  pub state: State,
}

impl CheckpointContacts {
  pub fn load(library: &Library, owner: &str) -> Result<Self, String> {
    let read = |name: &str| {
      library
        .find_in(name, owner)
        .ok_or_else(|| format!("{owner}: missing {name}"))
    };
    let binding = race_archive::checkpoints(read(&format!("{owner}.RAB"))?)?;
    let table = CheckpointTable::load(read(&binding.file)?, false).map_err(|e| e.to_string())?;
    let placements = world::collision_instances(read("COLLIDE.WDB")?).map_err(|e| e.to_string())?;
    let mut matches = placements
      .iter()
      .filter(|p| p.model.eq_ignore_ascii_case(&binding.collider));
    let placement = matches.next().ok_or("checkpoint collider not placed")?;
    if matches.next().is_some() {
      return Err("ambiguous checkpoint collider placement".into());
    }
    let tree = collision_tree::parse(read(&format!("{}.BVB", binding.collider))?)?;
    let mesh = &tree.mesh;
    let materials = lrformats::collision_materials::surfaces(read(&format!("{owner}.TMB"))?, false)
      .map_err(|e| e.to_string())?;
    let ordinary_surfaces = mesh
      .names
      .iter()
      .map(|n| {
        materials
          .get(&n.to_ascii_lowercase())
          .copied()
          .unwrap_or_default()
      })
      .collect();
    // Game_BindNamedTextures00435e70 assigns CPB records in digit-prefixed
    // surface-table order; the numeric value spelled in the name is ignored.
    let mut next = 0;
    let surfaces: Vec<_> = mesh
      .names
      .iter()
      .map(|name| {
        if name.as_bytes().first().is_some_and(u8::is_ascii_digit) {
          let index = next;
          next += 1;
          Some(index)
        } else {
          None
        }
      })
      .collect();
    if next != table.records.len() {
      return Err(format!(
        "checkpoint surface/record count mismatch: {next}/{}",
        table.records.len()
      ));
    }
    let forward = normalized(placement.forward);
    let up = normalized(placement.up);
    let left = normalized(cross(up, forward));
    let vertices: Vec<_> = mesh
      .vertices
      .iter()
      .map(|p| {
        std::array::from_fn::<_, 3, _>(|i| {
          placement.position[i] + forward[i] * p[0] + left[i] * p[1] + up[i] * p[2]
        })
      })
      .collect();
    let mut gates = Vec::new();
    for triangle in &mesh.triangles {
      if let Some(checkpoint) = surfaces.get(triangle.surface as usize).copied().flatten() {
        let triangle = triangle.indices.map(|i| vertices[i as usize]);
        let normal = normalized(cross(
          sub(triangle[1], triangle[0]),
          sub(triangle[2], triangle[0]),
        ));
        gates.push(Gate {
          checkpoint,
          triangle,
          normal,
        });
      }
    }
    if gates.is_empty() {
      return Err("no finite checkpoint triangles".into());
    }
    Ok(Self {
      table,
      gates,
      collider: binding.collider,
      tree: std::sync::Arc::new(tree),
      surfaces,
      ordinary_surfaces,
      transform: crate::collider_transform::ColliderTransform {
        origin: placement.position,
        axes: [forward, left, up],
      },
    })
  }

  pub fn ordinary_surface(&self, index: u32) -> lrformats::collision_materials::Surface {
    self.ordinary_surfaces[index as usize]
  }

  pub fn wheel_surface(&self, index: u32) -> lrformats::collision_materials::Surface {
    if let Some(record) = self.surfaces[index as usize] {
      //0042aad0 reads descriptor+8 even for a CPB record. Every shipped
      // CPB plane.z is signed zero: no coefficient/effect flags, hence
      // ordinary defaults. Geometry is not perfectly vertical; do not
      // assume these descriptors can never be reached by wheel queries.
      let flags = self.table.records[record].plane[2].to_bits();
      assert_eq!(flags & 0x7fffffff, 0, "unresolved CPB wheel-effect flags");
      lrformats::collision_materials::Surface {
        flags,
        ..Default::default()
      }
    } else {
      self.ordinary_surface(index)
    }
  }

  pub fn query_collider(&self) -> crate::chassis_dispatch::Collider {
    crate::chassis_dispatch::Collider {
      tree: self.tree.clone(),
      transform: self.transform,
    }
  }

  /// The digit descriptor on this owner invokes00439fc0and returns false.
  /// A non-digit surface falls through to ordinary material/effect handling.
  pub fn touch_hit(&self, state: &mut State, hit: &crate::mesh_query::Hit) -> bool {
    let Some(index) = self.surfaces.get(hit.surface as usize).copied().flatten() else {
      return false;
    };
    let checkpoint = &self.table.records[index];
    state.touch(
      index,
      checkpoint.progress,
      [
        checkpoint.plane[0],
        checkpoint.plane[1],
        checkpoint.plane[2],
      ],
      hit.normal,
    );
    true
  }

  pub fn advance_probes(&self, state: &mut State, start: [[f32; 3]; 4], end: [[f32; 3]; 4]) -> u32 {
    let mut count = 0;
    for (start, end) in start.into_iter().zip(end) {
      // Each original TraceSegment returns ONE selected face in BSP order.
      // Do not sort all finite intersections or dispatch both windings.
      let Some(hit) = crate::mesh_query::trace(
        &self.tree,
        self.transform.inverse_point(start),
        self.transform.inverse_point(end),
      ) else {
        continue;
      };
      //0042a830 passes the query's LOCAL triangle to00439fc0. The
      // separate world-normal response in UpdateCollisions does not
      // overwrite that callback record.
      count += u32::from(self.touch_hit(state, &hit));
    }
    count
  }

  /// Actual shipped geometry for complete original query/callback tests.
  /// Only this finite paired face is installed into the original query fixture;
  /// this does not establish full BSP/world collider ordering.
  pub fn probe_fixtures(&self) -> Result<Vec<ProbeFixture>, String> {
    let mut fixtures = Vec::new();
    for index in 0..self.table.records.len() {
      let gate = self
        .gates
        .iter()
        .find(|g| g.checkpoint == index)
        .ok_or("missing checkpoint face")?;
      let center =
        std::array::from_fn::<_, 3, _>(|i| gate.triangle.iter().map(|v| v[i]).sum::<f32>() / 3.0);
      let front = std::array::from_fn(|i| center[i] + gate.normal[i] * 0.1);
      let back = std::array::from_fn(|i| center[i] - gate.normal[i] * 0.1);
      let opposite = self
        .gates
        .iter()
        .find(|g| {
          g.checkpoint == index
            && dot(g.normal, gate.normal) < -0.99
            && crate::lap_zones::segment_triangle(front, back, g.triangle).is_some()
        })
        .ok_or_else(|| {
          format!(
            "{}: checkpoint{} missing opposite finite face",
            self.collider, index
          )
        })?;
      for direction in [-1.0, 1.0] {
        let start = std::array::from_fn(|i| center[i] + gate.normal[i] * direction);
        let end = std::array::from_fn(|i| center[i] - gate.normal[i] * direction);
        let mut state = State::default();
        self.advance_probes(&mut state, [start; 4], [end; 4]);
        let checkpoint = &self.table.records[index];
        fixtures.push(ProbeFixture {
          checkpoint: index,
          triangle: gate.triangle,
          opposite: opposite.triangle,
          start,
          end,
          plane: checkpoint.plane,
          progress: checkpoint.progress,
          state,
        });
      }
    }
    Ok(fixtures)
  }
}

pub fn probes(position: [f32; 3], basis: [f32; 9], gear_range: [f32; 2]) -> [[f32; 3]; 4] {
  let x = gear_range[1] * 0.5;
  let y = gear_range[0] * 0.5;
  [[x, -y, 3.5], [x, y, 3.5], [-x, -y, 3.5], [-x, y, 3.5]].map(|p| {
    std::array::from_fn(|i| position[i] + (0..3).map(|j| basis[i + 3 * j] * p[j]).sum::<f32>())
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn all_thirteen_original_checkpoint_colliders_bind_every_cpb_record() {
    let library = Library::open(
      std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../extracted/Program_Files_Group/LEGO.JAM"),
    )
    .unwrap();
    let mut count = 0;
    for owner in library
      .jam()
      .tables
      .iter()
      .filter(|t| t.name.starts_with("RACEC"))
    {
      let checkpoints = CheckpointContacts::load(&library, &owner.name).unwrap();
      for (index, record) in checkpoints.surfaces.iter().enumerate() {
        if record.is_some() {
          let surface = checkpoints.wheel_surface(index as u32);
          assert_eq!(surface.flags & 0x7fffffff, 0);
          assert_eq!(surface.slope_friction, 0.5);
          assert_eq!(surface.quadratic_drag, 0.0);
          assert_eq!(surface.force, [0.0; 3]);
        }
      }
      assert!(checkpoints.table.records.len() > 2);
      for index in 0..checkpoints.table.records.len() {
        assert!(
          checkpoints
            .gates
            .iter()
            .any(|gate| gate.checkpoint == index),
          "{} record{} has no finite gate",
          owner.name,
          index
        );
      }
      count += 1;
    }
    assert_eq!(count, 13);
  }
}
