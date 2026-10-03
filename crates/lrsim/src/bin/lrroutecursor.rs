//! Real recorded-route cursor oracle adapter, never a playable opponent.
use lrformats::route::RouteRecord;
use lrsim::route_cursor::RouteCursor;
use std::io::{self, Read};

fn state(cursor: &RouteCursor<'_>) -> serde_json::Value {
  serde_json::json!({"position":cursor.position,"rotation":cursor.rotation,"kind":cursor.kind,"widths":[cursor.width_a,cursor.width_b],"time":cursor.time})
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let mut input = String::new();
  io::stdin().read_to_string(&mut input)?;
  let paths: Vec<String> = serde_json::from_str(&input)?;
  let mut output = Vec::new();
  for path in paths {
    let bytes = std::fs::read(&path)?;
    for mirror in [false, true] {
      let r = RouteRecord::load(&bytes, mirror)?;
      for beginning in [false, true] {
        let total: i32 = r.points.iter().map(|p| p.length()).sum();
        let ticks = [
          0.0,
          10.25,
          21.75,
          250.0,
          1000.0,
          total as f32,
          250.0,
          500.0,
          -100.0,
          -500.0,
        ];
        let mut cursor = if beginning {
          RouteCursor::start_at_beginning(&r)
        } else {
          RouteCursor::start(&r)
        };
        // Reset-plus-record binding leaves original output fields unfilled
        // until Advance. Compare after that real zero-time initialization.
        if beginning {
          cursor.advance(0.0);
        }
        let mut trace = vec![state(&cursor)];
        for dt in ticks {
          cursor.advance(dt);
          trace.push(state(&cursor));
        }
        let points: Vec<_> = r
          .points
          .iter()
          .map(|p| {
            vec![
              i32::from(p.position_x),
              i32::from(p.position_y),
              i32::from(p.position_z),
              i32::from(p.rotation[0]),
              i32::from(p.rotation[1]),
              i32::from(p.rotation[2]),
              i32::from(p.rotation[3]),
              i32::from(p.width_left),
              i32::from(p.width_right),
              i32::from(p.packed_type_and_length),
            ]
          })
          .collect();
        output.push(serde_json::json!({"path":path,"mirror":mirror,"beginning":beginning,"ticks":ticks,"record":{"points":points,"start_position":r.start_position,"start_rotation":r.start_rotation,"loop_position":r.loop_position,"loop_rotation":r.loop_rotation,"loop_time":r.loop_time,"loop_index":r.loop_index},"trace":trace}));
      }
    }
  }
  println!("{}", serde_json::to_string(&output)?);
  Ok(())
}
