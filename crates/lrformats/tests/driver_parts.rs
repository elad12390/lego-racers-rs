use lrformats::{driver_parts,part_geometry,library::Library};
#[test]
fn original_driver_selection_and_all_head_geometry_load() {
    let library=Library::open(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../extracted/Program_Files_Group/LEGO.JAM")).unwrap();
    let read=|file:&str,table:&str|library.find_at(file,"MENUDATA",table).unwrap();
    let parts=driver_parts::Catalog::parse(read("BODYPART.PCB","PARTDB")).unwrap();
    assert_eq!([parts.hats.len(),parts.faces.len(),parts.torsos.len(),parts.legs.len()],[36,30,29,20]);
    assert_eq!(parts.heads[0],"head");assert_eq!(parts.torsos[0].name,"tu_chst");assert_eq!(parts.hats[35].unlock,128);
    let game=part_geometry::Geometry::parse(read("ICB_CHAR.GCB","GAMEPART")).unwrap();
    let menu=part_geometry::Geometry::parse(read("CBBODIES.GCB","MENUPART")).unwrap();
    assert_eq!(game.parts.len(),36);assert_eq!(menu.parts.len(),73);
    for (hat,head) in parts.hats.iter().zip(&parts.heads) {assert!(menu.parts.contains_key(&hat.name),"{}",hat.name);assert!(game.parts.contains_key(head),"{head}");}
    for (name,g) in [("game",&game),("menu",&menu)] {for (part,mesh) in &g.parts {let indices=mesh.surfaces.iter().flat_map(|s|s.triangles.iter().flatten());let mut lo=[f32::INFINITY;3];let mut hi=[f32::NEG_INFINITY;3];for i in indices {for axis in 0..3 {let v=g.vertices[*i as usize].position[axis]*mesh.scale;lo[axis]=lo[axis].min(v);hi[axis]=hi[axis].max(v);}}eprintln!("{name}/{part}: {lo:?}..{hi:?}");}}
}
