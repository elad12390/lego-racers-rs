//! Observable recovery boundaries on a real shipped route. Expected poses and
//! controls additionally compared with complete original DriverTick in tools.
use lrsim::recovery_driver::{RecoveryDriver,Pose,State};

fn record()->lrformats::route::RouteRecord {
    let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path=root.join("assets/jam/GAMEDATA/RACEC0R0/R1_F_0.RRB");
    lrformats::route::RouteRecord::load(&std::fs::read(path).unwrap(),false).unwrap()
}
fn state()->State {
    State {target:[100.0,0.0,0.0],rotation:[0.0,0.0,0.0,1.0],route_time:250,
        flags:2|0x40,steer_current:0.0,steer_target:0.0,skid_ms:990}
}
fn pose()->Pose {
    Pose {position:[0.0;3],forward:[1.0,0.0,0.0],left:[0.0,1.0,0.0],direction:[1.0,0.0,0.0],
        wheels:4,speed:0.0,forward_speed_ms:0.0,spin_hold:false}
}

#[test]
fn stuck_racer_reverses_then_retries_forward_without_teleporting() {
    let record=record();let mut driver=RecoveryDriver::new(&record,state());
    let reverse=driver.tick(&pose(),10).unwrap();
    assert!(reverse.rejoin.is_none());assert_eq!(reverse.throttle,-1.0);
    for _ in 0..199 {assert_eq!(driver.tick(&pose(),10).unwrap().throttle,-1.0);}
    let retry=driver.tick(&pose(),10).unwrap();
    assert!(retry.rejoin.is_none());assert_eq!(retry.throttle,1.0/3.0);
}

#[test]
fn recovery_rejoins_only_below_three_units_and_disables_pursuit() {
    let record=record();let mut initial=state();initial.target=[3.0,0.0,0.0];
    let mut driver=RecoveryDriver::new(&record,initial);
    assert!(driver.tick(&pose(),10).unwrap().rejoin.is_none());
    driver.state.target=[2.9,0.0,0.0];
    let command=driver.tick(&pose(),10).unwrap();
    assert_eq!(command.throttle,0.0);assert_eq!(command.turn_radius,0.0);
    let rejoin=command.rejoin.unwrap();assert_eq!(rejoin.position,record.loop_position);
    assert_eq!(driver.state.flags&0x40,0);assert_eq!(driver.state.route_time,0);
}

#[test]
fn spin_hold_cancels_reverse_and_initializer_uses_shipped_route() {
    let record=record();let mut initial=state();initial.flags|=0x20;
    let mut driver=RecoveryDriver::new(&record,initial);let mut held=pose();held.spin_hold=true;
    assert_eq!(driver.tick(&held,10).unwrap().throttle,1.0/3.0);
    assert_eq!(driver.state.flags&0x20,0);driver.begin();
    assert_eq!(driver.state.route_time,1000);assert_ne!(driver.state.target,[100.0,0.0,0.0]);
    assert!(driver.state.target.into_iter().all(f32::is_finite));
}
