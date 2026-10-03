//! Race checkpoint table (`RACE.CPB`).
//!
//! Faithful port of the original exe:
//! - `CheckpointTable_Load` (0x41e770): entry count, limit of 254, 0x24-byte records.
//! - per-record parser (0x41e640): keyword 0x28 plane, 0x29 four link bytes, 0x2a center,
//!   with the Y component negated when the track is mirrored.
//! - record constructor (0x41e600): progress starts at -1.0, links at 0xff.
//! - `ChainLength` (0x41ea60), `FollowToAssigned` (0x41ea90) and the progress pass (0x41e950).

use std::fmt;

use crate::tok::{self, Node, TokError};

pub const MAX_CHECKPOINTS: usize = 254;
const NO_LINK: u8 = 0xff;
const UNASSIGNED: f32 = -1.0;

#[derive(Debug, PartialEq)]
pub enum CheckpointError {
  Tok(TokError),
  TooMany(usize),
  Shape(&'static str),
}

impl fmt::Display for CheckpointError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      CheckpointError::Tok(e) => write!(f, "{e}"),
      CheckpointError::TooMany(n) => write!(f, "Too many CheckPoints ({n})"),
      CheckpointError::Shape(why) => write!(f, "unexpected checkpoint layout: {why}"),
    }
  }
}

impl std::error::Error for CheckpointError {}

impl From<TokError> for CheckpointError {
  fn from(error: TokError) -> Self {
    CheckpointError::Tok(error)
  }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Checkpoint {
  /// Gate plane: `normal . p + d = 0` with `plane = [nx, ny, nz, d]`.
  pub plane: [f32; 4],
  pub center: [f32; 3],
  /// Fraction of the lap (0..1) at this checkpoint, filled in by the progress pass.
  pub progress: f32,
  /// Indices of the checkpoints that may follow this one; `next[0]` is the main route.
  pub next: [u8; 4],
}

impl Default for Checkpoint {
  fn default() -> Self {
    Checkpoint {
      plane: [0.0; 4],
      center: [0.0; 3],
      progress: UNASSIGNED,
      next: [NO_LINK; 4],
    }
  }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CheckpointTable {
  pub records: Vec<Checkpoint>,
}

impl CheckpointTable {
  /// Loads a table. `mirror` flips the track left to right (negates Y), as the game does for
  /// mirrored races.
  pub fn load(data: &[u8], mirror: bool) -> Result<Self, CheckpointError> {
    let nodes = tok::parse(data)?;
    let body = nodes
      .iter()
      .find_map(|n| match n {
        Node::Block(body) => Some(body.as_slice()),
        _ => None,
      })
      .ok_or(CheckpointError::Shape("no entry list"))?;
    let mut records = Vec::new();
    let mut i = 0;
    while i < body.len() {
      if body[i] == Node::Keyword(0x27) {
        if let Some(Node::Block(fields)) = body.get(i + 1) {
          records.push(parse_record(fields, mirror)?);
          i += 1;
        }
      }
      i += 1;
    }
    if records.len() > MAX_CHECKPOINTS {
      return Err(CheckpointError::TooMany(records.len()));
    }
    let mut table = CheckpointTable { records };
    table.assign_progress();
    Ok(table)
  }

  /// Number of checkpoints on the main route: steps from record 0 following `next[0]`
  /// until it returns to record 0 (or the table size is exceeded).
  pub fn chain_length(&self) -> usize {
    let mut steps = 0usize;
    let mut at = 0usize;
    loop {
      steps += 1;
      at = usize::from(self.records[at].next[0]);
      if at == 0 || steps >= self.records.len() {
        return steps;
      }
    }
  }

  /// Starting at `start`, follows `next[0]` until reaching a record whose progress is already
  /// assigned. Returns that record's index and the number of steps taken.
  pub fn follow_to_assigned(&self, start: usize) -> (usize, usize) {
    let mut steps = 0usize;
    let mut at = start;
    if self.records[at].progress == UNASSIGNED {
      loop {
        if steps >= self.records.len() {
          return (start, steps);
        }
        at = usize::from(self.records[at].next[0]);
        steps += 1;
        if self.records[at].progress != UNASSIGNED {
          break;
        }
      }
    }
    (at, steps)
  }

  fn assign_progress(&mut self) {
    if self.records.is_empty() {
      return;
    }
    let step = 1.0 / self.chain_length() as f32;
    let mut progress = 0.0f32;
    let mut at = 0usize;
    loop {
      self.records[at].progress = progress;
      progress += step;
      at = usize::from(self.records[at].next[0]);
      if at == 0 || progress >= 1.0 {
        break;
      }
    }
    // Branches: spread progress evenly from the fork to the record where the branch rejoins.
    let mut at = 0usize;
    loop {
      for slot in 1..4 {
        let first = self.records[at].next[slot];
        if first == NO_LINK {
          continue;
        }
        let (rejoin, steps) = self.follow_to_assigned(usize::from(first));
        let from = self.records[at].progress;
        let to = self.records[rejoin].progress;
        let delta = (to - from) / (steps + 1) as f32;
        let mut value = from;
        let mut node = usize::from(first);
        loop {
          value += delta;
          self.records[node].progress = value;
          node = usize::from(self.records[node].next[0]);
          if node == rejoin {
            break;
          }
        }
      }
      at = usize::from(self.records[at].next[0]);
      if at == 0 {
        break;
      }
    }
  }

