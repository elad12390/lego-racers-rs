//! Screen-local original mouse capture: activate on down, commit on inside up.
//! Keyboard shortcuts remain separate native application bindings.
#[derive(Default)]
pub struct Pointer {
  armed: Option<&'static str>,
}
#[derive(Clone, Copy)]
pub struct Frame {
  pub pressed: Option<&'static str>,
  pub active: Option<&'static str>,
  pub committed: Option<&'static str>,
}
impl Pointer {
  /// Original Unhighlight_Flags cancels an activated control before moving focus.
  pub fn cancel(&mut self) {
    self.armed = None;
  }
  pub fn update(&mut self, hovered: Option<&'static str>, down: bool, up: bool) -> Frame {
    let pressed = if down { hovered } else { None };
    if down {
      self.armed = hovered;
    }
    let committed = if up && self.armed == hovered {
      self.armed
    } else {
      None
    };
    if up {
      self.armed = None;
    }
    Frame {
      pressed,
      active: self.armed,
      committed,
    }
  }
}
#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn original_mouse_capture_commits_only_matching_inside_release() {
    let mut pointer = Pointer::default();
    let down = pointer.update(Some("snapshot"), true, false);
    assert_eq!(down.pressed, Some("snapshot"));
    assert_eq!(down.active, Some("snapshot"));
    assert_eq!(down.committed, None);
    let held = pointer.update(None, false, false);
    assert_eq!(held.active, Some("snapshot"));
    assert_eq!(held.pressed, None);
    assert_eq!(held.committed, None);
    assert_eq!(
      pointer.update(Some("snapshot"), false, true).committed,
      Some("snapshot")
    );
    assert_eq!(
      pointer.update(Some("snapshot"), false, true).committed,
      None
    );
    pointer.update(Some("snapshot"), true, false);
    assert_eq!(pointer.update(None, false, true).committed, None);
    pointer.update(Some("snapshot"), true, false);
    assert_eq!(pointer.update(Some("cancel"), false, true).committed, None);
    pointer.update(None, true, false);
    assert_eq!(
      pointer.update(Some("snapshot"), false, true).committed,
      None
    );
  }
}
