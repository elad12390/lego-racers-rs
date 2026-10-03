//! Shared original race-data loading for native runtime and real headless tests.
use crate::contact::Contacts;
use lrformats::{
  cmb::{self, Chassis},
  library::Library,
  route::RouteRecord,
  world,
};

pub struct RaceData {
  pub contacts: Contacts,
  pub start: world::StartPosition,
  pub chassis: Chassis,
  pub diagnostic_line: Vec<[f32; 3]>,
  pub lap_zones: crate::lap_zones::LapZones,
  pub checkpoints: crate::checkpoint_contacts::CheckpointContacts,
}

impl RaceData {
  pub fn load(library: &Library, table_name: &str, chassis_name: &str) -> Result<Self, String> {
    let table = library
      .jam()
      .tables
      .iter()
      .find(|t| t.name.eq_ignore_ascii_case(table_name))
      .ok_or_else(|| format!("missing race {table_name}"))?;
    let entry = table
      .entries
      .iter()
      .find(|e| e.name.to_ascii_uppercase().ends_with(".SPB"))
      .ok_or("missing original start grid")?;
    let start = world::start_positions(library.jam().bytes(entry).map_err(|e| e.to_string())?)
      .map_err(|e| e.to_string())?
      .into_iter()
      .min_by_key(|s| s.slot)
      .ok_or("empty start grid")?;
    let mut merged = world::CollisionMesh::default();
    let checkpoints = crate::checkpoint_contacts::CheckpointContacts::load(library, table_name)?;
    let materials = lrformats::collision_materials::surfaces(
      library
        .find_in(&format!("{table_name}.TMB"), table_name)
        .ok_or("missing collision materials")?,
      false,
    )
    .map_err(|e| e.to_string())?;
    let binding = lrformats::race_archive::collisions(
      library
        .find_in(&format!("{table_name}.RAB"), table_name)
        .ok_or("missing race archive")?,
    )?;
    let placements = world::collision_instances(
      library
        .find_in(&binding.file, table_name)
        .ok_or("missing collision world")?,
    )
    .map_err(|e| e.to_string())?;
    //004478b0's PRIMARY branch passes world endpoints directly regardless
    // of the WDB transform (some shipped placements have nonzero offsets).
    // Only the secondary branch invokes the collider inverse transform.
    placements
      .iter()
      .find(|p| p.model.eq_ignore_ascii_case(&binding.primary))
      .ok_or("missing primary collision placement")?;
    let primary_tree = lrformats::collision_tree::parse(
      library
        .find_in(&format!("{}.BVB", binding.primary), table_name)
        .ok_or("missing primary BVB")?,
    )?;
    let primary_surfaces = primary_tree
      .mesh
      .names
      .iter()
      .map(|n| {
        materials
          .get(&n.to_ascii_lowercase())
          .copied()
          .unwrap_or_default()
      })
      .collect();
    for instance in placements {
      // Original CarBody::OnContact treats the selected checkpoint collider
      // as progress metadata, not a physical wall (digit-named surfaces).
      // Its route gates remain separately available for progress logic.
      if instance.model.eq_ignore_ascii_case(&checkpoints.collider) {
        continue;
      }
      let mesh = world::collision_mesh(
        library
          .find_in(&format!("{}.BVB", instance.model), table_name)
          .ok_or("missing collision mesh")?,
      )
      .map_err(|e| e.to_string())?;
      let base = merged.vertices.len() as u32;
      let surface_base = merged.names.len() as u32;
      let forward = crate::contact::normalized(instance.forward);
      let up = crate::contact::normalized(instance.up);
      let left = crate::contact::normalized(crate::contact::cross(up, forward));
      merged.names.extend(mesh.names);
      merged
        .vertices
        .extend(mesh.vertices.into_iter().map(|point| {
          std::array::from_fn::<_, 3, _>(|i| {
            instance.position[i] + forward[i] * point[0] + left[i] * point[1] + up[i] * point[2]
          })
        }));
      merged.triangles.extend(
        mesh
          .triangles
          .into_iter()
          .map(|t| world::CollisionTriangle {
            indices: t.indices.map(|i| i + base),
            surface: t.surface + surface_base,
          }),
      );
    }
    if merged.triangles.is_empty() {
      return Err("no original collision geometry".into());
    }
    let chassis = cmb::parse(
      library
        .find_in("CHASSIS.CMB", "COMMON")
        .ok_or("missing chassis database")?,
    )
    .map_err(|e| e.to_string())?
    .into_iter()
    .find(|c| c.name.eq_ignore_ascii_case(chassis_name))
    .ok_or_else(|| format!("missing chassis {chassis_name}"))?;
    let route = RouteRecord::load(
      library
        .find_in("R1_F_0.RRB", table_name)
        .ok_or("missing original diagnostic driving line")?,
      false,
    )
    .map_err(|e| e.to_string())?;
    let lap_zones = crate::lap_zones::LapZones::load(library, table_name)?;
    let surfaces: Vec<_> = merged
      .names
      .iter()
      .map(|n| {
        materials
          .get(&n.to_ascii_lowercase())
          .copied()
          .unwrap_or_default()
      })
      .collect();
    let mut contacts = Contacts::with_surfaces(merged, &surfaces);
    contacts.set_primary(primary_tree, primary_surfaces)?;
    Ok(Self {
      contacts,
      start,
      chassis,
      diagnostic_line: route.absolute_positions(),
      lap_zones,
      checkpoints,
    })
  }
}
