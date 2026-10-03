use lrformats::{appearance, library::Library, race_catalog};
use std::path::PathBuf;

fn library() -> Library {
  Library::open(
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../extracted/Program_Files_Group/LEGO.JAM"),
  )
  .unwrap()
}

#[test]
fn original_circuits_have_actual_five_rival_rosters_not_player_clones() {
  let l = library();
  let circuits = race_catalog::circuits(l.find_in("LEGORACE.CRB", "MENUDATA").unwrap()).unwrap();
  assert_eq!(circuits.len(), 7);
  assert_eq!(
    circuits.iter().find(|c| c.name == "c0").unwrap().racers,
    ["KK", "CR", "GB", "RH", "AD", "PH"]
  );
  assert_eq!(
    circuits.iter().find(|c| c.name == "c6").unwrap().racers,
    ["RR", "RR", "BK", "AL", "GS", "NH"]
  );
  let cars = appearance::cars(l.find_in("CHAMPS.CCB", "COMMON").unwrap()).unwrap();
  let drivers = appearance::drivers(l.find_in("DRIVERS.DDB", "COMMON").unwrap()).unwrap();
  assert_eq!(cars.len(), 20);
  assert_eq!(drivers.len(), 24);
  for circuit in &circuits {
    for name in &circuit.racers {
      let driver = drivers
        .iter()
        .find(|d| d.name.eq_ignore_ascii_case(name))
        .unwrap();
      let car = cars
        .iter()
        .find(|c| c.name.eq_ignore_ascii_case(&driver.car))
        .unwrap();
      for model in car.models.iter().chain(driver.models.iter()) {
        assert!(
          l.find_in(&format!("{model}.GDB"), "COMMON").is_some(),
          "missing original model{model}"
        );
        assert!(
          l.find_in(&format!("{model}.MDB"), "COMMON").is_some(),
          "missing original material{model}"
        );
      }
    }
  }
}

#[test]
fn menu_catalog_covers_all_original_tracks_and_preserves_mirrored_entries() {
  let l = library();
  let races = race_catalog::races(l.find_in("LEGORACE.RCB", "MENUDATA").unwrap()).unwrap();
  let circuits = race_catalog::circuits(l.find_in("LEGORACE.CRB", "MENUDATA").unwrap()).unwrap();
  assert_eq!(races.len(), 27);
  let mut tables = std::collections::BTreeSet::new();
  for race in &races {
    if race.name == "test" {
      continue;
    }
    if let Some(circuit) = &race.circuit {
      assert!(circuits.iter().any(|c| &c.name == circuit));
    } else {
      assert_eq!(race.name, "rr2");
    }
    assert!(
      l.tables().any(|t| t.eq_ignore_ascii_case(&race.table)),
      "missing original track{}",
      race.table
    );
    tables.insert(race.table.to_ascii_uppercase());
  }
  assert_eq!(tables.len(), 13);
  let normal = races.iter().find(|r| r.name == "rkr").unwrap();
  let mirrored = races.iter().find(|r| r.name == "rkr2").unwrap();
  assert_eq!(normal.table, "racec0r0");
  assert_eq!(normal.text_id, 4);
  assert_eq!(normal.circuit.as_deref(), Some("c1"));
  assert_eq!(mirrored.table, normal.table);
  assert!(!normal.mirrored);
  assert!(mirrored.mirrored);
  assert_eq!(mirrored.circuit.as_deref(), Some("c4"));
}
