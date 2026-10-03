//! Lightweight behavioral checks over the real PWB/config, not parity replay.
use lrsim::{
  power_weapons::Attack,
  powerups::{Powerups, Racer},
};
fn state() -> (Powerups, Vec<Racer>) {
  let library = lrformats::library::Library::open(
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
      .join("../../extracted/Program_Files_Group/LEGO.JAM"),
  )
  .unwrap();
  let placements =
    lrformats::powerup::parse(library.find_in("POWERUP.PWB", "RACEC0R0").unwrap(), false).unwrap();
  (
    Powerups::new(placements, 2).unwrap(),
    vec![
      Racer {
        position: [0.0, 0.0, 0.0],
        forward: [1.0, 0.0, 0.0],
        up: [0.0, 0.0, 1.0],
        finished: false,
      },
      Racer {
        position: [40.0, 0.0, 3.5],
        forward: [1.0, 0.0, 0.0],
        up: [0.0, 0.0, 1.0],
        finished: false,
      },
    ],
  )
}

#[test]
fn pickup_respawn_fades_in_before_it_becomes_collectable_and_clock_does_not_wrap() {
  let (mut powers, mut racers) = state();
  powers.pickups.retain(|p| p.kind != 0);
  powers.pickups.truncate(1);
  racers.truncate(1);
  racers[0].position = powers.pickups[0].source.position;
  powers.collect(&racers);
  assert_eq!(powers.inventories[0].pickups, 1);
  assert_eq!(powers.pickups[0].opacity(&powers.rules), 0);
  let delay = powers.pickups[0].source.delay_ms;
  powers.advance((delay + 200) as f32 / 1000.0, &racers, |_, _| false);
  assert!((126..=128).contains(&powers.pickups[0].opacity(&powers.rules)));
  powers.collect(&racers);
  assert_eq!(powers.inventories[0].pickups, 1);
  powers.advance(0.4, &racers, |_, _| false);
  assert_eq!(powers.pickups[0].opacity(&powers.rules), 255);
  powers.collect(&racers);
  assert_eq!(powers.inventories[0].pickups, 2);
  assert_eq!(
    powers.pickups[0].opacity(&powers.rules),
    0,
    "swap is not a respawn fade"
  );
  assert!(
    powers.elapsed_seconds() > std::f32::consts::TAU / powers.rules.rotation_radians_per_second
  );
  powers.reset();
  assert_eq!(powers.elapsed_seconds(), 0.0);
  assert_eq!(powers.pickups[0].opacity(&powers.rules), 255);
}
#[test]
fn original_red_tiers_have_distinct_player_visible_results() {
  for (tier, attack) in [
    Attack::Cannon,
    Attack::Grapple,
    Attack::Lightning,
    Attack::Missile,
  ]
  .into_iter()
  .enumerate()
  {
    let (mut powers, racers) = state();
    powers.inventories[0].kind = 1;
    powers.inventories[0].whites = tier as u8;
    assert!(powers.use_power(0, &racers));
    assert_eq!(powers.inventories[0].whites, 0);
    for _ in 0..12 {
      powers.advance(1.0 / 60.0, &racers, |_, _| false);
    }
    assert!(powers.weapons.hits[attack as usize] > 0, "{attack:?}");
    if attack == Attack::Grapple {
      assert_eq!(powers.inventories[0].grapple_target, Some(1));
      assert_eq!(powers.inventories[1].hit_ms, 0.0);
    } else {
      assert!(powers.inventories[1].hit_ms > 0.0);
    }
  }
}
#[test]
fn rear_weapons_oil_mine_and_curse_are_not_shared_missiles() {
  for (tier, attack) in [
    Attack::Oil,
    Attack::Barrel,
    Attack::MagneticMine,
    Attack::Curse,
  ]
  .into_iter()
  .enumerate()
  {
    let (mut powers, mut racers) = state();
    racers[1].position = [-40.0, 0.0, 3.5];
    powers.inventories[0].kind = 4;
    powers.inventories[0].whites = tier as u8;
    assert!(powers.use_power(0, &racers));
    if matches!(attack, Attack::Oil | Attack::MagneticMine) {
      assert!(powers.weapons.projectiles.is_empty());
      racers[1].position = powers.weapons.zones[0].position;
    }
    for _ in 0..40 {
      powers.advance(1.0 / 60.0, &racers, |_, _| false);
    }
    assert!(powers.weapons.hits[attack as usize] > 0, "{attack:?}");
    if attack == Attack::Oil {
      assert!(powers.inventories[1].oil_ms > 0.0);
    }
    if attack == Attack::Curse {
      assert!(powers.inventories[1].curse_ms > 0.0);
    }
  }
}
#[test]
fn shield_blocks_and_fourth_tier_reflects_to_previous_attacker() {
  let (mut powers, racers) = state();
  powers.inventories[0].kind = 1;
  powers.inventories[1].shield.start(1000);
  powers.use_power(0, &racers);
  for _ in 0..15 {
    powers.advance(1.0 / 60.0, &racers, |_, _| false);
  }
  assert_eq!(powers.inventories[1].hit_ms, 0.0);
  assert_eq!(powers.weapons.blocked, 1);
  powers.reset();
  powers.inventories[0].kind = 1;
  powers.inventories[1].shield.start(1000);
  powers.inventories[1].shield_tier = 3;
  powers.use_power(0, &racers);
  for _ in 0..25 {
    powers.advance(1.0 / 60.0, &racers, |_, _| false);
  }
  assert_eq!(powers.weapons.reflected, 1);
  assert!(powers.inventories[0].hit_ms > 0.0);
  assert_eq!(powers.inventories[1].hit_ms, 0.0);
}

#[test]
fn shield_fade_keeps_protection_and_blue_activation_stops_the_existing_turbo() {
  let (mut powers, racers) = state();
  powers.inventories[1].kind = 3;
  assert!(powers.use_power(1, &racers));
  assert!(powers.inventories[1].turbo.active());
  powers.inventories[1].kind = 2;
  assert!(powers.use_power(1, &racers));
  assert!(!powers.inventories[1].turbo.active());
  powers.advance(4.0, &racers, |_, _| false);
  assert_eq!(
    powers.inventories[1].shield.phase,
    lrsim::shield_effect::Phase::Fading
  );
  powers
    .weapons
    .impact(0, 1, Attack::Cannon, &mut powers.inventories, &powers.rules);
  assert_eq!(powers.inventories[1].hit_ms, 0.0);
  powers.advance(1.0, &racers, |_, _| false);
  assert!(!powers.inventories[1].shield.active());
  powers
    .weapons
    .impact(0, 1, Attack::Cannon, &mut powers.inventories, &powers.rules);
  assert!(powers.inventories[1].hit_ms > 0.0);
}
