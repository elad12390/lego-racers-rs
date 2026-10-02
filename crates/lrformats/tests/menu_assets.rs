use lrformats::{library::Library,bitmap_font,bmp,string_table::StringTable};

#[test]
fn original_menu_text_and_font_maps_load_from_exact_owner() {
    let library=Library::open(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../extracted/Program_Files_Group/LEGO.JAM")).unwrap();
    let text=StringTable::parse(library.find_at("MENUTEXT.SRF","MENUDATA","ENGLISH").unwrap()).unwrap();
    assert_eq!(text.get(37).unwrap(),"BUILD");
    let fonts=bitmap_font::parse(library.find_at("GFONTS.FDB","MENUDATA","ENGLISH").unwrap()).unwrap();
    for name in ["fontmenu","font_ths"] {
        let spec=fonts.iter().find(|f|f.name==name).unwrap();
        let image=bmp::decode(library.find_at(&format!("{name}.BMP"),"MENUDATA","ENGLISH").unwrap()).unwrap();
        let glyphs=bitmap_font::glyphs(spec,&image).unwrap();
        assert!(glyphs.iter().any(|g|g.character=='A'&&g.width>0));
    }
    let circuits=StringTable::parse(library.find_at("CIRCUIT.SRF","MENUDATA","ENGLISH").unwrap()).unwrap();
    assert_eq!(circuits.get(12).unwrap(),"KNIGHTMARE-ATHON");
}
