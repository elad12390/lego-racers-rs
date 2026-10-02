use lrformats::{library::Library, model::Model, scene::SceneBindings};
use std::path::PathBuf;

#[test]
fn original_desert_road_resolves_shared_material_texture_alias() {
    let library = Library::open(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../extracted/Program_Files_Group/LEGO.JAM"),
    )
    .unwrap();
    let world = library.find_in("TEST.WDB", "RACEC0R2").unwrap();
    let bindings = SceneBindings::load(&library, "RACEC0R2", world).unwrap();
    let track = Model::load_with_materials(&library, "test", Some("RACEC0R2"), &bindings.materials)
        .unwrap();
    let roads: Vec<_> = track
        .surfaces
        .iter()
        .filter(|s| s.material.as_deref() == Some("adroad"))
        .collect();
    assert!(!roads.is_empty());
    assert!(roads
        .iter()
        .all(|s| s.texture.as_deref() == Some("adroad7")));
    assert!(track.images.contains_key("adroad7"));
    assert!(track.images.contains_key("adrock1"));
    assert!(track.images.contains_key("adsand2"));
}

#[test]
fn every_original_track_and_static_instance_resolves_its_scene_materials() {
    let library=Library::open(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../extracted/Program_Files_Group/LEGO.JAM")).unwrap();
    let mut tracks=0;
    for table in &library.jam().tables {
        if !table.name.starts_with("RACEC") {continue;}
        let mut found_track=false;
        for entry in &table.entries {
            if !entry.name.to_ascii_uppercase().ends_with(".WDB") {continue;}
            let bytes=library.jam().bytes(entry).unwrap();
            let instances=lrformats::world::instances(bytes).unwrap();
            let track=lrformats::world::track_model(bytes);
            if instances.is_empty() && track.is_none() {continue;}
            let bindings=SceneBindings::load(&library,&table.name,bytes).unwrap_or_else(|e|panic!("{e}"));
            for name in track.iter().chain(instances.iter().map(|i|&i.model)) {
                let model=Model::load_with_materials(&library,name,Some(&table.name),&bindings.materials)
                    .unwrap_or_else(|e|panic!("{} / {} / {}: {}",table.name,entry.name,name,e));
                assert!(!model.mesh.vertices.is_empty());
            }
            found_track|=track.is_some();
        }
        assert!(found_track,"{}: no track",table.name);
        tracks+=1;
    }
    assert_eq!(tracks,13);
}
