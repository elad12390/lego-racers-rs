//! Original 00477210 / 0047fec0: gray ambient, white normalized (-1,-1,-1) key.
use lrformats::cinematic_lighting::Lighting;
use lrsim::brick_build::Rules;

pub fn from_rules(rules: &Rules) -> Lighting {
  Lighting {
    ambient: [(rules.preview_ambient * 255.0).round() as u8; 3],
    directional: vec![([255; 3], lrsim::contact::normalized(rules.preview_light))],
  }
}

/// CarBuilderAnimation::Load0047d460, distinct from the brick editor's light.
pub fn driver(rules: &Rules) -> Lighting {
  Lighting {
    ambient: [rules.driver_preview_ambient; 3],
    directional: vec![(
      [rules.driver_preview_key; 3],
      lrsim::contact::normalized(rules.preview_light),
    )],
  }
}

/// CameraManAnimation::Load0047b470 uses a dimmer key for the license photograph.
pub fn license(rules: &Rules) -> Lighting {
  Lighting {
    ambient: [rules.license_preview_ambient; 3],
    directional: vec![(
      [rules.license_preview_key; 3],
      lrsim::contact::normalized(rules.preview_light),
    )],
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn original_garage_light_preserves_ambient_floor_and_key_saturation() {
    let rules: Rules =
      serde_json::from_str(include_str!("../../../assets/native/builder.json")).unwrap();
    let light = from_rules(&rules);
    assert_eq!(light.ambient, [120; 3]);
    assert_eq!(light.shade([0.0, 0.0, 1.0]), [255; 3]);
    assert_eq!(light.shade([0.0, 0.0, -1.0]), [120; 3]);
    assert_eq!(light.shade([1.0, 0.0, 0.0]), [255; 3]);
    assert_eq!(light.shade([-1.0, 0.0, 0.0]), [120; 3]);
    let direction = light.directional[0].1;
    assert_eq!(light.shade(direction), [120; 3]);
    assert_eq!(light.shade(direction.map(|v| -v)), [255; 3]);
  }
  #[test]
  fn original_driver_and_license_screens_keep_their_distinct_light_levels() {
    let rules = Rules::load().unwrap();
    let driver = driver(&rules);
    let license = license(&rules);
    assert_eq!(driver.ambient, [128; 3]);
    assert_eq!(driver.directional[0].0, [255; 3]);
    assert_eq!(license.ambient, [120; 3]);
    assert_eq!(license.directional[0].0, [180; 3]);
    for light in [driver, license] {
      assert_eq!(light.shade(light.directional[0].1), light.ambient);
      let length = lrsim::contact::length(light.directional[0].1);
      assert!((length - 1.0).abs() < 1e-6);
    }
  }
  #[test]
  fn original_custom_bricks_keep_rotated_normals_and_unlit_source_colors() {
    let library = lrformats::library::Library::open(
      std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../extracted/Program_Files_Group/LEGO.JAM"),
    )
    .unwrap();
    let data = lrsim::brick_build::BuilderData::load(&library).unwrap();
    let build = data.default_build();
    let model = crate::brick_model::model(&library, &data, &build).unwrap();
    assert_eq!(model.mesh.normals.len(), model.mesh.vertices.len());
    assert!(model
      .mesh
      .normals
      .iter()
      .all(|n| n.iter().all(|v| v.is_finite())));
    for rotation in 0..4 {
      let part = lrsim::brick_build::PlacedBrick {
        name: "L300300".into(),
        color: "red".into(),
        x: 0,
        y: 0,
        z: 4,
        rotation,
      };
      let model = crate::brick_model::candidate(&library, &data, &build, &part).unwrap();
      assert_eq!(model.mesh.normals.len(), model.mesh.vertices.len());
      assert!(
        model
          .mesh
          .vertices
          .iter()
          .any(|vertex| vertex.rgba == [255; 4]),
        "untextured bricks must not retain provisional baked shading"
      );
      let polygon = &data.database.find(&part.name).unwrap().faces[0];
      // Stud cylinders precede the selected brick. Its first normal is at the
      // final brick's polygon section, so compare all expected normals there.
      let count: usize = data
        .database
        .find(&part.name)
        .unwrap()
        .faces
        .iter()
        .map(|f| f.points.len())
        .sum();
      let actual = model.mesh.normals[model.mesh.normals.len() - count];
      let [x, y, z] = polygon.normals[0];
      let expected = match rotation {
        0 => [x, y, z],
        1 => [y, -x, z],
        2 => [-x, -y, z],
        _ => [-y, x, z],
      };
      assert_eq!(actual, expected);
    }
  }
}
