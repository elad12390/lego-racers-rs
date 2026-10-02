use lrformats::{library::Library,menu_layout::Layout};
#[test]
fn original_menu_widgets_preserve_named_positions_and_parent_relationships() {
    let library=Library::open(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../extracted/Program_Files_Group/LEGO.JAM")).unwrap();
    let load=|name:&str|Layout::parse(library.find_at(name,"MENUDATA","MENUDATA").unwrap()).unwrap();
    let main=load("MAINMENU.MIB");
    assert_eq!(main.widgets["racers"].rect,Some([35,4,0,0]));
    assert_eq!(main.widgets["garage"].rect,Some([3,118,0,0]));
    assert_eq!(main.widgets["time"].rect.unwrap()[1],298);
    assert_eq!(main.widgets["quit"].rect.unwrap()[1],418);
    assert_eq!(main.widgets["garage"].parent.as_deref(),Some("backdrp"));
    let garage=load("GARAGE.MIB");
    assert_eq!(garage.widgets["showcase"].rect.unwrap()[..2],[251,125]);
    for name in ["CIRCRACE.MIB","SINGRACE.MIB","EDITDRVR.MIB","DRVRLICE.MIB","CARBUILD.MIB"] {load(name);}
}
