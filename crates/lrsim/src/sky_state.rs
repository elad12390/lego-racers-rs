//! SkyDatabase 0041ccb0: strict boundaries, one cyclic step, discarded excess.
use lrformats::sky::Sky;

#[derive(Clone, Default)]
struct Cursor {
  index: usize,
  elapsed_ms: u32,
}
pub struct Player {
  pub sky: Sky,
  cursors: Vec<Cursor>,
  active: usize,
  previous: usize,
  transition_ms: u32,
  transition_elapsed: u32,
}
type Colors = [[u8; 3]; 3];

fn blend(a: Colors, b: Colors, fraction: f32) -> Colors {
  // ColorRGB_LerpBytes retains the float argument but weights in x87 before
  // truncating, not rounded byte interpolation or a saturating float cast.
  let t = f64::from(fraction);
  std::array::from_fn(|i| {
    std::array::from_fn(|j| (f64::from(b[i][j]) * t + f64::from(a[i][j]) * (1.0 - t)) as i32 as u8)
  })
}
impl Player {
  pub fn new(sky: Sky) -> Self {
    let active = sky
      .profiles
      .iter()
      .position(|p| p.name == sky.default)
      .unwrap();
    Self {
      cursors: vec![Cursor::default(); sky.profiles.len()],
      sky,
      active,
      previous: 0,
      transition_ms: 0,
      transition_elapsed: 0,
    }
  }
  pub fn reset(&mut self) {
    self.cursors.fill(Cursor::default());
    self.active = self
      .sky
      .profiles
      .iter()
      .position(|p| p.name == self.sky.default)
      .unwrap();
    self.previous = 0;
    self.transition_ms = 0;
    self.transition_elapsed = 0;
  }
  pub fn select(&mut self, name: &str, transition_ms: u32) -> Result<(), String> {
    let index = self
      .sky
      .profiles
      .iter()
      .position(|p| p.name.eq_ignore_ascii_case(name))
      .ok_or_else(|| format!("missing original sky sequence {name}"))?;
    self.previous = self.active;
    self.active = index;
    // SetCurrentByName_Table leaves an in-flight timer alone when a repeated
    // event names the already active entry. It still replaces the source index.
    if self.active != self.previous {
      self.transition_ms = transition_ms;
      self.transition_elapsed = 0;
    }
    Ok(())
  }
  pub fn advance(&mut self, elapsed_ms: u32) {
    for (cursor, profile) in self.cursors.iter_mut().zip(&self.sky.profiles) {
      cursor.elapsed_ms = cursor.elapsed_ms.wrapping_add(elapsed_ms);
      if cursor.elapsed_ms > profile.frames[cursor.index].duration_ms {
        cursor.index = (cursor.index + 1) % profile.frames.len();
        cursor.elapsed_ms = 0;
      }
    }
    if self.transition_ms != 0 {
      self.transition_elapsed = self.transition_elapsed.wrapping_add(elapsed_ms);
      if self.transition_elapsed > self.transition_ms {
        self.transition_ms = 0;
        self.transition_elapsed = 0;
      }
    }
  }
  fn sequence(&self, index: usize) -> Colors {
    let profile = &self.sky.profiles[index];
    let cursor = &self.cursors[index];
    let current = &profile.frames[cursor.index];
    if profile.frames.len() == 1 {
      return current.colors;
    }
    let next = &profile.frames[(cursor.index + 1) % profile.frames.len()];
    blend(
      current.colors,
      next.colors,
      cursor.elapsed_ms as f32 / current.duration_ms as f32,
    )
  }
  pub fn colors(&self) -> Colors {
    let active = self.sequence(self.active);
    if self.transition_ms == 0 {
      active
    } else {
      blend(
        self.sequence(self.previous),
        active,
        self.transition_elapsed as f32 / self.transition_ms as f32,
      )
    }
  }
  pub fn name(&self) -> &str { &self.sky.profiles[self.active].name }
  pub fn settled(&self) -> bool {
    self.transition_ms == 0 || self.transition_elapsed >= self.transition_ms
  }
}
