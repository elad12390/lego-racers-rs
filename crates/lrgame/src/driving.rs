//! Native diagnostic driving; no race completion acceptance implied.
use crate::platform::prelude::*;
use crate::{capture, drive_assets::DriveAssets, gpu::TrackGpu, options::Options};
use lrsim::{
  contact::length,
  vehicle::{Actions, Vehicle},
};
use serde::Serialize;

#[derive(Serialize)]
struct DriveReport {
  mode: &'static str,
  native_frames_presented: u32,
  distance_travelled: f32,
  supported_frames: u32,
  frame_hash_first: u64,
  frame_hash_last: u64,
  non_background_pixels_first: usize,
  non_background_pixels_last: usize,
  position: [f32; 3],
  speed: f32,
  collisions: u32,
  unresolved_contacts: u32,
  secondary_wheel_contacts: u32,
  rejected_empty_manifolds: u64,
  original_line_samples_reached: usize,
  original_line_samples_total: usize,
  completed_laps: u32,
  race_finished: bool,
  race_elapsed: f64,
  lap_times: Vec<f64>,
  race_events: Vec<lrsim::race::RaceEvent>,
  countdown_frames: u32,
  countdown_max_player_displacement: f32,
  countdown_max_rival_displacement: f32,
  countdown_racer_contacts: u32,
  camera_eye: [f32; 3],
  camera_forward: [f32; 3],
  rivals: Vec<lrsim::rivals::RivalReport>,
  racer_contacts: u32,
  positions: lrsim::race_positions::Report,
  music_cues: Vec<crate::race_music::Cue>,
  player_driver_clip: String,
  rival_driver_clips: Vec<String>,
  post_finish_frames: u32,
  post_finish_rival_distance: f32,
  player_finish_time: Option<f64>,
  powerup_inventories: Vec<lrsim::powerups::Inventory>,
  powerup_impacts: u32,
  powerup_hits_by_kind: [u32; 8],
  limitations: Vec<&'static str>,
}

#[derive(Serialize)]
pub struct RaceResult {
  pub names: Vec<String>,
  pub ranks: Vec<u32>,
  pub time: f64,
  pub laps: u32,
  pub trial_ghost: Option<crate::profile::TrialGhost>,
  pub original_ghost_time: Option<f64>,
}

pub async fn run(options: Options, track: TrackGpu) -> Result<(), String> {
  run_race(options, track).await.map(|_| ())
}

