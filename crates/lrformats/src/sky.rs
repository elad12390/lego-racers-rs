//! Original SKB named, cyclic RGB sequences and optional vertical sky offset.
use crate::tok::{self, Node};

#[derive(Debug)]
pub struct Profile {
  pub name: String,
  pub colors: [[u8; 3]; 3],
  pub frames: Vec<Frame>,
}

#[derive(Debug, Clone)]
pub struct Frame {
  pub duration_ms: u32,
  pub colors: [[u8; 3]; 3],
}

#[derive(Debug)]
pub struct Sky {
  pub default: String,
  pub profiles: Vec<Profile>,
  pub vertical_offset: f32,
}

pub fn parse(data: &[u8]) -> Result<Sky, String> {
  let nodes = tok::parse(data).map_err(|e| e.to_string())?;
  let default = nodes
    .windows(2)
    .find_map(|pair| match pair {
      [Node::Keyword(0x2d), Node::Str(name)] => Some(name.clone()),
      _ => None,
    })
    .ok_or("SKB lacks default environment")?;
  let body = nodes
    .iter()
    .find_map(|node| {
      if let Node::Block(body) = node {
        Some(body)
      } else {
        None
      }
    })
    .ok_or("SKB lacks profiles")?;
  let mut profiles = Vec::new();
  for triple in body.windows(3) {
    let [Node::Count(_), Node::Str(name), Node::Block(frames)] = triple else {
      continue;
    };
    let mut sequence = Vec::new();
    for fields in frames.iter().filter_map(|node| match node {
      Node::Block(fields) => Some(fields),
      _ => None,
    }) {
      sequence.push(parse_frame(fields)?);
    }
    let colors = sequence.first().ok_or("SKB profile lacks palette")?.colors;
    profiles.push(Profile {
      name: name.clone(),
      colors,
      frames: sequence,
    });
  }
  if !profiles.iter().any(|p| p.name == default) {
    return Err("SKB default profile missing".into());
  }
  let vertical_offset = match crate::named_records::value(&nodes, 0x2e) {
    None => 0.0,
    Some(Node::Float(v)) if v.is_finite() => *v,
    _ => return Err("invalid SKB vertical offset".into()),
  };
  Ok(Sky {
    default,
    profiles,
    vertical_offset,
  })
}

fn parse_frame(fields: &[Node]) -> Result<Frame, String> {
  let mut colors = [[0; 3]; 3];
  for (i, kind) in [0x17, 0x18, 0x19].into_iter().enumerate() {
    let values = fields
      .iter()
      .find_map(|n| match n {
        Node::Record { kind: k, fields } if *k == kind => Some(fields),
        _ => None,
      })
      .ok_or("SKB profile lacks a color")?;
    if values.len() != 3 {
      return Err("SKB palette color requires3channels".into());
    }
    for (channel, value) in colors[i].iter_mut().zip(values) {
      *channel = value
        .as_u32()
        .and_then(|v| u8::try_from(v).ok())
        .ok_or("SKB invalid color channel")?;
    }
  }
  let duration_ms = match crate::named_records::value(fields, 0x28) {
    None => 1000,
    Some(Node::Int(v)) if *v > 0 => *v as u32,
    _ => return Err("invalid SKB frame duration".into()),
  };
  Ok(Frame {
    duration_ms,
    colors,
  })
}
