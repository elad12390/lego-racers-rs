use lrformats::{library::Library, scene::SceneBindings, world_billboards};
fn library() -> Library {
  Library::open(
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
      .join("../../extracted/Program_Files_Group/LEGO.JAM"),
  )
  .unwrap()
}

#[test]
fn royal_knights_original_tree_billboards_keep_dimensions_axis_and_transparency() {
  let library = library();
  let bytes = library.find_in("RRTRK.WDB", "RACEC0R0").unwrap();
  let sprites = world_billboards::parse(bytes).unwrap();
  assert_eq!(sprites.len(), 8);
  let first = &sprites[0];
  assert!(matches!(&first.material,world_billboards::MaterialRef::Name(n) if n=="rktree"));
  assert_eq!(first.axis, Some([0.0, 0.0, 1.0]));
  assert!((first.width - 20.0).abs() < 0.0001);
  assert!((first.height - 25.493645).abs() < 0.0001);
  assert_eq!(first.range, 500.0);
  let bindings = SceneBindings::load(&library, "RACEC0R0", bytes).unwrap();
  assert_eq!(
    bindings.materials["rktree"].texture.as_deref(),
    Some("rndtree")
  );
  let texture = &bindings.textures["rndtree"];
  assert!(!texture.targa);
  assert!(texture.color_key.is_some());
  let image = lrformats::bmp::decode(library.find_in("RNDTREE.BMP", "RACEC0R0").unwrap()).unwrap();
  assert!(image
    .to_rgba()
    .chunks_exact(4)
    .any(|p| p[..3] == texture.color_key.unwrap()));
}

#[test]
fn shipped_race_world_sprites_resolve_their_declared_image_catalogs() {
  let library = library();
  let mut total = 0;
  let mut worlds = 0;
  for table in library
    .jam()
    .tables
    .iter()
    .filter(|t| t.group.eq_ignore_ascii_case("GAMEDATA") && t.name.starts_with("RACEC"))
  {
    for entry in table
      .entries
      .iter()
      .filter(|e| e.name.to_ascii_uppercase().ends_with(".WDB"))
    {
      let bytes = library.jam().bytes(entry).unwrap();
      let sprites = world_billboards::parse(bytes)
        .unwrap_or_else(|e| panic!("{}/{}: {e}", table.name, entry.name));
      if sprites.is_empty() {
        continue;
      }
      worlds += 1;
      let bindings = SceneBindings::load(&library, &table.name, bytes).unwrap();
      for sprite in sprites {
        let material = bindings
          .sprite_material(&sprite.material)
          .unwrap_or_else(|e| panic!("{}/{}: {e}", table.name, entry.name));
        let name = material.texture.as_ref().unwrap();
        let texture = bindings
          .textures
          .get(&name.to_ascii_lowercase())
          .unwrap_or_else(|| {
            panic!(
              "{}/{}: missing sprite texture {name}",
              table.name, entry.name
            )
          });
        let file = format!("{name}.{}", if texture.targa { "TGA" } else { "BMP" });
        assert!(
          library
            .find_in(&file, &table.name)
            .or_else(|| library.find_in(&file, "COMMON"))
            .is_some(),
          "missing {}/{file}",
          table.name
        );
        total += 1;
      }
    }
  }
  assert_eq!((worlds, total), (9, 135));
}
