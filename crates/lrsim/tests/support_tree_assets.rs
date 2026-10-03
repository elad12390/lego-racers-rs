//! Retained original IGCOLLID persistent-query case68; downward-facing
//! winding must remain in the cache, not force a fresh tree lookup per wheel.
use lrformats::{collision_tree, library::Library};
use lrsim::{
  ground::Ground,
  support::{solve_cached, ContactCache, SupportCase},
};

#[test]
fn downward_winding_support_keeps_original_cached_hits_and_alignment() {
  let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
  let library = Library::open(root.join("extracted/Program_Files_Group/LEGO.JAM")).unwrap();
  let tree = collision_tree::parse(library.find_in("IGCOLLID.BVB", "RACEC0R1").unwrap()).unwrap();
  let ground = Ground::with_tree(std::sync::Arc::new(tree), Vec::new());
  let case = SupportCase {
    position: [348.6308898925781, 367.4441223144531, 38.806678771972656],
    forward: [1.0, 0.0, 0.0],
    left: [0.0, 1.0, 0.0],
    up: [0.0, 0.0, 1.0],
    base: [0.75, 0.75, -0.25],
    width: 0.25,
    length: 0.25,
    radius: 3.0,
    downward_movement: 0.0,
    grounded: true,
    dt: 0.01,
  };
  let hit = ground
    .trace_vertical(349.3808898925781, 367.9441223144531, 41.95668, 38.156677)
    .unwrap();
  assert!(
    hit.normal[2] < -0.98,
    "the original face must retain its winding"
  );
  let result = solve_cached(&case, &ground, &mut ContactCache::default());
  assert_eq!(result.wheels, 4);
  assert_eq!(
    result.position,
    [348.6308898925781, 367.4441223144531, 38.95833206176758]
  );
  assert_eq!(
    result.forward,
    [0.9867761731147766, 0.0, -0.16208870708942413]
  );
  assert_eq!(
    result.up,
    [0.16189414262771606, -0.048981815576553345, 0.9855917096138]
  );
}
