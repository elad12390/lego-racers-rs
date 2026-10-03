//! Original scene text and timed fullscreen fade composition at native UI scale.
use crate::platform::prelude::*;
use crate::{bitmap_text::BitmapText, cinematic_scene::Scene, menu_ui::MenuUi};
use lrformats::{cinematic_overlay::Plan, library::Library};
use std::collections::HashMap;
pub struct Overlay {
  plan: Plan,
  fonts: HashMap<String, BitmapText>,
  fps: f32,
}
impl Overlay {
  pub fn load(
    library: &Library,
    table: &str,
    cdb: &str,
    scene: &Scene,
    host: Option<&str>,
  ) -> Result<Self, String> {
    let plan = lrformats::cinematic_overlay::load(library, table, cdb, &scene.events, host)?;
    let mut fonts = HashMap::new();
    for text in &plan.texts {
      if !fonts.contains_key(&text.font) {
        fonts.insert(text.font.clone(), BitmapText::load(library, &text.font)?);
      }
    }
    Ok(Self {
      plan,
      fonts,
      fps: scene.fps,
    })
  }
  pub fn text_capture_frame(&self) -> Option<u32> {
    self
      .plan
      .texts
      .first()
      .map(|t| t.start + (t.end - t.start) / 2)
  }
  pub fn draw(&self, seconds: f32) {
    let frame = seconds * self.fps;
    let scale = MenuUi::scale();
    let origin = MenuUi::origin();
    for text in &self.plan.texts {
      if frame < text.start as f32 || frame >= text.end as f32 {
        continue;
      }
      let font = &self.fonts[&text.font];
      let size = font.native_height();
      let x = text
        .x
        .map_or((640.0 - font.width(&text.value, size)) / 2.0, |v| v * 640.0);
      let y = text.y.map_or((480.0 - size) / 2.0, |v| v * 480.0);
      font.draw(
        &text.value,
        origin.x + x * scale,
        origin.y + (y + size) * scale,
        size * scale,
        WHITE,
      );
    }
    for fade in &self.plan.fades {
      if frame < fade.start as f32 || fade.end.is_some_and(|end| frame >= end as f32) {
        continue;
      }
      let fraction =
        ((frame - fade.start as f32) * 1000.0 / (self.fps * fade.millis as f32)).clamp(0.0, 1.0);
      let alpha = if fade.fade_in {
        1.0 - fraction
      } else {
        fraction
      };
      let [r, g, b] = fade.color;
      draw_rectangle(
        origin.x,
        origin.y,
        640.0 * scale,
        480.0 * scale,
        Color::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, alpha),
      );
    }
  }
}
