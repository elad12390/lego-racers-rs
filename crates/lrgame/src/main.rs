mod assets;
mod capture;
mod capture_video;
mod drive_assets;
mod environment;
mod driving;
mod gpu;
mod options;
mod preview;
mod skeleton;
mod skinned_gpu;
mod wheels;
mod race_audio;
mod race_music;
mod camera_rig;
mod camera_view;
mod driver_animation;
mod rival_gpu;
mod game_catalog;
mod profile;
mod menu_ui;
mod application;
mod powerup_gpu;
mod race_view;
mod engine_audio;
mod bitmap_text;
mod brick_model;
mod garage_builder;
mod race_hud;
mod time_trial;
mod racer_preview;
mod custom_driver;
mod driver_editor;
mod license_editor;
mod builder_palette;
mod cinematic_scene;
mod circuit_menu;
mod garage_roster;
mod progression;
mod track_preview;
mod track_menu;
mod menu_driver;
mod menu_camera;
mod circuit_award;
mod award_car;
mod cinematic_audio;
mod cinematic_overlay;
mod cinematic_material;
mod scene_material;
mod versus_race;
mod versus_selection;

use std::process::ExitCode;

use lrformats::model::Model;
use macroquad::prelude::*;

use options::Options;

fn main() -> ExitCode {
    let options = match Options::parse(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(error) => { eprintln!("{error}"); return ExitCode::FAILURE; }
    };
    let model = if options.play {None} else {match assets::load_track(&options) {
        Ok(model) => Some(model),
        Err(error) => { eprintln!("{error}"); return ExitCode::FAILURE; }
    }};
    macroquad::Window::from_config(Conf {
        window_title: if options.play {"LEGO Racers — Native".into()} else if options.drive { "LEGO Racers — diagnostic driving".into() }
            else { "LEGO Racers — original assets preview (not gameplay)".into() },
        window_width: 1000,
        window_height: 760,
        window_resizable: true,
        ..Default::default()
    }, async move {
        let result=if options.play {application::run(options).await} else {run(options,model.unwrap()).await};
        if let Err(error) = result {
            eprintln!("{error}");
            std::process::exit(1);
        }
    });
    ExitCode::SUCCESS
}

async fn run(options: Options, model: Model) -> Result<(), String> {
    let track = gpu::TrackGpu::upload(&model)?;
    if options.drive { return driving::run(options,track).await; }
    let mut camera = preview::PreviewCamera::default();
    let mut report = capture::Report {
        mode: "asset_preview_not_gameplay",
        native_frames_presented: 0,
        triangles: model.surfaces.iter().map(|surface| surface.triangles.len()).sum(),
        textures: model.images.len(),
        frame_hash_first: 0,
        frame_hash_last: 0,
        non_background_pixels_first: 0,
        non_background_pixels_last: 0,
        physical_keyboard_verified: false,
        rendering_discrepancies: vec!["diagnostic orbit camera, not original race camera",
            "unlit material/texture rendering; original lighting and vertex attributes not yet matched",
            "track model only; world instances, sky and animated scenery are not yet integrated"],
    };
    if let Some(directory) = &options.capture {
        std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    }
    loop {
        if is_key_pressed(KeyCode::Escape) || is_quit_requested() { break; }
        let dt = get_frame_time().min(0.1);
        let axis = |positive, negative| f32::from(is_key_down(positive)) - f32::from(is_key_down(negative));
        camera.apply(axis(KeyCode::Right, KeyCode::Left) * dt,
            axis(KeyCode::Up, KeyCode::Down) * dt,
            axis(KeyCode::S, KeyCode::W) * dt);
        if options.scripted_camera && report.native_frames_presented > 0 {
            // Exercise the same diagnostic action path, not fake OS key events.
            camera.apply(4.0_f32.to_radians(), -4.0_f32.to_radians(), 0.0);
        }
        clear_background(Color::from_rgba(24, 31, 45, 255));
        set_camera(&camera.native(&track));
        track.draw();
        set_default_camera();
        draw_rectangle(0.0, 0.0, screen_width(), 60.0, Color::new(0.0, 0.0, 0.0, 0.85));
        draw_text(&format!("LEGO RACERS / {} / ORIGINAL ASSET PREVIEW", options.table), 20.0, 27.0, 24.0, WHITE);
        draw_text("NOT GAMEPLAY — arrows: orbit/tilt | W/S: zoom | Esc: exit", 20.0, 49.0, 18.0, LIGHTGRAY);
        let last = options.frames == Some(report.native_frames_presented + 1);
        if let Some(directory) = &options.capture {
            if report.native_frames_presented == 0 || last {
                let image = capture::screen_data();
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
        if last { break; }
    }
    if let Some(directory) = &options.capture {
        capture::save_report(&directory.join("capture.json"), &report)?;
    }
    println!("native preview: {} GPU frames presented; not gameplay; physical keys not verified by scripted actions", report.native_frames_presented);
    Ok(())
}
