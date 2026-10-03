use lrformats::{
  bmp, library::Library, menu_frame, menu_layout::Layout, string_table::StringTable,
};

#[test]
fn original_license_name_focus_frame_keeps_roundbox_art_colors_and_bounds() {
  let library = Library::open(
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
      .join("../../extracted/Program_Files_Group/LEGO.JAM"),
  )
  .unwrap();
  let read = |name: &str| library.find_at(name, "MENUDATA", "MENUDATA").unwrap();
  let names = StringTable::parse(read("MENUNAME.SRF")).unwrap();
  assert_eq!(names.get(0xd9).unwrap(), "firstbox");
  assert_eq!(names.get(0x48).unwrap(), "roundbox");
  let layout = Layout::parse(read("DRVRLICE.MIB")).unwrap();
  assert_eq!(layout.widgets["firstbox"].rect, Some([-8, 112, 264, 174]));
  assert_eq!(
    layout.widgets["firstbox"].parent.as_deref(),
    Some("license")
  );
  let styles = menu_frame::parse_styles(read("GSTYLES.MSB")).unwrap();
  let frame = &styles["roundbox"];
  assert_eq!(
    frame.images,
    ["lul", "ltb", "lur", "lrb", "lbr", "lbb", "lbl", "llb"]
  );
  assert_eq!(frame.edge_color, [8, 8, 115, 255]);
  assert_eq!(frame.panel_color, [8, 8, 115, 255]);
  for name in &frame.images {
    let image = bmp::decode(read(&format!("{name}.BMP"))).unwrap();
    assert_eq!((image.width, image.height), (16, 16));
  }
  let driver = Layout::parse(read("EDITDRVR.MIB")).unwrap();
  assert_eq!(driver.widgets["platform"].rect, Some([291, 64, 627, 450]));
  assert_eq!(driver.widgets["platform"].scene.as_deref(), Some("cbset"));
  let frame = driver.frame_for("platform").unwrap();
  assert!(frame.shown);
  assert_eq!(
    frame.images,
    ["tul", "tt", "tur", "tr", "tbr", "tb", "tbl", "tl"]
  );
  assert_eq!(frame.edge_color, [25, 26, 221, 255]);
  assert_eq!(frame.panel_color, [0, 0, 55, 255]);
  for name in &frame.images {
    let image = bmp::decode(read(&format!("{name}.BMP"))).unwrap();
    assert_eq!((image.width, image.height), (16, 16));
  }
}
