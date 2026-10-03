//! Driver/license/car edits are one local transaction. No partial racer is
//! saved on creation, back-navigation or cancellation.
use crate::profile::Profile;
use std::path::Path;

pub struct Draft {
  before: Profile,
  pub creating: bool,
}
impl Draft {
  pub fn new_racer(profile: &mut Profile) -> Result<Self, String> {
    let before = profile.clone();
    profile.new_racer()?;
    Ok(Self {
      before,
      creating: true,
    })
  }
  pub fn edit(profile: &Profile) -> Self {
    Self {
      before: profile.clone(),
      creating: false,
    }
  }
  pub fn cancel(self, profile: &mut Profile) {
    *profile = self.before;
  }
  pub fn commit(self, profile: &mut Profile, path: &Path) -> Result<(), String> {
    if let Err(error) = profile.save(path) {
      self.cancel(profile);
      return Err(error);
    }
    profile.sync_racer();
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  fn directory() -> std::path::PathBuf {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
      .join("../../tmp")
      .join(format!(
        "garage-draft-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
          .duration_since(std::time::UNIX_EPOCH)
          .unwrap()
          .as_nanos()
      ));
    std::fs::create_dir_all(&root).unwrap();
    root
  }
  #[test]
  fn canceled_new_racer_keeps_original_slots_and_on_disk_progress() {
    let root = directory();
    let path = root.join("profile.json");
    let mut profile = Profile::default();
    profile.name = "ORIGINAL".into();
    profile.unlocked_circuit = 4;
    profile.best_times.insert("rkr".into(), 111.25);
    profile.sync_racer();
    profile.save(&path).unwrap();
    let original = std::fs::read(&path).unwrap();
    let draft = Draft::new_racer(&mut profile).unwrap();
    profile.name = "UNFINISHED".into();
    profile.custom_driver = Some(crate::custom_driver::Build {
      hat: 15,
      face: 22,
      torso: 22,
      legs: 11,
    });
    assert_eq!(std::fs::read(&path).unwrap(), original);
    draft.cancel(&mut profile);
    assert_eq!(profile.name, "ORIGINAL");
    assert_eq!(profile.racers.len(), 1);
    assert_eq!(profile.unlocked_circuit, 4);
    assert_eq!(profile.best_times["rkr"], 111.25);
    assert_eq!(std::fs::read(&path).unwrap(), original);
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(root).unwrap();
  }
  #[test]
  fn complete_racer_commits_all_changes_together_and_failed_save_rolls_back() {
    let root = directory();
    let path = root.join("profile.json");
    let mut profile = Profile::default();
    profile.sync_racer();
    profile.save(&path).unwrap();
    let draft = Draft::new_racer(&mut profile).unwrap();
    profile.name = "COMPLETE".into();
    profile.car = "KK".into();
    profile.custom_driver = Some(crate::custom_driver::Build {
      hat: 15,
      face: 22,
      torso: 22,
      legs: 11,
    });
    profile.license_photo = Some(crate::profile::Photo {
      width: 1,
      height: 1,
      rgba: vec![20, 30, 40, 255],
    });
    assert_eq!(Profile::load(&path).unwrap().racers.len(), 1);
    draft.commit(&mut profile, &path).unwrap();
    let restored = Profile::load(&path).unwrap();
    assert_eq!(restored.racers.len(), 2);
    assert_eq!(restored.name, "COMPLETE");
    assert_eq!(restored.car, "KK");
    assert_eq!(restored.custom_driver, profile.custom_driver);
    assert!(restored.license_photo == profile.license_photo);
    let original = std::fs::read(&path).unwrap();
    let draft = Draft::edit(&profile);
    profile.name = "FAILED".into();
    assert!(draft
      .commit(&mut profile, &path.join("child.json"))
      .is_err());
    assert_eq!(profile.name, "COMPLETE");
    assert_eq!(std::fs::read(&path).unwrap(), original);
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(root).unwrap();
  }
}
