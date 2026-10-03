//! Actual original-parser outputs rendered through the current native backend.
//! Allocation-free source prefixes only, not original raster/full-loader parity.
use crate::{
  custom_driver::Data,
  editor_gpu_test::{capture_editor_frame, logical_pixel},
  gpu::TrackGpu,
  menu_ui::MenuUi,
  platform::{self, prelude::*},
};
use lrformats::{gdb, library::Library, model::Model};
use serde_json::Value;
use std::{path::Path, time::Duration};

fn report(root: &Path, directory: &str) -> Value {
  serde_json::from_slice(&std::fs::read(root.join(directory).join("execution.json")).unwrap())
    .unwrap()
}

fn values<T: serde::de::DeserializeOwned>(report: &Value, key: &str) -> T {
  serde_json::from_value(report.get(key).unwrap().clone()).unwrap()
}

fn original_mesh(
  native: &Model,
  positions: &[[f32; 3]],
  uv: &[[f32; 2]],
  normals: &[[f32; 3]],
  scale: f32,
  triangles: &[[u32; 3]],
) -> Model {
  let mut mesh = native.mesh.clone();
  mesh.vertices = positions
    .iter()
    .zip(uv)
    .map(|(position, uv)| gdb::Vertex {
      position: position.map(|v| v * scale / mesh.scale),
      uv: *uv,
      rgba: [255; 4],
    })
    .collect();
  mesh.normals = normals.to_vec();
  let mut surfaces = native.surfaces.clone();
  assert_eq!(surfaces.len(), 1);
  surfaces[0].triangles = triangles.to_vec();
  Model {
    mesh,
    surfaces,
    images: native.images.clone(),
  }
}

fn draw(ui: &MenuUi, gpu: &TrackGpu, center: Vec3, angle: f32) {
  platform::test_state(|s| s.batches.clear());
  clear_background(Color::from_rgba(0, 0, 55, 255));
  set_default_camera();
  ui.text("HEADS: ORIGINAL PARSER OUTPUT", 30.0, 28.0, 24.0, WHITE);
  ui.text(
    "Current native Metal / diagnostic asset preview",
    35.0,
    452.0,
    18.0,
    WHITE,
  );
  set_camera(&Camera3D {
    position: vec3(3.4, 0.0, 0.0),
    target: Vec3::ZERO,
    up: Vec3::Y,
    fovy: 45.0f32.to_radians(),
    z_near: 0.1,
    z_far: 20.0,
    ..Default::default()
  });
  gpu.draw_at(Mat4::from_rotation_y(angle) * Mat4::from_translation(-center));
  set_default_camera();
}

