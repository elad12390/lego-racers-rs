use lrformats::{cinematic::Timeline, library::Library};
#[test]
fn captain_redbeard_preview_uses_original_scene_camera_and_character_track() {
  let library = Library::open(
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
      .join("../../extracted/Program_Files_Group/LEGO.JAM"),
  )
  .unwrap();
  let scenes = Timeline::parse(
    library
      .find_at("CIR1-01.CDB", "MENUDATA", "CIRCUIT1")
      .unwrap(),
  )
  .unwrap();
  let scene = &scenes[0];
  assert_eq!(scene.fps, 30.0);
  assert_eq!(scene.duration, 490);
  assert_eq!(scene.cameras[0].name, "Camera01");
  let guy = scene.objects.iter().find(|o| o.name == "guy1").unwrap();
  assert!(guy.animated);
  assert_eq!(guy.position, [17.856056, -10.502232, 1.463949]);
  assert_eq!(scene.objects.len(), 10);
}
