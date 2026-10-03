use lrformats::{brick_database::BrickDatabase, library::Library};
#[test]
fn original_bricks_decode_to_renderable_faces_and_stud_footprints() {
  let library = Library::open(
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
      .join("../../extracted/Program_Files_Group/LEGO.JAM"),
  )
  .unwrap();
  let db = BrickDatabase::parse(
    library
      .find_at("LPIECELO.LEB", "MENUDATA", "PIECEDB")
      .unwrap(),
  )
  .unwrap();
  assert_eq!(db.bricks.len(), 161);
  let brick = db.find("l300300").unwrap();
  assert_eq!((brick.width, brick.depth), (2, 2));
  assert!(!brick.faces.is_empty());
  assert!(db.find("kkchas0").is_ok());
  let materials = lrformats::mdb::parse(
    library
      .find_at("LPIECELO.MDB", "MENUDATA", "PIECEDB")
      .unwrap(),
  )
  .unwrap();
  let red = materials
    .iter()
    .find(|m| m.name == "red")
    .unwrap()
    .base_color();
  assert!(red[0] > red[1] && red[0] > red[2]);
  for brick in &db.bricks {
    assert_eq!(
      brick.cells.len(),
      brick.width as usize * brick.depth as usize
    );
    for face in &brick.faces {
      assert!([3, 4].contains(&face.points.len()));
      assert!(face.points.iter().flatten().all(|v| v.is_finite()));
    }
  }
}
