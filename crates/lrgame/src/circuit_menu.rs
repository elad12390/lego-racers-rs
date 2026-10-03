//! Original PC circuit selector with native original champion scene previews.
use crate::platform::prelude::*;
use crate::{cinematic_scene::Scene, game_catalog::Catalog, menu_ui::MenuUi, profile::Profile};
use lrformats::library::Library;
pub enum Action {
  None,
  Back,
  Start(usize),
}
pub struct CircuitMenu {
  pub selected: usize,
  slot: usize,
  loaded: Option<(usize, usize)>,
  scene: Option<Scene>,
  start: f64,
}
impl CircuitMenu {
  pub fn new() -> Self {
    Self {
      selected: 0,
      slot: 0,
      loaded: None,
      scene: None,
      start: 0.0,
    }
  }
  pub fn draw(
    &mut self,
    library: &Library,
    catalog: &Catalog,
    profile: &Profile,
    ui: &mut MenuUi,
  ) -> Result<Action, String> {
    ui.original_background("Circuit Race", false);
    let previous = ui.icon_button("arrowlu", 3.0, 29.0) || ui.navigation_key(KeyCode::Left);
    let next = ui.icon_button("arrowru", 188.0, 29.0) || ui.navigation_key(KeyCode::Right);
    if previous {
      self.selected = (self.selected + catalog.circuits.len() - 1) % catalog.circuits.len();
    }
    if next {
      self.selected = (self.selected + 1) % catalog.circuits.len();
    }
    ui.image(
      [
        "pirate", "islander", "magical", "adventur", "jungle", "alien", "rr",
      ][self.selected],
      35.0,
      29.0,
      152.0,
      40.0,
    );
    let host = &catalog.rules.circuit_names[self.selected];
    let words = host.split_whitespace().collect::<Vec<_>>();
    let split = if words.len() > 2 { 2 } else { 1 };
    let races = catalog.circuit_races(&catalog.circuits[self.selected].name);
    if races.is_empty() {
      return Err("circuit has no original tracks".into());
    }
    if self.loaded.is_some_and(|(c, _)| c != self.selected) {
      self.slot = 0;
    }
    if self.loaded.is_some_and(|(c, _)| c == self.selected)
      && self
        .scene
        .as_ref()
        .is_some_and(|s| get_time() - self.start > s.duration as f64)
    {
      self.slot = (self.slot + 1) % races.len();
    }
    self.slot = self.slot.min(races.len() - 1);
    let race = &races[self.slot];
    ui.centered_text(
      &catalog.title(&catalog.races[*race]),
      420.0,
      112.0,
      24.0,
      WHITE,
    );
    ui.preview_frame(257.0, 132.0, 326.0, 163.0);
    if self.loaded != Some((self.selected, self.slot)) {
      self.scene = Some(crate::track_preview::load(library, catalog, *race, false)?);
      self.loaded = Some((self.selected, self.slot));
      self.start = get_time();
    }
    if let Some(scene) = &mut self.scene {
      scene.draw(
        Rect::new(260.0, 135.0, 320.0, 157.0),
        (get_time() - self.start) as f32,
      )?;
    }
    ui.banner("Circuit Race");
    ui.centered_text("Circuit Number:", 122.0, 112.0, 24.0, WHITE);
    ui.centered_text(&(self.selected + 1).to_string(), 122.0, 145.0, 24.0, WHITE);
    ui.centered_text("Hosting", 122.0, 222.0, 24.0, WHITE);
    ui.centered_text("Champion:", 122.0, 255.0, 24.0, WHITE);
    ui.centered_text(&words[..split].join(" "), 122.0, 298.0, 24.0, WHITE);
    ui.centered_text(&words[split..].join(" "), 122.0, 331.0, 24.0, WHITE);
    let unlocked = self.selected as u32 <= profile.unlocked_circuit;
    let saved = profile
      .career
      .as_ref()
      .filter(|c| c.circuit == catalog.circuits[self.selected].name && c.completed < races.len());
    for (slot, index) in races.iter().enumerate() {
      let title = catalog.title(&catalog.races[*index]);
      ui.fitted_text(
        &format!("{}. {title}", slot + 1),
        260.0,
        318.0 + slot as f32 * 22.0,
        16.0,
        350.0,
        if saved.is_some_and(|c| slot < c.completed) {
          YELLOW
        } else {
          WHITE
        },
      );
    }
    if let Some(rank) = profile
      .circuit_medals
      .get(&catalog.circuits[self.selected].name)
    {
      if let Some(trophy) = match rank {
        1 => Some("gtrophy"),
        2 => Some("strophy"),
        3 => Some("btrophy"),
        _ => None,
      } {
        ui.image(trophy, 98.0, 342.0, 36.0, 36.0);
      }
      ui.text(&format!("Best place: {rank}"), 32.0, 389.0, 16.0, YELLOW);
    }
    ui.fitted_text(
      if unlocked {
        "Gold: champion car + driver parts"
      } else {
        "Locked: finish 1st or 2nd in the previous circuit"
      },
      245.0,
      421.0,
      15.0,
      365.0,
      if unlocked { YELLOW } else { WHITE },
    );
    ui.text(
      "Left / Right: circuit   Up / Down: action",
      245.0,
      449.0,
      15.0,
      WHITE,
    );
    let labels = [
      if !unlocked {
        "Locked".into()
      } else if let Some(saved) = saved {
        format!("Resume Race {}", saved.completed + 1)
      } else {
        "Start Circuit".into()
      },
      "Main Menu".into(),
    ];
    let choice = ui.buttons(&labels, 24.0, 406.0, 40.0);
    if choice == Some(1) {
      return Ok(Action::Back);
    }
    Ok(if choice == Some(0) && unlocked {
      Action::Start(self.selected)
    } else {
      Action::None
    })
  }
}
