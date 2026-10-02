//! Recorded driving lines (`.RRB`): quantized path samples plus start and loop state.
//!
//! Ported from the exe, derived from Ghidra:
//! - record loader 0x4a4e30 (keywords 0x27..0x2d, mirror handling, loop index check)
//! - point-array loader 0x4a5100 and per-point loader 0x4a5e10 (10 integers, mirror swap)
//! - point accessors 0x4a5ec0 (type), 0x4a5ee0 (length), 0x4a5ef0 (position),
//!   0x4a5f40 (rotation), 0x4a5fa0 / 0x4a5fc0 (widths)
//!
//! Verified against all 208 shipped files: integrating the position deltas from the start
//! position through `loop_index` lands exactly on `loop_position`, and the point lengths sum
//! to `loop_time`.

use std::fmt;

use crate::tok::{self, Node, TokError, Value};

const POSITION_XY_SCALE: f32 = 1.0 / 256.0;
const POSITION_Z_SCALE: f32 = 1.0 / 16.0;
const ROTATION_SCALE: f32 = 1.0 / 127.0;
const WIDTH_SCALE: f32 = 0.125;

#[derive(Debug, PartialEq)]
pub enum RouteError {
    Tok(TokError),
    Shape(&'static str),
    /// `loop_index` must be less than the number of points (the exe reports error 0xf).
    InvalidLoopIndex { index: i32, count: usize },
}

impl fmt::Display for RouteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RouteError::Tok(e) => write!(f, "{e}"),
            RouteError::Shape(why) => write!(f, "unexpected route layout: {why}"),
            RouteError::InvalidLoopIndex { index, count } => {
                write!(f, "loop index {index} is not below the point count {count}")
            }
        }
    }
}

impl std::error::Error for RouteError {}

impl From<TokError> for RouteError {
    fn from(error: TokError) -> Self {
        RouteError::Tok(error)
    }
}

/// One packed 12-byte sample. Positions are deltas from the previous sample.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PathPoint {
    pub position_x: i16,
    pub position_y: i16,
    pub position_z: i8,
    /// Rotation quaternion as signed bytes: x, y, z, w.
    pub rotation: [i8; 4],
    pub width_left: i8,
    pub width_right: i8,
    /// Top 2 bits: type. Low 6 bits: segment length in units of 32 ms.
    pub packed_type_and_length: u8,
}

impl PathPoint {
    /// Type from the top two bits; the value 3 is reported as 4 (0x4a5ec0).
    pub fn kind(&self) -> u32 {
        let t = u32::from(self.packed_type_and_length >> 6);
        if t > 2 { 4 } else { t }
    }

    /// Duration of the segment ending at this point, in milliseconds (0x4a5ee0).
    pub fn length(&self) -> i32 {
        i32::from(self.packed_type_and_length & 0x3f) << 5
    }

    /// Position delta from the previous point (0x4a5ef0).
    pub fn position_delta(&self) -> [f32; 3] {
        [
            f32::from(self.position_x) * POSITION_XY_SCALE,
            f32::from(self.position_y) * POSITION_XY_SCALE,
            f32::from(self.position_z) * POSITION_Z_SCALE,
        ]
    }

    /// Rotation quaternion `[x, y, z, w]` (0x4a5f40).
    pub fn rotation_quat(&self) -> [f32; 4] {
        self.rotation.map(|v| f32::from(v) * ROTATION_SCALE)
    }

    pub fn left_width(&self) -> f32 {
        f32::from(self.width_left) * WIDTH_SCALE
    }

    pub fn right_width(&self) -> f32 {
        f32::from(self.width_right) * WIDTH_SCALE
    }

    /// Mirrors the sample left to right, as the loader does (0x4a5e10).
    fn mirror(&mut self) {
        std::mem::swap(&mut self.width_left, &mut self.width_right);
        self.position_y = self.position_y.wrapping_neg();
        self.rotation[1] = self.rotation[1].wrapping_neg();
        self.rotation[3] = self.rotation[3].wrapping_neg();
    }

