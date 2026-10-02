use lrformats::library::Library;
use lrsim::{race_data::RaceData,vehicle::{Vehicle,Actions}};
#[test]
fn falling_player_restores_last_supported_pose_and_can_drive_again() {
    let library=Library::open(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../extracted/Program_Files_Group/LEGO.JAM")).unwrap();
    let data=RaceData::load(&library,"RACEC0R0","bkchas0").unwrap();let mut car=Vehicle::spawn(&data.start,&data.chassis,&data.contacts);
    for _ in 0..50 {car.step(Actions {throttle:1.0,steer:0.0},&data.contacts,0.01);}
    let supported=car.position;let basis=car.basis();assert_eq!(car.supported_wheels,4);
    car.effect_position([supported[0],supported[1],-251.0]);car.set_velocity([10.0,5.0,-20.0]);car.step(Actions::default(),&data.contacts,0.01);
    assert_eq!(car.position,supported);assert_eq!(car.basis(),basis);assert_eq!(car.velocity(),[0.0;3]);assert_eq!(car.out_of_world_recoveries,1);
    for _ in 0..50 {car.step(Actions {throttle:1.0,steer:0.0},&data.contacts,0.01);}
    assert_eq!(car.supported_wheels,4);assert!(car.speed()>0.0);assert!(car.position!=supported);
    let next=car.position;car.effect_position([next[0],next[1],341.0]);car.step(Actions::default(),&data.contacts,0.01);assert_eq!(car.position,next);assert_eq!(car.out_of_world_recoveries,2);
}
