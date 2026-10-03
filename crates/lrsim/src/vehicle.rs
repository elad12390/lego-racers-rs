//! Driving with x86-calibrated force laws and original geometry.
//! Support/probe and steering smoothing fidelity are still being evaluated.
use crate::contact::{cross, dot, length, normalized, Contacts};
use crate::handling::Handling;
use crate::handling::{landing_response, LandingCase};
use crate::support::{self, SupportCase};
use lrformats::{cmb::Chassis, world::StartPosition};

#[derive(Clone, Copy, Default)]
pub struct Actions {
  pub throttle: f32,
  pub steer: f32,
}

/// Controls already delivered to CarBody; bypass Driver grip/pedal selection.
/// Used for complete original CarBody::Step calibration, not player input.
#[derive(Clone,Copy)]
pub struct BodyControls {pub throttle:f32,pub reference_speed:f32,pub radius:f32}
#[derive(Clone,Copy)]
enum ControlInput {Player(Actions),Body(BodyControls)}

#[derive(Clone)]
pub struct Vehicle {
  pub position: [f32; 3],
  /// Original integrator units (world units per millisecond). Convert only at
  /// system boundaries, not back and forth on each force/integration tick.
  velocity_ms: [f32; 3],
  pub heading: f32,
  pub up: [f32; 3],
  pub grounded: bool,
  pub collisions: u32,
  /// Last complete body-step latch, not the cumulative response counter.
  pub contact_hits: u32,
  pub unresolved_contacts: u32,
  pub secondary_wheel_contacts: u32,
  pub out_of_world_recoveries: u32,
  pub handling: Handling,
  pub supported_wheels: u32,
  pub turn_rate: f32,
  wheel_base: [f32; 3],
  wheel_width: f32,
  wheel_length: f32,
  support_radius: f32,
  contact_normal: [f32; 3],
  ///00445dc0 caches this slope force for the next00445500 force update.
  slope_force: [f32; 3],
  drive_direction: [f32; 3],
  wheel_surfaces: [lrformats::collision_materials::Surface; 4],
  support_cache: support::ContactCache,
  support_supported: [bool; 4],
  mass: f32,
  airborne_seconds: f32,
  angular_motion: crate::angular_motion::AngularMotion,
  support_points: [[f32; 3]; 4],
  center_offset: [f32; 3],
  forward_axis: [f32; 3],
  left_axis: [f32; 3],
  body_probes: [[f32; 3]; 4],
  stable_pose: StablePose,
  turbo_drive: crate::turbo_drive::State,
}
#[derive(Clone, Copy)]
struct StablePose {
  position: [f32; 3],
  forward: [f32; 3],
  left: [f32; 3],
  up: [f32; 3],
}

