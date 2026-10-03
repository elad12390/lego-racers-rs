//! Native racer slots; edits, copies and selection share one atomic profile save.
use crate::{
  custom_driver,
  profile::{Photo, Profile},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize, PartialEq)]
pub struct Racer {
  pub name: String,
  pub car: String,
  pub driver: String,
  pub custom_build: Option<lrsim::brick_build::Build>,
  pub custom_enabled: bool,
  pub custom_driver: Option<custom_driver::Build>,
  pub license_photo: Option<Photo>,
  #[serde(default)]
  pub license_expression: u8,
}
impl Racer {
  pub fn from_profile(profile: &Profile) -> Self {
    Self {
      name: profile.name.clone(),
      car: profile.car.clone(),
      driver: profile.driver.clone(),
      custom_build: profile.custom_build.clone(),
      custom_enabled: profile.custom_enabled,
      custom_driver: profile.custom_driver.clone(),
      license_photo: profile.license_photo.clone(),
      license_expression: profile.license_expression,
    }
  }
  fn apply(&self, profile: &mut Profile) {
    profile.name = self.name.clone();
    profile.car = self.car.clone();
    profile.driver = self.driver.clone();
    profile.custom_build = self.custom_build.clone();
    profile.custom_enabled = self.custom_enabled;
    profile.custom_driver = self.custom_driver.clone();
    profile.license_photo = self.license_photo.clone();
    profile.license_expression = self.license_expression;
  }
}
impl Profile {
  pub fn sync_racer(&mut self) {
    let racer = Racer::from_profile(self);
    if self.racers.is_empty() {
      self.racers.push(racer);
      self.active_racer = 0;
    } else {
      self.racers[self.active_racer] = racer;
    }
  }
  pub fn select_racer(&mut self, index: usize) -> Result<(), String> {
    self.sync_racer();
    let racer = self
      .racers
      .get(index)
      .ok_or("racer slot outside garage")?
      .clone();
    self.active_racer = index;
    racer.apply(self);
    Ok(())
  }
  pub fn new_racer(&mut self) -> Result<(), String> {
    self.sync_racer();
    if self.racers.len() >= 8 {
      return Err("garage has eight racers; delete a racer before creating another".into());
    }
    let mut racer = Racer::from_profile(&Profile::default());
    racer.name = format!("Racer {}", self.racers.len() + 1);
    self.racers.push(racer);
    self.select_racer(self.racers.len() - 1)
  }
  pub fn copy_racer(&mut self) -> Result<(), String> {
    self.sync_racer();
    if self.racers.len() >= 8 {
      return Err("garage has eight racers; delete a racer before copying".into());
    }
    let mut racer = Racer::from_profile(self);
    racer.name = format!("{} COPY", racer.name.chars().take(11).collect::<String>());
    self.racers.push(racer);
    self.select_racer(self.racers.len() - 1)
  }
  pub fn delete_racer(&mut self) -> Result<(), String> {
    self.sync_racer();
    if self.racers.len() == 1 {
      return Err("keep at least one racer in the garage".into());
    }
    self.racers.remove(self.active_racer);
    self.active_racer = self.active_racer.min(self.racers.len() - 1);
    self.racers[self.active_racer].clone().apply(self);
    Ok(())
  }
}
#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn editing_copying_selecting_and_deleting_preserve_other_racers_on_disk() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tmp");
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join(format!("garage-roster-{}.json", std::process::id()));
    let mut profile = Profile::default();
    profile.license_expression = 4;
    profile.name = "ORIGINAL".into();
    profile.custom_driver = Some(custom_driver::Build {
      hat: 15,
      face: 22,
      torso: 22,
      legs: 11,
    });
    profile.copy_racer().unwrap();
    assert_eq!(profile.license_expression, 4);
    profile.license_expression = 3;
    profile.name = "COPY EDIT".into();
    profile.save(&path).unwrap();
    let mut loaded = Profile::load(&path).unwrap();
    assert_eq!(loaded.name, "COPY EDIT");
    assert_eq!(loaded.license_expression, 3);
    loaded.select_racer(0).unwrap();
    assert_eq!(loaded.name, "ORIGINAL");
    assert_eq!(loaded.license_expression, 4);
    assert_eq!(loaded.custom_driver, profile.custom_driver);
    loaded.new_racer().unwrap();
    assert_eq!(loaded.license_expression, 0);
    assert!(loaded.custom_driver.is_none());
    loaded.delete_racer().unwrap();
    loaded.save(&path).unwrap();
    let mut loaded = Profile::load(&path).unwrap();
    assert_eq!(loaded.racers.len(), 2);
    loaded.select_racer(1).unwrap();
    assert_eq!(loaded.name, "COPY EDIT");
    assert_eq!(loaded.license_expression, 3);
    std::fs::remove_file(path).unwrap();
  }
}
