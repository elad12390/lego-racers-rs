use super::state;
use bevy::math::{Mat4, Vec2, Vec3, Vec4};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
  pub r: f32,
  pub g: f32,
  pub b: f32,
  pub a: f32,
}
impl Color {
  pub fn to_vec(self) -> Vec4 {
    Vec4::new(self.r, self.g, self.b, self.a)
  }
  pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
    Self { r, g, b, a }
  }
  pub const fn from_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
    Self::new(
      r as f32 / 255.0,
      g as f32 / 255.0,
      b as f32 / 255.0,
      a as f32 / 255.0,
    )
  }
}
impl From<Color> for [u8; 4] {
  fn from(c: Color) -> Self {
    [c.r, c.g, c.b, c.a].map(|v| (v.clamp(0.0, 1.0) * 255.0) as u8)
  }
}
pub const WHITE: Color = Color::new(1.0, 1.0, 1.0, 1.0);
pub const BLACK: Color = Color::new(0.0, 0.0, 0.0, 1.0);
pub const LIGHTGRAY: Color = Color::from_rgba(200, 200, 200, 255);
pub const DARKGRAY: Color = Color::from_rgba(80, 80, 80, 255);
pub const YELLOW: Color = Color::from_rgba(253, 249, 0, 255);
pub const RED: Color = Color::from_rgba(230, 41, 55, 255);
pub const GREEN: Color = Color::from_rgba(0, 228, 48, 255);
pub const BLUE: Color = Color::from_rgba(0, 121, 241, 255);
pub const ORANGE: Color = Color::from_rgba(255, 161, 0, 255);
pub const GRAY: Color = Color::from_rgba(130, 130, 130, 255);

#[derive(Clone, Debug)]
pub struct Image {
  pub bytes: Vec<u8>,
  pub width: u16,
  pub height: u16,
}
#[derive(Clone, Debug)]
pub struct Texture2D {
  pub(crate) id: usize,
  pub(crate) width: u16,
  pub(crate) height: u16,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FilterMode {
  #[default]
  Nearest,
  Linear,
}
impl Texture2D {
  pub fn from_rgba8(width: u16, height: u16, bytes: &[u8]) -> Self {
    assert_eq!(bytes.len(), width as usize * height as usize * 4);
    let key = (width, height, state::fingerprint(bytes));
    let id = state::with(|s| {
      if let Some(ids) = s.texture_keys.get(&key) {
        if let Some(id) = ids.iter().find(|id| s.textures[**id].bytes == bytes) {
          return *id;
        }
      }
      let id = s.textures.len();
      s.textures.push(state::Texture {
        width,
        height,
        bytes: bytes.to_vec(),
        filter: FilterMode::Nearest,
      });
      s.texture_keys.entry(key).or_default().push(id);
      id
    });
    Self { id, width, height }
  }
  pub fn set_filter(&self, filter: FilterMode) {
    state::with(|s| s.textures[self.id].filter = filter);
  }
  pub fn width(&self) -> f32 {
    self.width as f32
  }
  pub fn height(&self) -> f32 {
    self.height as f32
  }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex {
  pub position: Vec3,
  pub uv: Vec2,
  pub color: [u8; 4],
  pub normal: Vec4,
}
impl Vertex {
  pub fn new(x: f32, y: f32, z: f32, u: f32, v: f32, color: Color) -> Self {
    Self {
      position: Vec3::new(x, y, z),
      uv: Vec2::new(u, v),
      color: color.into(),
      normal: Vec4::ZERO,
    }
  }
}
#[derive(Clone, Debug)]
pub struct Mesh {
  pub vertices: Vec<Vertex>,
  pub indices: Vec<u16>,
  pub texture: Option<Texture2D>,
}
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct Rect {
  pub x: f32,
  pub y: f32,
  pub w: f32,
  pub h: f32,
}
impl Rect {
  pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
    Self { x, y, w, h }
  }
  pub fn contains(self, p: Vec2) -> bool {
    p.x >= self.x && p.y >= self.y && p.x < self.x + self.w && p.y < self.y + self.h
  }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Camera3D {
  pub position: Vec3,
  pub target: Vec3,
  pub up: Vec3,
  pub fovy: f32,
  pub aspect: Option<f32>,
  pub z_near: f32,
  pub z_far: f32,
  pub viewport: Option<(i32, i32, i32, i32)>,
}
impl Default for Camera3D {
  fn default() -> Self {
    Self {
      position: Vec3::new(0.0, 0.0, 1.0),
      target: Vec3::ZERO,
      up: Vec3::Y,
      fovy: 45.0f32.to_radians(),
      aspect: None,
      z_near: 0.01,
      z_far: 1000.0,
      viewport: None,
    }
  }
}
#[derive(Clone, Copy, Default, Debug)]
pub struct DrawTextureParams {
  pub dest_size: Option<Vec2>,
  pub source: Option<Rect>,
  pub rotation: f32,
  pub flip_x: bool,
  pub flip_y: bool,
  pub pivot: Option<Vec2>,
}
#[derive(Clone, Copy, Default, Debug)]
pub struct TextDimensions {
  pub width: f32,
  pub height: f32,
  pub offset_y: f32,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Material {
  pub blend: Option<[u8; 2]>,
  pub depth_write: bool,
  pub depth_test: bool,
  pub cull: bool,
}
impl Material {
  pub fn scene(blend: Option<[u8; 2]>) -> Self {
    Self {
      blend,
      depth_write: blend.is_none(),
      depth_test: true,
      cull: true,
    }
  }
  pub fn sky() -> Self {
    Self {
      blend: None,
      depth_write: false,
      depth_test: false,
      cull: false,
    }
  }
  pub fn overlay() -> Self {
    Self {
      blend: Some([6, 8]),
      depth_write: false,
      depth_test: false,
      cull: false,
    }
  }
}
pub(crate) fn transform_position(m: Mat4, p: Vec3) -> Vec3 {
  m.transform_point3(p)
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn bevy_boundary_reuses_exact_assets_when_a_menu_or_race_is_reloaded() {
    let a = Texture2D::from_rgba8(1, 1, &[255, 0, 0, 255]);
    let b = Texture2D::from_rgba8(1, 1, &[255, 0, 0, 255]);
    let c = Texture2D::from_rgba8(1, 1, &[0, 255, 0, 255]);
    assert_eq!(a.id, b.id);
    assert_ne!(a.id, c.id);
  }
}
