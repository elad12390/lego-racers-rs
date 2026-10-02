//! Lightweight behavioral checks over the real PWB/config, not parity replay.
use lrsim::{powerups::{Powerups,Racer},power_weapons::Attack};
fn state()->(Powerups,Vec<Racer>) {
    let library=lrformats::library::Library::open(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../extracted/Program_Files_Group/LEGO.JAM")).unwrap();
    let placements=lrformats::powerup::parse(library.find_in("POWERUP.PWB","RACEC0R0").unwrap(),false).unwrap();
    (Powerups::new(placements,2).unwrap(),vec![Racer {position:[0.0,0.0,0.0],forward:[1.0,0.0,0.0],finished:false},Racer {position:[40.0,0.0,3.5],forward:[1.0,0.0,0.0],finished:false}])
}
#[test]
fn original_red_tiers_have_distinct_player_visible_results() {
    for (tier,attack) in [Attack::Cannon,Attack::Grapple,Attack::Lightning,Attack::Missile].into_iter().enumerate() {
        let (mut powers,racers)=state();powers.inventories[0].kind=1;powers.inventories[0].whites=tier as u8;
        assert!(powers.use_power(0,&racers));assert_eq!(powers.inventories[0].whites,0);
        for _ in 0..12 {powers.advance(1.0/60.0,&racers,|_,_|false);}
        assert!(powers.weapons.hits[attack as usize]>0,"{attack:?}");
        if attack==Attack::Grapple {assert_eq!(powers.inventories[0].grapple_target,Some(1));assert_eq!(powers.inventories[1].hit_ms,0.0);}
        else {assert!(powers.inventories[1].hit_ms>0.0);}
    }
}
#[test]
fn rear_weapons_oil_mine_and_curse_are_not_shared_missiles() {
    for (tier,attack) in [Attack::Oil,Attack::Barrel,Attack::MagneticMine,Attack::Curse].into_iter().enumerate() {
        let (mut powers,mut racers)=state();racers[1].position=[-40.0,0.0,3.5];powers.inventories[0].kind=4;powers.inventories[0].whites=tier as u8;
        assert!(powers.use_power(0,&racers));
        if matches!(attack,Attack::Oil|Attack::MagneticMine) {assert!(powers.weapons.projectiles.is_empty());racers[1].position=powers.weapons.zones[0].position;}
        for _ in 0..40 {powers.advance(1.0/60.0,&racers,|_,_|false);}
        assert!(powers.weapons.hits[attack as usize]>0,"{attack:?}");
        if attack==Attack::Oil {assert!(powers.inventories[1].oil_ms>0.0);}
        if attack==Attack::Curse {assert!(powers.inventories[1].curse_ms>0.0);}
    }
}
#[test]
fn shield_blocks_and_fourth_tier_reflects_to_previous_attacker() {
    let (mut powers,racers)=state();powers.inventories[0].kind=1;powers.inventories[1].shield_ms=1000.0;
    powers.use_power(0,&racers);for _ in 0..15 {powers.advance(1.0/60.0,&racers,|_,_|false);}
    assert_eq!(powers.inventories[1].hit_ms,0.0);assert_eq!(powers.weapons.blocked,1);
    powers.reset();powers.inventories[0].kind=1;powers.inventories[1].shield_ms=1000.0;powers.inventories[1].shield_tier=3;
    powers.use_power(0,&racers);for _ in 0..25 {powers.advance(1.0/60.0,&racers,|_,_|false);}
    assert_eq!(powers.weapons.reflected,1);assert!(powers.inventories[0].hit_ms>0.0);assert_eq!(powers.inventories[1].hit_ms,0.0);
}
