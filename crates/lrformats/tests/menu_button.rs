use lrformats::{library::Library, menu_button};

#[test]
fn original_license_button_styles_keep_six_font_image_and_color_states() {
  let library = Library::open(
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
      .join("../../extracted/Program_Files_Group/LEGO.JAM"),
  )
  .unwrap();
  let styles = menu_button::parse(
    library
      .find_at("GSTYLES.MSB", "MENUDATA", "MENUDATA")
      .unwrap(),
  )
  .unwrap();
  assert_eq!(styles.len(), 5);
  for (name, image) in [
    ("nubutton", "clear32"),
    ("buttonra", "txtaror"),
    ("buttonla", "txtarol"),
    ("buttonca", "txtx"),
  ] {
    let style = &styles[name];
    assert!(style.fonts.iter().all(|f| f == "font_ths"));
    assert!(style.images.iter().all(|f| f == image));
    assert_eq!(style.text_colors[2], [118, 107, 15, 255]);
    assert_eq!(style.text_colors[4], [246, 230, 6, 255]);
    assert_eq!(style.text_colors[5], [3, 254, 9, 255]);
    if name == "nubutton" {
      assert_eq!(style.image_colors[2], [255; 4]);
    } else if name == "buttonca" {
      assert_eq!(style.image_colors[2], [125, 18, 0, 255]);
      assert_eq!(style.image_colors[4], [255, 36, 0, 255]);
      assert_eq!(style.image_colors[5], [255, 36, 0, 255]);
    } else {
      assert_eq!(style.image_colors[2], [118, 107, 15, 255]);
      assert_eq!(style.image_colors[4], [255, 223, 0, 255]);
      assert_eq!(style.image_colors[5], [3, 254, 9, 255]);
    }
  }
}
