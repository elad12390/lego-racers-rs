//! Retained mixed-probe original case: checkpoint callbacks are nonblocking,
//! precede primary selection and survive the collision retry result.
use lrformats::{collision_tree, library::Library};
use lrsim::{
  chassis_dispatch::Collider,
  checkpoint_contacts::{CheckpointContacts, State},
  collider_transform::ColliderTransform,
  world_dispatch::ChassisWorld,
};

#[test]
fn checkpoint_progress_survives_a_primary_collision_retry() {
  let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
  let library = Library::open(root.join("extracted/Program_Files_Group/LEGO.JAM")).unwrap();
  let checkpoints = CheckpointContacts::load(&library, "RACEC0R0").unwrap();
  let tree = std::sync::Arc::new(
    collision_tree::parse(library.find_in("COLLIDE.BVB", "RACEC0R0").unwrap()).unwrap(),
  );
  let world = ChassisWorld::new(
    Collider {
      tree,
      transform: ColliderTransform {
        origin: [0.0; 3],
        axes: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
      },
    },
    &checkpoints,
  );
  let starts = [
    [390.62607, 223.5385, -3.5395381],
    [408.39218, 257.9692, 0.9999693],
    [390.62607, 223.5385, -3.5395381],
    [377.99997, 490.50513, 16.986876],
  ];
  let ends = [
    [390.62607, 225.5385, -3.5395381],
    [408.39218, 257.9692, -1.0000306],
    [390.62607, 225.5385, -3.5395381],
    [377.99997, 490.8281, 15.013125],
  ];
  let mut state = State::default();
  let mut order = Vec::new();
  let (selection, count) = world.dispatch(
    starts,
    ends,
    true,
    &mut state,
    |_, _, _| true,
    |collider, probe, _| order.push((collider, probe)),
  );
  assert_eq!(order, [(1, 0), (1, 2), (0, 1), (0, 3)]);
  assert_eq!(count, 2);
  assert_eq!(state.checkpoint, Some(0));
  assert_eq!(state.contact_count, 0);
  assert_eq!(selection.probe_hits, [false, true, false, true]);
  assert_eq!(selection.improvement_count, 2);
  assert_eq!(
    ((40.0 * f64::from(selection.fraction)) as u32).saturating_sub(5),
    14
  );
}

#[test]
fn route_collision_retries_keep_predicted_cursor_and_checkpoint_callbacks() {
  let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
  let library = Library::open(root.join("extracted/Program_Files_Group/LEGO.JAM")).unwrap();
  let checkpoints = CheckpointContacts::load(&library, "RACEC0R0").unwrap();
  let tree = std::sync::Arc::new(
    collision_tree::parse(library.find_in("COLLIDE.BVB", "RACEC0R0").unwrap()).unwrap(),
  );
  let world = ChassisWorld::new(
    Collider {
      tree,
      transform: ColliderTransform {
        origin: [0.0; 3],
        axes: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
      },
    },
    &checkpoints,
  );
  let record =
    lrformats::route::RouteRecord::load(library.find_in("R1_F_0.RRB", "RACEC0R0").unwrap(), false)
      .unwrap();
  let mut motion = lrsim::route_motion::ContactFreeRoute::at_start(&record);
  motion.cursor.speed = 0.0;
  let mut state = State::default();
  let mut callbacks = 0;
  let mut blocked = Vec::new();
  for _ in 0..27 {
    blocked.push(motion.advance_queried(200, [10.0, 16.0], |starts, ends| {
      // Ordinary blocking nondigit surfaces, exactly as in the retained
      // complete-original-call fixture. Digit CPB callbacks remain real.
      let (selected, count) = world.dispatch(
        starts,
        ends,
        false,
        &mut state,
        |_, _, _| true,
        |_, _, _| {},
      );
      callbacks += count;
      selected
    }));
    if blocked.len() == 25 {
      assert!((motion.cursor.time - 4102.21875).abs() < 0.002);
      for (actual, original) in motion
        .position
        .into_iter()
        .zip([337.00214, 526.5366, 19.19997])
      {
        assert!((actual - original).abs() < 0.002);
      }
      assert_eq!(motion.cursor.speed, -0.1);
    }
  }
  assert!(blocked[..24].iter().all(|b| !b));
  assert_eq!(&blocked[24..], &[true, true, true]);
  // Zero-length retry keeps the external predicted pose, the retained starts
  // and saved velocity; it must not undo cursor progress or owner callbacks.
  assert!((motion.cursor.time - 4101.7314).abs() < 0.002);
  assert_eq!(motion.velocity, [0.0; 3]);
  assert!(callbacks > 0);
  assert!(state.checkpoint.is_some());
}

#[test]
fn secondary_wheels_precede_primary_and_unhit_wheels_use_primary_cache() {
  let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
  let library = Library::open(root.join("extracted/Program_Files_Group/LEGO.JAM")).unwrap();
  let tree = std::sync::Arc::new(
    collision_tree::parse(library.find_in("COLLIDE.BVB", "RACEC0R2").unwrap()).unwrap(),
  );
  let ground = lrsim::ground::Ground::with_tree(tree.clone(), Vec::new());
  let secondary = Collider {
    tree,
    transform: ColliderTransform {
      origin: [11.25, -7.5, 0.125],
      axes: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
    },
  };
  let case = lrsim::support::SupportCase {
    position: [693.69196, -222.0, -44.96425],
    forward: [0.8, 0.6, 0.0],
    left: [-0.6, 0.8, 0.0],
    up: [0.0, 0.0, 1.0],
    base: [0.75, 0.75, -0.25],
    width: 0.25,
    length: 0.25,
    radius: 3.0,
    downward_movement: 0.0,
    grounded: true,
    dt: 0.008,
  };
  let mut cache = lrsim::support::ContactCache::default();
  for _ in 0..2 {
    let support =
      lrsim::support::solve_world(&case, &ground, &mut cache, std::slice::from_ref(&secondary));
    assert_eq!(support.colliders, [Some(1), Some(0), Some(1), Some(0)]);
    assert_eq!(support.wheels, 2);
    assert_eq!(support.surfaces, [Some(2); 4]);
    assert!((support.position[2] - (-42.63756)).abs() < 0.00001);
    assert!((support.up[0] - 0.005370842).abs() < 0.00001);
  }
}
