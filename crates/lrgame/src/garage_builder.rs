//! Native brick-by-brick editor over the original PIECEDB geometry.
//! Widgets and placement validation remain provisional until original parity.
use crate::platform::prelude::*;
use crate::{game_catalog::Catalog, gpu::TrackGpu, menu_ui::MenuUi, profile::Profile};
use lrformats::library::Library;
use lrsim::brick_build::{Build, BuilderData, PlacedBrick};

pub async fn run(
  library: &Library,
  catalog: &Catalog,
  profile: &Profile,
  ui: &mut MenuUi,
  smoke: Option<&std::path::Path>,
) -> Result<Option<(Build, String)>, String> {
  let data = BuilderData::load(library)?;
  let lighting = crate::garage_lighting::from_rules(&data.rules);
  let mut build = profile
    .custom_build
    .clone()
    .unwrap_or_else(|| data.default_build());
  data.validate(&build)?;
  let chassis_choices = catalog
    .cars
    .iter()
    .filter(|c| data.database.find(&c.chassis).is_ok())
    .collect::<Vec<_>>();
  let mut chassis_index = chassis_choices
    .iter()
    .position(|c| c.chassis.eq_ignore_ascii_case(&build.chassis))
    .ok_or("saved custom chassis not in original car catalog")?;
  let parts = data
    .database
    .bricks
    .iter()
    .filter(|b| {
      b.id >= 0x800 && b.name.to_ascii_lowercase().starts_with('l') && b.width > 0 && b.depth > 0
    })
    .collect::<Vec<_>>();
  if parts.is_empty() {
    return Err("original brick catalog is empty".into());
  }
  let mut selected = parts
    .iter()
    .position(|b| b.name.eq_ignore_ascii_case("l300300"))
    .unwrap_or(0);
  let mut color = 5usize;
  let mut candidate = PlacedBrick {
    name: parts[selected].name.clone(),
    color: data.rules.colors[color].clone(),
    x: 1,
    y: 1,
    z: 3,
    rotation: 0,
  };
  let mut message = String::new();
  let mut gpu = TrackGpu::upload(&crate::brick_model::model(library, &data, &build)?)?;
  let mut revision = true;
  let mut frame = 0u32;
  let room =
    lrformats::model::Model::load(library, "garage", Some("GARAGE")).map_err(|e| e.to_string())?;
  let room = TrackGpu::upload(&room)?;
  let world = library
    .find_at("GARAGE.WDB", "MENUDATA", "GARAGE")
    .ok_or("missing original builder room")?;
  let camera = crate::menu_camera::MenuCamera::load(world)?;
  let instances = lrformats::world::instances(world).map_err(|e| e.to_string())?;
  let instance = instances.first().ok_or("empty builder room")?;
  let up = crate::gpu::world_position(instance.up, 1.0);
  let forward = crate::gpu::world_position(instance.forward, 1.0);
  let room_transform = Mat4::from_cols(
    forward.extend(0.0),
    up.extend(0.0),
    forward.cross(up).extend(0.0),
    crate::gpu::world_position(instance.position, 1.0).extend(1.0),
  );
  let mut ghost = None;
  let mut last_candidate = None;
  let mut wheel_name = String::new();
  let mut wheels = None;
  let chassis_data = lrformats::cmb::parse(
    library
      .find_in("CHASSIS.CMB", "COMMON")
      .ok_or("missing builder chassis")?,
  )
  .map_err(|e| e.to_string())?;
  let appearance = catalog
    .drivers
    .iter()
    .find(|d| d.name.eq_ignore_ascii_case(&profile.driver))
    .ok_or("missing builder driver")?;
  let driver_model = if let Some(build) = &profile.custom_driver {
    crate::custom_driver::Data::load(library)?.model(library, build)?
  } else {
    lrformats::model::Model::load(library, &appearance.models[0], Some("COMMON"))
      .map_err(|e| e.to_string())?
  };
  let mut driver = crate::driver_animation::DriverAnimation::load(library, &driver_model)?;
  driver.set_scene_lighting(&lighting, Mat4::IDENTITY);
  let mut palette = crate::builder_palette::Palette::new();
  let mut camera_rotation = 0.0f32;
  loop {
    if is_quit_requested() || ui.cancellation_key(KeyCode::Escape) {
      return Ok(None);
    }
    ui.original_background("", false);
    ui.original_image("mta", 51.0, 40.0);
    let rotate = ui.icon_button("rotatea", 60.0, 160.0);
    let attach = ui.icon_button("downa", 60.0, 211.0);
    let remove = ui.icon_button("upa", 60.0, 253.0);
    if ui.icon_button("cameraa", 51.0, 301.0) {
      camera_rotation += std::f32::consts::FRAC_PI_2;
    }
    let mut save = ui.icon_button("exita", 35.0, 408.0) || ui.activation_key(KeyCode::S);
    if ui.navigation_key(KeyCode::N) {
      chassis_index = (chassis_index + 1) % chassis_choices.len();
      build = Build {
        chassis: chassis_choices[chassis_index].chassis.clone(),
        color: build.color.clone(),
        bricks: Vec::new(),
      };
      revision = true;
    }
    if ui.navigation_key(KeyCode::V) {
      let index = data
        .rules
        .colors
        .iter()
        .position(|c| c == &build.color)
        .unwrap_or(0);
      build.color = data.rules.colors[(index + 1) % data.rules.colors.len()].clone();
      revision = true;
    }
    if ui.navigation_key(KeyCode::Tab) {
      selected = (selected
        + if is_key_down(KeyCode::LeftShift) {
          parts.len() - 1
        } else {
          1
        })
        % parts.len();
    }
    if ui.navigation_key(KeyCode::C) {
      color = (color + 1) % data.rules.colors.len();
    }
    if ui.navigation_key(KeyCode::R) || rotate {
      candidate.rotation = (candidate.rotation + 1) % 4;
    }
    let base = data.database.find(&build.chassis)?;
    if ui.navigation_key(KeyCode::Left) {
      candidate.x = candidate.x.saturating_sub(1);
    }
    if ui.navigation_key(KeyCode::Right) {
      candidate.x = (candidate.x + 1).min(base.width - 1);
    }
    if ui.navigation_key(KeyCode::Up) {
      candidate.y = candidate.y.saturating_sub(1);
    }
    if ui.navigation_key(KeyCode::Down) {
      candidate.y = (candidate.y + 1).min(base.depth - 1);
    }
    let mouse = (Vec2::from(mouse_position()) - MenuUi::origin()) / MenuUi::scale();
    if is_mouse_button_pressed(MouseButton::Left)
      && Rect::new(51.0, 40.0, 120.0, 112.0).contains(mouse)
    {
      let delta = mouse - vec2(111.0, 96.0);
      if delta.x.abs() > 18.0 || delta.y.abs() > 18.0 {
        if let Some(audio) = &ui.audio {
          audio.moved();
        }
      }
      if delta.x < -18.0 {
        candidate.x = candidate.x.saturating_sub(1);
      }
      if delta.x > 18.0 {
        candidate.x = (candidate.x + 1).min(base.width - 1);
      }
      if delta.y < -18.0 {
        candidate.y = candidate.y.saturating_sub(1);
      }
      if delta.y > 18.0 {
        candidate.y = (candidate.y + 1).min(base.depth - 1);
      }
    }
    if ui.navigation_key(KeyCode::PageUp) {
      candidate.z = (candidate.z + 1).min(data.rules.max_height);
    }
    if ui.navigation_key(KeyCode::PageDown) {
      candidate.z = candidate.z.saturating_sub(1);
    }
    candidate.name = parts[selected].name.clone();
    candidate.color = data.rules.colors[color].clone();
    let change = palette.draw(library, &data, &build, &candidate, &mut color, ui)?;
    if change != 0 {
      selected = (selected as isize + change as isize).rem_euclid(parts.len() as isize) as usize;
      candidate.name = parts[selected].name.clone();
    }
    candidate.color = data.rules.colors[color].clone();
    let mut trial = build.clone();
    let validity = data.add(&mut trial, candidate.clone());
    if last_candidate.as_ref() != Some(&candidate) || revision {
      let mut preview = TrackGpu::upload(&crate::brick_model::candidate(
        library, &data, &build, &candidate,
      )?)?;
      preview.set_scene_lighting(&lighting, Mat4::IDENTITY);
      preview.tint(Color::new(1.0, 1.0, 1.0, 0.6));
      ghost = Some(preview);
      last_candidate = Some(candidate.clone());
    }
    if ui.activation_key(KeyCode::Enter) || attach {
      match validity.as_ref() {
        Ok(()) => {
          build = trial.clone();
          revision = true;
          message = "Brick attached".into();
        }
        Err(error) => message = error.clone(),
      }
    }
    if ui.activation_key(KeyCode::Backspace) || remove {
      let index = build
        .bricks
        .iter()
        .rposition(|p| {
          data.cells(p).is_ok_and(|cells| {
            cells
              .iter()
              .any(|c| c.x == candidate.x && c.y == candidate.y)
          })
        })
        .or_else(|| build.bricks.len().checked_sub(1));
      if let Some(index) = index {
        let mut trial = build.clone();
        trial.bricks.remove(index);
        match data.validate(&trial) {
          Ok(()) => {
            build = trial;
            revision = true;
            message = "Brick removed".into();
          }
          Err(_) => message = "Remove the supported bricks first".into(),
        }
      }
    }
    if smoke.is_some() && frame == 4 {
      let mut added = 0;
      'outer: for x in 0..base.width {
        for y in 0..base.depth {
          for z in 0..data.rules.max_height {
            let part = PlacedBrick {
              x,
              y,
              z,
              ..candidate.clone()
            };
            if data.add(&mut build, part).is_ok() {
              added += 1;
              if added == 2 {
                break 'outer;
              }
            }
          }
        }
      }
      if added < 2 {
        return Err("builder smoke could not attach two original bricks".into());
      }
      let removed = build.bricks.pop().ok_or("missing builder smoke part")?;
      data.validate(&build)?;
      data.add(&mut build, removed)?;
      candidate = build.bricks.last().unwrap().clone();
      revision = true;
      message = "Two original bricks attached; remove/re-add verified".into();
    }
    if revision {
      gpu = TrackGpu::upload(&crate::brick_model::model(library, &data, &build)?)?;
      gpu.set_scene_lighting(&lighting, Mat4::IDENTITY);
      revision = false;
    }
    ui.preview_frame(223.0, 129.0, 390.0, 324.0);
    let rotation = camera_rotation
      + if is_key_down(KeyCode::Q) {
        get_time() as f32 * 0.5
      } else {
        0.0
      };
    camera.draw(Rect::new(226.0, 132.0, 384.0, 318.0), Vec3::ZERO, rotation);
    room.draw_at(room_transform);
    gpu.draw();
    if let Some(ghost) = &ghost {
      gl_use_material(&Material::scene(Some([6, 8])));
      ghost.draw();
      gl_use_default_material();
    }
    let chassis = chassis_data
      .iter()
      .find(|c| c.name.eq_ignore_ascii_case(&build.chassis))
      .ok_or("missing builder wheel chassis")?;
    let name = &chassis
      .wheel_models
      .first()
      .ok_or("missing builder wheel model")?
      .1;
    if wheel_name != *name {
      let mut loaded = crate::wheels::Wheels::load(library, name)?;
      loaded.set_scene_lighting(&lighting, Mat4::IDENTITY);
      wheels = Some(loaded);
      wheel_name = name.clone();
    }
    if let Some(wheels) = &wheels {
      wheels.draw_at(Mat4::IDENTITY);
    }
    driver.draw_at(Mat4::from_translation(crate::gpu::world_position(
      chassis.size,
      1.0,
    )));
    set_default_camera();
    ui.text(
      validity
        .as_ref()
        .err()
        .map(String::as_str)
        .unwrap_or("Enter to attach brick"),
      224.0,
      472.0,
      14.0,
      if validity.is_ok() { WHITE } else { YELLOW },
    );
    if !message.is_empty() {
      ui.text(&message, 16.0, 397.0, 11.0, YELLOW);
    }
    if let Some(directory) = smoke {
      if frame == 2 {
        crate::capture::save_frame(
          &directory.join("builder-chassis.png"),
          &crate::capture::screen_data().await,
        )?;
      }
      if frame == 8 {
        crate::capture::save_frame(
          &directory.join("builder-bricks.png"),
          &crate::capture::screen_data().await,
        )?;
        save = true;
      }
    }
    next_frame().await;
    frame += 1;
    if save {
      data.validate(&build)?;
      return Ok(Some((build, chassis_choices[chassis_index].name.clone())));
    }
  }
}
