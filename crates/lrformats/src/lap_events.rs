//! Original EVB lap block modes and TMB material event associations.
//! EffectEventTable::ReadBlocks00461990: default1; keyword36=>0,37=>2.
use crate::tok::{self, Node, TokError};
use std::collections::HashMap;

pub fn modes(data: &[u8]) -> Result<HashMap<i32, u32>, TokError> {
  let nodes = tok::parse(data)?;
  let Some(body) = statement(&nodes, 0x51) else {
    return Ok(HashMap::new());
  };
  Ok(
    body
      .windows(3)
      .filter_map(|triple| {
        let [Node::Keyword(0x27), Node::Int(id), Node::Block(fields)] = triple else {
          return None;
        };
        let mode = fields.iter().fold(1, |mode, n| match n {
          Node::Keyword(0x36) => 0,
          Node::Keyword(0x37) => 2,
          _ => mode,
        });
        Some((*id, mode))
      })
      .collect(),
  )
}

pub fn material_events(data: &[u8]) -> Result<HashMap<String, i32>, TokError> {
  let nodes = tok::parse(data)?;
  let Some(body) = statement(&nodes, 0x27) else {
    return Ok(HashMap::new());
  };
  Ok(
    body
      .windows(3)
      .filter_map(|triple| {
        let [Node::Keyword(0x27), Node::Str(name), Node::Block(fields)] = triple else {
          return None;
        };
        fields.windows(2).find_map(|pair| match pair {
          [Node::Keyword(0x2a), Node::Int(id)] => Some((name.to_ascii_lowercase(), *id)),
          _ => None,
        })
      })
      .collect(),
  )
}

fn statement(nodes: &[Node], keyword: u8) -> Option<&[Node]> {
  let index = nodes.iter().position(|n| *n == Node::Keyword(keyword))?;
  nodes[index + 1..].iter().find_map(|n| {
    if let Node::Block(body) = n {
      Some(body.as_slice())
    } else {
      None
    }
  })
}
