//! Authored MIB item-list slots and projection; UiItemList/UiButtonRow inputs.
use crate::tok::{Node, Value};

#[derive(Debug)]
pub struct Selector {
  pub selected_slot: usize,
  /// Third keyword33 value; timing is supplied separately by the list style.
  pub option: f32,
  pub slots: Vec<[i32; 4]>,
  pub fov_degrees: f32,
  pub near: f32,
  pub far: f32,
}

pub fn parse(fields: &[Node]) -> Result<Selector, String> {
  let (count, selected, option) = fields
    .windows(4)
    .find_map(|s| match s {
      [Node::Keyword(0x33), Node::Int(count), Node::Int(selected), Node::Float(option)] => {
        Some((*count, *selected, *option))
      }
      _ => None,
    })
    .ok_or("missing MIB selector slot count/origin/option")?;
  if count <= 0 || selected < 0 || selected >= count || !option.is_finite() {
    return Err("invalid MIB selector count/origin/option".into());
  }
  let values = fields
    .windows(2)
    .find_map(|s| match s {
      [Node::Keyword(0x2f), Node::Packed { kind: 4, rows }] => Some(rows),
      _ => None,
    })
    .ok_or("missing MIB selector slots")?
    .iter()
    .flatten()
    .map(|v| match v {
      Value::I32(v) => Ok(*v),
      _ => Err("MIB selector slot is not integer"),
    })
    .collect::<Result<Vec<_>, _>>()?;
  if values.len() != count as usize * 4 {
    return Err("MIB selector slot count mismatch".into());
  }
  let slots = values
    .chunks_exact(4)
    .map(|r| [r[0], r[1], r[2], r[3]])
    .collect::<Vec<_>>();
  if slots.iter().any(|r| r[2] <= r[0] || r[3] <= r[1]) {
    return Err("invalid MIB selector slot bounds".into());
  }
  let camera = fields
    .windows(2)
    .find_map(|s| match s {
      [Node::Keyword(0x2e), Node::Packed { kind: 3, rows }] => Some(rows),
      _ => None,
    })
    .ok_or("missing MIB selector projection")?
    .iter()
    .flatten()
    .map(|v| match v {
      Value::F32(v) if v.is_finite() => Ok(*v),
      _ => Err("invalid MIB selector projection float"),
    })
    .collect::<Result<Vec<_>, _>>()?;
  if camera.len() != 9
    || !(0.0..180.0).contains(&camera[6])
    || camera[6] == 0.0
    || camera[7] <= 0.0
    || camera[8] <= camera[7]
  {
    return Err("invalid MIB selector projection".into());
  }
  Ok(Selector {
    selected_slot: selected as usize,
    option,
    slots,
    fov_degrees: camera[6],
    near: camera[7],
    far: camera[8],
  })
}

/// UiItemList context+0c from the named list style, NOT the MIB option float.
pub fn scroll_duration(bytes: &[u8], name: &str) -> Result<u32, String> {
  let nodes = crate::tok::parse(bytes).map_err(|e| e.to_string())?;
  for section in nodes.windows(3) {
    if let [Node::Keyword(0x38), Node::Count(_), Node::Block(body)] = section {
      for entry in body.chunks_exact(3) {
        if let [Node::Keyword(_), Node::Str(found), Node::Block(fields)] = entry {
          if found.eq_ignore_ascii_case(name) {
            return match crate::named_records::value(fields, 0x2d) {
              Some(Node::Int(value)) if *value > 0 => Ok(*value as u32),
              _ => Err("invalid original selector scroll duration".into()),
            };
          }
        }
      }
    }
  }
  Err(format!("missing original selector style {name}"))
}

#[cfg(test)]
mod tests {
  use super::*;
  fn fields(count: i32, selected: i32, slots: Vec<i32>, projection: Vec<f32>) -> Vec<Node> {
    vec![
      Node::Keyword(0x33),
      Node::Int(count),
      Node::Int(selected),
      Node::Float(0.0),
      Node::Keyword(0x2f),
      Node::Packed {
        kind: 4,
        rows: slots.into_iter().map(|v| vec![Value::I32(v)]).collect(),
      },
      Node::Keyword(0x2e),
      Node::Packed {
        kind: 3,
        rows: projection
          .into_iter()
          .map(|v| vec![Value::F32(v)])
          .collect(),
      },
    ]
  }
  #[test]
  fn selector_rejects_invalid_count_origin_bounds_and_projection() {
    let camera = vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 4.0, 2.0, 275.0];
    let slot = vec![0, 0, 32, 32];
    assert!(parse(&fields(1, 0, slot.clone(), camera.clone())).is_ok());
    for (count, origin, slots) in [
      (0, 0, slot.clone()),
      (1, -1, slot.clone()),
      (1, 1, slot.clone()),
      (2, 0, slot.clone()),
      (1, 0, vec![0, 0, 0, 32]),
      (1, 0, vec![0, 32, 32, 32]),
    ] {
      assert!(parse(&fields(count, origin, slots, camera.clone())).is_err());
    }
    for (field, value) in [
      (6, 0.0),
      (6, 180.0),
      (6, f32::NAN),
      (7, 0.0),
      (8, 2.0),
      (8, f32::INFINITY),
    ] {
      let mut invalid = camera.clone();
      invalid[field] = value;
      assert!(parse(&fields(1, 0, slot.clone(), invalid)).is_err());
    }
    assert!(parse(&fields(1, 0, slot, camera[..8].to_vec())).is_err());
  }
}
