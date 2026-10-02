//! Original PC circuit selector with native original champion scene previews.
use crate::{game_catalog::Catalog,menu_ui::MenuUi,profile::Profile,cinematic_scene::Scene};
use lrformats::library::Library;
use macroquad::prelude::*;
pub enum Action {None,Back,Start(usize)}
pub struct CircuitMenu {pub selected:usize,slot:usize,loaded:Option<(usize,usize)>,scene:Option<Scene>,start:f64}
impl CircuitMenu {
    pub fn new()->Self {Self {selected:0,slot:0,loaded:None,scene:None,start:0.0}}
    pub fn draw(&mut self,library:&Library,catalog:&Catalog,profile:&Profile,ui:&MenuUi)->Result<Action,String> {
        ui.original_background("Circuit Race",false);
        let previous=ui.icon_button("arrowlu",3.0,29.0)||is_key_pressed(KeyCode::Left);
        let next=ui.icon_button("arrowru",188.0,29.0)||is_key_pressed(KeyCode::Right);
        if previous {self.selected=(self.selected+catalog.circuits.len()-1)%catalog.circuits.len();}if next {self.selected=(self.selected+1)%catalog.circuits.len();}
        ui.image(["pirate","islander","magical","adventur","jungle","alien","rr"][self.selected],35.0,29.0,152.0,40.0);
        let host=&catalog.rules.circuit_names[self.selected];let words=host.split_whitespace().collect::<Vec<_>>();
        let split=if words.len()>2 {2}else {1};
        let races=catalog.circuit_races(&catalog.circuits[self.selected].name);
        if races.is_empty() {return Err("circuit has no original tracks".into());}
        if self.loaded.is_some_and(|(c,_)|c!=self.selected) {self.slot=0;}
        if self.loaded.is_some_and(|(c,_)|c==self.selected)&&self.scene.as_ref().is_some_and(|s|get_time()-self.start>s.duration as f64) {self.slot=(self.slot+1)%races.len();}
        self.slot=self.slot.min(races.len()-1);let race=&races[self.slot];
        ui.centered_text(&catalog.title(&catalog.races[*race]),420.0,112.0,24.0,WHITE);
        ui.preview_frame(257.0,132.0,326.0,261.0);
        if self.loaded!=Some((self.selected,self.slot)) {
            self.scene=Some(crate::track_preview::load(library,catalog,*race,false)?);self.loaded=Some((self.selected,self.slot));self.start=get_time();
        }
        if let Some(scene)=&mut self.scene {scene.draw(Rect::new(260.0,135.0,320.0,255.0),(get_time()-self.start) as f32)?;}
        ui.banner("Circuit Race");
        ui.centered_text("Circuit Number:",122.0,112.0,24.0,WHITE);ui.centered_text(&(self.selected+1).to_string(),122.0,145.0,24.0,WHITE);
        ui.centered_text("Hosting",122.0,222.0,24.0,WHITE);ui.centered_text("Champion:",122.0,255.0,24.0,WHITE);
        ui.centered_text(&words[..split].join(" "),122.0,298.0,24.0,WHITE);ui.centered_text(&words[split..].join(" "),122.0,331.0,24.0,WHITE);
        let unlocked=self.selected as u32<=profile.unlocked_circuit;
        let start=ui.action(if unlocked {"OK"}else {"Locked"},35.0,406.0,Some("chck"))||is_key_pressed(KeyCode::Enter);
        if ui.action("Main Menu",35.0,446.0,Some("txtarol")) {return Ok(Action::Back);}
        Ok(if start&&unlocked {Action::Start(self.selected)}else {Action::None})
    }
}
