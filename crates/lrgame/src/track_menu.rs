//! PC single/time track selection; the preview is original animated geometry.
use crate::{game_catalog::Catalog,profile::Profile,menu_ui::MenuUi,cinematic_scene::Scene,track_preview};
use lrformats::library::Library;
use macroquad::prelude::*;
pub enum Action {None,Back,Start(usize)}
pub struct TrackMenu {pub circuit:usize,pub slot:usize,loaded:Option<(usize,bool)>,scene:Option<Scene>,start:f64}
impl TrackMenu {
    pub fn new()->Self {Self {circuit:0,slot:0,loaded:None,scene:None,start:0.0}}
    pub fn draw(&mut self,library:&Library,catalog:&Catalog,profile:&Profile,ui:&MenuUi,time_trial:bool)->Result<Action,String> {
        ui.original_background(if time_trial {"Time Race"}else {"Single Race"},false);
        if ui.icon_button("arrowlu",3.0,29.0)||is_key_pressed(KeyCode::Up) {self.circuit=(self.circuit+catalog.circuits.len()-1)%catalog.circuits.len();self.slot=0;}
        if ui.icon_button("arrowru",188.0,29.0)||is_key_pressed(KeyCode::Down) {self.circuit=(self.circuit+1)%catalog.circuits.len();self.slot=0;}
        ui.image(["pirate","islander","magical","adventur","jungle","alien","rr"][self.circuit],35.0,29.0,152.0,40.0);
        let races=catalog.circuit_races(&catalog.circuits[self.circuit].name);if races.is_empty() {return Err("selected circuit has no tracks".into());}self.slot=self.slot.min(races.len()-1);
        if ui.icon_button("arrowlu",239.0,87.0)||is_key_pressed(KeyCode::Left) {self.slot=(self.slot+races.len()-1)%races.len();}
        if ui.icon_button("arrowru",572.0,87.0)||is_key_pressed(KeyCode::Right) {self.slot=(self.slot+1)%races.len();}
        let index=races[self.slot];ui.centered_text(&catalog.title(&catalog.races[index]),420.0,112.0,24.0,YELLOW);ui.preview_frame(257.0,132.0,326.0,261.0);
        if self.loaded!=Some((index,time_trial)) {self.scene=Some(track_preview::load(library,catalog,index,time_trial)?);self.loaded=Some((index,time_trial));self.start=get_time();}
        self.scene.as_mut().unwrap().draw(track_preview::VIEWPORT,(get_time()-self.start) as f32)?;
        if time_trial {ui.centered_text("Best Lap Time",130.0,197.0,24.0,WHITE);let best=profile.trial_ghosts.get(&catalog.races[index].name).and_then(|g|g.run.lap_times.iter().copied().reduce(f64::min));ui.centered_text(&best.map_or("None".into(),|t|format!("{}:{:05.2}",(t/60.0) as u32,t%60.0)),130.0,237.0,24.0,WHITE);}
        let unlocked=self.circuit as u32<=profile.unlocked_circuit;
        let done=ui.action(if unlocked {"OK"}else {"Locked"},35.0,406.0,Some("chck"))||is_key_pressed(KeyCode::Enter);
        if ui.action("Main Menu",35.0,446.0,Some("txtarol")) {return Ok(Action::Back);}Ok(if done&&unlocked {Action::Start(index)}else {Action::None})
    }
}
