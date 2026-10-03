//! Original PCM start/contact sounds on the native audio backend.
//! Engine pitch/mixing and audible-output acceptance remain open.
use crate::platform::audio::{self, PlaySoundParams, Sound};
use lrformats::{library::Library, pcm, wav};

pub struct RaceAudio {
  countdown: Sound,
  go: Sound,
  contact: Sound,
  scrape: Sound,
  racer_contact: Sound,
  contact_cooldown: f32,
  enabled: bool,
  suspended: bool,
  music: crate::race_music::RaceMusic,
  effects: Vec<Sound>,
  pickup: Sound,
  whites: Vec<Sound>,
  effect_gain: f32,
  engine: crate::engine_audio::EngineAudio,
}

impl RaceAudio {
  pub async fn load(
    library: &Library,
    table: &str,
    directory: &std::path::Path,
    enabled: bool,
  ) -> Result<Self, String> {
    Self::load_settings(library, table, directory, enabled, enabled).await
  }

  pub async fn load_settings(
    library: &Library,
    table: &str,
    directory: &std::path::Path,
    enabled: bool,
    music_enabled: bool,
  ) -> Result<Self, String> {
    async fn sound(library: &Library, name: &str) -> Result<Sound, String> {
      let data = library
        .find_in(name, "COMMON")
        .ok_or_else(|| format!("missing original sound{name}"))?;
      let decoded = pcm::decode(data).map_err(|e| format!("{name}:{e}"))?;
      if decoded.sample_rate == 0 || decoded.samples.is_empty() {
        return Err(format!("{name}:empty/invalid decoded audio"));
      }
      audio::load_sound_from_bytes(&wav::encode_mono_16(decoded.sample_rate, &decoded.samples))
        .await
        .map_err(|e| format!("{name}:{e}"))
    }
    let rules = crate::engine_audio::Rules::load()?;
    let mut effects = Vec::new();
    let mut whites = Vec::new();
    for name in rules.effect_files.iter().flatten() {
      effects.push(sound(library, name).await?);
    }
    for name in &rules.white_files {
      whites.push(sound(library, name).await?);
    }
    Ok(Self {
      countdown: sound(library, "321.PCM").await?,
      go: sound(library, "GO.PCM").await?,
      contact: sound(library, "HITENV.PCM").await?,
      scrape: sound(library, "SKRENV.PCM").await?,
      racer_contact: sound(library, "HITCAR.PCM").await?,
      contact_cooldown: 0.0,
      enabled,
      suspended: false,
      effects,
      pickup: sound(library, &rules.pickup_file).await?,
      whites,
      effect_gain: rules.effect_gain,
      engine: crate::engine_audio::EngineAudio::load(library, enabled).await?,
      music: crate::race_music::RaceMusic::load(library, table, directory, music_enabled).await?,
    })
  }

  fn play(&self, sound: &Sound) {
    if self.enabled && !self.suspended {
      audio::play_sound(
        sound,
        PlaySoundParams {
          looped: false,
          volume: 0.5,
        },
      );
    }
  }

  pub fn start(&mut self) {
    self.start_sequence(true);
  }
  pub fn start_sequence(&mut self, countdown: bool) {
    self.set_suspended(false);
    audio::stop_sound(&self.countdown);
    audio::stop_sound(&self.go);
    audio::stop_sound(&self.contact);
    audio::stop_sound(&self.scrape);
    audio::stop_sound(&self.racer_contact);
    self.contact_cooldown = 0.0;
    if countdown {
      self.countdown();
    }
    self.music.start();
  }
  pub fn countdown(&self) {
    self.play(&self.countdown);
  }

  pub fn go(&mut self) {
    audio::stop_sound(&self.countdown);
    self.play(&self.go);
    self.music.race();
  }
  pub fn contact(&self) {
    self.play(&self.contact);
  }
  pub fn driving_feedback(
    &mut self,
    speed: f32,
    forward_speed: f32,
    throttle: f32,
    slip: f32,
    grounded: bool,
    wall_hit: bool,
    racer_hit: bool,
    paused: bool,
    dt: f32,
  ) {
    self.set_suspended(paused);
    if !paused && dt > 0.0 {
      self.contact_cooldown = (self.contact_cooldown - dt).max(0.0);
      if (wall_hit || racer_hit) && self.contact_cooldown == 0.0 {
        self.play(if racer_hit {
          &self.racer_contact
        } else if crate::platform::rand::gen_range(0, 2) == 0 {
          &self.scrape
        } else {
          &self.contact
        });
        // Original 004377f0 and car/car 00438f20 suppress repeat bumps for 250ms.
        self.contact_cooldown = 0.25;
      }
    }
    self
      .engine
      .update_driving(speed, forward_speed, throttle, slip, grounded, paused, dt);
  }
  pub fn finish(&mut self, rank: u32) {
    self.music.finish(rank);
  }
  pub fn music_history(&self) -> Vec<crate::race_music::Cue> {
    self.music.history()
  }
  pub fn pickup(&self, white_count: Option<u8>) {
    self.play(
      white_count
        .map(|count| &self.whites[count.saturating_sub(1).min(2) as usize])
        .unwrap_or(&self.pickup),
    );
  }
  pub fn effect(&self, kind: u8, tier: u8) {
    if self.enabled && !self.suspended && (1..=4).contains(&kind) {
      audio::play_sound(
        &self.effects[(kind as usize - 1) * 4 + tier.min(3) as usize],
        PlaySoundParams {
          looped: false,
          volume: self.effect_gain,
        },
      );
    }
  }
  fn set_suspended(&mut self, paused: bool) {
    if self.suspended != paused {
      self.suspended = paused;
      self.music.set_muted(paused);
      for sound in self.whites.iter().chain([
        &self.pickup,
        &self.contact,
        &self.scrape,
        &self.racer_contact,
        &self.countdown,
        &self.go,
      ]) {
        audio::set_sound_volume(sound, if paused { 0.0 } else { 0.5 });
      }
      for sound in &self.effects {
        audio::set_sound_volume(sound, if paused { 0.0 } else { self.effect_gain });
      }
    }
  }
  pub fn engine(&mut self, speed: f32, steer: f32, paused: bool) {
    self.set_suspended(paused);
    self.engine.update(speed, steer, paused);
  }
}

impl Drop for RaceAudio {
  fn drop(&mut self) {
    for sound in self.effects.iter().chain(&self.whites).chain([
      &self.pickup,
      &self.contact,
      &self.scrape,
      &self.racer_contact,
      &self.countdown,
      &self.go,
    ]) {
      audio::stop_sound(sound);
    }
  }
}
