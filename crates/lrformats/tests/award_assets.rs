use lrformats::{bmp,cinematic::Timeline,library::Library};
fn library()->Library {Library::open(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../extracted/Program_Files_Group/LEGO.JAM")).unwrap()}
#[test]
fn original_award_worlds_and_truecolor_images_decode_without_palette_quantization() {
    let library=library();
    for (table,name) in [("C_AWARD1","1STPLACE"),("C_AWARD2","TWO"),("C_AWARD3","3-01"),("C_AWARD4","LOSE"),("WINCAR","WINCAR")] {
        let scenes=Timeline::parse(library.find_at(&format!("{name}.CDB"),"MENUDATA",table).unwrap()).unwrap();
        assert_eq!(scenes.len(),1);let scene=&scenes[0];assert_eq!(scene.worlds.len(),3);assert!(scene.objects.iter().any(|o|o.name=="guy1"));
        for world in &scene.worlds {assert!(library.find_at(&format!("{world}.WDB"),"MENUDATA",table).is_some());}
    }
    let image=bmp::decode(library.find_at("EX10.BMP","MENUDATA","C_AWARD1").unwrap()).unwrap();assert_eq!((image.width,image.height),(64,64));assert!(image.direct_rgba.is_some());assert!(image.palette.is_empty());assert_eq!(image.to_rgba().len(),64*64*4);
    let pixels=image.to_rgba();let colors=pixels.chunks_exact(4).collect::<std::collections::HashSet<_>>();assert!(colors.len()>32);assert!(pixels.chunks_exact(4).all(|p|p[0]==p[1]&&p[1]==p[2]&&p[3]==255));
}

#[test]
fn original_award_named_events_resolve_owning_sound_bank_and_exact_frames() {
    let library=library();let scene=Timeline::parse(library.find_at("1STPLACE.CDB","MENUDATA","C_AWARD1").unwrap()).unwrap().remove(0);
    let cues=lrformats::cinematic_audio::cues(&library,"C_AWARD1","1STPLACE",&scene.events).unwrap();
    assert_eq!(cues.len(),9);assert_eq!(cues[0].frame,49);assert_eq!(cues[0].file,"flshblb4.pcm");let fireworks=cues.iter().filter(|c|c.frame==144).collect::<Vec<_>>();assert_eq!(fireworks.len(),2);assert_eq!(fireworks[0].file,"firewrks.pcm");assert_eq!(fireworks[1].gain,0.5);
    for (table,name) in [("C_AWARD1","1STPLACE"),("C_AWARD2","TWO"),("C_AWARD3","3-01"),("C_AWARD4","LOSE")] {
        let scene=Timeline::parse(library.find_at(&format!("{name}.CDB"),"MENUDATA",table).unwrap()).unwrap().remove(0);
        for cue in lrformats::cinematic_audio::cues(&library,table,name,&scene.events).unwrap() {let decoded=lrformats::pcm::decode(library.find_at(&cue.file,"MENUDATA",table).unwrap()).unwrap();assert!(decoded.sample_rate>0&&!decoded.samples.is_empty());}
    }
}

#[test]
fn rocket_sparkle_keeps_original_targa_alpha() {
    let library=library();let image=lrformats::tga::decode(library.find_at("RRSPRKL.TGA","MENUDATA","WINRRCAR").unwrap()).unwrap();
    assert_eq!((image.width,image.height),(32,32));let pixels=image.to_rgba();assert!(pixels.chunks_exact(4).any(|p|p[3]==0));assert!(pixels.chunks_exact(4).any(|p|p[3]>0));
    let materials=lrformats::scene::SceneBindings::load(&library,"WINRRCAR",library.find_at("END.WDB","MENUDATA","WINRRCAR").unwrap()).unwrap();
    let model=lrformats::model::Model::load_with_materials(&library,"spkle02",Some("WINRRCAR"),&materials.materials).unwrap();assert!(model.images.contains_key("rrsprkl"));
}

#[test]
fn original_reward_text_and_material_keys_have_real_activation_boundaries() {
    let library=library();
    let timeline=Timeline::parse(library.find_at("WINCAR.CDB","MENUDATA","WINCAR").unwrap()).unwrap().remove(0);
    let plan=lrformats::cinematic_overlay::load(&library,"WINCAR","WINCAR",&timeline.events,Some("KK")).unwrap();
    assert_eq!(plan.texts.len(),2);assert_eq!((plan.texts[0].start,plan.texts[0].end),(272,384));assert!(plan.texts[1].value.to_ascii_uppercase().contains("KING KAHUKA"));assert!(!plan.texts.iter().any(|t|t.value.contains("Redbeard")));assert_eq!(plan.texts[1].y,Some(0.87));
    assert_eq!(plan.fades.len(),1);assert_eq!(plan.fades[0].millis,1834);assert_eq!(plan.fades[0].start,385);
    let animation=lrformats::material_animation::Animation::parse(library.find_at("WINCAR.MAB","MENUDATA","WINCAR").unwrap()).unwrap();
    assert_eq!(animation.sample(0,274.0/30.0).unwrap(),"sad");assert_eq!(animation.sample(0,275.1/30.0).unwrap(),"blink");assert_eq!(animation.sample(0,285.1/30.0).unwrap(),"dflt");assert_eq!(animation.sample(0,320.1/30.0).unwrap(),"happy");
    let guy=timeline.objects.iter().find(|o|o.name=="guy1").unwrap();assert_eq!(guy.materials.len(),1);assert_eq!(guy.materials[0].channel,0);
    for (table,cdb) in [("WINRRCAR","END"),("WINVVCAR","VVWIN")] {
        let timeline=Timeline::parse(library.find_at(&format!("{cdb}.CDB"),"MENUDATA",table).unwrap()).unwrap().remove(0);
        let plan=lrformats::cinematic_overlay::load(&library,table,cdb,&timeline.events,None).unwrap();assert!(!plan.texts.is_empty());assert!(plan.texts.iter().all(|t|t.start<t.end&&!t.value.is_empty()));
    }
}

#[test]
fn texture_key_is_declared_rgb_not_every_black_pixel() {
    let library=library();let textures=lrformats::texture_catalog::parse(library.find_at("COMBINED.TDB","MENUDATA","WINRRCAR").unwrap()).unwrap();
    assert_eq!(textures.iter().find(|t|t.name=="stream").unwrap().color_key,Some([140;3]));assert!(textures.iter().find(|t|t.name=="rrsprkl").unwrap().targa);
    let materials=lrformats::scene::SceneBindings::load(&library,"WINRRCAR",library.find_at("END.WDB","MENUDATA","WINRRCAR").unwrap()).unwrap();
    assert_eq!(materials.materials["strm"].color_key,Some([140;3]));assert_eq!(materials.materials["leg"].color_key,None);
}

#[test]
fn award_material_rows_bind_each_owning_world_not_a_synthetic_type_or_offset() {
    let library=library();let mut worlds=std::collections::BTreeSet::new();let mut tracks=0;
    for (table,cdb) in [("C_AWARD1","1STPLACE"),("C_AWARD2","TWO"),("C_AWARD3","3-01"),("C_AWARD4","LOSE"),("WINCAR","WINCAR"),("WINRRCAR","END"),("WINVVCAR","VVWIN")] {
        let scene=Timeline::parse(library.find_at(&format!("{cdb}.CDB"),"MENUDATA",table).unwrap()).unwrap().remove(0);
        for object in &scene.objects {for reference in &object.materials {
            let animation=lrformats::material_animation::load_track(&library,table,&scene.worlds,reference).unwrap();
            assert!(!animation.sample(reference.channel,0.0).unwrap().is_empty());worlds.insert(reference.world);tracks+=1;
            if table=="WINVVCAR"&&object.name=="guy1" {assert_eq!((reference.world,reference.table,reference.channel,reference.slot,reference.level),(0,0,0,7,0));assert_eq!(animation.sample(0,0.0).unwrap(),"sad");assert_eq!(animation.sample(0,266.1/30.0).unwrap(),"happy");}
        }}
    }
    assert!(tracks>10);assert!(worlds.contains(&0)&&worlds.contains(&1));
}

#[test]
fn veronica_movie_is_the_source_screen_excluded_from_minifigure_substitution() {
    let library=library();let names=lrformats::string_table::StringTable::parse(library.find_in("MENUNAME.SRF","MENUDATA").unwrap()).unwrap();
    assert_eq!(names.get(0x1c).unwrap(),"winvvcar");
    let mesh=lrformats::gdb::parse(library.find_at("GUY1.GDB","MENUDATA","WINVVCAR").unwrap()).unwrap();assert_eq!(mesh.scale,0.01);
    let scene=Timeline::parse(library.find_at("VVWIN.CDB","MENUDATA","WINVVCAR").unwrap()).unwrap().remove(0);
    let bindings=lrformats::scene::SceneBindings::load(&library,"WINVVCAR",library.find_at("VVWIN.WDB","MENUDATA","WINVVCAR").unwrap()).unwrap();
    let model=lrformats::model::Model::load_with_materials(&library,"guy1",Some("WINVVCAR"),&bindings.materials).unwrap();
    let reference=&scene.objects.iter().find(|o|o.name=="guy1").unwrap().materials[0];
    assert!(model.surfaces.iter().any(|s|s.material.as_deref()==Some(mesh.textures[reference.slot].as_str())));
}
