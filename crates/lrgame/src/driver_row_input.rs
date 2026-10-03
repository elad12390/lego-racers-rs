//! Driver-row input semantics verified by `tools/test_driver_mouse_repeat_x86.py`.
//! No repeat timer: UiFramedPanel::OnPressed (00467a00) retries the captured
//! arrow every update; the existing selector scroll gate accepts or rejects it.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arrow {
  Previous,
  Next,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DriverRowInput {
  capture: Option<(u32, Arrow)>,
}

impl DriverRowInput {
  /// Original panel unhighlight00467320/00472200 deactivates both arrows.
  pub fn cancel(&mut self) {
    self.capture = None;
  }
  /// Call after hit testing an enabled row arrow. `repeat` is event byte +0xd,
  /// not whether the selector is busy. Capture survives a rejected scroll.
  /// 00467b50 -> 00467560; a held event cannot begin or switch capture.
  pub fn arrow_event(&mut self, code: u32, arrow: Arrow, repeat: bool) -> Option<Arrow> {
    match self.capture {
      None if !repeat => {
        self.capture = Some((code, arrow));
        Some(arrow)
      }
      Some((captured_code, captured_arrow))
        if repeat && captured_code == code && captured_arrow == arrow =>
      {
        Some(arrow)
      }
      _ => None,
    }
  }

  /// 00467a00: retry without needing another mouse event or cursor hit test.
  pub fn frame_arrow(&self) -> Option<Arrow> {
    self.capture.map(|(_, arrow)| arrow)
  }

  /// 00467be0: matching release clears capture even outside either arrow.
  /// Return true if this row owned the release.
  pub fn release(&mut self, code: u32) -> bool {
    if self.capture.is_some_and(|(captured, _)| captured == code) {
      self.capture = None;
      true
    } else {
      false
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThumbnailSelection {
  pub selected: usize,
  pub wrapped_origin: usize,
  /// 00484100 writes selection/origin while busy, but skips rebuilding slots.
  pub rebuild_now: bool,
}

/// Call only after the original row bounds AND an authored slot rectangle hit.
/// `count` is the unlocked choice count, not the five visible slots; indices are
/// positions in that choice list. Busy clicks are deliberately NOT discarded.
/// 0046d5c0 -> WrapIndex_Modulo0046c9a0 -> SelectItemByIndex00484100.
pub fn thumbnail_selection(
  selected: usize,
  scroll_origin: usize,
  authored_slot: usize,
  count: usize,
  busy: bool,
) -> Option<ThumbnailSelection> {
  if count == 0 {
    return None;
  }
  let count = count as i128;
  let selected =
    (selected as i128 - scroll_origin as i128 + authored_slot as i128).rem_euclid(count);
  Some(ThumbnailSelection {
    selected: selected as usize,
    wrapped_origin: (selected - scroll_origin as i128).rem_euclid(count) as usize,
    rebuild_now: !busy,
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  const MOUSE: u32 = 0x20000000;

  #[test]
  fn captured_arrow_survives_busy_rejection_and_ignores_pointer_switch() {
    let mut input = DriverRowInput::default();
    assert_eq!(input.arrow_event(MOUSE, Arrow::Next, true), None);
    assert_eq!(
      input.arrow_event(MOUSE, Arrow::Next, false),
      Some(Arrow::Next)
    );
    // The caller's busy selector rejects this attempt; capture remains.
    assert_eq!(input.arrow_event(MOUSE, Arrow::Previous, true), None);
    assert_eq!(input.frame_arrow(), Some(Arrow::Next));
    assert!(!input.release(MOUSE + 1));
    assert_eq!(input.frame_arrow(), Some(Arrow::Next));
    assert!(input.release(MOUSE));
    assert_eq!(input.frame_arrow(), None);
  }

  #[test]
  fn released_then_opposite_press_during_scroll_retries_opposite() {
    let mut input = DriverRowInput::default();
    input.arrow_event(MOUSE, Arrow::Next, false);
    input.release(MOUSE);
    input.arrow_event(MOUSE, Arrow::Previous, false);
    assert_eq!(input.frame_arrow(), Some(Arrow::Previous));
  }

  #[test]
  fn focus_loss_cancels_captured_mouse_or_key_without_undoing_selection() {
    for code in [MOUSE, 0x100000cb, 0x100000cd] {
      let mut input = DriverRowInput::default();
      input.arrow_event(code, Arrow::Next, false);
      input.cancel();
      assert_eq!(input.frame_arrow(), None);
      assert!(!input.release(code));
      assert_eq!(input.arrow_event(code, Arrow::Next, true), None);
    }
  }

  #[test]
  fn authored_slots_wrap_in_choice_space_without_starting_animation() {
    assert_eq!(
      thumbnail_selection(0, 2, 1, 6, false),
      Some(ThumbnailSelection {
        selected: 5,
        wrapped_origin: 3,
        rebuild_now: true,
      })
    );
    assert_eq!(
      thumbnail_selection(1, 2, 3, 6, true),
      Some(ThumbnailSelection {
        selected: 2,
        wrapped_origin: 0,
        rebuild_now: false,
      })
    );
    assert_eq!(thumbnail_selection(0, 2, 1, 0, false), None);
  }
}
