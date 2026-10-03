//! WDB token 0x3f: the two original normalized texture-coordinate rates.
use crate::tok::{self, Node};

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Scroll { pub rate: [f32; 2] }
pub fn for_object(bytes: &[u8], name: &str) -> Result<Scroll, String> {
  let nodes = tok::parse(bytes).map_err(|e| e.to_string())?;
  for section in [0x2e, 0x2f, 0x30] {
    let Some((count, body)) = nodes.windows(3).find_map(|row| match row {
      [Node::Keyword(k), Node::Count(count), Node::Block(body)] if *k == section => Some((*count, body)),
      _ => None,
    }) else { continue; };
    let mut remaining = body.as_slice();
    let mut found = None;
    for _ in 0..count {
      let (object, fields, consumed) = match remaining {
        [Node::Keyword(k), Node::Str(object), Node::Block(fields), ..] if *k == section =>
          (object.as_str(), fields, 3),
        [Node::Keyword(k), Node::Block(fields), ..] if *k == section => ("", fields, 2),
        _ => return Err("malformed WDB texture-scroll object".into()),
      };
      remaining = &remaining[consumed..];
      if !object.eq_ignore_ascii_case(name) { continue; }
      if found.is_some() { return Err(format!("duplicate WDB texture-scroll object {name}")); }
      let rate = match fields.iter().position(|n| *n == Node::Keyword(0x3f)) {
        None => [0.0, 0.0],
        Some(at) => match fields.get(at+1..at+3) {
          Some([Node::Float(u), Node::Float(v)]) if u.is_finite() && v.is_finite() => [*u,*v],
          _ => return Err("invalid WDB texture-scroll rates".into()),
        },
      };
      found = Some(Scroll {rate});
    }
    if !remaining.is_empty() { return Err("WDB texture-scroll object count mismatch".into()); }
    if let Some(scroll) = found { return Ok(scroll); }
  }
  Err(format!("missing WDB texture-scroll object {name}"))
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn original_powerup_and_planet_uv_rates_are_owned_by_the_named_object() {
    let library = crate::library::Library::open(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
      .join("../../extracted/Program_Files_Group/LEGO.JAM")).unwrap();
    let world = library.find_in("POWERUP.WDB", "COMMON").unwrap();
    for (name, expected) in [("TurboL0", [0.0,0.0]),("turb0f1",[0.0,4.0]),("turb0f2",[0.0,2.0]),
      ("shield0",[0.0,4.0]),("shldin0",[0.0,4.0])] {
      assert_eq!(for_object(world,name).unwrap().rate, expected, "{name}");
    }
    assert_eq!(for_object(library.find_in("BACKGRD.WDB","RACEC3R0").unwrap(),"planet").unwrap().rate,[0.3,0.0]);
    assert!(for_object(world,"missing-effect").is_err());
    // Every original animated binding can independently retrieve its rates.
    for source in crate::world_animation::parse(world).unwrap() { for_object(world,&source.name).unwrap(); }
  }
  #[test]
  fn original_race_worlds_resolve_each_placed_objects_uv_rates() {
    let library = crate::library::Library::open(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
      .join("../../extracted/Program_Files_Group/LEGO.JAM")).unwrap();
    let mut scrolls = 0;
    for table in library.jam().tables.iter().filter(|t| t.name.starts_with("RACEC")) {
      for entry in table.entries.iter().filter(|e| e.name.ends_with(".WDB")) {
        let bytes = library.jam().bytes(entry).unwrap();
        let names = crate::world::named_instances(bytes).unwrap().into_iter().map(|o| o.name)
          .chain(crate::world_animation::parse(bytes).unwrap().into_iter().map(|o| o.name));
        for name in names {
          let rates = for_object(bytes,&name).unwrap().rate;
          if rates != [0.0,0.0] { scrolls += 1; }
        }
      }
    }
    assert!(scrolls > 4, "placed original scrolls: {scrolls}");
  }
}
