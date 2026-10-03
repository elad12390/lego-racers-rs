//! Calibrated ordinary on-route movement and independent spatial lap clocks.
//! No collision response/recovery/powerups/difficulty acceptance is implied.
use crate::{
  lap_zones::LapZones, race::Race, rival_data::RivalData, route_motion::ContactFreeRoute,
};
use serde::Serialize;

pub struct Rival<'a> {
  pub data: &'a RivalData,
  pub motion: ContactFreeRoute<'a>,
  pub race: Race,
  pub hit: crate::racer_hit::HitState,
  fractional_ms: f64,
  pub world_collisions: u32,
}

#[derive(Serialize)]
pub struct RivalReport {
  pub name: String,
  pub route: String,
  pub car_model: String,
  pub driver_model: String,
  pub position: [f32; 3],
  pub completed_laps: u32,
  pub finished: bool,
  pub elapsed: f64,
  pub lap_times: Vec<f64>,
  pub events: Vec<crate::race::RaceEvent>,
  pub hit: crate::racer_hit::HitState,
  pub world_collisions: u32,
}

impl<'a> Rival<'a> {
  pub fn new(data: &'a RivalData) -> Result<Self, String> {
    Ok(Self {
      data,
      motion: ContactFreeRoute::at_start(&data.record),
      race: Race::new(3)?,
      fractional_ms: 0.0,
      hit: Default::default(),
      world_collisions: 0,
    })
  }
  pub fn advance(&mut self, dt: f64, zones: &LapZones) {
    // OriginalRacerTable0043c030 calls every CarBody after the finish
    // flag is set. Freeze the finish time, not a rival's body/collider.
    if !dt.is_finite() || dt <= 0.0 {
      return;
    }
    self.fractional_ms += dt * 1000.0;
    let ticks = self.fractional_ms.floor() as u32;
    self.fractional_ms -= f64::from(ticks);
    let old = self.motion.position;
    self.motion.advance(ticks);
    self.hit.advance(ticks);
    self
      .race
      .advance(old, self.motion.position, zones, f64::from(ticks) / 1000.0);
  }
  pub fn report(&self) -> RivalReport {
    RivalReport {
      name: self.data.name.clone(),
      route: self.data.route_name.clone(),
      car_model: self.data.car_model.clone(),
      driver_model: self.data.driver_model.clone(),
      position: self.motion.position,
      completed_laps: self.race.laps.completed_laps,
      finished: self.race.finished,
      elapsed: self.race.elapsed,
      lap_times: self.race.lap_times.clone(),
      events: self.race.events.clone(),
      hit: self.hit,
      world_collisions: self.world_collisions,
    }
  }

  pub fn advance_dispatched(
    &mut self,
    dt: f64,
    zones: &LapZones,
    contacts: &crate::contact::Contacts,
    world: &crate::world_dispatch::ChassisWorld<'_>,
    state: &mut crate::checkpoint_contacts::State,
  ) -> u32 {
    if !dt.is_finite() || dt <= 0.0 {
      return 0;
    }
    let old = self.motion.position;
    self.fractional_ms += dt * 1000.0;
    let ticks = self.fractional_ms.floor() as u32;
    self.fractional_ms -= f64::from(ticks);
    let mut count = 0;
    let blocked =
      self
        .motion
        .advance_queried(ticks, self.data.chassis.gear_range, |starts, ends| {
          let (selection, hits) = world.material_selection(contacts, starts, ends, false, state);
          count += hits;
          selection
        });
    self.world_collisions += u32::from(blocked);
    self.hit.advance(ticks);
    self
      .race
      .advance(old, self.motion.position, zones, f64::from(ticks) / 1000.0);
    count
  }
}
