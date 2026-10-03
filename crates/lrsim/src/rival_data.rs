//! Original race/circuit roster and numbered recorded-run bindings.
use lrformats::{appearance, cmb, library::Library, race_catalog, route::RouteRecord};

pub struct RivalData {
  pub name: String,
  pub route_name: String,
  pub record: RouteRecord,
  pub car_model: String,
  pub driver_model: String,
  pub chassis: cmb::Chassis,
  pub mass: f32,
}

/// Matches App_LoadRouteFiles00433190's contiguous F/M/Svariant enumeration.
/// The randomness source is supplied by the caller; recorded routes are not
/// replaced with diagnostic player actions or synthesized waypoint geometry.
pub fn load(library: &Library, table: &str, samples: [u32; 5]) -> Result<Vec<RivalData>, String> {
  load_selected(library, table, None, samples)
}

pub fn load_selected(
  library: &Library,
  table: &str,
  race_name: Option<&str>,
  samples: [u32; 5],
) -> Result<Vec<RivalData>, String> {
  let read = |name: &str, owner: &str| {
    library
      .find_in(name, owner)
      .ok_or_else(|| format!("missing original{owner}/{name}"))
  };
  let races = race_catalog::races(read("LEGORACE.RCB", "MENUDATA")?)?;
  let mut matches = races.iter().filter(|r| {
    r.table.eq_ignore_ascii_case(table)
      && race_name.map_or(!r.mirrored, |name| r.name.eq_ignore_ascii_case(name))
  });
  let race = matches
    .next()
    .ok_or_else(|| format!("missing nonmirrored catalog race for{table}"))?;
  if matches.next().is_some() {
    return Err(format!("ambiguous catalog race for{table}"));
  }
  let circuits = race_catalog::circuits(read("LEGORACE.CRB", "MENUDATA")?)?;
  let circuit = circuits
    .iter()
    .find(|c| Some(&c.name) == race.circuit.as_ref())
    .ok_or("missing race circuit")?;
  let cars = appearance::cars(read("CHAMPS.CCB", "COMMON")?)?;
  let drivers = appearance::drivers(read("DRIVERS.DDB", "COMMON")?)?;
  let chassis = cmb::parse(read("CHASSIS.CMB", "COMMON")?).map_err(|e| e.to_string())?;
  circuit.racers[1..]
    .iter()
    .zip(samples)
    .enumerate()
    .map(|(index, (name, sample))| {
      let slot = index + 1;
      let mut variants = Vec::new();
      for class in ["F", "M", "S"] {
        for variant in 0..10 {
          let name = format!("R{slot}_{class}_{variant}.RRB");
          if library.find_in(&name, table).is_none() {
            break;
          }
          variants.push(name);
        }
      }
      if variants.is_empty() {
        return Err(format!("no original route for rival slot{slot}in{table}"));
      }
      let route_name = variants[sample as usize % variants.len()].clone();
      let record =
        RouteRecord::load(read(&route_name, table)?, false).map_err(|e| e.to_string())?;
      let driver = drivers
        .iter()
        .find(|d| d.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| format!("missing original driver{name}"))?;
      let car = cars
        .iter()
        .find(|c| c.name.eq_ignore_ascii_case(&driver.car))
        .ok_or_else(|| format!("missing original car{}", driver.car))?;
      let chassis = chassis
        .iter()
        .find(|c| c.name.eq_ignore_ascii_case(&car.chassis))
        .ok_or_else(|| format!("missing original chassis{}", car.chassis))?
        .clone();
      Ok(RivalData {
        name: name.clone(),
        route_name,
        record,
        car_model: car.models[0].clone(),
        driver_model: driver.models[0].clone(),
        chassis,
        mass: car.mass,
      })
    })
    .collect()
}