pub async fn run_race(options: Options, mut track: TrackGpu) -> Result<Option<RaceResult>, String> {
  let race_view = crate::race_view::RaceView {
    mirrored: options.mirrored,
  };
  let mut assets = DriveAssets::load(&options)?;
  let library = lrformats::library::Library::open(&options.jam).map_err(|e| e.to_string())?;
  let mut track_materials =
    crate::world_material::Player::main_track(&library, &options.table, &mut track)?;
  let mut world_time = 0.0;
  // Repeated diagnostic runs load the real backend but stay silent. Interactive
  // driving uses original clips; no audio output is claimed from dispatch logs.
  let audible = options.audible_diagnostic || (!options.scripted_drive && !options.scripted_route);
  let mut audio = crate::race_audio::RaceAudio::load_settings(
    &library,
    &options.table,
    options
      .jam
      .parent()
      .ok_or("missing original tune directory")?,
    audible && options.sound,
    audible && options.music,
  )
  .await?;
  let prelude = options.play || options.intro_smoke;
  let mut intro = if prelude {
    crate::race_intro::Intro::load(&library, &options.table)?
  } else {
    None
  };
  let mut intro_delay = if prelude && intro.is_none() {
    2.0f32
  } else {
    0.0
  };
  audio.start_sequence(!prelude);
  let mut car = Vehicle::spawn(&assets.start, &assets.chassis, &assets.contacts);
  let mut driver = lrsim::diagnostic_driver::DiagnosticDriver::new(assets.diagnostic_line.clone());
  let mut race = lrsim::race::Race::new(3)?;
  let mut start_gate = lrsim::start_gate::StartGate::default();
  // Ordinary original constrained-route rivals are visible, but they are
  // NOT full contested opponents until contact/recovery/powerup branches work.
  // Random values replace only the noise source, not the original uniform
  // selection among contiguous available F/M/Sfiles.
  let rival_data = if options.time_trial {
    Vec::new()
  } else {
    lrsim::rival_data::load_selected(
      &library,
      &options.table,
      options.race_name.as_deref(),
      std::array::from_fn(|_| rand::gen_range(0, u32::MAX)),
    )?
  };
  let mut rivals = rival_data
    .iter()
    .map(lrsim::rivals::Rival::new)
    .collect::<Result<Vec<_>, _>>()?;
  let mut positions =
    lrsim::race_positions::RacePositions::new(&lrsim::race_positions::poses(&car, &rivals));
  let world = lrsim::world_dispatch::ChassisWorld::new(
    assets
      .contacts
      .primary_collider()
      .ok_or("missing primary query")?
      .clone(),
    &assets.checkpoints,
  );
  let mut rival_gpu = rival_data
    .iter()
    .map(|d| crate::rival_gpu::RivalGpu::load(&library, d))
    .collect::<Result<Vec<_>, _>>()?;
  let mut powers = if options.play || options.turbo_smoke {
    Some(lrsim::powerups::Powerups::load(
      &library,
      &options.table,
      rivals.len() + 1,
    )?)
  } else {
    None
  };
  let mut power_gpu = powers
    .as_ref()
    .map(|p| crate::powerup_gpu::PowerupGpu::load(&library, &p.rules))
    .transpose()?;
  let hud = if options.play {
    Some(crate::race_hud::RaceHud::load(
      &library,
      &options.table,
      options.mirrored,
    )?)
  } else {
    None
  };
  let mut camera = crate::camera_rig::ChaseRig::new(car.position, car.forward());
  let mut suspension_kick = crate::camera_view::SuspensionKick::default();
  camera.set_mode(options.camera_mode);
  camera.update_view(car.position, car.forward(), car.up, 0.0, 1.0 / 60.0);
  let mut camera_check = crate::camera_check::Check::from_options(&options);
  let mut trial = if options.time_trial {
    Some(crate::time_trial::TimeTrial::load(
      &library, &options, &car,
    )?)
  } else {
    None
  };
  let mut finish_frame = None;
  let mut paused = false;
  let mut input_gate = crate::input_gate::InputGate::default();
  let mut restart_requested = false;
  let mut video = None;
  let mut pause_ui = crate::menu_ui::MenuUi::load(&library)?;
  let mut report = DriveReport {
    mode: if options.play {
      "native_playable_flow_fidelity_incomplete"
    } else {
      "diagnostic_driving_not_complete_race"
    },
    native_frames_presented: 0,
    distance_travelled: 0.0,
    supported_frames: 0,
    frame_hash_first: 0,
    frame_hash_last: 0,
    non_background_pixels_first: 0,
    non_background_pixels_last: 0,
    position: car.position,
    speed: 0.0,
    collisions: 0,
    unresolved_contacts: 0,
    secondary_wheel_contacts: 0,
    rejected_empty_manifolds: 0,
    original_line_samples_reached: 0,
    original_line_samples_total: driver.points.len(),
    completed_laps: 0,
    race_finished: false,
    race_elapsed: 0.0,
    lap_times: Vec::new(),
    race_events: Vec::new(),
    countdown_frames: 0,
    countdown_max_player_displacement: 0.0,
    countdown_max_rival_displacement: 0.0,
    countdown_racer_contacts: 0,
    camera_eye: camera.pose().0,
    camera_forward: camera.pose().1,
    rivals: Vec::new(),
    racer_contacts: 0,
    positions: positions.report(),
    music_cues: Vec::new(),
    player_driver_clip: assets.driver.clip_name().into(),
    rival_driver_clips: Vec::new(),
    post_finish_frames: 0,
    post_finish_rival_distance: 0.0,
    player_finish_time: None,
    powerup_inventories: Vec::new(),
    powerup_impacts: 0,
    powerup_hits_by_kind: [0; 8],
    limitations: vec![
      "80 isolated force/radius scenarios compared to unchanged x86, not whole-game feel",
      "43200complete coupled ordinary vehicle frames at5/8/10ms and2880changing-control frames match unchanged x86; full world/effect/input/feel acceptance remains open",
      "empty box manifolds are counted and rejected: explicit safety deviation from original division-by-zero, not a fabricated contact point",
      "original wheel ADB/SDB animated with source-derived1.8*forward-speed rate and separate loop periods; complete visual-update integration still open",
      "2880complete normal-camera updates through actual GolDP receiver and288projection corners match; intro/reverse/impact/finish camera modes remain open",
      "original physical tilt clamp matches54x86cases; persistent eight-contact support cache integrated, all-track world-query ordering remains open",
      "physical keyboard delivery not verified by scripted actions",
      "150 player throttle/brake/steering cases and367 lap histories match unchanged x86",
      "original3second release gate integrated; countdown numerals/results UI diagnostic, cinematic intro and original HUD pending",
      "original roster/on-route rivals, ordinary box contact and independent spatial laps integrated; recovery, difficulty/power/effect locks and contested acceptance remain open",
      "driver finish/reaction/reverse selection matches864original bounded/no-change cases; finish clip chains rendered, full playback/idle/reaction dispatch and voices remain open",
      "original LEGOMSCintro/race/win/lose cues loaded on native backend; race selection matches80original prefixes, fixed gain provisional and audible output unverified",
      "78complete original CPBnumeric ranking snapshots,180checkpoint-side and144finish calls match; finite four-probe dispatch and full contact order remain diagnostic",
      "player movement remains frozen after finish pending original player-finish control/camera; rivals keep moving and colliding, clocks/ranks freeze",
    ],
  };
  if let Some(directory) = &options.capture {
    std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
  }
  loop {
    if input_gate.poll(options.scripted_drive || options.scripted_route) {
      paused = true;
      input_gate.suspend();
    }
    if is_quit_requested() || (!options.play && is_key_pressed(KeyCode::Escape)) {
      break;
    }
    if options.play && input_gate.pressed(KeyCode::Escape) {
      paused = !paused;
      input_gate.suspend();
    }
    if options.play
      && race.finished
      && rivals.iter().all(|r| r.race.finished)
      && input_gate.pressed(KeyCode::Enter)
    {
      break;
    }
    let frame = report.native_frames_presented;
    if restart_requested
      || (input_gate.pressed(KeyCode::R) && !options.scripted_drive && !options.scripted_route)
    {
      restart_requested = false;
      input_gate.suspend();
      paused = false;
      car = Vehicle::spawn(&assets.start, &assets.chassis, &assets.contacts);
      race = lrsim::race::Race::new(3)?;
      start_gate = lrsim::start_gate::StartGate::default();
      if let Some(intro) = &mut intro {
        intro.reset();
      }
      intro_delay = if prelude && intro.is_none() { 2.0 } else { 0.0 };
      let mode = camera.mode();
      camera = crate::camera_rig::ChaseRig::new(car.position, car.forward());
      suspension_kick = crate::camera_view::SuspensionKick::default();
      camera.set_mode(mode);
      assets.wheels.reset()?;
      assets.driver.reset()?;
      rivals = rival_data
        .iter()
        .map(lrsim::rivals::Rival::new)
        .collect::<Result<Vec<_>, _>>()?;
      positions =
        lrsim::race_positions::RacePositions::new(&lrsim::race_positions::poses(&car, &rivals));
      for gpu in &mut rival_gpu {
        gpu.reset()?;
      }
      for object in &mut assets.scenery {
        object.reset()?;
      }
      world_time = 0.0;
      assets.environment.reset()?;
      track_materials.reset();
      for object in &mut assets.animated_scenery {
        object.reset()?;
      }
      audio.start_sequence(!prelude);
      driver = lrsim::diagnostic_driver::DiagnosticDriver::new(assets.diagnostic_line.clone());
      if let Some(powers) = &mut powers {
        powers.reset();
      }
      if let Some(trial) = &mut trial {
        trial.reset(&car);
      }
      finish_frame = None;
      report.player_finish_time = None;
      report.post_finish_frames = 0;
      report.post_finish_rival_distance = 0.0;
    }
    let dt = if paused || !input_gate.active() || frame == 0 {
      0.0
    } else if options.scripted_drive || options.scripted_route {
      1.0 / 60.0
    } else {
      get_frame_time().min(0.1)
    };
    let axis = |a, b| f32::from(input_gate.down(a)) - f32::from(input_gate.down(b));
    let intro_was_ready = intro.as_ref().map_or(intro_delay <= 0.0, |i| i.ready());
    if let Some(intro) = &mut intro {
      if let Some((eye, forward, up)) = intro.advance(
        dt,
        !options.scripted_drive && !options.scripted_route && input_gate.pressed(KeyCode::Enter),
      )? {
        if options.intro_smoke {
          println!("intro handoff frame {frame}");
        }
        camera.handoff(eye, forward, up);
      }
    } else {
      intro_delay = (intro_delay - dt).max(0.0);
    }
    let intro_ready = intro.as_ref().map_or(intro_delay <= 0.0, |i| i.ready());
    if !intro_was_ready && intro_ready {
      if options.intro_smoke {
        println!("intro countdown frame {frame}");
      }
      audio.countdown();
    }
    let actions = if options.scripted_route || (options.play && race.finished) {
      driver.actions(&car)
    } else if options.scripted_drive {
      Actions {
        throttle: 1.0,
        steer: 0.0,
      }
    } else {
      Actions {
        throttle: axis(KeyCode::Up, KeyCode::Down),
        steer: axis(KeyCode::Left, KeyCode::Right) * if options.mirrored { -1.0 } else { 1.0 },
      }
    };
    let previous_lap = race.laps.completed_laps;
    let was_finished = race.finished;
    let released = start_gate.released();
    let collisions = car.collisions;
    let racer_contacts = report.racer_contacts;
    let was_grounded = car.grounded;
    let previous_velocity = car.velocity();
    if frame > 0 {
      let dt = if intro_was_ready {
        start_gate.advance(f64::from(dt)) as f32
      } else {
        0.0
      };
      let actors = power_racers(&car, &race, &rivals);
      if let Some(powers) = &mut powers {
        if options.turbo_smoke && frame == 240 {
          powers.inventories[0].kind = 3;
          powers.inventories[0].whites = 1;
          powers.use_power(0, &actors);
        }
        powers.advance(dt, &actors, |old, new| {
          assets.contacts.sweep(old, new).is_some()
        });
        if dt > 0.0 {
          let before = powers.inventories[0].clone();
          powers.collect(&actors);
          if powers.inventories[0].pickups > before.pickups {
            audio.pickup(
              (powers.inventories[0].whites > before.whites)
                .then_some(powers.inventories[0].whites),
            );
          }
          for index in 0..actors.len() {
            let use_power = if index == 0 && !options.scripted_route {
              input_gate.pressed(KeyCode::Space)
            } else {
              let inventory = &powers.inventories[index];
              inventory.whites == 3 || inventory.held_ms > 2000.0
            };
            if use_power {
              let kind = powers.inventories[index].kind;
              let tier = powers.inventories[index].whites;
              if powers.use_power(index, &actors) {
                if index == 0 {
                  audio.effect(kind, tier);
                  assets.driver.power_reaction(kind == 3);
                } else {
                  rival_gpu[index - 1].power_reaction(kind == 3);
                }
              }
            }
          }
        }
      }
      let old = car.position;
      if !race.finished || options.play {
        let hits = lrsim::power_vehicle::advance(
          &mut car,
          actions,
          powers.as_ref().map(|p| (&p.inventories[0], &p.rules)),
          &actors,
          &assets.contacts,
          &assets.checkpoints,
          &world,
          positions.state_mut(0),
          dt,
        )?;
        positions.add_contacts(hits);
      }
      let travelled = length(lrsim::contact::sub(car.position, old));
      report.distance_travelled += travelled;
      if !race.finished || options.play {
        assets.wheels.advance(car.drive_speed(), dt)?;
      }
      race.advance(old, car.position, &assets.lap_zones, f64::from(dt));
      for (index, rival) in rivals.iter_mut().enumerate() {
        if let Some(powers) = &powers {
          let inventory = &powers.inventories[index + 1];
          rival.motion.multiplier = if inventory.turbo.active() {
            powers.rules.turbo_route_speed
          } else {
            1.0
          };
          if inventory.curse_ms > 0.0 {
            rival.motion.multiplier *= powers.rules.weapons.curse_speed_scale;
          }
          if inventory.oil_ms > 0.0 {
            rival.motion.multiplier *= powers.rules.weapons.curse_speed_scale;
          }
          if inventory.grapple_ms > 0.0 {
            rival.motion.multiplier = powers.rules.turbo_route_speed;
          }
          // Original AI warp uses CarBody::SetFlags30008_Boost:
          // immediate route time scale4.5, not player CPB transit.
          if inventory.warp_ms > 0.0 && inventory.warp_ms <= powers.rules.warp_transit_ms as f32 {
            rival.motion.multiplier = powers.rules.warp_route_speed;
            rival.motion.cursor.speed = powers.rules.warp_route_speed;
          }
          if inventory.hit_ms > 0.0 {
            rival.motion.cursor.speed = 0.0;
            rival.motion.multiplier = 0.0;
          }
        }
        let old = rival.motion.position;
        let hits = rival.advance_dispatched(
          f64::from(dt),
          &assets.lap_zones,
          &assets.contacts,
          &world,
          positions.state_mut(index + 1),
        );
        positions.add_contacts(hits);
        if was_finished {
          report.post_finish_rival_distance +=
            length(lrsim::contact::sub(rival.motion.position, old));
        }
      }
      if dt > 0.0 {
        let (pairs, hits) = lrsim::race_contacts::resolve_dispatched(
          &mut car,
          &assets.chassis,
          &assets.contacts,
          &mut rivals,
          &world,
          positions.state_mut(0),
        );
        report.racer_contacts += pairs;
        positions.add_contacts(hits);
        positions.rank_dispatched(
          &lrsim::race_positions::poses(&car, &rivals),
          &assets.checkpoints,
        );
        positions.finish(&lrsim::race_positions::finished(&race, &rivals));
      }
      let ranks = positions.report().ranks;
      assets.driver.advance_race(
        car.drive_speed(),
        actions.throttle,
        actions.steer,
        if race.finished { Some(ranks[0]) } else { None },
        dt,
      )?;
      for (index, (rival, gpu)) in rivals.iter().zip(&mut rival_gpu).enumerate() {
        gpu.advance(
          rival,
          if rival.race.finished {
            Some(ranks[index + 1])
          } else {
            None
          },
          dt,
        )?;
      }
      if was_finished {
        report.post_finish_frames += 1;
      }
    }
    if !released && start_gate.released() {
      audio.go();
    }
    if !was_finished && race.finished {
      audio.finish(positions.report().ranks[0]);
      finish_frame = Some(frame);
      report.player_finish_time = Some(race.elapsed);
    }
    let velocity = Vec3::from_array(car.velocity());
    let left = Vec3::from_array(car.up).cross(Vec3::from_array(car.forward()));
    // Audible slip follows actual lateral motion, never the steering key.
    // 20 world units/s is a modern mix full-scale, not a physics threshold.
    let slip = (velocity.dot(left).abs() / 20.0).clamp(0.0, 1.0);
    audio.driving_feedback(
      velocity.length(),
      car.speed(),
      actions.throttle,
      slip,
      car.grounded,
      car.collisions > collisions,
      report.racer_contacts > racer_contacts,
      paused || !input_gate.active(),
      dt,
    );
    let landing = !was_grounded && car.grounded;
    let hit = car.collisions > collisions || report.racer_contacts > racer_contacts;
    let impulse = if landing {
      (-previous_velocity[2] / 80.0).clamp(0.0, 1.0)
    } else if hit {
      (Vec3::from_array(previous_velocity).distance(velocity) / 100.0).clamp(0.0, 1.0)
    } else {
      0.0
    };
    suspension_kick.advance(dt, impulse);
    if !start_gate.released() {
      report.countdown_frames += 1;
      report.countdown_max_player_displacement =
        report
          .countdown_max_player_displacement
          .max(length(lrsim::contact::sub(
            car.position,
            assets.start.position,
          )));
      for rival in &rivals {
        report.countdown_max_rival_displacement =
          report
            .countdown_max_rival_displacement
            .max(length(lrsim::contact::sub(
              rival.motion.position,
              rival.data.record.start_position,
            )));
      }
      report.countdown_racer_contacts = report.racer_contacts;
    }
    if car.grounded {
      report.supported_frames += 1;
    }
    if !paused && !options.scripted_route && input_gate.pressed(KeyCode::C) {
      camera.set_mode(camera.mode().next());
    }
    let rear_view = if let Some(check) = &camera_check {
      check.select(frame, &mut camera)
    } else {
      !options.scripted_route && !options.scripted_drive && input_gate.down(KeyCode::V)
    };
    camera.update_view(car.position, car.forward(), car.up, car.turn_rate, dt);
    world_time += dt;
    assets.environment.update(dt)?;
    track_materials.update(world_time, |name, surface| {
      track.set_scene_surface(name, surface)
    })?;
    for object in &mut assets.scenery {
      object.update(dt)?;
    }
    for object in &mut assets.animated_scenery {
      object.update(dt)?;
    }
    if let Some(trial) = &mut trial {
      trial.advance(
        race.elapsed,
        &car,
        if paused || !start_gate.released() || race.finished {
          0.0
        } else {
          dt
        },
      )?;
    }
    #[cfg(test)]
    crate::playable_loop_test::observe_vehicle(&car, &assets.diagnostic_line);
    #[cfg(test)]
    crate::playable_loop_test::observe_race(crate::playable_loop_test::Snapshot {
      frame,
      table: options.table.clone(),
      scripted: options.scripted_route || options.scripted_drive,
      position: car.position,
      start: assets.start.position,
      speed: car.speed(),
      heading: car.heading,
      throttle: actions.throttle,
      steer: actions.steer,
      dt,
      paused,
      released: start_gate.released(),
      elapsed: race.elapsed,
      laps: race.laps.completed_laps,
      finished: race.finished,
      collisions: car.collisions,
      racer_contacts: report.racer_contacts,
      rivals: rivals
        .iter()
        .map(|r| {
          (
            r.motion.position,
            r.race.elapsed,
            r.race.laps.completed_laps,
          )
        })
        .collect(),
      all_rivals_finished: rivals.iter().all(|r| r.race.finished),
      ranks: positions.report().ranks,
    });
    report.positions = positions.report();
    clear_background(Color::from_rgba(24, 31, 45, 255));
    let intro_showing = intro.as_ref().is_some_and(|i| i.showing());
    let (eye, direction, camera_up) = if intro_showing {
      intro.as_ref().unwrap().pose()?
    } else {
      camera.view_pose(rear_view)
    };
    report.camera_eye = eye;
    report.camera_forward = direction;
    let mut view = crate::camera_view::view(
      race_view.original(eye),
      race_view.original(direction),
      race_view.original(camera_up),
      screen_width() / screen_height(),
    );
    if intro_showing {
      let (fov, near, far) = intro.as_ref().unwrap().projection();
      view.fovy = fov;
      view.z_near = near;
      view.z_far = far;
    } else if options.play && camera_check.is_none() {
      suspension_kick.apply(&mut view);
    }
    let eye = race_view.native(eye);
    set_camera(&view);
    assets.environment.draw(eye);
    track.draw_at(race_view.world_matrix());
    gl_use_default_material();
    for object in &assets.scenery {
      object.draw(race_view);
    }
    for object in &assets.animated_scenery {
      object.draw(report.camera_eye, race_view);
    }
    let transform = race_view.car(car.position, car.forward(), car.up);
    assets.body.draw_at(transform);
    assets.wheels.draw_at(transform);
    assets.driver.draw_at(transform * assets.driver_seat);
    if let Some(trial) = &trial {
      trial.draw(race_view);
    }
    for (rival, gpu) in rivals.iter().zip(&rival_gpu) {
      gpu.draw_view(rival, race_view);
    }
    if let (Some(powers), Some(gpu)) = (&powers, &mut power_gpu) {
      gpu.draw(powers, &power_racers(&car, &race, &rivals), race_view)?;
    }
    for billboards in &assets.billboards {
      billboards.draw(report.camera_eye, camera_up, race_view);
    }
    set_default_camera();
    if let Some(check) = &mut camera_check {
      check
        .capture(frame, &camera, rear_view, car.position)
        .await?;
    }
    if options.play {
      if let Some(hud) = hud.as_ref().filter(|_| intro_ready) {
        hud.draw(
          &race,
          report.positions.ranks[0],
          &power_racers(&car, &race, &rivals),
          powers.as_ref(),
          start_gate.numeral(),
          rivals.iter().all(|r| r.race.finished),
        );
      }
      if let Some(trial) = &trial {
        trial.draw_times(&pause_ui);
      }
    } else {
      draw_rectangle(
        0.0,
        0.0,
        screen_width(),
        65.0,
        Color::new(0.0, 0.0, 0.0, 0.85),
      );
      draw_text(
        &format!("LEGO RACERS / {} / DIAGNOSTIC DRIVING", options.table),
        20.0,
        25.0,
        22.0,
        WHITE,
      );
      draw_text(
        "FIDELITY INCOMPLETE - arrows: drive | R: restart | Esc: exit",
        20.0,
        48.0,
        18.0,
        LIGHTGRAY,
      );
      draw_text(
        &format!(
          "Speed {:5.1}  |  Distance {:5.1}  |  {}  |  Contacts {}",
          car.speed(),
          report.distance_travelled,
          if car.grounded { "Grounded" } else { "Airborne" },
          car.collisions
        ),
        20.0,
        screen_height() - 20.0,
        22.0,
        WHITE,
      );
      if let Some(numeral) = start_gate.numeral().filter(|_| intro_ready) {
        let text = numeral.to_string();
        let width = measure_text(&text, None, 90, 1.0).width;
        draw_text(
          &text,
          (screen_width() - width) * 0.5,
          screen_height() * 0.4,
          90.0,
          YELLOW,
        );
      } else if race.finished {
        draw_rectangle(250.0, 260.0, 500.0, 170.0, Color::new(0.0, 0.0, 0.0, 0.9));
        draw_text("THREE LAPS FINISHED", 290.0, 305.0, 30.0, YELLOW);
        draw_text(
          &format!("Time {:.2} | R: race again", race.elapsed),
          290.0,
          345.0,
          24.0,
          WHITE,
        );
        draw_text(
          "Diagnostic UI - original results UI pending",
          290.0,
          385.0,
          18.0,
          LIGHTGRAY,
        );
      } else {
        draw_text(
          &format!(
            "POS {} | LAP {} / 3 | TIME {:.2}",
            report.positions.ranks[0],
            race.laps.completed_laps + 1,
            race.elapsed
          ),
          screen_width() - 400.0,
          screen_height() - 20.0,
          22.0,
          YELLOW,
        );
      }
      for (index, rival) in rivals.iter().enumerate() {
        draw_text(
          &format!(
            "{}  P{}  {} / 3  {}",
            rival.data.name,
            report.positions.ranks[index + 1],
            rival.race.laps.completed_laps,
            if rival.race.finished {
              format!("{:.2}s", rival.race.elapsed)
            } else {
              "on-route / effects incomplete".into()
            }
          ),
          20.0,
          90.0 + index as f32 * 20.0,
          16.0,
          YELLOW,
        );
      }
    }
    if paused {
      draw_rectangle(
        0.0,
        0.0,
        screen_width(),
        screen_height(),
        Color::new(0.0, 0.0, 0.0, 0.75),
      );
      pause_ui.text("Paused", 245.0, 130.0, 38.0, YELLOW);
      let labels = ["Return to Race", "Restart Race", "Exit Race"].map(str::to_string);
      if let Some(choice) = input_gate
        .active()
        .then(|| pause_ui.buttons(&labels, 220.0, 210.0, 60.0))
        .flatten()
      {
        input_gate.suspend();
        match choice {
          0 => paused = false,
          1 => restart_requested = true,
          _ => break,
        }
      }
    }
    let last = options.frames == Some(frame + 1)
      || (options.scripted_route
        && (!options.play || rivals.iter().all(|r| r.race.finished))
        && finish_frame.is_some_and(|finished| frame - finished >= options.finish_tail_frames));
    if let Some(directory) = &options.capture {
      if options.turbo_smoke && matches!(frame, 241 | 260 | 290) {
        let name = match frame {
          241 => "turbo-pitch.png",
          260 => "turbo-grounded.png",
          _ => "turbo-running.png",
        };
        capture::save_frame(&directory.join(name), &capture::screen_data().await)?;
      }
      if options.capture_clip
        && !options.intro_smoke
        && !options.camera_smoke
        && options.scripted_route
        && (240..600).contains(&frame)
        && frame % 2 == 0
      {
        let image = capture::screen_data().await;
        if video.is_none() {
          video = Some(crate::capture_video::Video::start(
            &directory.join("driving-motion.mp4"),
            image.width,
            image.height,
          )?);
        }
        video.as_mut().unwrap().frame(&image)?;
      }
      if options.intro_smoke {
        if frame == 0 || frame == 120 {
          capture::save_frame(
            &directory.join(if frame == 0 {
              "intro-camera.png"
            } else {
              "intro-chase.png"
            }),
            &capture::screen_data().await,
          )?;
        }
        if options.capture_clip && frame < 360 && frame % 2 == 0 {
          let image = capture::screen_data().await;
          if video.is_none() {
            video = Some(crate::capture_video::Video::start(
              &directory.join("intro-motion.mp4"),
              image.width,
              image.height,
            )?);
          }
          video.as_mut().unwrap().frame(&image)?;
        }
      }
      if let Some(powers) = &powers {
        let inventory = &powers.inventories[0];
        if inventory.pickups > 0 && !directory.join("pickup.png").exists() {
          capture::save_frame(&directory.join("pickup.png"), &capture::screen_data().await)?;
        }
        if inventory.uses > 0 && !directory.join("power-active.png").exists() {
          capture::save_frame(
            &directory.join("power-active.png"),
            &capture::screen_data().await,
          )?;
        }
      }
      if frame == 120 {
        capture::save_frame(
          &directory.join("countdown.png"),
          &capture::screen_data().await,
        )?;
      }
      if frame == 300 {
        capture::save_frame(&directory.join("moving.png"), &capture::screen_data().await)?;
      }
      if race.laps.completed_laps > previous_lap {
        capture::save_frame(
          &directory.join(format!("lap-{}.png", race.laps.completed_laps)),
          &capture::screen_data().await,
        )?;
      }
      if frame == 0 || last {
        let image = capture::screen_data().await;
        if frame == 0 {
          let (hash, pixels) = capture::save_frame(&directory.join("first.png"), &image)?;
          report.frame_hash_first = hash;
          report.non_background_pixels_first = pixels;
        }
        if last {
          let (hash, pixels) = capture::save_frame(&directory.join("last.png"), &image)?;
          report.frame_hash_last = hash;
          report.non_background_pixels_last = pixels;
        }
      }
    }
    next_frame().await;
    report.native_frames_presented += 1;
    if last {
      break;
    }
  }
  report.position = car.position;
  if let Some(video) = video.take() {
    video.finish()?;
  }
  report.speed = car.speed();
  report.collisions = car.collisions;
  report.unresolved_contacts = car.unresolved_contacts;
  report.secondary_wheel_contacts = car.secondary_wheel_contacts;
  report.rejected_empty_manifolds = lrsim::racer_box::rejected_empty_manifolds();
  report.original_line_samples_reached = driver.index;
  report.completed_laps = race.laps.completed_laps;
  report.race_finished = race.finished;
  report.race_elapsed = race.elapsed;
  let original_ghost_time = trial.as_ref().map(|t| t.original_time);
  let trial_ghost = if race.finished {
    trial
      .map(|t| t.finish(&options, race.lap_times.clone()))
      .transpose()?
  } else {
    None
  };
  report.lap_times = race.lap_times;
  report.race_events = race.events;
  report.rivals = rivals.iter().map(|r| r.report()).collect();
  report.music_cues = audio.music_history();
  report.player_driver_clip = assets.driver.clip_name().into();
  report.rival_driver_clips = rival_gpu
    .iter()
    .map(|gpu| gpu.driver_clip().into())
    .collect();
  if let Some(powers) = powers {
    report.powerup_inventories = powers.inventories;
    report.powerup_impacts = powers.weapons.impacts;
    report.powerup_hits_by_kind = powers.weapons.hits;
  }
  if let Some(directory) = &options.capture {
    let json = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
    std::fs::write(directory.join("drive.json"), json).map_err(|e| e.to_string())?;
  }
  println!(
    "native diagnostic drive: {} frames, {:.2} units, {} supported frames, {} wall contacts",
    report.native_frames_presented,
    report.distance_travelled,
    report.supported_frames,
    report.collisions
  );
  Ok(if race.finished {
    Some(RaceResult {
      names: std::iter::once(options.car.clone())
        .chain(rivals.iter().map(|r| r.data.name.clone()))
        .collect(),
      ranks: positions.report().ranks,
      time: race.elapsed,
      laps: race.laps.completed_laps,
      trial_ghost,
      original_ghost_time,
    })
  } else {
    None
  })
}

fn power_racers(
  car: &Vehicle,
  race: &lrsim::race::Race,
  rivals: &[lrsim::rivals::Rival<'_>],
) -> Vec<lrsim::powerups::Racer> {
  std::iter::once(lrsim::powerups::Racer {
    position: car.position,
    forward: car.forward(),
    finished: race.finished,
    up: car.up,
  })
  .chain(rivals.iter().map(|r| lrsim::powerups::Racer {
    position: r.motion.position,
    forward: r.motion.basis[..3].try_into().unwrap(),
    up: r.motion.basis[6..9].try_into().unwrap(),
    finished: r.race.finished,
  }))
  .collect()
}
