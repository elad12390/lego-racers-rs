//! Two simultaneously rendered and independently simulated native player views.
//! Keyboard defaults and split HUD composition remain provisional.
use crate::platform::prelude::*;
use crate::{
  camera_rig::ChaseRig, drive_assets::DriveAssets, gpu::TrackGpu, options::Options,
  profile::Profile, race_view::RaceView,
};
use lrformats::{library::Library, world};
use lrsim::{
  diagnostic_driver::DiagnosticDriver, race_data::RaceData, vehicle::Actions, versus::Session,
};
use serde::Serialize;
#[derive(Serialize)]
pub struct Outcome {
  pub times: [f64; 2],
  pub ranks: [u32; 2],
  pub names: [String; 2],
}
fn player_options(options: &Options, profile: &Profile) -> Options {
  let mut options = options.clone();
  options.car = profile.car.clone();
  options.driver = profile.driver.clone();
  options.camera_mode = profile.camera_mode;
  options.custom_driver = profile.custom_driver.clone();
  options.custom_build = if profile.custom_enabled {
    profile.custom_build.clone()
  } else {
    None
  };
  options
}
pub async fn run(
  options: &Options,
  library: &Library,
  players: &[Profile; 2],
  mut track: TrackGpu,
) -> Result<Option<Outcome>, String> {
  let selected = players.each_ref().map(|p| player_options(options, p));
  let mut track_materials =
    crate::world_material::Player::main_track(library, &options.table, &mut track)?;
  let mut world_time = 0.0;
  let mut assets = [
    DriveAssets::load(&selected[0])?,
    DriveAssets::load(&selected[1])?,
  ];
  let mut data = [
    RaceData::load(library, &options.table, &assets[0].chassis.name)?,
    RaceData::load(library, &options.table, &assets[1].chassis.name)?,
  ];
  let table = library
    .jam()
    .tables
    .iter()
    .find(|t| t.name.eq_ignore_ascii_case(&options.table))
    .ok_or("missing versus race")?;
  let entry = table
    .entries
    .iter()
    .find(|e| e.name.ends_with(".SPB"))
    .ok_or("missing versus grid")?;
  let mut starts = world::start_positions(library.jam().bytes(entry).map_err(|e| e.to_string())?)
    .map_err(|e| e.to_string())?;
  starts.sort_by_key(|s| s.slot);
  if starts.len() < 2 {
    return Err("original race has fewer than two start positions".into());
  }
  for i in 0..2 {
    data[i].start = starts[i].clone();
    data[i].chassis.mass = assets[i].chassis.mass;
  }
  let mut session = Session::new(
    &data,
    lrsim::powerups::Powerups::load(library, &options.table, 2)?,
  )?;
  let mut power_gpu = crate::powerup_gpu::PowerupGpu::load(library, &session.powers.rules)?;
  let mut intro = crate::race_intro::Intro::load(library, &options.table)?;
  let mut intro_delay = if intro.is_none() { 2.0f32 } else { 0.0 };
  let hud = crate::race_hud::RaceHud::load(library, &options.table, options.mirrored)?;
  let mut drivers =
    std::array::from_fn::<_, 2, _>(|i| DiagnosticDriver::new(data[i].diagnostic_line.clone()));
  let mut cameras = std::array::from_fn::<_, 2, _>(|i| {
    let mut rig = ChaseRig::new(session.cars[i].position, session.cars[i].forward());
    rig.set_mode(players[i].camera_mode);
    rig.update_view(
      session.cars[i].position,
      session.cars[i].forward(),
      session.cars[i].up,
      0.0,
      1.0 / 60.0,
    );
    rig
  });
  let mut ui = crate::menu_ui::MenuUi::load(library)?;
  let mut paused = false;
  let mut input_gate = crate::input_gate::InputGate::default();
  let mut frames = 0u32;
  let mut travel = [0.0f32; 2];
  let mut countdown_displacement = [0.0f32; 2];
  let mut finish_sound = false;
  let silent = options.versus_smoke;
  let view = RaceView {
    mirrored: options.mirrored,
  };
  let mut video = None;
  let mut video_frames = 0;
  let mut audio = crate::race_audio::RaceAudio::load_settings(
    library,
    &options.table,
    options
      .jam
      .parent()
      .ok_or("missing original tune directory")?,
    (!silent || options.audible_diagnostic) && players[0].sound,
    (!silent || options.audible_diagnostic) && players[0].music,
  )
  .await?;
  audio.start_sequence(false);
  loop {
    if input_gate.poll(silent) {
      paused = true;
      input_gate.suspend();
    }
    if is_quit_requested() {
      return Ok(None);
    }
    if input_gate.pressed(KeyCode::Escape) {
      paused = !paused;
      input_gate.suspend();
    }
    if !silent && session.races.iter().all(|r| r.finished) && input_gate.pressed(KeyCode::Enter) {
      break;
    }
    let restart = input_gate.pressed(KeyCode::R) && !silent;
    if restart {
      input_gate.suspend();
      session.reset()?;
      paused = false;
      if let Some(intro) = &mut intro {
        intro.reset();
      }
      intro_delay = if intro.is_none() { 2.0 } else { 0.0 };
      audio.start_sequence(false);
      finish_sound = false;
      for i in 0..2 {
        let mode = cameras[i].mode();
        cameras[i] = ChaseRig::new(session.cars[i].position, session.cars[i].forward());
        cameras[i].set_mode(mode);
        drivers[i] = DiagnosticDriver::new(data[i].diagnostic_line.clone());
        assets[i].driver.reset()?;
        assets[i].wheels.reset()?;
      }
      assets[0].reset_world()?;
      world_time = 0.0;
      track_materials.reset();
    }
    let axis = |a, b| f32::from(input_gate.down(a)) - f32::from(input_gate.down(b));
    let keys = [
      (
        KeyCode::Up,
        KeyCode::Down,
        KeyCode::Left,
        KeyCode::Right,
        KeyCode::Space,
      ),
      (KeyCode::W, KeyCode::S, KeyCode::A, KeyCode::D, KeyCode::F),
    ];
    let actions = std::array::from_fn(|i| {
      if silent || session.races[i].finished {
        drivers[i].actions(&session.cars[i])
      } else {
        Actions {
          throttle: axis(keys[i].0, keys[i].1),
          steer: axis(keys[i].2, keys[i].3) * if options.mirrored { -1.0 } else { 1.0 },
        }
      }
    });
    let uses = std::array::from_fn(|i| {
      if silent {
        session.powers.inventories[i].held_ms > 2000.0
      } else {
        input_gate.pressed(keys[i].4)
      }
    });
    let old = session.cars.each_ref().map(|c| c.position);
    let released = session.gate.released();
    let dt = if paused || !input_gate.active() || frames == 0 {
      0.0
    } else if silent {
      1.0 / 60.0
    } else {
      get_frame_time().min(0.1)
    };
    let inventories = session.powers.inventories.clone();
    let contacts = session.contacts;
    let intro_was_ready = intro.as_ref().map_or(intro_delay <= 0.0, |i| i.ready());
    if let Some(intro) = &mut intro {
      if let Some((eye, forward, up)) =
        intro.advance(dt, !silent && input_gate.pressed(KeyCode::Enter))?
      {
        for camera in &mut cameras {
          camera.handoff(eye, forward, up);
        }
      }
    } else {
      intro_delay = (intro_delay - dt).max(0.0);
    }
    let intro_ready = intro.as_ref().map_or(intro_delay <= 0.0, |i| i.ready());
    if !intro_was_ready && intro_ready {
      audio.countdown();
    }
    let active = session.step(if intro_was_ready { dt } else { 0.0 }, actions, uses)?;
    assets[0].update_world(dt)?;
    world_time += dt;
    track_materials.update(world_time, |name, surface| {
      track.set_scene_surface(name, surface)
    })?;
    if session.contacts > contacts {
      audio.contact();
    }
    for (i, previous) in inventories.iter().enumerate() {
      let now = &session.powers.inventories[i];
      if now.pickups > previous.pickups {
        audio.pickup((now.whites > previous.whites).then_some(now.whites));
      }
      if now.uses > previous.uses {
        audio.effect(previous.kind, previous.whites);
      }
    }
    if !released && session.gate.released() {
      audio.go();
    }
    let ranks = session.positions.report().ranks;
    if session.races[0].finished && !finish_sound {
      audio.finish(ranks[0]);
      finish_sound = true;
    }
    audio.engine(session.cars[0].speed(), actions[0].steer, paused);
    for i in 0..2 {
      travel[i] += lrsim::contact::length(lrsim::contact::sub(session.cars[i].position, old[i]));
      if !session.gate.released() {
        countdown_displacement[i] = countdown_displacement[i].max(lrsim::contact::length(
          lrsim::contact::sub(session.cars[i].position, data[i].start.position),
        ));
      }
      assets[i]
        .wheels
        .advance(session.cars[i].drive_speed(), active)?;
      assets[i].driver.advance_race(
        session.cars[i].drive_speed(),
        actions[i].throttle,
        actions[i].steer,
        session.races[i].finished.then_some(ranks[i]),
        active,
      )?;
      if !paused && !silent && input_gate.pressed(if i == 0 { KeyCode::C } else { KeyCode::Q }) {
        cameras[i].set_mode(cameras[i].mode().next());
      }
      cameras[i].update_view(
        session.cars[i].position,
        session.cars[i].forward(),
        session.cars[i].up,
        session.cars[i].turn_rate,
        dt,
      );
    }
    clear_background(BLACK);
    for player in 0..2 {
      let rect = Rect::new(
        0.0,
        player as f32 * screen_height() / 2.0,
        screen_width(),
        screen_height() / 2.0,
      );
      let rear = !silent && input_gate.down(if player == 0 { KeyCode::V } else { KeyCode::E });
      let intro_showing = intro.as_ref().is_some_and(|i| i.showing());
      let (eye, forward, up) = if intro_showing {
        intro.as_ref().unwrap().pose()?
      } else {
        cameras[player].view_pose(rear)
      };
      let mut camera = crate::camera_view::view(
        view.original(eye),
        view.original(forward),
        view.original(up),
        rect.w / rect.h,
      );
      camera.viewport = Some((
        0,
        (screen_height() - rect.y - rect.h) as i32,
        rect.w as i32,
        rect.h as i32,
      ));
      if intro_showing {
        let (fov, near, far) = intro.as_ref().unwrap().projection();
        camera.fovy = fov;
        camera.z_near = near;
        camera.z_far = far;
      }
      set_camera(&camera);
      assets[0].environment.draw(view.native(eye));
      track.draw_at(view.world_matrix());
      gl_use_default_material();
      for object in &assets[0].scenery {
        object.draw(view);
      }
      for object in &assets[0].animated_scenery {
        object.draw(eye, view);
      }
      for i in 0..2 {
        let car = &session.cars[i];
        let transform = view.car(car.position, car.forward(), car.up);
        assets[i].body.draw_at(transform);
        assets[i].wheels.draw_at(transform);
        assets[i].driver.draw_at(transform * assets[i].driver_seat);
      }
      power_gpu.draw(&session.powers, &session.actors(), view)?;
      for billboards in &assets[0].billboards {
        billboards.draw(eye, up, view);
      }
      set_default_camera();
      if intro_ready {
        hud.draw_viewport(
          rect,
          player,
          &session.races[player],
          ranks[player],
          &session.actors(),
          Some(&session.powers),
          session.gate.numeral(),
          session.races.iter().all(|r| r.finished),
        );
      }
    }
    draw_rectangle(0.0, screen_height() / 2.0 - 1.0, screen_width(), 2.0, BLACK);
    if options.capture_clip && (360..1080).contains(&frames) && frames % 2 == 0 {
      let image = crate::capture::screen_data().await;
      if video.is_none() {
        video = Some(crate::capture_video::Video::start(
          &options
            .capture
            .as_ref()
            .unwrap()
            .join("versus-gameplay.mp4"),
          image.width,
          image.height,
        )?);
      }
      video.as_mut().unwrap().frame(&image)?;
    }
    if frames == 1080 {
      if let Some(video) = video.take() {
        video_frames = video.finish()?;
      }
    }
    if paused {
      ui.original_background("Paused", false);
      if let Some(choice) = input_gate
        .active()
        .then(|| {
          ui.buttons(
            &["Resume".into(), "Restart Race".into(), "Quit Race".into()],
            240.0,
            220.0,
            55.0,
          )
        })
        .flatten()
      {
        input_gate.suspend();
        match choice {
          0 => paused = false,
          1 => {
            session.reset()?;
            paused = false;
            if let Some(intro) = &mut intro {
              intro.reset();
            }
            intro_delay = if intro.is_none() { 2.0 } else { 0.0 };
            audio.start_sequence(false);
            finish_sound = false;
            for i in 0..2 {
              let mode = cameras[i].mode();
              cameras[i] = ChaseRig::new(session.cars[i].position, session.cars[i].forward());
              cameras[i].set_mode(mode);
              drivers[i] = DiagnosticDriver::new(data[i].diagnostic_line.clone());
              assets[i].driver.reset()?;
              assets[i].wheels.reset()?;
            }
            assets[0].reset_world()?;
            world_time = 0.0;
            track_materials.reset();
          }
          _ => return Ok(None),
        }
      }
    }
    if let Some(dir) = &options.capture {
      if frames == 60 || frames == 900 {
        crate::capture::save_frame(
          &dir.join(if frames == 60 {
            "versus-countdown.png"
          } else {
            "versus-moving.png"
          }),
          &crate::capture::screen_data().await,
        )?;
      }
    }
    frames += 1;
    if silent && session.races.iter().all(|r| r.finished) {
      break;
    }
    if options.frames.is_some_and(|limit| frames >= limit) && silent {
      break;
    }
    next_frame().await;
  }
  let outcome = Outcome {
    times: session.races.each_ref().map(|r| r.elapsed),
    ranks: session
      .positions
      .report()
      .ranks
      .try_into()
      .map_err(|_| "versus ranks missing")?,
    names: players.each_ref().map(|p| p.name.clone()),
  };
  if let Some(video) = video.take() {
    video_frames = video.finish()?;
  }
  if let Some(dir) = &options.capture {
    crate::capture::save_frame(
      &dir.join("versus-last.png"),
      &crate::capture::screen_data().await,
    )?;
    let evidence = serde_json::json!({"mode":"native_two_free_vehicles_scripted_actions_not_physical_input_or_parity_acceptance","outcome":outcome,"frames":frames,"laps":session.races.each_ref().map(|r|r.laps.completed_laps),"finished":session.races.each_ref().map(|r|r.finished),"travel":travel,"countdown_displacement":countdown_displacement,"racer_contacts":session.contacts,"inventories":session.powers.inventories,"world_unresolved":session.cars.each_ref().map(|c|c.unresolved_contacts)});
    std::fs::write(
      dir.join("versus.json"),
      serde_json::to_vec_pretty(&evidence).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if options.capture_clip {
      std::fs::write(dir.join("versus-video.json"),serde_json::to_vec_pretty(&serde_json::json!({"mode":"actual_native_framebuffer_scripted_actions_silent_not_physical_input_acceptance","fps":30,"samples":video_frames,"duration_seconds":video_frames as f32/30.0,"simulation_frames":[360,1080]})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
    }
  }
  if silent && !session.races.iter().all(|r| r.finished) {
    return Err("two-player native smoke did not finish both cars; evidence preserved".into());
  }
  Ok(session.races.iter().all(|r| r.finished).then_some(outcome))
}
