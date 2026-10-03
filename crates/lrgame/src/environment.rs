//! Original 33-vertex sky shell, SKB sequences and camera-relative height.
//! Named TIB sky events and RAB-owned camera-relative sky-world children.
use crate::platform::prelude::*;
use lrformats::{library::Library, sky};

pub struct Environment {
  mesh: Mesh,
  material: Material,
  player: lrsim::sky_events::Player,
  fractional_ms: f64,
  diagnostic_noise: Option<u16>,
  children: Vec<crate::static_scenery::Object>,
  animated_children: Vec<crate::animated_scenery::Object>,
}

impl Environment {
  pub fn load(library: &Library, table: &str, _radius: f32) -> Result<Self, String> {
    let owner = library
      .jam()
      .tables
      .iter()
      .find(|t| t.name.eq_ignore_ascii_case(table))
      .ok_or("missing sky table")?;
    let entry = owner
      .entries
      .iter()
      .find(|e| e.name.to_ascii_uppercase().ends_with(".SKB"))
      .ok_or("original sky file missing")?;
    let sky = sky::parse(library.jam().bytes(entry).map_err(|e| e.to_string())?)?;
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    // SkyDatabase::Load requests ControlRecord_Animate_B with radius 100,
    // eleven slices, hemisphere enabled and neither cap. Three rings: the
    // last polar angle is forced to pi/2. WriteColorGroups reverses A/B/C.
    let step = std::f32::consts::TAU / 11.0;
    for row in 0..3 {
      let polar = if row == 2 {
        std::f32::consts::FRAC_PI_2
      } else {
        step * (row + 1) as f32
      };
      for column in 0..11 {
        let longitude = step * column as f32;
        vertices.push(Vertex::new(
          100.0 * polar.sin() * longitude.cos(),
          100.0 * polar.cos(),
          -100.0 * polar.sin() * longitude.sin(),
          0.0,
          0.0,
          WHITE,
        ));
      }
    }
    for row in 0..2 {
      for column in 0..11 {
        let a = (row * 11 + column) as u16;
        let b = (row * 11 + (column + 1) % 11) as u16;
        indices.extend([a, b, a + 11, b, b + 11, a + 11]);
      }
    }
    let material = Material::sky();
    let actions = owner
      .entries
      .iter()
      .find(|e| e.name.eq_ignore_ascii_case("EVENT.EVB"))
      .map(|e| {
        library
          .jam()
          .bytes(e)
          .map_err(|e| e.to_string())
          .and_then(lrformats::environment_events::parse)
      })
      .transpose()?
      .unwrap_or_default();
    let timers = owner
      .entries
      .iter()
      .find(|e| e.name.eq_ignore_ascii_case("TIMER.TIB"))
      .map(|e| {
        library
          .jam()
          .bytes(e)
          .map_err(|e| e.to_string())
          .and_then(lrformats::timed_events::parse)
      })
      .transpose()?
      .unwrap_or_default();
    let background = owner
      .entries
      .iter()
      .find(|e| e.name.to_ascii_uppercase().ends_with(".RAB"))
      .map(|e| {
        library
          .jam()
          .bytes(e)
          .map_err(|e| e.to_string())
          .and_then(lrformats::race_archive::background_world)
      })
      .transpose()?
      .flatten();
    let (children, animated_children) = if let Some(name) = background {
      let bytes = library
        .find_in(&name, table)
        .ok_or_else(|| format!("missing RAB sky world {table}/{name}"))?;
      let mut bindings = lrformats::scene::SceneBindings::load(library, table, bytes)?;
      bindings.inherit_named_resources(&lrformats::scene::SceneBindings::race_resources(
        library, table,
      )?);
      (
        crate::static_scenery::Object::load(library, table, bytes, &bindings)?,
        crate::animated_scenery::Object::load(library, table, bytes, &bindings)?,
      )
    } else {
      (Vec::new(), Vec::new())
    };
    let mut environment = Self {
      mesh: Mesh {
        vertices,
        indices,
        texture: None,
      },
      material,
      player: lrsim::sky_events::Player::new(sky, actions, timers, || rand::gen_range(0, 1024))?,
      fractional_ms: 0.0,
      diagnostic_noise: None,
      children,
      animated_children,
    };
    environment.update(0.0)?;
    Ok(environment)
  }
  pub fn update(&mut self, dt: f32) -> Result<(), String> {
    for child in &mut self.children {
      child.update(dt)?;
    }
    for child in &mut self.animated_children {
      child.update(dt)?;
    }
    if dt.is_finite() && dt > 0.0 {
      self.fractional_ms += f64::from(dt) * 1000.0;
      let ticks = self.fractional_ms.floor() as u32;
      self.fractional_ms -= f64::from(ticks);
      let fixed = self.diagnostic_noise;
      self
        .player
        .advance(ticks, || fixed.unwrap_or_else(|| rand::gen_range(0, 1024)))?;
    }
    for (row, rgb) in self
      .mesh
      .vertices
      .chunks_exact_mut(11)
      .zip(self.player.sky.colors().into_iter().rev())
    {
      for vertex in row {
        vertex.color = [rgb[0], rgb[1], rgb[2], 255];
      }
    }
    Ok(())
  }
  pub fn reset(&mut self) -> Result<(), String> {
    for child in &mut self.children {
      child.reset()?;
    }
    for child in &mut self.animated_children {
      child.reset()?;
    }
    let fixed = self.diagnostic_noise;
    self
      .player
      .reset(|| fixed.unwrap_or_else(|| rand::gen_range(0, 1024)));
    self.fractional_ms = 0.0;
    self.update(0.0)
  }
  pub fn select(&mut self, name: &str, transition_ms: u32) -> Result<(), String> {
    self.player.sky.select(name, transition_ms)
  }
  pub fn name(&self) -> &str {
    self.player.sky.name()
  }
  pub fn dispatches(&self) -> u32 {
    self.player.named_dispatches
  }
  pub fn child_names(&self) -> Vec<&str> {
    self
      .children
      .iter()
      .map(|o| o.name.as_str())
      .chain(self.animated_children.iter().map(|o| o.name.as_str()))
      .collect()
  }
  /// Only isolated asset capture supplies a fixed stream. Live races use fresh
  /// 10-bit samples, not the copied original NoiseTable.
  pub fn diagnostic_clock_noise(&mut self, value: u16) -> Result<(), String> {
    self.diagnostic_noise = Some(value & 1023);
    self.reset()
  }
  pub fn settled(&self) -> bool {
    self.player.sky.settled()
  }
  pub fn draw(&self, eye: Vec3) {
    if !self.player.shell_visible() {
      return;
    }
    // Original SetHeightOffset_Scenery: camera Z - (10 - SKB offset).
    let eye = eye + Vec3::Y * (self.player.sky.sky.vertical_offset - 10.0);
    gl_use_material(&self.material);
    push_model_matrix(Mat4::from_translation(eye));
    draw_mesh(&self.mesh);
    pop_model_matrix();
    gl_use_default_material();
    if self.player.children_visible() {
      // The mutable source camera vector first receives offset-10, then
      // 40-offset: children receive camera Z+30, independent of SKB offset.
      let child_position = eye + Vec3::Y * (40.0 - self.player.sky.sky.vertical_offset);
      for child in &self.children {
        child.draw_sky(child_position);
      }
      for child in &self.animated_children {
        child.draw_sky(child_position);
      }
    }
  }
}
