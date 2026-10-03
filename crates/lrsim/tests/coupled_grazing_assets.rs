//! Retained original normal-Step outcomes from complete400-frame5ms runs.
//! Source: coupled-world-forces-angular-spills-grazing.json; this is a finite
//! shared plane, not whole-track contact or physical keyboard/feel acceptance.
use lrformats::{
  cmb,
  library::Library,
  world::{CollisionMesh, CollisionTriangle, StartPosition},
};
use lrsim::{
  contact::Contacts,
  vehicle::{BodyControls, Vehicle},
};

fn fixture(slope: f32, speed: f32) -> (Contacts, Vehicle) {
  let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
  let library = Library::open(root.join("extracted/Program_Files_Group/LEGO.JAM")).unwrap();
  let chassis = cmb::parse(library.find_in("CHASSIS.CMB", "COMMON").unwrap())
    .unwrap()
    .into_iter()
    .find(|c| c.name.eq_ignore_ascii_case("bkchas0"))
    .unwrap();
  let contacts = Contacts::new(CollisionMesh {
    vertices: vec![
      [-1000.0, -1000.0, -1000.0 * slope],
      [3000.0, -1000.0, 3000.0 * slope],
      [-1000.0, 3000.0, -1000.0 * slope],
    ],
    triangles: vec![CollisionTriangle {
      indices: [0, 1, 2],
      surface: 0,
    }],
    ..Default::default()
  });
  let length = (1.0 + f64::from(slope).powi(2)).sqrt();
  let start = StartPosition {
    slot: 0,
    position: [0.0, 0.0, 0.2],
    forward: [
      (1.0 / length) as f32,
      0.0,
      (f64::from(slope) / length) as f32,
    ],
    up: [
      (-f64::from(slope) / length) as f32,
      0.0,
      (1.0 / length) as f32,
    ],
  };
  let mut car = Vehicle::spawn(&start, &chassis, &contacts);
  car.set_velocity([speed, 0.0, 0.0]);
  (contacts, car)
}

fn replay(slope: f32, throttle: f32, steer: f32, expected: &[(usize, u32, [f32; 3])]) {
  let (contacts, mut car) = fixture(slope, 20.0);
  for frame in 0..400 {
    step_body(&mut car,&contacts,throttle,steer,0.005);
    if let Some((_, wheels, position)) = expected.iter().find(|e| e.0 == frame) {
      assert_eq!(
        car.supported_wheels, *wheels,
        "frame{frame}:support diverged at a retained grazing transition"
      );
      assert!(
        car
          .position
          .into_iter()
          .zip(position)
          .all(|(a, b)| (a - b).abs() < 0.01),
        "frame{frame}:position {:?} != {position:?}",
        car.position
      );
    }
  }
  assert_eq!(car.unresolved_contacts, 0);
}

fn step_body(car:&mut Vehicle,contacts:&Contacts,throttle:f32,steer:f32,dt:f32){
  // The retained original trace explicitly sets body throttle/radius and calls
  // 00444ef0, without 0041fee0 Driver grip. Do not compare it to player steering.
  let controls=BodyControls{throttle:car.handling.player_throttle(throttle,car.drive_speed()),
    radius:car.handling.player_radius(steer,lrsim::contact::length(car.velocity())),reference_speed:car.handling.forward_limit};
  car.step_body_observed(controls,contacts,dt,&mut |_|{});
}

#[test]
fn flat_grazing_support_keeps_original_loss_and_recovery_timing() {
  replay(
    0.0,
    1.0,
    0.0,
    &[
      (0, 0, [0.10065732, 0.0, 0.2]),
      (5, 4, [0.62359524, 0.0, 0.2]),
      (242, 4, [59.538273, 0.0, 0.20000024]),
      (245, 2, [60.656033, 0.0, 0.20152594]),
      (246, 0, [61.030334, 0.0, 0.20152573]),
      (250, 0, [62.53605, 0.0, 0.010789142]),
      (251, 4, [62.914585, 0.0, 0.19875109]),
      (399, 4, [127.04914, 0.0, 0.1999995]),
    ],
  );
}

#[test]
fn sloped_braking_retains_original_two_wheel_then_airborne_transition() {
  replay(
    0.1,
    0.0,
    0.6,
    &[
      (98, 4, [7.6104975, 1.0919112, 0.9610486]),
      (99, 2, [7.6668897, 1.111183, 0.9666911]),
      (100, 0, [7.7229233, 1.1305554, 0.9722994]),
      (200, 4, [11.631551, 3.1658764, 1.3631536]),
      (399, 4, [13.800451, 6.2839913, 1.5800439]),
    ],
  );
}

#[test]
fn high_speed_steering_release_retains_then_expires_original_yaw_hold() {
  // Complete original profile10incontrol-release-hold-profiles-first.json:
  // release at80, reverse steering at140,10msframes and originalbkchas0.
  let expected = [
    (
      80,
      [43.182053, 65.63108, 0.1999997],
      [-0.39630127, 0.9181205, 0.0],
    ),
    (
      100,
      [31.921816, 86.083466, 0.19999975],
      [-0.7590515, 0.65103054, 0.0],
    ),
    (
      120,
      [15.761966, 102.0387, 0.20000003],
      [-0.7590515, 0.65103054, 0.0],
    ),
    (
      140,
      [-1.5513595, 117.13973, 0.19999985],
      [-0.74281764, 0.66949385, 0.0],
    ),
    (
      239,
      [35.22641, 198.76323, 0.20000005],
      [0.9936056, -0.112906314, 0.0],
    ),
  ];
  let (contacts, mut car) = fixture(0.0, 110.0);
  for frame in 0..240 {
    step_body(&mut car,&contacts,1.0,
        if frame < 80 {
          1.0
        } else if frame < 140 {
          0.0
        } else {
          -1.0
        },
      0.01,
    );
    if let Some((_, position, forward)) = expected.iter().find(|e| e.0 == frame) {
      assert!(
        car
          .position
          .into_iter()
          .zip(position)
          .all(|(a, b)| (a - b).abs() < 0.01),
        "frame{frame}:position {:?}",
        car.position
      );
      assert!(
        car
          .forward()
          .into_iter()
          .zip(forward)
          .all(|(a, b)| (a - b).abs() < 0.0005),
        "frame{frame}:steering release changed yaw-hold behavior: {:?}",
        car.forward()
      );
    }
  }
}
