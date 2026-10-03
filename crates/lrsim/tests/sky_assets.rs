use lrformats::{library::Library, sky};
use lrsim::sky_state::Player;
use std::path::PathBuf;

fn library() -> Library {
  Library::open(
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../extracted/Program_Files_Group/LEGO.JAM"),
  )
  .unwrap()
}

#[test]
fn original_sky_sequences_transition_by_name_and_restart_without_mutating_assets() {
  let library = library();
  let sky = sky::parse(library.find_in("BACKGRND.SKB", "RACEC0R0").unwrap()).unwrap();
  assert_eq!(sky.profiles[0].frames[0].duration_ms, 5000);
  assert_eq!(sky.profiles[1].frames[0].duration_ms, 1000);
  let mut player = Player::new(sky);
  let initial = player.colors();
  player.select("castle", 1000).unwrap();
  assert_eq!(player.colors(), initial);
  player.advance(500);
  assert_eq!(
    player.colors(),
    [[125, 125, 150], [50, 50, 147], [12, 12, 127]]
  );
  player.advance(500);
  assert_eq!(player.colors(), [[0, 0, 180], [0, 0, 40], [0, 0, 0]]);
  player.advance(1);
  assert_eq!(player.colors(), [[0, 0, 180], [0, 0, 40], [0, 0, 0]]);
  assert!(player.select("missing-original-profile", 0).is_err());
  player.reset();
  assert_eq!(player.colors(), initial);
}

#[test]
fn every_shipped_sky_preserves_all_frames_and_survives_long_ticks() {
  let library = library();
  let mut files = 0;
  let mut animated = 0;
  for table in &library.jam().tables {
    if !table.group.eq_ignore_ascii_case("GAMEDATA") {
      continue;
    }
    for entry in &table.entries {
      if !entry.name.to_ascii_uppercase().ends_with(".SKB") {
        continue;
      }
      let sky = sky::parse(library.jam().bytes(entry).unwrap()).unwrap();
      let mut player = Player::new(sky);
      for profile in 0..player.sky.profiles.len() {
        let name = player.sky.profiles[profile].name.clone();
        let frames = player.sky.profiles[profile].frames.clone();
        assert_eq!(player.sky.profiles[profile].colors, frames[0].colors);
        if frames.len() == 1 {
          player.select(&name, 0).unwrap();
          player.advance(100_000);
          assert_eq!(player.colors(), frames[0].colors);
          continue;
        }
        animated += 1;
        player.reset();
        player.select(&name, 0).unwrap();
        player.advance(frames[0].duration_ms);
        // Exact boundary still interpolates toward next; the cursor does not
        // advance yet. The following large tick discards all excess elapsed.
        assert_eq!(player.colors(), frames[1].colors, "{}/{name}", table.name);
        player.advance(100_000);
        assert_eq!(player.colors(), frames[1].colors, "{}/{name}", table.name);
        player.advance(frames[1].duration_ms + 1);
        assert_eq!(player.colors(), frames[2 % frames.len()].colors);
      }
      files += 1;
    }
  }
  // Thirteen playable worlds plus the original diagnostic race table.
  assert_eq!(files, 14);
  // No timed multi-frame sequences are authored in this PC archive.
  assert_eq!(animated, 0);
}

#[test]
fn constructed_multiframe_sequence_uses_strict_boundaries_and_discards_excess() {
  let library = library();
  let mut sky = sky::parse(library.find_in("BACKGRND.SKB", "RACEC0R0").unwrap()).unwrap();
  let next = sky.profiles[1].frames[0].clone();
  sky.profiles[0].frames.push(next.clone());
  let mut player = Player::new(sky);
  player.advance(5000);
  assert_eq!(player.colors(), next.colors);
  player.advance(100_000);
  assert_eq!(player.colors(), next.colors);
  player.advance(1000);
  assert_eq!(player.colors(), player.sky.profiles[0].frames[0].colors);
  player.advance(1);
  assert_eq!(player.colors(), player.sky.profiles[0].frames[0].colors);
}
