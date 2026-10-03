//! WDB per-object MAB bindings: table, channel, target slot, model level.
use crate::{
  library::Library,
  material_animation::Animation,
  tok::{self, Node},
};

#[derive(Debug, Clone)]
pub struct Reference {
  pub table: usize,
  pub channel: usize,
  pub slot: usize,
  pub level: usize,
}
pub struct Object {
  pub name: String,
  pub references: Vec<Reference>,
}
pub fn parse(bytes: &[u8]) -> Result<Vec<Object>, String> {
  let nodes = tok::parse(bytes).map_err(|e| e.to_string())?;
  let mut objects = Vec::new();
  for section in [0x2e, 0x2f, 0x30] {
    let Some(body) = nodes.windows(3).find_map(|v| {
      if let [Node::Keyword(k), Node::Count(_), Node::Block(body)] = v {
        (*k == section).then_some(body)
      } else {
        None
      }
    }) else {
      continue;
    };
    for row in body.chunks_exact(3) {
      let [Node::Keyword(k), Node::Str(name), Node::Block(fields)] = row else {
        return Err("invalid WDB material object".into());
      };
      if *k != section {
        return Err("WDB material object kind mismatch".into());
      }
      let Some((count, bindings)) = fields.windows(3).find_map(|v| {
        if let [Node::Keyword(0x3e), Node::Count(count), Node::Block(body)] = v {
          Some((*count, body))
        } else {
          None
        }
      }) else {
        continue;
      };
      let mut values = Vec::new();
      for node in bindings {
        match node {
          Node::Int(value) => {
            values.push(usize::try_from(*value).map_err(|_| "negative WDB material reference")?)
          }
          Node::Packed { rows, .. } => {
            for value in rows.iter().flatten() {
              values.push(
                value
                  .as_u32()
                  .ok_or("invalid packed WDB material reference")? as usize,
              );
            }
          }
          _ => return Err("invalid WDB material binding".into()),
        }
      }
      if values.len() != count as usize * 4 {
        return Err("WDB material binding count mismatch".into());
      }
      let references = values
        .chunks_exact(4)
        .map(|v| Reference {
          table: v[0],
          channel: v[1],
          slot: v[2],
          level: v[3],
        })
        .collect();
      objects.push(Object {
        name: name.clone(),
        references,
      });
    }
    if body.len() % 3 != 0 {
      return Err("incomplete WDB material object".into());
    }
  }
  Ok(objects)
}
pub fn load(
  library: &Library,
  owner: &str,
  world: &[u8],
  reference: &Reference,
) -> Result<Animation, String> {
  let nodes = tok::parse(world).map_err(|e| e.to_string())?;
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
    .ok_or("WDB material table outside MAB list")?;
  let filename = format!("{name}.MAB");
  let data = library
    .find_in(&filename, owner)
    .or_else(|| library.find_in(&filename, "COMMON"))
    .ok_or_else(|| format!("missing original MAB {owner}/{filename}"))?;
  let animation = Animation::parse(data)?;
  if reference.channel >= animation.channels.len() {
    return Err("WDB channel outside MAB resource".into());
  }
  if reference.level >= 3 {
    return Err("WDB material model level outside original three levels".into());
  }
  Ok(animation)
}
