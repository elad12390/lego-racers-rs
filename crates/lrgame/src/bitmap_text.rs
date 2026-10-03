//! Native GPU draws using the game's original English bitmap fonts.
use crate::platform::prelude::*;
use lrformats::{
  bitmap_font::{self, Glyph},
  bmp,
  library::Library,
};
use std::collections::HashMap;
pub struct BitmapText {
  texture: Texture2D,
  glyphs: HashMap<char, Glyph>,
  height: f32,
  spacing: f32,
  space_advance: f32,
}
impl BitmapText {
  pub fn load(library: &Library, name: &str) -> Result<Self, String> {
    let fonts = bitmap_font::parse(
      library
        .find_at("GFONTS.FDB", "MENUDATA", "ENGLISH")
        .ok_or("missing original menu fonts")?,
    )?;
    let spec = fonts
      .iter()
      .find(|f| f.name.eq_ignore_ascii_case(name))
      .ok_or("original font not in FDB")?;
    let image = bmp::decode(
      library
        .find_at(&format!("{}.BMP", spec.name), "MENUDATA", "ENGLISH")
        .ok_or("missing original font image")?,
    )
    .map_err(|e| e.to_string())?;
    let glyphs = bitmap_font::glyphs(spec, &image)?
      .into_iter()
      .map(|g| (g.character, g))
      .collect();
    // FONT_THS/FONT_EMB metrics are backed by unchanged GolDP prefixes over
    // actual pixels. Preserve other strips until their mapping is verified.
    let space_advance =
      if name.eq_ignore_ascii_case("font_ths") || name.eq_ignore_ascii_case("font_emb") {
        f32::from(bitmap_font::leading_space_width(&image)?)
      } else {
        image.height as f32 * 0.45
      };
    let mut bytes = image.to_rgba();
    for pixel in bytes.chunks_exact_mut(4) {
      if pixel[..3] == spec.color_key {
        pixel[3] = 0;
      }
    }
    let texture = Texture2D::from_rgba8(image.width, image.height, &bytes);
    texture.set_filter(FilterMode::Nearest);
    Ok(Self {
      texture,
      glyphs,
      height: image.height as f32,
      spacing: spec.spacing as f32,
      space_advance,
    })
  }
  pub fn supports(&self, text: &str) -> bool {
    text
      .chars()
      .all(|c| c == ' ' || self.glyphs.contains_key(&c.to_ascii_uppercase()))
  }
  pub fn native_height(&self) -> f32 {
    self.height
  }
  pub fn width(&self, text: &str, size: f32) -> f32 {
    let scale = size / self.height;
    text
      .chars()
      .map(|c| {
        if c == ' ' {
          self.space_advance * scale
        } else {
          self.glyphs.get(&c.to_ascii_uppercase()).map_or(0.0, |g| {
            (g.width as f32 + self.spacing) * size / self.height
          })
        }
      })
      .sum()
  }
  pub fn draw(&self, text: &str, mut x: f32, y: f32, size: f32, color: Color) {
    let scale = size / self.height;
    for character in text.chars().map(|c| c.to_ascii_uppercase()) {
      if character == ' ' {
        x += self.space_advance * scale;
        continue;
      }
      if let Some(glyph) = self
        .glyphs
        .get(&character)
        .or_else(|| self.glyphs.get(&'?'))
      {
        draw_texture_ex(
          &self.texture,
          x,
          y - size,
          color,
          DrawTextureParams {
            source: Some(Rect::new(
              glyph.x as f32,
              0.0,
              glyph.width as f32,
              self.height,
            )),
            dest_size: Some(vec2(glyph.width as f32 * scale, size)),
            ..Default::default()
          },
        );
        x += (glyph.width as f32 + self.spacing) * scale;
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn original_embossed_license_font_keeps_authored_ink_and_keys_only_its_authored_background() {
    let library = Library::open(
      std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../extracted/Program_Files_Group/LEGO.JAM"),
    )
    .unwrap();
    let fonts = bitmap_font::parse(
      library
        .find_at("GFONTS.FDB", "MENUDATA", "ENGLISH")
        .unwrap(),
    )
    .unwrap();
    for (name, key) in [
      ("fontmenu", [0; 3]),
      ("font_ths", [0; 3]),
      ("font_emb", [187, 195, 245]),
    ] {
      let spec = fonts.iter().find(|s| s.name == name).unwrap();
      assert_eq!(spec.color_key, key);
      let font = BitmapText::load(&library, name).unwrap();
      assert!(font.supports("ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 "));
      if name == "font_ths" || name == "font_emb" {
        assert_eq!(font.native_height(), 32.0);
        assert_eq!(
          font.width(" ", 32.0),
          8.0,
          "verified original space advance"
        );
        assert_eq!(font.width("A A", 32.0), font.width("AA", 32.0) + 8.0);
      }
      let original = bmp::decode(
        library
          .find_at(&format!("{name}.BMP"), "MENUDATA", "ENGLISH")
          .unwrap(),
      )
      .unwrap()
      .to_rgba();
      let rendered = crate::platform::test_state(|s| s.textures[font.texture.id].bytes.clone());
      let mut keyed = 0;
      let mut retained_ink = 0;
      for (before, after) in original.chunks_exact(4).zip(rendered.chunks_exact(4)) {
        assert_eq!(&after[..3], &before[..3]);
        assert_eq!(after[3], if before[..3] == key { 0 } else { before[3] });
        keyed += usize::from(after[3] == 0);
        retained_ink += usize::from(after[3] != 0);
      }
      assert!(keyed > 100);
      if name == "font_emb" {
        assert!(
          retained_ink > 100,
          "original embossed glyphs must not become transparent"
        );
        assert!(font.width("GPU RACER 1234", font.native_height()) <= 268.0);
        assert!(font.native_height() <= 34.0);
      }
    }
  }
}
