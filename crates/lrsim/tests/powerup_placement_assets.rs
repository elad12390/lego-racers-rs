//! Audit every original PWB anchor using the actual car contact ground.
//! Known authored/ground discrepancies are explicit, not silently repositioned.
use lrformats::library::Library;
use lrsim::{
  powerups::{Powerups, Racer},
  race_data::RaceData,
  vehicle::{Actions, Vehicle},
};

#[test]
fn every_track_pickup_is_audited_and_supported_pickups_are_collected_by_a_moving_car() {
  let library = Library::open(
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
      .join("../../extracted/Program_Files_Group/LEGO.JAM"),
  )
  .unwrap();
  let expected_counts = [39, 34, 42, 35, 36, 38, 29, 40, 52, 42, 52, 44, 38];
  let mut outliers = Vec::new();
  let mut total = 0;
  let mut supported = 0;
  for (track, table) in library
    .jam()
    .tables
    .iter()
    .filter(|t| t.name.starts_with("RACEC"))
    .enumerate()
  {
    let powers = Powerups::load(&library, &table.name, 1).unwrap();
    let data = RaceData::load(&library, &table.name, "bkchas0").unwrap();
    assert_eq!(
      powers.pickups.len(),
      expected_counts[track],
      "{}",
      table.name
    );
    let mut range = (f32::INFINITY, f32::NEG_INFINITY);
    let mut checked = 0;
    for (index, pickup) in powers.pickups.iter().enumerate() {
      let [x, y, z] = pickup.source.position;
      let hit = data.contacts.ground.at(x, y, z);
      if !hit.is_some_and(|h| (0.0..=10.0).contains(&(z - h.height))) {
        outliers.push((table.name.clone(), index));
        println!(
          "GROUND OUTLIER {} #{index} source={:?} ground={:?}",
          table.name,
          pickup.source.position,
          hit.map(|h| h.height)
        );
        continue;
      }
      let hit = hit.unwrap();
      range.0 = range.0.min(z - hit.height);
      range.1 = range.1.max(z - hit.height);
      // Normal and mirrored races both run canonical physics; the renderer's
      // actual mirrored rig transform is covered by powerup_animation tests.
      {
        let source = pickup.source.clone();
        let mut start = data.start.clone();
        start.position = [x, y, hit.height + 3.5];
        start.forward = [1.0, 0.0, 0.0];
        start.up = hit.normal;
        let mut car = Vehicle::spawn(&start, &data.chassis, &data.contacts);
        car.add_velocity([30.0, 0.0, 0.0]);
        car.step(
          Actions {
            throttle: 1.0,
            steer: 0.0,
          },
          &data.contacts,
          1.0 / 60.0,
        );
        let mut single = Powerups::new(vec![source], 1).unwrap();
        single.collect(&[Racer {
          position: car.position,
          forward: car.forward(),
          up: car.up,
          finished: false,
        }]);
        assert_eq!(
          single.inventories[0].pickups, 1,
          "{} #{index} car={:?}",
          table.name, car.position
        );
      }
      checked += 1;
    }
    total += powers.pickups.len();
    supported += checked;
    println!(
      "{} count={} grounded/collected={} source clearance={:.3}..{:.3}",
      table.name,
      powers.pickups.len(),
      checked,
      range.0,
      range.1
    );
  }
  assert_eq!(total, 521);
  assert_eq!(supported, 513);
  // Preserve and expose these original-data/primary-ground discrepancies;
  // moving them without checking track geometry would invent a new layout.
  assert_eq!(
    outliers,
    [
      ("RACEC0R2", 5),
      ("RACEC2R0", 32),
      ("RACEC2R0", 33),
      ("RACEC2R0", 34),
      ("RACEC2R0", 50),
      ("RACEC2R0", 51),
      ("RACEC2R1", 21),
      ("RACEC2R1", 37),
    ]
    .map(|(name, index)| (name.to_string(), index))
  );
}
