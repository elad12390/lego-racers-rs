//! Real edited-draft discard paths; font/layout parity remains provisional.
use crate::{
  custom_driver::{Build, Data},
  editor_gpu_test::{capture_editor_frame, logical_pixel, poll, send, send_mouse},
  game_catalog::Catalog,
  menu_ui::MenuUi,
  platform,
  profile::Profile,
};
use bevy::{input::ButtonState, prelude::*};
use lrformats::library::Library;
use std::{future::Future, path::Path, pin::Pin, task::Poll};

type ResultBuild = Result<Option<Build>, String>;

fn key<F: Future<Output = ResultBuild>>(
  app: &mut App,
  window: Entity,
  editor: Pin<&mut F>,
  code: KeyCode,
  state: ButtonState,
) -> Poll<ResultBuild> {
  send(app, window, code, state, None);
  platform::test_state(|s| s.dt = 0.0);
  poll(editor)
}

fn tap<F: Future<Output = ResultBuild>>(
  app: &mut App,
  window: Entity,
  mut editor: Pin<&mut F>,
  code: KeyCode,
) {
  for state in [ButtonState::Pressed, ButtonState::Released] {
    assert!(key(app, window, editor.as_mut(), code, state).is_pending());
  }
}

fn tick<F: Future<Output = ResultBuild>>(app: &mut App, editor: Pin<&mut F>) {
  app.update();
  platform::sync(app.world_mut());
  platform::test_state(|s| s.dt = 0.0);
  assert!(poll(editor).is_pending());
}

fn mouse<F: Future<Output = ResultBuild>>(
  app: &mut App,
  window: Entity,
  editor: Pin<&mut F>,
  point: Vec2,
  state: ButtonState,
) -> Poll<ResultBuild> {
  send_mouse(app, window, point, state);
  platform::test_state(|s| s.dt = 0.0);
  poll(editor)
}

fn reset(app: &mut App, window: Entity) {
  for code in [
    KeyCode::Enter,
    KeyCode::Space,
    KeyCode::Tab,
    KeyCode::ArrowUp,
    KeyCode::ArrowDown,
    KeyCode::ArrowRight,
    KeyCode::Escape,
  ] {
    send(app, window, code, ButtonState::Released, None);
  }
  send_mouse(app, window, Vec2::new(630.0, 470.0), ButtonState::Released);
  platform::test_state(|s| {
    s.dt = 0.0;
    s.audio_commands.clear();
  });
}

fn open_changed<F: Future<Output = ResultBuild>>(
  app: &mut App,
  window: Entity,
  mut editor: Pin<&mut F>,
) {
  assert!(poll(editor.as_mut()).is_pending());
  tap(app, window, editor.as_mut(), KeyCode::ArrowDown); // Hat -> Face.
  tap(app, window, editor.as_mut(), KeyCode::ArrowRight); // Exactly one accepted edit.
  for _ in 0..5 {
    tap(app, window, editor.as_mut(), KeyCode::ArrowDown);
  }
  // Face -> Torso -> Legs -> Mix -> Make License -> Cancel.
  assert!(key(
    app,
    window,
    editor.as_mut(),
    KeyCode::Enter,
    ButtonState::Pressed
  )
  .is_pending());
  assert!(key(
    app,
    window,
    editor.as_mut(),
    KeyCode::Enter,
    ButtonState::Released
  )
  .is_pending());
  tick(app, editor); // Opening release cannot activate the modal default.
}

fn commit<F: Future<Output = ResultBuild>>(
  app: &mut App,
  window: Entity,
  mut editor: Pin<&mut F>,
  expected: &Build,
) {
  // Root focus remains Cancel when the modal closes.
  tap(app, window, editor.as_mut(), KeyCode::ArrowUp);
  assert!(key(
    app,
    window,
    editor.as_mut(),
    KeyCode::Enter,
    ButtonState::Pressed
  )
  .is_pending());
  let Poll::Ready(Ok(Some(build))) =
    key(app, window, editor, KeyCode::Enter, ButtonState::Released)
  else {
    panic!("Make License did not preserve the kept draft")
  };
  assert_eq!(
    &build, expected,
    "modal leaked input or discarded the edited Face"
  );
}

fn center(rect: platform::prelude::Rect) -> Vec2 {
  Vec2::new(rect.x + rect.w * 0.5, rect.y + rect.h * 0.5)
}

fn has_tint(image: &platform::Image, rect: platform::prelude::Rect, green: bool) -> bool {
  for y in rect.y.ceil() as i32..(rect.y + rect.h).floor() as i32 {
    for x in rect.x.ceil() as i32..(rect.x + rect.w).floor() as i32 {
      let [r, g, b, _] = logical_pixel(image, Vec2::new(x as f32, y as f32));
      let (r, g, b) = (u16::from(r), u16::from(g), u16::from(b));
      if if green {
        g > 150 && g > r + 40 && g > b + 40
      } else {
        r > 150 && g > 150 && b < 140
      } {
        return true;
      }
    }
  }
  false
}

