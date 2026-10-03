//! Original nohat geometry is loaded, then viewed from each side on real Metal.
use crate::{
  custom_driver::Data,
  editor_gpu_test::capture_editor_frame,
  menu_ui::MenuUi,
  platform::{self, prelude::*},
};
use lrformats::library::Library;

pub fn verify_nohat(
  app: &mut bevy::prelude::App,
  library: &Library,
  ui: &MenuUi,
  output: &std::path::Path,
) {
  let data = Data::load(library).unwrap();
  assert_eq!(data.parts.hats[0].name, "nohat");
  let model = data.thumbnail(library, 0, 0).unwrap();
  assert_eq!(model.surfaces.len(), 1);
  assert_eq!(model.surfaces[0].material.as_deref(), Some("face"));
  assert_eq!(model.surfaces[0].triangles.len(), 1);
  assert_eq!(model.surfaces[0].texture, None);
  let points = model.surfaces[0].triangles[0].map(|i| {
    crate::gpu::world_position(model.mesh.vertices[i as usize].position, model.mesh.scale)
  });
  assert!(
    (points[1] - points[0]).cross(points[2] - points[0]).x < 0.0,
    "authored triangle must face away from original +X picker camera"
  );
  let lighting = crate::garage_lighting::driver(&lrsim::brick_build::Rules::load().unwrap());
  let thumbnail = crate::driver_thumbnail::load(&data, library, 0, 0, &lighting)
    .unwrap()
    .unwrap();
  assert_eq!(thumbnail.gpu.meshes.len(), 1);
  let mut counts = Vec::new();
  for (side, x) in [("front", 4.0), ("back", -4.0)] {
    platform::test_state(|s| s.batches.clear());
    clear_background(Color::from_rgba(0, 0, 55, 255));
    set_default_camera();
    ui.text(&format!("NO HAT: {side} camera"), 120.0, 35.0, 24.0, WHITE);
    ui.text(
      "Same original mesh / diagnostic asset preview",
      85.0,
      440.0,
      14.0,
      WHITE,
    );
    set_camera(&Camera3D {
      position: vec3(x, 0.0, 0.0),
      target: Vec3::ZERO,
      up: Vec3::Y,
      fovy: 45.0f32.to_radians(),
      z_near: 0.1,
      z_far: 10.0,
      ..Default::default()
    });
    thumbnail
      .gpu
      .draw_at(Mat4::from_translation(-thumbnail.center));
    set_default_camera();
    let image = capture_editor_frame(app);
    crate::capture::save_frame(
      &output.join(format!("driver-nohat-{side}-diagnostic.png")),
      &image,
    )
    .unwrap();
    let mut visible = 0;
    for y in 80..400 {
      for x in 100..540 {
        if crate::editor_gpu_test::logical_pixel(&image, vec2(x as f32, y as f32))
          != [0, 0, 55, 255]
        {
          visible += 1;
        }
      }
    }
    counts.push(visible);
  }
  assert_eq!(
    counts[0], 0,
    "front view must hide authored nohat via real GPU culling, not a mesh-name branch"
  );
  assert!(
    counts[1] > 1000,
    "reverse view must prove original nohat mesh actually reached GPU"
  );
  std::fs::write(
    output.join("driver-nohat.json"),
    serde_json::to_vec_pretty(&serde_json::json!({
    "mode":"scripted_offscreen_diagnostic_asset_preview","authored_triangle_loaded":true,
    "front_visible_pixels":counts[0],"back_visible_pixels":counts[1],
    "native_center":thumbnail.center.to_array(),"native_radius":thumbnail.radius,
    "original_backend_culling_parity":false}))
    .unwrap(),
  )
  .unwrap();
}