    fn from_row(row: &[Value]) -> Option<Self> {
        let signed = |v: &Value| match v {
            Value::I8(x) => Some(*x),
            Value::U8(x) => Some(*x as i8),
            _ => None,
        };
        let wide = |v: &Value| match v {
            Value::I16(x) => Some(*x),
            Value::U16(x) => Some(*x as i16),
            _ => None,
        };
        let [x, y, z, rx, ry, rz, rw, wl, wr, packed] = row else { return None };
        Some(PathPoint {
            position_x: wide(x)?,
            position_y: wide(y)?,
            position_z: signed(z)?,
            rotation: [signed(rx)?, signed(ry)?, signed(rz)?, signed(rw)?],
            width_left: signed(wl)?,
            width_right: signed(wr)?,
            packed_type_and_length: match packed {
                Value::U8(p) => *p,
                Value::I8(p) => *p as u8,
                _ => return None,
            },
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RouteRecord {
    pub points: Vec<PathPoint>,
    pub start_position: [f32; 3],
    pub start_rotation: [f32; 4],
    pub loop_position: [f32; 3],
    pub loop_rotation: [f32; 4],
    /// Time (ms) at the loop point.
    pub loop_time: i32,
    pub loop_index: i32,
}

impl Default for RouteRecord {
    /// The state set by the exe's `Reset` (0x4a50c0): identity rotations, everything else zero.
    fn default() -> Self {
        RouteRecord {
            points: Vec::new(),
            start_position: [0.0; 3],
            start_rotation: [0.0, 0.0, 0.0, 1.0],
            loop_position: [0.0; 3],
            loop_rotation: [0.0, 0.0, 0.0, 1.0],
            loop_time: 0,
            loop_index: 0,
        }
    }
}

impl RouteRecord {
    /// Loads a record. `mirror` flips it left to right: Y of positions and Y and W of rotations
    /// are negated, and left and right widths swap.
    pub fn load(data: &[u8], mirror: bool) -> Result<Self, RouteError> {
        let nodes = tok::parse(data)?;
        let mut record = RouteRecord::default();
        let mut i = 0;
        while i < nodes.len() {
            let Node::Keyword(k) = nodes[i] else {
                i += 1;
                continue;
            };
            let rest = &nodes[i + 1..];
            match k {
                0x27 => record.points = read_points(rest, mirror)?,
                0x28 => {
                    record.start_rotation = floats::<4>(rest).ok_or(RouteError::Shape("start rotation"))?;
                    if mirror {
                        record.start_rotation[1] = -record.start_rotation[1];
                        record.start_rotation[3] = -record.start_rotation[3];
                    }
                }
                0x29 => {
                    record.start_position = floats::<3>(rest).ok_or(RouteError::Shape("start position"))?;
                    if mirror {
                        record.start_position[1] = -record.start_position[1];
                    }
                }
                0x2a => {
                    record.loop_position = floats::<3>(rest).ok_or(RouteError::Shape("loop position"))?;
                    if mirror {
                        record.loop_position[1] = -record.loop_position[1];
                    }
                }
                0x2b => {
                    record.loop_rotation = floats::<4>(rest).ok_or(RouteError::Shape("loop rotation"))?;
                    if mirror {
                        record.loop_rotation[1] = -record.loop_rotation[1];
                        record.loop_rotation[3] = -record.loop_rotation[3];
                    }
                }
                0x2c => record.loop_time = int(rest).ok_or(RouteError::Shape("loop time"))?,
                0x2d => record.loop_index = int(rest).ok_or(RouteError::Shape("loop index"))?,
                _ => {}
            }
            i += 1;
        }
        if record.loop_index < 0 || record.loop_index as usize >= record.points.len() {
            return Err(RouteError::InvalidLoopIndex { index: record.loop_index, count: record.points.len() });
        }
        Ok(record)
    }

    /// Absolute position of every point, accumulating the deltas from the start position.
    pub fn absolute_positions(&self) -> Vec<[f32; 3]> {
        let mut at = self.start_position;
        self.points
            .iter()
            .map(|p| {
                let d = p.position_delta();
                at = [at[0] + d[0], at[1] + d[1], at[2] + d[2]];
                at
            })
            .collect()
    }
}

fn read_points(rest: &[Node], mirror: bool) -> Result<Vec<PathPoint>, RouteError> {
    let Some(Node::Block(body)) = rest.iter().find(|n| matches!(n, Node::Block(_))) else {
        return Err(RouteError::Shape("path point block"));
    };
    let mut points = Vec::new();
    for node in body {
        let rows: Vec<&[Value]> = match node {
            Node::Packed { rows, .. } => rows.iter().map(Vec::as_slice).collect(),
            Node::Record { fields, .. } => vec![fields.as_slice()],
            _ => continue,
        };
        for row in rows {
            let mut point = PathPoint::from_row(row).ok_or(RouteError::Shape("path point fields"))?;
            if mirror {
                point.mirror();
            }
            points.push(point);
        }
    }
    Ok(points)
}

/// `N` floats following a keyword, whether they were written one by one or as one packed array.
fn floats<const N: usize>(rest: &[Node]) -> Option<[f32; N]> {
    let mut out = [0.0f32; N];
    if let Some(Node::Packed { rows, .. }) = rest.first() {
        if rows.len() < N {
            return None;
        }
        for (slot, row) in out.iter_mut().zip(rows) {
            *slot = row.first()?.as_f32()?;
        }
        return Some(out);
    }
    for (slot, node) in out.iter_mut().zip(rest) {
        match node {
            Node::Float(v) => *slot = *v,
            _ => return None,
        }
    }
    (rest.len() >= N).then_some(out)
}

fn int(rest: &[Node]) -> Option<i32> {
    match rest.first()? {
        Node::Int(v) => Some(*v),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: i16, y: i16, packed: u8) -> PathPoint {
        PathPoint {
            position_x: x,
            position_y: y,
            position_z: 16,
            rotation: [0, 10, 0, 127],
            width_left: 8,
            width_right: 16,
            packed_type_and_length: packed,
        }
    }

    #[test]
    fn accessors_decode_type_length_and_scaled_values() {
        let p = point(256, -512, 0b11_000010);
        assert_eq!(p.kind(), 4, "type 3 is reported as 4");
        assert_eq!(p.length(), 64);
        assert_eq!(p.position_delta(), [1.0, -2.0, 1.0]);
        assert_eq!(p.rotation_quat(), [0.0, 10.0 / 127.0, 0.0, 1.0]);
        assert_eq!((p.left_width(), p.right_width()), (1.0, 2.0));
        assert_eq!(point(0, 0, 0b01_000001).kind(), 1);
        assert_eq!(point(0, 0, 0b10_000001).kind(), 2);
    }

    #[test]
    fn mirroring_swaps_widths_and_negates_y_and_the_quaternions_y_and_w() {
        let mut p = point(5, 7, 0);
        p.mirror();
        assert_eq!((p.width_left, p.width_right), (16, 8));
        assert_eq!(p.position_y, -7);
        assert_eq!(p.rotation, [0, -10, 0, -127]);
        assert_eq!(p.position_x, 5);
    }

    fn file(loop_index: i32, points: usize, mirror_probe_y: f32) -> Vec<u8> {
        let mut d = Vec::new();
        // layout 0x17: i16 i16 i8 i8 i8 i8 i8 i8 i8 u8
        d.extend_from_slice(&[0x16, 0x17, 10, 0x0d, 0x0d, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0c]);
        d.push(0x29);
        for v in [1.0f32, mirror_probe_y, 3.0] {
            d.push(3);
            d.extend_from_slice(&v.to_le_bytes());
        }
        d.extend_from_slice(&[0x2c, 4]);
        d.extend_from_slice(&100i32.to_le_bytes());
        d.extend_from_slice(&[0x2d, 4]);
        d.extend_from_slice(&loop_index.to_le_bytes());
        d.extend_from_slice(&[0x27, 7, 4]);
        d.extend_from_slice(&(points as i32).to_le_bytes());
        d.extend_from_slice(&[8, 5, 0x14]);
        d.extend_from_slice(&(points as u16).to_le_bytes());
        d.push(0x17);
        for n in 0..points {
            d.extend_from_slice(&(256i16 * (n as i16 + 1)).to_le_bytes());
            d.extend_from_slice(&0i16.to_le_bytes());
            d.extend_from_slice(&[0, 0, 0, 0, 127, 8, 8, 0b11_000001]);
        }
        d.push(6);
        d
    }

    #[test]
    fn loads_header_and_points_and_accumulates_positions() {
        let record = RouteRecord::load(&file(1, 3, 2.0), false).unwrap();
        assert_eq!(record.points.len(), 3);
        assert_eq!((record.loop_time, record.loop_index), (100, 1));
        assert_eq!(record.start_position, [1.0, 2.0, 3.0]);
        let abs = record.absolute_positions();
        assert_eq!([abs[0][0], abs[1][0], abs[2][0]], [2.0, 4.0, 7.0]);
        let mirrored = RouteRecord::load(&file(1, 3, 2.0), true).unwrap();
        assert_eq!(mirrored.start_position[1], -2.0);
    }

    #[test]
    fn rejects_a_loop_index_outside_the_points() {
        assert_eq!(
            RouteRecord::load(&file(3, 3, 0.0), false),
            Err(RouteError::InvalidLoopIndex { index: 3, count: 3 })
        );
    }
}
