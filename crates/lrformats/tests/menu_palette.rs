use lrformats::{library::Library,bmp};
#[test]
fn original_pc_menu_background_is_blue_not_red_and_logo_is_yellow() {
    let library=Library::open(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../extracted/Program_Files_Group/LEGO.JAM")).unwrap();
    let background=bmp::decode(library.find_at("BACKDRP.BMP","MENUDATA","MENUDATA").unwrap()).unwrap();
    let totals=background.indices.iter().fold([0u64;3],|mut totals,i| {for c in 0..3 {totals[c]+=background.palette[*i as usize][c] as u64;}totals});
    assert!(totals[2]>totals[0]*5);
    let logo=bmp::decode(library.find_at("RACERS.BMP","MENUDATA","MENUDATA").unwrap()).unwrap();
    assert!(logo.palette.iter().any(|p|p[0]>200&&p[1]>200&&p[2]<80));
}
