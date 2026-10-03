use lrformats::{library::Library, model::Model, scene::SceneBindings, world_material};

fn library() -> Library {
  Library::open(
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
      .join("../../extracted/Program_Files_Group/LEGO.JAM"),
  )
  .unwrap()
}

#[test]
fn magma_code_panel_uses_its_declared_channel_slot_and_original_keys() {
  let library = library();
  let world = library.find_in("MGMMN.WDB", "RACEC0R3").unwrap();
  let objects = world_material::parse(world).unwrap();
  let panel = objects.iter().find(|o| o.name == "mmcode1").unwrap();
  let reference = &panel.references[0];
  assert_eq!(
    (
      reference.table,
      reference.channel,
      reference.slot,
      reference.level
    ),
    (0, 5, 1, 0)
  );
  let animation = world_material::load(&library, "RACEC0R3", world, reference).unwrap();
  let channel = &animation.channels[reference.channel];
  let keys = &animation.keys[channel.start..channel.start + channel.count];
  assert!(keys.len() > 1);
  assert_ne!(keys.first().unwrap().name, keys.last().unwrap().name);
  for key in keys {
    assert_eq!(
      animation
        .sample(reference.channel, key.frame as f32 / channel.fps)
        .unwrap(),
      key.name
    );
  }
}

#[test]
fn all_placed_world_material_channels_resolve_their_owning_resources() {
  let library = library();
  let mut total = 0;
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
      let world = library.jam().bytes(entry).unwrap();
      let objects = world_material::parse(world)
        .unwrap_or_else(|e| panic!("{}/{}: {e}", table.name, entry.name));
      if objects.is_empty() {
        continue;
      }
      let mut bindings = SceneBindings::load(&library, &table.name, world).unwrap();
      bindings
        .inherit_named_resources(&SceneBindings::race_resources(&library, &table.name).unwrap());
      let placed = lrformats::world::named_instances(world).unwrap();
      let animated = lrformats::world_animation::parse(world).unwrap();
      for object in objects {
        let model_name = placed
          .iter()
          .find(|i| i.name.eq_ignore_ascii_case(&object.name))
          .map(|i| i.placement.model.as_str())
          .or_else(|| {
            animated
              .iter()
              .find(|i| i.name.eq_ignore_ascii_case(&object.name))
              .map(|i| i.placement.model.as_str())
          });
        let model = model_name.map(|name| {
          Model::load_with_materials(&library, name, Some(&table.name), &bindings.materials)
            .unwrap()
        });
        for reference in object.references {
          let animation = world_material::load(&library, &table.name, world, &reference).unwrap();
          if let Some(model) = &model {
            assert_eq!(
              reference.level, 0,
              "{}/{} alternate LOD",
              table.name, object.name
            );
            assert!(
              reference.slot < model.mesh.textures.len(),
              "{}/{} slot {}",
              table.name,
              object.name,
              reference.slot
            );
          }
          let channel = &animation.channels[reference.channel];
          for key in &animation.keys[channel.start..channel.start + channel.count] {
            let material = bindings
              .materials
              .get(&key.name.to_ascii_lowercase())
              .unwrap_or_else(|| {
                panic!(
                  "{}/{}/{} material {}",
                  table.name, entry.name, object.name, key.name
                )
              });
            if let Some(texture) = &material.texture {
              let image = &bindings.textures[&texture.to_ascii_lowercase()];
              let file = format!("{texture}.{}", if image.targa { "TGA" } else { "BMP" });
              assert!(library
                .find_in(&file, &table.name)
                .or_else(|| library.find_in(&file, "COMMON"))
                .is_some());
            }
          }
          total += 1;
        }
      }
    }
  }
  println!("original placed world material bindings: {total}");
  assert!(total > 0);
}
