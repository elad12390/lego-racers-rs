//! Original PC license artwork, native name entry and actual framebuffer snapshot.
use crate::platform::prelude::*;
use crate::{
  custom_driver::Data,
  menu_driver::MenuDriver,
  menu_ui::MenuUi,
  profile::{Photo, Profile},
};
use lrformats::library::Library;
pub async fn run(
  library: &Library,
  profile: &mut Profile,
  ui: &MenuUi,
  capture: Option<&std::path::Path>,
) -> Result<bool, String> {
  let data = Data::load(library)?;
  let build = if let Some(build) = &profile.custom_driver {
    build.clone()
  } else {
    let catalog = crate::game_catalog::Catalog::load(library)?;
    let driver = catalog
      .drivers
      .iter()
      .find(|d| d.name.eq_ignore_ascii_case(&profile.driver))
      .ok_or("missing license driver")?;
    data.original_driver(driver)?
  };
  let rules = lrsim::brick_build::Rules::load()?;
  // SpinControl004659b0 chooses CAM.WDB's existing camera, not the MIB points.
  let camera = ui.license_camera(library)?;
  let viewport = ui.license_viewport()?;
  let placement = crate::gpu::instance_transform(&lrformats::world::Instance {
    model: String::new(),
    position: rules.license_preview_position,
    forward: rules.license_preview_forward,
    up: rules.license_preview_up,
  });
  let mut expression = profile.license_expression;
  let mut driver = MenuDriver::load_license_expression(library, &build, expression)?;
  let lighting = crate::garage_lighting::license(&rules);
  let mut name: String = profile
    .name
    .chars()
    .take(rules.license_name_capacity)
    .collect();
  let mut photo = profile.license_photo.clone();
  let mut texture = photo
    .as_ref()
    .map(|p| Texture2D::from_rgba8(p.width, p.height, &p.rgba));
  let mut frame = 0;
  let mut focus = crate::license_focus::Focus::new(rules.license_caret_period_ms);
  let mut pointer = crate::menu_pointer::Pointer::default();
  let mut keyboard = crate::menu_keyboard::Keyboard::default();
  loop {
    if is_quit_requested() || ui.cancellation_key(KeyCode::Escape) {
      return Ok(false);
    }
    use crate::license_focus::Control;
    let mouse = (Vec2::from(mouse_position()) - MenuUi::origin()) / MenuUi::scale();
    let mouse_event = mouse_delta_position().length_squared() > 0.0
      || is_mouse_button_pressed(MouseButton::Left)
      || is_mouse_button_released(MouseButton::Left);
    let hovered = if mouse_event {
      if MenuUi::editor_contains(ui.license_name_bounds()?, mouse) {
        Some(Control::Name)
      } else if MenuUi::editor_contains(
        ui.license_action_bounds("swapface", 0x3b, "nubutton")?,
        mouse,
      ) {
        Some(Control::Snapshot)
      } else if MenuUi::editor_contains(ui.license_action_bounds("gonext", 0xb, "buttonra")?, mouse)
      {
        Some(Control::BuildCar)
      } else if MenuUi::editor_contains(ui.license_action_bounds("goback", 9, "buttonla")?, mouse) {
        Some(Control::BuildDriver)
      } else {
        None
      }
    } else {
      None
    };
    let mut pointer_frame = pointer.update(
      match hovered {
        Some(Control::Snapshot) => Some("swapface"),
        Some(Control::BuildCar) => Some("gonext"),
        Some(Control::BuildDriver) => Some("goback"),
        _ => None,
      },
      is_mouse_button_pressed(MouseButton::Left),
      is_mouse_button_released(MouseButton::Left),
    );
    // ActivateLast_ChainItem00472bc0 captures movement on the pressed control;
    // OnMouseMove00472790 cannot transfer highlight until that capture ends.
    let focus_hover = match keyboard.captured().or(pointer_frame.active) {
      Some("swapface") => Some(Control::Snapshot),
      Some("gonext") => Some(Control::BuildCar),
      Some("goback") => Some(Control::BuildDriver),
      _ => hovered,
    };
    let previous_focus = focus.control;
    focus.update(focus_hover, (get_frame_time().max(0.0) * 1000.0) as u32);
    if focus.control != previous_focus {
      keyboard.cancel();
    }
    let reverse = if is_key_pressed(KeyCode::Tab) {
      Some(is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift))
    } else if is_key_pressed(KeyCode::Up) {
      Some(true)
    } else if is_key_pressed(KeyCode::Down) {
      Some(false)
    } else {
      None
    };
    if let Some(reverse) = reverse {
      focus.navigate(reverse);
      pointer.cancel();
      keyboard.cancel();
      pointer_frame = pointer.update(None, false, false);
      if let Some(audio) = &ui.audio {
        audio.moved();
      }
    }
    use crate::menu_keyboard::Key;
    let released = get_keys_released();
    let key = |enter, space| {
      if enter {
        Some(Key::Enter)
      } else if space {
        Some(Key::Space)
      } else {
        None
      }
    };
    let keyboard_frame = keyboard.update(
      match focus.control {
        Control::Name => None,
        Control::Snapshot => Some("swapface"),
        Control::BuildCar => Some("gonext"),
        Control::BuildDriver => Some("goback"),
      },
      key(
        is_key_pressed(KeyCode::Enter),
        is_key_pressed(KeyCode::Space),
      ),
      key(
        released.contains(&KeyCode::Enter),
        released.contains(&KeyCode::Space),
      ),
    );
    // Reuse the original named button's active tint and key-down effect, but
    // execute its operation only after its matching key release.
    pointer_frame = crate::menu_pointer::Frame {
      pressed: pointer_frame.pressed.or(keyboard_frame.pressed),
      active: pointer_frame.active.or(keyboard_frame.active),
      committed: pointer_frame.committed.or(keyboard_frame.committed),
    };
    while let Some(c) = get_char_pressed() {
      if focus.control == Control::Name
        && ui.license_name_character(c)?
        && name.chars().count() < rules.license_name_capacity
      {
        name.push(c.to_ascii_uppercase());
      }
    }
    if focus.control == Control::Name && is_key_pressed(KeyCode::Backspace) {
      name.pop();
    }
    if capture.is_some() && frame == 1 {
      name = "NATIVE RACER".into();
    }
    ui.original_background("Make License", false);
    let card = ui.layout_rect("drvrlice", "license")?;
    ui.original_image("license", card.x, card.y);
    if focus.control == Control::Name {
      ui.license_name_frame()?;
    }
    ui.license_name(&name)?;
    if focus.caret_visible(name.chars().count(), rules.license_name_capacity) {
      ui.license_caret(&name)?;
    }
    let snapshot = ui.license_action(
      "swapface",
      0x3b,
      "nubutton",
      focus.control == Control::Snapshot,
      pointer_frame,
    )? || (capture.is_some() && frame == 2)
      || ui.activation_key(KeyCode::F5);
    let build_car = ui.license_action(
      "gonext",
      0xb,
      "buttonra",
      focus.control == Control::BuildCar,
      pointer_frame,
    )? || (focus.control == Control::Name && ui.activation_key(KeyCode::Enter));
    if ui.license_action(
      "goback",
      9,
      "buttonla",
      focus.control == Control::BuildDriver,
      pointer_frame,
    )? {
      return Ok(false);
    }
    if snapshot {
      // CameraManAnimation::OnButton: increment the stored expression modulo
      // six and assign the original face suffix before producing the photograph.
      expression = ((usize::from(expression) + 1) % rules.license_expressions.len()) as u8;
      driver.set_license_expression(library, &build, expression)?;
      texture = None;
    }
    let rect = Rect::new(375.0, 82.0, 143.0, 139.0);
    let s = MenuUi::scale();
    let o = MenuUi::origin();
    // Stored native photographs stay still, but must not reset or stop the
    // screen-owned CMAMAN timeline when a new expression is photographed.
    driver.advance(if capture.is_some() {
      1.0 / 60.0
    } else {
      get_frame_time().min(0.1)
    })?;
    ui.license_preview_background()?;
    if let Some(texture) = &texture {
      draw_texture_ex(
        texture,
        o.x + rect.x * s,
        o.y + rect.y * s,
        WHITE,
        DrawTextureParams {
          dest_size: Some(vec2(rect.w * s, rect.h * s)),
          ..Default::default()
        },
      );
    } else {
      driver.set_scene_lighting(&lighting, placement);
      camera.draw_original(viewport, Vec3::ZERO, 0.0);
      driver.draw_at(placement);
      set_default_camera();
    }
    if snapshot {
      let image = crate::capture::screen_data().await;
      let width = 143u16;
      let height = 139u16;
      let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
      for y in 0..height {
        for x in 0..width {
          let sx = ((o.x + (rect.x + x as f32) * s) * image.width as f32 / screen_width()).floor()
            as usize;
          let sy = ((o.y + (rect.y + y as f32) * s) * image.height as f32 / screen_height()).floor()
            as usize;
          let start = ((image.height as usize - 1 - sy) * image.width as usize + sx) * 4;
          rgba.extend_from_slice(
            image
              .bytes
              .get(start..start + 4)
              .ok_or("license snapshot outside framebuffer")?,
          );
        }
      }
      photo = Some(Photo {
        width,
        height,
        rgba,
      });
      texture = photo
        .as_ref()
        .map(|p| Texture2D::from_rgba8(p.width, p.height, &p.rgba));
    }
    if let Some(dir) = capture {
      if frame == 0 {
        crate::capture::save_frame(
          &dir.join("license-preview.png"),
          &crate::capture::screen_data().await,
        )?;
      }
      if frame == 4 {
        crate::capture::save_frame(
          &dir.join("license.png"),
          &crate::capture::screen_data().await,
        )?;
      }
    }
    if build_car || (capture.is_some() && frame == 5) {
      if name.trim().is_empty() {
        name = "RACER".into();
      }
      profile.name = name;
      profile.license_photo = photo;
      profile.license_expression = expression;
      return Ok(true);
    }
    next_frame().await;
    frame += 1;
  }
}
