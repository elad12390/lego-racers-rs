//! GCB shared short-position / normal / UV pools and named triangle meshes.
use crate::tok::{self, Node, Value};
use std::collections::BTreeMap;
#[derive(Clone)]
pub struct Vertex {
  pub position: [f32; 3],
  pub uv: [f32; 2],
  pub normal: [f32; 3],
}
pub struct Surface {
  pub material: String,
  pub triangles: Vec<[u32; 3]>,
}
pub struct Part {
  pub scale: f32,
  pub surfaces: Vec<Surface>,
}
pub struct Geometry {
  pub vertices: Vec<Vertex>,
  pub parts: BTreeMap<String, Part>,
}

fn integers(nodes: &[Node]) -> Result<Vec<i32>, String> {
  let mut out = Vec::new();
  for node in nodes {
    let values = match node {
      Node::Int(v) => {
        out.push(*v);
        continue;
      }
      Node::Record { fields, .. } => fields.as_slice(),
      Node::Packed { rows, .. } => {
        for row in rows {
          out.extend(row.iter().map(integer).collect::<Result<Vec<_>, _>>()?);
        }
        continue;
      }
      _ => return Err("noninteger GCB pool token".into()),
    };
    out.extend(values.iter().map(integer).collect::<Result<Vec<_>, _>>()?);
  }
  Ok(out)
}
fn integer(value: &Value) -> Result<i32, String> {
  Ok(match value {
    Value::I8(v) => *v as i32,
    Value::U8(v) => *v as i32,
    Value::I16(v) => *v as i32,
    Value::U16(v) => *v as i32,
    Value::I32(v) => *v,
    Value::F32(_) => return Err("float in GCB integer pool".into()),
  })
}
fn counted(nodes: &[Node], key: u8) -> Result<(usize, &[Node]), String> {
  nodes
    .windows(3)
    .find_map(|p| {
      if let [Node::Keyword(k), Node::Count(n), Node::Block(body)] = p {
        (*k == key).then_some((*n as usize, body.as_slice()))
      } else {
        None
      }
    })
    .ok_or_else(|| format!("missing GCB section {key:x}"))
}
impl Geometry {
  pub fn parse(bytes: &[u8]) -> Result<Self, String> {
    let nodes = tok::parse(bytes).map_err(|e| e.to_string())?;
    let (count, pool) = counted(&nodes, 0x28)?;
    let flat = integers(pool)?;
    if flat.len() != count * 8 {
      return Err("GCB vertex pool count mismatch".into());
    }
    let vertices = flat
      .chunks_exact(8)
      .map(|v| Vertex {
        position: [v[0] as f32, v[1] as f32, v[2] as f32],
        uv: [v[3] as f32 / 4096.0, v[4] as f32 / 4096.0],
        normal: [
          v[5] as f32 / 127.0,
          v[6] as f32 / 127.0,
          v[7] as f32 / 127.0,
        ],
      })
      .collect::<Vec<_>>();
    let (count, models) = counted(&nodes, 0x2c)?;
    let mut parts = BTreeMap::new();
    for row in models.chunks_exact(3) {
      let [Node::Keyword(0x2c), Node::Str(name), Node::Block(fields)] = row else {
        return Err("invalid GCB named part".into());
      };
      let scale = fields
        .windows(2)
        .find_map(|p| {
          if let [Node::Keyword(0x2d), Node::Float(scale)] = p {
            Some(*scale)
          } else {
            None
          }
        })
        .ok_or("missing GCB scale")?;
      if !scale.is_finite() || scale <= 0.0 {
        return Err("invalid GCB scale".into());
      }
      let (count, blocks) = counted(fields, 0x2b)?;
      let mut surfaces = Vec::new();
      for block in blocks.chunks_exact(2) {
        let [Node::Keyword(0x2b), Node::Block(fields)] = block else {
          return Err("invalid GCB material block".into());
        };
        let material = crate::named_records::string(fields, 0x27)?;
        let (count, indices) = counted(fields, 0x2a)?;
        let flat = integers(indices)?;
        if flat.len() != count * 3 || flat.iter().any(|i| *i < 0 || *i as usize >= vertices.len()) {
          return Err(format!("invalid GCB triangle pool in {name}/{material}"));
        }
        let triangles = flat
          .chunks_exact(3)
          .map(|v| [v[0] as u32, v[1] as u32, v[2] as u32])
          .collect();
        surfaces.push(Surface {
          material,
          triangles,
        });
      }
      if blocks.len() % 2 != 0 || surfaces.len() != count {
        return Err("GCB surface count mismatch".into());
      }
      if parts
        .insert(name.to_ascii_lowercase(), Part { scale, surfaces })
        .is_some()
      {
        return Err("duplicate GCB part".into());
      }
    }
    if models.len() % 3 != 0 || parts.len() != count {
      return Err("GCB part count mismatch".into());
    }
    Ok(Self { vertices, parts })
  }
}
