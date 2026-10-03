//! Original editor buttons arm on key down and commit on matching release.
//! Name editing and driver row shortcuts are separate; pass their focus as `None`.

use crate::menu_pointer::Frame;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
  Enter,
  Space,
}

#[derive(Default)]
pub struct Keyboard {
  armed: Option<(&'static str, Key)>,
}

impl Keyboard {
  /// An accepted down owns pointer capture until matching release or cancel.
  pub fn captured(&self) -> Option<&'static str> {
    self.armed.map(|(widget, _)| widget)
  }

  /// Cancel on navigation, screen exit, or focus loss, including intermediate
  /// focus changes that return to the same widget before the next update.
  pub fn cancel(&mut self) {
    self.armed = None;
  }

  /// License and driver bottom actions share these original button semantics.
  /// Do not route editable fields or selector panels through this state.
  pub fn update(
    &mut self,
    widget: Option<&'static str>,
    pressed: Option<Key>,
    released: Option<Key>,
  ) -> Frame {
    let widget = widget.filter(|id| matches!(*id, "swapface" | "mix" | "gonext" | "goback"));
    if self.armed.map(|(id, _)| id) != widget {
      self.cancel();
    }
    let pressed = if self.armed.is_none() {
      if let Some(pair) = widget.zip(pressed) {
        self.armed = Some(pair);
        Some(pair.0)
      } else {
        None
      }
    } else {
      None
    };
    let committed = match self.armed {
      Some((id, key)) if released == Some(key) => {
        self.cancel();
        Some(id)
      }
      _ => None,
    };
    Frame {
      pressed,
      active: self.armed.map(|(id, _)| id),
      committed,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn every_button_activates_once_then_commits_only_on_matching_release() {
    for widget in ["swapface", "mix", "gonext", "goback"] {
      for key in [Key::Enter, Key::Space] {
        let mut keyboard = Keyboard::default();
        assert_eq!(keyboard.captured(), None);
        let down = keyboard.update(Some(widget), Some(key), None);
        assert_eq!(keyboard.captured(), Some(widget));
        assert_eq!(down.pressed, Some(widget));
        assert_eq!(down.active, Some(widget));
        assert_eq!(down.committed, None);
        for _ in 0..3 {
          let repeat = keyboard.update(Some(widget), Some(key), None);
          assert_eq!(repeat.pressed, None);
          assert_eq!(repeat.active, Some(widget));
          assert_eq!(repeat.committed, None);
        }
        let up = keyboard.update(Some(widget), None, Some(key));
        assert_eq!(keyboard.captured(), None);
        assert_eq!(up.pressed, None);
        assert_eq!(up.active, None);
        assert_eq!(up.committed, Some(widget));
        assert_eq!(
          keyboard.update(Some(widget), None, Some(key)).committed,
          None
        );
      }
    }
  }

  #[test]
  fn other_key_cannot_steal_or_release_armed_button() {
    let mut keyboard = Keyboard::default();
    keyboard.update(Some("swapface"), Some(Key::Enter), None);
    let other = keyboard.update(Some("swapface"), Some(Key::Space), Some(Key::Space));
    assert_eq!(other.pressed, None);
    assert_eq!(other.active, Some("swapface"));
    assert_eq!(other.committed, None);
    assert_eq!(keyboard.captured(), Some("swapface"));
    assert_eq!(
      keyboard
        .update(Some("swapface"), None, Some(Key::Enter))
        .committed,
      Some("swapface")
    );
  }

  #[test]
  fn focus_change_name_and_focus_loss_cancel_without_committing() {
    for target in [Some("gonext"), Some("name"), None] {
      let mut keyboard = Keyboard::default();
      keyboard.update(Some("swapface"), Some(Key::Enter), None);
      let changed = keyboard.update(target, None, Some(Key::Enter));
      assert_eq!(changed.pressed, None);
      assert_eq!(changed.active, None);
      assert_eq!(changed.committed, None);
      assert_eq!(keyboard.captured(), None);
      assert_eq!(
        keyboard
          .update(Some("swapface"), None, Some(Key::Enter))
          .committed,
        None
      );
    }
    let mut keyboard = Keyboard::default();
    for target in [Some("name"), None] {
      let name = keyboard.update(target, Some(Key::Enter), Some(Key::Enter));
      assert_eq!(name.pressed, None);
      assert_eq!(name.active, None);
      assert_eq!(name.committed, None);
    }
  }

  #[test]
  fn explicit_cancel_covers_intermediate_navigation_back_to_same_button() {
    let mut keyboard = Keyboard::default();
    keyboard.update(Some("gonext"), Some(Key::Space), None);
    keyboard.cancel();
    assert_eq!(
      keyboard
        .update(Some("gonext"), None, Some(Key::Space))
        .committed,
      None
    );
    let fresh = keyboard.update(Some("gonext"), Some(Key::Space), Some(Key::Space));
    assert_eq!(fresh.pressed, Some("gonext"));
    assert_eq!(fresh.active, None);
    assert_eq!(fresh.committed, Some("gonext"));
  }
}
