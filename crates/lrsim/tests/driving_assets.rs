//! Real original JAM, chassis/start/collision data. No stub terrain or mock car.
use lrformats::library::Library;
use lrsim::{
    race_data::RaceData,
    vehicle::{Actions, Vehicle},
};
use std::path::PathBuf;

fn race() -> RaceData {
    let jam = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../extracted/Program_Files_Group/LEGO.JAM");
    RaceData::load(&Library::open(jam).unwrap(), "RACEC0R0", "bkchas0").unwrap()
}

#[test]
fn spawn_preserves_the_original_start_slot_and_height() {
    let data = race();
    let car = Vehicle::spawn(&data.start, &data.chassis, &data.contacts);
    assert_eq!(car.position, data.start.position);
    assert!(car.grounded);
}

#[test]
fn player_throttle_brakes_then_drives_in_reverse_without_a_guessed_speed_limit() {
    let data = race();
    let mut car = Vehicle::spawn(&data.start, &data.chassis, &data.contacts);
    let mut driver = lrsim::diagnostic_driver::DiagnosticDriver::new(data.diagnostic_line.clone());
    for _ in 0..600 {
        car.step(driver.actions(&car), &data.contacts, 1.0 / 120.0);
    }
    assert!(car.speed() > 100.0 && car.speed() < car.handling.forward_limit + 5.0);
    let heading = car.heading;
    for _ in 0..120 {
        car.step(
            Actions {
                throttle: -1.0,
                steer: 0.0,
            },
            &data.contacts,
            1.0 / 120.0,
        );
    }
    assert!(car.speed() < 10.0, "braking speed {}", car.speed());
    for _ in 0..120 {
        car.step(
            Actions {
                throttle: -1.0,
                steer: 0.0,
            },
            &data.contacts,
            1.0 / 120.0,
        );
    }
    assert!(car.speed() < 0.0, "reverse speed {}", car.speed());
    assert!((car.heading - heading).abs() < 0.001);
}

#[test]
fn unsupported_car_falls_under_original_airborne_gravity() {
    let data = race();
    let mut car = Vehicle::spawn(&data.start, &data.chassis, &data.contacts);
    car.position[2] += 20.0;
    car.grounded = false;
    let initial = car.position[2];
    car.step(Actions::default(), &data.contacts, 0.25);
    // Original00445500: fourfold39units/sec²; integrator uses half a*t².
    assert!((car.velocity()[2] + 39.0).abs() < 0.001);
    assert!((car.position[2] - (initial - 4.875)).abs() < 0.001);
    assert!(!car.grounded);
    for _ in 0..120 {
        car.step(Actions::default(), &data.contacts, 1.0 / 120.0);
    }
    assert!(car.grounded);
    // Spawn preserves SPB; landing follows original support alignment instead:
    // Reset_Contacts/AlignToSupportingContacts adds0.2Z (x86 comparison retained).
    let deck = data
        .contacts
        .ground
        .at(car.position[0], car.position[1], car.position[2])
        .unwrap();
    assert!((car.position[2] - (deck.height + 0.2)).abs() < 0.001);
}

#[test]
fn original_hard_landing_rebounds_before_settling_on_actual_track() {
    let data=race();
    let mut car=Vehicle::spawn(&data.start,&data.chassis,&data.contacts);
    car.position[2]+=30.0;car.grounded=false;
    let mut bounced=false;
    for _ in 0..240 {
        let descending=car.velocity()[2]< -50.0;
        car.step(Actions::default(),&data.contacts,1.0/120.0);
        if descending && car.velocity()[2]>0.0 {
            assert!(!car.grounded);
            assert_eq!(car.supported_wheels,0);
            bounced=true;
        }
    }
    assert!(bounced,"hard landing was snapped to rest instead of original rebound");
    assert!(car.grounded);
}

#[test]
fn leaving_previous_support_applies_one_original_drop_before_airborne_gravity() {
    let data=race();
    let mut car=Vehicle::spawn(&data.start,&data.chassis,&data.contacts);
    // Model an already-grounded body losing contact. Retain its prior support
    // state, but move the fixture above the real track so all four rays miss.
    car.position[2]+=20.0;
    let height=car.position[2];
    car.step(Actions::default(),&data.contacts,0.005);
    assert!(!car.grounded);
    assert_eq!(car.supported_wheels,0);
    assert!((car.position[2]-height).abs()<0.001);
    assert!((car.velocity()[2]+8.0).abs()<0.001,"support-loss velocity {:?}",car.velocity());
    car.step(Actions::default(),&data.contacts,0.005);
    assert!((car.velocity()[2]+8.78).abs()<0.001,"drop was applied repeatedly: {:?}",car.velocity());
}

#[test]
fn steering_moves_the_car_both_ways_on_real_start_grid_ground() {
    let data = race();
    let mut left = Vehicle::spawn(&data.start, &data.chassis, &data.contacts);
    let mut right = Vehicle::spawn(&data.start, &data.chassis, &data.contacts);
    let heading = left.heading;
    let forward = left.forward();
    left.set_velocity(forward.map(|v| v * 50.0));
    right.set_velocity(left.velocity());
    for _ in 0..30 {
        left.step(
            Actions {
                throttle: 0.0,
                steer: 1.0,
            },
            &data.contacts,
            1.0 / 120.0,
        );
        right.step(
            Actions {
                throttle: 0.0,
                steer: -1.0,
            },
            &data.contacts,
            1.0 / 120.0,
        );
    }
    assert!(left.heading > heading + 0.15);
    assert!(right.heading < heading - 0.15);
    assert!(
        left.position[0] < data.start.position[0] && right.position[0] > data.start.position[0]
    );
    assert!(left.grounded && right.grounded);
}

#[test]
fn uphill_driving_does_not_detach_based_on_world_vertical_speed() {
    let jam = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../extracted/Program_Files_Group/LEGO.JAM");
    let data = RaceData::load(&Library::open(jam).unwrap(), "RACEC0R2", "bkchas0").unwrap();
    let mut car = Vehicle::spawn(&data.start, &data.chassis, &data.contacts);
    let mut driver = lrsim::diagnostic_driver::DiagnosticDriver::new(data.diagnostic_line);
    for _ in 0..20 * 120 {
        car.step(driver.actions(&car), &data.contacts, 1.0 / 120.0);
        assert!(
            car.position[2] > -500.0,
            "car fell through original track: {:?}",
            car.position
        );
    }
    assert!(
        driver.index > 120,
        "car failed to progress on steep track: {}",
        driver.index
    );
}
