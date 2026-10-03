use lrformats::{library::Library, menu_layout::Layout};
#[test]
fn original_menu_widgets_preserve_named_positions_and_parent_relationships() {
  let library = Library::open(
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
      .join("../../extracted/Program_Files_Group/LEGO.JAM"),
  )
  .unwrap();
  let load =
    |name: &str| Layout::parse(library.find_at(name, "MENUDATA", "MENUDATA").unwrap()).unwrap();
  let main = load("MAINMENU.MIB");
  assert_eq!(main.widgets["racers"].rect, Some([35, 4, 0, 0]));
  assert_eq!(main.widgets["garage"].rect, Some([3, 118, 0, 0]));
  assert_eq!(main.widgets["time"].rect.unwrap()[1], 298);
  assert_eq!(main.widgets["quit"].rect.unwrap()[1], 418);
  assert_eq!(main.widgets["garage"].parent.as_deref(), Some("backdrp"));
  let garage = load("GARAGE.MIB");
  assert_eq!(garage.widgets["showcase"].rect.unwrap()[..2], [251, 125]);
  let license = load("DRVRLICE.MIB");
  assert_eq!(license.widgets["platform"].scene.as_deref(), Some("cam"));
  assert_eq!(
    license.widgets["platform"].parent.as_deref(),
    Some("license")
  );
  assert_eq!(license.widgets["platform"].rect, Some([238, -18, 448, 186]));
  assert_eq!(license.widgets["ftext"].rect, Some([16, 129, 284, 163]));
  let strings = lrformats::string_table::StringTable::parse(
    library
      .find_at("MENUTEXT.SRF", "MENUDATA", "ENGLISH")
      .unwrap(),
  )
  .unwrap();
  assert_eq!(
    strings.get(1).unwrap(),
    " ABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890"
  );
  let frame = license.frame_for("platform").unwrap();
  let driver = load("EDITDRVR.MIB");
  for (name, y) in [("mix", 338), ("gonext", 378), ("goback", 418)] {
    assert_eq!(driver.widgets[name].rect, Some([3, y, 0, 0]));
  }
  for (id, label) in [(0x38, "MIX"), (10, "MAKE LICENSE"), (31, "CANCEL")] {
    assert_eq!(strings.get(id).unwrap(), label);
  }
  assert!(frame.shown);
  assert!(frame.images.iter().all(|name| name == "clear32"));
  assert_eq!(frame.edge_color, [83, 90, 140, 255]);
  assert_eq!(frame.panel_color, [83, 90, 140, 255]);
  let clear = lrformats::bmp::decode(
    library
      .find_at("CLEAR32.BMP", "MENUDATA", "MENUDATA")
      .unwrap(),
  )
  .unwrap();
  assert_eq!((clear.width, clear.height), (32, 32));
  assert!(clear
    .to_rgba()
    .chunks_exact(4)
    .all(|pixel| pixel[..3] == [0; 3]));
  for name in [
    "CIRCRACE.MIB",
    "SINGRACE.MIB",
    "EDITDRVR.MIB",
    "DRVRLICE.MIB",
    "CARBUILD.MIB",
  ] {
    load(name);
  }
}
