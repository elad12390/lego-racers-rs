//! Shipped PWB colored-generator and white-enhancer placements.
use crate::tok::{self, Node, Value};
#[derive(Clone)]
pub struct Pickup {
  pub position: [f32; 3],
  pub kind: u8,
  pub delay_ms: u32,
}

pub fn parse(data: &[u8], mirrored: bool) -> Result<Vec<Pickup>, String> {
  let nodes = tok::parse(data).map_err(|e| e.to_string())?;
  let mut pickups = Vec::new();
  for section in nodes.windows(3) {
    let [Node::Keyword(key @ (0x27 | 0x2f)), Node::Count(count), Node::Block(rows)] = section
    else {
      continue;
    };
    let mut parsed = 0;
    for row in rows.windows(2) {
      let [Node::Keyword(row_key), Node::Block(fields)] = row else {
        continue;
      };
      if row_key != key {
        continue;
      }
      let mut position = None;
      let mut kind = if *key == 0x2f { 0 } else { 3 };
      let mut delay_ms = 3000;
      for (i, node) in fields.iter().enumerate() {
        match node {
          Node::Record { kind: 0x17, fields } if fields.len() == 3 => {
            let values = fields
              .iter()
              .map(|v| match v {
                Value::F32(v) if v.is_finite() => Ok(*v),
                _ => Err("invalid PWB position"),
              })
              .collect::<Result<Vec<_>, _>>()?;
            position = Some([
              values[0],
              if mirrored { -values[1] } else { values[1] },
              values[2],
            ]);
          }
          Node::Keyword(k) if *key == 0x27 => match k {
            0x2a => kind = 1,
            0x2b => kind = 4,
            0x2c => kind = 2,
            0x2d => kind = 3,
            0x2e => {
              if let Some(Node::Int(delay)) = fields.get(i + 1) {
                delay_ms = u32::try_from(*delay).map_err(|_| "negative PWB delay")?;
              }
            }
            _ => {}
          },
          _ => {}
        }
      }
      pickups.push(Pickup {
        position: position.ok_or("PWB pickup position missing")?,
        kind,
        delay_ms,
      });
      parsed += 1;
    }
    if parsed != *count {
      return Err(format!("PWB count mismatch: {parsed}/{count}"));
    }
  }
  if pickups.is_empty() {
    return Err("PWB has no pickup placements".into());
  }
  Ok(pickups)
}
