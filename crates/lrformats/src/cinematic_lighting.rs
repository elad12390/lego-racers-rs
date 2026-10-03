//! CDB ambient/directional light intervals in original world coordinates.
use crate::{named_records, tok::Node};

pub struct Track {
  pub start: u32,
  pub end: u32,
  pub color: [u8; 3],
  pub direction: Option<[f32; 3]>,
}
#[derive(Default)]
pub struct Timeline {
  pub tracks: Vec<Track>,
}
pub struct Lighting {
  pub ambient: [u8; 3],
  pub directional: Vec<([u8; 3], [f32; 3])>,
}

impl Timeline {
  pub fn parse(fields: &[Node]) -> Result<Self, String> {
    let mut tracks = Vec::new();
    for key in [0x35, 0x3a] {
      let Some(body) = fields.windows(3).find_map(|v| {
        if let [Node::Keyword(k), Node::Count(_), Node::Block(body)] = v {
          (*k == key).then_some(body)
        } else {
          None
        }
      }) else {
        continue;
      };
      for row in body.chunks_exact(3) {
        let [Node::Keyword(k), Node::Str(_), Node::Block(fields)] = row else {
          return Err("invalid CDB light track".into());
        };
        if *k != key {
          return Err("unexpected CDB light record".into());
        }
        let start = u32::try_from(named_records::integer(fields, 0x2b)?)
          .map_err(|_| "negative CDB light start")?;
        let duration = u32::try_from(named_records::integer(fields, 0x2c)?)
          .map_err(|_| "negative CDB light duration")?;
        let at = fields
          .iter()
          .position(|v| *v == Node::Keyword(0x38))
          .ok_or("missing CDB light RGB")?;
        let color = fields
          .get(at + 1..at + 4)
          .ok_or("truncated CDB light RGB")?
          .iter()
          .map(|n| match n {
            Node::Int(v) => u8::try_from(*v).map_err(|_| "invalid CDB light RGB"),
            _ => Err("invalid CDB light RGB"),
          })
          .collect::<Result<Vec<_>, _>>()?
          .try_into()
          .unwrap();
        let direction = if key == 0x3a {
          let at = fields
            .iter()
            .position(|v| *v == Node::Keyword(0x39))
            .ok_or("missing CDB light direction")?;
          let values = fields
            .get(at + 1..at + 4)
            .ok_or("truncated CDB light direction")?
            .iter()
            .map(|n| match n {
              Node::Float(v) if v.is_finite() => Ok(*v),
              _ => Err("invalid CDB light direction"),
            })
            .collect::<Result<Vec<_>, _>>()?;
          let length = values.iter().map(|v| v * v).sum::<f32>().sqrt();
          if length == 0.0 {
            return Err("zero CDB light direction".into());
          }
          Some(std::array::from_fn(|i| values[i] / length))
        } else {
          None
        };
        tracks.push(Track {
          start,
          end: start
            .checked_add(duration)
            .ok_or("CDB light interval overflow")?,
          color,
          direction,
        });
      }
      if body.len() % 3 != 0 {
        return Err("incomplete CDB light track".into());
      }
    }
    Ok(Self { tracks })
  }
  pub fn sample(&self, frame: f32) -> Lighting {
    let active = self
      .tracks
      .iter()
      .filter(|t| frame >= t.start as f32 && frame < t.end as f32)
      .collect::<Vec<_>>();
    let ambient = active
      .iter()
      .rev()
      .find(|t| t.direction.is_none())
      .map_or([255; 3], |t| t.color);
    let directional = active
      .into_iter()
      .filter_map(|t| t.direction.map(|direction| (t.color, direction)))
      .take(7)
      .collect();
    Lighting {
      ambient,
      directional,
    }
  }
}
impl Lighting {
  /// Original 0040ede0: subtract each truncated light-dot contribution,
  /// then clamp each channel between its ambient value and 255. Light
  /// vectors point into the surface, so positive dots do not brighten it.
  pub fn shade(&self, normal: [f32; 3]) -> [u8; 3] {
    let mut color = self.ambient.map(i32::from);
    for (rgb, direction) in &self.directional {
      let dot = normal
        .iter()
        .zip(direction)
        .map(|(a, b)| a * b)
        .sum::<f32>();
      for i in 0..3 {
        color[i] -= (dot * f32::from(rgb[i])) as i32;
      }
    }
    std::array::from_fn(|i| color[i].clamp(i32::from(self.ambient[i]), 255) as u8)
  }
}
