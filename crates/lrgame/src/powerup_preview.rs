//! Isolated effect-asset playback using the same renderer as solo and versus.
//! Forced inventory states are diagnostics, not earned pickups or gameplay.
use crate::platform::prelude::*;
use crate::{drive_assets::DriveAssets, gpu::TrackGpu, options::Options, race_view::RaceView};

pub async fn run(options: &Options, mut track: TrackGpu) -> Result<(), String> {
  let mut assets = DriveAssets::load(options)?;
  let library = lrformats::library::Library::open(&options.jam).map_err(|e| e.to_string())?;
  let mut materials =
    crate::world_material::Player::main_track(&library, &options.table, &mut track)?;
  let mut powers = lrsim::powerups::Powerups::new(Vec::new(), 1)?;
  let mut gpu = crate::powerup_gpu::PowerupGpu::load(&library, &powers.rules)?;
  let car = lrsim::vehicle::Vehicle::spawn(&assets.start, &assets.chassis, &assets.contacts);
  let racers = [lrsim::powerups::Racer {
    position: car.position,
    forward: car.forward(),
    up: car.up,
    finished: false,
  }];
  let view = RaceView {
    mirrored: options.mirrored,
  };
  let center = view.native(car.position) + Vec3::Y * 5.0;
  let eye = center + vec3(38.0, 24.0, 45.0);
  let transform = view.car(car.position, car.forward(), car.up);
  let frames = options.frames.ok_or("unbounded effect inspection")?;
  let directory = options.capture.as_ref().ok_or("effect capture missing")?;
  std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
  let mut video = None;
  let mut hashes = Vec::new();
  let mut native_focus_samples = Vec::new();
  let turbo = options.scenery_object.as_deref() == Some("turbo");
  let shield_fade = options.scenery_object.as_deref() == Some("shield-fade");
  let stem = if turbo { "turbo" } else { "shield" };
  for frame in 0..frames {
    if is_quit_requested() {
      return Err("effect inspection interrupted".into());
    }
    let tier = if frame < frames / 2 {
      0
    } else if turbo {
      1
    } else {
      3
    };
    if turbo {
      if frame == 0 || frame == frames / 2 {
        powers.inventories[0].kind = 3;
        powers.inventories[0].whites = tier;
        powers.use_power(0, &racers);
      }
      if frame > 0 {
        powers.advance(1.0 / 30.0, &racers, |_, _| false);
      }
    } else if shield_fade {
      if frame == 0 {
        powers.inventories[0].kind = 2;
        powers.inventories[0].whites = 0;
        powers.use_power(0, &racers);
      } else {
        powers.advance(1.0 / 30.0, &racers, |_, _| false);
      }
    } else {
      let seconds = (frame % (frames / 2).max(1)) as f32 / 30.0;
      powers.inventories[0].shield_tier = tier;
      powers.inventories[0]
        .shield
        .start(powers.rules.shield_duration_ms[tier as usize]);
      powers.inventories[0]
        .shield
        .advance((seconds * 1000.0) as u32, powers.rules.shield_fade_ms);
    }
    clear_background(BLACK);
    let mut camera = Camera3D {
      position: eye,
      target: center,
      up: Vec3::Y,
      fovy: crate::camera_view::FOV_DEGREES.to_radians(),
      z_near: crate::camera_view::NEAR,
      z_far: 10000.0,
      ..Default::default()
    };
    set_camera(&camera);
    assets.environment.draw(eye);
    camera.z_far = crate::camera_view::FAR;
    set_camera(&camera);
    materials.update(frame as f32 / 30.0, |name, surface| {
      track.set_scene_surface(name, surface)
    })?;
    track.draw_at(view.world_matrix());
    gl_use_default_material();
    assets.update_world(1.0 / 30.0)?;
    for object in &assets.scenery {
      object.draw(view);
    }
    let original_eye = [eye.x, -eye.z, eye.y];
    for object in &assets.animated_scenery {
      object.draw(original_eye, view);
    }
    assets.body.draw_at(transform);
    assets.wheels.draw_at(transform);
    assets.driver.draw_at(transform * assets.driver_seat);
    gpu.draw(&powers, &racers, view)?;
    set_default_camera();
    draw_rectangle(
      0.0,
      0.0,
      screen_width(),
      65.0,
      Color::new(0.0, 0.0, 0.0, 0.85),
    );
    draw_text(
      &if turbo {
        format!(
          "ORIGINAL TURBO {} / THREE RIGS / {:?}",
          tier + 1,
          powers.inventories[0].turbo.phase
        )
      } else {
        format!(
          "ORIGINAL SHIELD {} / PAIRED RIG / {:?} / ALPHA {}",
          powers.inventories[0].shield_tier + 1,
          powers.inventories[0].shield.phase,
          powers.inventories[0].shield.inner_alpha()
        )
      },
      20.0,
      27.0,
      22.0,
      WHITE,
    );
    draw_text(
      "Native effect-asset inspection / forced inventory / not gameplay acceptance",
      20.0,
      49.0,
      18.0,
      YELLOW,
    );
    let image = crate::capture::screen_data().await;
    if if turbo {
      frame == 30 || frame == frames / 2 + 60
    } else if shield_fade {
      frame == 60 || frame == 136
    } else {
      frame == 0 || frame + 1 == frames
    } {
      native_focus_samples.push(crate::native_window::focused());
      hashes.push(
        crate::capture::save_frame(
          &directory.join(if turbo {
            if frame == 30 {
              "turbo-active.png"
            } else {
              "turbo-tail.png"
            }
          } else if shield_fade {
            if frame == 60 {
              "shield-active.png"
            } else {
              "shield-fade.png"
            }
          } else if frame == 0 {
            "shield-first.png"
          } else {
            "shield-last.png"
          }),
          &image,
        )?
        .0,
      );
    }
    if options.capture_clip {
      if video.is_none() {
        video = Some(crate::capture_video::Video::start(
          &directory.join(format!("{stem}-motion.mp4")),
          image.width,
          image.height,
        )?);
      }
      video.as_mut().unwrap().frame(&image)?;
    }
    next_frame().await;
  }
  let video_frames = video.map(|v| v.finish()).transpose()?;
  std::fs::write(directory.join("powerup-preview.json"), serde_json::to_vec_pretty(&serde_json::json!({
    "mode": "isolated_native_effect_asset_playback_not_gameplay", "frames": frames,
    "video_frames": video_frames, "frame_hashes": hashes, "physical_input_verified": false,
    "native_window_focus_samples": native_focus_samples,
    "audible_output_verified": false, "source_world": "COMMON/POWERUP.WDB",
    "source_objects": if turbo { vec!["TurboL0", "turb0f1", "turb0f2", "TurboL1", "turb1f1", "turb1f2"] }
      else { vec!["shield0", "shldin0", "shield3", "shldin3"] },
    "limitations": ["Forced inventory states", "Provisional world lighting", "Grip, pitch impulse, smoke and live gameplay parity not established"]
  })).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
  Ok(())
}
