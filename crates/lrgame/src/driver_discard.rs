//! Existing-racer changed-part discard; new-racer record recycling is separate.
use crate::{
  menu_keyboard::{Key, Keyboard},
  menu_pointer::{Frame, Pointer},
  menu_ui::MenuUi,
  platform::prelude::*,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
  Pending,
  KeepEditing,
  Discard,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kind {
  #[default]
  ChangedParts,
  NewRacer,
}
impl Kind {
  pub fn prompt(self) -> usize {
    match self {
      Self::ChangedParts => 0x7b,
      Self::NewRacer => 0x77,
    }
  }
  pub fn labels(self) -> [usize; 2] {
    match self {
      Self::ChangedParts => [0x20, 0x1f],
      Self::NewRacer => [0x73, 0x74],
    }
  }
}

pub struct Dialog {
  selected: usize,
  pointer: Pointer,
  keyboard: Keyboard,
  kind: Kind,
}
impl Default for Dialog {
  fn default() -> Self {
    // Original004684e0 selects second button for extra_context=0.
    Self {
      selected: 1,
      pointer: Pointer::default(),
      keyboard: Keyboard::default(),
      kind: Kind::ChangedParts,
    }
  }
}
impl Dialog {
  pub fn new_racer() -> Self {
    Self {
      kind: Kind::NewRacer,
      ..Self::default()
    }
  }
  pub fn kind(&self) -> Kind {
    self.kind
  }
  pub fn selected(&self) -> usize {
    self.selected
  }
  pub fn update(&mut self, ui: &MenuUi, bounds: [Rect; 2]) -> Frame {
    let mouse = (Vec2::from(mouse_position()) - MenuUi::origin()) / MenuUi::scale();
    let hovered = bounds
      .iter()
      .position(|rect| MenuUi::editor_contains(*rect, mouse));
    let widgets = ["gonext", "goback"];
    let mut pointer = self.pointer.update(
      hovered.map(|i| widgets[i]),
      is_mouse_button_pressed(MouseButton::Left),
      is_mouse_button_released(MouseButton::Left),
    );
    let old = self.selected;
    if let Some(active) = pointer.active {
      self.selected = usize::from(active == "goback");
    } else if mouse_delta_position().length_squared() > 0.0 {
      if let Some(hovered) = hovered {
        self.selected = hovered;
      }
    }
    if is_key_pressed(KeyCode::Tab) || is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::Down)
    {
      self.selected = 1 - self.selected;
      self.pointer.cancel();
      self.keyboard.cancel();
      pointer = self.pointer.update(None, false, false);
    }
    if self.selected != old {
      self.keyboard.cancel();
      if let Some(audio) = &ui.audio {
        audio.moved();
      }
    }
    let key = |enter, space| {
      if enter {
        Some(Key::Enter)
      } else if space {
        Some(Key::Space)
      } else {
        None
      }
    };
    let released = get_keys_released();
    let keyboard = self.keyboard.update(
      Some(widgets[self.selected]),
      key(
        is_key_pressed(KeyCode::Enter),
        is_key_pressed(KeyCode::Space),
      ),
      key(
        released.contains(&KeyCode::Enter),
        released.contains(&KeyCode::Space),
      ),
    );
    Frame {
      pressed: pointer.pressed.or(keyboard.pressed),
      active: pointer.active.or(keyboard.active),
      committed: pointer.committed.or(keyboard.committed),
    }
  }
  pub fn outcome(frame: Frame) -> Outcome {
    match frame.committed {
      Some("gonext") => Outcome::Discard,
      Some("goback") => Outcome::KeepEditing,
      _ => Outcome::Pending,
    }
  }
}
#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn default_cancels_discard_and_no_unmatched_event_exits() {
    assert_eq!(Dialog::default().selected(), 1);
    for (widget, result) in [
      (None, Outcome::Pending),
      (Some("goback"), Outcome::KeepEditing),
      (Some("gonext"), Outcome::Discard),
    ] {
      assert_eq!(
        Dialog::outcome(Frame {
          pressed: None,
          active: None,
          committed: widget
        }),
        result
      );
    }
  }

  #[test]
  fn new_process_has_its_own_prompt_and_safe_second_button_default() {
    let dialog = Dialog::new_racer();
    assert_eq!(dialog.selected(), 1);
    assert_eq!(dialog.kind(), Kind::NewRacer);
    assert_eq!(dialog.kind().prompt(), 0x77);
    assert_eq!(dialog.kind().labels(), [0x73, 0x74]);
    assert_eq!(Dialog::default().kind().labels(), [0x20, 0x1f]);
  }
}
