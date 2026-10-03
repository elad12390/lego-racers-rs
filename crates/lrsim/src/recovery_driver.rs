//! Ordinary no-boost004203b0 recovery: pursue,250mslookahead and <3unit rejoin.
//! Separate from recorded-route movement. Not yet an opponent effect/AI dispatcher.
use crate::{handling::clamp_radius, route_cursor::RouteCursor, route_motion::quaternion_basis};
use lrformats::route::RouteRecord;
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
pub struct State {
  pub target: [f32; 3],
  pub rotation: [f32; 4],
  pub route_time: u32,
  pub flags: u32,
  pub steer_current: f32,
  pub steer_target: f32,
  pub skid_ms: u32,
}
#[derive(Clone, Deserialize)]
pub struct Pose {
  pub position: [f32; 3],
  pub forward: [f32; 3],
  pub left: [f32; 3],
  pub direction: [f32; 3],
  pub wheels: u32,
  pub speed: f32,
  pub forward_speed_ms: f32,
  pub spin_hold: bool,
}
#[derive(Serialize)]
pub struct Rejoin {
  pub position: [f32; 3],
  pub basis: [f32; 9],
  pub cursor_position: [f32; 3],
  pub cursor_rotation: [f32; 4],
  pub cursor_time: f32,
}
#[derive(Serialize)]
pub struct Command {
  pub turn_radius: f32,
  pub throttle: f32,
  pub rejoin: Option<Rejoin>,
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f64 {
  let p = |i: usize| f64::from(a[i]) * f64::from(b[i]);
  (p(2) + p(1)) + p(0)
}

impl State {
  fn tick(
    &mut self,
    record: &RouteRecord,
    target_cursor: &mut RouteCursor<'_>,
    pose: &Pose,
    elapsed: u32,
  ) -> Result<Command, String> {
    // Flags1/4/8/10/100 have boost/noise/effect dispatch beyond this branch.
    if self.flags & !(2 | 0x20 | 0x40) != 0 || self.flags & 2 == 0 {
      return Err("recovery boost/noise flags require original effect dispatcher".into());
    }
    let offset = std::array::from_fn(|i| self.target[i] - pose.position[i]);
    let distance = dot(offset, offset).sqrt() as f32;
    if distance < 3.0 {
      let mut cursor = RouteCursor::start(record);
      // Actual ResetRouteCursors calls FindAhead with zero displacement;
      // this still loads the segment rotations before the final Advance.
      cursor.displace([0.0; 3]);
      cursor.advance(self.route_time as f32);
      self.flags &= !0x40;
      self.route_time = 0;
      self.steer_target = 0.0;
      // Original places the shape at loop origin BEFORE advancing the
      // body cursor. Its next CarBody::Step supplies the advanced pose.
      return Ok(Command {
        turn_radius: 0.0,
        throttle: 0.0,
        rejoin: Some(Rejoin {
          position: record.loop_position,
          basis: quaternion_basis(record.loop_rotation),
          cursor_position: cursor.position,
          cursor_rotation: cursor.rotation,
          cursor_time: cursor.time,
        }),
      });
    }
    self.steer_current = self.steer_target;
    self.skid_ms = self.skid_ms.wrapping_add(elapsed);
    if pose.spin_hold {
      self.skid_ms = 0;
      self.flags &= !0x20;
    } else if self.flags & 0x20 == 0 {
      if pose.forward_speed_ms > 0.009 || pose.forward_speed_ms < -0.009 {
        self.skid_ms = 0;
      } else if self.skid_ms > 999 {
        self.skid_ms = 0;
        self.flags |= 0x20;
      }
    } else if self.skid_ms > 1999 {
      self.skid_ms = 0;
      self.flags &= !0x20;
    }
    let negative = dot(pose.left, offset) < 0.0;
    let left = if negative {
      pose.left.map(|v| -v)
    } else {
      pose.left
    };
    //00420529keeps the inverse in x87; X/Ynormalized components spill,
    // Zand the final sine stay retained through division into the radius.
    let inverse = 1.0 / f64::from(distance);
    let x = (inverse * f64::from(offset[0])) as f32;
    let y = (inverse * f64::from(offset[1])) as f32;
    let z = inverse * f64::from(offset[2]);
    let sine = (z * f64::from(left[2]) + f64::from(y) * f64::from(left[1]))
      + f64::from(x) * f64::from(left[0]);
    let mut radius = if sine >= f64::from(0.0005f32) {
      (f64::from(distance) / (sine + sine)) as f32
    } else {
      4096.0
    };
    if negative {
      radius = -radius;
    }
    if self.flags & 0x20 != 0 {
      radius = -radius;
    }
    let direction = if pose.wheels < 3 {
      pose.direction
    } else {
      pose.forward
    };
    let target_basis = quaternion_basis(self.rotation);
    let target_forward = target_basis[..3].try_into().unwrap();
    if (distance < 30.0 || dot(direction, target_forward) < 0.5) && distance <= 80.0 {
      self.route_time = self.route_time.wrapping_add(250);
      target_cursor.restart_loop();
      target_cursor.advance(self.route_time as f32);
      self.target = target_cursor.position;
      self.rotation = target_cursor.rotation;
    }
    if radius != 0.0 && pose.speed < 40.0 {
      radius *= 0.2;
    }
    self.steer_target = radius;
    Ok(Command {
      turn_radius: clamp_radius(radius),
      throttle: if self.flags & 0x20 != 0 {
        -1.0
      } else {
        1.0 / 3.0
      },
      rejoin: None,
    })
  }
}

pub struct RecoveryDriver<'a> {
  pub state: State,
  record: &'a RouteRecord,
  target_cursor: RouteCursor<'a>,
}
impl<'a> RecoveryDriver<'a> {
  pub fn new(record: &'a RouteRecord, state: State) -> Self {
    Self {
      state,
      record,
      target_cursor: RouteCursor::start(record),
    }
  }
  ///004202f0 starts at this record's loop and takes a1000mslookahead.
  /// Caller must detach the actual body effects/disable400 before driving.
  pub fn begin(&mut self) {
    self.state.route_time = 1000;
    self.state.flags = (self.state.flags | 0x40) & !(4 | 8);
    self.state.steer_target = 0.0;
    self.target_cursor.restart_loop();
    self.target_cursor.speed = 1.0;
    self.target_cursor.advance(1000.0);
    self.state.target = self.target_cursor.position;
    self.state.rotation = self.target_cursor.rotation;
  }
  pub fn tick(&mut self, pose: &Pose, elapsed: u32) -> Result<Command, String> {
    self
      .state
      .tick(self.record, &mut self.target_cursor, pose, elapsed)
  }
}
