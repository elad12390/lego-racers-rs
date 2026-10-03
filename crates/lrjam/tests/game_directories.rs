use lrjam::Jam;

#[test]
fn original_archive_exposes_builder_and_localization_beneath_menu_files() {
  let jam = Jam::open(
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
      .join("../../extracted/Program_Files_Group/LEGO.JAM"),
  )
  .unwrap();
  let menu = jam.tables.iter().find(|t| t.name == "MENUDATA").unwrap();
  assert!(menu.entries.iter().any(|e| e.name == "LEGORACE.RCB"));
  let pieces = jam.tables.iter().find(|t| t.name == "PIECEDB").unwrap();
  assert!(pieces.entries.iter().any(|e| e.name == "LPIECELO.LEB"));
  assert!(pieces.entries.iter().any(|e| e.name == "L_COLORS.LEB"));
  assert!(jam.tables.iter().any(|t| t.name == "ENGLISH"));
  for table in &jam.tables {
    for entry in &table.entries {
      assert_eq!(jam.bytes(entry).unwrap().len(), entry.size as usize);
    }
  }
}
