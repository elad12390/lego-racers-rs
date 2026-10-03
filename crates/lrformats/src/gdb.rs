use std::fmt;
use std::ops::Range;

use crate::tok::{self, Node, TokError, Value};

const KW_TEXTURES: u8 = 0x27;
const KW_SCALE: u8 = 0x33;
const KW_VERTICES: u8 = 0x2a;
const KW_TRIANGLES: u8 = 0x2d;
const KW_PARTS: u8 = 0x2e;

#[derive(Debug, PartialEq)]
pub enum GdbError {
  Tok(TokError),
  Missing(&'static str),
  Malformed(String),
}

impl fmt::Display for GdbError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      GdbError::Tok(e) => write!(f, "{e}"),
      GdbError::Missing(what) => write!(f, "missing {what}"),
      GdbError::Malformed(what) => write!(f, "malformed {what}"),
    }
  }
}

impl std::error::Error for GdbError {}

impl From<TokError> for GdbError {
  fn from(error: TokError) -> Self {
    GdbError::Tok(error)
  }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vertex {
  pub position: [f32; 3],
  pub uv: [f32; 2],
  pub rgba: [u8; 4],
}

/// One drawable piece: a texture plus a vertex range and the triangle range that indexes it.
/// Triangle indices are local to the vertex range.
#[derive(Debug, Clone, PartialEq)]
pub struct Part {
  pub texture: u16,
  pub joint: u16,
  pub flag: u8,
  pub vertices: Range<u32>,
  pub triangles: Range<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Mesh {
  pub textures: Vec<String>,
  pub scale: f32,
  pub vertices: Vec<Vertex>,
  /// Normal-vertex GDBs use keyword 0x29 instead of baked RGBA 0x2a.
  pub normals: Vec<[f32; 3]>,
  pub triangles: Vec<[u8; 3]>,
  pub parts: Vec<Part>,
}

pub fn parse(data: &[u8]) -> Result<Mesh, GdbError> {
  let nodes = tok::parse(data)?;
  let textures = textures(&nodes);
  let scale = keyword_value(&nodes, KW_SCALE)
    .and_then(|n| {
      if let Node::Float(v) = n {
        Some(*v)
      } else {
        None
      }
    })
    .unwrap_or(1.0);
  let (vertices, normals) = if let Some(body) = statement_body(&nodes, KW_VERTICES) {
    (vertices(body)?, Vec::new())
  } else {
    normal_vertices(statement_body(&nodes, 0x29).ok_or(GdbError::Missing("vertices"))?)?
  };
  let triangles =
    triangles(statement_body(&nodes, KW_TRIANGLES).ok_or(GdbError::Missing("triangles"))?)?;
  let parts = parts(statement_body(&nodes, KW_PARTS).ok_or(GdbError::Missing("parts"))?)?;
  Ok(Mesh {
    textures,
    scale,
    vertices,
    normals,
    triangles,
    parts,
  })
}

fn keyword_value(nodes: &[Node], keyword: u8) -> Option<&Node> {
  let at = nodes.iter().position(|n| *n == Node::Keyword(keyword))?;
  nodes.get(at + 1)
}

/// The `{ ... }` block following `KEYWORD [n]`.
fn statement_body(nodes: &[Node], keyword: u8) -> Option<&[Node]> {
  let at = nodes.iter().position(|n| *n == Node::Keyword(keyword))?;
  nodes[at + 1..].iter().find_map(|n| match n {
    Node::Block(body) => Some(body.as_slice()),
    _ => None,
  })
}

fn textures(nodes: &[Node]) -> Vec<String> {
  match statement_body(nodes, KW_TEXTURES) {
    Some(body) => body
      .iter()
      .flat_map(|n| match n {
        Node::Str(s) => vec![s.clone()],
        Node::PackedStrings(list) => list.clone(),
        _ => Vec::new(),
      })
      .collect(),
    None => Vec::new(),
  }
}

fn vertices(body: &[Node]) -> Result<Vec<Vertex>, GdbError> {
  let mut values = Vec::new();
  for node in body {
    match node {
      Node::Packed { rows, .. } => values.extend(rows.iter().flatten().cloned()),
      Node::Record { fields, .. } => values.extend(fields.iter().cloned()),
      Node::Float(f) => values.push(Value::F32(*f)),
      Node::Int(i) => values.push(Value::I32(*i)),
      Node::Comma | Node::Semi => {}
      _ => return Err(GdbError::Malformed("unexpected vertex token".into())),
    }
  }
  if values.len() % 9 != 0 {
    return Err(GdbError::Malformed("incomplete mixed vertex record".into()));
  }
  values
    .chunks_exact(9)
    .map(|row| {
      vertex(row).ok_or_else(|| GdbError::Malformed(format!("vertex record shape {}", shape(row))))
    })
    .collect()
}

fn shape(row: &[Value]) -> String {
  row
    .iter()
    .map(|v| match v {
      Value::F32(_) => 'f',
      Value::I32(_) => 'i',
      Value::U8(_) | Value::I8(_) => 'b',
      Value::U16(_) | Value::I16(_) => 'h',
    })
    .collect()
}

fn normal_vertices(body: &[Node]) -> Result<(Vec<Vertex>, Vec<[f32; 3]>), GdbError> {
  let mut flat = Vec::new();
  for node in body {
    match node {
      Node::Packed { rows, .. } => flat.extend(rows.iter().flatten().copied()),
      Node::Record { fields, .. } => flat.extend(fields.iter().copied()),
      _ => return Err(GdbError::Malformed("unexpected normal vertex token".into())),
    }
  }
  if flat.len() % 8 != 0 {
    return Err(GdbError::Malformed("incomplete normal vertex".into()));
  }
  let mut vertices = Vec::new();
  let mut normals = Vec::new();
  for row in flat.chunks_exact(8) {
    let v = row
      .iter()
      .map(|v| {
        v.as_f32()
          .ok_or_else(|| GdbError::Malformed("nonfloat normal vertex".into()))
      })
      .collect::<Result<Vec<_>, _>>()?;
    if v.iter().any(|v| !v.is_finite()) {
      return Err(GdbError::Malformed("nonfinite normal vertex".into()));
    }
    vertices.push(Vertex {
      position: [v[0], v[1], v[2]],
      uv: [v[3], v[4]],
      rgba: [255; 4],
    });
    normals.push([v[5], v[6], v[7]]);
  }
  Ok((vertices, normals))
}

fn vertex(row: &[Value]) -> Option<Vertex> {
  let f = |i: usize| row.get(i)?.as_f32();
  let b = |i: usize| {
    row
      .get(i)?
      .as_u32()
      .and_then(|value| u8::try_from(value).ok())
  };
  Some(Vertex {
    position: [f(0)?, f(1)?, f(2)?],
    uv: [f(3)?, f(4)?],
    rgba: [b(5)?, b(6)?, b(7)?, b(8)?],
  })
}

fn triangles(body: &[Node]) -> Result<Vec<[u8; 3]>, GdbError> {
  let mut out = Vec::new();
  for node in body {
    let rows: Vec<&[Value]> = match node {
      Node::Packed { rows, .. } => rows.iter().map(Vec::as_slice).collect(),
      Node::Record { fields, .. } => vec![fields.as_slice()],
      _ => continue,
    };
    for row in rows {
      match row {
        [Value::U8(a), Value::U8(b), Value::U8(c)] => out.push([*a, *b, *c]),
        _ => return Err(GdbError::Malformed("triangle record".into())),
      }
    }
  }
  Ok(out)
}

/// Groups are `marker(texture)` records followed by vertex-range `(flag, start, count)` and
/// triangle-range `(start, count)` records, told apart by their field shapes.
fn parts(body: &[Node]) -> Result<Vec<Part>, GdbError> {
  let mut texture = 0u16;
  let mut joint = 0u16;
  let mut pending_vertices: Option<Part> = None;
  let mut out = Vec::new();
  for node in body {
    let Node::Record { kind, fields } = node else {
      continue;
    };
    match fields.as_slice() {
      [Value::U16(t)] if *kind == 0x1c => texture = *t,
      [Value::U16(j)] if *kind == 0x1d => joint = *j,
      [Value::U8(flag), Value::U16(start), Value::U16(count)] => {
        // A second load need not draw. Retain the first load in the
        // persistent vertex cache, including its joint at load time.
        if let Some(pending) = pending_vertices.take() {
          out.push(pending);
        }
        let start = u32::from(*start);
        pending_vertices = Some(Part {
          texture,
          joint,
          flag: *flag,
          vertices: start..start + u32::from(*count),
          triangles: 0..0,
        });
      }
      [Value::U16(start), Value::U16(count)] => {
        let mut part = pending_vertices.take().ok_or(GdbError::Malformed(
          "triangle range before vertex range".into(),
        ))?;
        let start = u32::from(*start);
        part.texture = texture;
        part.triangles = start..start + u32::from(*count);
        out.push(part);
      }
      _ => return Err(GdbError::Malformed("part record".into())),
    }
  }
  if let Some(pending) = pending_vertices {
    out.push(pending);
  }
  Ok(out)
}

impl Mesh {
  /// Largest triangle index used by a part, if it has any triangles.
  pub fn max_index(&self, part: &Part) -> Option<u8> {
    self
      .triangles
      .get(part.triangles.start as usize..part.triangles.end as usize)?
      .iter()
      .flatten()
      .copied()
      .max()
  }

