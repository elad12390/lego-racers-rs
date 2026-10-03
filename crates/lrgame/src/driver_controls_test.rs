//! Actual original-asset driver loop, Metal captures and Bevy mouse messages.
use crate::{
  custom_driver::Data,
  editor_gpu_test::{capture_editor_frame, poll, ready, send_mouse},
  game_catalog::Catalog,
  menu_audio::MenuAudio,
  menu_ui::MenuUi,
  platform,
  profile::Profile,
};
use bevy::{input::ButtonState, prelude::*};
use lrformats::{library::Library, pcm, wav};
use std::{task::Poll, time::Duration};

fn expect_activate(sound: &platform::audio::Sound) {
  let commands = platform::test_state(|s| std::mem::take(&mut s.audio_commands));
  assert!(
    matches!(commands.as_slice(), [platform::audio::AudioCommand::Play(id,false,gain)] if *id == sound.0 && *gain > 0.0)
  );
}

#[test]
#[ignore = "requires original JAM and real Metal GPU; scripted/offscreen, not physical input or speaker playback"]
fn bevy_driver_original_buttons_mouse_mix_commit_cancel_and_held_silence() {
  let jam = std::env::var("LR_TEST_JAM").expect("original LR_TEST_JAM");
  let root =
    std::path::PathBuf::from(std::env::var("LR_TEST_OUTPUT").expect("isolated LR_TEST_OUTPUT"));
  std::fs::create_dir(&root).expect("test output must be a new isolated directory");
  let library = Library::open(&jam).unwrap();
  let catalog = Catalog::load(&library).unwrap();
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
    std::path::Path::new(&jam).parent().unwrap(),
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
  let profile = Profile::default();
  platform::rand::srand(0x9824_b93f_73cd_170b);
  let before = serde_json::to_vec(&profile).unwrap();
  let save = root.join("profile.json");
  profile.save(&save).unwrap();
  let file_before = std::fs::read(&save).unwrap();
  let mix = ui.driver_action_bounds("mix", 0x38, "nubutton").unwrap();
  let viewport = ui.driver_viewport().unwrap();
  assert_eq!(
    (viewport.x, viewport.y, viewport.w, viewport.h),
    (307.0, 80.0, 304.0, 354.0)
  );
  let commit = ui.driver_action_bounds("gonext", 10, "buttonra").unwrap();
  let cancel = ui.driver_action_bounds("goback", 31, "buttonca").unwrap();
  assert_eq!(
    (mix.x, mix.y, commit.x, commit.y, cancel.x, cancel.y),
    (3.0, 338.0, 3.0, 378.0, 3.0, 418.0)
  );
  let build = {
    let mut editor = std::pin::pin!(crate::driver_editor::run(
      &library, &profile, &catalog, &ui, None
    ));
    assert!(poll(editor.as_mut()).is_pending());
    let image = capture_editor_frame(&mut app);
    assert_eq!(
      crate::editor_gpu_test::logical_pixel(&image, Vec2::new(320.0, 90.0)),
      [0, 0, 55, 255],
      "original driver frame panel pixel"
    );
    crate::capture::save_frame(&root.join("driver-original-controls.png"), &image).unwrap();
    // Measured icon/text bounds, not the old generic 250px hit box.
    let outside = Vec2::new(mix.x + mix.w + 2.0, mix.y + mix.h * 0.5);
    send_mouse(&mut app, window, outside, ButtonState::Pressed);
    assert!(poll(editor.as_mut()).is_pending());
    assert!(platform::test_state(|s| s.audio_commands.is_empty()));
    send_mouse(&mut app, window, outside, ButtonState::Released);
    assert!(poll(editor.as_mut()).is_pending());
    let point = Vec2::new(mix.x + 1.0, mix.y + mix.h * 0.5);
    send_mouse(&mut app, window, point, ButtonState::Pressed);
    assert!(poll(editor.as_mut()).is_pending());
    expect_activate(&activate);
    app.update();
    platform::sync(app.world_mut());
    assert!(poll(editor.as_mut()).is_pending());
    assert!(
      platform::test_state(|s| s.audio_commands.is_empty()),
      "held Mix mouse repeats its action sound"
    );
    send_mouse(&mut app, window, point, ButtonState::Released);
    assert!(poll(editor.as_mut()).is_pending());
    app.update();
    platform::sync(app.world_mut());
    assert!(poll(editor.as_mut()).is_pending());
    let image = capture_editor_frame(&mut app);
    crate::capture::save_frame(&root.join("driver-mixed-original-controls.png"), &image).unwrap();
    assert_eq!(
      crate::editor_gpu_test::logical_pixel(&image, Vec2::new(320.0, 90.0)),
      [0, 0, 55, 255],
      "Mix must retain the original preview panel"
    );
    assert_eq!(
      crate::editor_gpu_test::logical_pixel(&image, Vec2::new(382.0, 398.0)),
      [255, 255, 255, 255],
      "Mix must retain the original stand top"
    );
    send_mouse(
      &mut app,
      window,
      Vec2::new(commit.x + 1.0, commit.y + commit.h * 0.5),
      ButtonState::Pressed,
    );
    assert!(
      poll(editor.as_mut()).is_pending(),
      "Make License must wait for mouse release"
    );
    expect_activate(&activate);
    send_mouse(
      &mut app,
      window,
      Vec2::new(600.0, 460.0),
      ButtonState::Released,
    );
    assert!(
      poll(editor.as_mut()).is_pending(),
      "release outside must cancel navigation"
    );
    send_mouse(
      &mut app,
      window,
      Vec2::new(commit.x + 1.0, commit.y + commit.h * 0.5),
      ButtonState::Pressed,
    );
    assert!(poll(editor.as_mut()).is_pending());
    expect_activate(&activate);
    send_mouse(
      &mut app,
      window,
      Vec2::new(commit.x + 1.0, commit.y + commit.h * 0.5),
      ButtonState::Released,
    );
    let Poll::Ready(Ok(Some(build))) = poll(editor.as_mut()) else {
      panic!("mouse Make License did not return edited driver")
    };
    build
  };
  let data = Data::load(&library).unwrap();
  assert_ne!(
    build,
    crate::custom_driver::Build::default(),
    "seeded mouse Mix must change the returned driver draft"
  );
  data.validate(&build).unwrap();
  for part in [
    &data.parts.hats[build.hat],
    &data.parts.faces[build.face],
    &data.parts.torsos[build.torso],
    &data.parts.legs[build.legs],
  ] {
    assert!(
      profile.driver_part_allowed(part.unlock, &catalog),
      "Mix selected a locked part"
    );
  }
  send_mouse(
    &mut app,
    window,
    Vec2::new(600.0, 460.0),
    ButtonState::Released,
  );
  {
    let mut editor = std::pin::pin!(crate::driver_editor::run(
      &library, &profile, &catalog, &ui, None
    ));
    assert!(poll(editor.as_mut()).is_pending());
    send_mouse(
      &mut app,
      window,
      Vec2::new(cancel.x + 1.0, cancel.y + cancel.h * 0.5),
      ButtonState::Pressed,
    );
    assert!(
      poll(editor.as_mut()).is_pending(),
      "Cancel must wait for mouse release"
    );
    expect_activate(&activate);
    send_mouse(
      &mut app,
      window,
      Vec2::new(cancel.x + 1.0, cancel.y + cancel.h * 0.5),
      ButtonState::Released,
    );
    assert!(matches!(poll(editor.as_mut()), Poll::Ready(Ok(None))));
  }
  crate::driver_row_test::verify(&mut app, window, &library, &catalog, &ui, &profile, &root);
  crate::driver_scroll_test::verify(&mut app, window, &library, &catalog, &ui, &profile, &root);
  crate::driver_wrap_test::verify(&mut app, window, &library, &catalog, &ui, &profile, &root);
  crate::driver_multirow_test::verify(&mut app, window, &library, &catalog, &ui, &profile, &root);
  crate::driver_pointer_test::verify(&mut app, window, &library, &catalog, &ui, &profile, &root);
  crate::driver_motion_test::verify(&mut app, window, &library, &catalog, &ui, &profile, &root);
  crate::driver_activation_test::verify(&mut app, window, &library, &catalog, &ui, &profile, &root);
  crate::driver_capture_focus_test::verify(
    &mut app, window, &library, &catalog, &ui, &profile, &root,
  );
  crate::driver_discard_test::verify(&mut app, window, &library, &catalog, &ui, &profile, &root);
  crate::driver_new_cancel_test::verify(&mut app, window, &library, &catalog, &ui, &profile, &root);
  crate::driver_row_focus_test::verify(&mut app, window, &library, &catalog, &ui, &profile, &root);
  crate::driver_hover_test::verify(&mut app, window, &library, &catalog, &ui, &profile, &root);
  crate::driver_thumbnail::export_geometry(
    &library,
    &ui,
    &root.join("driver-thumbnail-geometry.json"),
  );
  crate::driver_navigation_test::verify(&mut app, window, &library, &catalog, &ui, &profile, &root);
  crate::driver_thumbnail_test::verify_nohat(&mut app, &library, &ui, &root);
  assert_eq!(serde_json::to_vec(&profile).unwrap(), before);
  assert_eq!(
    std::fs::read(&save).unwrap(),
    file_before,
    "driver mouse actions wrote a readonly profile"
  );
  let report = serde_json::json!({"mode":"scripted_offscreen_real_gpu_not_physical_or_audible",
    "original_mib_button_anchors":true,"original_brick_frame_and_panel_pixel":true,"original_viewport":[307,80,304,354],"native_font_and_style":true,"mouse_mix_gutter":true,
    "make_license_mouse_commit":true,"cancel_mouse_returns_no_draft":true,"held_mix_silent":true,"release_only_navigation_and_outside_cancel":true,
    "measured_bounds_miss":true,"mix_parts_valid_and_unlocked":true,"profile_and_saved_file_unchanged":true,
    "eight_authored_parent_relative_row_arrow_bounds":true,"row_arrow_upper_corner_steps_once_and_just_outside_misses":true,
    "initial_hat_focus":true,"all35_keyboard_focus_transitions_and_wraps":true,"bottom_right_does_not_edit_last_row":true,
    "keyboard_cancels_held_mix":true});
  std::fs::write(
    root.join("driver-input.json"),
    serde_json::to_vec_pretty(&report).unwrap(),
  )
  .unwrap();
}
