//! Two separate racer choices; selection never edits the garage's active slot.
use crate::platform::prelude::*;
use crate::{
  game_catalog::Catalog, menu_ui::MenuUi, profile::Profile, racer_preview::RacerPreview,
};
use lrformats::library::Library;
pub async fn run(
  library: &Library,
  catalog: &Catalog,
  profile: &Profile,
  ui: &mut MenuUi,
) -> Result<Option<[Profile; 2]>, String> {
  let mut players = [profile.clone(), profile.clone()];
  if profile.racers.len() > 1 {
    players[1].select_racer((profile.active_racer + 1) % profile.racers.len())?;
  }
  let mut active = 0;
  let mut preview = RacerPreview::load(library, catalog, &players[active])?;
  loop {
    if is_quit_requested() || is_key_pressed(KeyCode::Escape) {
      return Ok(None);
    }
    ui.original_background("Versus Race", false);
    for i in 0..2 {
      if ui.action(
        &format!("Player {}: {}", i + 1, players[i].name),
        45.0,
        145.0 + i as f32 * 65.0,
        None,
      ) {
        active = i;
        preview = RacerPreview::load(library, catalog, &players[active])?;
      }
    }
    if is_key_pressed(KeyCode::Tab) {
      active = 1 - active;
      preview = RacerPreview::load(library, catalog, &players[active])?;
    }
    let delta = if ui.icon_button("arrowlu", 239.0, 87.0) || is_key_pressed(KeyCode::Left) {
      -1
    } else if ui.icon_button("arrowru", 572.0, 87.0) || is_key_pressed(KeyCode::Right) {
      1
    } else {
      0
    };
    if delta != 0 {
      let index = (players[active].active_racer as isize + delta)
        .rem_euclid(profile.racers.len() as isize) as usize;
      players[active].select_racer(index)?;
      preview = RacerPreview::load(library, catalog, &players[active])?;
    }
    preview.garage(ui)?;
    ui.centered_text(
      &format!("Player {}", active + 1),
      420.0,
      121.0,
      26.0,
      YELLOW,
    );
    ui.text("P1: Arrows + Space", 35.0, 288.0, 18.0, WHITE);
    ui.text("P2: W A S D + F", 35.0, 322.0, 18.0, WHITE);
    if ui.action("OK", 35.0, 406.0, Some("chck")) || is_key_pressed(KeyCode::Enter) {
      return Ok(Some(players));
    }
    if ui.action("Cancel", 35.0, 446.0, Some("txtx")) {
      return Ok(None);
    }
    next_frame().await;
  }
}