pub fn verify(
  app: &mut App,
  window: Entity,
  library: &Library,
  catalog: &Catalog,
  ui: &MenuUi,
  profile: &Profile,
  output: &Path,
) {
  let before = serde_json::to_vec(profile).unwrap();
  let save = output.join("driver-discard-profile.json");
  profile.save(&save).unwrap();
  let saved = std::fs::read(&save).unwrap();
  let data = Data::load(library).unwrap();
  let mut edited = profile.custom_driver.clone().unwrap_or_default();
  let choices: Vec<_> = data
    .row(1)
    .iter()
    .enumerate()
    .filter(|(i, part)| profile.driver_part_allowed(part.unlock, catalog) || *i == edited.face)
    .map(|(i, _)| i)
    .collect();
  assert!(choices.len() > 1);
  let index = choices.iter().position(|i| *i == edited.face).unwrap();
  edited.face = choices[(index + 1) % choices.len()];
  data.validate(&edited).unwrap();
  let buttons = ui.discard_layout().unwrap().buttons;
  let outside = Vec2::new(630.0, 470.0);
  let mix = center(ui.driver_action_bounds("mix", 0x38, "nubutton").unwrap());
  assert!(
    !buttons.iter().any(|r| MenuUi::editor_contains(*r, mix)),
    "underlying Mix test point must not hit a modal button"
  );

  for code in [KeyCode::Enter, KeyCode::Space] {
    reset(app, window);
    {
      let mut editor = std::pin::pin!(crate::driver_editor::run(
        library, profile, catalog, ui, None
      ));
      open_changed(app, window, editor.as_mut());
      if code == KeyCode::Enter {
        let image = capture_editor_frame(app);
        assert!(
          has_tint(&image, buttons[1], false),
          "default CANCEL must be yellow"
        );
        crate::capture::save_frame(&output.join("driver-discard-default-cancel.png"), &image)
          .unwrap();
      }
      // Modal blocks underlying keyboard and actual bottom-Mix mouse activation.
      tap(app, window, editor.as_mut(), KeyCode::ArrowRight);
      assert!(mouse(app, window, editor.as_mut(), mix, ButtonState::Pressed).is_pending());
      assert!(mouse(app, window, editor.as_mut(), mix, ButtonState::Released).is_pending());
      // Also click the real Face arrow under the modal; no second Face edit.
      let arrow = center(ui.driver_row_arrow_bounds(1, true).unwrap());
      assert!(!buttons.iter().any(|r| MenuUi::editor_contains(*r, arrow)));
      assert!(mouse(app, window, editor.as_mut(), arrow, ButtonState::Pressed).is_pending());
      assert!(mouse(app, window, editor.as_mut(), outside, ButtonState::Released).is_pending());
      tap(app, window, editor.as_mut(), code); // Default CANCEL keeps editing.
      commit(app, window, editor.as_mut(), &edited);
    }
    reset(app, window);
    {
      let mut editor = std::pin::pin!(crate::driver_editor::run(
        library, profile, catalog, ui, None
      ));
      open_changed(app, window, editor.as_mut());
      tap(app, window, editor.as_mut(), KeyCode::ArrowUp); // CONTINUE.
      assert!(key(app, window, editor.as_mut(), code, ButtonState::Pressed).is_pending());
      tap(app, window, editor.as_mut(), KeyCode::ArrowDown);
      tap(app, window, editor.as_mut(), KeyCode::ArrowUp);
      assert!(
        key(app, window, editor.as_mut(), code, ButtonState::Released).is_pending(),
        "navigation away and back must cancel the armed discard"
      );
      assert!(key(app, window, editor.as_mut(), code, ButtonState::Pressed).is_pending());
      if code == KeyCode::Enter {
        let image = capture_editor_frame(app);
        assert!(
          has_tint(&image, buttons[0], true),
          "armed CONTINUE must be green"
        );
        crate::capture::save_frame(&output.join("driver-discard-continue-armed.png"), &image)
          .unwrap();
      }
      assert!(matches!(
        key(app, window, editor.as_mut(), code, ButtonState::Released),
        Poll::Ready(Ok(None))
      ));
    }
    assert_eq!(serde_json::to_vec(profile).unwrap(), before);
    assert_eq!(std::fs::read(&save).unwrap(), saved);
  }

  for selected in [1usize, 0] {
    reset(app, window);
    {
      let mut editor = std::pin::pin!(crate::driver_editor::run(
        library, profile, catalog, ui, None
      ));
      open_changed(app, window, editor.as_mut());
      let point = center(buttons[selected]);
      assert!(mouse(app, window, editor.as_mut(), point, ButtonState::Pressed).is_pending());
      assert!(mouse(app, window, editor.as_mut(), outside, ButtonState::Released).is_pending());
      // After outside release, the same modal button must still accept a fresh
      // click. CANCEL preserves the exact draft; CONTINUE exits only now.
      assert!(mouse(app, window, editor.as_mut(), point, ButtonState::Pressed).is_pending());
      let result = mouse(app, window, editor.as_mut(), point, ButtonState::Released);
      if selected == 0 {
        assert!(matches!(result, Poll::Ready(Ok(None))));
      } else {
        assert!(result.is_pending());
        commit(app, window, editor.as_mut(), &edited);
      }
    }
    assert_eq!(serde_json::to_vec(profile).unwrap(), before);
    assert_eq!(std::fs::read(&save).unwrap(), saved);
  }
  // No changed parts: Cancel has no warning to acknowledge.
  reset(app, window);
  {
    let mut editor = std::pin::pin!(crate::driver_editor::run(
      library, profile, catalog, ui, None
    ));
    assert!(poll(editor.as_mut()).is_pending());
    for _ in 0..6 {
      tap(app, window, editor.as_mut(), KeyCode::ArrowDown);
    }
    assert!(key(
      app,
      window,
      editor.as_mut(),
      KeyCode::Enter,
      ButtonState::Pressed
    )
    .is_pending());
    assert!(matches!(
      key(
        app,
        window,
        editor.as_mut(),
        KeyCode::Enter,
        ButtonState::Released
      ),
      Poll::Ready(Ok(None))
    ));
  }
  assert_eq!(serde_json::to_vec(profile).unwrap(), before);
  assert_eq!(std::fs::read(&save).unwrap(), saved);
  reset(app, window);
}