impl Vehicle {
  pub fn spawn(start: &StartPosition, chassis: &Chassis, contacts: &Contacts) -> Self {
    let mut car = Self {
      position: start.position,
      velocity_ms: [0.0; 3],
      heading: start.forward[1].atan2(start.forward[0]),
      up: start.up,
      grounded: false,
      collisions: 0,
      contact_hits: 0,
      unresolved_contacts: 0,
      secondary_wheel_contacts: 0,
      out_of_world_recoveries: 0,
      handling: Handling::from_chassis(chassis),
      supported_wheels: 0,
      turn_rate: 0.0,
      wheel_base: chassis.points_a[1],
      wheel_width: chassis.points_a[1][1] - chassis.points_a[0][1],
      wheel_length: chassis.points_a[0][0] - chassis.points_a[2][0],
      support_radius: chassis.points_a.into_iter().map(length).fold(0.0, f32::max),
      contact_normal: start.up,
      slope_force: [0.0; 3],
      drive_direction: start.forward,
      wheel_surfaces: [lrformats::collision_materials::Surface::default(); 4],
      support_cache: Default::default(),
      support_supported: [false; 4],
      mass: chassis.mass,
      airborne_seconds: 0.0,
      angular_motion: crate::angular_motion::AngularMotion::new(chassis.mass),
      support_points: [[0.0; 3]; 4],
      center_offset: chassis.offset,
      forward_axis: normalized(start.forward),
      left_axis: cross(start.up, start.forward),
      //004371c0 builds chassis probes from RacerVisual cam_offset_y/z,
      // copied from chassis+dc/e0 (gear_range), at constant local Z3.5.
      // points_b are visual metadata, not this runtime collision box.
      body_probes: [
        [
          chassis.gear_range[1] * 0.5,
          -chassis.gear_range[0] * 0.5,
          3.5,
        ],
        [
          chassis.gear_range[1] * 0.5,
          chassis.gear_range[0] * 0.5,
          3.5,
        ],
        [
          -chassis.gear_range[1] * 0.5,
          -chassis.gear_range[0] * 0.5,
          3.5,
        ],
        [
          -chassis.gear_range[1] * 0.5,
          chassis.gear_range[0] * 0.5,
          3.5,
        ],
      ],
      stable_pose: StablePose {
        position: start.position,
        forward: normalized(start.forward),
        left: cross(start.up, start.forward),
        up: start.up,
      },
      turbo_drive: crate::turbo_drive::State::new(
        1.0 - (50.0 - f32::from(chassis.rating_c)) * 0.001,
        1.0 - (50.0 - f32::from(chassis.rating_b)) * 0.001),
    };
    if let Some(hit) = contacts
      .ground
      .at(car.position[0], car.position[1], car.position[2] + 1.0)
    {
      // Preserve original SPB origin, not a fabricated one-unit lift.
      car.up = hit.normal;
      car.left_axis = cross(car.up, car.forward_axis);
      car.contact_normal = hit.normal;
      car.grounded = true;
      car.supported_wheels = 4;
      car.support_supported = [true; 4];
    }
    car.support_points = support::suspension_points(&car.support_case(0.0, 0.0));
    car.remember_stable_pose();
    car
  }

  pub fn forward(&self) -> [f32; 3] {
    self.forward_axis
  }

  pub fn velocity(&self) -> [f32; 3] {
    self.velocity_ms.map(|v| (f64::from(v) * 1000.0) as f32)
  }
  pub fn set_velocity(&mut self, velocity: [f32; 3]) {
    self.velocity_ms = velocity.map(|v| (f64::from(v) / 1000.0) as f32);
  }
  pub fn sync_turbo(&mut self, activation: Option<u32>, tier: u8, rules: &crate::powerups::Rules) {
    if self.turbo_drive.update(activation, self.contact_hits, rules) {
      // Racer::StartRace calls ResetVelocityDirection once at activation.
      self.drive_direction = self.forward_axis;
      self.angular_motion.apply_pitch(
        crate::attitude::Basis {forward: self.forward_axis, left: self.left_axis, up: self.up},
        rules.turbo_pitch_rate_ms, rules.turbo_pitch_hold_ms[tier.min(2) as usize]);
    }
  }
  pub fn turbo_throttle(&self) -> Option<f32> { self.turbo_drive.throttle }
  pub fn clear_turbo(&mut self) { self.turbo_drive.clear(); }
  pub fn turbo_reference_speed(&self, limit: f32) -> f32 { self.turbo_drive.reference_speed(limit) }
  pub fn add_velocity(&mut self, change: [f32; 3]) {
    for (v, change) in self.velocity_ms.iter_mut().zip(change) {
      *v = (f64::from(*v) + f64::from(change) / 1000.0) as f32;
    }
  }

  /// Scripted effect transit, not ordinary movement or diagnostic teleport.
  pub fn effect_position(&mut self, position: [f32; 3]) {
    self.position = position;
    self.support_cache = Default::default();
    self.support_points = support::suspension_points(&self.support_case(0.0, 0.0));
  }

