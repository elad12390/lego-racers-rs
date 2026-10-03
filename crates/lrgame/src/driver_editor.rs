//! Native PC build-driver screen using the original part catalog and meshes.
use crate::platform::prelude::*;
use crate::{
  custom_driver::{Build, Data},
  gpu::TrackGpu,
  menu_driver::MenuDriver,
  menu_ui::MenuUi,
};
use lrformats::library::Library;

pub async fn run(
  library: &Library,
  profile: &crate::profile::Profile,
  catalog: &crate::game_catalog::Catalog,
  ui: &MenuUi,
  capture: Option<&std::path::Path>,
) -> Result<Option<Build>, String> {
  run_with_creation(library, profile, catalog, ui, capture, false).await
}

/// Creation is supplied by the owning garage transaction, never inferred from
/// driver parts, photograph presence or persisted customization data.
pub async fn run_with_creation(
  library: &Library,
  profile: &crate::profile::Profile,
  catalog: &crate::game_catalog::Catalog,
  ui: &MenuUi,
  capture: Option<&std::path::Path>,
  creating: bool,
) -> Result<Option<Build>, String> {
  let data = Data::load(library)?;
  let lighting = crate::garage_lighting::driver(&lrsim::brick_build::Rules::load()?);
  let mut build = profile.custom_driver.clone().unwrap_or_default();
  let original = build.clone();
  let mut discard: Option<crate::driver_discard::Dialog> = None;
  data.validate(&build)?;
  // Screen_ApplyStoredSelection0047d840 highlights part_panels[0] without SFX.
  let mut focus = crate::driver_focus::Control::Hat;
  let mut rotation = 0.0f32;
  let mut frame = 0;
  let mut rendered = None;
  let mut previous: Option<Build> = None;
  let mut fractional_ms = 0.0f64;
  let mut pointer = crate::menu_pointer::Pointer::default();
  let mut keyboard = crate::menu_keyboard::Keyboard::default();
  let mut row_input = [crate::driver_row_input::DriverRowInput::default(); 4];
  const MOUSE_EVENT: u32 = 0x20000000;
  const LEFT_EVENT: u32 = 0x100000cb;
  const RIGHT_EVENT: u32 = 0x100000cd;
  let choices = (0..4)
    .map(|row| {
      data
        .row(row)
        .iter()
        .enumerate()
        .filter(|(i, part)| {
          profile.driver_part_allowed(part.unlock, catalog) || *i == *slot(&mut build, row)
        })
        .map(|(i, _)| i)
        .collect::<Vec<_>>()
    })
    .collect::<Vec<_>>();
  let mut rows = (0..4)
    .map(|r| {
      crate::driver_thumbnail::row(
        &data,
        library,
        ui,
        &choices[r],
        r,
        *slot(&mut build, r),
        &lighting,
      )
    })
    .collect::<Result<Vec<_>, _>>()?;
  let stage_world = library
    .find_at("CBSET.WDB", "MENUDATA", "CB_SET")
    .ok_or("missing original driver stage")?;
  let camera = crate::menu_camera::MenuCamera::load(stage_world)?;
  let viewport = ui.driver_viewport()?;
  let stage_origin = crate::gpu::world_position(
    lrformats::world::instances(stage_world)
      .map_err(|e| e.to_string())?
      .first()
      .ok_or("driver stage has no original stand")?
      .position,
    1.0,
  );
  // The original stand's top is below the minifigure's pelvis origin. Only
  // recenter X/Y; raising the stand to the pelvis hides the entire lower legs.
  let stage_offset = vec3(stage_origin.x, 0.0, stage_origin.z);
  let bindings = lrformats::scene::SceneBindings::load(library, "CB_SET", stage_world)?;
  let stand = TrackGpu::upload(
    &lrformats::model::Model::load_with_materials(
      library,
      "STAND",
      Some("CB_SET"),
      &bindings.materials,
    )
    .map_err(|e| e.to_string())?,
  )?;
  loop {
    // Preserve sub-millisecond frame time before computing integer UI ticks.
    fractional_ms += f64::from(get_frame_time()) * 1000.0;
    let elapsed_ms = fractional_ms as u32;
    fractional_ms -= f64::from(elapsed_ms);
    if is_quit_requested() || ui.cancellation_key(KeyCode::Escape) {
      return Ok(None);
    }
    let mouse = (Vec2::from(mouse_position()) - MenuUi::origin()) / MenuUi::scale();
    let previous_focus = focus;
    let mut hovered_action = None;
    for (widget, label, style) in [
      ("mix", 0x38, "nubutton"),
      ("gonext", 10, "buttonra"),
      ("goback", 31, "buttonca"),
    ] {
      if MenuUi::editor_contains(ui.driver_action_bounds(widget, label, style)?, mouse) {
        hovered_action = Some(widget);
      }
    }
    let mut pointer_frame = if discard.is_some() {
      pointer.update(None, false, false)
    } else {
      pointer.update(
        hovered_action,
        is_mouse_button_pressed(MouseButton::Left),
        is_mouse_button_released(MouseButton::Left),
      )
    };
    if pointer_frame.active.is_none()
      && discard.is_none()
      && keyboard.captured().is_none()
      && !row_input.iter().any(|input| input.frame_arrow().is_some())
      && (mouse_delta_position().length_squared() > 0.0
        || is_mouse_button_pressed(MouseButton::Left))
    {
      if let Some(control) = hovered_action.and_then(crate::driver_focus::Control::from_widget) {
        focus = control;
      } else {
        // Original outer motion router 004691e0 visits hit panels only when
        // root capture is empty; crossing rows never cancels a captured hold.
        for row in 0..4 {
          if MenuUi::editor_contains(ui.driver_row_bounds(row)?, mouse) {
            focus = crate::driver_focus::Control::from_row(row);
            break;
          }
        }
      }
    }
    let reverse = if discard.is_some() {
      None
    } else if is_key_pressed(KeyCode::Tab) {
      Some(is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift))
    } else if is_key_pressed(KeyCode::Up) {
      Some(true)
    } else if is_key_pressed(KeyCode::Down) {
      Some(false)
    } else {
      None
    };
    if let Some(reverse) = reverse {
      focus = focus.navigate(reverse);
      pointer.cancel();
      keyboard.cancel();
      for input in &mut row_input {
        input.cancel();
      }
      pointer_frame = pointer.update(None, false, false);
      if let Some(audio) = &ui.audio {
        audio.moved();
      }
    }
    if let Some(control) = pointer_frame
      .active
      .and_then(crate::driver_focus::Control::from_widget)
    {
      focus = control;
    }
    let mut scrolled = [false; 4];
    let released = get_keys_released();
    if previous_focus != focus {
      for input in &mut row_input {
        input.cancel();
      }
    }
    for r in 0..4 {
      use crate::driver_row_input::Arrow;
      if is_mouse_button_released(MouseButton::Left) {
        row_input[r].release(MOUSE_EVENT);
      }
      if released.contains(&KeyCode::Left) {
        row_input[r].release(LEFT_EVENT);
      }
      if released.contains(&KeyCode::Right) {
        row_input[r].release(RIGHT_EVENT);
      }
      if discard.is_none() && focus.row() == Some(r) {
        if is_key_pressed(KeyCode::Left) {
          row_input[r].arrow_event(LEFT_EVENT, Arrow::Previous, false);
        }
        if is_key_pressed(KeyCode::Right) {
          row_input[r].arrow_event(RIGHT_EVENT, Arrow::Next, false);
        }
      }
      if discard.is_none() && is_mouse_button_pressed(MouseButton::Left) {
        let arrow = if MenuUi::editor_contains(ui.driver_row_arrow_bounds(r, false)?, mouse) {
          Some(Arrow::Previous)
        } else if MenuUi::editor_contains(ui.driver_row_arrow_bounds(r, true)?, mouse) {
          Some(Arrow::Next)
        } else {
          None
        };
        if let Some(arrow) = arrow {
          if focus.row() != Some(r) {
            for input in &mut row_input {
              input.cancel();
            }
          }
          row_input[r].arrow_event(MOUSE_EVENT, arrow, false);
          focus = crate::driver_focus::Control::from_row(r);
        } else {
          let (selector, viewport) = ui.driver_selector(r)?;
          if MenuUi::editor_contains(viewport, mouse) {
            let local = mouse - vec2(viewport.x, viewport.y);
            if let Some(authored_slot) = selector.slots.iter().position(|s| {
              MenuUi::editor_contains(
                Rect::new(
                  s[0] as f32,
                  s[1] as f32,
                  (s[2] - s[0]) as f32,
                  (s[3] - s[1]) as f32,
                ),
                local,
              )
            }) {
              if focus.row() != Some(r) {
                for input in &mut row_input {
                  input.cancel();
                }
              }
              let index = slot(&mut build, r);
              let selected = choices[r].iter().position(|i| i == index).unwrap();
              let selection = crate::driver_row_input::thumbnail_selection(
                selected,
                selector.selected_slot,
                authored_slot,
                choices[r].len(),
                rows[r].busy(),
              )
              .unwrap();
              *index = choices[r][selection.selected];
              focus = crate::driver_focus::Control::from_row(r);
              if selection.rebuild_now {
                rows[r] = crate::driver_thumbnail::row(
                  &data,
                  library,
                  ui,
                  &choices[r],
                  r,
                  *index,
                  &lighting,
                )?;
                scrolled[r] = true;
              }
            }
          }
        }
      }
    }
    // Dispatch explicit pointer focus changes before any panel's repeat tick;
    // a held key must not move the former row after a new row was highlighted.
    for r in 0..4 {
      use crate::driver_row_input::Arrow;
      let held = row_input[r].frame_arrow();
      let direction = if held == Some(Arrow::Previous) {
        -1
      } else if held == Some(Arrow::Next) {
        1
      } else {
        0
      };
      if direction != 0 && !rows[r].busy() {
        if let Some(audio) = &ui.audio {
          audio.moved();
        }
        focus = crate::driver_focus::Control::from_row(r);
        let index = slot(&mut build, r);
        let current = choices[r].iter().position(|v| v == index).unwrap_or(0);
        *index =
          choices[r][(current as isize + direction).rem_euclid(choices[r].len() as isize) as usize];
        let (selector, viewport) = ui.driver_selector(r)?;
        let edge = if direction > 0 {
          selector.slots.len() - 1
        } else {
          0
        };
        let offset = edge as isize - selector.selected_slot as isize;
        let incoming_index = choices[r]
          [(current as isize + direction + offset).rem_euclid(choices[r].len() as isize) as usize];
        let incoming = crate::driver_thumbnail::load(&data, library, r, incoming_index, &lighting)?;
        rows[r].shift(direction, incoming, selector, viewport);
        scrolled[r] = true;
      }
    }
    if capture.is_some() && frame == 3 {
      build = Build {
        hat: 15,
        face: 22,
        torso: 22,
        legs: 11,
      };
    } // actual unlocked hair / face / torso / red legs
    if previous.as_ref() != Some(&build) {
      let character = MenuDriver::load(library, &build)?;
      for r in 0..4 {
        let changed = previous.as_ref().is_some_and(|old| {
          let mut old = old.clone();
          *slot(&mut old, r) != *slot(&mut build, r)
        });
        if changed && !scrolled[r] && !rows[r].busy() {
          rows[r] = crate::driver_thumbnail::row(
            &data,
            library,
            ui,
            &choices[r],
            r,
            *slot(&mut build, r),
            &lighting,
          )?;
        }
      }
      rendered = Some(character);
      previous = Some(build.clone());
    }
    if previous_focus != focus {
      keyboard.cancel();
    }
    use crate::menu_keyboard::Key;
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
      if discard.is_none() {
        focus.widget()
      } else {
        None
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
    pointer_frame = crate::menu_pointer::Frame {
      pressed: pointer_frame.pressed.or(keyboard_frame.pressed),
      active: pointer_frame.active.or(keyboard_frame.active),
      committed: pointer_frame.committed.or(keyboard_frame.committed),
    };
    ui.original_background("Build Driver", false);
    ui.driver_preview_frame()?;
    for r in 0..4 {
      let left = ui.driver_row_arrow_bounds(r, false)?;
      let right = ui.driver_row_arrow_bounds(r, true)?;
      ui.original_image(
        if focus.row() == Some(r) {
          "arrowls"
        } else {
          "arrowlu"
        },
        left.x,
        left.y,
      );
      ui.original_image(
        if focus.row() == Some(r) {
          "arrowrs"
        } else {
          "arrowru"
        },
        right.x,
        right.y,
      );
    }
    if discard.is_none() && is_mouse_button_down(MouseButton::Left) && viewport.contains(mouse) {
      rotation += mouse_delta_position().x / MenuUi::scale() * 0.01;
    }
    if discard.is_none() && is_key_down(KeyCode::A) {
      rotation -= get_frame_time();
    }
    if discard.is_none() && is_key_down(KeyCode::D) {
      rotation += get_frame_time();
    }
    if let Some(character) = &mut rendered {
      character.advance(if capture.is_some() {
        1.0 / 60.0
      } else {
        get_frame_time().min(0.1)
      })?;
      let character_transform = Mat4::from_rotation_y(rotation);
      character.set_scene_lighting(&lighting, character_transform);
      for r in 0..4 {
        let (selector, viewport) = ui.driver_selector(r)?;
        // One shared clipped row viewport, not five independent mini-cameras.
        if !scrolled[r] {
          rows[r].advance(elapsed_ms);
        }
        crate::driver_thumbnail::draw(
          selector,
          viewport,
          rows[r].items.iter().filter_map(Option::as_ref),
        );
      }
      camera.draw(viewport, stage_offset, 0.0);
      stand.draw_at(Mat4::from_translation(vec3(0.0, stage_origin.y, 0.0)));
      character.draw_at(character_transform);
      set_default_camera();
    }
    if ui.driver_action(
      "mix",
      0x38,
      "nubutton",
      pointer_frame,
      focus.widget() == Some("mix"),
    )? {
      for r in 0..4 {
        let count = choices[r].len();
        *slot(&mut build, r) = choices[r][rand::gen_range(0, count)];
      }
    }
    let done =
      ui.driver_action(
        "gonext",
        10,
        "buttonra",
        pointer_frame,
        focus.widget() == Some("gonext"),
      )? || (discard.is_none() && focus.row().is_some() && ui.activation_key(KeyCode::Enter));
    if ui.driver_action(
      "goback",
      31,
      "buttonca",
      pointer_frame,
      focus.widget() == Some("goback"),
    )? {
      if !creating && build == original {
        return Ok(None);
      }
      discard = Some(if creating {
        crate::driver_discard::Dialog::new_racer()
      } else {
        crate::driver_discard::Dialog::default()
      });
      pointer.cancel();
      keyboard.cancel();
      for input in &mut row_input {
        input.cancel();
      }
      // The release that opened the dialog cannot activate its default button.
      next_frame().await;
      continue;
    }
    if let Some(dialog) = &mut discard {
      let layout = ui.driver_cancel_layout(dialog.kind())?;
      let events = dialog.update(ui, layout.buttons);
      ui.draw_discard(&layout, dialog.selected(), events)?;
      match crate::driver_discard::Dialog::outcome(events) {
        crate::driver_discard::Outcome::Discard => return Ok(None),
        crate::driver_discard::Outcome::KeepEditing => {
          discard = None;
          pointer.cancel();
          keyboard.cancel();
        }
        crate::driver_discard::Outcome::Pending => {}
      }
    }
    if let Some(dir) = capture {
      if frame == 2 || frame == 5 {
        crate::capture::save_frame(
          &dir.join(if frame == 2 {
            "driver-default.png"
          } else {
            "driver-mixed.png"
          }),
          &crate::capture::screen_data().await,
        )?;
      }
      if frame == 6 {
        return Ok(Some(build));
      }
    }
    if done {
      return Ok(Some(build));
    }
    next_frame().await;
    frame += 1;
  }
}
fn slot(build: &mut Build, row: usize) -> &mut usize {
  match row {
    0 => &mut build.hat,
    1 => &mut build.face,
    2 => &mut build.torso,
    _ => &mut build.legs,
  }
}
