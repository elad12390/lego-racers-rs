//! Retained complete-original four-probe cases296..299 from
//! chassis-world-dispatch-retained-depth-comparison.json; real JAM checkpoint
//! mesh used as blocking geometry to isolate query order, not checkpoint policy.
use lrformats::{collision_tree, library::Library};
use lrsim::{
  chassis_dispatch::{self, Collider},
  collider_transform::ColliderTransform,
};

#[test]
fn owner_callbacks_precede_strict_record_selection_and_later_colliders_skip_only_improved_probes() {
  let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
  let library = Library::open(root.join("extracted/Program_Files_Group/LEGO.JAM")).unwrap();
  let data = library.find_in("CHCKPT00.BVB", "RACEC2R3").unwrap();
  let transform = ColliderTransform {
    origin: [0.0; 3],
    axes: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
  };
  let colliders: Vec<_> = (0..3)
    .map(|_| Collider {
      tree: std::sync::Arc::new(collision_tree::parse(data).unwrap()),
      transform,
    })
    .collect();
  let starts = [
    [-350.3821716308594, -342.3827209472656, 108.40132904052734],
    [-350.2774963378906, -344.3799743652344, 108.40132904052734],
    [-352.9466247558594, -293.44989013671875, 108.40132141113281],
    [-347.7130126953125, -393.3128356933594, 108.4013442993164],
  ];
  let ends = [starts[1], starts[0], starts[3], starts[2]];
  for (include_primary, blocked, expected_colliders) in [
    (false, false, vec![1, 1, 1, 1, 2]),
    (false, true, vec![1, 1, 1, 1, 2, 2, 2, 2]),
    (true, false, vec![1, 1, 1, 1, 2, 0]),
    (true, true, vec![1, 1, 1, 1, 2, 2, 2, 2, 0]),
  ] {
    let mut callbacks = Vec::new();
    let selection = chassis_dispatch::dispatch(
      &colliders,
      starts,
      ends,
      include_primary,
      |index, _, hit| {
        assert_eq!(hit.surface, 38);
        callbacks.push(index);
        !(index == 1 && blocked)
      },
    );
    assert_eq!(callbacks, expected_colliders);
    assert_eq!(selection.probe_hits, [true, true, true, false]);
    assert_eq!(
      selection.improvement_count, 3,
      "rounding distance before comparing loses a retained record improvement"
    );
    assert_eq!(selection.fraction, 0.4999999403953552);
  }
}

#[test]
fn real_primary_tree_wall_blocks_airborne_vehicle_and_applies_original_impact_turn() {
  let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
  let library = Library::open(root.join("extracted/Program_Files_Group/LEGO.JAM")).unwrap();
  let data = lrsim::race_data::RaceData::load(&library, "RACEC0R0", "bkchas0").unwrap();
  // Original COLLIDE.BVB triangle381: large vertical face at y675.99994.
  // Start12units before it, chassis query height3.5below the face centroid.
  let start = lrformats::world::StartPosition {
    slot: 0,
    position: [158.01162, 663.99994, 161.47655],
    forward: [0.0, 1.0, 0.0],
    up: [0.0, 0.0, 1.0],
  };
  let mut car = lrsim::vehicle::Vehicle::spawn(&start, &data.chassis, &data.contacts);
  car.set_velocity([0.0, 120.0, 0.0]);
  let initial_forward = car.forward();
  car.step(lrsim::vehicle::Actions::default(), &data.contacts, 0.25);
  assert!(car.collisions > 0, "real primary BSP face was bypassed");
  assert!(
    car.position[1] < 675.99994,
    "car crossed the original wall: {:?}",
    car.position
  );
  assert!(
    car.velocity()[1] < 0.0,
    "original rebound did not push away: {:?}",
    car.velocity()
  );
  assert!(
    (car.forward()[0] - initial_forward[0]).abs() > 0.1,
    "front-facing wall did not apply original200msimpact yaw"
  );
  assert_eq!(car.unresolved_contacts, 0);
}
