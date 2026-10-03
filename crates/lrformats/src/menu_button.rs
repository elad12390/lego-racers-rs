//! Original MSB labeled-button font/image and ARGB presets (00480760).
use crate::tok::{self, Node};
use std::collections::BTreeMap;

#[derive(Debug)]
pub struct Style {
  pub images: [String; 6],
  pub fonts: [String; 6],
  pub image_colors: [[u8; 4]; 6],
  pub text_colors: [[u8; 4]; 6],
}

pub fn parse(bytes: &[u8]) -> Result<BTreeMap<String, Style>, String> {
  let nodes = tok::parse(bytes).map_err(|e| e.to_string())?;
  let mut styles = BTreeMap::new();
  for section in nodes.windows(3) {
    let [Node::Keyword(0x3e), Node::Count(count), Node::Block(body)] = section else {
      continue;
    };
    if body.len() != *count as usize * 3 {
      return Err("MSB button count mismatch".into());
    }
    for row in body.chunks_exact(3) {
      let [Node::Keyword(0x3e), Node::Str(name), Node::Block(fields)] = row else {
        return Err("invalid MSB button entry".into());
      };
      let names = |keyword| {
        fields
          .windows(2)
          .find_map(|pair| match pair {
            [Node::Keyword(k), Node::PackedStrings(values)] if *k == keyword => Some(values),
            _ => None,
          })
          .ok_or("missing MSB button names")?
          .clone()
          .try_into()
          .map_err(|_| "MSB button needs six names")
      };
      let colors = fields
        .windows(2)
        .find_map(|pair| match pair {
          [Node::Keyword(0x2a), Node::Packed { kind: 4, rows }] => Some(rows),
          _ => None,
        })
        .ok_or("missing MSB button colors")?;
      let colors = colors
        .iter()
        .flatten()
        .map(|value| {
          value
            .as_u32()
            .and_then(|v| u8::try_from(v).ok())
            .ok_or("invalid MSB color channel")
        })
        .collect::<Result<Vec<_>, _>>()?;
      if colors.len() != 52 {
        return Err("MSB button needs default and twelve ARGB colors".into());
      }
      // Token order: default tint, six icon tints, six text tints. Runtime
      // copies icon colors to descriptor+0xc0 and text colors to descriptor+0.
      let presets = |offset| {
        std::array::from_fn(|index| {
          let at = offset + index * 4;
          [colors[at + 1], colors[at + 2], colors[at + 3], colors[at]]
        })
      };
      let style = Style {
        images: names(0x28)?,
        fonts: names(0x29)?,
        image_colors: presets(4),
        text_colors: presets(28),
      };
      if styles.insert(name.to_ascii_lowercase(), style).is_some() {
        return Err(format!("duplicate MSB button {name}"));
      }
    }
  }
  if styles.is_empty() {
    return Err("MSB contains no labeled button styles".into());
  }
  Ok(styles)
}
