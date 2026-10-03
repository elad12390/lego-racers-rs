use lrformats::{library::Library, sky};
use std::path::PathBuf;

#[test]
fn original_default_sky_profile_and_colors_are_preserved() {
  let library = Library::open(
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../extracted/Program_Files_Group/LEGO.JAM"),
  )
  .unwrap();
  let sky = sky::parse(library.find_in("BACKGRND.SKB", "RACEC0R0").unwrap()).unwrap();
  assert_eq!(sky.default, "openair");
  assert_eq!(sky.profiles.len(), 2);
  assert_eq!(
    sky.profiles[0].colors,
    [[250, 250, 120], [100, 100, 255], [25, 25, 255]]
  );
  assert_eq!(sky.profiles[1].name, "castle");
}

#[test]
fn original_rab_background_worlds_are_distinct_and_only_rocket_run_places_a_sky_child() {
  let library = Library::open(
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../extracted/Program_Files_Group/LEGO.JAM"),
  ).unwrap();
  let mut worlds = 0;
  let mut children = Vec::new();
  for table in library.jam().tables.iter().filter(|t| t.name.starts_with("RACEC")) {
    let Some(rab) = table.entries.iter().find(|e| e.name.ends_with(".RAB")) else { continue; };
    let name = lrformats::race_archive::background_world(library.jam().bytes(rab).unwrap()).unwrap().unwrap();
    assert_eq!(name.to_ascii_uppercase(), "BACKGRD.WDB");
    let bytes = library.find_in(&name, &table.name).unwrap();
    for child in lrformats::world::named_instances(bytes).unwrap() {
      children.push((table.name.clone(), child.name));
    }
    assert!(lrformats::world_animation::parse(bytes).unwrap().is_empty());
    worlds += 1;
  }
  assert_eq!(worlds, 13);
  assert_eq!(children, vec![("RACEC3R0".to_string(), "planet".to_string())]);
}
