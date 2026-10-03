use crate::platform::prelude::*;
use crate::{
  gpu::{world_position, TrackGpu},
  options::Options,
};
use lrformats::{library::Library, model::Model, world};
use lrsim::{contact::Contacts, handling::Handling, race_data::RaceData};

pub struct DriveAssets {
  pub contacts: Contacts,
  pub chassis: lrformats::cmb::Chassis,
  pub start: world::StartPosition,
  pub body: TrackGpu,
  pub wheels: crate::wheels::Wheels,
  pub driver: crate::driver_animation::DriverAnimation,
  pub driver_seat: Mat4,
  pub scenery: Vec<crate::static_scenery::Object>,
  pub billboards: Vec<crate::world_billboards::Billboards>,
  pub animated_scenery: Vec<crate::animated_scenery::Object>,
  pub diagnostic_line: Vec<[f32; 3]>,
  pub lap_zones: lrsim::lap_zones::LapZones,
  pub checkpoints: lrsim::checkpoint_contacts::CheckpointContacts,
  pub environment: crate::environment::Environment,
}

impl DriveAssets {
  pub fn update_world(&mut self, dt: f32) -> Result<(), String> {
    self.environment.update(dt)?;
    for object in &mut self.scenery {
      object.update(dt)?;
    }
    for object in &mut self.animated_scenery {
      object.update(dt)?;
    }
    Ok(())
  }
  pub fn reset_world(&mut self) -> Result<(), String> {
    self.environment.reset()?;
    for object in &mut self.scenery {
      object.reset()?;
    }
    for object in &mut self.animated_scenery {
      object.reset()?;
    }
    Ok(())
  }
  pub fn load(options: &Options) -> Result<Self, String> {
    let library = Library::open(&options.jam).map_err(|e| e.to_string())?;
    let table = library
      .jam()
      .tables
      .iter()
      .find(|t| t.name.eq_ignore_ascii_case(&options.table))
      .ok_or("missing race")?;
    let cars = lrformats::appearance::cars(
      library
        .find_in("CHAMPS.CCB", "COMMON")
        .ok_or("missing original cars")?,
    )?;
    let selected = cars
      .iter()
      .find(|c| c.name.eq_ignore_ascii_case(&options.car))
      .ok_or_else(|| format!("missing car {}", options.car))?;
    let drivers = lrformats::appearance::drivers(
      library
        .find_in("DRIVERS.DDB", "COMMON")
        .ok_or("missing original drivers")?,
    )?;
    let selected_driver = drivers
      .iter()
      .find(|d| d.name.eq_ignore_ascii_case(&options.driver))
      .ok_or_else(|| format!("missing driver {}", options.driver))?;
    let RaceData {
      contacts,
      start,
      chassis,
      diagnostic_line,
      lap_zones,
      checkpoints,
    } = RaceData::load(&library, &options.table, &selected.chassis)?;
    let mut chassis = chassis;
    chassis.mass = selected.mass;
    let handling = Handling::from_chassis(&chassis);
    let body_model = if let Some(build) = &options.custom_build {
      if !build.chassis.eq_ignore_ascii_case(&selected.chassis) {
        return Err("custom car and chassis selection disagree".into());
      }
      crate::brick_model::model(
        &library,
        &lrsim::brick_build::BuilderData::load(&library)?,
        build,
      )?
    } else {
      Model::load(&library, &selected.models[0], Some("COMMON")).map_err(|e| e.to_string())?
    };
    let body = TrackGpu::upload(&body_model)?;
    let wheel_name = chassis
      .wheel_models
      .first()
      .ok_or("missing original wheel model")?
      .1
      .clone();
    let wheels = crate::wheels::Wheels::load(&library, &wheel_name)?;
    let driver_model = if let Some(build) = &options.custom_driver {
      crate::custom_driver::Data::load(&library)?.model(&library, build)?
    } else {
      Model::load(&library, &selected_driver.models[0], Some("COMMON"))
        .map_err(|e| e.to_string())?
    };
    let driver = crate::driver_animation::DriverAnimation::load(&library, &driver_model)?;
    // Original RacerTable::LoadAppearance copies chassis+c4 into the
    // descriptor consumed as RacerVisual.desc_4c: driver attachment offset.
    let driver_seat = Mat4::from_translation(world_position(chassis.size, 1.0));
    let shared = lrformats::scene::SceneBindings::race_resources(&library, &options.table)?;
    let mut scenery = Vec::new();
    let mut billboards = Vec::new();
    let mut animated_scenery = Vec::new();
    let background = table
      .entries
      .iter()
      .find(|e| e.name.to_ascii_uppercase().ends_with(".RAB"))
      .map(|e| {
        library
          .jam()
          .bytes(e)
          .map_err(|e| e.to_string())
          .and_then(lrformats::race_archive::background_world)
      })
      .transpose()?
      .flatten();
    for entry in &table.entries {
      if !entry.name.to_ascii_uppercase().ends_with(".WDB") {
        continue;
      }
      if background
        .as_ref()
        .is_some_and(|name| name.eq_ignore_ascii_case(&entry.name))
      {
        continue;
      }
      let data = library.jam().bytes(entry).map_err(|e| e.to_string())?;
      let mut bindings = lrformats::scene::SceneBindings::load(&library, &options.table, data)?;
      bindings.inherit_named_resources(&shared);
      let sprites =
        crate::world_billboards::Billboards::load(&library, &options.table, data, &bindings)?;
      if sprites.len() > 0 {
        billboards.push(sprites);
      }
      animated_scenery.extend(crate::animated_scenery::Object::load(
        &library,
        &options.table,
        data,
        &bindings,
      )?);
      scenery.extend(crate::static_scenery::Object::load(
        &library,
        &options.table,
        data,
        &bindings,
      )?);
    }
    println!("native driving assets: original {} car/wheel skeleton, {} collision triangles, {} scenery instances; chassis={handling:?}",selected.name, contacts.ground.triangle_count(), scenery.len());
    let environment = crate::environment::Environment::load(&library, &options.table, 5000.0)?;
    Ok(Self {
      contacts,
      chassis,
      start,
      body,
      wheels,
      driver,
      driver_seat,
      scenery,
      billboards,
      animated_scenery,
      diagnostic_line,
      lap_zones,
      checkpoints,
      environment,
    })
  }
}
