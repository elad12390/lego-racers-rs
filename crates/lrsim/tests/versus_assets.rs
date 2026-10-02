//! Focused two-player behavior over the actual original track and collider tree.
use lrformats::{library::Library,world};
use lrsim::{race_data::RaceData,powerups::Powerups,vehicle::Actions,versus::Session,world_dispatch::ChassisWorld};
fn fixture()->([RaceData;2],Powerups) {
    let library=Library::open(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../extracted/Program_Files_Group/LEGO.JAM")).unwrap();
    let mut data=[RaceData::load(&library,"RACEC0R0","bkchas0").unwrap(),RaceData::load(&library,"RACEC0R0","bkchas0").unwrap()];
    let table=library.jam().tables.iter().find(|t|t.name=="RACEC0R0").unwrap();let entry=table.entries.iter().find(|e|e.name.ends_with(".SPB")).unwrap();
    let mut starts=world::start_positions(library.jam().bytes(entry).unwrap()).unwrap();starts.sort_by_key(|s|s.slot);for i in 0..2 {data[i].start=starts[i].clone();}
    (data,Powerups::load(&library,"RACEC0R0",2).unwrap())
}
#[test]
fn shared_countdown_independent_actions_pause_and_restart() {
    let (data,powers)=fixture();let mut game=Session::new(&data,powers).unwrap();let initial=game.cars.each_ref().map(|c|c.position);
    let input=[Actions {throttle:1.0,steer:0.0},Actions::default()];
    for _ in 0..120 {game.step(1.0/60.0,input,[false;2]).unwrap();}assert_eq!(game.cars.each_ref().map(|c|c.position),initial);assert_eq!(game.elapsed,0.0);
    for _ in 0..180 {game.step(1.0/60.0,input,[false;2]).unwrap();}
    let distances=std::array::from_fn::<_,2,_>(|i|lrsim::contact::length(lrsim::contact::sub(game.cars[i].position,initial[i])));assert!(distances[0]>30.0);assert!(distances[1]<1.0,"stationary P2 moved {distances:?}");
    let paused=(game.elapsed,game.cars.each_ref().map(|c|c.position));for _ in 0..20 {game.step(0.0,input,[true;2]).unwrap();}assert_eq!((game.elapsed,game.cars.each_ref().map(|c|c.position)),paused);
    game.powers.inventories[1].kind=3;game.reset().unwrap();assert_eq!(game.elapsed,0.0);assert_eq!(game.cars.each_ref().map(|c|c.position),initial);assert!(game.powers.inventories.iter().all(|i|i.kind==0&&i.pickups==0));assert!(game.races.iter().all(|r|!r.finished&&r.laps.completed_laps==0));
}
#[test]
fn two_free_cars_displace_and_exchange_impulse_on_actual_track() {
    let (data,powers)=fixture();let mut game=Session::new(&data,powers).unwrap();
    let position=game.cars[0].position;let forward=game.cars[0].forward();game.cars[1].effect_position(std::array::from_fn(|i|position[i]+forward[i]*6.0));game.cars[0].set_velocity(forward.map(|v|v*40.0));
    let before=game.cars.each_ref().map(|c|c.position);let world=ChassisWorld::new(data[0].contacts.primary_collider().unwrap().clone(),&data[0].checkpoints);
    let [a,b]=&mut game.cars;let (count,_)=lrsim::race_contacts::resolve_players(a,&data[0].chassis,b,&data[1].chassis,&data[0].contacts,&world,game.positions.first_two_states_mut());
    assert!(count>0);assert_ne!(game.cars.each_ref().map(|c|c.position),before);assert!(game.cars[1].speed()>0.0);
    let momentum=std::array::from_fn::<_,3,_>(|i|game.cars[0].velocity()[i]*data[0].chassis.mass+game.cars[1].velocity()[i]*data[1].chassis.mass);
    for i in 0..3 {assert!((momentum[i]-forward[i]*40.0*data[0].chassis.mass).abs()<0.1);}assert!(game.cars.iter().all(|c|c.unresolved_contacts==0&&c.velocity().iter().all(|v|v.is_finite())));
}
#[test]
fn shared_pickup_is_owned_once_and_player_two_uses_its_own_inventory() {
    let (data,powers)=fixture();let mut game=Session::new(&data,powers).unwrap();let pickup=game.powers.pickups.iter().find(|p|p.source.kind==3).unwrap().source.position;
    game.cars[0].effect_position(pickup);game.cars[1].effect_position(pickup);game.powers.collect(&game.actors());assert_eq!(game.powers.inventories.iter().map(|i|i.pickups).sum::<u32>(),1);
    game.powers.inventories[1].kind=3;game.powers.inventories[1].whites=2;let first=game.powers.inventories[0].kind;assert!(game.powers.use_power(1,&game.actors()));assert_eq!(game.powers.inventories[1].uses,1);assert_eq!(game.powers.inventories[1].kind,0);assert!(game.powers.inventories[1].turbo_ms>0.0);assert_eq!(game.powers.inventories[0].kind,first);assert_eq!(game.powers.inventories[0].uses,0);
}
