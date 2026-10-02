use lrformats::{library::Library, sky};
use std::path::PathBuf;

#[test]
fn original_default_sky_profile_and_colors_are_preserved() {
    let library = Library::open(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../extracted/Program_Files_Group/LEGO.JAM"),
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
