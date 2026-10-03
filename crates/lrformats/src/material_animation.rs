//! Original MAB named material keys, channel spans and frame rates.
use crate::{
  named_records,
  tok::{self, Node},
};
/// Resolve the CDB world/table indices against that world's own WDB list.
/// The first integer is not a material kind, and the fifth is not time.
pub fn load_track(
  library: &crate::library::Library,
  table: &str,
  worlds: &[String],
  reference: &crate::cinematic::MaterialTrack,
) -> Result<Animation, String> {
  let world = worlds
    .get(reference.world)
    .ok_or("CDB material world outside WDB list")?;
  let data = library
    .find_at(&format!("{world}.WDB"), "MENUDATA", table)
    .ok_or_else(|| format!("missing material world {table}/{world}.WDB"))?;
  let nodes = tok::parse(data).map_err(|e| e.to_string())?;
  let names = nodes
    .windows(3)
    .find_map(|v| {
      if let [Node::Keyword(0x3d), Node::Count(_), Node::Block(body)] = v {
        Some(
          body
            .iter()
            .flat_map(|n| match n {
              Node::Str(name) => vec![name.as_str()],
              Node::PackedStrings(names) => names.iter().map(String::as_str).collect(),
              _ => Vec::new(),
            })
            .collect::<Vec<_>>(),
        )
      } else {
        None
      }
    })
    .ok_or("missing WDB material animation list")?;
  let name = names
    .get(reference.table)
    .ok_or("CDB material table outside world MAB list")?;
  let data = library
    .find_at(&format!("{name}.MAB"), "MENUDATA", table)
    .ok_or_else(|| format!("missing original MAB {table}/{name}"))?;
  let animation = Animation::parse(data)?;
  if reference.channel >= animation.channels.len() {
    return Err("CDB channel outside MAB resource".into());
  }
  if reference.level >= 3 {
    return Err("CDB material model level outside original three levels".into());
  }
  Ok(animation)
}
pub struct Key {
  pub name: String,
  pub frame: u32,
}
pub struct Channel {
  pub start: usize,
  pub count: usize,
  pub duration: u32,
  pub fps: f32,
}
pub struct Animation {
  pub keys: Vec<Key>,
  pub channels: Vec<Channel>,
}
impl Animation {
  pub fn parse(bytes: &[u8]) -> Result<Self, String> {
    let nodes = tok::parse(bytes).map_err(|e| e.to_string())?;
    let section = |key| {
      nodes
        .windows(3)
        .find_map(|v| {
          if let [Node::Keyword(k), Node::Count(count), Node::Block(body)] = v {
            (*k == key).then_some((*count as usize, body.as_slice()))
          } else {
            None
          }
        })
        .ok_or("missing MAB table")
    };
    let (count, body) = section(0x27)?;
    let mut keys = Vec::new();
    for row in body.chunks_exact(2) {
      let [Node::Str(name), Node::Int(frame)] = row else {
        return Err("invalid MAB key".into());
      };
      keys.push(Key {
        name: name.clone(),
        frame: u32::try_from(*frame).map_err(|_| "negative MAB frame")?,
      });
    }
    if body.len() % 2 != 0 || keys.len() != count {
      return Err("MAB key count mismatch".into());
    }
    let (count, body) = section(0x28)?;
    let mut channels = Vec::new();
    for row in body.chunks_exact(2) {
      let [Node::Keyword(0x28), Node::Block(fields)] = row else {
        return Err("invalid MAB channel".into());
      };
      let (start, count) = fields
        .windows(3)
        .find_map(|v| {
          if let [Node::Keyword(0x27), Node::Int(start), Node::Int(count)] = v {
            Some((*start, *count))
          } else {
            None
          }
        })
        .ok_or("missing MAB channel span")?;
      let start = usize::try_from(start).map_err(|_| "negative MAB channel start")?;
      let count = usize::try_from(count).map_err(|_| "negative MAB channel count")?;
      let duration = u32::try_from(named_records::integer(fields, 0x29)?)
        .map_err(|_| "negative MAB duration")?;
      let fps = named_records::integer(fields, 0x2a)? as f32;
      let keys = keys
        .get(start..start.checked_add(count).ok_or("MAB range overflow")?)
        .ok_or("MAB channel outside key pool")?;
      if count == 0
        || duration == 0
        || fps <= 0.0
        || keys.windows(2).any(|k| k[1].frame < k[0].frame)
      {
        return Err("invalid MAB channel timing".into());
      }
      channels.push(Channel {
        start,
        count,
        duration,
        fps,
      });
    }
    if body.len() % 2 != 0 || channels.len() != count {
      return Err("MAB channel count mismatch".into());
    }
    Ok(Self { keys, channels })
  }
  pub fn sample(&self, channel: usize, seconds: f32) -> Result<&str, String> {
    let channel = self
      .channels
      .get(channel)
      .ok_or("MAB channel outside original table")?;
    let keys = &self.keys[channel.start..channel.start + channel.count];
    let frame = (seconds * channel.fps)
      .max(0.0)
      .rem_euclid(channel.duration as f32);
    // Original 004104c0 falls back to the LAST key when no key has fired.
    Ok(
      &keys
        .iter()
        .rev()
        .find(|k| k.frame as f32 <= frame)
        .unwrap_or(keys.last().unwrap())
        .name,
    )
  }
}
