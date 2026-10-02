//! Actual original assets and native vehicle, no replayed/teleported car poses.
use lrformats::library::Library;
use lrsim::{
    diagnostic_driver::DiagnosticDriver, lap_zones::LapZones, race::Race, race_data::RaceData,
    vehicle::Vehicle,
};
use std::path::PathBuf;

fn data() -> RaceData {
    let jam = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../extracted/Program_Files_Group/LEGO.JAM");
    RaceData::load(&Library::open(jam).unwrap(), "RACEC0R0", "bkchas0").unwrap()
}

fn drive_three_laps(data: &RaceData) -> Race {
    let mut car = Vehicle::spawn(&data.start, &data.chassis, &data.contacts);
    let mut driver = DiagnosticDriver::new(data.diagnostic_line.clone());
    let mut race = Race::new(3).unwrap();
    for tick in 0..180 * 120 {
        let old = car.position;
        car.step(driver.actions(&car), &data.contacts, 1.0 / 120.0);
        race.advance(old, car.position, &data.lap_zones, 1.0 / 120.0);
        assert!(car.position.iter().chain(&car.velocity()).all(|v| v.is_finite()),"tick {tick}: position {:?}, velocity {:?}, forward {:?}, up {:?}, wheels {}",car.position,car.velocity(),car.forward(),car.up,car.supported_wheels);
        assert_eq!(car.unresolved_contacts,0,"tick{tick}: chassis collision safety termination is not accepted gameplay");
        if race.finished {
            break;
        }
    }
    assert!(
        race.finished,
        "lap history {:?} events {}",
        race.laps,
        race.events.len()
    );
    assert_eq!(race.laps.completed_laps, 3);
    assert_eq!(race.lap_times.len(), 3);
    assert!(race.lap_times.iter().all(|t| *t > 20.0 && *t < 60.0));
    let events: Vec<_> = race
        .events
        .iter()
        .filter(|e| e.state_changed)
        .map(|e| e.event.id)
        .collect();
    assert_eq!(
        events,
        vec![101, 102, 100, 101, 102, 100, 101, 102, 100, 101]
    );
    race
}

#[test]
fn actual_vehicle_drives_three_spatially_counted_laps_and_can_restart() {
    let data = data();
    let first = drive_three_laps(&data);
    let second = drive_three_laps(&data);
    assert_eq!(first.elapsed, second.elapsed);
    assert_eq!(first.lap_times, second.lap_times);
}

#[test]
fn finish_gate_is_finite_and_repeated_crossing_is_not_a_lap() {
    let data = data();
    let inside = [362.0, 216.0, 1.0];
    let outside = [362.0, 218.0, 1.0];
    let first = data.lap_zones.crossings(inside, outside);
    assert_eq!(first.len(), 1);
    assert_eq!((first[0].id, first[0].mode), (101, 1));
    assert!(data
        .lap_zones
        .crossings([1000.0, 216.0, 1.0], [1000.0, 218.0, 1.0])
        .is_empty());
    let mut race = Race::new(3).unwrap();
    for _ in 0..10 {
        race.advance(inside, outside, &data.lap_zones, 1.0);
        race.advance(outside, inside, &data.lap_zones, 1.0);
    }
    assert_eq!(race.laps.counter, 0);
    assert_eq!(race.laps.completed_laps, 0);
    assert!(!race.finished);
}

#[test]
fn clock_and_results_freeze_after_spatial_finish() {
    let data = data();
    let mut race = drive_three_laps(&data);
    let before = (race.elapsed, race.events.len(), race.lap_times.clone());
    race.advance(
        [362.0, 216.0, 1.0],
        [362.0, 218.0, 1.0],
        &data.lap_zones,
        100.0,
    );
    assert_eq!((race.elapsed, race.events.len(), race.lap_times), before);
}

#[test]
fn all_shipped_races_resolve_original_spatial_lap_zones() {
    let jam = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../extracted/Program_Files_Group/LEGO.JAM");
    let library = Library::open(jam).unwrap();
    for table in &library.jam().tables {
        if table.name.starts_with("RACEC") {
            LapZones::load(&library, &table.name).unwrap_or_else(|e| panic!("{e}"));
        }
    }
}

#[test]
fn diagnostic_car_does_not_get_stuck_on_nonblocking_trigger_materials() {
    let jam = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../extracted/Program_Files_Group/LEGO.JAM");
    let library = Library::open(jam).unwrap();
    let data = RaceData::load(&library, "RACEC3R0", "bkchas0").unwrap();
    let race = drive_three_laps(&data);
    assert!(race.elapsed < 180.0);
}

#[test]
fn pyramid_race_counts_laps_using_its_own_trigger_file() {
    let jam = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../extracted/Program_Files_Group/LEGO.JAM");
    let library = Library::open(jam).unwrap();
    let data = RaceData::load(&library, "RACEC0R2", "bkchas0").unwrap();
    let race = drive_three_laps(&data);
    assert!(race.elapsed < 180.0);
}

#[test]
fn full_race_with_landings_does_not_wedge_collision_probes() {
    let jam=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../extracted/Program_Files_Group/LEGO.JAM");
    let library=Library::open(jam).unwrap();
    let data=RaceData::load(&library,"RACEC2R2","bkchas0").unwrap();
    drive_three_laps(&data);
}

#[test]
fn three_laps_on_curved_sloped_track_retain_support_after_contacts() {
    let jam=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../extracted/Program_Files_Group/LEGO.JAM");
    let library=Library::open(jam).unwrap();
    let data=RaceData::load(&library,"RACEC1R3","bkchas0").unwrap();
    drive_three_laps(&data);
}

#[test]
fn tilted_wall_contacts_do_not_leave_car_inverted_and_stalled() {
    let jam=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../extracted/Program_Files_Group/LEGO.JAM");
    let library=Library::open(jam).unwrap();
    let data=RaceData::load(&library,"RACEC2R1","bkchas0").unwrap();
    drive_three_laps(&data);
}

#[test]
fn held_throttle_cannot_move_car_or_clock_during_restart_countdown() {
    let data=data();
    for _ in 0..2 {
        let mut car=Vehicle::spawn(&data.start,&data.chassis,&data.contacts);
        let mut race=Race::new(3).unwrap();
        let mut gate=lrsim::start_gate::StartGate::default();
        for _ in 0..299 {
            let dt=gate.advance(0.01);
            let old=car.position;
            car.step(lrsim::vehicle::Actions {throttle:1.0,steer:0.0},&data.contacts,dt as f32);
            race.advance(old,car.position,&data.lap_zones,dt);
        }
        assert_eq!(car.position,data.start.position);
        assert_eq!(car.velocity(),[0.0;3]);
        assert_eq!(race.elapsed,0.0);
        assert!(race.events.is_empty());
        let dt=gate.advance(0.02);
        let old=car.position;
        car.step(lrsim::vehicle::Actions {throttle:1.0,steer:0.0},&data.contacts,dt as f32);
        race.advance(old,car.position,&data.lap_zones,dt);
        assert!(gate.released());assert_ne!(car.position,old);
        assert!((race.elapsed-0.01).abs()<1e-12);
    }
}
