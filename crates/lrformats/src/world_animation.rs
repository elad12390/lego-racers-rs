//! Original WDB animated-object resource references and initial looping clip.
use crate::{
  named_records,
  tok::{self, Node},
  world::Instance,
};

#[derive(Debug, Clone)]
pub enum Clip {
  Index(usize),
  Name(String),
}
#[derive(Debug, Clone)]
pub struct AnimatedInstance {
  pub name: String,
  pub placement: Instance,
  pub skeleton: String,
  pub animation: String,
  pub clip: Option<Clip>,
  pub range: f32,
}

pub(crate) fn names(nodes: &[Node], key: u8) -> Vec<String> {
  nodes
    .windows(3)
    .find_map(|row| {
      if let [Node::Keyword(k), Node::Count(_), Node::Block(body)] = row {
        (*k == key).then_some(body)
      } else {
        None
      }
    })
    .into_iter()
    .flatten()
    .flat_map(|n| match n {
      Node::Str(s) => vec![s.clone()],
      Node::PackedStrings(names) => names.clone(),
      _ => Vec::new(),
    })
    .collect()
}
pub fn parse(bytes: &[u8]) -> Result<Vec<AnimatedInstance>, String> {
  let nodes = tok::parse(bytes).map_err(|e| e.to_string())?;
  let meshes = names(&nodes, 0x2a);
  let skeletons = names(&nodes, 0x2c);
  let animations = names(&nodes, 0x29);
  let Some((count, body)) = nodes.windows(3).find_map(|row| {
    if let [Node::Keyword(0x2f), Node::Count(n), Node::Block(body)] = row {
      Some((*n, body))
    } else {
      None
    }
  }) else {
    return Ok(Vec::new());
  };
  if body.len() != count as usize * 3 {
    return Err("WDB animated object count mismatch".into());
  }
  let mut out = Vec::new();
  for row in body.chunks_exact(3) {
    let [Node::Keyword(0x2f), Node::Str(name), Node::Block(fields)] = row else {
      return Err("invalid WDB animated object".into());
    };
    let reference = if let Some(values) = fields.iter().find_map(|n| {
      if let Node::Record { kind: 0x19, fields } = n {
        Some(fields)
      } else {
        None
      }
    }) {
      let [mesh, skeleton, animation, range] = values.as_slice() else {
        return Err("invalid packed WDB animated references".into());
      };
      Some((
        mesh.as_u32().ok_or("invalid WDB animated mesh")?,
        skeleton.as_u32().ok_or("invalid WDB skeleton")?,
        animation.as_u32().ok_or("invalid WDB animation")?,
        range.as_f32().ok_or("invalid WDB animated range")?,
      ))
    } else {
      fields.windows(5).find_map(|v|if let [Node::Keyword(0x33),Node::Int(mesh),Node::Int(skeleton),Node::Int(animation),Node::Float(range)]=v {Some((*mesh as u32,*skeleton as u32,*animation as u32,*range))}else {None})
    };
    // WDB camera dummies bind only skeleton/animation (0x2c), no mesh.
    // They remain camera resources, not drawable scenery.
    let Some((mesh, skeleton, animation, range)) = reference else {
      if fields.contains(&Node::Keyword(0x2c)) {
        continue;
      }
      return Err(format!(
        "{name}: unsupported WDB animated geometry reference"
      ));
    };
    let vector = |kind, len| {
      fields
        .iter()
        .find_map(|n| {
          if let Node::Record { kind: k, fields } = n {
            (*k == kind).then_some(fields)
          } else {
            None
          }
        })
        .ok_or("missing WDB animated placement")
        .and_then(|v| {
          let numbers = v
            .iter()
            .map(|v| {
              v.as_f32()
                .filter(|f| f.is_finite())
                .ok_or("invalid WDB animated placement")
            })
            .collect::<Result<Vec<_>, _>>()?;
          if numbers.len() != len {
            return Err("invalid WDB animated placement length");
          }
          Ok(numbers)
        })
    };
    let position = vector(0x17, 3)?;
    let basis = vector(0x18, 6)?;
    let clip = match named_records::value(fields, 0x35) {
      Some(Node::Int(index)) if *index >= 0 => Some(Clip::Index(*index as usize)),
      Some(Node::Int(_)) | None => None,
      Some(Node::Str(name)) => Some(Clip::Name(name.clone())),
      _ => return Err("invalid WDB initial clip".into()),
    };
    if !range.is_finite() {
      return Err("nonfinite WDB animated draw range".into());
    }
    out.push(AnimatedInstance {
      name: name.clone(),
      placement: Instance {
        model: meshes
          .get(mesh as usize)
          .ok_or("WDB animated mesh outside resource list")?
          .clone(),
        position: position.try_into().unwrap(),
        forward: basis[..3].try_into().unwrap(),
        up: basis[3..].try_into().unwrap(),
      },
      skeleton: skeletons
        .get(skeleton as usize)
        .ok_or("WDB skeleton outside resource list")?
        .clone(),
      animation: animations
        .get(animation as usize)
        .ok_or("WDB animation outside resource list")?
        .clone(),
      clip,
      range,
    });
  }
  Ok(out)
}
