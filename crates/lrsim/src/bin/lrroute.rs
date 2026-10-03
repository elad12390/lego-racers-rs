//! Plays every recorded rival run of a race through the ported route cursor, checks it against
//! the collision mesh and draws the paths over the track.
//! usage: lrroute <LEGO.JAM> <RACE_TABLE> <out.png>

use std::fs;
use std::process::ExitCode;

use lrformats::library::Library;
use lrformats::model::Model;
use lrformats::render::{render_with, Camera, Projector};
use lrformats::route::RouteRecord;
use lrformats::world::{self, CollisionMesh, CollisionTriangle};
use lrsim::ground::Ground;
use lrsim::route_cursor::RouteCursor;

const FRAME_MS: f32 = 1000.0 / 60.0;
const COLORS: [[u8; 3]; 5] = [
  [255, 60, 60],
  [60, 255, 60],
  [80, 140, 255],
  [255, 255, 60],
  [255, 80, 255],
];

fn main() -> ExitCode {
  let args: Vec<String> = std::env::args().skip(1).collect();
  let [jam, table, out] = args.as_slice() else {
    eprintln!("usage: lrroute <LEGO.JAM> <RACE_TABLE> <out.png>");
    return ExitCode::FAILURE;
  };
  match run(jam, table, out) {
    Ok(()) => ExitCode::SUCCESS,
    Err(message) => {
      eprintln!("{message}");
      ExitCode::FAILURE
    }
  }
}

fn run(jam: &str, table: &str, out: &str) -> Result<(), String> {
  let library = Library::open(jam).map_err(|e| format!("{jam}: {e}"))?;
  let entries: Vec<_> = library
    .jam()
    .tables
    .iter()
    .filter(|t| t.name.eq_ignore_ascii_case(table))
    .flat_map(|t| t.entries.iter())
    .collect();
  let bytes = |name: &str| -> Option<&[u8]> {
    entries
      .iter()
      .find(|e| e.name.eq_ignore_ascii_case(name))
      .and_then(|e| library.jam().bytes(e).ok())
  };

  let mut merged = CollisionMesh::default();
  for entry in &entries {
    let name = entry.name.to_ascii_uppercase();
    let trigger =
      name.starts_with("CHCKPT") || name.starts_with("STARTFIN") || name.starts_with("STRTFIN");
    if !name.ends_with(".BVB") || trigger {
      continue;
    }
    let mesh = world::collision_mesh(library.jam().bytes(entry).map_err(|e| e.to_string())?)
      .map_err(|e| e.to_string())?;
    let base = merged.vertices.len() as u32;
    merged.vertices.extend(mesh.vertices);
    merged
      .triangles
      .extend(mesh.triangles.into_iter().map(|t| CollisionTriangle {
        indices: t.indices.map(|i| i + base),
        surface: t.surface,
      }));
  }
  let ground = Ground::new(merged);

  let wdb = entries
    .iter()
    .filter(|e| e.name.to_ascii_uppercase().ends_with(".WDB"))
    .find_map(|e| world::track_model(library.jam().bytes(e).ok()?))
    .ok_or("no track model")?;
  let model = Model::load(&library, &wdb, Some(table)).map_err(|e| e.to_string())?;
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
      *c = (u16::from(*c) * 5 / 10) as u8;
    }
  }

  let mut names: Vec<&str> = entries
    .iter()
    .map(|e| e.name.as_str())
    .filter(|n| n.to_ascii_uppercase().ends_with(".RRB"))
    .collect();
  names.sort();
  let (mut total_samples, mut on_ground, mut max_step, mut worst_file) =
    (0usize, 0usize, 0.0f32, String::new());
  for name in &names {
    let record = RouteRecord::load(bytes(name).ok_or("missing rrb")?, false)
      .map_err(|e| format!("{name}: {e}"))?;
    let rival = name.as_bytes()[1].saturating_sub(b'1') as usize % COLORS.len();
    let lap_after_loop: i32 = record.points[record.loop_index as usize + 1..]
      .iter()
      .map(|p| p.length())
      .sum();
    let total_ms = record.loop_time as f32 + 2.0 * lap_after_loop as f32;
    let mut cursor = RouteCursor::start_at_beginning(&record);
    let mut last: Option<[f32; 3]> = None;
    let mut elapsed = 0.0f32;
    while elapsed < total_ms {
      cursor.advance(FRAME_MS);
      elapsed += FRAME_MS;
      let p = cursor.position;
      total_samples += 1;
      if let Some(hit) = ground.at(p[0], p[1], p[2] + 8.0) {
        if (hit.height - p[2]).abs() < 6.0 {
          on_ground += 1;
        }
      }
      if let Some(l) = last {
        let step = ((p[0] - l[0]).powi(2) + (p[1] - l[1]).powi(2) + (p[2] - l[2]).powi(2)).sqrt();
        // The wrap back to the loop point is allowed to jump; everything else must be smooth.
        if step > max_step && step < 40.0 {
          max_step = step;
          worst_file = name.to_string();
        }
      }
      last = Some(p);
      let s = projector.project(p);
      frame.dot(s[0], s[1], 1, COLORS[rival]);
    }
  }
  let file = fs::File::create(out).map_err(|e| format!("{out}: {e}"))?;
  let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), size as u32, size as u32);
  encoder.set_color(png::ColorType::Rgba);
  encoder.set_depth(png::BitDepth::Eight);
  encoder
    .write_header()
    .and_then(|mut w| w.write_image_data(&frame.rgba))
    .map_err(|e| e.to_string())?;
  println!(
        "{table}: {} recordings, {total_samples} samples, {:.1}% on the collision mesh, largest smooth step {max_step:.2} units/frame ({worst_file})",
        names.len(),
        100.0 * on_ground as f32 / total_samples.max(1) as f32
    );
  Ok(())
}
