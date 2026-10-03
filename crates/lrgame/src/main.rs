mod animated_scenery;
mod application;
mod assets;
mod audio_assets;
#[cfg(test)]
mod audio_handoff_test;
mod award_car;
mod bitmap_text;
mod brick_model;
mod builder_palette;
mod camera_check;
mod camera_rig;
mod camera_view;
mod capture;
mod capture_video;
mod cinematic_audio;
mod cinematic_material;
mod cinematic_overlay;
mod cinematic_scene;
mod circuit_award;
mod circuit_menu;
mod custom_driver;
mod drive_assets;
#[cfg(test)]
mod driver_activation_test;
mod driver_animation;
#[cfg(test)]
mod driver_capture_focus_test;
#[cfg(test)]
mod driver_controls_test;
mod driver_discard;
#[cfg(test)]
mod driver_discard_test;
mod driver_editor;
mod driver_focus;
#[cfg(test)]
mod driver_heads_parser_test;
#[cfg(test)]
mod driver_hover_test;
#[cfg(test)]
mod driver_motion_test;
#[cfg(test)]
mod driver_multirow_test;
#[cfg(test)]
mod driver_navigation_test;
#[cfg(test)]
mod driver_new_cancel_test;
#[cfg(test)]
mod driver_pointer_test;
#[cfg(test)]
mod driver_row_focus_test;
mod driver_row_input;
#[cfg(test)]
mod driver_row_test;
mod driver_scroll;
#[cfg(test)]
mod driver_scroll_test;
mod driver_thumbnail;
#[cfg(test)]
mod driver_thumbnail_test;
#[cfg(test)]
mod driver_wrap_test;
mod driving;
#[cfg(test)]
mod editor_audio_test;
#[cfg(test)]
mod editor_gpu_test;
mod engine_audio;
mod environment;
mod game_catalog;
mod garage_builder;
mod garage_edit;
mod garage_lighting;
mod garage_roster;
mod gpu;
mod input_gate;
#[cfg(test)]
mod license_activation_test;
mod license_editor;
mod license_focus;
#[cfg(test)]
mod license_navigation_test;
#[cfg(test)]
mod license_snapshot_test;
mod menu_audio;
mod menu_camera;
mod menu_driver;
mod menu_keyboard;
mod menu_pointer;
mod menu_ui;
mod native_window;
mod options;
mod platform;
#[cfg(test)]
mod playable_loop_test;
mod powerup_animation;
mod powerup_gpu;
mod powerup_preview;
mod preview;
mod profile;
mod progression;
mod race_audio;
mod race_hud;
mod race_intro;
mod race_music;
mod race_view;
mod racer_preview;
mod rival_gpu;
mod scene_material;
mod scenery_preview;
mod skeleton;
mod skinned_gpu;
mod static_scenery;
mod time_trial;
mod track_menu;
mod track_preview;
mod versus_race;
mod versus_selection;
mod wheels;
mod world_billboards;
mod world_material;

use std::process::ExitCode;

use crate::platform::prelude::*;
use lrformats::model::Model;

use options::Options;

fn main() -> ExitCode {
  let options = match Options::parse(std::env::args().skip(1)) {
    Ok(options) => options,
    Err(error) => {
      eprintln!("{error}");
      return ExitCode::FAILURE;
    }
  };
  let model = if options.play {
    None
  } else {
    match assets::load_track(&options) {
      Ok(model) => Some(model),
      Err(error) => {
        eprintln!("{error}");
        return ExitCode::FAILURE;
      }
    }
  };
  let title = if options.play {
    "LEGO Racers — Bevy".into()
  } else if options.drive {
    "LEGO Racers — diagnostic driving".into()
  } else {
    "LEGO Racers — original assets preview (not gameplay)".into()
  };
  // Frame-by-frame diagnostic recordings use the same Bevy render image but
  // must not show a deliberately readback-paced window as live gameplay.
  let show_window = !options.capture_clip;
  platform::run(title, show_window, async move {
    let result = if options.play {
      application::run(options).await
    } else {
      run(options, model.unwrap()).await
    };
    result
  })
}

async fn run(options: Options, model: Model) -> Result<(), String> {
  let track = gpu::TrackGpu::upload(&model)?;
  if options.drive {
    return driving::run(options, track).await;
  }
  if options.scenery_smoke {
    return scenery_preview::run(&options, track).await;
  }
  let mut camera = preview::PreviewCamera::default();
  let mut report = capture::Report {
    mode: "asset_preview_not_gameplay",
    native_frames_presented: 0,
    triangles: model
      .surfaces
      .iter()
      .map(|surface| surface.triangles.len())
      .sum(),
    textures: model.images.len(),
    frame_hash_first: 0,
    frame_hash_last: 0,
    non_background_pixels_first: 0,
    non_background_pixels_last: 0,
    physical_keyboard_verified: false,
    rendering_discrepancies: vec![
      "diagnostic orbit camera, not original race camera",
      "unlit material/texture rendering; original lighting and vertex attributes not yet matched",
      "track model only; world instances, sky and animated scenery are not yet integrated",
    ],
  };
  if let Some(directory) = &options.capture {
    std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
  }
  loop {
    if is_key_pressed(KeyCode::Escape) || is_quit_requested() {
      break;
    }
    let dt = get_frame_time().min(0.1);
    let axis =
      |positive, negative| f32::from(is_key_down(positive)) - f32::from(is_key_down(negative));
    camera.apply(
      axis(KeyCode::Right, KeyCode::Left) * dt,
      axis(KeyCode::Up, KeyCode::Down) * dt,
      axis(KeyCode::S, KeyCode::W) * dt,
    );
    if options.scripted_camera && report.native_frames_presented > 0 {
      // Exercise the same diagnostic action path, not fake OS key events.
      camera.apply(4.0_f32.to_radians(), -4.0_f32.to_radians(), 0.0);
    }
    clear_background(Color::from_rgba(24, 31, 45, 255));
    set_camera(&camera.native(&track));
    track.draw();
    set_default_camera();
    draw_rectangle(
      0.0,
      0.0,
      screen_width(),
      60.0,
      Color::new(0.0, 0.0, 0.0, 0.85),
    );
    draw_text(
      &format!("LEGO RACERS / {} / ORIGINAL ASSET PREVIEW", options.table),
      20.0,
      27.0,
      24.0,
      WHITE,
    );
    draw_text(
      "NOT GAMEPLAY — arrows: orbit/tilt | W/S: zoom | Esc: exit",
      20.0,
      49.0,
      18.0,
      LIGHTGRAY,
    );
    let last = options.frames == Some(report.native_frames_presented + 1);
    if let Some(directory) = &options.capture {
      if report.native_frames_presented == 0 || last {
        let image = capture::screen_data().await;
        if report.native_frames_presented == 0 {
          let (hash, populated) = capture::save_frame(&directory.join("first.png"), &image)?;
          report.frame_hash_first = hash;
          report.non_background_pixels_first = populated;
        }
        if last {
          let (hash, populated) = capture::save_frame(&directory.join("last.png"), &image)?;
          report.frame_hash_last = hash;
          report.non_background_pixels_last = populated;
        }
      }
    }
    next_frame().await;
    report.native_frames_presented += 1;
    if last {
      break;
    }
  }
  if let Some(directory) = &options.capture {
    capture::save_report(&directory.join("capture.json"), &report)?;
  }
  println!("native preview: {} GPU frames presented; not gameplay; physical keys not verified by scripted actions", report.native_frames_presented);
  Ok(())
}
