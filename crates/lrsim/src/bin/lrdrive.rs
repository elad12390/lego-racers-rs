//! Headless autopilot: drives one race with the real data and draws the path it took.
//! usage: lrdrive <LEGO.JAM> <RACE_TABLE> <out.png> [chassis-name]

use std::fs;
use std::process::ExitCode;

use lrformats::checkpoints::CheckpointTable;
use lrformats::cmb;
use lrformats::library::Library;
use lrformats::model::Model;
use lrformats::render::{render_with, Camera, Projector};
use lrformats::world;
use lrsim::car::{Car, Controls, Tuning};
use lrsim::ground::Ground;

const DT: f32 = 1.0 / 60.0;
const REACH_RADIUS: f32 = 35.0;

fn main() -> ExitCode {
  let args: Vec<String> = std::env::args().skip(1).collect();
  let [jam, table, out, rest @ ..] = args.as_slice() else {
    eprintln!("usage: lrdrive <LEGO.JAM> <RACE_TABLE> <out.png> [chassis]");
    return ExitCode::FAILURE;
  };
  match run(jam, table, out, rest.first().map(String::as_str)) {
    Ok(()) => ExitCode::SUCCESS,
    Err(message) => {
      eprintln!("{message}");
      ExitCode::FAILURE
    }
  }
}

