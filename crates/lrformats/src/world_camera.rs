//! Original WDB camera table and exact animated-object/joint bindings.
use crate::{
  named_records,
  tok::{self, Node},
  world::Instance,
};

#[derive(Debug)]
pub struct Rig {
  pub skeleton: String,
  pub animation: String,
  pub joint: usize,
  pub placement: Instance,
}
#[derive(Debug)]
pub struct Camera {
  pub placement: Instance,
  pub near: f32,
  pub far: f32,
  pub fov_degrees: f32,
  pub rig: Option<Rig>,
}

fn table(nodes: &[Node], key: u8) -> Result<&[Node], String> {
  let Some((count, body)) = nodes.windows(3).find_map(|row| match row {
    [Node::Keyword(k), Node::Count(count), Node::Block(body)] if *k == key => Some((*count, body)),
    _ => None,
  }) else {
    return Ok(&[]);
  };
  if body.len() != count as usize * 3 {
    return Err("WDB camera/object count mismatch".into());
  }
  Ok(body)
}
fn vector<const N: usize>(
  fields: &[Node],
  kind: u8,
  default: [f32; N],
) -> Result<[f32; N], String> {
  let Some(values) = fields.iter().find_map(|n| match n {
    Node::Record { kind: k, fields } if *k == kind => Some(fields),
    _ => None,
  }) else {
    return Ok(default);
  };
  values
    .iter()
    .map(|v| {
      v.as_f32()
        .filter(|f| f.is_finite())
        .ok_or("invalid WDB camera vector")
    })
    .collect::<Result<Vec<_>, _>>()?
    .try_into()
    .map_err(|_| "invalid WDB camera vector length".into())
}
fn placement(fields: &[Node], name: &str) -> Result<Instance, String> {
  let orientation = vector(fields, 0x18, [1.0, 0.0, 0.0, 0.0, 0.0, 1.0])?;
  Ok(Instance {
    model: name.into(),
    position: vector(fields, 0x17, [0.0; 3])?,
    forward: orientation[..3].try_into().unwrap(),
    up: orientation[3..].try_into().unwrap(),
  })
}
fn float(fields: &[Node], key: u8) -> Result<f32, String> {
  match named_records::value(fields, key) {
    Some(Node::Float(v)) if v.is_finite() => Ok(*v),
    _ => Err(format!("missing WDB camera field {key:02x}")),
  }
}
pub fn parse(bytes: &[u8]) -> Result<Vec<Camera>, String> {
  let nodes = tok::parse(bytes).map_err(|e| e.to_string())?;
  let objects = table(&nodes, 0x2f)?;
  let skeletons = crate::world_animation::names(&nodes, 0x2c);
  let animations = crate::world_animation::names(&nodes, 0x29);
  table(&nodes, 0x43)?.chunks_exact(3).map(|row| {
    let [Node::Keyword(0x43), Node::Str(name), Node::Block(fields)] = row else {
      return Err("invalid WDB camera entry".into());
    };
    let binding = fields.windows(3).find_map(|v| match v {
      [Node::Keyword(0x2f), Node::Int(object), Node::Int(joint)] => Some((*object, *joint)), _ => None,
    });
    let rig = if let Some((object, joint)) = binding {
      let index = usize::try_from(object).map_err(|_| "negative WDB camera object")?;
      let joint = usize::try_from(joint).map_err(|_| "negative WDB camera joint")?;
      let [Node::Keyword(0x2f), Node::Str(object_name), Node::Block(object_fields)] =
        objects.get(index * 3..index * 3 + 3).ok_or("WDB camera object outside world")? else {
          return Err("invalid WDB camera object entry".into());
        };
      let refs = object_fields.iter().find_map(|n| match n {
        Node::Record {kind: 0x19, fields} => {
          let [_, skeleton, animation, _] = fields.as_slice() else { return None; };
          Some((skeleton.as_u32()? as usize, animation.as_u32()? as usize))
        }, _ => None,
      }).or_else(|| object_fields.windows(4).find_map(|v| match v {
        [Node::Keyword(0x2c), Node::Int(skeleton), Node::Int(animation), Node::Float(_)] if *skeleton >= 0 && *animation >= 0 => Some((*skeleton as usize, *animation as usize)), _ => None,
      })).or_else(|| object_fields.windows(5).find_map(|v| match v {
        [Node::Keyword(0x33), Node::Int(_), Node::Int(skeleton), Node::Int(animation), Node::Float(_)] if *skeleton >= 0 && *animation >= 0 => Some((*skeleton as usize, *animation as usize)), _ => None,
      })).ok_or("unsupported WDB camera rig reference")?;
      Some(Rig { skeleton: skeletons.get(refs.0).ok_or("WDB camera skeleton outside resources")?.clone(),
        animation: animations.get(refs.1).ok_or("WDB camera animation outside resources")?.clone(),
        joint, placement: placement(object_fields, object_name)? })
    } else { None };
    let near = float(fields, 0x45)?;
    let far = float(fields, 0x46)?;
    let fov_degrees = float(fields, 0x47)?;
    if near <= 0.0 || far <= near || fov_degrees <= 0.0 || fov_degrees >= 180.0 {
      return Err("invalid WDB camera projection".into());
    }
    Ok(Camera {placement: placement(fields, name)?, near, far, fov_degrees, rig})
  }).collect()
}
