use lrformats::{lap_events, library::Library, world};
use std::path::PathBuf;

#[test]
fn original_c0r0_lap_events_and_finish_placement_are_resolved() {
  let library = Library::open(
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../extracted/Program_Files_Group/LEGO.JAM"),
  )
  .unwrap();
  let modes = lap_events::modes(library.find("EVENT.EVB", Some("RACEC0R0")).unwrap()).unwrap();
  assert_eq!(modes.len(), 3);
  assert_eq!((modes[&100], modes[&101], modes[&102]), (0, 1, 2));
  let materials =
    lap_events::material_events(library.find("RACEC0R0.TMB", Some("RACEC0R0")).unwrap()).unwrap();
  assert_eq!(materials["rkstan"], 101);
  let placements =
    world::collision_instances(library.find("COLLIDE.WDB", Some("RACEC0R0")).unwrap()).unwrap();
  let finish = placements.iter().find(|i| i.model == "startfin").unwrap();
  assert!((finish.position[0] - 362.054).abs() < 0.001);
  assert_eq!(finish.forward, [1.0, 0.0, 0.0]);
  assert_eq!(finish.up, [0.0, 0.0, 1.0]);
}

#[test]
fn original_trigger_materials_cannot_block_chassis_or_support_wheels() {
  let library = Library::open(
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../extracted/Program_Files_Group/LEGO.JAM"),
  )
  .unwrap();
  let flags =
    lrformats::collision_materials::parse(library.find("RACEC0R0.TMB", Some("RACEC0R0")).unwrap())
      .unwrap();
  assert_eq!(flags["out"] & 0x30000, 0x30000);
  assert_eq!(flags["in"] & 0x30000, 0x30000);
  assert_eq!(flags["rkstan"] & 0x30000, 0x30000);
  assert_eq!(flags["go_thru"] & 0x30000, 0x20000);
  assert_eq!(flags["grass"] & 0x30000, 0);
}

#[test]
fn race_local_lookup_never_borrows_another_tracks_trigger_file() {
  let library = Library::open(
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../extracted/Program_Files_Group/LEGO.JAM"),
  )
  .unwrap();
  assert!(library.find_in("MAINTRIG.TRB", "RACEC0R2").is_none());
  assert!(library.find_in("NEWTRIG.TRB", "RACEC0R2").is_some());
}
