//! Timed original cinematic PCM dispatch; diagnostic scenes remain silent.
use crate::platform::audio::{self, PlaySoundParams, Sound};
use lrformats::{library::Library, pcm, wav};
use std::collections::HashMap;
pub struct Audio {
  sounds: HashMap<String, Sound>,
  cues: Vec<lrformats::cinematic_audio::Cue>,
  next: usize,
  enabled: bool,
  pub unavailable_banks: Vec<String>,
}
impl Audio {
  pub async fn load(
    library: &Library,
    table: &str,
    cdb: &str,
    scene: &crate::cinematic_scene::Scene,
    enabled: bool,
  ) -> Result<Self, String> {
    let plan = lrformats::cinematic_audio::plan(library, table, cdb, &scene.events)?;
    let cues = plan.cues;
    let mut sounds = HashMap::new();
    for cue in &cues {
      if sounds.contains_key(&cue.file) {
        continue;
      }
      let bytes = library
        .find_at(&cue.file, "MENUDATA", table)
        .or_else(|| library.find_in(&cue.file, "COMMON"))
        .ok_or_else(|| format!("missing original cinematic PCM {}", cue.file))?;
      let sound = pcm::decode(bytes).map_err(|e| e.to_string())?;
      if sound.sample_rate == 0 || sound.samples.is_empty() {
        return Err("empty original cinematic PCM".into());
      }
      sounds.insert(
        cue.file.clone(),
        audio::load_sound_from_bytes(&wav::encode_mono_16(sound.sample_rate, &sound.samples))
          .await
          .map_err(|e| e.to_string())?,
      );
    }
    Ok(Self {
      sounds,
      cues,
      next: 0,
      enabled,
      unavailable_banks: plan.unavailable_banks,
    })
  }
  pub fn advance(&mut self, frame: f32) {
    while let Some(cue) = self.cues.get(self.next).filter(|c| c.frame as f32 <= frame) {
      if self.enabled {
        audio::play_sound(
          &self.sounds[&cue.file],
          PlaySoundParams {
            looped: false,
            volume: (cue.gain * 0.5).min(1.0),
          },
        );
      }
      self.next += 1;
    }
  }
  pub fn dispatched(&self) -> usize {
    self.next
  }
}
impl Drop for Audio {
  fn drop(&mut self) {
    for sound in self.sounds.values() {
      audio::stop_sound(sound);
    }
  }
}