fn run(jam: &str, table: &str, out: &str, chassis_name: Option<&str>) -> Result<(), String> {
  let library = Library::open(jam).map_err(|e| format!("{jam}: {e}"))?;
  let in_table = |ext: &str| -> Option<&[u8]> {
    let t = library
      .jam()
      .tables
      .iter()
      .find(|t| t.name.eq_ignore_ascii_case(table))?;
    let entry = t
      .entries
      .iter()
      .find(|e| e.name.to_ascii_uppercase().ends_with(ext))?;
    library.jam().bytes(entry).ok()
  };
  // Collision files are named differently per race (COLLIDE, IGCOLLID, CM, SHTCTCOL...). Every
  // .BVB except the checkpoint and start/finish trigger volumes is drivable geometry.
  let ground_mesh = {
    let t = library
      .jam()
      .tables
      .iter()
      .find(|t| t.name.eq_ignore_ascii_case(table))
      .ok_or("unknown table")?;
    let mut merged = world::CollisionMesh::default();
    for entry in &t.entries {
      let name = entry.name.to_ascii_uppercase();
      let is_trigger =
        name.starts_with("CHCKPT") || name.starts_with("STARTFIN") || name.starts_with("STRTFIN");
      if !name.ends_with(".BVB") || is_trigger {
        continue;
      }
      let mesh = world::collision_mesh(library.jam().bytes(entry).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
      let base = merged.vertices.len() as u32;
      merged.vertices.extend(mesh.vertices);
      merged.triangles.extend(
        mesh
          .triangles
          .into_iter()
          .map(|t| world::CollisionTriangle {
            indices: t.indices.map(|i| i + base),
            surface: t.surface,
          }),
      );
    }
    if merged.triangles.is_empty() {
      return Err("no collision geometry in this race".into());
    }
    merged
  };
  let ground = Ground::new(ground_mesh);
  let starts =
    world::start_positions(in_table(".SPB").ok_or("no start grid")?).map_err(|e| e.to_string())?;
  let gate_table = CheckpointTable::load(in_table(".CPB").ok_or("no checkpoints")?, false)
    .map_err(|e| e.to_string())?;
  // The lap is the main route: record 0 following `next[0]` until it wraps back to record 0.
  let route = gate_table.main_route();
  println!(
    "lap = {} gates on the main route ({} records, forks excluded)",
    route.len(),
    gate_table.records.len()
  );
  let checkpoints: Vec<_> = route.iter().map(|&i| gate_table.records[i]).collect();
  let track =
    world::track_model(in_table(".WDB").ok_or("no world file")?).ok_or("no track model")?;

  let chassis_list = cmb::parse(library.find("CHASSIS.CMB", None).ok_or("no CHASSIS.CMB")?)
    .map_err(|e| e.to_string())?;
  let chassis = match chassis_name {
    Some(name) => chassis_list
      .iter()
      .find(|c| c.name.eq_ignore_ascii_case(name))
      .ok_or("unknown chassis")?,
    None => chassis_list.first().ok_or("empty chassis table")?,
  };
  let tuning = Tuning::from_chassis(chassis);
  println!("chassis {} -> {:?}", chassis.name, tuning);

  let start = starts
    .iter()
    .min_by_key(|s| s.slot)
    .ok_or("no start positions")?;
  let mut car = Car::new(start.position, start.forward, tuning);
  // Nudge the spawn onto the ground mesh.
  if let Some(hit) = ground.at(car.position[0], car.position[1], car.position[2] + 50.0) {
    car.position[2] = hit.height;
  }

  let mut path = vec![car.position];
  let (mut next, mut stuck_for, mut time) = (0usize, 0.0f32, 0.0f32);
  let mut lap_time = None;
  while time < 300.0 {
    let target = checkpoints[next].center;
    let to = [target[0] - car.position[0], target[1] - car.position[1]];
    if (to[0] * to[0] + to[1] * to[1]).sqrt() < REACH_RADIUS {
      next += 1;
      if next == checkpoints.len() {
        lap_time = Some(time);
        break;
      }
      continue;
    }
    let mut error = to[1].atan2(to[0]) - car.heading;
    while error > std::f32::consts::PI {
      error -= std::f32::consts::TAU;
    }
    while error < -std::f32::consts::PI {
      error += std::f32::consts::TAU;
    }
    let controls = Controls {
      steer: (error * 2.0).clamp(-1.0, 1.0),
      throttle: if error.abs() > 0.8 { 0.3 } else { 1.0 },
    };
    car.step(controls, &ground, DT);
    time += DT;
    if time as usize % 1 == 0 && path.len() < 100_000 && (time * 60.0) as usize % 6 == 0 {
      path.push(car.position);
    }
    stuck_for = if car.speed().abs() < 1.0 {
      stuck_for + DT
    } else {
      0.0
    };
    if stuck_for > 3.0 {
      println!(
        "STUCK at {:?} heading checkpoint {next}/{}",
        car.position,
        checkpoints.len()
      );
      let f = car.forward();
      println!(
        "  car z {:.2} heading {:.2} speed {:.2}",
        car.position[2],
        car.heading,
        car.speed()
      );
      for d in [0.0f32, 1.0, 3.0, 6.0, 12.0] {
        let (x, y) = (car.position[0] + f[0] * d, car.position[1] + f[1] * d);
        let hits: Vec<String> = ground
          .all_at(x, y)
          .iter()
          .map(|h| format!("z{:.1}(n.z {:.2},s{})", h.height, h.normal[2], h.surface))
          .collect();
        println!(
          "  ahead {d:4.1}: {}",
          if hits.is_empty() {
            "NO GEOMETRY".into()
          } else {
            hits.join(" ")
          }
        );
      }
      for n in next.saturating_sub(1)..(next + 3).min(checkpoints.len()) {
        let c = &checkpoints[n];
        println!(
          "  checkpoint {n}: center {:?} plane {:?} next {:?}",
          c.center, c.plane, c.next
        );
      }
      break;
    }
  }
  match lap_time {
    Some(t) => println!(
      "lap complete: all {} checkpoints in {t:.1}s",
      checkpoints.len()
    ),
    None => println!(
      "did not finish: reached checkpoint {next}/{} after {time:.1}s",
      checkpoints.len()
    ),
  }

  let model = Model::load(&library, &track, Some(table)).map_err(|e| e.to_string())?;
  let size = 1000;
  let projector = Projector::fit(
    &model,
    &Camera {
      azimuth_deg: 0.0,
      elevation_deg: 90.0,
    },
    size,
  );
  let mut frame = render_with(&model, &projector, size);
  for pixel in frame.rgba.chunks_exact_mut(4) {
    for c in &mut pixel[..3] {
      *c = (u16::from(*c) * 6 / 10) as u8;
    }
  }
  let at = |p: [f32; 3]| {
    let s = projector.project(p);
    [s[0], s[1]]
  };
  for pair in path.windows(2) {
    frame.line(at(pair[0]), at(pair[1]), [60, 255, 60]);
  }
  for c in &checkpoints {
    let p = at(c.center);
    frame.dot(p[0], p[1], 3, [255, 255, 0]);
  }
  let p = at(start.position);
  frame.dot(p[0], p[1], 5, [255, 60, 60]);
  let file = fs::File::create(out).map_err(|e| format!("{out}: {e}"))?;
  let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), size as u32, size as u32);
  encoder.set_color(png::ColorType::Rgba);
  encoder.set_depth(png::BitDepth::Eight);
  encoder
    .write_header()
    .and_then(|mut w| w.write_image_data(&frame.rgba))
    .map_err(|e| e.to_string())?;
  if lap_time.is_some() {
    Ok(())
  } else {
    Err("autopilot did not complete the lap".into())
  }
}
