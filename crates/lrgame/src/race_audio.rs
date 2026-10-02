//! Original PCM start/contact sounds on the native audio backend.
//! Engine pitch/mixing and audible-output acceptance remain open.
use lrformats::{library::Library,pcm,wav};
use macroquad::audio::{self,Sound,PlaySoundParams};

pub struct RaceAudio {countdown:Sound,go:Sound,contact:Sound,enabled:bool,music:crate::race_music::RaceMusic,
    effects:Vec<Sound>,pickup:Sound,whites:Vec<Sound>,effect_gain:f32,engine:crate::engine_audio::EngineAudio}

impl RaceAudio {
    pub async fn load(library:&Library,table:&str,directory:&std::path::Path,enabled:bool)->Result<Self,String> {
        Self::load_settings(library,table,directory,enabled,enabled).await
    }

    pub async fn load_settings(library:&Library,table:&str,directory:&std::path::Path,enabled:bool,music_enabled:bool)->Result<Self,String> {
        async fn sound(library:&Library,name:&str)->Result<Sound,String> {
            let data=library.find_in(name,"COMMON").ok_or_else(||format!("missing original sound{name}"))?;
            let decoded=pcm::decode(data).map_err(|e|format!("{name}:{e}"))?;
            if decoded.sample_rate==0 || decoded.samples.is_empty() {return Err(format!("{name}:empty/invalid decoded audio"));}
            audio::load_sound_from_bytes(&wav::encode_mono_16(decoded.sample_rate,&decoded.samples)).await.map_err(|e|format!("{name}:{e}"))
        }
        let rules=crate::engine_audio::Rules::load()?;let mut effects=Vec::new();let mut whites=Vec::new();
        for name in rules.effect_files.iter().flatten() {effects.push(sound(library,name).await?);}
        for name in &rules.white_files {whites.push(sound(library,name).await?);}
        Ok(Self {countdown:sound(library,"321.PCM").await?,go:sound(library,"GO.PCM").await?,contact:sound(library,"HITENV.PCM").await?,enabled,
            effects,pickup:sound(library,&rules.pickup_file).await?,whites,effect_gain:rules.effect_gain,engine:crate::engine_audio::EngineAudio::load(library,enabled).await?,
            music:crate::race_music::RaceMusic::load(library,table,directory,music_enabled).await?})
    }

    fn play(&self,sound:&Sound) {if self.enabled {audio::play_sound(sound,PlaySoundParams {looped:false,volume:0.5});}}

    pub fn start(&mut self) {
        audio::stop_sound(&self.countdown);audio::stop_sound(&self.go);audio::stop_sound(&self.contact);
        self.play(&self.countdown);
        self.music.start();
    }

    pub fn go(&mut self) {audio::stop_sound(&self.countdown);self.play(&self.go);self.music.race();}
    pub fn contact(&self) {self.play(&self.contact);}
    pub fn finish(&mut self,rank:u32) {self.music.finish(rank);}
    pub fn music_history(&self)->Vec<crate::race_music::Cue> {self.music.history()}
    pub fn pickup(&self,white_count:Option<u8>) {self.play(white_count.map(|count|&self.whites[count.saturating_sub(1).min(2) as usize]).unwrap_or(&self.pickup));}
    pub fn effect(&self,kind:u8,tier:u8) {if self.enabled&&(1..=4).contains(&kind) {audio::play_sound(&self.effects[(kind as usize-1)*4+tier.min(3) as usize],PlaySoundParams {looped:false,volume:self.effect_gain});}}
    pub fn engine(&mut self,speed:f32,steer:f32,paused:bool) {self.engine.update(speed,steer,paused);}
}

impl Drop for RaceAudio {fn drop(&mut self) {for sound in self.effects.iter().chain(&self.whites).chain([&self.pickup,&self.contact,&self.countdown,&self.go]) {audio::stop_sound(sound);}}}
