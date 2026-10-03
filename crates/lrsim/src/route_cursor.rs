//! Plays back a [`RouteRecord`] over time.
//!
//! Ported from the exe, derived from Ghidra: start (0x4a5220), advance (0x4a5320) and the
//! quaternion interpolation (0x4495e0). Field offsets of the original object are noted so the
//! port can be compared against the decompilation.
//!
//! Faithfulness notes:
//! - Time is in milliseconds, truncated to an integer for segment selection (`ftol`).
//! - When playback runs off the end it wraps to the loop point. The original shifts the clock
//!   by `loop_time - end` but keeps using the old `end` for the next segment's end; this port
//!   keeps that behaviour.
//! - The original uses x87 extended precision for intermediate values; this port uses `f32`.

use lrformats::route::{PathPoint, RouteRecord};

#[derive(Debug, Clone)]
pub struct RouteCursor<'a> {
  record: &'a RouteRecord,
  /// +0x00: interpolated position.
  pub position: [f32; 3],
  /// +0x0c: interpolated rotation quaternion `[x, y, z, w]`.
  pub rotation: [f32; 4],
  /// +0x1c: type of the segment being played.
  pub kind: u32,
  /// +0x20 and +0x24: interpolated widths (from the +9 and +10 bytes of the points).
  pub width_a: f32,
  pub width_b: f32,
  /// +0x2c: time scale; owned by whoever drives the cursor.
  pub speed: f32,
  /// +0x30: clock in milliseconds.
  pub time: f32,
  /// +0x34 and +0x38: indices of the segment's start and end points (-1 before the first point).
  index_a: i32,
  index_b: i32,
  /// +0x3c and +0x48: absolute positions of those points.
  position_a: [f32; 3],
  position_b: [f32; 3],
  /// +0x54 and +0x64: their rotations.
  rotation_a: [f32; 4],
  rotation_b: [f32; 4],
  /// +0x74: time (ms) at point A.
  start_time: i32,
}

