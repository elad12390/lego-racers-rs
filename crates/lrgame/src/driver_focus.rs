//! Driver-local original focus order; not the generic original UI event tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
  Hat,
  Face,
  Torso,
  Legs,
  Mix,
  License,
  Cancel,
}
impl Control {
  pub fn navigate(self, reverse: bool) -> Self {
    // CarBuilderAnimation0047d2f0 creation order; UiScrollTarget slot14
    // Tab/Down calls FocusPrev on prepended siblings, slot18 reverses.
    let order = [
      Self::Hat,
      Self::Face,
      Self::Torso,
      Self::Legs,
      Self::Mix,
      Self::License,
      Self::Cancel,
    ];
    let index = order.iter().position(|c| *c == self).unwrap();
    order[(index + if reverse { order.len() - 1 } else { 1 }) % order.len()]
  }
  pub fn row(self) -> Option<usize> {
    match self {
      Self::Hat => Some(0),
      Self::Face => Some(1),
      Self::Torso => Some(2),
      Self::Legs => Some(3),
      _ => None,
    }
  }
  pub fn from_row(row: usize) -> Self {
    [Self::Hat, Self::Face, Self::Torso, Self::Legs][row]
  }
  pub fn from_widget(widget: &str) -> Option<Self> {
    match widget {
      "mix" => Some(Self::Mix),
      "gonext" => Some(Self::License),
      "goback" => Some(Self::Cancel),
      _ => None,
    }
  }
  pub fn widget(self) -> Option<&'static str> {
    match self {
      Self::Mix => Some("mix"),
      Self::License => Some("gonext"),
      Self::Cancel => Some("goback"),
      _ => None,
    }
  }
}
#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn original_driver_focus_includes_bottom_actions_and_wraps_both_ways() {
    let mut focus = Control::Hat;
    for expected in [
      Control::Face,
      Control::Torso,
      Control::Legs,
      Control::Mix,
      Control::License,
      Control::Cancel,
      Control::Hat,
    ] {
      focus = focus.navigate(false);
      assert_eq!(focus, expected);
    }
    for expected in [
      Control::Cancel,
      Control::License,
      Control::Mix,
      Control::Legs,
      Control::Torso,
      Control::Face,
      Control::Hat,
    ] {
      focus = focus.navigate(true);
      assert_eq!(focus, expected);
    }
    assert_eq!(Control::Mix.row(), None);
    assert_eq!(Control::Hat.widget(), None);
  }
}