  /// Triangles as absolute indices into `vertices`.
  ///
  /// The format drives a persistent table of vertex slots, like the vertex cache of
  /// fixed-function hardware. Each part copies `count` vertices starting at `start` into the
  /// slots beginning at its flag byte; triangle indices then address slots, and slots not
  /// reloaded by a part keep the vertices of earlier parts.
  pub fn resolved_triangles(&self) -> Result<Vec<[u32; 3]>, String> {
    self
      .resolved_joint_triangles()
      .map(|triangles| triangles.into_iter().map(|t| t.map(|v| v.0)).collect())
  }
  /// Cache slots retain the joint used when they were loaded, even when a
  /// following partial load uses another joint before drawing a triangle.
  pub fn resolved_joint_triangles(&self) -> Result<Vec<[(u32, u16); 3]>, String> {
    let mut out = vec![[(0, 0); 3]; self.triangles.len()];
    for (part, draw) in self.parts.iter().zip(self.resolved_draws()?) {
      for (row, triangle) in part.triangles.clone().zip(draw) {
        out[row as usize] = triangle;
      }
    }
    Ok(out)
  }
  /// One resolved draw per part. Normal skinned GDBs reuse the same triangle
  /// range after loading different vertices; a global triangle list loses
  /// those earlier draws. Callers rendering geometry must use this method.
  pub fn resolved_draws(&self) -> Result<Vec<Vec<[(u32, u16); 3]>>, String> {
    // Slots start out holding vertices 0, 1, 2, ...; some files rely on that before any load.
    let mut slots: Vec<Option<(u32, u16)>> = (0..256u32)
      .map(|k| ((k as usize) < self.vertices.len()).then_some((k, 0)))
      .collect();
    let mut out = Vec::new();
    let mut covered = vec![false; self.triangles.len()];
    for (n, part) in self.parts.iter().enumerate() {
      for (k, vertex) in part.vertices.clone().enumerate() {
        let slot = usize::from(part.flag) + k;
        *slots
          .get_mut(slot)
          .ok_or_else(|| format!("part {n} loads past slot 255"))? = Some((vertex, part.joint));
      }
      let range = part.triangles.start as usize..part.triangles.end as usize;
      let triangles = self
        .triangles
        .get(range.clone())
        .ok_or_else(|| format!("part {n} triangle range is out of bounds"))?;
      let mut draw = Vec::new();
      for (row, triangle) in range.zip(triangles) {
        let mut resolved = [(0, 0); 3];
        for (corner, &index) in triangle.iter().enumerate() {
          let vertex = slots[usize::from(index)]
            .ok_or_else(|| format!("part {n} uses slot {index} that no part has loaded"))?;
          if vertex.0 as usize >= self.vertices.len() {
            return Err(format!(
              "part {n} resolves to vertex {} of {}",
              vertex.0,
              self.vertices.len()
            ));
          }
          resolved[corner] = vertex;
        }
        covered[row] = true;
        draw.push(resolved);
      }
      out.push(draw);
    }
    if let Some(missing) = covered.iter().position(|&c| !c) {
      return Err(format!("triangle {missing} belongs to no part"));
    }
    Ok(out)
  }