#[test]
#[ignore = "requires original JAM, verified original-parser reports and real Metal; scripted/offscreen asset diagnostic"]
fn bevy_heads_original_parser_geometry_native_render_agreement() {
  let root =
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../progress/artifacts/solo-race");
  let output = std::path::PathBuf::from(std::env::var("LR_TEST_OUTPUT").unwrap());
  std::fs::create_dir(&output).expect("new isolated native evidence directory required");
  let composition = report(&root, "driver-heads-parser-composition-20261003");
  assert_eq!(
    composition["status"],
    "verified_original_parser_output_finalization"
  );
  assert_eq!(composition["full_loader_success"], false);
  assert_eq!(composition["synthetic_terminator_added"], false);
  let positions: Vec<[f32; 3]> = values(&composition, "original_positions");
  let uv: Vec<[f32; 2]> = values(&composition, "original_uv");
  let normals: Vec<[f32; 3]> = values(&composition, "original_normals");
  let colors: Vec<u32> = values(&composition, "original_colors");
  let triangles: Vec<[u8; 3]> = values(&composition, "original_triangles");
  let words: Vec<u32> = values(&composition, "original_words");
  let scale: f32 = values(&composition, "original_scale");
  let original_center: [f32; 3] = values(&composition, "original_center");
  let original_radius: f32 = values(&composition, "original_radius");
  assert_eq!(positions.len(), 59);
  assert_eq!(uv.len(), 59);
  assert_eq!(normals.len(), 59);
  assert_eq!(colors, vec![u32::MAX; 59]);
  assert_eq!(triangles.len(), 58);
  assert_eq!(words, [0x8000_0000, 0x003a_0000, 0x203a_0000]);
  let library = Library::open(std::env::var("LR_TEST_JAM").unwrap()).unwrap();
  let raw = library
    .find_at("HEADS.GDB", "MENUDATA", "MENUPART")
    .unwrap();
  let decoded = gdb::parse(raw).unwrap();
  assert_eq!(
    raw,
    std::fs::read(root.join("../../../tmp/license-rig-inspection/HEADS.GDB")).unwrap()
  );
  assert_eq!(decoded.scale, scale);
  assert_eq!(
    decoded
      .vertices
      .iter()
      .map(|v| v.position)
      .collect::<Vec<_>>(),
    positions
  );
  assert_eq!(
    decoded.vertices.iter().map(|v| v.uv).collect::<Vec<_>>(),
    uv
  );
  assert_eq!(decoded.normals, normals);
  assert_eq!(decoded.triangles, triangles);
  assert_eq!(decoded.parts.len(), 1);
  let part = &decoded.parts[0];
  assert_eq!(
    (
      part.texture,
      part.flag,
      part.vertices.clone(),
      part.triangles.clone()
    ),
    (0, 0, 0..59, 0..58)
  );
  let resolved = triangles
    .iter()
    .map(|t| t.map(u32::from))
    .collect::<Vec<_>>();
  assert_eq!(decoded.resolved_triangles().unwrap(), resolved);
  let data = Data::load(&library).unwrap();
  let native = data.thumbnail(&library, 1, 0).unwrap();
  let original = original_mesh(&native, &positions, &uv, &normals, scale, &resolved);
  assert_eq!(original.mesh.vertices, native.mesh.vertices);
  assert_eq!(original.mesh.normals, native.mesh.normals);
  assert_eq!(original.surfaces[0].triangles, native.surfaces[0].triangles);
  let (center, radius) = crate::driver_thumbnail::sphere(&original).unwrap();
  let original_native_center = crate::gpu::world_position(original_center, 1.0);
  for (actual, expected) in center
    .to_array()
    .into_iter()
    .zip(original_native_center.to_array())
  {
    assert!(
      (actual - expected).abs() < 1e-6,
      "original/native center differs"
    );
  }
  assert!(
    (radius - original_radius).abs() < 1e-6,
    "original/native radius differs"
  );
  assert_eq!(
    crate::driver_thumbnail::sphere(&native).unwrap(),
    (center, radius)
  );
  let mut app = platform::offscreen_test_app();
  app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
    Duration::from_secs_f64(1.0 / 60.0),
  ));
  app
    .world_mut()
    .spawn((bevy::window::Window::default(), bevy::window::PrimaryWindow));
  app.update();
  platform::sync(app.world_mut());
  let ui = MenuUi::load(&library).unwrap();
  let lighting = crate::garage_lighting::driver(&lrsim::brick_build::Rules::load().unwrap());
  let mut source_gpu = TrackGpu::upload(&original).unwrap();
  let mut native_gpu = TrackGpu::upload(&native).unwrap();
  source_gpu.set_scene_lighting(&lighting, Mat4::IDENTITY);
  native_gpu.set_scene_lighting(&lighting, Mat4::IDENTITY);
  let mut compared_pixels = 0;
  for (name, angle) in [("front", 0.0), ("oblique", 0.55)] {
    draw(&ui, &source_gpu, center, angle);
    let source_image = capture_editor_frame(&mut app);
    draw(&ui, &native_gpu, center, angle);
    let native_image = capture_editor_frame(&mut app);
    let mut visible = 0;
    for y in 70..420 {
      for x in 100..540 {
        let point = vec2(x as f32, y as f32);
        let actual = logical_pixel(&source_image, point);
        assert_eq!(
          actual,
          logical_pixel(&native_image, point),
          "parser/native pixels differ at {x},{y}"
        );
        visible += usize::from(actual != [0, 0, 55, 255]);
        compared_pixels += 1;
      }
    }
    assert!(
      visible > 1000,
      "parsed HEADS must produce visible native geometry"
    );
    crate::capture::save_frame(
      &output.join(format!("heads-parsed-{name}.png")),
      &source_image,
    )
    .unwrap();
  }
  std::fs::write(
    output.join("heads-native-parser-agreement.json"),
    serde_json::to_vec_pretty(&serde_json::json!({
    "mode":"scripted_offscreen_diagnostic_asset_preview",
      "vertices":positions.len(), "triangles":triangles.len(), "original_words":words,
      "all_positions_uv_normals_colors_and_triangle_indices_match":true,
      "native_source_output_viewport_pixels_equal":compared_pixels,
      "original_loader_allocation_or_raster_parity":false,
    "native_center":center.to_array(), "native_radius":radius,
    "original_center":original_center, "original_radius":original_radius,
    "original_center_in_native_coordinates":original_native_center.to_array(),
    "original_scale":scale, "synthetic_terminator_added":false,
      "materials":"native authored face style reused; original material loader not proved"
    }))
    .unwrap(),
  )
  .unwrap();
}
