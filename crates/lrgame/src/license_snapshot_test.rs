//! Real Bevy/Metal readback and original-asset license loop; scripted, not physical input.
use crate::editor_gpu_test::{capture_editor_frame, logical_pixel, poll, ready, send, send_mouse};
use crate::{menu_audio::MenuAudio, menu_ui::MenuUi, platform, profile::Profile};
use bevy::{input::ButtonState, prelude::*};
use lrformats::{library::Library, pcm, wav};
use std::{
  future::Future,
  pin::Pin,
  task::Poll,
  time::{Duration, Instant},
};

fn expect_effect(sound: &platform::audio::Sound) {
  let commands = platform::test_state(|s| std::mem::take(&mut s.audio_commands));
  assert!(
    matches!(commands.as_slice(), [platform::audio::AudioCommand::Play(id, false, gain)] if *id == sound.0 && *gain > 0.0)
  );
}
pub(crate) fn finish_readback(
  app: &mut App,
  editor: Pin<&mut impl Future<Output = Result<bool, String>>>,
) -> platform::Image {
  let mut editor = editor;
  let deadline = Instant::now() + Duration::from_secs(30);
  platform::submit_test_frame(app);
  loop {
    app.update();
    if let Some(image) = platform::test_state(|s| s.readback.clone()) {
      // Only a real GPU observer can fill this image; preserve it before the
      // application consumes it, for independently checking the saved photograph.
      assert!(poll(editor.as_mut()).is_pending());
      assert!(!platform::test_state(|s| s.capture_inflight));
      return image;
    }
    platform::submit_test_frame(app);
    assert!(Instant::now() < deadline, "native GPU readback timed out");
    std::thread::sleep(Duration::from_millis(5));
  }
}

