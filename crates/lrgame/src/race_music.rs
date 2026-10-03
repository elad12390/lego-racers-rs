//! Source-selected original tunes on the native backend, gain provisional.
use crate::platform::{
  audio::{self, PlaySoundParams, Sound},
  rand,
};
use lrformats::{library::Library, music_catalog};
use serde::Serialize;
use std::path::Path;

#[derive(Clone, Serialize)]
pub struct Cue {
  pub state: &'static str,
  pub index: usize,
  pub tune: String,
  pub looped: bool,
}

pub struct RaceMusic {
  names: Vec<String>,
  sounds: Vec<Sound>,
  enabled: bool,
  muted: bool,
  history: Vec<Cue>,
}

impl RaceMusic {
  pub async fn load(
    library: &Library,
    table: &str,
    directory: &Path,
    enabled: bool,
  ) -> Result<Self, String> {
    let names = music_catalog::parse(
      library
        .find_in("LEGOMSC", table)
        .ok_or("missing race music catalog")?,
    )?;
    let mut sounds = Vec::new();
    for name in &names {
      sounds.push(crate::audio_assets::tune(directory, name).await?);
    }
    Ok(Self {
      names,
      sounds,
      enabled,
      muted: false,
      history: Vec::new(),
    })
  }

  fn select(&mut self, state: &'static str, index: usize, looped: bool) {
    for sound in &self.sounds {
      audio::stop_sound(sound);
    }
    self.history.push(Cue {
      state,
      index,
      tune: self.names[index].clone(),
      looped,
    });
    // Scripted diagnostic runs load the actual backend and record selection
    // but remain silent. Fixed gain is provisional, not original mixer QA.
    if self.enabled {
      audio::play_sound(
        &self.sounds[index],
        PlaySoundParams {
          looped,
          volume: if self.muted { 0.0 } else { 0.5 },
        },
      );
    }
  }
  pub fn start(&mut self) {
    self.select("countdown", 0, false);
  }
  pub fn race(&mut self) {
    let index =
      lrsim::music_selection::race_index(self.names.len(), false, rand::gen_range(0, 65536) as u16);
    self.select("race", index, true);
  }
  pub fn finish(&mut self, rank: u32) {
    self.select("finish", lrsim::music_selection::finish_index(rank), false);
  }
  pub fn history(&self) -> Vec<Cue> {
    self.history.clone()
  }
  pub fn set_muted(&mut self, muted: bool) {
    if self.muted != muted {
      self.muted = muted;
      for sound in &self.sounds {
        audio::set_sound_volume(sound, if muted { 0.0 } else { 0.5 });
      }
    }
  }
}

impl Drop for RaceMusic {
  fn drop(&mut self) {
    for sound in &self.sounds {
      audio::stop_sound(sound);
    }
  }
}
