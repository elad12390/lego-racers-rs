//! Source-selected original tunes on the native backend, gain provisional.
use lrformats::{library::Library,music_catalog,tun,wav};
use macroquad::{audio::{self,Sound,PlaySoundParams},rand};
use serde::Serialize;
use std::{collections::HashMap,path::Path};

#[derive(Clone,Serialize)]
pub struct Cue {pub state:&'static str,pub index:usize,pub tune:String,pub looped:bool}

pub struct RaceMusic {
    names:Vec<String>,
    sounds:Vec<Sound>,
    enabled:bool,
    history:Vec<Cue>,
}

impl RaceMusic {
    pub async fn load(library:&Library,table:&str,directory:&Path,enabled:bool)->Result<Self,String> {
        let names=music_catalog::parse(library.find_in("LEGOMSC",table).ok_or("missing race music catalog")?)?;
        let mut files=HashMap::new();
        for entry in std::fs::read_dir(directory).map_err(|e|e.to_string())? {
            let entry=entry.map_err(|e|e.to_string())?;
            if entry.file_type().map_err(|e|e.to_string())?.is_file() {
                let name=entry.file_name().to_string_lossy().to_ascii_lowercase();
                if name.ends_with(".tun") && files.insert(name.clone(),entry.path()).is_some() {return Err(format!("ambiguous original tune {name}"));}
            }
        }
        let mut sounds=Vec::new();
        for name in &names {
            let file=files.get(&name.to_ascii_lowercase()).ok_or_else(||format!("missing original tune {name}"))?;
            let bytes=std::fs::read(file).map_err(|e|format!("{name}: {e}"))?;
            let decoded=tun::decode(&bytes).map_err(|e|format!("{name}: {e}"))?;
            if decoded.samples.is_empty() {return Err(format!("{name}: empty tune"));}
            sounds.push(audio::load_sound_from_bytes(&wav::encode_16(decoded.sample_rate,decoded.channels,&decoded.samples)).await.map_err(|e|format!("{name}: {e}"))?);
        }
        Ok(Self {names,sounds,enabled,history:Vec::new()})
    }

    fn select(&mut self,state:&'static str,index:usize,looped:bool) {
        for sound in &self.sounds {audio::stop_sound(sound);}
        self.history.push(Cue {state,index,tune:self.names[index].clone(),looped});
        // Scripted diagnostic runs load the actual backend and record selection
        // but remain silent. Fixed gain is provisional, not original mixer QA.
        if self.enabled {audio::play_sound(&self.sounds[index],PlaySoundParams {looped,volume:0.5});}
    }
    pub fn start(&mut self) {self.select("countdown",0,false);}
    pub fn race(&mut self) {
        let index=lrsim::music_selection::race_index(self.names.len(),false,rand::gen_range(0,65536) as u16);
        self.select("race",index,true);
    }
    pub fn finish(&mut self,rank:u32) {self.select("finish",lrsim::music_selection::finish_index(rank),false);}
    pub fn history(&self)->Vec<Cue> {self.history.clone()}
}

impl Drop for RaceMusic {
    fn drop(&mut self) {for sound in &self.sounds {audio::stop_sound(sound);}}
}
