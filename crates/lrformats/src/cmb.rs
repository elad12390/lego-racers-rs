//! `CHASSIS.CMB`: the chassis table. Field meanings come from the exe's loader (which stores
//! each keyword at a fixed offset of a 0x104-byte record). Names marked "unverified" are guesses
//! from the values; the layout itself is exact.

use crate::tok::{self, Node, TokError};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Chassis {
  pub name: String,
  /// Wheel model names from the `28` block (keywords 0x34..=0x38).
  pub wheel_models: Vec<(u8, String)>,
  /// Name of the brick set used by this chassis (keyword 0x39).
  pub brick_set: String,
  /// Keyword 0x2a: three floats (unverified: center-of-mass offset).
  pub offset: [f32; 3],
  /// Keyword 0x2b: three floats (unverified: body dimensions or inertia).
  pub size: [f32; 3],
  /// Keyword 0x2c (unverified: mass).
  pub mass: f32,
  /// Keyword 0x2d: two floats.
  pub speed_range: [f32; 2],
  /// Keyword 0x2e: two floats.
  pub gear_range: [f32; 2],
  /// Keyword 0x2f (unverified: grip).
  pub grip: f32,
  /// Keyword 0x30: two leading floats followed by four 3D points (unverified: front axle).
  pub points_a_head: [f32; 2],
  pub points_a: [[f32; 3]; 4],
  /// Keyword 0x31: four 3D points (unverified: rear axle).
  pub points_b: [[f32; 3]; 4],
  pub flag_32: i32,
  pub flag_33: i32,
  /// Bytes at record offsets 0x100, 0x101, 0x102 (keywords 0x3a, 0x3c, 0x3b); the loader's
  /// defaults are 50, 80, 50.
  pub rating_a: u8,
  pub rating_b: u8,
  pub rating_c: u8,
}

pub fn parse(data: &[u8]) -> Result<Vec<Chassis>, TokError> {
  let nodes = tok::parse(data)?;
  let Some(Node::Block(body)) = nodes.iter().find(|n| matches!(n, Node::Block(_))) else {
    return Ok(Vec::new());
  };
  let mut out = Vec::new();
  let mut i = 0;
  while i + 2 < body.len() {
    if let (Node::Keyword(0x27), Node::Str(name), Node::Block(fields)) =
      (&body[i], &body[i + 1], &body[i + 2])
    {
      out.push(chassis(name, fields));
      i += 3;
    } else {
      i += 1;
    }
  }
  Ok(out)
}

fn chassis(name: &str, fields: &[Node]) -> Chassis {
  let mut c = Chassis {
    name: name.to_string(),
    rating_a: 50,
    rating_b: 80,
    rating_c: 50,
    ..Chassis::default()
  };
  let mut i = 0;
  while i < fields.len() {
    let Node::Keyword(k) = fields[i] else {
      i += 1;
      continue;
    };
    let rest = &fields[i + 1..];
    match k {
      0x28 => {
        if let Some(Node::Block(models)) = rest.first() {
          let mut j = 0;
          while j + 1 < models.len() {
            if let (Node::Keyword(slot), Node::Str(model)) = (&models[j], &models[j + 1]) {
              c.wheel_models.push((*slot, model.clone()));
              j += 2;
            } else {
              j += 1;
            }
          }
        }
      }
      0x39 => {
        if let Some(Node::Str(s)) = rest.first() {
          c.brick_set = s.clone();
        }
      }
      0x2a => read_floats(rest, &mut c.offset),
      0x2b => read_floats(rest, &mut c.size),
      0x2c => read_floats(rest, std::slice::from_mut(&mut c.mass)),
      0x2d => read_floats(rest, &mut c.speed_range),
      0x2e => read_floats(rest, &mut c.gear_range),
      0x2f => read_floats(rest, std::slice::from_mut(&mut c.grip)),
      0x30 => {
        let flat = block_floats(rest);
        if flat.len() >= 14 {
          c.points_a_head = [flat[0], flat[1]];
          c.points_a = points(&flat[2..14]);
        }
      }
      0x31 => {
        let flat = block_floats(rest);
        if flat.len() >= 12 {
          c.points_b = points(&flat[..12]);
        }
      }
      0x32 => c.flag_32 = int(rest),
      0x33 => c.flag_33 = int(rest),
      0x3a => c.rating_a = int(rest) as u8,
      0x3b => c.rating_c = int(rest) as u8,
      0x3c => c.rating_b = int(rest) as u8,
      _ => {}
    }
    i += 1;
  }
  c
}

fn read_floats(rest: &[Node], out: &mut [f32]) {
  for (slot, node) in out.iter_mut().zip(rest) {
    match node {
      Node::Float(v) => *slot = *v,
      _ => break,
    }
  }
}

fn int(rest: &[Node]) -> i32 {
  match rest.first() {
    Some(Node::Int(v)) => *v,
    _ => 0,
  }
}

fn block_floats(rest: &[Node]) -> Vec<f32> {
  let Some(Node::Block(body)) = rest.first() else {
    return Vec::new();
  };
  body
    .iter()
    .filter_map(|n| match n {
      Node::Packed { rows, .. } => Some(
        rows
          .iter()
          .filter_map(|r| r.first()?.as_f32())
          .collect::<Vec<_>>(),
      ),
      _ => None,
    })
    .flatten()
    .collect()
}

fn points(flat: &[f32]) -> [[f32; 3]; 4] {
  let mut out = [[0.0; 3]; 4];
  for (point, chunk) in out.iter_mut().zip(flat.chunks_exact(3)) {
    *point = [chunk[0], chunk[1], chunk[2]];
  }
  out
}

#[cfg(test)]
mod tests {
  use super::*;

  fn f(v: f32) -> Vec<u8> {
    let mut b = vec![3];
    b.extend_from_slice(&v.to_le_bytes());
    b
  }

  #[test]
  fn reads_scalars_ratings_and_wheel_points() {
    let mut data = vec![5, 0x27, 2, b'c', 0, 5];
    data.extend_from_slice(&[0x2c]);
    data.extend(f(2500.0));
    data.extend_from_slice(&[0x2d]);
    data.extend(f(4.0));
    data.extend(f(10.5));
    data.extend_from_slice(&[0x3a, 4, 30, 0, 0, 0]);
    data.extend_from_slice(&[0x39, 2, b'b', b'k', 0]);
    // 31 { 14 count=12 kind=3 <12 floats> }
    data.extend_from_slice(&[0x31, 5, 0x14, 12, 0, 3]);
    for n in 0..12 {
      data.extend_from_slice(&(n as f32).to_le_bytes());
    }
    data.extend_from_slice(&[6, 6, 6]);
    let chassis = parse(&data).unwrap();
    assert_eq!(chassis.len(), 1);
    let c = &chassis[0];
    assert_eq!(c.name, "c");
    assert_eq!((c.mass, c.speed_range), (2500.0, [4.0, 10.5]));
    assert_eq!((c.rating_a, c.rating_b, c.rating_c), (30, 80, 50));
    assert_eq!(c.brick_set, "bk");
    assert_eq!(c.points_b[1], [3.0, 4.0, 5.0]);
  }
}