  pub fn checkpoint_probes(&self) -> [[f32; 3]; 4] {
    let forward = self.forward();
    let left = self.left_axis;
    self.body_probes.map(|p| {
      std::array::from_fn(|i| {
        self.position[i] + forward[i] * p[0] + left[i] * p[1] + self.up[i] * p[2]
      })
    })
  }

  pub fn basis(&self) -> [f32; 9] {
    let left = self.left_axis;
    [self.forward(), left, self.up].concat().try_into().unwrap()
  }

  /// Original TryDisplace rejects translation if chassis probes cross a wall.
  pub fn try_displace(&mut self, displacement: [f32; 3], contacts: &Contacts) {
    let old = self.chassis_probes();
    let saved = self.position;
    self.position = std::array::from_fn(|i| self.position[i] + displacement[i]);
    if crate::collision_step::sweep(contacts, old, self.chassis_probes()).is_some() {
      self.position = saved;
    }
  }

  pub fn try_displace_dispatched(
    &mut self,
    displacement: [f32; 3],
    contacts: &Contacts,
    world: &crate::world_dispatch::ChassisWorld<'_>,
    state: &mut crate::checkpoint_contacts::State,
  ) -> u32 {
    let old = self.chassis_probes();
    let saved = self.position;
    self.position = std::array::from_fn(|i| self.position[i] + displacement[i]);
    let (hit, count) = world.sweep(contacts, old, self.chassis_probes(), state);
    if hit.is_some() {
      self.position = saved;
    }
    count
  }

  pub fn speed(&self) -> f32 {
    dot(self.velocity(), self.forward())
  }

  pub fn drive_speed(&self) -> f32 {
    dot(self.velocity(), self.drive_direction)
  }

  pub fn step(&mut self, actions: Actions, contacts: &Contacts, dt: f32) {
    self.step_observed(actions, contacts, dt, &mut |_| {});
  }

  /// Passive pre-query observation; the callback receives no mutable gameplay
  /// state and cannot alter integration/contact selection.
  pub fn step_observed(
    &mut self,
    actions: Actions,
    contacts: &Contacts,
    dt: f32,
    observer: &mut impl FnMut(&SupportCase),
  ) {
    self.step_queries(
      ControlInput::Player(actions),
      contacts,
      dt,
      None,
      observer,
      &mut |saved, trial| {
        crate::collision_step::sweep(contacts, saved.chassis_probes(), trial.chassis_probes())
      },
    );
  }

  /// Retain the body-only verification boundary: original probes explicitly
  /// call SetThrottle/SetRadius and Step, never Driver::UpdateSteering.
  pub fn step_body_observed(&mut self,controls:BodyControls,contacts:&Contacts,dt:f32,observer:&mut impl FnMut(&SupportCase)){
    self.step_queries(ControlInput::Body(controls),contacts,dt,None,observer,&mut |saved,trial|{
      crate::collision_step::sweep(contacts,saved.chassis_probes(),trial.chassis_probes())
    });
  }

  /// Query owner state is deliberately OUTSIDE cloned/restored physics state.
  pub fn step_dispatched(
    &mut self,
    actions: Actions,
    contacts: &Contacts,
    dt: f32,
    world: &crate::world_dispatch::ChassisWorld<'_>,
    state: &mut crate::checkpoint_contacts::State,
  ) -> u32 {
    let mut count = 0;
    self.step_queries(
      ControlInput::Player(actions),
      contacts,
      dt,
      Some(world),
      &mut |_| {},
      &mut |saved, trial| {
        let (hit, contacts_count) = world.sweep(
          contacts,
          saved.chassis_probes(),
          trial.chassis_probes(),
          state,
        );
        count += contacts_count;
        hit
      },
    );
    count
  }