  /// Indices of the main route in driving order.
  pub fn main_route(&self) -> Vec<usize> {
    let mut route = vec![0usize];
    let mut at = usize::from(self.records[0].next[0]);
    while at != 0 && route.len() < self.records.len() {
      route.push(at);
      at = usize::from(self.records[at].next[0]);
    }
    route
  }
}

fn parse_record(fields: &[Node], mirror: bool) -> Result<Checkpoint, CheckpointError> {
  let flip = if mirror { -1.0 } else { 1.0 };
  let mut record = Checkpoint::default();
  let mut i = 0;
  while i < fields.len() {
    if let Node::Keyword(k) = fields[i] {
      match (k, fields.get(i + 1)) {
        (0x28, Some(Node::Packed { rows, .. })) if rows.len() >= 4 => {
          for (slot, row) in record.plane.iter_mut().zip(rows) {
            *slot = row
              .first()
              .and_then(|v| v.as_f32())
              .ok_or(CheckpointError::Shape("plane value"))?;
          }
          record.plane[1] *= flip;
        }
        (0x29, Some(Node::Packed { rows, .. })) if rows.len() >= 4 => {
          for (slot, row) in record.next.iter_mut().zip(rows) {
            *slot = row
              .first()
              .and_then(|v| v.as_u32())
              .ok_or(CheckpointError::Shape("link value"))? as u8;
          }
        }
        (0x2a, _) => {
          if let (Some(Node::Float(x)), Some(Node::Float(y)), Some(Node::Float(z))) =
            (fields.get(i + 1), fields.get(i + 2), fields.get(i + 3))
          {
            record.center = [*x, y * flip, *z];
          } else {
            return Err(CheckpointError::Shape("center"));
          }
        }
        _ => {}
      }
    }
    i += 1;
  }
  Ok(record)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn ring(next: &[[u8; 4]]) -> CheckpointTable {
    let mut table = CheckpointTable {
      records: next
        .iter()
        .map(|n| Checkpoint {
          next: *n,
          ..Checkpoint::default()
        })
        .collect(),
    };
    table.assign_progress();
    table
  }

  const X: u8 = 0xff;

  #[test]
  fn a_plain_ring_spreads_progress_evenly_from_zero() {
    let t = ring(&[[1, X, X, X], [2, X, X, X], [3, X, X, X], [0, X, X, X]]);
    assert_eq!(t.chain_length(), 4);
    let p: Vec<f32> = t.records.iter().map(|r| r.progress).collect();
    assert_eq!(p, vec![0.0, 0.25, 0.5, 0.75]);
    assert_eq!(t.main_route(), vec![0, 1, 2, 3]);
  }

  #[test]
  fn a_branch_gets_progress_interpolated_between_its_fork_and_rejoin() {
    // Main ring 0 -> 1 -> 2 -> 3 -> 0. Record 1 also forks to branch nodes 4, 5 which rejoin at 3.
    let t = ring(&[
      [1, X, X, X],
      [2, 4, X, X],
      [3, X, X, X],
      [0, X, X, X],
      [5, X, X, X],
      [3, X, X, X],
    ]);
    let p = |i: usize| t.records[i].progress;
    assert_eq!((p(1), p(3)), (0.25, 0.75));
    // two branch nodes between 0.25 and 0.75: 0.25 + 0.5/3 and 0.25 + 2 * 0.5/3
    assert!((p(4) - (0.25 + 0.5 / 3.0)).abs() < 1e-6, "{}", p(4));
    assert!((p(5) - (0.25 + 1.0 / 3.0)).abs() < 1e-6, "{}", p(5));
    // the main route ignores the branch
    assert_eq!(t.main_route(), vec![0, 1, 2, 3]);
  }

  #[test]
  fn defaults_match_the_record_constructor() {
    let r = Checkpoint::default();
    assert_eq!((r.progress, r.next), (-1.0, [0xff; 4]));
  }

  fn floats(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
  }

  fn file(y_center: f32, y_normal: f32) -> Vec<u8> {
    // 27 [1] { 27 { 28 packed(4 f32) 2a x y z 29 packed(4 i32) } }
    let mut d = vec![0x27, 7, 4, 1, 0, 0, 0, 8, 5, 0x27, 5, 0x28, 0x14, 4, 0, 3];
    d.extend(floats(&[0.0, y_normal, 0.0, 5.0]));
    d.push(0x2a);
    for v in [1.0f32, y_center, 3.0] {
      d.push(3);
      d.extend(floats(&[v]));
    }
    d.extend_from_slice(&[0x29, 0x14, 4, 0, 4]);
    for v in [0i32, 255, 255, 255] {
      d.extend_from_slice(&v.to_le_bytes());
    }
    d.extend_from_slice(&[6, 6]);
    d
  }

  #[test]
  fn loads_a_record_and_mirroring_negates_y_of_the_normal_and_center() {
    let straight = CheckpointTable::load(&file(2.0, -1.0), false).unwrap();
    let r = straight.records[0];
    assert_eq!(
      (r.plane, r.center, r.next),
      ([0.0, -1.0, 0.0, 5.0], [1.0, 2.0, 3.0], [0, 255, 255, 255])
    );
    let mirrored = CheckpointTable::load(&file(2.0, -1.0), true).unwrap();
    let m = mirrored.records[0];
    assert_eq!(
      (m.plane, m.center),
      ([0.0, 1.0, 0.0, 5.0], [1.0, -2.0, 3.0])
    );
  }
}
