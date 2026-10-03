//! Spatial lap events from original EVB/TRB/TMB and WDB-placed BVB geometry.
use crate::contact::{cross, dot, normalized, sub};
use lrformats::{lap_events, library::Library, world};

#[derive(Clone)]
enum Zone {
  Sphere { center: [f32; 3], radius: f32 },
  Triangle([[f32; 3]; 3]),
}

#[derive(Clone)]
struct Binding {
  id: i32,
  mode: u32,
  zone: Zone,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct LapEvent {
  pub id: i32,
  pub mode: u32,
}

pub struct LapZones {
  bindings: Vec<Binding>,
}

impl LapZones {
  pub fn load(library: &Library, table: &str) -> Result<Self, String> {
    let read = |name: &str| {
      library
        .find_in(name, table)
        .ok_or_else(|| format!("{table}: missing {name}"))
    };
    let modes = lap_events::modes(read("EVENT.EVB")?).map_err(|e| e.to_string())?;
    let materials =
      lap_events::material_events(read(&format!("{table}.TMB"))?).map_err(|e| e.to_string())?;
    let mut bindings = Vec::new();
    let owner = library
      .jam()
      .tables
      .iter()
      .find(|t| t.name.eq_ignore_ascii_case(table))
      .ok_or("missing race table")?;
    for entry in &owner.entries {
      if !entry.name.to_ascii_uppercase().ends_with(".TRB") {
        continue;
      }
      for trigger in world::triggers(library.jam().bytes(entry).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?
      {
        if let Some(&mode) = modes.get(&trigger.id) {
          bindings.push(Binding {
            id: trigger.id,
            mode,
            zone: Zone::Sphere {
              center: trigger.position,
              radius: trigger.radius,
            },
          });
        }
      }
    }
    for instance in world::collision_instances(read("COLLIDE.WDB")?).map_err(|e| e.to_string())? {
      let mesh = world::collision_mesh(read(&format!("{}.BVB", instance.model))?)
        .map_err(|e| e.to_string())?;
      let forward = normalized(instance.forward);
      let up = normalized(instance.up);
      let left = normalized(cross(up, forward));
      let vertices: Vec<_> = mesh
        .vertices
        .iter()
        .map(|point| {
          std::array::from_fn::<_, 3, _>(|i| {
            instance.position[i] + forward[i] * point[0] + left[i] * point[1] + up[i] * point[2]
          })
        })
        .collect();
      for triangle in mesh.triangles {
        let name = mesh
          .names
          .get(triangle.surface as usize)
          .ok_or("invalid trigger surface")?;
        if let Some(&id) = materials.get(&name.to_ascii_lowercase()) {
          if let Some(&mode) = modes.get(&id) {
            bindings.push(Binding {
              id,
              mode,
              zone: Zone::Triangle(triangle.indices.map(|i| vertices[i as usize])),
            });
          }
        }
      }
    }
    if !bindings.iter().any(|b| b.mode == 1) {
      return Err(format!("{table}: no spatial finish event"));
    }
    Ok(Self { bindings })
  }

  pub fn crossings(&self, start: [f32; 3], end: [f32; 3]) -> Vec<LapEvent> {
    let mut hits: Vec<_> = self
      .bindings
      .iter()
      .filter_map(|binding| {
        let fraction = match binding.zone {
          Zone::Triangle(triangle) => segment_triangle(start, end, triangle),
          Zone::Sphere { center, radius } => sphere_entry(start, end, center, radius),
        }?;
        Some((fraction, binding.id, binding.mode))
      })
      .collect();
    hits.sort_by(|a, b| a.0.total_cmp(&b.0));
    // Triangulated gates may hit their shared edge twice. Dispatch once.
    hits.dedup_by(|a, b| a.1 == b.1 && (a.0 - b.0).abs() < 0.0001);
    hits
      .into_iter()
      .map(|(_, id, mode)| LapEvent { id, mode })
      .collect()
  }
}

fn sphere_entry(start: [f32; 3], end: [f32; 3], center: [f32; 3], radius: f32) -> Option<f32> {
  let relative = sub(start, center);
  let direction = sub(end, start);
  let a = dot(direction, direction);
  let c = dot(relative, relative) - radius * radius;
  if c <= 0.0 || a < 1e-12 {
    return None;
  }
  let b = dot(relative, direction);
  let discriminant = b * b - a * c;
  if discriminant < 0.0 {
    return None;
  }
  let fraction = (-b - discriminant.sqrt()) / a;
  (fraction > 0.0 && fraction <= 1.0).then_some(fraction)
}

pub(crate) fn segment_triangle(
  start: [f32; 3],
  end: [f32; 3],
  [a, b, c]: [[f32; 3]; 3],
) -> Option<f32> {
  let direction = sub(end, start);
  let e1 = sub(b, a);
  let e2 = sub(c, a);
  let p = cross(direction, e2);
  let det = dot(e1, p);
  if det.abs() < 1e-7 {
    return None;
  }
  let offset = sub(start, a);
  let u = dot(offset, p) / det;
  let q = cross(offset, e1);
  let v = dot(direction, q) / det;
  if !(-0.00001..=1.00001).contains(&u) || v < -0.00001 || u + v > 1.00001 {
    return None;
  }
  let fraction = dot(e2, q) / det;
  (fraction > 0.0 && fraction <= 1.0).then_some(fraction)
}