  fn step_queries(
    &mut self,
    actions: ControlInput,
    contacts: &Contacts,
    dt: f32,
    world: Option<&crate::world_dispatch::ChassisWorld<'_>>,
    observer: &mut impl FnMut(&SupportCase),
    query: &mut impl FnMut(&Self, &Self) -> Option<crate::collision_step::Hit>,
  ) {
    if dt <= 0.0 || !dt.is_finite() {
      return;
    }
    // Max 10ms keeps contacts and converted force model stable across host FPS.
    let mut remaining = dt.min(0.25);
    let collisions = self.collisions;
    self.contact_hits = 0;
    while remaining > 0.0 {
      let interval = remaining.min(0.01);
      self.substep(actions, contacts, interval, world, observer, query);
      remaining -= interval;
    }
    // CarBody::Update records a four-wheel, no-chassis-response pose and
    // restores it outside the original vertical bounds. Chassis response
    // counting is the native latch here; rejected trial-hit latch fidelity
    // remains separate from the basic recovery behavior.
    if self.supported_wheels == 4 && self.collisions == collisions {
      self.remember_stable_pose();
    }
    self.recover_out_of_world();
  }

  fn remember_stable_pose(&mut self) {
    self.stable_pose = StablePose {
      position: self.position,
      forward: self.forward_axis,
      left: self.left_axis,
      up: self.up,
    };
  }
  fn recover_out_of_world(&mut self) {
    if self.position[2] >= -250.0 && self.position[2] <= 340.0 {
      return;
    }
    self.position = self.stable_pose.position;
    self.forward_axis = self.stable_pose.forward;
    self.left_axis = self.stable_pose.left;
    self.up = self.stable_pose.up;
    self.heading = self.forward_axis[1].atan2(self.forward_axis[0]);
    self.velocity_ms = [0.0; 3];
    self.turn_rate = 0.0;
    self.angular_motion = crate::angular_motion::AngularMotion::new(self.mass);
    self.support_cache = Default::default();
    self.grounded = false;
    self.supported_wheels = 0;
    self.support_supported = [false; 4];
    self.airborne_seconds = 0.0;
    self.support_points = support::suspension_points(&self.support_case(0.0, 0.0));
    self.out_of_world_recoveries += 1;
  }

  fn substep(
    &mut self,
    actions: ControlInput,
    contacts: &Contacts,
    dt: f32,
    world: Option<&crate::world_dispatch::ChassisWorld<'_>>,
    observer: &mut impl FnMut(&SupportCase),
    query: &mut impl FnMut(&Self, &Self) -> Option<crate::collision_step::Hit>,
  ) {
    let complete = crate::collision_step::advance(
      self,
      dt,
      |car, interval| car.free_substep(actions, contacts, interval, world, observer),
      query,
      |car, normal| {
        let old = car.velocity_ms;
        let response = crate::wall_contact::solve(&crate::wall_contact::Case {
          velocity: old,
          normal,
          forward: car.forward_axis,
          left: car.left_axis,
          spin_hold: false,
        });
        car.velocity_ms = response.velocity;
        if let Some(yaw) = response.yaw_rate {
          car.angular_motion.set_yaw(
            crate::attitude::Basis {
              forward: car.forward_axis,
              left: car.left_axis,
              up: car.up,
            },
            yaw,
          );
        }
        car.collisions += 1;
        //00444ef0 restores this latch after any accepted response, even when
        // the final query has no new hit. It survives subsequent subintervals.
        car.contact_hits = 1;
        old != car.velocity_ms
      },
    );
    if !complete {
      self.unresolved_contacts += 1;
    }
  }

  fn chassis_probes(&self) -> [[f32; 3]; 4] {
    let forward = self.forward();
    let left = self.left_axis;
    self.body_probes.map(|p| {
      std::array::from_fn(|i| {
        self.position[i] + forward[i] * p[0] + left[i] * p[1] + self.up[i] * p[2]
      })
    })
  }

