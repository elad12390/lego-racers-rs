use lrformats::{library::Library,bitmap_font,bmp};
#[test]
fn menu_font_strip_preserves_punctuation_positions() {
    let library=Library::open(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../extracted/Program_Files_Group/LEGO.JAM")).unwrap();
    let fonts=bitmap_font::parse(library.find_at("GFONTS.FDB","MENUDATA","ENGLISH").unwrap()).unwrap();
    for spec in fonts {let image=bmp::decode(library.find_at(&format!("{}.BMP",spec.name),"MENUDATA","ENGLISH").unwrap()).unwrap();
        let mut count=0;let mut filled_before=false;for x in 0..image.width as usize {let filled=(0..image.height as usize).any(|y|image.palette[image.indices[y*image.width as usize+x] as usize]!=[0,0,0]);if filled&&!filled_before {count+=1;}filled_before=filled;}
        if spec.name=="font_ths" {assert_eq!(count,spec.characters.len());let glyphs=bitmap_font::glyphs(&spec,&image).unwrap();assert_eq!(glyphs[43].character,':');assert!(glyphs[43].width>0);assert_eq!(glyphs.len(),63);}
    }
}
