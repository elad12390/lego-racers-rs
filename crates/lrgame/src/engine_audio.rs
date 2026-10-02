//! Original PCM clips on the existing native backend, no synthesized engine.
//! Discrete sample-rate pitch bank is a modern mixer adaptation, provisional.
use lrformats::{library::Library,pcm,wav};
use macroquad::audio::{self,Sound,PlaySoundParams};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Rules {
    engine_file:String,idle_file:String,skid_file:String,pub effect_files:[[String;4];4],pub pickup_file:String,pub white_files:[String;3],
    engine_pitch_steps:usize,engine_pitch_min:f32,engine_pitch_max:f32,engine_gain:f32,idle_gain:f32,skid_gain:f32,pub effect_gain:f32,reference_speed:f32,
}
impl Rules {
    pub fn load()->Result<Self,String> {
        let rules:Self=serde_json::from_str(include_str!("../../../assets/native/audio.json")).map_err(|e|e.to_string())?;
        if !(2..=32).contains(&rules.engine_pitch_steps)||rules.engine_pitch_min<=0.0||rules.engine_pitch_max<rules.engine_pitch_min||rules.reference_speed<=0.0 {return Err("invalid native audio configuration".into());}
        Ok(rules)
    }
}
pub async fn sound(library:&Library,name:&str)->Result<Sound,String> {
    let decoded=pcm::decode(library.find_in(name,"COMMON").ok_or_else(||format!("missing original sound {name}"))?).map_err(|e|e.to_string())?;
    if decoded.sample_rate==0||decoded.samples.is_empty() {return Err(format!("invalid original sound {name}"));}
    audio::load_sound_from_bytes(&wav::encode_mono_16(decoded.sample_rate,&decoded.samples)).await.map_err(|e|e.to_string())
}
pub struct EngineAudio {sounds:Vec<Sound>,idle:Sound,skid:Sound,active:Option<usize>,enabled:bool,rules:Rules,started:bool}
impl EngineAudio {
    pub async fn load(library:&Library,enabled:bool)->Result<Self,String> {
        let rules=Rules::load()?;let decoded=pcm::decode(library.find_in(&rules.engine_file,"COMMON").ok_or("missing original engine")?).map_err(|e|e.to_string())?;
        let mut sounds=Vec::new();
        for index in 0..rules.engine_pitch_steps {
            let pitch=rules.engine_pitch_min+(rules.engine_pitch_max-rules.engine_pitch_min)*index as f32/(rules.engine_pitch_steps-1) as f32;
            sounds.push(audio::load_sound_from_bytes(&wav::encode_mono_16((decoded.sample_rate as f32*pitch) as u32,&decoded.samples)).await.map_err(|e|e.to_string())?);
        }
        Ok(Self {sounds,idle:sound(library,&rules.idle_file).await?,skid:sound(library,&rules.skid_file).await?,active:None,enabled,rules,started:false})
    }
    pub fn update(&mut self,speed:f32,steer:f32,paused:bool) {
        if !self.enabled {return;}
        if !self.started {for sound in [&self.idle,&self.skid] {audio::play_sound(sound,PlaySoundParams {looped:true,volume:0.0});}self.started=true;}
        let amount=(speed.abs()/self.rules.reference_speed).clamp(0.0,1.0);
        let index=(amount*(self.sounds.len()-1) as f32).round() as usize;
        if self.active!=Some(index) {if let Some(old)=self.active {audio::stop_sound(&self.sounds[old]);}audio::play_sound(&self.sounds[index],PlaySoundParams {looped:true,volume:0.0});self.active=Some(index);}
        let gain=if paused {0.0} else {1.0};
        audio::set_sound_volume(&self.sounds[index],gain*self.rules.engine_gain*amount);
        audio::set_sound_volume(&self.idle,gain*self.rules.idle_gain*(1.0-amount));
        audio::set_sound_volume(&self.skid,gain*self.rules.skid_gain*amount*steer.abs());
    }
}
impl Drop for EngineAudio {fn drop(&mut self) {for sound in self.sounds.iter().chain([&self.idle,&self.skid]) {audio::stop_sound(sound);}}}
