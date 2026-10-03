//! Editable original stud-cell car layout. Native save never modifies .LRS.
//! Attachment/clearance validation is provisional, not original-builder parity.
use lrformats::{
  brick_database::{Brick, BrickDatabase},
  library::Library,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct PlacedBrick {
  pub name: String,
  pub color: String,
  pub x: u8,
  pub y: u8,
  pub z: u8,
  pub rotation: u8,
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct Build {
  pub chassis: String,
  pub color: String,
  pub bricks: Vec<PlacedBrick>,
}
#[derive(Deserialize)]
pub struct Rules {
  pub database: String,
  pub materials: String,
  pub default_chassis: String,
  pub max_parts: usize,
  pub max_height: u8,
  pub vertical_step: f32,
  pub geometry_scale: f32,
  pub preview_ambient: f32,
  pub preview_light: [f32; 3],
  pub driver_preview_ambient: u8,
  pub driver_preview_key: u8,
  pub license_preview_ambient: u8,
  pub license_preview_key: u8,
  pub driver_animation_file: String,
  pub driver_idle_clip: String,
  pub license_animation_file: String,
  pub license_expressions: Vec<String>,
  pub license_name_capacity: usize,
  pub license_name_font: String,
  pub license_caret_period_ms: u32,
  pub license_preview_position: [f32; 3],
  pub license_preview_forward: [f32; 3],
  pub license_preview_up: [f32; 3],
  pub colors: Vec<String>,
}
pub struct BuilderData {
  pub database: BrickDatabase,
  pub rules: Rules,
}
#[derive(Clone, Copy)]
pub struct Cell {
  pub x: u8,
  pub y: u8,
  pub bottom: u8,
  pub top: u8,
}
impl Rules {
  pub fn load() -> Result<Self, String> {
    let rules: Rules = serde_json::from_str(include_str!("../../../assets/native/builder.json"))
      .map_err(|e| e.to_string())?;
    if rules.max_parts == 0
      || rules.max_parts > 80
      || rules.max_height == 0
      || !rules.vertical_step.is_finite()
      || rules.vertical_step <= 0.0
      || !rules.geometry_scale.is_finite()
      || rules.geometry_scale <= 0.0
      || !(0.0..=1.0).contains(&rules.preview_ambient)
      || rules.preview_light.iter().any(|v| !v.is_finite())
      || crate::contact::length(rules.preview_light) == 0.0
      || rules.colors.is_empty()
      || rules.driver_animation_file.is_empty()
      || rules.driver_idle_clip.is_empty()
      || rules.license_animation_file.is_empty()
      || rules.license_expressions.len() != 6
      || rules.license_name_capacity == 0
      || rules.license_name_capacity > 13
      || rules.license_name_font.is_empty()
      || rules.license_caret_period_ms == 0
      || rules
        .license_preview_position
        .iter()
        .any(|v| !v.is_finite())
      || rules.license_preview_forward.iter().any(|v| !v.is_finite())
      || rules.license_preview_up.iter().any(|v| !v.is_finite())
      || crate::contact::length(crate::contact::cross(
        rules.license_preview_forward,
        rules.license_preview_up,
      )) == 0.0
      || rules.license_expressions.iter().any(|name| {
        name.is_empty() || name.len() > 5 || !name.bytes().all(|c| c.is_ascii_lowercase())
      })
    {
      return Err("invalid native builder rules".into());
    }
    Ok(rules)
  }
}
impl BuilderData {
  pub fn load(library: &Library) -> Result<Self, String> {
    let rules = Rules::load()?;
    let database = BrickDatabase::parse(
      library
        .find_at(&rules.database, "MENUDATA", "PIECEDB")
        .ok_or("missing original brick database")?,
    )?;
    database.find(&rules.default_chassis)?;
    Ok(Self { database, rules })
  }
  pub fn default_build(&self) -> Build {
    Build {
      chassis: self.rules.default_chassis.clone(),
      color: "ltgray".into(),
      bricks: Vec::new(),
    }
  }
  pub fn cells(&self, part: &PlacedBrick) -> Result<Vec<Cell>, String> {
    if part.rotation > 3 || !self.rules.colors.contains(&part.color) {
      return Err("invalid saved brick rotation/color".into());
    }
    let brick = self.database.find(&part.name)?;
    let mut cells = Vec::new();
    for x in 0..brick.width {
      for y in 0..brick.depth {
        let [top, bottom] = brick.cells[x as usize * brick.depth as usize + y as usize];
        let top = top & 63;
        let bottom = bottom & 63;
        if top <= bottom {
          continue;
        }
        let (x, y) = match part.rotation {
          0 => (x, y),
          1 => (y, brick.width - 1 - x),
          2 => (brick.width - 1 - x, brick.depth - 1 - y),
          _ => (brick.depth - 1 - y, x),
        };
        cells.push(Cell {
          x: part.x.checked_add(x).ok_or("brick X overflow")?,
          y: part.y.checked_add(y).ok_or("brick Y overflow")?,
          bottom: part.z.checked_add(bottom).ok_or("brick bottom overflow")?,
          top: part.z.checked_add(top).ok_or("brick top overflow")?,
        });
      }
    }
    Ok(cells)
  }
  pub fn add(&self, build: &mut Build, part: PlacedBrick) -> Result<(), String> {
    if build.bricks.len() >= self.rules.max_parts {
      return Err(format!("Maximum {} bricks", self.rules.max_parts));
    }
    let chassis = self.database.find(&build.chassis)?;
    if chassis.id >= 0x800 || !self.rules.colors.contains(&build.color) {
      return Err("invalid original chassis/color".into());
    }
    if self.database.find(&part.name)?.id < 0x800 {
      return Err("a chassis is not an attachable brick".into());
    }
    let base = PlacedBrick {
      name: chassis.name.clone(),
      color: build.color.clone(),
      x: 0,
      y: 0,
      z: 0,
      rotation: 0,
    };
    let mut occupied = self.cells(&base)?;
    for brick in &build.bricks {
      occupied.extend(self.cells(brick)?);
    }
    let cells = self.cells(&part)?;
    if cells.is_empty() {
      return Err("brick has no placement cells".into());
    }
    let mut supported = false;
    for cell in &cells {
      if cell.x >= chassis.width || cell.y >= chassis.depth || cell.top > self.rules.max_height {
        return Err("Brick is outside the chassis build region".into());
      }
      for old in &occupied {
        if old.x == cell.x && old.y == cell.y {
          if cell.bottom < old.top && old.bottom < cell.top {
            return Err("Brick overlaps an existing part".into());
          }
          if cell.bottom == old.top {
            supported = true;
          }
        }
      }
    }
    if !supported {
      return Err("Brick must attach to the chassis or another brick".into());
    }
    build.bricks.push(part);
    Ok(())
  }
  pub fn validate(&self, build: &Build) -> Result<(), String> {
    let chassis = self.database.find(&build.chassis)?;
    if chassis.id >= 0x800 || !self.rules.colors.contains(&build.color) {
      return Err("invalid saved chassis/color".into());
    }
    let mut checked = Build {
      chassis: build.chassis.clone(),
      color: build.color.clone(),
      bricks: Vec::new(),
    };
    for part in &build.bricks {
      self.add(&mut checked, part.clone())?;
    }
    Ok(())
  }
  pub fn heights(&self, build: &Build) -> Result<BTreeMap<(u8, u8), u8>, String> {
    let mut heights = BTreeMap::new();
    for part in std::iter::once(PlacedBrick {
      name: build.chassis.clone(),
      color: build.color.clone(),
      x: 0,
      y: 0,
      z: 0,
      rotation: 0,
    })
    .chain(build.bricks.iter().cloned())
    {
      for cell in self.cells(&part)? {
        let top = heights.entry((cell.x, cell.y)).or_insert(0u8);
        *top = (*top).max(cell.top);
      }
    }
    Ok(heights)
  }
  pub fn origin(&self, build: &Build) -> Result<[f32; 3], String> {
    let brick = self.database.find(&build.chassis)?;
    Ok(
      brick
        .origin
        .unwrap_or([brick.width as f32 * 0.5, brick.depth as f32 * 0.5, 0.0]),
    )
  }
}
pub fn rotated_point(brick: &Brick, [x, y, z]: [f32; 3], rotation: u8) -> [f32; 3] {
  match rotation {
    0 => [x, y, z],
    1 => [y, brick.width as f32 - x, z],
    2 => [brick.width as f32 - x, brick.depth as f32 - y, z],
    _ => [brick.depth as f32 - y, x, z],
  }
}
