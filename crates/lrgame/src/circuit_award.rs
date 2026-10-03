//! Original PC award CDB scenes and first-time championship reward handoff.
//! PS1 reference screenshots do not establish PC overlay/layout parity.
use crate::platform::prelude::*;
use crate::{
  cinematic_scene::Scene, custom_driver::Data, game_catalog::Catalog, menu_ui::MenuUi,
  profile::Profile,
};
use lrformats::library::Library;

pub fn scene_spec(rank: u32) -> (&'static str, &'static str) {
  match rank {
    1 => ("C_AWARD1", "1STPLACE"),
    2 => ("C_AWARD2", "TWO"),
    3 => ("C_AWARD3", "3-01"),
    _ => ("C_AWARD4", "LOSE"),
  }
}
pub async fn run(
  library: &Library,
  catalog: &Catalog,
  profile: &Profile,
  rank: u32,
  new_parts: Option<usize>,
  ui: &MenuUi,
  capture: Option<&std::path::Path>,
) -> Result<(), String> {
  let data = Data::load(library)?;
  let driver = profile.custom_driver.clone().map(Ok).unwrap_or_else(|| {
    data.original_driver(
      catalog
        .drivers
        .iter()
        .find(|d| d.name.eq_ignore_ascii_case(&profile.driver))
        .ok_or("missing award driver")?,
    )
  })?;
  let (table, cdb) = scene_spec(rank);
  let mut scene = Scene::load_named(library, table, cdb, None, Some(&driver))?;
  let audio = crate::cinematic_audio::Audio::load(
    library,
    table,
    cdb,
    &scene,
    profile.sound && capture.is_none(),
  )
  .await?;
  let overlay = crate::cinematic_overlay::Overlay::load(library, table, cdb, &scene, None)?;
  present(
    &mut scene,
    audio,
    overlay,
    ui,
    capture.map(|p| (p, format!("award-{rank}"))),
  )
  .await?;
  if let Some(circuit) = new_parts {
    // CRB host is slot one. Unlock scenes show the champion, not an
    // unrelated placeholder minifigure or the player's currently built car.
    let host = &catalog
      .circuits
      .get(circuit)
      .ok_or("award circuit outside original catalog")?
      .racers[1];
    unlock(
      library,
      catalog,
      profile,
      host,
      ui,
      capture.map(|p| (p, format!("unlock-{}", circuit + 1))),
    )
    .await?;
  }
  Ok(())
}
pub async fn unlock(
  library: &Library,
  catalog: &Catalog,
  profile: &Profile,
  host: &str,
  ui: &MenuUi,
  capture: Option<(&std::path::Path, String)>,
) -> Result<(), String> {
  let data = Data::load(library)?;
  let champion = catalog
    .drivers
    .iter()
    .find(|d| d.name.eq_ignore_ascii_case(host))
    .ok_or("missing original reward champion")?;
  let driver = data.original_driver(champion)?;
  let car = catalog
    .cars
    .iter()
    .find(|c| c.name.eq_ignore_ascii_case(&champion.car))
    .ok_or("missing reward car")?;
  let chassis = lrformats::cmb::parse(
    library
      .find_in("CHASSIS.CMB", "COMMON")
      .ok_or("missing original reward chassis catalog")?,
  )
  .map_err(|e| e.to_string())?
  .into_iter()
  .find(|c| c.name.eq_ignore_ascii_case(&car.chassis))
  .ok_or("missing champion chassis")?;
  let model = crate::award_car::load(library, car, &chassis)?;
  // MIB movie widgets name the resource, not the owning table.
  let (table, cdb) = if champion.name.eq_ignore_ascii_case("RR") {
    ("WINRRCAR", "END")
  } else if champion.name.eq_ignore_ascii_case("VV") {
    ("WINVVCAR", "VVWIN")
  } else {
    ("WINCAR", "WINCAR")
  };
  // MENUNAME id 0x1c is WINVVCAR. MinifigPreviewScreen 00475fd0
  // explicitly excludes it from driver/car substitution: Veronica's movie
  // has its own 30-joint rig and 0.01-scaled geometry, not MENUPART's 29.
  let mut scene = if champion.name.eq_ignore_ascii_case("VV") {
    Scene::load(library, table, cdb)?
  } else {
    Scene::load_custom(library, table, cdb, None, Some(&driver), Some(&model))?
  };
  let audio = crate::cinematic_audio::Audio::load(
    library,
    table,
    cdb,
    &scene,
    profile.sound && capture.is_none(),
  )
  .await?;
  let overlay = crate::cinematic_overlay::Overlay::load(library, table, cdb, &scene, Some(host))?;
  present(&mut scene, audio, overlay, ui, capture).await
}
async fn present(
  scene: &mut Scene,
  mut audio: crate::cinematic_audio::Audio,
  overlay: crate::cinematic_overlay::Overlay,
  ui: &MenuUi,
  capture: Option<(&std::path::Path, String)>,
) -> Result<(), String> {
  let mut seconds = 0.0f32;
  let mut frame = 0u32;
  loop {
    if is_quit_requested() {
      return Ok(());
    }
    clear_background(BLACK);
    audio.advance(seconds * scene.fps);
    scene.draw(
      Rect::new(0.0, 0.0, 640.0, 480.0),
      seconds.min(scene.duration - 1.0 / 30.0),
    )?;
    overlay.draw(seconds);
    let finished = seconds >= scene.duration;
    if capture.is_none() && finished {
      if ui.action("Continue", 55.0, 453.0, Some("chck"))
        || is_key_pressed(KeyCode::Enter)
        || is_key_pressed(KeyCode::Space)
        || is_key_pressed(KeyCode::Escape)
      {
        return Ok(());
      }
    }
    if let Some((dir, name)) = &capture {
      // Capture two different actual original animation poses, with no
      // diagnostic overlay. This is scene/load evidence, not won races.
      if frame == 60 || frame == 180 {
        crate::capture::save_frame(
          &dir.join(format!(
            "{name}-{}.png",
            if frame == 60 { "early" } else { "late" }
          )),
          &crate::capture::screen_data().await,
        )?;
      }
      if overlay.text_capture_frame() == Some(frame) {
        crate::capture::save_frame(
          &dir.join(format!("{name}-text.png")),
          &crate::capture::screen_data().await,
        )?;
      }
      if frame >= 240 && finished {
        std::fs::write(dir.join(format!("{name}-audio.json")),serde_json::to_vec_pretty(&serde_json::json!({"mode":"silent_native_pcm_decode_and_timeline_dispatch_not_audible_acceptance","dispatched":audio.dispatched(),"frames":frame,"completed_timeline":finished,"unavailable_banks":audio.unavailable_banks,"material_gaps":scene.material_gaps})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
        next_frame().await;
        return Ok(());
      }
    } else if !finished
      && (is_key_pressed(KeyCode::Enter)
        || is_key_pressed(KeyCode::Space)
        || is_key_pressed(KeyCode::Escape))
    {
      seconds = scene.duration;
    }
    next_frame().await;
    frame += 1;
    seconds += if capture.is_some() {
      1.0 / 30.0
    } else {
      get_frame_time().min(0.1)
    };
  }
}
