//! License-local focus and the original one-transition-per-update caret timer.
//! Native shortcuts are separate; this is not the generic original UI event tree.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Control {
  Name,
  Snapshot,
  BuildCar,
  BuildDriver,
}
pub struct Focus {
  pub control: Control,
  active: bool,
  remaining_ms: u32,
  period_ms: u32,
}
impl Focus {
  pub fn new(period_ms: u32) -> Self {
    // CameraManAnimation0047b470 highlights the field on load without sound.
    // UiBlinkingControl reset: active=1, remaining=0.
    Self {
      control: Control::Name,
      active: true,
      remaining_ms: 0,
      period_ms,
    }
  }
  pub fn update(&mut self, hovered: Option<Control>, elapsed_ms: u32) {
    if let Some(control) = hovered {
      self.control = control;
    }
    if self.control == Control::Name {
      // UiBlinkingControl004680e0 discards overshoot, toggling at most once.
      if elapsed_ms < self.remaining_ms {
        self.remaining_ms -= elapsed_ms;
      } else {
        self.active = !self.active;
        self.remaining_ms = self.period_ms;
      }
    }
  }
  pub fn navigate(&mut self, reverse: bool) {
    // CameraManAnimation0047b300 links these four enabled/shown controls.
    // UiScrollTarget slot14 (Tab/Down) calls FocusPrev, despite its name;
    // slot18 (ShiftTab/Up) calls FocusNext. Their fallback searches wrap.
    let order = [
      Control::Name,
      Control::Snapshot,
      Control::BuildCar,
      Control::BuildDriver,
    ];
    let index = order.iter().position(|c| *c == self.control).unwrap();
    self.control = order[(index + if reverse { 3 } else { 1 }) % order.len()];
    // Highlight/unhighlight only reparent the caret; retain its blink phase.
  }
  pub fn caret_visible(&self, length: usize, capacity: usize) -> bool {
    self.control == Control::Name && self.active && length < capacity
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn original_license_tab_and_arrow_order_wrap_and_retain_caret_phase() {
    let mut focus = Focus::new(1000);
    for expected in [
      Control::Snapshot,
      Control::BuildCar,
      Control::BuildDriver,
      Control::Name,
    ] {
      focus.navigate(false);
      assert_eq!(focus.control, expected);
    }
    assert!(focus.caret_visible(4, 13));
    focus.update(None, 16);
    for expected in [
      Control::BuildDriver,
      Control::BuildCar,
      Control::Snapshot,
      Control::Name,
    ] {
      focus.navigate(true);
      assert_eq!(focus.control, expected);
    }
    assert!(!focus.caret_visible(4, 13));
  }
  #[test]
  fn original_license_caret_countdown_focus_and_full_name_visibility() {
    let capacity = lrsim::brick_build::Rules::load()
      .unwrap()
      .license_name_capacity;
    assert_eq!(capacity, 13);
    let mut focus = Focus::new(1000);
    assert!(focus.caret_visible(4, capacity));
    focus.update(None, 16);
    assert!(!focus.caret_visible(4, capacity));
    focus.update(None, 999);
    assert!(!focus.caret_visible(4, capacity));
    focus.update(None, 1);
    assert!(focus.caret_visible(12, capacity));
    assert!(!focus.caret_visible(13, capacity));
    assert!(!focus.caret_visible(14, capacity));
    focus.update(Some(Control::Snapshot), 500);
    assert!(!focus.caret_visible(4, capacity));
    focus.update(None, 9000); // Outside controls retains focus; detached timer stops.
    assert!(focus.control == Control::Snapshot);
    focus.update(Some(Control::Name), 0);
    assert!(focus.caret_visible(4, capacity));
    focus.update(None, 9000); // Long frame toggles once, not modulo elapsed time.
    assert!(!focus.caret_visible(4, capacity));
    focus.update(None, 1000);
    assert!(focus.caret_visible(4, capacity));
  }
}
