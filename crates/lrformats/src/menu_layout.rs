//! Named MIB widgets. Rectangles are retained as original inset values;
//! interpretation (including parent-relative anchoring) belongs to the UI.
pub use crate::menu_frame::Frame;
use crate::tok::{self, Node, Value};
use std::collections::BTreeMap;

#[derive(Debug)]
pub struct Widget {
  pub kind: u8,
  pub rect: Option<[i32; 4]>,
  pub parent: Option<String>,
  /// SpinControl's named scene; an existing scene camera overrides its points.
  pub scene: Option<String>,
  pub frame_name: Option<String>,
  pub frame: Option<Frame>,
  pub selector: Option<crate::menu_selector::Selector>,
}
pub struct Layout {
  pub widgets: BTreeMap<String, Widget>,
}
impl Layout {
  pub fn parse(bytes: &[u8]) -> Result<Self, String> {
    let nodes = tok::parse(bytes).map_err(|e| e.to_string())?;
    let mut widgets = BTreeMap::new();
    for section in nodes.windows(3) {
      let [Node::Keyword(kind), Node::Count(count), Node::Block(body)] = section else {
        continue;
      };
      let mut found = 0;
      for row in body.chunks_exact(3) {
        let [Node::Keyword(tag), Node::Str(name), Node::Block(fields)] = row else {
          return Err("invalid MIB widget".into());
        };
        if tag != kind {
          return Err("MIB widget type mismatch".into());
        }
        let style = fields
          .windows(2)
          .find_map(|pair| {
            if let [Node::Keyword(0x36), Node::Block(style)] = pair {
              Some(style)
            } else {
              None
            }
          })
          .unwrap_or(fields);
        let packed = style.windows(2).find_map(|pair| {
          if let [Node::Keyword(0x2f), Node::Packed { kind: 4, rows }] = pair {
            Some(rows)
          } else {
            None
          }
        });
        let rect = packed
          .map(|packed| {
            let values = packed
              .iter()
              .flatten()
              .map(|v| {
                if let Value::I32(v) = v {
                  Ok(*v)
                } else {
                  Err("MIB rectangle is not integer")
                }
              })
              .collect::<Result<Vec<_>, _>>()?;
            values
              .try_into()
              .map_err(|_| "MIB rectangle needs four insets")
          })
          .transpose()?;
        let parent = style.windows(2).find_map(|pair| {
          if let [Node::Keyword(0x31), Node::Str(name)] = pair {
            Some(name.clone())
          } else {
            None
          }
        });
        let scene = if *kind == 0x42 {
          fields.windows(2).find_map(|pair| {
            if let [Node::Keyword(0x2d), Node::Str(name)] = pair {
              Some(name.clone())
            } else {
              None
            }
          })
        } else {
          None
        };
        let frame_name = if *kind == 0x42 {
          fields.windows(2).find_map(|pair| match pair {
            [Node::Keyword(0x3a), Node::Str(name)] => Some(name.to_ascii_lowercase()),
            _ => None,
          })
        } else {
          None
        };
        let frame = if *kind == 0x3a {
          crate::menu_frame::parse_fields(fields)?
        } else {
          None
        };
        let selector = if *kind == 0x3f {
          Some(crate::menu_selector::parse(fields)?)
        } else {
          None
        };
        if widgets
          .insert(
            name.to_ascii_lowercase(),
            Widget {
              kind: *kind,
              rect,
              parent,
              scene,
              frame_name,
              frame,
              selector,
            },
          )
          .is_some()
        {
          return Err(format!("duplicate MIB widget {name}"));
        }
        found += 1;
      }
      if body.len() % 3 != 0 || found != *count {
        return Err("MIB widget count mismatch".into());
      }
    }
    if widgets.is_empty() {
      return Err("MIB layout contains no widgets".into());
    }
    Ok(Self { widgets })
  }
  pub fn frame_for(&self, widget: &str) -> Result<&Frame, String> {
    let name = self
      .widgets
      .get(widget)
      .and_then(|w| w.frame_name.as_deref())
      .ok_or("missing MIB preview frame name")?;
    self
      .widgets
      .get(name)
      .and_then(|w| w.frame.as_ref())
      .ok_or_else(|| format!("missing MIB frame style {name}"))
  }
}
