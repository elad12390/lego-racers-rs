//! Actual new-racer transaction + native modal paths; no original loader/raster claim.
use crate::{
  custom_driver::{Build, Data},
  driver_discard::Kind,
  editor_gpu_test::{capture_editor_frame, logical_pixel, poll, send, send_mouse},
  game_catalog::Catalog,
  garage_edit::Draft,
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
    KeyCode::ArrowUp,
    KeyCode::ArrowDown,
    KeyCode::ArrowRight,
    KeyCode::Tab,
  ] {
    send(app, window, code, ButtonState::Released, None);
  }
  send_mouse(app, window, Vec2::new(630.0, 470.0), ButtonState::Released);
  platform::test_state(|s| {
    s.dt = 0.0;
    s.audio_commands.clear();
  });
}

fn center(rect: platform::prelude::Rect) -> Vec2 {
  Vec2::new(rect.x + rect.w * 0.5, rect.y + rect.h * 0.5)
}

fn tint(image: &platform::Image, rect: platform::prelude::Rect, green: bool) -> bool {
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

fn prompt_mask(image: &platform::Image, rect: platform::prelude::Rect) -> Vec<bool> {
  let mut result = Vec::new();
  for y in rect.y.ceil() as i32..(rect.y + rect.h).floor() as i32 {
    for x in rect.x.ceil() as i32..(rect.x + rect.w).floor() as i32 {
      let [r, g, b, _] = logical_pixel(image, Vec2::new(x as f32, y as f32));
      result.push(r > 200 && g > 200 && b > 200 && r.abs_diff(g) < 12 && r.abs_diff(b) < 12);
    }
  }
  result
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
  let baseline = serde_json::to_vec(profile).unwrap();
  let save = output.join("driver-new-cancel-existing-profile.json");
  // Only the pre-existing profile is saved. Creation never writes a profile.
  profile.save(&save).unwrap();
  let saved = std::fs::read(&save).unwrap();
  let data = Data::load(library).unwrap();
  let layout = ui.driver_cancel_layout(Kind::NewRacer).unwrap();
  assert_eq!(
    ui.strings.get(0x77).unwrap(),
    "ARE YOU SURE YOU WANT TO CANCEL THE NEW RACER PROCESS?"
  );
  let text_rect = platform::prelude::Rect::new(
    layout.frame.x + 20.0,
    layout.frame.y + 20.0,
    layout.frame.w - 40.0,
    layout.buttons[0].y - layout.frame.y - 20.0,
  );
  let mut unchanged_prompt = None;
  let mut cases = Vec::new();
  for code in [KeyCode::Enter, KeyCode::Space] {
    for changed in [false, true] {
      for keep in [true, false] {
        cases.push((code, changed, keep, false));
      }
    }
  }
  cases.extend([
    (KeyCode::Enter, true, true, true),
    (KeyCode::Enter, true, false, true),
  ]);

  for (code, changed, keep, pointer) in cases {
    reset(app, window);
    let mut working = profile.clone();
    let transaction =
      Draft::new_racer(&mut working).expect("new-racer fixture needs a free garage slot");
    assert!(transaction.creating);
    let new_baseline = serde_json::to_vec(&working).unwrap();
    let mut expected = working.custom_driver.clone().unwrap_or_default();
    if changed {
      let choices: Vec<_> = data
        .row(1)
        .iter()
        .enumerate()
        .filter(|(i, part)| {
          working.driver_part_allowed(part.unlock, catalog) || *i == expected.face
        })
        .map(|(i, _)| i)
        .collect();
      assert!(choices.len() > 1);
      let current = choices.iter().position(|i| *i == expected.face).unwrap();
      expected.face = choices[(current + 1) % choices.len()];
    }
    let result = {
      let mut editor = std::pin::pin!(crate::driver_editor::run_with_creation(
        library,
        &working,
        catalog,
        ui,
        None,
        transaction.creating
      ));
      assert!(poll(editor.as_mut()).is_pending());
      if changed {
        tap(app, window, editor.as_mut(), KeyCode::ArrowDown);
        tap(app, window, editor.as_mut(), KeyCode::ArrowRight);
      }
      for _ in 0..if changed { 5 } else { 6 } {
        tap(app, window, editor.as_mut(), KeyCode::ArrowDown);
      }
      // Opening must stay pending even for an unchanged new racer.
      tap(app, window, editor.as_mut(), KeyCode::Enter);
      app.update();
      platform::sync(app.world_mut());
      platform::test_state(|s| s.dt = 0.0);
      assert!(poll(editor.as_mut()).is_pending());
      if code == KeyCode::Enter && keep && !pointer {
        let image = capture_editor_frame(app);
        assert!(
          tint(&image, layout.buttons[1], false),
          "new process defaults to yellow NO"
        );
        let mask = prompt_mask(&image, text_rect);
        assert!(
          mask.iter().any(|pixel| *pixel),
          "new-process prompt must be rendered"
        );
        if changed {
          assert_eq!(
            Some(&mask),
            unchanged_prompt.as_ref(),
            "changed new parts must retain the new-process prompt, not discard-warning text"
          );
        } else {
          unchanged_prompt = Some(mask);
          crate::capture::save_frame(&output.join("driver-new-racer-no-default.png"), &image)
            .unwrap();
        }
      }
      let outcome = if pointer {
        let point = center(layout.buttons[usize::from(keep)]);
        assert!(mouse(app, window, editor.as_mut(), point, ButtonState::Pressed).is_pending());
        assert!(mouse(
          app,
          window,
          editor.as_mut(),
          Vec2::new(630.0, 470.0),
          ButtonState::Released
        )
        .is_pending());
        assert!(mouse(app, window, editor.as_mut(), point, ButtonState::Pressed).is_pending());
        mouse(app, window, editor.as_mut(), point, ButtonState::Released)
      } else {
        if !keep {
          tap(app, window, editor.as_mut(), KeyCode::ArrowUp);
        }
        assert!(key(app, window, editor.as_mut(), code, ButtonState::Pressed).is_pending());
        assert!(key(app, window, editor.as_mut(), code, ButtonState::Pressed).is_pending());
        let other = if code == KeyCode::Enter {
          KeyCode::Space
        } else {
          KeyCode::Enter
        };
        assert!(key(app, window, editor.as_mut(), other, ButtonState::Released).is_pending());
        if code == KeyCode::Enter && !keep && !changed {
          let image = capture_editor_frame(app);
          assert!(
            tint(&image, layout.buttons[0], true),
            "armed YES must be green"
          );
          crate::capture::save_frame(&output.join("driver-new-racer-yes-armed.png"), &image)
            .unwrap();
        }
        key(app, window, editor.as_mut(), code, ButtonState::Released)
      };
      if keep {
        assert!(
          outcome.is_pending(),
          "NO must preserve the new-racer process"
        );
        // Root focus remains Cancel. Going Up commits only the driver draft,
        // never the owning creation transaction or its persistent profile.
        tap(app, window, editor.as_mut(), KeyCode::ArrowUp);
        assert!(key(app, window, editor.as_mut(), code, ButtonState::Pressed).is_pending());
        let Poll::Ready(Ok(Some(build))) =
          key(app, window, editor.as_mut(), code, ButtonState::Released)
        else {
          panic!("NO did not keep the new draft available for Make License")
        };
        assert_eq!(build, expected);
        data.validate(&build).unwrap();
        Some(build)
      } else {
        assert!(
          matches!(outcome, Poll::Ready(Ok(None))),
          "YES must cancel on matching release only"
        );
        None
      }
    };
    assert!(
      transaction.creating,
      "editor completion must not complete creation"
    );
    assert_eq!(
      serde_json::to_vec(&working).unwrap(),
      new_baseline,
      "read-only driver editor changed owner profile"
    );
    if let Some(build) = result {
      // Mirror the actual owner accepting a driver draft, then cancel the still
      // unfinished creation via its real transaction rollback.
      working.custom_driver = Some(build);
    }
    assert_eq!(
      std::fs::read(&save).unwrap(),
      saved,
      "unfinished creation was saved"
    );
    transaction.cancel(&mut working);
    assert_eq!(
      serde_json::to_vec(&working).unwrap(),
      baseline,
      "Draft.cancel must restore every field, existing racers, photos and race progress"
    );
    assert_eq!(std::fs::read(&save).unwrap(), saved);
    assert_eq!(serde_json::to_vec(profile).unwrap(), baseline);
  }
  reset(app, window);
}
