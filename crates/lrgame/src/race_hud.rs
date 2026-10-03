//! Original PC sprites in a compact race HUD, source-footage composition.
//! Native layout/map-dot projection remains provisional, not platform parity.
use crate::platform::prelude::*;
use lrformats::{
  bmp,
  library::Library,
  tok::{self, Node, Value},
};
use lrsim::{
  powerups::{Powerups, Racer},
  race::Race,
};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
struct Rules {
  map_bitmap: String,
  map_rect: [f32; 4],
  power_rect: [f32; 4],
  power_icons: [[String; 4]; 4],
  racer_colors: [[u8; 3]; 6],
  dot_radius: f32,
}
pub struct RaceHud {
  rules: Rules,
  text: crate::bitmap_text::BitmapText,
  map: Texture2D,
  sprites: HashMap<String, Texture2D>,
  bounds: [f32; 4],
  mirrored: bool,
}
#[derive(Clone, Copy)]
struct Canvas {
  scale: f32,
  origin: Vec2,
}
impl Canvas {
  fn new(rect: Rect) -> Self {
    let scale = (rect.w / 640.0).min(rect.h / 480.0);
    Self {
      scale,
      origin: vec2(
        rect.x + (rect.w - 640.0 * scale) / 2.0,
        rect.y + (rect.h - 480.0 * scale) / 2.0,
      ),
    }
  }
}
fn texture(bytes: &[u8]) -> Result<Texture2D, String> {
  let image = bmp::decode(bytes).map_err(|e| e.to_string())?;
  let mut rgba = image.to_rgba();
  for pixel in rgba.chunks_exact_mut(4) {
    if pixel[..3] == [0, 0, 0] {
      pixel[3] = 0;
    }
  }
  let texture = Texture2D::from_rgba8(image.width, image.height, &rgba);
  texture.set_filter(FilterMode::Nearest);
  Ok(texture)
}
impl RaceHud {
  pub fn load(library: &Library, table: &str, mirrored: bool) -> Result<Self, String> {
    let rules: Rules = serde_json::from_str(include_str!("../../../assets/native/hud.json"))
      .map_err(|e| e.to_string())?;
    if rules
      .map_rect
      .iter()
      .chain(&rules.power_rect)
      .any(|v| !v.is_finite())
      || rules.map_rect[2..]
        .iter()
        .chain(&rules.power_rect[2..])
        .any(|v| *v <= 0.0)
      || !rules.dot_radius.is_finite()
      || rules.dot_radius <= 0.0
    {
      return Err("invalid native HUD configuration".into());
    }
    let map = texture(
      library
        .find_in(&rules.map_bitmap, table)
        .ok_or("missing original track map image")?,
    )?;
    let nodes = tok::parse(
      library
        .find_in(&format!("{table}.RAB"), table)
        .ok_or("missing original map bindings")?,
    )
    .map_err(|e| e.to_string())?;
    let body = nodes
      .iter()
      .find_map(|n| {
        if let Node::Block(b) = n {
          Some(b)
        } else {
          None
        }
      })
      .ok_or("missing race archive body")?;
    let extents = body
      .windows(2)
      .find_map(|p| {
        if let [Node::Keyword(0x46), Node::Packed { kind: 3, rows }] = p {
          Some(rows)
        } else {
          None
        }
      })
      .ok_or("missing original map extents")?;
    let values = extents
      .iter()
      .flat_map(|r| r.iter())
      .filter_map(|v| {
        if let Value::F32(f) = v {
          Some(*f)
        } else {
          None
        }
      })
      .collect::<Vec<_>>();
    let bounds: [f32; 4] = values
      .try_into()
      .map_err(|_| "invalid original map extents")?;
    if bounds.iter().any(|v| !v.is_finite()) || bounds[0] >= bounds[1] || bounds[3] >= bounds[2] {
      return Err("invalid original map range".into());
    }
    let mut sprites = HashMap::new();
    for name in rules
      .power_icons
      .iter()
      .flatten()
      .map(String::as_str)
      .chain(["holder", "holderwp", "oneenh", "twoenh", "threeenh"])
    {
      sprites.insert(
        name.into(),
        texture(
          library
            .find_in(&format!("{name}.BMP"), "COMMON")
            .ok_or_else(|| format!("missing original HUD sprite {name}"))?,
        )?,
      );
    }
    Ok(Self {
      rules,
      text: crate::bitmap_text::BitmapText::load(library, "font_ths")?,
      map,
      sprites,
      bounds,
      mirrored,
    })
  }
  fn text(&self, c: Canvas, text: &str, x: f32, y: f32, size: f32, color: Color) {
    let s = c.scale;
    let o = c.origin;
    if self.text.supports(text) {
      self.text.draw(
        text,
        o.x + (x + 1.0) * s,
        o.y + (y + 2.0) * s,
        size * s,
        BLACK,
      );
      self
        .text
        .draw(text, o.x + x * s, o.y + y * s, size * s, color);
    } else {
      draw_text(
        text,
        o.x + (x + 1.0) * s,
        o.y + (y + 2.0) * s,
        size * s,
        BLACK,
      );
      draw_text(text, o.x + x * s, o.y + y * s, size * s, color);
    }
  }
  fn centered(&self, c: Canvas, text: &str, y: f32, size: f32, color: Color) {
    let width = if self.text.supports(text) {
      self.text.width(text, size)
    } else {
      measure_text(text, None, size as u16, 1.0).width
    };
    self.text(c, text, (640.0 - width) * 0.5, y, size, color);
  }
  fn image(&self, c: Canvas, image: &Texture2D, rect: [f32; 4], flip_y: bool) {
    let s = c.scale;
    let o = c.origin;
    draw_texture_ex(
      image,
      o.x + rect[0] * s,
      o.y + rect[1] * s,
      WHITE,
      DrawTextureParams {
        dest_size: Some(vec2(rect[2] * s, rect[3] * s)),
        flip_y,
        ..Default::default()
      },
    );
  }
  pub fn draw(
    &self,
    race: &Race,
    rank: u32,
    actors: &[Racer],
    powers: Option<&Powerups>,
    countdown: Option<u32>,
    all_finished: bool,
  ) {
    self.draw_viewport(
      Rect::new(0.0, 0.0, screen_width(), screen_height()),
      0,
      race,
      rank,
      actors,
      powers,
      countdown,
      all_finished,
    );
  }
  pub fn draw_viewport(
    &self,
    viewport: Rect,
    player: usize,
    race: &Race,
    rank: u32,
    actors: &[Racer],
    powers: Option<&Powerups>,
    countdown: Option<u32>,
    all_finished: bool,
  ) {
    let c = Canvas::new(viewport);
    let suffix = match rank {
      1 => "ST",
      2 => "ND",
      3 => "RD",
      _ => "TH",
    };
    self.text(c, &rank.to_string(), 40.0, 44.0, 34.0, WHITE);
    self.text(c, suffix, 60.0, 29.0, 12.0, WHITE);
    self.text(
      c,
      &format!("LAP {}/3", (race.laps.completed_laps + 1).min(3)),
      435.0,
      28.0,
      16.0,
      WHITE,
    );
    self.text(c, &clock(race.elapsed), 520.0, 28.0, 16.0, WHITE);
    if let Some(best) = race.lap_times.iter().copied().reduce(f64::min) {
      self.text(
        c,
        &format!("BEST {}", clock(best)),
        435.0,
        48.0,
        14.0,
        WHITE,
      );
    }
    if let Some(numeral) = countdown {
      self.centered(c, &numeral.to_string(), 220.0, 72.0, YELLOW);
    } else if !race.finished && race.elapsed < 0.8 {
      // Modern short release flash; original 3000ms start gate is unchanged.
      self.centered(c, "GO!", 220.0, 62.0, YELLOW);
    } else if !race.finished
      && !race.lap_times.is_empty()
      && race.elapsed - race.lap_times.iter().sum::<f64>() < 2.0
    {
      // Original UpdateLapState's lap_banner_timer is 2000ms.
      self.centered(
        c,
        if race.laps.completed_laps == 2 {
          "FINAL LAP"
        } else {
          "LAP 2"
        },
        145.0,
        30.0,
        YELLOW,
      );
      self.centered(
        c,
        &format!("LAP TIME {}", clock(*race.lap_times.last().unwrap())),
        172.0,
        16.0,
        WHITE,
      );
    }
    if race.finished {
      self.centered(
        c,
        if rank == 1 { "YOU WIN!" } else { "FINISH" },
        145.0,
        34.0,
        YELLOW,
      );
      self.centered(
        c,
        &format!("{}{}   {}", rank, suffix, clock(race.elapsed)),
        174.0,
        20.0,
        WHITE,
      );
      if all_finished {
        self.centered(c, "ENTER FOR RESULTS", 199.0, 13.0, WHITE);
      } else {
        self.centered(c, "WAITING FOR RACERS", 199.0, 13.0, WHITE);
      }
    }
    let rect = self.rules.map_rect;
    self.image(c, &self.map, rect, self.mirrored);
    let scale = c.scale;
    let origin = c.origin;
    for (index, actor) in actors.iter().enumerate().rev() {
      let u =
        ((actor.position[0] - self.bounds[0]) / (self.bounds[1] - self.bounds[0])).clamp(0.0, 1.0);
      let mut v =
        ((self.bounds[2] - actor.position[1]) / (self.bounds[2] - self.bounds[3])).clamp(0.0, 1.0);
      if self.mirrored {
        v = 1.0 - v;
      }
      let x = origin.x + (rect[0] + u * rect[2]) * scale;
      let y = origin.y + (rect[1] + v * rect[3]) * scale;
      let rgb = self.rules.racer_colors[index % 6];
      if index == player {
        draw_circle(x, y, (self.rules.dot_radius + 2.0) * scale, WHITE);
      }
      draw_circle(x, y, (self.rules.dot_radius + 0.6) * scale, BLACK);
      draw_circle(
        x,
        y,
        self.rules.dot_radius * scale,
        Color::from_rgba(rgb[0], rgb[1], rgb[2], 255),
      );
      if index == player {
        let heading = vec2(
          actor.forward[0],
          if self.mirrored {
            actor.forward[1]
          } else {
            -actor.forward[1]
          },
        )
        .normalize_or_zero();
        let tip = vec2(x, y) + heading * (self.rules.dot_radius + 6.0) * scale;
        for fraction in [0.5, 0.75, 1.0] {
          let marker = vec2(x, y).lerp(tip, fraction);
          draw_circle(marker.x, marker.y, scale, WHITE);
        }
      }
    }
    let rect = self.rules.power_rect;
    self.image(c, &self.sprites["holder"], rect, false);
    if let Some(powers) = powers {
      let inventory = &powers.inventories[player];
      if (1..=4).contains(&inventory.kind) {
        self.image(
          c,
          &self.sprites[&self.rules.power_icons[inventory.kind as usize - 1]
            [inventory.whites.min(3) as usize]],
          rect,
          false,
        );
      }
      if inventory.whites > 0 {
        self.image(
          c,
          &self.sprites[["oneenh", "twoenh", "threeenh"][inventory.whites.min(3) as usize - 1]],
          [rect[0] - 8.0, rect[1] + 40.0, 70.0, 25.0],
          false,
        );
      }
    }
  }
}

fn clock(seconds: f64) -> String {
  let ticks = (seconds.max(0.0) * 100.0) as u64;
  format!(
    "{}:{:02}.{:02}",
    ticks / 6000,
    ticks / 100 % 60,
    ticks % 100
  )
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn race_clock_never_rounds_seconds_to_sixty() {
    assert_eq!(clock(59.999), "0:59.99");
    assert_eq!(clock(60.0), "1:00.00");
    assert_eq!(clock(123.456), "2:03.45");
  }
}
