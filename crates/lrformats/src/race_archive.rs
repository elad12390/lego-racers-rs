//! Original RAB checkpoint-resource binding (Race archive keyword0x48).
use crate::tok::{self, Node};

pub struct Checkpoints {
  pub file: String,
  pub collider: String,
}

/// RAB's background world is a distinct camera-relative scene, not road scenery.
pub fn background_world(bytes: &[u8]) -> Result<Option<String>, String> {
  let nodes = tok::parse(bytes).map_err(|e| e.to_string())?;
  let body = nodes.iter().find_map(|node| match node {
    Node::Block(body) => Some(body), _ => None,
  }).ok_or("missing RAB body")?;
  match crate::named_records::value(body, 0x27) {
    Some(Node::Str(name)) if !name.is_empty() => {
      let stem = name.rsplit_once('.').map_or(name.as_str(), |(stem, _)| stem);
      Ok(Some(format!("{stem}.WDB")))
    }
    None | Some(Node::Str(_)) => Ok(None),
    _ => Err("invalid RAB background world name".into()),
  }
}

pub fn intro_camera(bytes: &[u8]) -> Result<Option<String>, String> {
  let nodes = tok::parse(bytes).map_err(|e| e.to_string())?;
  let body = nodes
    .iter()
    .find_map(|n| match n {
      Node::Block(body) => Some(body),
      _ => None,
    })
    .ok_or("missing RAB body")?;
  match crate::named_records::value(body, 0x49) {
    Some(Node::Str(name)) if !name.is_empty() => Ok(Some(name.clone())),
    None | Some(Node::Str(_)) => Ok(None),
    _ => Err("invalid RAB intro camera name".into()),
  }
}

/// Game archive collision binding (0x2b, packed string layout0x17).
pub struct Collisions {
  pub file: String,
  pub primary: String,
  pub start_surface: String,
}

pub fn collisions(bytes: &[u8]) -> Result<Collisions, String> {
  let nodes = tok::parse(bytes).map_err(|e| e.to_string())?;
  let body = nodes
    .iter()
    .find_map(|n| {
      if let Node::Block(b) = n {
        Some(b)
      } else {
        None
      }
    })
    .ok_or("missing RAB body")?;
  let mut records = body.iter().filter_map(|n| match n {
    Node::StringRecord { kind: 0x17, fields } if fields.len() == 3 => Some(fields),
    _ => None,
  });
  let fields = records.next().ok_or("missing RAB collision binding")?;
  if records.next().is_some() {
    return Err("ambiguous RAB collision binding".into());
  }
  if !fields[0].to_ascii_lowercase().ends_with(".wdf") {
    return Err("unsupported collision world filename".into());
  }
  Ok(Collisions {
    file: format!("{}b", &fields[0][..fields[0].len() - 1]),
    primary: fields[1].clone(),
    start_surface: fields[2].clone(),
  })
}

pub fn checkpoints(bytes: &[u8]) -> Result<Checkpoints, String> {
  let nodes = tok::parse(bytes).map_err(|e| e.to_string())?;
  let body = nodes
    .iter()
    .find_map(|n| match n {
      Node::Block(b) => Some(b),
      _ => None,
    })
    .ok_or("missing RAB body")?;
  let mut matches = body.windows(3).filter_map(|nodes| match nodes {
    [Node::Keyword(0x48), Node::Str(file), Node::Str(collider)] => Some(Checkpoints {
      file: file.clone(),
      collider: collider.clone(),
    }),
    _ => None,
  });
  let binding = matches.next().ok_or("missing RAB checkpoint binding")?;
  if matches.next().is_some() {
    return Err("ambiguous RAB checkpoint binding".into());
  }
  // Original .CPF authoring file resolves to the shipped tokenized .CPB.
  if !binding.file.to_ascii_lowercase().ends_with(".cpf") {
    return Err("unsupported checkpoint filename".into());
  }
  Ok(Checkpoints {
    file: format!("{}b", &binding.file[..binding.file.len() - 1]),
    collider: binding.collider,
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::library::Library;
  #[test]
  fn all_shipped_race_archives_resolve_their_own_checkpoint_file_and_collider() {
    let library = Library::open(
      std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../extracted/Program_Files_Group/LEGO.JAM"),
    )
    .unwrap();
    let mut count = 0;
    for table in library
      .jam()
      .tables
      .iter()
      .filter(|t| t.name.starts_with("RACEC"))
    {
      let binding = checkpoints(
        library
          .find_in(&format!("{}.RAB", table.name), &table.name)
          .unwrap(),
      )
      .unwrap();
      assert!(library.find_in(&binding.file, &table.name).is_some());
      assert!(crate::world::collision_instances(
        library.find_in("COLLIDE.WDB", &table.name).unwrap()
      )
      .unwrap()
      .iter()
      .any(|c| c.model.eq_ignore_ascii_case(&binding.collider)));
      count += 1;
    }
    assert_eq!(count, 13);
  }

  #[test]
  fn all_thirteen_rab_primary_collision_bindings_resolve_their_named_meshes() {
    let library = Library::open(
      std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../extracted/Program_Files_Group/LEGO.JAM"),
    )
    .unwrap();
    let mut count = 0;
    for table in library
      .jam()
      .tables
      .iter()
      .filter(|t| t.name.starts_with("RACEC"))
    {
      let binding = collisions(
        library
          .find_in(&format!("{}.RAB", table.name), &table.name)
          .unwrap(),
      )
      .unwrap();
      let placements =
        crate::world::collision_instances(library.find_in(&binding.file, &table.name).unwrap())
          .unwrap();
      let primary = placements
        .iter()
        .find(|p| p.model.eq_ignore_ascii_case(&binding.primary))
        .unwrap();
      assert!(library
        .find_in(&format!("{}.BVB", binding.primary), &table.name)
        .is_some());
      assert!(primary.position.into_iter().all(f32::is_finite));
      assert!(!binding.start_surface.is_empty());
      count += 1;
    }
    assert_eq!(count, 13);
  }
}
