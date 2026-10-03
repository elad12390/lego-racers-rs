//! Versioned native saves. Original .LRS files are never overwritten.
use serde::{Deserialize, Serialize};
use std::{
  collections::BTreeMap,
  path::{Path, PathBuf},
};

#[derive(Serialize, Deserialize, Clone)]
pub struct Profile {
  pub version: u32,
  pub name: String,
  pub car: String,
  pub driver: String,
  pub unlocked_circuit: u32,
  pub circuit_medals: BTreeMap<String, u32>,
  pub best_times: BTreeMap<String, f64>,
  pub music: bool,
  pub sound: bool,
  #[serde(default)]
  pub camera_mode: crate::camera_rig::Mode,
  #[serde(default)]
  pub custom_build: Option<lrsim::brick_build::Build>,
  #[serde(default)]
  pub custom_enabled: bool,
  #[serde(default)]
  pub trial_ghosts: BTreeMap<String, TrialGhost>,
  #[serde(default)]
  pub trial_wins: BTreeMap<String, bool>,
  #[serde(default)]
  pub custom_driver: Option<crate::custom_driver::Build>,
  #[serde(default)]
  pub license_photo: Option<Photo>,
  #[serde(default)]
  pub license_expression: u8,
  #[serde(default)]
  pub racers: Vec<crate::garage_roster::Racer>,
  #[serde(default)]
  pub active_racer: usize,
  /// Native convenience: resume at the next race, never mid-race.
  #[serde(default)]
  pub career: Option<CareerProgress>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct CareerProgress {
  pub circuit: String,
  pub completed: usize,
  pub scores: [u32; 6],
  pub names: [String; 6],
}
#[derive(Clone, Serialize, Deserialize)]
pub struct TrialGhost {
  pub car: String,
  pub driver: String,
  pub build: Option<lrsim::brick_build::Build>,
  #[serde(default)]
  pub driver_build: Option<crate::custom_driver::Build>,
  pub run: lrsim::ghost_run::Run,
}
#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct Photo {
  pub width: u16,
  pub height: u16,
  pub rgba: Vec<u8>,
}
impl Default for Profile {
  fn default() -> Self {
    Self {
      version: 1,
      name: "Racer".into(),
      car: "BK".into(),
      driver: "BK".into(),
      unlocked_circuit: 0,
      circuit_medals: BTreeMap::new(),
      best_times: BTreeMap::new(),
      music: true,
      sound: true,
      camera_mode: crate::camera_rig::Mode::default(),
      custom_build: None,
      custom_enabled: false,
      trial_ghosts: BTreeMap::new(),
      trial_wins: BTreeMap::new(),
      custom_driver: None,
      license_photo: None,
      license_expression: 0,
      racers: Vec::new(),
      active_racer: 0,
      career: None,
    }
  }
}
impl Profile {
  pub fn path() -> PathBuf {
    std::env::var_os("HOME")
      .map(PathBuf::from)
      .unwrap_or_default()
      .join("Library/Application Support/LEGO Racers Native/profile.json")
  }
  pub fn load(path: &Path) -> Result<Self, String> {
    match std::fs::read(path) {
      Ok(bytes) => {
        let mut profile: Self = serde_json::from_slice(&bytes)
          .map_err(|e| format!("Save is invalid; preserved at {}: {e}", path.display()))?;
        if profile.version != 1
          || profile.license_expression >= 6
          || profile.racers.iter().any(|r| r.license_expression >= 6)
          || profile.unlocked_circuit > 6
          || profile.career.as_ref().is_some_and(|c| c.completed > 4)
          || profile
            .best_times
            .values()
            .any(|v| !v.is_finite() || *v <= 0.0)
        {
          return Err("unsupported or invalid save; original file preserved".into());
        }
        for ghost in profile.trial_ghosts.values() {
          ghost.run.validate()?;
        }
        if let Some(photo) = &profile.license_photo {
          if photo.width == 0
            || photo.height == 0
            || photo.width > 256
            || photo.height > 256
            || photo.rgba.len() != photo.width as usize * photo.height as usize * 4
          {
            return Err("invalid license photo; save preserved".into());
          }
        }
        if profile.racers.len() > 8
          || (!profile.racers.is_empty() && profile.active_racer >= profile.racers.len())
        {
          return Err("invalid native garage slots; save preserved".into());
        }
        profile.sync_racer();
        Ok(profile)
      }
      Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
      Err(e) => Err(e.to_string()),
    }
  }
  pub fn save(&self, path: &Path) -> Result<(), String> {
    let mut saved = self.clone();
    saved.sync_racer();
    let parent = path
      .parent()
      .filter(|p| !p.as_os_str().is_empty())
      .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let temporary = parent.join(format!(
      ".profile-{}-{}.tmp",
      std::process::id(),
      std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos()
    ));
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
      .write(true)
      .create_new(true)
      .open(&temporary)
      .map_err(|e| e.to_string())?;
    file
      .write_all(&serde_json::to_vec_pretty(&saved).map_err(|e| e.to_string())?)
      .and_then(|_| file.sync_all())
      .map_err(|e| e.to_string())?;
    std::fs::rename(&temporary, path).map_err(|e| e.to_string())
  }
  pub fn record_time(&mut self, race: &str, time: f64) {
    if time.is_finite() && time > 0.0 {
      let best = self.best_times.entry(race.into()).or_insert(time);
      *best = best.min(time);
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn recorded_finish_time_survives_native_json_save_exactly() {
    let mut profile = Profile::default();
    profile.record_time("rkr", 111.51667263917625);
    let restored: Profile = serde_json::from_slice(&serde_json::to_vec(&profile).unwrap()).unwrap();
    assert_eq!(profile.best_times, restored.best_times);
  }

  #[test]
  fn camera_setting_restores_and_older_saves_default_to_chase() {
    let mut profile = Profile::default();
    profile.camera_mode = crate::camera_rig::Mode::Cockpit;
    let mut saved = serde_json::to_value(&profile).unwrap();
    let restored: Profile = serde_json::from_value(saved.clone()).unwrap();
    assert_eq!(restored.camera_mode, crate::camera_rig::Mode::Cockpit);
    saved.as_object_mut().unwrap().remove("camera_mode");
    let legacy: Profile = serde_json::from_value(saved).unwrap();
    assert_eq!(legacy.camera_mode, crate::camera_rig::Mode::Chase);
  }

  #[test]
  fn license_expressions_default_for_old_saves_and_invalid_variants_preserve_the_file() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
      "../../tmp/license-expression-{}.json",
      std::process::id()
    ));
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut profile = Profile::default();
    profile.license_expression = 5;
    profile.sync_racer();
    let mut legacy = serde_json::to_value(&profile).unwrap();
    legacy.as_object_mut().unwrap().remove("license_expression");
    legacy["racers"][0]
      .as_object_mut()
      .unwrap()
      .remove("license_expression");
    std::fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
    assert_eq!(Profile::load(&path).unwrap().license_expression, 0);
    for invalid_slot in [false, true] {
      let mut invalid = serde_json::to_value(&profile).unwrap();
      if invalid_slot {
        invalid["racers"][0]["license_expression"] = 6.into();
      } else {
        invalid["license_expression"] = 6.into();
      }
      let bytes = serde_json::to_vec(&invalid).unwrap();
      std::fs::write(&path, &bytes).unwrap();
      assert!(Profile::load(&path).is_err());
      assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
    std::fs::remove_file(path).unwrap();
  }

  #[test]
  fn audio_settings_save_and_restore_music_and_sound_independently() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
      "../../tmp/audio-settings-{}-{}.json",
      std::process::id(),
      std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
    ));
    let default = Profile::load(&path).unwrap();
    assert!(
      default.music && default.sound,
      "new profiles must start with audio enabled"
    );
    for (music, sound) in [(true, false), (false, true), (false, false), (true, true)] {
      let mut profile = default.clone();
      profile.music = music;
      profile.sound = sound;
      profile.save(&path).unwrap();
      let restored = Profile::load(&path).unwrap();
      assert_eq!((restored.music, restored.sound), (music, sound));
    }
    std::fs::remove_file(path).unwrap();
  }
}