/// Quaternion interpolation that takes the shorter arc (0x4495e0): a plain lerp when the dot
/// product is positive, otherwise a lerp toward the negated target. Not normalised.
pub fn lerp_quat(a: &[f32; 4], b: &[f32; 4], t: f32) -> [f32; 4] {
  let dot = a[0] * b[0] + a[3] * b[3] + a[2] * b[2] + a[1] * b[1];
  if 0.0 < dot {
    [0, 1, 2, 3].map(|k| (b[k] - a[k]) * t + a[k])
  } else {
    [0, 1, 2, 3].map(|k| a[k] - (a[k] + b[k]) * t)
  }
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
  [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

impl<'a> RouteCursor<'a> {
  /// Original Start rewrites loop fields but leaves speed and endpoint-B
  /// quaternion intact. Recovery restarts the SAME cursor, not a fresh one.
  pub fn restart_loop(&mut self) {
    let speed = self.speed;
    let rotation_b = self.rotation_b;
    *self = Self::start(self.record);
    self.speed = speed;
    self.rotation_b = rotation_b;
  }

  /// Original004a5750consumes the route-tangent portion of a world
  /// displacement, rewrites its residual, and repositions the route clock.
  /// Unlike Advance, this routine does not wrap while searching at boundaries.
  pub fn displace(&mut self, mut residual: [f32; 3]) -> [f32; 3] {
    use crate::contact::{dot, length, sub};
    let mut ia = self.index_a;
    let mut ib = self.index_b;
    let mut start = self.start_time;
    let mut end = start + self.record.points[ib as usize].length();
    let mut a = self.position_a;
    let mut b = self.position_b;
    let mut direction;
    let mut distance;
    let next = |ib: i32, b: [f32; 3]| {
      self
        .record
        .points
        .get(ib as usize)
        .map(|p| add(b, p.position_delta()))
    };
    // Skip stationary segments without publishing a partial cursor update.
    loop {
      direction = sub(b, a);
      distance = length(direction);
      if distance * distance >= 0.001 {
        break;
      }
      ia = ib;
      ib += 1;
      a = b;
      start = end;
      let Some(p) = self.record.points.get(ib as usize) else {
        return residual;
      };
      b = next(ib, b).unwrap();
      end += p.length();
    }
    direction = direction.map(|v| v / distance);
    let projected = dot(direction, residual);
    let fraction_distance = if projected >= 0.0 {
      let mut left = length(sub(self.position, b));
      let mut projected = projected;
      while left < projected {
        residual = std::array::from_fn(|i| residual[i] - direction[i] * left);
        loop {
          ia = ib;
          ib += 1;
          a = b;
          start = end;
          let Some(p) = self.record.points.get(ib as usize) else {
            return residual;
          };
          b = next(ib, b).unwrap();
          end += p.length();
          direction = sub(b, a);
          distance = length(direction);
          if distance * distance >= 0.001 {
            break;
          }
        }
        direction = direction.map(|v| v / distance);
        left = distance;
        projected = dot(direction, residual).max(0.0);
      }
      projected + distance - left
    } else {
      let mut travelled = length(sub(self.position, a));
      let mut wanted = -projected;
      while wanted > travelled {
        residual = std::array::from_fn(|i| residual[i] + direction[i] * travelled);
        loop {
          ib = ia;
          ia -= 1;
          b = a;
          end = start;
          if ia < 0 {
            return residual;
          }
          let p = &self.record.points[ib as usize];
          start -= p.length();
          a = sub(a, p.position_delta());
          direction = sub(b, a);
          distance = length(direction);
          if distance * distance >= 0.001 {
            break;
          }
        }
        direction = direction.map(|v| v / distance);
        travelled = distance;
        let projection = dot(direction, residual);
        if projection > 0.0 {
          wanted = 0.0;
          break;
        }
        wanted = -projection;
      }
      travelled - wanted
    };
    if ia < 0 {
      return residual;
    }
    self.index_a = ia;
    self.index_b = ib;
    self.start_time = start;
    self.position_a = a;
    self.position_b = b;
    self.time = start as f32 + fraction_distance / distance * (end - start) as f32;
    self.rotation_a = self.record.points[ia as usize].rotation_quat();
    self.rotation_b = self.record.points[ib as usize].rotation_quat();
    self.advance(0.0);
    residual
  }

  /// Starts playback at the record's loop point (0x4a5220).
  pub fn start(record: &'a RouteRecord) -> Self {
    let index_a = record.loop_index;
    let index_b = index_a + 1;
    let point_a = &record.points[index_a as usize];
    let delta = record
      .points
      .get(index_b as usize)
      .map_or([0.0; 3], PathPoint::position_delta);
    RouteCursor {
      record,
      position: record.loop_position,
      rotation: record.loop_rotation,
      kind: point_a.kind(),
      width_a: point_a.left_width(),
      width_b: point_a.right_width(),
      speed: 1.0,
      time: record.loop_time as f32,
      index_a,
      index_b,
      position_a: record.loop_position,
      position_b: add(record.loop_position, delta),
      rotation_a: record.loop_rotation,
      // Start leaves this alone; actual004a5190Reset initializes Wto1.
      rotation_b: [0.0, 0.0, 0.0, 1.0],
      start_time: record.loop_time,
    }
  }

  /// Starts playback before the first point: at the record's start position and rotation,
  /// time zero, heading for point 0. The advance routine's `index_a < 0` case sets everything
  /// else up on the first call.
  pub fn start_at_beginning(record: &'a RouteRecord) -> Self {
    RouteCursor {
      record,
      position: record.start_position,
      rotation: record.start_rotation,
      kind: 4,
      width_a: 0.0,
      width_b: 0.0,
      speed: 1.0,
      time: 0.0,
      index_a: -1,
      index_b: 0,
      position_a: record.start_position,
      position_b: record.start_position,
      rotation_a: record.start_rotation,
      rotation_b: [0.0, 0.0, 0.0, 1.0],
      start_time: 0,
    }
  }

  /// Advances the clock by `dt` (in the same units as `speed` scales, i.e. milliseconds) and
  /// recomputes the interpolated state (0x4a5320).
  pub fn advance(&mut self, dt: f32) {
    let record = self.record;
    let points = &record.points;
    self.time += self.speed * dt;
    let mut ti = self.time as i32;

    let (mut prev_left, mut prev_right);
    if self.index_a < 0 {
      prev_left = 0.0;
      prev_right = 0.0;
      self.position_a = record.start_position;
      self.rotation_a = record.start_rotation;
      self.start_time = 0;
      let point_b = &points[self.index_b as usize];
      self.position_b = add(self.position_a, point_b.position_delta());
      self.rotation_b = point_b.rotation_quat();
      self.kind = 4;
    } else {
      let point_a = &points[self.index_a as usize];
      prev_right = point_a.right_width();
      prev_left = point_a.left_width();
    }
    let point_b = &points[self.index_b as usize];
    let mut next_right = point_b.right_width();
    let mut next_left = point_b.left_width();
    let start = self.start_time;
    let mut end = point_b.length() + start;

    if ti < start {
      // Rewind through earlier segments.
      let mut start_now = start;
      let mut reached = false;
      if self.index_a > 0 {
        loop {
          end = start_now;
          let leaving = self.index_a;
          self.index_b = leaving;
          self.position_b = self.position_a;
          self.rotation_b = self.rotation_a;
          next_right = prev_right;
          next_left = prev_left;
          let point = &points[leaving as usize];
          let delta = point.position_delta();
          self.position_a = [
            self.position_a[0] - delta[0],
            self.position_a[1] - delta[1],
            self.position_a[2] - delta[2],
          ];
          self.index_a = leaving - 1;
          self.start_time -= point.length();
          let earlier = &points[self.index_a as usize];
          self.kind = earlier.kind();
          prev_right = earlier.right_width();
          prev_left = earlier.left_width();
          self.rotation_a = earlier.rotation_quat();
          if self.start_time <= ti {
            reached = true;
            break;
          }
          start_now = self.start_time;
          if self.index_a == 0 {
            break;
          }
        }
      }
      if !reached {
        ti = 0;
        self.time = 0.0;
      }
    } else if ti >= end {
      // Move forward through later segments, wrapping at the end of the record.
      loop {
        self.start_time = end;
        self.index_a = self.index_b;
        self.position_a = self.position_b;
        self.rotation_a = self.rotation_b;
        prev_right = next_right;
        prev_left = next_left;
        self.index_b += 1;
        if points.len() as i32 <= self.index_b {
          self.position_a = record.loop_position;
          self.position_b = record.loop_position;
          self.rotation_a = record.loop_rotation;
          self.start_time = record.loop_time;
          self.index_a = record.loop_index;
          ti += record.loop_time - end;
          self.index_b = self.index_a + 1;
          self.time = ti as f32;
        }
        let point = &points[self.index_b as usize];
        self.kind = point.kind();
        next_right = point.right_width();
        next_left = point.left_width();
        self.rotation_b = point.rotation_quat();
        self.position_b = add(self.position_b, point.position_delta());
        end += point.length();
        if end > ti {
          break;
        }
      }
    }

    let start = self.start_time;
    let t = (ti - start) as f32 / (end - start) as f32;
    self.width_a = (next_left - prev_left) * t + prev_left;
    self.width_b = (next_right - prev_right) * t + prev_right;
    let (a, b) = (self.position_a, self.position_b);
    self.position = [
      (b[0] - a[0]) * t + a[0],
      (b[1] - a[1]) * t + a[1],
      (b[2] - a[2]) * t + a[2],
    ];
    self.rotation = lerp_quat(&self.rotation_a, &self.rotation_b, t);
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn point(dx: i16, packed: u8) -> PathPoint {
    PathPoint {
      position_x: dx,
      position_y: 0,
      position_z: 0,
      rotation: [0, 0, 0, 127],
      width_left: 8,
      width_right: 8,
      packed_type_and_length: packed,
    }
  }

  /// Four points, each 1 unit further along X, each segment 64 ms (packed length 2 << 5).
  /// start (0,0,0); loop at index 1 (position x = 2, time 128).
  fn record() -> RouteRecord {
    RouteRecord {
      points: vec![point(256, 0b11_000010); 4],
      start_position: [0.0; 3],
      start_rotation: [0.0, 0.0, 0.0, 1.0],
      loop_position: [2.0, 0.0, 0.0],
      loop_rotation: [0.0, 0.0, 0.0, 1.0],
      loop_time: 128,
      loop_index: 1,
    }
  }

  #[test]
  fn starting_at_the_loop_point_reports_the_loop_state() {
    let r = record();
    let cursor = RouteCursor::start(&r);
    assert_eq!(cursor.position, [2.0, 0.0, 0.0]);
    assert_eq!(cursor.time, 128.0);
    assert_eq!(cursor.kind, 4);
  }

  #[test]
  fn first_loop_interval_keeps_original_reset_identity_end_quaternion() {
    let mut r = record();
    r.loop_rotation = [0.0, 0.0, -0.6, 0.8];
    let mut cursor = RouteCursor::start(&r);
    cursor.advance(32.0);
    assert_eq!(cursor.rotation, [0.0, 0.0, -0.3, 0.9]);
  }

  #[test]
  fn interpolates_linearly_within_a_segment() {
    let r = record();
    let mut cursor = RouteCursor::start(&r);
    cursor.advance(32.0); // halfway through the 64 ms segment from x=2 to x=3
    assert!(
      (cursor.position[0] - 2.5).abs() < 1e-5,
      "{:?}",
      cursor.position
    );
    cursor.advance(16.0);
    assert!((cursor.position[0] - 2.75).abs() < 1e-5);
  }

  #[test]
  fn crossing_a_segment_boundary_continues_along_the_path() {
    let r = record();
    let mut cursor = RouteCursor::start(&r);
    cursor.advance(96.0); // 1.5 segments past the loop point: x = 3.5
    assert!(
      (cursor.position[0] - 3.5).abs() < 1e-5,
      "{:?}",
      cursor.position
    );
  }

  #[test]
  fn running_off_the_end_wraps_to_the_loop_point() {
    let r = record();
    let mut cursor = RouteCursor::start(&r);
    // Points 2 and 3 follow the loop point; after 128 ms the path is at x=4 and wraps.
    cursor.advance(128.0 + 10.0);
    // Back in the loop domain: time is loop_time + overshoot, position near the loop point.
    assert!(
      cursor.position[0] < 3.5,
      "wrapped position {:?}",
      cursor.position
    );
    assert!(cursor.time < 128.0 + 128.0, "clock {}", cursor.time);
  }

  #[test]
  fn rewinding_to_a_time_before_the_segment_walks_backwards() {
    let r = record();
    let mut cursor = RouteCursor::start(&r);
    cursor.advance(-32.0); // half a segment before the loop point: x = 1.5 (between points 0 and 1)
    assert!(
      (cursor.position[0] - 1.5).abs() < 1e-5,
      "{:?}",
      cursor.position
    );
  }

  #[test]
  fn starting_at_the_beginning_heads_for_point_zero() {
    let r = record();
    let mut cursor = RouteCursor::start_at_beginning(&r);
    cursor.advance(32.0);
    assert!(
      (cursor.position[0] - 0.5).abs() < 1e-5,
      "{:?}",
      cursor.position
    );
  }

  #[test]
  fn quaternion_lerp_takes_the_short_arc() {
    let a = [0.0, 0.0, 0.0, 1.0];
    let same_side = lerp_quat(&a, &[0.0, 0.0, 0.6, 0.8], 0.5);
    assert!((same_side[3] - 0.9).abs() < 1e-6 && (same_side[2] - 0.3).abs() < 1e-6);
    // The negated quaternion is the same rotation; the lerp must not pass through zero.
    let flipped = lerp_quat(&a, &[0.0, 0.0, -0.6, -0.8], 0.5);
    assert!(
      (flipped[3] - 0.9).abs() < 1e-6 && (flipped[2] - 0.3).abs() < 1e-6,
      "{flipped:?}"
    );
  }
}
