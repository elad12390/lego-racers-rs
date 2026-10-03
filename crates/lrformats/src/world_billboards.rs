//! Original WDB 0x37 sprite placements, dimensions, orientation and draw range.
use crate::{
  named_records,
  tok::{self, Node, Value},
};

#[derive(Debug, Clone)]
pub enum MaterialRef {
  Name(String),
  Index { database: usize, slot: usize },
}
#[derive(Debug, Clone)]
pub struct Billboard {
  pub material: MaterialRef,
  pub position: [f32; 3],
  pub axis: Option<[f32; 3]>,
  pub width: f32,
  pub height: f32,
  pub range: f32,
}

fn vector(fields: &[Node], kind: u8) -> Result<Option<[f32; 3]>, String> {
  let Some(values) = fields.iter().find_map(|n| {
    if let Node::Record { kind: k, fields } = n {
      (*k == kind).then_some(fields)
    } else {
      None
    }
  }) else {
    return Ok(None);
  };
  let values = values
    .iter()
    .map(|v| match v {
      Value::F32(v) if v.is_finite() => Ok(*v),
      _ => Err("invalid WDB sprite vector"),
    })
    .collect::<Result<Vec<_>, _>>()?;
  Ok(Some(
    values
      .try_into()
      .map_err(|_| "invalid WDB sprite vector length")?,
  ))
}
pub fn parse(bytes: &[u8]) -> Result<Vec<Billboard>, String> {
  let nodes = tok::parse(bytes).map_err(|e| e.to_string())?;
  let Some((count, body)) = nodes.windows(3).find_map(|row| {
    if let [Node::Keyword(0x37), Node::Count(n), Node::Block(body)] = row {
      Some((*n, body))
    } else {
      None
    }
  }) else {
    return Ok(Vec::new());
  };
  let mut out = Vec::new();
  for row in body.chunks_exact(2) {
    let [Node::Keyword(0x37), Node::Block(fields)] = row else {
      return Err("invalid WDB sprite record".into());
    };
    let number = |key, default| match named_records::value(fields, key) {
      Some(Node::Float(v)) if v.is_finite() => Ok(*v),
      None => Ok(default),
      _ => Err("invalid WDB sprite dimension"),
    };
    let width = number(0x3a, 1.0)?;
    let height = number(0x3b, 1.0)?;
    let range = number(0x3c, -1.0)?;
    if width <= 0.0 || height <= 0.0 {
      return Err("nonpositive WDB sprite size".into());
    }
    let axis = vector(fields, 0x1b)?;
    if axis.is_some_and(|v| v.iter().map(|a| a * a).sum::<f32>() < f32::EPSILON) {
      return Err("zero WDB sprite axis".into());
    }
    let material = if let Some(at) = fields.iter().position(|n| *n == Node::Keyword(0x3e)) {
      let Some([Node::Int(database), Node::Int(slot)]) = fields.get(at + 1..at + 3) else {
        return Err("invalid indexed WDB sprite material".into());
      };
      MaterialRef::Index {
        database: usize::try_from(*database).map_err(|_| "negative WDB sprite database")?,
        slot: usize::try_from(*slot).map_err(|_| "negative WDB sprite material")?,
      }
    } else {
      MaterialRef::Name(named_records::string(fields, 0x39)?)
    };
    out.push(Billboard {
      material,
      position: vector(fields, 0x17)?.ok_or("missing WDB sprite position")?,
      axis,
      width,
      height,
      range,
    });
  }
  if body.len() % 2 != 0 || out.len() != count as usize {
    return Err("WDB sprite count mismatch".into());
  }
  Ok(out)
}
