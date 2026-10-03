//! Original named sky actions driven by the same race TIB clock as other effects.
use lrformats::{environment_events::Action, sky::Sky, timed_events::Timer};
use crate::timed_events::{Event, Player as Timers};

pub struct Player {
  pub sky: crate::sky_state::Player,
  actions: Vec<Action>,
  timers: Timers,
  pub named_dispatches: u32,
  pub flag_dispatches: u32,
  visibility_flags: u8,
}
impl Player {
  pub fn new(sky: Sky, actions: Vec<Action>, timers: Vec<Timer>, noise: impl FnMut() -> u16) -> Result<Self, String> {
    for action in &actions {
      if !action.name.is_empty() && !sky.profiles.iter().any(|p| p.name.eq_ignore_ascii_case(&action.name)) {
        return Err(format!("missing event sky profile {}", action.name));
      }
    }
    Ok(Self {sky: crate::sky_state::Player::new(sky), actions, timers: Timers::new(timers, noise),
      named_dispatches: 0, flag_dispatches: 0, visibility_flags: 0})
  }
  pub fn reset(&mut self, noise: impl FnMut() -> u16) {
    self.sky.reset(); self.timers.reset(noise);
    self.named_dispatches = 0; self.flag_dispatches = 0;
    self.visibility_flags = 0;
  }
  pub fn dispatch(&mut self, event: Event) -> Result<(), String> {
    for action in self.actions.iter().filter(|a| a.id == event.id && a.stop == event.stop) {
      if !action.name.is_empty() {
        self.sky.select(&action.name, action.transition_ms)?;
        self.named_dispatches += 1;
      }
      // NamedEntryEffect::Start applies clears before sets on each bit.
      // ptr_20 is word-addressed: +0x31 is SkyDatabase's byte at +0xc4.
      if action.flags & 1 != 0 { self.visibility_flags &= !1; }
      if action.flags & 2 != 0 { self.visibility_flags |= 1; }
      if action.flags & 4 != 0 { self.visibility_flags &= !2; }
      if action.flags & 8 != 0 { self.visibility_flags |= 2; }
      if action.flags != 0 { self.flag_dispatches += 1; }
    }
    Ok(())
  }
  /// DrawIfVisible_WithChannels: bit zero suppresses the entire sky;
  /// bit one independently suppresses only the sky world's children.
  pub fn shell_visible(&self) -> bool { self.visibility_flags & 1 == 0 }
  pub fn children_visible(&self) -> bool { self.visibility_flags & 3 == 0 }
  pub fn advance(&mut self, ticks: u32, noise: impl FnMut() -> u16) -> Result<Vec<Event>, String> {
    // RaceGame updates SkyDatabase before timer callbacks, so a selected
    // transition does not consume the triggering tick retroactively.
    self.sky.advance(ticks);
    let events = self.timers.advance(ticks, noise);
    for event in &events { self.dispatch(*event)?; }
    Ok(events)
  }
}
