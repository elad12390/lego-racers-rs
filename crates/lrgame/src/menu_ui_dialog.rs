//! Existing-racer discard with original labels/frame/button/font resources.
//! Font space/line metrics and final label/button dimensions are source-backed;
//! full descriptor initialization, modal loader and raster parity are unproved.
use super::MenuUi;
use crate::platform::prelude::*;
pub struct DialogLayout {
  pub frame: Rect,
  pub buttons: [Rect; 2],
  lines: Vec<String>,
  text_origin: Vec2,
  labels: [usize; 2],
}
impl MenuUi {
  pub fn discard_layout(&self) -> Result<DialogLayout, String> {
    self.driver_cancel_layout(crate::driver_discard::Kind::ChangedParts)
  }
  pub fn driver_cancel_layout(
    &self,
    kind: crate::driver_discard::Kind,
  ) -> Result<DialogLayout, String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    let height = self.small.native_height();
    // DIALOG.MIB dialtext width320; original font wraps at word boundaries.
    for word in self.strings.get(kind.prompt())?.split_whitespace() {
      let candidate = if line.is_empty() {
        word.to_string()
      } else {
        format!("{line} {word}")
      };
      if !line.is_empty() && self.small.width(&candidate, height) > 320.0 {
        lines.push(std::mem::take(&mut line));
        line = word.to_string();
      } else {
        line = candidate;
      }
    }
    if !line.is_empty() {
      lines.push(line);
    }
    let text_width = lines
      .iter()
      .map(|line| self.small.width(line, height))
      .fold(0.0f32, f32::max);
    let text_height = height * lines.len() as f32;
    let labels = kind.labels();
    let first = self.action_bounds_at(Vec2::ZERO, labels[0], "buttonch")?;
    let second = self.action_bounds_at(Vec2::ZERO, labels[1], "buttonca")?;
    let width = text_width + 40.0;
    let total_height = text_height + first.h + second.h + 80.0;
    // Screen_LayoutOptionsPanels00468590 centers from the logical screen halves.
    let frame = Rect::new(
      (320.0 - width * 0.5).trunc(),
      (240.0 - total_height * 0.5).trunc(),
      width,
      total_height,
    );
    let buttons = [
      Rect::new(
        frame.x + 25.0,
        frame.y + text_height + 20.0,
        first.w,
        first.h,
      ),
      Rect::new(
        frame.x + 25.0,
        frame.y + text_height + first.h + 40.0,
        second.w,
        second.h,
      ),
    ];
    Ok(DialogLayout {
      frame,
      buttons,
      lines,
      text_origin: vec2(frame.x + 20.0, frame.y + 20.0),
      labels,
    })
  }
  pub fn draw_discard(
    &self,
    layout: &DialogLayout,
    selected: usize,
    pointer: crate::menu_pointer::Frame,
  ) -> Result<(), String> {
    let frame = self
      .layouts
      .get("dialog")
      .and_then(|layout| layout.widgets.get("dialfrme"))
      .and_then(|widget| widget.frame.as_ref())
      .ok_or("missing original dialog frame")?;
    self.draw_original_frame(layout.frame, frame)?;
    let height = self.small.native_height();
    let scale = Self::scale();
    let origin = Self::origin();
    for (index, line) in layout.lines.iter().enumerate() {
      self.small.draw(
        line,
        origin.x + layout.text_origin.x * scale,
        origin.y + (layout.text_origin.y + (index + 1) as f32 * height) * scale,
        height * scale,
        Color::from_rgba(239, 239, 239, 255),
      );
    }
    self.named_action_at(
      "gonext",
      layout.labels[0],
      "buttonch",
      layout.buttons[0],
      Some(selected == 0),
      pointer,
    )?;
    self.named_action_at(
      "goback",
      layout.labels[1],
      "buttonca",
      layout.buttons[1],
      Some(selected == 1),
      pointer,
    )?;
    Ok(())
  }
}