  fn free_substep(
    &mut self,
    input: ControlInput,
    contacts: &Contacts,
    dt: f32,
    world: Option<&crate::world_dispatch::ChassisWorld<'_>>,
    observer: &mut impl FnMut(&SupportCase),
  ) {
    let longitudinal = self.drive_speed();
    let BodyControls{throttle,reference_speed,radius}=match input{
      ControlInput::Body(controls)=>controls,
      ControlInput::Player(actions)=>BodyControls{
        throttle:self.turbo_drive.throttle.unwrap_or_else(||self.handling.player_throttle(actions.throttle,longitudinal)),
        reference_speed:self.handling.forward_limit,
        radius:crate::driver_grip::solve(&crate::driver_grip::Case {
          requested_radius:self.handling.requested_radius(actions.steer),
          speed_ms:length(self.velocity_ms),forward_speed_ms:dot(self.velocity_ms,self.drive_direction),mass:self.mass,
          grounded:self.support_supported,grip:self.wheel_surfaces.map(|s|s.grounded_grip),
          grounded_scale:if self.turbo_drive.throttle.is_some(){1.5}else{1.0},
        }).radius,
      },
    };
    let result = crate::ordinary_forces::solve(&crate::ordinary_forces::Case {
      velocity: self.velocity_ms,
      direction: self.drive_direction,
      forward: self.forward_axis,
      mass: self.mass,
      throttle,
      reference_speed,
      radius,
      wheels: if self.grounded {
        self.supported_wheels
      } else {
        0
      },
      slope_force: self.slope_force,
      friction: self
        .wheel_surfaces
        .iter()
        .map(|s| s.slope_friction)
        .sum::<f32>()
        * 0.25,
      surface_drag: self
        .wheel_surfaces
        .iter()
        .map(|s| s.quadratic_drag)
        .sum::<f32>()
        * 0.25,
      surface_force: std::array::from_fn(|i| self.wheel_surfaces.iter().map(|s| s.force[i]).sum()),
    });
    self.turn_rate = result.turn_rate * 1000.0;
    let old = self.position;
    let elapsed_ms = dt * 1000.0;
    let inverse_mass = 1.0 / self.mass;
    let transform = crate::collider_transform::ColliderTransform {
      origin: self.position,
      axes: [self.forward_axis, self.left_axis, self.up],
    };
    let scale = (f64::from(self.mass) * f64::from(0.001f32) * f64::from(0.001f32)) as f32;
    let basis = self.angular_motion.advance(
      crate::attitude::Basis {
        forward: self.forward_axis,
        left: self.left_axis,
        up: self.up,
      },
      transform.point(self.center_offset),
      self.support_points,
      if self.grounded {
        self.support_supported
      } else {
        [false; 4]
      },
      if self.grounded {
        self.supported_wheels
      } else {
        0
      },
      self.contact_normal,
      -39.0 * scale,
      (radius != 0.0).then_some(result.yaw_rate),
      elapsed_ms,
    );
    self.forward_axis = basis.forward;
    self.left_axis = basis.left;
    self.up = basis.up;
    for i in 0..3 {
      let increment =
        (f64::from(result.force[i]) * f64::from(elapsed_ms) * f64::from(inverse_mass)) as f32;
      self.position[i] = (f64::from(self.velocity_ms[i]) * f64::from(elapsed_ms)
        + f64::from(increment) * f64::from(elapsed_ms * 0.5)
        + f64::from(self.position[i])) as f32;
      self.velocity_ms[i] += increment;
    }
    //00445c30 applies yaw about the physical third column, including slopes
    // and banks. A world-Z rotation loses steering rate after alignment.
    //00445500 uses the PREVIOUS contacts to prepare angular momentum/torque;
    //00440e10 advances orientation before00445dc0 queries the NEW contacts.
    let clamped = crate::attitude::clamp_tilt(crate::attitude::Basis {
      forward: self.forward(),
      left: self.left_axis,
      up: self.up,
    });
    self.forward_axis = clamped.forward;
    self.left_axis = clamped.left;
    self.up = clamped.up;
    self.heading = self.forward_axis[1].atan2(self.forward_axis[0]);
    let support_case = SupportCase {
      position: self.position,
      forward: self.forward(),
      left: self.left_axis,
      up: self.up,
      base: self.wheel_base,
      width: self.wheel_width,
      length: self.wheel_length,
      radius: self.support_radius,
      downward_movement: (old[2] - self.position[2]).max(0.0),
      grounded: self.grounded,
      dt,
    };
    observer(&support_case);
    let support = support::solve_world_locked(
      &support_case,
      &contacts.ground,
      &mut self.support_cache,
      world.map_or(&[], |world| world.secondary()),
      self.angular_motion.alignment_locked(),
    );
    self.secondary_wheel_contacts += support
      .colliders
      .iter()
      .filter(|c| c.is_some_and(|c| c > 0))
      .count() as u32;
    //00445dc0: losing existing support without any wheel ray hit applies
    // an immediate -0.008 units/ms drop after position integration.
    if self.grounded && support.wheels == 0 {
      self.velocity_ms[2] -= 0.008;
    }
    let velocity = self.velocity();
    let landing = landing_response(LandingCase {
      velocity,
      normal: support.contact_normal,
      airborne_ms: (self.airborne_seconds * 1000.0).round() as u32,
      was_airborne: !self.grounded,
      wheels: support.wheels,
    });
    if landing.velocity != velocity {
      self.set_velocity(landing.velocity);
    }
    self.grounded = landing.wheels > 0;
    self.supported_wheels = landing.wheels;
    self.support_supported = if self.grounded {
      support.supported
    } else {
      [false; 4]
    };
    self.wheel_surfaces = std::array::from_fn(|i| {
      if support.supported[i] {
        support.surfaces[i]
          .map(|index| {
            world.map_or_else(
              || contacts.surface(index),
              |world| world.wheel_surface(contacts, support.colliders[i].unwrap(), index),
            )
          })
          .unwrap_or_default()
      } else {
        Default::default()
      }
    });
    // Alignment happens before the original hard-landing branch clears support.
    if support.wheels > 0 {
      self.position = support.position;
      self.up = support.up;
      self.forward_axis = support.forward;
      self.left_axis = support.left;
      self.heading = support.forward[1].atan2(support.forward[0]);
    }
    //0044864e rebuilds wheel positions with the ALIGNED basis, not a
    // translation of pre-alignment probes; next tick's torque uses these.
    self.support_points = support::suspension_points(&self.support_case(dt, 0.0));
    if self.grounded {
      self.contact_normal = support.contact_normal;
      let scale = (f64::from(self.mass) * f64::from(0.001f32) * f64::from(0.001f32)) as f32;
      let gravity = -39.0 * scale;
      let component = gravity * support.contact_normal[2];
      self.slope_force = std::array::from_fn(|i| {
        if i == 2 {
          gravity - support.contact_normal[i] * component
        } else {
          -support.contact_normal[i] * component
        }
      });
      // Contact translation and normal-force damping are separate in the
      // original. Do not divide by normal.z to snap velocity: an edge
      // alignment can be vertical and this fabricates unbounded energy.
    }
    if self.grounded {
      self.airborne_seconds = 0.0;
    } else {
      self.airborne_seconds += dt;
    }
    //00446fd0 ordinary target selection; the large-angle TurnToward lookup
    // branch is still a separate calibration item, not recovered by this.
    self.drive_direction = if self.grounded && self.supported_wheels < 3 && longitudinal >= 30.0 {
      crate::attitude::perpendicular(self.contact_normal, self.forward_axis)
    } else {
      self.forward_axis
    };
  }

  fn support_case(&self, dt: f32, downward_movement: f32) -> SupportCase {
    SupportCase {
      position: self.position,
      forward: self.forward_axis,
      left: self.left_axis,
      up: self.up,
      base: self.wheel_base,
      width: self.wheel_width,
      length: self.wheel_length,
      radius: self.support_radius,
      downward_movement,
      grounded: self.grounded,
      dt,
    }
  }
}
