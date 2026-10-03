use lrformats::{
  library::Library,
  world_animation::{self, Clip},
};
fn library() -> Library {
  Library::open(
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
      .join("../../extracted/Program_Files_Group/LEGO.JAM"),
  )
  .unwrap()
}

#[test]
fn royal_knights_world_binds_the_original_hammer_mesh_rig_and_loop() {
  let library = library();
  let objects = world_animation::parse(library.find_in("RRTRK.WDB", "RACEC0R0").unwrap()).unwrap();
  assert_eq!(objects.len(), 1);
  let hammer = &objects[0];
  assert_eq!(hammer.name, "rkhamm02");
  assert_eq!(hammer.placement.model, "rkhamm02");
  assert_eq!(hammer.skeleton, "rkhamm02");
  assert_eq!(hammer.animation, "rkhamm02");
  assert!(matches!(hammer.clip, Some(Clip::Index(0))));
  assert_eq!(hammer.range, 400.0);
  // Raw exporter direction lengths are preserved by the parser, but the
  // original model orientation setter normalizes them before rendering.
  let direction_length = hammer
    .placement
    .forward
    .iter()
    .map(|v| v * v)
    .sum::<f32>()
    .sqrt();
  assert!(direction_length > 3.0 && direction_length < 3.3);
  let animation =
    lrformats::animation::parse(library.find_in("RKHAMM02.ADB", "RACEC0R0").unwrap()).unwrap();
  assert!(animation.clips[0].loop_duration > 0);
  assert_eq!(animation.clips[0].frames_per_second, 30.0);
}

#[test]
fn shipped_animated_world_geometry_resolves_all_original_rigs_and_initial_clips() {
  let library = library();
  let mut total = 0;
  for table in library
    .jam()
    .tables
    .iter()
    .filter(|t| t.group.eq_ignore_ascii_case("GAMEDATA") && t.name.starts_with("RACEC"))
  {
    for entry in table
      .entries
      .iter()
      .filter(|e| e.name.to_ascii_uppercase().ends_with(".WDB"))
    {
      let bytes = library.jam().bytes(entry).unwrap();
      let objects = world_animation::parse(bytes)
        .unwrap_or_else(|e| panic!("{}/{}: {e}", table.name, entry.name));
      for object in objects {
        let read = |name: &str, ext| {
          library
            .find_in(&format!("{name}.{ext}"), &table.name)
            .or_else(|| library.find_in(&format!("{name}.{ext}"), "COMMON"))
            .unwrap_or_else(|| panic!("missing {}/{name}.{ext}", table.name))
        };
        lrformats::gdb::parse(read(&object.placement.model, "GDB")).unwrap();
        lrformats::tok::parse(read(&object.skeleton, "SDB")).unwrap();
        let animation = lrformats::animation::parse(read(&object.animation, "ADB"))
          .unwrap_or_else(|e| panic!("{}/{}.ADB: {e}", table.name, object.animation));
        match object.clip {
          Some(Clip::Index(index)) => assert!(index < animation.clips.len()),
          Some(Clip::Name(name)) => assert!(animation
            .clips
            .iter()
            .any(|c| c.name.eq_ignore_ascii_case(&name))),
          None => {}
        }
        total += 1;
      }
    }
  }
  println!("original drawable animated world objects: {total}");
  assert!(total > 0);
}

#[test]
fn original_lava_duplicate_rotation_time_is_a_right_continuous_step_not_invalid_data() {
  let library = library();
  let animation =
    lrformats::animation::parse(library.find_in("MMLAVBL.ADB", "RACEC0R3").unwrap()).unwrap();
  let channel = &animation.channels[0];
  let times = &animation.times[channel.time_start..channel.time_start + channel.rotation_count];
  let first = times.iter().position(|t| *t == 91).unwrap();
  assert_eq!(times[first + 1], 91);
  let last = times.iter().rposition(|t| *t == 91).unwrap();
  for looped in [false, true] {
    let (a, _, fraction) = animation
      .sample_rotation("a0", 0, 91.0, looped)
      .unwrap()
      .unwrap();
    assert_eq!(a, animation.rotations[channel.rotation_start + last]);
    assert_eq!(fraction, 0.0);
    let (a, b, fraction) = animation
      .sample_rotation("a0", 0, 90.99, looped)
      .unwrap()
      .unwrap();
    assert_eq!(a, animation.rotations[channel.rotation_start + first - 1]);
    assert_eq!(b, animation.rotations[channel.rotation_start + first]);
    assert!(fraction.is_finite() && fraction < 1.0);
  }
}
