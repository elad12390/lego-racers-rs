//! Bounded native world-detail inspection, never physical-input/gameplay evidence.
use crate::platform::prelude::*;
use crate::{
  drive_assets::DriveAssets,
  gpu::{world_position, TrackGpu},
  options::Options,
  race_view::RaceView,
};

pub async fn run(options: &Options, mut track: TrackGpu) -> Result<(), String> {
  if matches!(
    options.scenery_object.as_deref(),
    Some("powerups" | "turbo" | "shield-fade")
  ) {
    return crate::powerup_preview::run(options, track).await;
  }
  // Stable supplied noise samples for diagnostic capture; not reproduction
  // of the original proprietary 1024-entry NoiseTable or its global cursor.
  rand::srand(1);
  let mut assets = DriveAssets::load(options)?;
  let library = lrformats::library::Library::open(&options.jam).map_err(|e| e.to_string())?;
  let mut track_materials =
    crate::world_material::Player::main_track(&library, &options.table, &mut track)?;
  let sky_inspection = options.scenery_object.as_deref() == Some("sky");
  let sky_timer = options.scenery_object.as_deref() == Some("sky-timer");
  let sky_world = options.scenery_object.as_deref() == Some("sky-world");
  if sky_timer {
    assets.environment.diagnostic_clock_noise(1023)?;
  }
  let (center, distance, height, target) = if sky_inspection || sky_timer || sky_world {
    (
      world_position(assets.start.position, 1.0) + Vec3::Y * 70.0,
      90.0,
      0.0,
      if sky_timer {
        "sky-timer".into()
      } else if sky_world {
        "sky-world".into()
      } else {
        "sky-shell".into()
      },
    )
  } else if let Some(name) = &options.scenery_object {
    let bounds = assets
      .animated_scenery
      .iter()
      .find(|o| o.name.eq_ignore_ascii_case(name))
      .map(|o| o.bounds())
      .or_else(|| {
        assets
          .scenery
          .iter()
          .find(|o| o.name.eq_ignore_ascii_case(name))
          .map(|o| o.bounds())
      })
      .ok_or("named original world-detail object missing")?;
    let (center, radius) = bounds;
    (center, radius * 1.8, radius * 1.8, name.clone())
  } else {
    let sprite = assets
      .billboards
      .iter()
      .find_map(|b| b.first())
      .ok_or("world-detail inspection requires an original billboard")?;
    (
      world_position(sprite.position, 1.0),
      sprite.width.max(sprite.height) * 2.0,
      sprite.height * 0.3,
      "billboard".into(),
    )
  };
  let view = RaceView {
    mirrored: options.mirrored,
  };
  eprintln!(
    "world-detail target={target} center={center:?} camera_distance={distance} height={height}"
  );
  let frames = options.frames.ok_or("unbounded world-detail inspection")?;
  let directory = options
    .capture
    .as_ref()
    .ok_or("world-detail capture missing")?;
  std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
  let mut hashes = Vec::new();
  let mut video = None;
  for frame in 0..frames {
    if is_quit_requested() {
      return Err("world-detail inspection interrupted".into());
    }
    let fraction = frame as f32 / frames.saturating_sub(1).max(1) as f32;
    let angle = if options.scenery_object.is_some() {
      0.0
    } else {
      fraction * 1.2
    };
    let eye = center
      + vec3(angle.sin() * distance, height, angle.cos() * distance)
      + if sky_world {
        vec3(fraction * 150.0, fraction * 60.0, 0.0)
      } else {
        Vec3::ZERO
      };
    if sky_inspection && frame == frames / 2 {
      assets.environment.select("castle", 1000)?;
    }
    assets.environment.update(1.0 / 30.0)?;
    clear_background(BLACK);
    let mut camera = Camera3D {
      position: eye,
      target: if sky_world {
        eye + vec3(0.0, 15.0, -90.0)
      } else {
        center
      },
      up: Vec3::Y,
      fovy: if sky_inspection || sky_timer || sky_world {
        crate::camera_view::FOV_DEGREES
      } else {
        50.0
      }
      .to_radians(),
      z_near: 0.1,
      z_far: 10000.0,
      ..Default::default()
    };
    set_camera(&camera);
    assets.environment.draw(eye);
    camera.z_far = crate::camera_view::FAR;
    set_camera(&camera);
    track_materials.update(frame as f32 / 30.0, |name, surface| {
      track.set_scene_surface(name, surface)
    })?;
    track.draw();
    gl_use_default_material();
    for object in &mut assets.scenery {
      object.update(1.0 / 30.0)?;
      object.draw(view);
    }
    for object in &mut assets.animated_scenery {
      object.update(1.0 / 30.0)?;
      object.draw([eye.x, -eye.z, eye.y], view);
    }
    for billboards in &assets.billboards {
      billboards.draw([eye.x, -eye.z, eye.y], [0.0, 0.0, 1.0], view);
    }
    set_default_camera();
    draw_rectangle(
      0.0,
      0.0,
      screen_width(),
      64.0,
      Color::new(0.0, 0.0, 0.0, 0.85),
    );
    draw_text(
      if sky_world {
        "ORIGINAL ROCKET RUN SKY WORLD / CAMERA-RELATIVE PLANET"
      } else if sky_timer {
        "ORIGINAL TIB CLOCK / NAMED SKY START AND STOP EVENTS"
      } else if sky_inspection {
        "ORIGINAL SKY SHELL / OPENAIR TO CASTLE PALETTES"
      } else {
        "ORIGINAL WORLD DETAILS / NATIVE ASSET INSPECTION"
      },
      20.0,
      27.0,
      22.0,
      WHITE,
    );
    draw_text(
      if sky_world {
        "Moving diagnostic camera / original geometry / not gameplay acceptance"
      } else if sky_timer {
        "Original event timing and RGB transitions / supplied noise stream / not gameplay parity"
      } else if sky_inspection {
        "Forced named transition / source geometry / not live trigger acceptance"
      } else {
        "Diagnostic camera — not gameplay or physical-input acceptance"
      },
      20.0,
      49.0,
      18.0,
      YELLOW,
    );
    if sky_timer
      && (frame == 0 || (assets.environment.name() == "flash" && assets.environment.settled()))
    {
      let phase = if frame == 0 { "ambient" } else { "flash" };
      let path = directory.join(format!("sky-timer-{phase}.png"));
      if !path.exists() {
        crate::capture::save_frame(&path, &crate::capture::screen_data().await)?;
      }
    }
    if frame == 0 || frame + 1 == frames {
      let phase = if frame == 0 {
        "front"
      } else if options.scenery_object.is_some() {
        "animated"
      } else {
        "oblique"
      };
      hashes.push(
        crate::capture::save_frame(
          &directory.join(format!("{target}-{phase}.png")),
          &crate::capture::screen_data().await,
        )?
        .0,
      );
    }
    if options.capture_clip {
      let image = crate::capture::screen_data().await;
      if video.is_none() {
        video = Some(crate::capture_video::Video::start(
          &directory.join("scenery-motion.mp4"),
          image.width,
          image.height,
        )?);
      }
      video.as_mut().unwrap().frame(&image)?;
    }
    next_frame().await;
  }
  let video_frames = video.map(|video| video.finish()).transpose()?;
  let report = serde_json::json!({"mode":"native_original_world_inspection_not_gameplay","target":target,"frames":frames,"billboards":assets.billboards.iter().map(|b|b.len()).sum::<usize>(),"animated_objects":assets.animated_scenery.iter().map(|o|o.name.as_str()).collect::<Vec<_>>(),"frame_hashes":hashes,"physical_input_verified":false,"named_sky_dispatches":assets.environment.dispatches(),"sky_profile":assets.environment.name(),"sky_children":assets.environment.child_names()});
  std::fs::write(
    directory.join("scenery.json"),
    serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
  )
  .map_err(|e| e.to_string())?;
  if let Some(samples) = video_frames {
    std::fs::write(directory.join("scenery-video.json"), serde_json::to_vec_pretty(&serde_json::json!({"fps":30,"samples":samples,"duration_seconds":samples as f32 / 30.0,"mode":"actual_native_framebuffer_silent_inspection_not_gameplay_acceptance"})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
  }
  Ok(())
}
