//! Original LPIECELO.LEB: shared command, point, normal, UV and stud-cell pools.
//! Decoding follows BrickDatabase_Load0049ee30 / PathGrid_AddRoute0049a450.
use crate::tok::{self, Node, Value};

pub struct Brick {
  pub name: String,
  pub id: u16,
  pub width: u8,
  pub depth: u8,
  pub cells: Vec<[u8; 2]>,
  pub origin: Option<[f32; 3]>,
  pub faces: Vec<Face>,
}
pub struct Face {
  pub material: u16,
  pub points: Vec<[f32; 3]>,
  pub normals: Vec<[f32; 3]>,
  pub uvs: Vec<[f32; 2]>,
}
pub struct BrickDatabase {
  pub bricks: Vec<Brick>,
}

fn integers(nodes: &[Node]) -> Result<Vec<i32>, String> {
  let mut values = Vec::new();
  for node in nodes {
    match node {
      Node::Int(i) => values.push(*i),
      Node::Packed { kind: 4, rows } => {
        for row in rows {
          for v in row {
            if let Value::I32(i) = v {
              values.push(*i);
            } else {
              return Err("noninteger brick pool value".into());
            }
          }
        }
      }
      _ => return Err("unsupported brick pool token".into()),
    }
  }
  Ok(values)
}
impl BrickDatabase {
  pub fn parse(data: &[u8]) -> Result<Self, String> {
    let nodes = tok::parse(data).map_err(|e| e.to_string())?;
    let section = |key| {
      nodes
        .windows(3)
        .find_map(|p| {
          if let [Node::Keyword(k), Node::Count(count), Node::Block(body)] = p {
            if *k == key {
              Some((*count, body.as_slice()))
            } else {
              None
            }
          } else {
            None
          }
        })
        .ok_or_else(|| format!("missing brick pool {key:#x}"))
    };
    let pool = |key, stride: usize| {
      let (count, body) = section(key)?;
      let values = integers(body)?;
      if values.len() != count as usize * stride {
        return Err(format!("brick pool {key:#x} count mismatch"));
      }
      Ok(values)
    };
    let commands = pool(0x28, 1)?;
    let points = pool(0x29, 3)?;
    let normals = pool(0x2a, 3)?;
    let uvs = pool(0x2b, 2)?;
    let cells = pool(0x2c, 2)?;
    let (count, body) = section(0x27)?;
    let mut bricks = Vec::new();
    let mut at = 0;
    while at < body.len() {
      let Node::Str(name) = &body[at] else {
        return Err("brick entry missing name".into());
      };
      at += 1;
      let start = at;
      while at < body.len() && !matches!(body[at], Node::Str(_)) {
        at += 1;
      }
      let values = integers(&body[start..at])?;
      let [id, cell_offset, face_count, command_offset] = values.as_slice() else {
        return Err("invalid brick entry metadata".into());
      };
      let cell_offset = *cell_offset as usize * 2;
      let width = *cells
        .get(cell_offset)
        .ok_or("brick footprint header outside pool")? as u8;
      let depth = *cells
        .get(cell_offset + 1)
        .ok_or("brick footprint header missing")? as u8;
      let footprint_end = cell_offset + 2 + width as usize * depth as usize * 2;
      let footprint = cells
        .get(cell_offset + 2..footprint_end)
        .ok_or("brick footprint outside pool")?
        .chunks_exact(2)
        .map(|p| [p[0] as u8, p[1] as u8])
        .collect();
      let origin = if *id < 0x800 {
        let pair = cells
          .get(footprint_end..footprint_end + 2)
          .ok_or("chassis origin missing")?;
        let index = (pair[0] as u8 as usize) | ((pair[1] as u8 as usize) << 8);
        if index == 0xffff {
          None
        } else {
          let point = points
            .get(index * 3..index * 3 + 3)
            .ok_or("chassis origin index outside pool")?;
          Some([
            point[0] as i16 as f32 / 256.0,
            point[1] as i16 as f32 / 256.0,
            point[2] as i16 as f32 / 256.0,
          ])
        }
      } else {
        None
      };
      let mut cursor = *command_offset as usize;
      let mut faces: Vec<Face> = Vec::new();
      let mut normal = None;
      let mut per_vertex = false;
      let mut textured = false;
      let read = |cursor: &mut usize| {
        let value = commands
          .get(*cursor)
          .copied()
          .ok_or("brick command outside pool")?;
        *cursor += 1;
        Ok::<u16, String>(value as u16)
      };
      for _ in 0..*face_count {
        let command = read(&mut cursor)?;
        let quad = command & 0x3000 == 0x2000;
        if !quad {
          per_vertex = command & 0x4000 != 0;
          textured = command & 0x3000 == 0x1000;
        }
        let mut face = Face {
          material: command & 0x7ff,
          points: Vec::new(),
          normals: Vec::new(),
          uvs: Vec::new(),
        };
        for i in 0..if quad { 1 } else { 3 } {
          let point = read(&mut cursor)? as usize;
          let point = points
            .get(point * 3..point * 3 + 3)
            .ok_or("brick point index outside pool")?;
          face.points.push([
            point[0] as i16 as f32 / 256.0,
            point[1] as i16 as f32 / 256.0,
            point[2] as i16 as f32 / 256.0,
          ]);
          if quad {
            if per_vertex {
              normal = Some(read(&mut cursor)? as usize);
            }
          } else if command & 0x8000 == 0 && (per_vertex || i == 0) {
            normal = Some(read(&mut cursor)? as usize);
          }
          let n = normal.ok_or("brick normal used before assignment")?;
          let n = normals
            .get(n * 3..n * 3 + 3)
            .ok_or("brick normal outside pool")?;
          face.normals.push([
            n[0] as i8 as f32 / 127.0,
            n[1] as i8 as f32 / 127.0,
            n[2] as i8 as f32 / 127.0,
          ]);
          let uv = if textured {
            let uv = read(&mut cursor)? as usize;
            let uv = uvs.get(uv * 2..uv * 2 + 2).ok_or("brick UV outside pool")?;
            [uv[0] as i16 as f32 / 1024.0, uv[1] as i16 as f32 / 1024.0]
          } else {
            [0.0; 2]
          };
          face.uvs.push(uv);
        }
        if quad {
          let previous = faces.last_mut().ok_or("brick quad before triangle")?;
          if previous.points.len() != 3 {
            return Err("brick quad after nontriangle".into());
          }
          previous.points.append(&mut face.points);
          previous.normals.append(&mut face.normals);
          previous.uvs.append(&mut face.uvs);
        } else {
          faces.push(face);
        }
      }
      bricks.push(Brick {
        name: name.clone(),
        id: *id as u16,
        width,
        depth,
        cells: footprint,
        origin,
        faces,
      });
    }
    if bricks.len() != count as usize {
      return Err("brick entry count mismatch".into());
    }
    Ok(Self { bricks })
  }
  pub fn find(&self, name: &str) -> Result<&Brick, String> {
    self
      .bricks
      .iter()
      .find(|b| b.name.eq_ignore_ascii_case(name))
      .ok_or_else(|| format!("original brick {name} not found"))
  }
}
