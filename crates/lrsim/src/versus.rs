//! Two free vehicles sharing original world queries, gate, pickups and standings.
//! Native physical input and original versus presentation still need acceptance.
use crate::{
  powerups::{Powerups, Racer},
  race::Race,
  race_data::RaceData,
  race_positions::{Pose, RacePositions},
  start_gate::StartGate,
  vehicle::{Actions, Vehicle},
  world_dispatch::ChassisWorld,
};
pub struct Session<'a> {
  pub cars: [Vehicle; 2],
  pub races: [Race; 2],
  pub positions: RacePositions,
  pub powers: Powerups,
  pub gate: StartGate,
  pub contacts: u32,
  pub elapsed: f64,
  data: &'a [RaceData; 2],
}
impl<'a> Session<'a> {
  pub fn new(data: &'a [RaceData; 2], powers: Powerups) -> Result<Self, String> {
    let cars =
      std::array::from_fn(|i| Vehicle::spawn(&data[i].start, &data[i].chassis, &data[i].contacts));
    let positions = RacePositions::new(&poses(&cars));
    Ok(Self {
      cars,
      races: [Race::new(3)?, Race::new(3)?],
      positions,
      powers,
      gate: Default::default(),
      contacts: 0,
      elapsed: 0.0,
      data,
    })
  }
  pub fn actors(&self) -> [Racer; 2] {
    std::array::from_fn(|i| Racer {
      position: self.cars[i].position,
      forward: self.cars[i].forward(),
      up: self.cars[i].up,
      finished: self.races[i].finished,
    })
  }
  pub fn step(
    &mut self,
    dt: f32,
    actions: [Actions; 2],
    use_power: [bool; 2],
  ) -> Result<f32, String> {
    let dt = self.gate.advance(f64::from(dt)) as f32;
    if dt <= 0.0 {
      return Ok(0.0);
    }
    self.elapsed += f64::from(dt);
    let actors = self.actors();
    let primary = &self.data[0];
    self.powers.advance(dt, &actors, |old, new| {
      primary.contacts.sweep(old, new).is_some()
    });
    self.powers.collect(&actors);
    for i in 0..2 {
      if use_power[i] {
        self.powers.use_power(i, &actors);
      }
    }
    let old = self.cars.each_ref().map(|c| c.position);
    let world = ChassisWorld::new(
      primary
        .contacts
        .primary_collider()
        .ok_or("missing versus primary collider")?
        .clone(),
      &primary.checkpoints,
    );
    for i in 0..2 {
      let data = &self.data[i];
      let hits = crate::power_vehicle::advance(
        &mut self.cars[i],
        actions[i],
        Some((&self.powers.inventories[i], &self.powers.rules)),
        &actors,
        &data.contacts,
        &data.checkpoints,
        &world,
        self.positions.state_mut(i),
        dt,
      )?;
      self.positions.add_contacts(hits);
    }
    let [a, b] = &mut self.cars;
    let (pairs, hits) = crate::race_contacts::resolve_players(
      a,
      &self.data[0].chassis,
      b,
      &self.data[1].chassis,
      &primary.contacts,
      &world,
      self.positions.first_two_states_mut(),
    );
    self.contacts += pairs;
    self.positions.add_contacts(hits);
    for i in 0..2 {
      self.races[i].advance(
        old[i],
        self.cars[i].position,
        &self.data[i].lap_zones,
        f64::from(dt),
      );
    }
    self
      .positions
      .rank_dispatched(&poses(&self.cars), &primary.checkpoints);
    self
      .positions
      .finish(&self.races.each_ref().map(|r| r.finished));
    Ok(dt)
  }
  pub fn reset(&mut self) -> Result<(), String> {
    self.powers.reset();
    self.cars = std::array::from_fn(|i| {
      Vehicle::spawn(
        &self.data[i].start,
        &self.data[i].chassis,
        &self.data[i].contacts,
      )
    });
    self.races = [Race::new(3)?, Race::new(3)?];
    self.positions = RacePositions::new(&poses(&self.cars));
    self.gate = Default::default();
    self.contacts = 0;
    self.elapsed = 0.0;
    Ok(())
  }
}
pub fn poses(cars: &[Vehicle; 2]) -> [Pose; 2] {
  std::array::from_fn(|i| Pose {
    position: cars[i].position,
    probes: cars[i].checkpoint_probes(),
  })
}
