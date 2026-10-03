//! Original picker translation and one-update-late completion gate.
use crate::{
  driver_thumbnail::{self, Thumbnail},
  platform::prelude::*,
};
use lrformats::menu_selector::Selector;

pub struct Row {
  pub items: Vec<Option<Thumbnail>>,
  duration_ms: u32,
  motion: Option<Motion>,
}
struct Motion {
  remaining_ms: u32,
  velocities: Vec<Vec3>,
  targets: Vec<Vec3>,
}
impl Row {
  pub fn new(
    items: Vec<Option<Thumbnail>>,
    duration_ms: u32,
    selector: &Selector,
    viewport: Rect,
  ) -> Self {
    assert!(duration_ms > 0);
    assert_eq!(items.len(), selector.slots.len());
    let mut row = Self {
      items,
      duration_ms,
      motion: None,
    };
    for (slot, item) in row.items.iter_mut().enumerate() {
      if let Some(item) = item {
        item.translation =
          driver_thumbnail::fit(selector, viewport, slot, item.center, item.radius);
      }
    }
    row
  }
  pub fn busy(&self) -> bool {
    self.motion.is_some()
  }
  pub fn shift(
    &mut self,
    direction: isize,
    incoming: Option<Thumbnail>,
    selector: &Selector,
    viewport: Rect,
  ) {
    assert!(!self.busy());
    assert!(direction == 1 || direction == -1);
    let edge = if direction > 0 {
      self.items.rotate_left(1);
      self.items.len() - 1
    } else {
      self.items.rotate_right(1);
      0
    };
    self.items[edge] = incoming;
    let targets = self
      .items
      .iter()
      .enumerate()
      .map(|(slot, item)| {
        item
          .as_ref()
          .map(|item| driver_thumbnail::fit(selector, viewport, slot, item.center, item.radius))
          .unwrap_or(Vec3::ZERO)
      })
      .collect::<Vec<_>>();
    // ScrollNext/Prev rotates retained objects at their old positions; newly
    // loaded offscreen edge gets Layout_Item immediately after Layout_Items.
    if let Some(item) = &mut self.items[edge] {
      item.translation = targets[edge];
    }
    let velocities = self
      .items
      .iter()
      .zip(&targets)
      .map(|(item, target)| {
        item
          .as_ref()
          .map(|item| (*target - item.translation) / self.duration_ms as f32)
          .unwrap_or(Vec3::ZERO)
      })
      .collect();
    self.motion = Some(Motion {
      remaining_ms: self.duration_ms,
      velocities,
      targets,
    });
  }
  pub fn advance(&mut self, elapsed_ms: u32) {
    let Some(motion) = &mut self.motion else {
      return;
    };
    if motion.remaining_ms == 0 {
      for (item, target) in self.items.iter_mut().zip(&motion.targets) {
        if let Some(item) = item {
          item.translation = *target;
        }
      }
      self.motion = None;
      return;
    }
    let elapsed = elapsed_ms.min(motion.remaining_ms);
    motion.remaining_ms -= elapsed;
    for (item, velocity) in self.items.iter_mut().zip(&motion.velocities) {
      if let Some(item) = item {
        item.translation += *velocity * elapsed as f32;
      }
    }
  }
}
