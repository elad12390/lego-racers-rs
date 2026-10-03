//! Distinct original rival bodies, wheels and driver skins; presentation still
//! retains route-derived steering gestures and dispatched power reactions.
use crate::platform::prelude::*;
use crate::{
  driver_animation::DriverAnimation,
  gpu::{world_position, TrackGpu},
  wheels::Wheels,
};
use lrformats::{library::Library, model::Model};
use lrsim::{rival_data::RivalData, rivals::Rival};

pub struct RivalGpu {
  body: TrackGpu,
  wheels: Wheels,
  driver: DriverAnimation,
  seat: Mat4,
}
impl RivalGpu {
  pub fn load(library: &Library, data: &RivalData) -> Result<Self, String> {
    let body = TrackGpu::upload(
      &Model::load(library, &data.car_model, Some("COMMON")).map_err(|e| e.to_string())?,
    )?;
    let wheel = data
      .chassis
      .wheel_models
      .first()
      .ok_or("missing rival wheel model")?;
    let wheels = Wheels::load(library, &wheel.1)?;
    let driver = DriverAnimation::load(
      library,
      &Model::load(library, &data.driver_model, Some("COMMON")).map_err(|e| e.to_string())?,
    )?;
    Ok(Self {
      body,
      wheels,
      driver,
      seat: Mat4::from_translation(world_position(data.chassis.size, 1.0)),
    })
  }
  pub fn reset(&mut self) -> Result<(), String> {
    self.wheels.reset()?;
    self.driver.reset()
  }
  pub fn advance(
    &mut self,
    rival: &Rival<'_>,
    finish_rank: Option<u32>,
    dt: f32,
  ) -> Result<(), String> {
    let forward = rival.motion.basis[..3].try_into().unwrap();
    let speed = lrsim::contact::dot(rival.motion.velocity, forward);
    self.wheels.advance(speed, dt)?;
    self
      .driver
      .advance_race(speed, 1.0, rival.motion.steering(), finish_rank, dt)
  }
  pub fn driver_clip(&self) -> &str {
    self.driver.clip_name()
  }
  pub fn power_reaction(&mut self, turbo: bool) {
    self.driver.power_reaction(turbo);
  }
  pub fn draw(&self, rival: &Rival<'_>) {
    self.draw_view(rival, crate::race_view::RaceView { mirrored: false });
  }
  pub fn draw_view(&self, rival: &Rival<'_>, view: crate::race_view::RaceView) {
    let b = rival.motion.basis;
    // Conjugate original +Z-up basis into the existing native asset axes.
    let transform = view.car(
      rival.motion.position,
      b[..3].try_into().unwrap(),
      b[6..9].try_into().unwrap(),
    );
    self.body.draw_at(transform);
    self.wheels.draw_at(transform);
    self.driver.draw_at(transform * self.seat);
  }
}