#[test]
#[ignore = "requires LR_TEST_JAM and a real GPU; no OS window, physical input or speaker playback"]
fn bevy_license_f5_snapshot_real_gpu_input_commit_reload_and_cancel() {
  let library = Library::open(std::env::var("LR_TEST_JAM").expect("LR_TEST_JAM")).unwrap();
  let root =
    std::path::PathBuf::from(std::env::var("LR_TEST_OUTPUT").expect("isolated LR_TEST_OUTPUT"));
  std::fs::create_dir(&root).expect("test output must be a new isolated directory");
  let mut app = platform::offscreen_test_app();
  app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
    Duration::from_secs_f64(1.0 / 60.0),
  ));
  let window = app
    .world_mut()
    .spawn((Window::default(), bevy::window::PrimaryWindow))
    .id();
  app.update();
  platform::sync(app.world_mut());
  let mut ui = MenuUi::load(&library).unwrap();
  let mut audio = ready(MenuAudio::load(
    &library,
    std::path::Path::new(&std::env::var("LR_TEST_JAM").unwrap())
      .parent()
      .unwrap(),
  ));
  audio.update(true, false, true);
  ui.audio = Some(audio);
  let decoded = pcm::decode(
    library
      .find_at("ACTIVATE.PCM", "MENUDATA", "SOUNDS")
      .unwrap(),
  )
  .unwrap();
  let activate = ready(platform::audio::load_sound_from_bytes(
    &wav::encode_mono_16(decoded.sample_rate, &decoded.samples),
  ));
  platform::test_state(|s| s.audio_commands.clear());
  let mut profile = Profile::default();
  profile.name = "".into();
  profile.custom_driver = Some(crate::custom_driver::Build {
    hat: 15,
    face: 22,
    torso: 22,
    legs: 11,
  });
  let image = {
    let mut editor = std::pin::pin!(crate::license_editor::run(
      &library,
      &mut profile,
      &ui,
      None
    ));
    assert!(poll(editor.as_mut()).is_pending());
    let scale = MenuUi::scale();
    let origin = MenuUi::origin();
    let screen_height = platform::prelude::screen_height();
    platform::test_state(|s| {
      let scene = s.batches.iter().find(|b| b.camera.is_some()).unwrap();
      let camera = scene.camera.as_ref().unwrap();
      // CAM.WDB is the live source camera, not DRVRLICE's fallback eye/target.
      assert!((camera.position - Vec3::new(2.281, 2.558, 3.632)).length() < 0.001);
      assert!((camera.fovy.to_degrees() - 17.460205).abs() < 0.001);
      assert_eq!((camera.z_near, camera.z_far), (5.0, 800.0));
      assert_eq!(
        camera.viewport,
        Some((
          (origin.x + 374.0 * scale) as i32,
          (screen_height - origin.y - 222.0 * scale) as i32,
          (146.0 * scale) as i32,
          (140.0 * scale) as i32
        ))
      );
      let points: Vec<_> = s
        .batches
        .iter()
        .filter(|b| b.camera.is_some())
        .flat_map(|b| b.mesh.vertices.iter().map(|v| v.position))
        .collect();
      assert!(!points.is_empty());
      assert!(
        points.iter().all(|p| p.is_finite() && p.x < -3.0),
        "license driver must occupy the original camera-man placement, not the origin"
      );
      let rules = lrsim::brick_build::Rules::load().unwrap();
      assert_eq!(
        rules.license_preview_position.map(f32::to_bits),
        [0xc0ab7fb0, 0xc04999e5, 0]
      );
      assert_eq!(
        rules.license_preview_up.map(f32::to_bits),
        [0, 0xbd0ef241, 0x3f7fd817]
      );
    });
    send(
      &mut app,
      window,
      KeyCode::KeyA,
      ButtonState::Pressed,
      Some("gpu racer 1234567"),
    );
    assert!(poll(editor.as_mut()).is_pending());
    send(&mut app, window, KeyCode::KeyA, ButtonState::Released, None);
    assert!(poll(editor.as_mut()).is_pending());
    // Compare expression-cycle pixels at the original nonrepeating clip's held
    // endpoint, not by rewinding CMAMAN for each face as the old code did.
    let cmaman = lrformats::animation::parse(
      library
        .find_at("CMAMAN.ADB", "MENUDATA", "MENUDATA")
        .unwrap(),
    )
    .unwrap();
    let clip = &cmaman.clips[0];
    let settle_frames =
      (f32::from(clip.duration) / clip.frames_per_second * 60.0).ceil() as usize + 1;
    assert!(
      settle_frames < 600,
      "license animation unexpectedly exceeds bounded capture"
    );
    for _ in 0..settle_frames {
      app.update();
      platform::sync(app.world_mut());
      assert!(poll(editor.as_mut()).is_pending());
    }
    send(&mut app, window, KeyCode::F5, ButtonState::Pressed, None);
    assert!(poll(editor.as_mut()).is_pending());
    expect_effect(&activate);
    let image = finish_readback(&mut app, editor.as_mut());
    for point in [Vec2::new(377.0, 85.0), Vec2::new(517.0, 85.0)] {
      assert_eq!(
        logical_pixel(&image, point),
        [83, 90, 140, 255],
        "original license frame panel must replace guessed blue fill/border"
      );
    }
    crate::capture::save_frame(&root.join("snapshot-source.png"), &image).unwrap();
    platform::sync(app.world_mut());
    assert!(poll(editor.as_mut()).is_pending());
    assert!(
      platform::test_state(|s| s.audio_commands.is_empty()),
      "held F5 repeats the effect"
    );
    send(&mut app, window, KeyCode::F5, ButtonState::Released, None);
    assert!(poll(editor.as_mut()).is_pending());
    let rules = lrsim::brick_build::Rules::load().unwrap();
    crate::capture::save_frame(&root.join("expression-angry.png"), &image).unwrap();
    let mut crops = std::collections::HashSet::new();
    crops.insert(photo_pixels(&image));
    for expression in [2, 3, 4, 5, 0, 1] {
      send(&mut app, window, KeyCode::F5, ButtonState::Pressed, None);
      assert!(poll(editor.as_mut()).is_pending());
      expect_effect(&activate);
      let next = finish_readback(&mut app, editor.as_mut());
      let crop = photo_pixels(&next);
      if expression == 1 {
        assert_eq!(
          crop,
          photo_pixels(&image),
          "seventh Snapshot wraps back to the same angry face"
        );
      } else {
        assert!(
          crops.insert(crop),
          "distinct original face expression did not render"
        );
        crate::capture::save_frame(
          &root.join(format!(
            "expression-{}.png",
            rules.license_expressions[expression]
          )),
          &next,
        )
        .unwrap();
      }
      send(&mut app, window, KeyCode::F5, ButtonState::Released, None);
      assert!(poll(editor.as_mut()).is_pending());
    }
    assert_eq!(crops.len(), 6);
    send(&mut app, window, KeyCode::Enter, ButtonState::Pressed, None);
    assert!(matches!(poll(editor.as_mut()), Poll::Ready(Ok(true))));
    expect_effect(&activate);
    image
  };
  assert_eq!(profile.name, "GPU RACER 123");
  assert_eq!(profile.license_expression, 1);
  let photo = profile.license_photo.as_ref().unwrap();
  assert_eq!((photo.width, photo.height), (143, 139));
  let scale = MenuUi::scale();
  let origin = MenuUi::origin();
  for y in 0..139usize {
    for x in 0..143usize {
      let sx = ((origin.x + (375.0 + x as f32) * scale) * image.width as f32
        / platform::prelude::screen_width())
      .floor() as usize;
      let sy = ((origin.y + (82.0 + y as f32) * scale) * image.height as f32
        / platform::prelude::screen_height())
      .floor() as usize;
      let source = ((image.height as usize - 1 - sy) * image.width as usize + sx) * 4;
      assert_eq!(
        &photo.rgba[(y * 143 + x) * 4..(y * 143 + x + 1) * 4],
        &image.bytes[source..source + 4]
      );
    }
  }
  let unique: std::collections::HashSet<_> = photo.rgba.chunks_exact(4).collect();
  assert!(
    unique.len() > 50,
    "photograph must contain original rendered geometry, not a blank crop"
  );
  let save = root.join("profile.json");
  profile.save(&save).unwrap();
  let mut restored = Profile::load(&save).unwrap();
  assert_eq!(restored.name, profile.name);
  assert_eq!(restored.license_expression, 1);
  assert!(restored.license_photo == profile.license_photo);
  let before = serde_json::to_vec(&restored).unwrap();
  let file_before = std::fs::read(&save).unwrap();
  send(
    &mut app,
    window,
    KeyCode::Enter,
    ButtonState::Released,
    None,
  );
  ui.audio.as_mut().unwrap().update(true, false, false);
  platform::test_state(|s| s.audio_commands.clear());
  {
    let mut editor = std::pin::pin!(crate::license_editor::run(
      &library,
      &mut restored,
      &ui,
      None
    ));
    assert!(poll(editor.as_mut()).is_pending());
    send(&mut app, window, KeyCode::F5, ButtonState::Pressed, None);
    assert!(poll(editor.as_mut()).is_pending());
    assert!(
      platform::test_state(|s| s.audio_commands.is_empty()),
      "Sound Off snapshot emits SFX"
    );
    let changed = finish_readback(&mut app, editor.as_mut());
    assert_ne!(
      photo_pixels(&changed),
      photo_pixels(&image),
      "retaking a saved photograph must render the next expression, not the old texture"
    );
    send(&mut app, window, KeyCode::F5, ButtonState::Released, None);
    assert!(poll(editor.as_mut()).is_pending());
    send(
      &mut app,
      window,
      KeyCode::Backspace,
      ButtonState::Pressed,
      None,
    );
    assert!(poll(editor.as_mut()).is_pending());
    send(
      &mut app,
      window,
      KeyCode::KeyA,
      ButtonState::Pressed,
      Some("z"),
    );
    assert!(poll(editor.as_mut()).is_pending());
    send(
      &mut app,
      window,
      KeyCode::Escape,
      ButtonState::Pressed,
      None,
    );
    assert!(matches!(poll(editor.as_mut()), Poll::Ready(Ok(false))));
  }
  assert_eq!(
    serde_json::to_vec(&restored).unwrap(),
    before,
    "cancelled photograph/name draft changed the profile"
  );
  assert_eq!(
    std::fs::read(&save).unwrap(),
    file_before,
    "cancel wrote to the saved profile"
  );
  assert!(platform::test_state(|s| s.audio_commands.is_empty()));
  send(
    &mut app,
    window,
    KeyCode::Escape,
    ButtonState::Released,
    None,
  );
  ui.audio.as_mut().unwrap().update(true, false, true);
  platform::test_state(|s| s.audio_commands.clear());
  let mut mouse_profile = restored.clone();
  let mouse_photo = {
    let mut editor = std::pin::pin!(crate::license_editor::run(
      &library,
      &mut mouse_profile,
      &ui,
      None
    ));
    assert!(poll(editor.as_mut()).is_pending());
    // Beyond measured text/icon bounds: the old generic 250px hit box wrongly
    // activated Snapshot here. The real original button ends before this point.
    send_mouse(
      &mut app,
      window,
      Vec2::new(260.0, 350.0),
      ButtonState::Pressed,
    );
    assert!(poll(editor.as_mut()).is_pending());
    assert!(!platform::test_state(
      |s| s.capture_requested || s.capture_inflight
    ));
    assert!(platform::test_state(|s| s.audio_commands.is_empty()));
    send_mouse(
      &mut app,
      window,
      Vec2::new(260.0, 350.0),
      ButtonState::Released,
    );
    assert!(poll(editor.as_mut()).is_pending());
    // Rect_ContainsRelative includes the right/bottom corner. Mouse-only
    // Snapshot must arm and release there, not just inside the text/gutter.
    let snapshot_bounds = ui
      .license_action_bounds("swapface", 0x3b, "nubutton")
      .unwrap();
    let snapshot_corner = Vec2::new(
      snapshot_bounds.x + snapshot_bounds.w,
      snapshot_bounds.y + snapshot_bounds.h,
    );
    send_mouse(&mut app, window, snapshot_corner, ButtonState::Pressed);
    assert!(poll(editor.as_mut()).is_pending());
    expect_effect(&activate);
    assert!(
      !platform::test_state(|s| s.capture_requested || s.capture_inflight),
      "Snapshot must not fire on mouse-down"
    );
    let pressed = capture_editor_frame(&mut app);
    crate::capture::save_frame(&root.join("license-mouse-pressed.png"), &pressed).unwrap();
    app.update();
    platform::sync(app.world_mut());
    assert!(poll(editor.as_mut()).is_pending());
    assert!(
      platform::test_state(|s| s.audio_commands.is_empty()),
      "held mouse repeated activation"
    );
    let name_hover = MenuUi::origin() + Vec2::new(125.0, 210.0) * MenuUi::scale();
    app
      .world_mut()
      .get_mut::<Window>(window)
      .unwrap()
      .set_cursor_position(Some(name_hover));
    app.update();
    platform::sync(app.world_mut());
    assert!(poll(editor.as_mut()).is_pending());
    send(
      &mut app,
      window,
      KeyCode::Backspace,
      ButtonState::Pressed,
      None,
    );
    assert!(poll(editor.as_mut()).is_pending());
    send(
      &mut app,
      window,
      KeyCode::Backspace,
      ButtonState::Released,
      None,
    );
    assert!(poll(editor.as_mut()).is_pending());
    send_mouse(
      &mut app,
      window,
      Vec2::new(600.0, 400.0),
      ButtonState::Released,
    );
    assert!(poll(editor.as_mut()).is_pending());
    assert!(
      !platform::test_state(|s| s.capture_requested || s.capture_inflight),
      "release outside must cancel Snapshot"
    );
    send_mouse(&mut app, window, snapshot_corner, ButtonState::Pressed);
    assert!(poll(editor.as_mut()).is_pending());
    expect_effect(&activate);
    send_mouse(&mut app, window, snapshot_corner, ButtonState::Released);
    assert!(poll(editor.as_mut()).is_pending());
    let clicked = finish_readback(&mut app, editor.as_mut());
    assert_ne!(
      photo_pixels(&pressed),
      photo_pixels(&clicked),
      "matching release must photograph the next expression"
    );
    crate::capture::save_frame(&root.join("license-mouse-snapshot.png"), &clicked).unwrap();
    send_mouse(
      &mut app,
      window,
      Vec2::new(4.0, 390.0),
      ButtonState::Pressed,
    );
    assert!(
      poll(editor.as_mut()).is_pending(),
      "Build Car must wait for release"
    );
    expect_effect(&activate);
    send_mouse(
      &mut app,
      window,
      Vec2::new(4.0, 390.0),
      ButtonState::Released,
    );
    assert!(matches!(poll(editor.as_mut()), Poll::Ready(Ok(true))));
    clicked
  };
  assert_eq!(mouse_profile.license_expression, 2);
  assert_eq!(
    mouse_profile.license_photo.as_ref().unwrap().rgba,
    photo_pixels(&mouse_photo)
  );
  assert_eq!(mouse_profile.name, restored.name);
  send_mouse(
    &mut app,
    window,
    Vec2::new(4.0, 390.0),
    ButtonState::Released,
  );
  let before_back = serde_json::to_vec(&mouse_profile).unwrap();
  {
    let mut editor = std::pin::pin!(crate::license_editor::run(
      &library,
      &mut mouse_profile,
      &ui,
      None
    ));
    assert!(poll(editor.as_mut()).is_pending());
    send_mouse(
      &mut app,
      window,
      Vec2::new(4.0, 430.0),
      ButtonState::Pressed,
    );
    assert!(
      poll(editor.as_mut()).is_pending(),
      "Build Driver must wait for release"
    );
    expect_effect(&activate);
    send_mouse(
      &mut app,
      window,
      Vec2::new(4.0, 430.0),
      ButtonState::Released,
    );
    assert!(matches!(poll(editor.as_mut()), Poll::Ready(Ok(false))));
  }
  assert_eq!(serde_json::to_vec(&mouse_profile).unwrap(), before_back);
  send_mouse(
    &mut app,
    window,
    Vec2::new(600.0, 400.0),
    ButtonState::Released,
  );
  platform::test_state(|s| s.audio_commands.clear());
  let mut focus_profile = restored.clone();
  focus_profile.name = "EDIT".into();
  {
    let mut editor = std::pin::pin!(crate::license_editor::run(
      &library,
      &mut focus_profile,
      &ui,
      None
    ));
    assert!(poll(editor.as_mut()).is_pending());
    let bounds = ui.license_name_bounds().unwrap();
    assert_eq!(
      (bounds.x, bounds.y, bounds.w, bounds.h),
      (120.0, 197.0, 268.0, 34.0)
    );
    let caret = ui.license_caret_bounds("EDIT").unwrap();
    assert_eq!((caret.y, caret.h), (225.0, 4.0));
    let point = Vec2::new(caret.x + caret.w * 0.5, caret.y + 2.0);
    let hidden = capture_editor_frame(&mut app);
    assert_eq!(
      logical_pixel(&hidden, Vec2::new(114.0, 200.0)),
      [8, 8, 115, 255],
      "name highlight must show original roundbox panel behind the text"
    );
    assert_ne!(logical_pixel(&hidden, point), [255; 4]);
    // Timer uses actual Bevy frame time with deterministic offscreen time updates.
    for _ in 0..64 {
      app.update();
      platform::sync(app.world_mut());
      assert!(poll(editor.as_mut()).is_pending());
    }
    let visible = capture_editor_frame(&mut app);
    assert_eq!(
      logical_pixel(&visible, point),
      [255; 4],
      "original white underline must render at the text end"
    );
    crate::capture::save_frame(&root.join("license-name-caret.png"), &visible).unwrap();
    // Hover, not clicking Snapshot: focus moves, typing/backspace must not edit.
    let hover = MenuUi::origin() + Vec2::new(4.0, 350.0) * MenuUi::scale();
    app
      .world_mut()
      .get_mut::<Window>(window)
      .unwrap()
      .set_cursor_position(Some(hover));
    app.update();
    platform::sync(app.world_mut());
    assert!(poll(editor.as_mut()).is_pending());
    let unfocused = capture_editor_frame(&mut app);
    assert_ne!(
      logical_pixel(&unfocused, Vec2::new(114.0, 200.0)),
      [8, 8, 115, 255],
      "original focus callback must remove the name frame"
    );
    assert_ne!(
      logical_pixel(&unfocused, point),
      [255; 4],
      "caret must detach when name loses highlight"
    );
    crate::capture::save_frame(&root.join("license-name-unfocused.png"), &unfocused).unwrap();
    send(
      &mut app,
      window,
      KeyCode::KeyA,
      ButtonState::Pressed,
      Some("lost"),
    );
    assert!(poll(editor.as_mut()).is_pending());
    send(
      &mut app,
      window,
      KeyCode::Backspace,
      ButtonState::Pressed,
      None,
    );
    assert!(poll(editor.as_mut()).is_pending());
    send_mouse(
      &mut app,
      window,
      Vec2::new(bounds.x + bounds.w, bounds.y + bounds.h),
      ButtonState::Pressed,
    );
    assert!(poll(editor.as_mut()).is_pending());
    send(&mut app, window, KeyCode::KeyA, ButtonState::Released, None);
    assert!(poll(editor.as_mut()).is_pending());
    send(
      &mut app,
      window,
      KeyCode::Backspace,
      ButtonState::Released,
      None,
    );
    assert!(poll(editor.as_mut()).is_pending());
    send(
      &mut app,
      window,
      KeyCode::KeyA,
      ButtonState::Pressed,
      Some("x!"),
    );
    assert!(poll(editor.as_mut()).is_pending());
    send(&mut app, window, KeyCode::Enter, ButtonState::Pressed, None);
    assert!(matches!(poll(editor.as_mut()), Poll::Ready(Ok(true))));
  }
  assert_eq!(
    focus_profile.name, "EDITX",
    "unfocused input leaked or refocused input/alphabet failed"
  );
  assert!(focus_profile.license_photo == restored.license_photo);
  send(
    &mut app,
    window,
    KeyCode::Enter,
    ButtonState::Released,
    None,
  );
  send(&mut app, window, KeyCode::KeyA, ButtonState::Released, None);
  send_mouse(
    &mut app,
    window,
    Vec2::new(600.0, 400.0),
    ButtonState::Released,
  );
  let mut boundary = restored.clone();
  boundary.name = "ABCDEFGHIJKL".into(); // Twelve: one free character.
  {
    let mut editor = std::pin::pin!(crate::license_editor::run(
      &library,
      &mut boundary,
      &ui,
      None
    ));
    assert!(poll(editor.as_mut()).is_pending());
    for _ in 0..64 {
      app.update();
      platform::sync(app.world_mut());
      assert!(poll(editor.as_mut()).is_pending());
    }
    let caret = ui.license_caret_bounds("ABCDEFGHIJKL").unwrap();
    let point = Vec2::new(caret.x + caret.w * 0.5, caret.y + 2.0);
    let twelve = capture_editor_frame(&mut app);
    assert_eq!(
      logical_pixel(&twelve, point),
      [255; 4],
      "12 characters leave the original caret visible"
    );
    crate::capture::save_frame(&root.join("license-name-12.png"), &twelve).unwrap();
    send(
      &mut app,
      window,
      KeyCode::KeyM,
      ButtonState::Pressed,
      Some("m"),
    );
    assert!(poll(editor.as_mut()).is_pending());
    send(&mut app, window, KeyCode::KeyM, ButtonState::Released, None);
    assert!(poll(editor.as_mut()).is_pending());
    let full_caret = ui.license_caret_bounds("ABCDEFGHIJKLM").unwrap();
    let full_point = Vec2::new(full_caret.x + full_caret.w * 0.5, full_caret.y + 2.0);
    let thirteen = capture_editor_frame(&mut app);
    assert_ne!(
      logical_pixel(&thirteen, full_point),
      [255; 4],
      "13 characters hide the full-field caret"
    );
    crate::capture::save_frame(&root.join("license-name-13.png"), &thirteen).unwrap();
    send(
      &mut app,
      window,
      KeyCode::KeyN,
      ButtonState::Pressed,
      Some("n"),
    );
    assert!(poll(editor.as_mut()).is_pending());
    let rejected = capture_editor_frame(&mut app);
    assert_eq!(
      rejected.bytes, thirteen.bytes,
      "14th character must not alter a held photo/name frame"
    );
    send(&mut app, window, KeyCode::KeyN, ButtonState::Released, None);
    assert!(poll(editor.as_mut()).is_pending());
    send(
      &mut app,
      window,
      KeyCode::Backspace,
      ButtonState::Pressed,
      None,
    );
    assert!(poll(editor.as_mut()).is_pending());
    let recovered = capture_editor_frame(&mut app);
    assert_eq!(
      logical_pixel(&recovered, point),
      [255; 4],
      "Backspace from full13 restores12-character caret"
    );
    send(
      &mut app,
      window,
      KeyCode::Backspace,
      ButtonState::Released,
      None,
    );
    assert!(poll(editor.as_mut()).is_pending());
    send(
      &mut app,
      window,
      KeyCode::KeyM,
      ButtonState::Pressed,
      Some("m"),
    );
    assert!(poll(editor.as_mut()).is_pending());
    send(&mut app, window, KeyCode::Enter, ButtonState::Pressed, None);
    assert!(matches!(poll(editor.as_mut()), Poll::Ready(Ok(true))));
  }
  assert_eq!(boundary.name, "ABCDEFGHIJKLM");
  assert!(boundary.license_photo == restored.license_photo);
  send(&mut app, window, KeyCode::KeyM, ButtonState::Released, None);
  send(
    &mut app,
    window,
    KeyCode::Enter,
    ButtonState::Released,
    None,
  );
  let mut legacy = restored.clone();
  legacy.name = "ABCDEFGHIJKLMN".into(); // Previously saved native14 stays valid.
  let legacy_save = root.join("legacy-fourteen-name.json");
  legacy.save(&legacy_save).unwrap();
  let old_file = std::fs::read(&legacy_save).unwrap();
  legacy = Profile::load(&legacy_save).unwrap();
  {
    let mut editor = std::pin::pin!(crate::license_editor::run(&library, &mut legacy, &ui, None));
    assert!(poll(editor.as_mut()).is_pending());
    send(
      &mut app,
      window,
      KeyCode::Escape,
      ButtonState::Pressed,
      None,
    );
    assert!(matches!(poll(editor.as_mut()), Poll::Ready(Ok(false))));
  }
  assert_eq!(
    legacy.name, "ABCDEFGHIJKLMN",
    "cancelling a shortened local draft must preserve saved14"
  );
  assert_eq!(std::fs::read(&legacy_save).unwrap(), old_file);
  crate::license_navigation_test::verify(&mut app, window, &library, &ui, &restored, &root);
  crate::license_activation_test::verify(&mut app, window, &library, &ui, &restored, &root);
  let report = serde_json::json!({
    "mode": "scripted_offscreen_real_gpu_not_physical_or_audible",
    "authored_panel_background_pixels": true,
    "original_thirteen_character_name_limit": true,
    "twelve_caret_thirteen_full_fourteen_rejected": true,
    "backspace_reopens_full_name": true,
    "legacy_fourteen_name_file_and_cancel_preserved": true,
    "f5_readback_crop_exact": true,
    "photo_restored": true,
    "expression_restored": true,
    "six_distinct_expressions_and_wrap": true,
    "retake_renders_new_face": true,
    "cancel_preserves_profile_and_file": true,
    "held_key_silent": true,
    "sound_off_silent": true,
    "original_button_mouse_gutter_snapshot_commit_cancel": true,
    "mouse_down_arms_release_inside_commits_outside_cancels": true,
    "mouse_capture_keeps_name_unfocused_while_dragging": true,
    "outside_measured_button_does_not_activate": true,
    "held_mouse_silent": true,
    "name_focus_gates_typing_and_backspace": true,
    "name_refocus_restores_input": true,
    "original_white_caret_gpu_pixels_and_detach": true,
    "expression_cycle_at_held_cmaman_endpoint": true,
    "original_roundbox_focus_frame_gpu_pixels_and_detach": true,
    "inclusive_name_and_snapshot_upper_corners": true,
    "tab_shift_tab_both_shift_keys_up_down_order_wrap_gpu": true,
    "held_navigation_silent_and_unfocused_input_ignored": true,
    "keyboard_navigation_cancels_held_mouse_operation": true,
  });
  std::fs::write(
    root.join("license-input.json"),
    serde_json::to_vec_pretty(&report).unwrap(),
  )
  .unwrap();
}

fn photo_pixels(image: &platform::Image) -> Vec<u8> {
  let scale = MenuUi::scale();
  let origin = MenuUi::origin();
  let mut bytes = Vec::new();
  for y in 0..139 {
    for x in 0..143 {
      let sx = ((origin.x + (375.0 + x as f32) * scale) * image.width as f32
        / platform::prelude::screen_width())
      .floor() as usize;
      let sy = ((origin.y + (82.0 + y as f32) * scale) * image.height as f32
        / platform::prelude::screen_height())
      .floor() as usize;
      let at = ((image.height as usize - 1 - sy) * image.width as usize + sx) * 4;
      bytes.extend_from_slice(&image.bytes[at..at + 4]);
    }
  }
  bytes
}