  /// Checks the invariants the format is expected to satisfy; returns the first violation.
  pub fn validate(&self) -> Result<(), String> {
    for (n, part) in self.parts.iter().enumerate() {
      if part.vertices.end as usize > self.vertices.len() {
        return Err(format!("part {n} vertex range is out of bounds"));
      }
    }
    self.resolved_triangles().map(|_| ())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn partial_load_keeps_previous_vertices_and_their_joint_before_draw() {
    let records = vec![
      Node::Record {
        kind: 0x1d,
        fields: vec![Value::U16(2)],
      },
      Node::Record {
        kind: 0x1a,
        fields: vec![Value::U8(0), Value::U16(3), Value::U16(2)],
      },
      Node::Record {
        kind: 0x1d,
        fields: vec![Value::U16(3)],
      },
      Node::Record {
        kind: 0x1a,
        fields: vec![Value::U8(2), Value::U16(5), Value::U16(1)],
      },
      Node::Record {
        kind: 0x1b,
        fields: vec![Value::U16(0), Value::U16(1)],
      },
    ];
    let mesh = Mesh {
      textures: vec![],
      scale: 1.0,
      vertices: (0..6).map(|n| vertex(n as f32)).collect(),
      normals: vec![],
      triangles: vec![[0, 1, 2]],
      parts: parts(&records).unwrap(),
    };
    assert_eq!(
      mesh.resolved_joint_triangles().unwrap(),
      vec![[(3, 2), (4, 2), (5, 3)]]
    );
    assert_eq!(mesh.resolved_triangles().unwrap(), vec![[3, 4, 5]]);
  }
  #[test]
  fn reused_triangle_commands_keep_each_draws_loaded_vertices() {
    let mesh = Mesh {
      textures: vec![],
      scale: 1.0,
      vertices: (0..6).map(|n| vertex(n as f32)).collect(),
      normals: vec![],
      triangles: vec![[0, 1, 2]],
      parts: vec![
        Part {
          texture: 0,
          joint: 1,
          flag: 0,
          vertices: 0..3,
          triangles: 0..1,
        },
        Part {
          texture: 0,
          joint: 2,
          flag: 0,
          vertices: 3..6,
          triangles: 0..1,
        },
      ],
    };
    assert_eq!(
      mesh.resolved_draws().unwrap(),
      vec![
        vec![[(0, 1), (1, 1), (2, 1)]],
        vec![[(3, 2), (4, 2), (5, 2)]]
      ]
    );
  }

  fn vertex(x: f32) -> Vertex {
    Vertex {
      position: [x, 0.0, 0.0],
      uv: [0.0, 0.0],
      rgba: [255; 4],
    }
  }

  #[test]
  fn joint_markers_do_not_overwrite_material_selection() {
    let records = vec![
      Node::Record {
        kind: 0x1c,
        fields: vec![Value::U16(0)],
      },
      Node::Record {
        kind: 0x1d,
        fields: vec![Value::U16(1)],
      },
      Node::Record {
        kind: 0x1a,
        fields: vec![Value::U8(0), Value::U16(0), Value::U16(3)],
      },
      Node::Record {
        kind: 0x1b,
        fields: vec![Value::U16(0), Value::U16(1)],
      },
    ];
    let parsed = parts(&records).unwrap();
    assert_eq!(parsed[0].texture, 0);
  }

  fn mesh(parts: Vec<Part>, triangles: Vec<[u8; 3]>, vertex_count: usize) -> Mesh {
    Mesh {
      textures: vec![],
      normals: vec![],
      scale: 1.0,
      vertices: (0..vertex_count).map(|n| vertex(n as f32)).collect(),
      triangles,
      parts,
    }
  }

  #[test]
  fn a_part_loads_into_slots_starting_at_its_flag() {
    // Part 0 fills slots 0..4 with vertices 0..4. Part 1 reloads slots 2..4 with vertices 4..6,
    // so slots 0 and 1 still hold vertices 0 and 1 from part 0.
    let parts = vec![
      Part {
        texture: 0,
        joint: 0,
        flag: 0,
        vertices: 0..4,
        triangles: 0..1,
      },
      Part {
        texture: 0,
        joint: 0,
        flag: 2,
        vertices: 4..6,
        triangles: 1..2,
      },
    ];
    let m = mesh(parts, vec![[0, 1, 2], [0, 2, 3]], 6);
    assert_eq!(m.resolved_triangles().unwrap(), vec![[0, 1, 2], [0, 4, 5]]);
  }

  #[test]
  fn slots_start_out_holding_the_first_vertices() {
    let parts = vec![Part {
      texture: 0,
      joint: 0,
      flag: 3,
      vertices: 3..5,
      triangles: 0..1,
    }];
    let m = mesh(parts, vec![[0, 3, 4]], 5);
    assert_eq!(m.resolved_triangles().unwrap(), vec![[0, 3, 4]]);
  }

  #[test]
  fn rejects_unloaded_slots_and_uncovered_triangles() {
    let parts = vec![Part {
      texture: 0,
      joint: 0,
      flag: 0,
      vertices: 0..2,
      triangles: 0..1,
    }];
    let m = mesh(parts.clone(), vec![[0, 1, 9]], 2);
    assert!(m.resolved_triangles().unwrap_err().contains("slot 9"));
    let m = mesh(parts, vec![[0, 1, 0], [0, 1, 0]], 2);
    assert!(m
      .resolved_triangles()
      .unwrap_err()
      .contains("triangle 1 belongs to no part"));
  }
}
