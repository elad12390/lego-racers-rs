use super::{
  audio::AudioCommand,
  input::{KeyCode, MouseButton},
  types::*,
};
use bevy::math::Mat4;
use std::{
  cell::RefCell,
  collections::{HashSet, VecDeque},
};

pub struct Texture {
  pub width: u16,
  pub height: u16,
  pub bytes: Vec<u8>,
  pub filter: FilterMode,
}
#[derive(Clone)]
pub struct Batch {
  pub mesh: Mesh,
  pub camera: Option<Camera3D>,
  pub material: Material,
}
pub struct State {
  pub textures: Vec<Texture>,
  pub batches: Vec<Batch>,
  pub dirty: bool,
  pub background: Color,
  pub texture_keys: std::collections::HashMap<(u16, u16, u64), Vec<usize>>,
  pub camera: Option<Camera3D>,
  pub material: Option<Material>,
  pub transforms: Vec<Mat4>,
  pub width: f32,
  pub height: f32,
  pub scale: f32,
  pub dt: f32,
  pub time: f64,
  pub frame: u64,
  pub focused: bool,
  pub quit: bool,
  pub down: HashSet<KeyCode>,
  pub pressed: HashSet<KeyCode>,
  pub released: HashSet<KeyCode>,
  pub mouse_down: HashSet<MouseButton>,
  pub mouse_pressed: HashSet<MouseButton>,
  pub mouse_released: HashSet<MouseButton>,
  pub mouse: (f32, f32),
  pub mouse_delta: bevy::math::Vec2,
  pub chars: VecDeque<char>,
  pub capture_requested: bool,
  pub capture_inflight: bool,
  pub readback: Option<Image>,
  pub audio_bytes: Vec<Vec<u8>>,
  pub audio_commands: Vec<AudioCommand>,
  pub audio_keys: std::collections::HashMap<u64, Vec<usize>>,
  pub font: fontdue::Font,
  pub glyphs: std::collections::HashMap<(char, u32), Texture2D>,
}
impl Default for State {
  fn default() -> Self {
    Self {
      textures: vec![],
      texture_keys: Default::default(),
      batches: vec![],
      dirty: false,
      background: BLACK,
      camera: None,
      material: None,
      transforms: vec![Mat4::IDENTITY],
      width: 1000.0,
      height: 760.0,
      scale: 1.0,
      dt: 1.0 / 60.0,
      time: 0.0,
      frame: 0,
      focused: true,
      quit: false,
      down: HashSet::new(),
      pressed: HashSet::new(),
      released: HashSet::new(),
      mouse_down: HashSet::new(),
      mouse_pressed: HashSet::new(),
      mouse_released: HashSet::new(),
      mouse: (0.0, 0.0),
      mouse_delta: bevy::math::Vec2::ZERO,
      chars: VecDeque::new(),
      capture_requested: false,
      capture_inflight: false,
      readback: None,
      audio_bytes: vec![],
      audio_keys: Default::default(),
      audio_commands: vec![],
      font: fontdue::Font::from_bytes(
        bevy::text::DEFAULT_FONT_DATA,
        fontdue::FontSettings::default(),
      )
      .expect("embedded fallback font"),
      glyphs: Default::default(),
    }
  }
}
thread_local! {static STATE:RefCell<State>=RefCell::new(State::default());}
pub fn with<T>(f: impl FnOnce(&mut State) -> T) -> T {
  STATE.with(|s| f(&mut s.borrow_mut()))
}
pub fn fingerprint(bytes: &[u8]) -> u64 {
  use std::hash::{Hash, Hasher};
  let mut hasher = std::collections::hash_map::DefaultHasher::new();
  bytes.hash(&mut hasher);
  hasher.finish()
}
