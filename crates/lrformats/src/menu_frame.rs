//! Original eight-piece frame styles shared by MIB geometry and MSB defaults.
use crate::tok::{self, Node};
use std::collections::BTreeMap;

#[derive(Debug)]
pub struct Frame {
  pub images: [String; 8],
  pub edge_color: [u8; 4],
  pub panel_color: [u8; 4],
  pub shown: bool,
}
pub fn parse_styles(bytes: &[u8]) -> Result<BTreeMap<String, Frame>, String> {
  let nodes = tok::parse(bytes).map_err(|e| e.to_string())?;
  let mut styles = BTreeMap::new();
  for section in nodes.windows(3) {
    let [Node::Keyword(0x34), Node::Count(count), Node::Block(body)] = section else {
      continue;
    };
    if body.len() != *count as usize * 3 {
      return Err("MSB frame count mismatch".into());
    }
    for row in body.chunks_exact(3) {
      let [Node::Keyword(0x34), Node::Str(name), Node::Block(fields)] = row else {
        return Err("invalid MSB frame entry".into());
      };
      let frame = parse_fields(fields)?.ok_or("missing MSB frame images")?;
      if styles.insert(name.to_ascii_lowercase(), frame).is_some() {
        return Err(format!("duplicate MSB frame {name}"));
      }
    }
  }
  if styles.is_empty() {
    return Err("MSB contains no frame styles".into());
  }
  Ok(styles)
}
pub(crate) fn parse_fields(fields: &[Node]) -> Result<Option<Frame>, String> {
  let Some(images) = fields.windows(2).find_map(|pair| match pair {
    [Node::Keyword(0x28), Node::PackedStrings(images)] => Some(images),
    _ => None,
  }) else {
    return Ok(None);
  }; // Geometry-only MIB entries inherit MSB defaults.
  let images = images
    .clone()
    .try_into()
    .map_err(|_| "frame needs eight images")?;
  let colors = fields
    .windows(2)
    .find_map(|pair| match pair {
      [Node::Keyword(0x2a), Node::Packed { kind: 4, rows }] => Some(rows),
      _ => None,
    })
    .ok_or("missing frame colors")?;
  let colors = colors
    .iter()
    .flatten()
    .map(|v| {
      v.as_u32()
        .and_then(|v| u8::try_from(v).ok())
        .ok_or("invalid frame color channel")
    })
    .collect::<Result<Vec<_>, _>>()?;
  let [a, r, g, b, pa, pr, pg, pb] = colors.as_slice() else {
    return Err("frame needs edge and panel ARGB colors".into());
  };
  let shown = !fields
    .windows(2)
    .any(|pair| matches!(pair, [Node::Keyword(0x33), Node::Int(0)]));
  Ok(Some(Frame {
    images,
    edge_color: [*r, *g, *b, *a],
    panel_color: [*pr, *pg, *pb, *pa],
    shown,
  }))
}
