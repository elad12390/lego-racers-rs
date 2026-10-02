//! Persistent launch/menu/garage/race/results/circuit loop.
//! Original assets/catalogs; native widgets/save encoding are not parity claims.
use crate::{game_catalog::Catalog,menu_ui::MenuUi,options::Options,profile::Profile,gpu::TrackGpu};
use lrformats::library::Library;
use macroquad::prelude::*;

enum Screen {Main,Races,Circuits,RacerSelect,Garage,EditRacer,Driver,License,DeleteRacer,Settings,Results}
struct CircuitRun {index:usize,races:Vec<usize>,next:usize,scores:[u32;6]}

pub async fn run(mut options:Options)->Result<(),String> {
    let library=Library::open(&options.jam).map_err(|e|e.to_string())?;let catalog=Catalog::load(&library)?;
    let path=options.profile.clone().unwrap_or_else(Profile::path);let mut profile=Profile::load(&path)?;
    if let Some(build)=&profile.custom_build {lrsim::brick_build::BuilderData::load(&library)?.validate(build)?;}
    if let Some(build)=&profile.custom_driver {crate::custom_driver::Data::load(&library)?.validate(build)?;}
    profile.sync_racer();
    if !catalog.cars.iter().any(|c|c.name.eq_ignore_ascii_case(&profile.car))||!catalog.drivers.iter().any(|d|d.name.eq_ignore_ascii_case(&profile.driver)) {return Err("saved appearance is not in the original catalog; save preserved".into());}
    let mut ui=MenuUi::load(&library)?;let mut screen=Screen::Main;let mut circuit:Option<CircuitRun>=None;
    let mut result:Option<crate::driving::RaceResult>=None;let mut result_race=0;let mut message=String::new();
    let mut preview_name=String::new();let mut preview=None;let mut frame=0u32;let mut smoke_stage=0;
    let mut circuit_menu=crate::circuit_menu::CircuitMenu::new();
    let mut track_menu=crate::track_menu::TrackMenu::new();let mut pending_race=None;
    let mut versus_players=None;
    if options.menu_preview_smoke {screen=Screen::Circuits;}
    if let Some(dir)=&options.capture {std::fs::create_dir_all(dir).map_err(|e|e.to_string())?;}
    if let Some(host)=&options.reward_smoke {
        let before=std::fs::read(&path).ok();let dir=options.capture.as_ref().unwrap();
        crate::circuit_award::unlock(&library,&catalog,&profile,host,&ui,Some((dir,format!("reward-{}",host.to_ascii_lowercase())))).await?;
        if std::fs::read(&path).ok()!=before {return Err("single reward smoke mutated isolated profile".into());}
        return Ok(());
    }
    if options.award_smoke {
        let before=std::fs::read(&path).ok();
        let dir=options.capture.as_ref().unwrap();for rank in 1..=4 {crate::circuit_award::run(&library,&catalog,&profile,rank,(rank==1).then_some(0),&ui,Some(dir)).await?;}
        for (i,circuit) in catalog.circuits.iter().enumerate().skip(1) {crate::circuit_award::unlock(&library,&catalog,&profile,&circuit.racers[1],&ui,Some((dir,format!("unlock-{}",i+1)))).await?;}
        crate::circuit_award::unlock(&library,&catalog,&profile,"VV",&ui,Some((dir,"unlock-veronica".into()))).await?;
        let unchanged=std::fs::read(&path).ok()==before;if !unchanged {return Err("award scene smoke mutated isolated profile".into());}
        std::fs::write(dir.join("award-scenes.json"),serde_json::to_vec_pretty(&serde_json::json!({"mode":"isolated_original_scene_loading_not_completed_circuits","ranks":[1,2,3,4],"profile_unchanged":unchanged})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;return Ok(());
    }
    if options.versus_smoke {
        let race=catalog.races.iter().find(|r|r.name.eq_ignore_ascii_case(options.smoke_race.as_deref().unwrap_or("rkr"))).ok_or("missing versus smoke race")?;options.table=race.table.to_ascii_uppercase();options.mirrored=race.mirrored;
        let mut second=Profile::default();second.name="Player 2".into();second.car="KK".into();second.driver="KK".into();
        let track=TrackGpu::upload(&crate::assets::load_track(&options)?)?;crate::versus_race::run(&options,&library,&[profile,second],track).await?;return Ok(());
    }
    loop {
        if is_quit_requested() {break;}
        if is_key_pressed(KeyCode::Escape) {if matches!(screen,Screen::Main) {break;}screen=Screen::Main;ui.selected=0;}
        let mut launch=None;
        // Upload textures before drawing the UI, not halfway through a queued
        // text batch: a newly created preview can invalidate its GL binding.
        if matches!(screen,Screen::Main|Screen::Garage|Screen::EditRacer|Screen::RacerSelect)&&preview_name!=format!("{}/{}",profile.car,profile.driver) {preview=Some(crate::racer_preview::RacerPreview::load(&library,&catalog,&profile)?);preview_name=format!("{}/{}",profile.car,profile.driver);}
        match screen {
            Screen::Main=> {
                ui.original_background("",true);
                let labels=[("garage",37,true),("circuit",33,true),("single",34,true),("vs",35,true),("time",36,true),("options",38,true),("quit",39,true)].into_iter().map(|(key,id,enabled)|Ok((key,ui.strings.get(id)?.to_owned(),enabled))).collect::<Result<Vec<_>,String>>()?;
                let selected=ui.original_buttons("mainmenu",&labels);
                if let Some(selected)=selected {ui.selected=0;options.versus=selected==3;screen=match selected {0=>Screen::Garage,1=>Screen::Circuits,2|3=>{options.time_trial=false;Screen::Races},4=>{options.time_trial=true;Screen::Races},5=>Screen::Settings,_=>break};}
                if let Some(racer)=&mut preview {racer.main_driver()?;}
            }
            Screen::Races=> {
                match track_menu.draw(&library,&catalog,&profile,&ui,options.time_trial)? {
                    crate::track_menu::Action::Back=>screen=Screen::Main,
                    crate::track_menu::Action::Start(index)=>{circuit=None;pending_race=Some(index);screen=Screen::RacerSelect;},crate::track_menu::Action::None=>{},
                }
            }
            Screen::Circuits=> {
                match circuit_menu.draw(&library,&catalog,&profile,&ui)? {
                    crate::circuit_menu::Action::Back=>{screen=Screen::Main;ui.selected=0;},
                    crate::circuit_menu::Action::Start(index)=> {
                        let races=catalog.circuit_races(&catalog.circuits[index].name);pending_race=races.first().copied();screen=Screen::RacerSelect;
                        circuit=Some(CircuitRun {index,races,next:0,scores:[0;6]});options.time_trial=false;
                    },crate::circuit_menu::Action::None=>{},
                }
            }
            Screen::RacerSelect=> {
                if options.versus {
                    if let Some(players)=crate::versus_selection::run(&library,&catalog,&profile,&mut ui).await? {versus_players=Some(players);launch=pending_race.take();}else {pending_race=None;screen=Screen::Races;}
                }else {
                ui.original_background("Racer Select",false);ui.centered_text(&profile.name,420.0,119.0,26.0,YELLOW);
                let delta=if ui.icon_button("arrowlu",239.0,87.0)||is_key_pressed(KeyCode::Left) {-1}else if ui.icon_button("arrowru",572.0,87.0)||is_key_pressed(KeyCode::Right) {1}else {0};
                if delta!=0 {let index=(profile.active_racer as isize+delta).rem_euclid(profile.racers.len() as isize) as usize;profile.select_racer(index)?;profile.save(&path)?;preview_name.clear();}
                if let Some(racer)=&mut preview {racer.garage(&ui)?;}
                if ui.action("Config",35.0,366.0,None) {screen=Screen::Settings;}
                if ui.action("OK",35.0,406.0,Some("chck"))||is_key_pressed(KeyCode::Enter) {launch=pending_race.take();}
                if ui.action("Cancel",35.0,446.0,Some("txtx")) {screen=if circuit.is_some() {Screen::Circuits}else {Screen::Races};circuit=None;pending_race=None;}
                }
            }
            Screen::Garage=> {
                ui.original_background("Build Menu",false);
                let labels=[("newracer","New Racer".into(),profile.racers.len()<8),("editracr","Edit Racer".into(),true),("copyracr","Copy Racer".into(),profile.racers.len()<8),("delracer","Delete Racer".into(),profile.racers.len()>1),("testtrck","Test Drive".into(),true),("goback","Main Menu".into(),true)];
                if let Some(choice)=ui.original_buttons("garage",&labels) {match choice {
                    0=>{profile.new_racer()?;profile.save(&path)?;preview_name.clear();screen=Screen::Driver;ui.selected=0;},1=>{screen=Screen::EditRacer;ui.selected=0;},
                    2=>{profile.copy_racer()?;profile.save(&path)?;preview_name.clear();},
                    3=>{screen=Screen::DeleteRacer;ui.selected=0;},
                    4=>{circuit=None;options.time_trial=false;launch=Some(catalog.races.iter().position(|r|r.name.eq_ignore_ascii_case("test")).ok_or("missing original test track")?);},
                    5=>{screen=Screen::Main;ui.selected=0;},_=>{},
                }}
                ui.text(&profile.name,365.0,121.0,26.0,Color::new(0.65,0.65,0.15,1.0));
                let delta=if ui.icon_button("arrowlu",235.0,92.0)||is_key_pressed(KeyCode::Left) {-1} else if ui.icon_button("arrowru",572.0,92.0)||is_key_pressed(KeyCode::Right) {1}else {0};
                if delta!=0 {let index=(profile.active_racer as isize+delta).rem_euclid(profile.racers.len() as isize) as usize;profile.select_racer(index)?;profile.save(&path)?;preview_name.clear();}
                if let Some(racer)=&mut preview {racer.garage(&ui)?;}
            }
            Screen::EditRacer=> {
                ui.original_background("Edit Racer",false);
                let labels=[("newracer","Select Racer".into(),false),("editdrvr","Build Driver".into(),true),("editlice","Make License".into(),true),("editcar","Build Car".into(),true),("testtrck","Test Drive".into(),true),("goback","Build Menu".into(),true)];
                if let Some(choice)=ui.original_buttons("garage",&labels) {
                    match choice {
                        1=>screen=Screen::Driver,2=>screen=Screen::License,
                        3=>{if let Some((build,car))=crate::garage_builder::run(&library,&catalog,&profile,&mut ui,None).await? {profile.custom_build=Some(build);profile.custom_enabled=true;profile.car=car;profile.save(&path)?;preview_name.clear();}ui.selected=0;},
                        4=>{circuit=None;options.time_trial=false;launch=Some(catalog.races.iter().position(|r|r.name.eq_ignore_ascii_case("test")).ok_or("missing test track")?);},
                        _=>{screen=Screen::Garage;ui.selected=0;},
                    }
                }
                if let Some(racer)=&mut preview {racer.garage(&ui)?;}
            }
            Screen::Driver=> {
                if let Some(build)=crate::driver_editor::run(&library,&profile,&catalog,&ui,None).await? {profile.custom_driver=Some(build);profile.license_photo=None;profile.save(&path)?;preview_name.clear();screen=Screen::License;}else {screen=Screen::EditRacer;}ui.selected=0;
            }
            Screen::DeleteRacer=> {
                ui.original_background("Delete Racer",false);ui.centered_text(&format!("Delete {}?",profile.name),320.0,190.0,28.0,WHITE);
                if let Some(choice)=ui.buttons(&["Cancel".into(),"Delete Racer".into()],230.0,265.0,50.0) {if choice==1 {profile.delete_racer()?;profile.save(&path)?;preview_name.clear();}screen=Screen::Garage;ui.selected=0;}
            }
            Screen::License=> {
                if crate::license_editor::run(&library,&mut profile,&ui,None).await? {profile.save(&path)?;if let Some((build,car))=crate::garage_builder::run(&library,&catalog,&profile,&mut ui,None).await? {profile.custom_build=Some(build);profile.custom_enabled=true;profile.car=car;profile.save(&path)?;}preview_name.clear();screen=Screen::Garage;} else {screen=Screen::Driver;}ui.selected=0;
            }
            Screen::Settings=> {
                ui.background("Options");let labels=vec![format!("Music: {}",if profile.music {"On"} else {"Off"}),format!("Sound: {}",if profile.sound {"On"} else {"Off"}),"Save / Back".into()];
                if let Some(choice)=ui.buttons(&labels,24.0,150.0,60.0) {match choice {0=>profile.music=!profile.music,1=>profile.sound=!profile.sound,_=>{profile.save(&path)?;screen=if pending_race.is_some() {Screen::RacerSelect}else {Screen::Main};ui.selected=0;}}}
                ui.text("Arrows: steer / accelerate / brake",275.0,170.0,20.0,WHITE);
                ui.text("Space: use powerup   Esc: pause",275.0,210.0,20.0,WHITE);
                ui.text("R: restart race",275.0,250.0,20.0,WHITE);
            }
            Screen::Results=> {
                ui.background("Race Results");
                if let Some(result)=&result {
                    ui.text(&catalog.title(&catalog.races[result_race]),245.0,88.0,20.0,WHITE);
                    let mut order=(0..result.names.len()).collect::<Vec<_>>();order.sort_by_key(|i|result.ranks[*i]);
                    for (row,index) in order.into_iter().enumerate() {
                        ui.text(&format!("{}. {}",result.ranks[index],result.names[index]),285.0,145.0+row as f32*35.0,24.0,if index==0 {YELLOW} else {WHITE});
                    }
                    ui.text(&format!("Your time: {:.2} s",result.time),280.0,383.0,22.0,YELLOW);
                    if let Some(time)=result.original_ghost_time {ui.text(&format!("Veronica: {time:.2} s"),280.0,412.0,18.0,WHITE);ui.text(if result.time<time {"VERONICA BEATEN"} else {"TRY TO BEAT VERONICA"},280.0,441.0,17.0,YELLOW);}
                }
                let labels=vec![if circuit.is_some() {"Continue Circuit".into()} else {"Race Again".into()},"Main Menu".into()];
                if let Some(choice)=ui.buttons(&labels,24.0,160.0,60.0) {
                    if choice==1 {screen=Screen::Main;circuit=None;ui.selected=0;}
                    else if let Some(run)=&mut circuit {
                        run.next+=1;
                        if run.next<run.races.len() {launch=Some(run.races[run.next]);}
                        else {
                            let old_gold=profile.circuit_medals.get(&catalog.circuits[run.index].name)==Some(&1);
                            let rank=profile.record_circuit(&catalog,run.index,run.scores)?;
                            profile.save(&path)?;
                            crate::circuit_award::run(&library,&catalog,&profile,rank,(rank==1&&!old_gold).then_some(run.index),&ui,None).await?;
                            message=format!("Circuit finished — place {rank}, {} points",run.scores[0]);screen=Screen::Circuits;circuit=None;ui.selected=0;
                        }
                    } else {launch=Some(result_race);}
                }
            }
        }
        if !message.is_empty() {ui.text(&message,245.0,460.0,15.0,YELLOW);}
        if options.menu_preview_smoke&&frame%8==7 {
            let dir=options.capture.as_ref().unwrap();crate::capture::save_frame(&dir.join(format!("circuit-{}.png",circuit_menu.selected+1)),&crate::capture::screen_data())?;
            circuit_menu.selected+=1;if circuit_menu.selected==catalog.circuits.len() {break;}
        }
        if options.menu_smoke {
            if frame==2&&smoke_stage==0 {crate::capture::save_frame(&options.capture.as_ref().unwrap().join("menu.png"),&crate::capture::screen_data())?;screen=Screen::Garage;ui.selected=0;smoke_stage=1;}
            if frame==5&&smoke_stage==1 {
                crate::capture::save_frame(&options.capture.as_ref().unwrap().join("garage.png"),&crate::capture::screen_data())?;
                if options.driver_smoke {profile.custom_driver=crate::driver_editor::run(&library,&profile,&catalog,&ui,options.capture.as_deref()).await?;crate::license_editor::run(&library,&mut profile,&ui,options.capture.as_deref()).await?;preview_name.clear();}
                if options.builder_smoke {let (build,car)=crate::garage_builder::run(&library,&catalog,&profile,&mut ui,options.capture.as_deref()).await?.ok_or("builder smoke canceled")?;profile.custom_build=Some(build);profile.car=car;profile.custom_enabled=true;}
                profile.save(&path)?;
                if Profile::load(&path)?.custom_build!=profile.custom_build {return Err("custom brick car save/reload mismatch".into());}
                if Profile::load(&path)?.custom_driver!=profile.custom_driver||Profile::load(&path)?.license_photo!=profile.license_photo {return Err("custom driver/license save/reload mismatch".into());}
                screen=Screen::Races;ui.selected=0;smoke_stage=2;
            }
            if frame==8&&smoke_stage==2 {crate::capture::save_frame(&options.capture.as_ref().unwrap().join("selection.png"),&crate::capture::screen_data())?;if options.menu_views_only {screen=Screen::RacerSelect;preview_name.clear();smoke_stage=4;}else {launch=Some(catalog.races.iter().position(|r|r.name.eq_ignore_ascii_case(options.smoke_race.as_deref().unwrap_or("rkr"))).ok_or("missing smoke race")?);smoke_stage=3;}}
            if frame==11&&smoke_stage==4 {crate::capture::save_frame(&options.capture.as_ref().unwrap().join("racer-select.png"),&crate::capture::screen_data())?;options.time_trial=true;screen=Screen::Races;smoke_stage=5;}
            if frame==14&&smoke_stage==5 {let dir=options.capture.as_ref().unwrap();crate::capture::save_frame(&dir.join("time-selection.png"),&crate::capture::screen_data())?;std::fs::write(dir.join("menu-views.json"),serde_json::to_vec_pretty(&serde_json::json!({"mode":"isolated_native_menu_views_not_race_or_physical_input_acceptance","build_restored":Profile::load(&path)?.custom_build==profile.custom_build,"driver_restored":Profile::load(&path)?.custom_driver==profile.custom_driver,"photo_restored":Profile::load(&path)?.license_photo==profile.license_photo})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;break;}
            if matches!(screen,Screen::Results)&&smoke_stage==3 {crate::capture::save_frame(&options.capture.as_ref().unwrap().join("results.png"),&crate::capture::screen_data())?;
                let restored=Profile::load(&path)?;
                 if restored.best_times.get(&catalog.races[result_race].name)!=profile.best_times.get(&catalog.races[result_race].name) {return Err("save restoration failed".into());}
                 if let Some(ghost)=profile.trial_ghosts.get(&catalog.races[result_race].name) {
                     let saved=restored.trial_ghosts.get(&catalog.races[result_race].name).ok_or("time-race ghost missing after save")?;
                     if serde_json::to_vec(ghost).map_err(|e|e.to_string())?!=serde_json::to_vec(saved).map_err(|e|e.to_string())? {return Err("time-race ghost save/reload mismatch".into());}
                 }
                std::fs::write(options.capture.as_ref().unwrap().join("menu-smoke.json"),serde_json::to_vec_pretty(&serde_json::json!({"mode":"native_scripted_menu_race_save_flow_not_physical_input_or_full_game_parity","profile":path,"race":catalog.races[result_race].name,"result":result,"restored_best_times":restored.best_times})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;break;
            }
        }
        next_frame().await;frame+=1;
        if let Some(index)=launch {
            message.clear();let race=&catalog.races[index];options.table=race.table.to_ascii_uppercase();options.race_name=Some(race.name.clone());options.car=profile.car.clone();options.driver=profile.driver.clone();
            options.mirrored=race.mirrored;
            options.custom_build=if profile.custom_enabled {profile.custom_build.clone()} else {None};
            options.custom_driver=profile.custom_driver.clone();
            if options.menu_smoke {options.scripted_route=true;options.frames=Some(12000);}
            options.music=profile.music;options.sound=profile.sound;
            let model=crate::assets::load_track(&options)?;let track=TrackGpu::upload(&model)?;
            result=if options.versus {
                crate::versus_race::run(&options,&library,versus_players.as_ref().ok_or("versus players not selected")?,track).await?.map(|outcome| {message=format!("Player 2: {:.2} s",outcome.times[1]);crate::driving::RaceResult {names:outcome.names.to_vec(),ranks:outcome.ranks.to_vec(),time:outcome.times[0],laps:3,trial_ghost:None,original_ghost_time:None}})
            }else {crate::driving::run_race(options.clone(),track).await?};result_race=index;
            if options.menu_smoke&&result.is_none() {return Err("native menu smoke race did not finish within the frame budget; evidence preserved".into());}
            if let Some(outcome)=&result {
                let previous_ghost_unlock=profile.driver_part_allowed(128,&catalog);
                if let Some(ghost)=&outcome.trial_ghost {
                    if profile.trial_ghosts.get(&race.name).is_none_or(|best|ghost.run.total()<best.run.total()) {profile.trial_ghosts.insert(race.name.clone(),ghost.clone());}
                    if outcome.original_ghost_time.is_some_and(|time|outcome.time<time) {profile.trial_wins.insert(race.name.clone(),true);}
                }
                if !options.time_trial&&!options.versus {profile.record_time(&race.name,outcome.time);}if !options.versus {profile.save(&path)?;}
                if !previous_ghost_unlock&&profile.driver_part_allowed(128,&catalog) {crate::circuit_award::unlock(&library,&catalog,&profile,"VV",&ui,None).await?;}
                if let Some(run)=&mut circuit {for (score,rank) in run.scores.iter_mut().zip(&outcome.ranks) {*score+=catalog.rules.finish_points[(*rank as usize).saturating_sub(1).min(5)];}}
                screen=Screen::Results;
            } else {screen=Screen::Main;circuit=None;}
            ui.selected=0;next_frame().await;
        }
    }
    Ok(())
}
