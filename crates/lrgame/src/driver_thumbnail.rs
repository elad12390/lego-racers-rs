//! Source-backed selector projection/fitting over native loaded thumbnail meshes.
//! Full original template loading/raster parity remain separate from this math.
use crate::{gpu::TrackGpu, menu_ui::MenuUi, platform::prelude::*};
use lrformats::{menu_selector::Selector, model::Model};

pub struct Thumbnail {
  pub gpu: TrackGpu,
  pub translation: Vec3,
  pub center: Vec3,
  pub radius: f32,
}

pub fn load(
  data: &crate::custom_driver::Data,
  library: &lrformats::library::Library,
  row: usize,
  index: usize,
  lighting: &lrformats::cinematic_lighting::Lighting,
) -> Result<Option<Thumbnail>, String> {
  let model = data.thumbnail(library, row, index)?;
  let Some((center, radius)) = sphere(&model) else {
    return Ok(None);
  };
  let mut gpu = TrackGpu::upload(&model)?;
  gpu.set_scene_lighting(lighting, Mat4::IDENTITY);
  Ok(Some(Thumbnail {
    gpu,
    center,
    radius,
    translation: Vec3::ZERO,
  }))
}

pub fn row(
  data: &crate::custom_driver::Data,
  library: &lrformats::library::Library,
  ui: &MenuUi,
  choices: &[usize],
  row: usize,
  index: usize,
  lighting: &lrformats::cinematic_lighting::Lighting,
) -> Result<crate::driver_scroll::Row, String> {
  let position = choices
    .iter()
    .position(|i| *i == index)
    .ok_or("driver row selection unavailable")?;
  let (selector, viewport) = ui.driver_selector(row)?;
  let items = (0..selector.slots.len())
    .map(|slot| {
      let offset = slot as isize - selector.selected_slot as isize;
      let index = choices[(position as isize + offset).rem_euclid(choices.len() as isize) as usize];
      load(data, library, row, index, lighting)
    })
    .collect::<Result<Vec<_>, _>>()?;
  Ok(crate::driver_scroll::Row::new(
    items,
    ui.driver_scroll_ms,
    selector,
    viewport,
  ))
}

/// GolDP100278c0: midpoint of referenced vertex extrema, maximum distance to it.
/// No diagonal-box radius or minimum-size clamp; nohat has a back-facing mesh.
pub fn sphere(model: &Model) -> Option<(Vec3, f32)> {
  let points = model
    .surfaces
    .iter()
    .flat_map(|s| &s.triangles)
    .flatten()
    .map(|i| {
      crate::gpu::world_position(model.mesh.vertices[*i as usize].position, model.mesh.scale)
    })
    .collect::<Vec<_>>();
  let first = *points.first()?;
  let (low, high) = points
    .iter()
    .fold((first, first), |(low, high), p| (low.min(*p), high.max(*p)));
  let center = (low + high) * 0.5;
  let radius = points
    .iter()
    .map(|p| p.distance_squared(center))
    .fold(0.0f32, f32::max)
    .sqrt();
  Some((center, radius))
}

/// Layout_Thumbs0046cf20 and ComputeScroll_Wheel0046d040 (real two-arg ABI).
pub fn fit(selector: &Selector, viewport: Rect, slot: usize, center: Vec3, radius: f32) -> Vec3 {
  let r = selector.slots[slot];
  let half_height = (selector.fov_degrees.to_radians() * 0.5).tan() * selector.near;
  let half_width = half_height * viewport.w / viewport.h;
  let sx = half_width * 2.0 / viewport.w;
  let sy = half_height * 2.0 / viewport.h;
  // Original integer width/2 is truncated, notably the201px headbox viewport.
  let left = (r[0] - (viewport.w as i32 / 2)) as f32 * sx;
  let right = (r[2] - (viewport.w as i32 / 2)) as f32 * sx;
  let diameter = (right - left).min((r[3] - r[1]) as f32 * sy);
  let model_diameter = radius * 2.0;
  let candidate = if model_diameter - diameter > 0.0 {
    -(selector.near / diameter) * (model_diameter - diameter)
  } else {
    model_diameter - diameter
  };
  let depth = candidate.min(-(model_diameter + 2.0));
  // Original X is depth, Y is horizontal, Z vertical; convert only once.
  let horizontal = (selector.near - depth) / selector.near * ((left + right) * 0.5);
  crate::gpu::world_position([depth, horizontal, 0.0], 1.0) - center
}

pub fn draw<'a>(
  selector: &Selector,
  viewport: Rect,
  thumbnails: impl Iterator<Item = &'a Thumbnail>,
) {
  // Original camera source[near,0,0], target[0,0,0], vertical[0,0,1].
  let scale = MenuUi::scale();
  let origin = MenuUi::origin();
  set_camera(&Camera3D {
    position: vec3(selector.near, 0.0, 0.0),
    target: Vec3::ZERO,
    up: Vec3::Y,
    aspect: Some(viewport.w / viewport.h),
    fovy: selector.fov_degrees.to_radians(),
    z_near: selector.near,
    z_far: selector.far,
    viewport: Some((
      (origin.x + viewport.x * scale) as i32,
      (screen_height() - origin.y - (viewport.y + viewport.h) * scale) as i32,
      (viewport.w * scale) as i32,
      (viewport.h * scale) as i32,
    )),
    ..Default::default()
  });
  for thumbnail in thumbnails {
    thumbnail
      .gpu
      .draw_at(Mat4::from_translation(thumbnail.translation));
  }
  set_default_camera();
}

#[cfg(test)]
pub fn export_geometry(
  library: &lrformats::library::Library,
  ui: &MenuUi,
  output: &std::path::Path,
) {
  let data = crate::custom_driver::Data::load(library).unwrap();
  let mut cases = Vec::new();
  for row in 0..4 {
    let (selector, viewport) = ui.driver_selector(row).unwrap();
    for index in 0..data.row(row).len() {
      let model = data.thumbnail(library, row, index).unwrap();
      if let Some((center, radius)) = sphere(&model) {
        let source_points = model
          .surfaces
          .iter()
          .flat_map(|s| &s.triangles)
          .flatten()
          .map(|i| {
            model.mesh.vertices[*i as usize]
              .position
              .map(|v| v * model.mesh.scale)
          })
          .collect::<Vec<_>>();
        let source_center = [center.x, -center.z, center.y];
        let fits = (0..selector.slots.len())
          .map(|slot| {
            let p = fit(selector, viewport, slot, center, radius);
            [p.x, -p.z, p.y]
          })
          .collect::<Vec<_>>();
        cases.push(
          serde_json::json!({"row":row,"index":index,"name":data.row(row)[index].name,
          "points":source_points,"center":source_center,"radius":radius,"fits":fits,
          "width":viewport.w,"height":viewport.h,"slots":selector.slots,
          "fov":selector.fov_degrees,"near":selector.near,"far":selector.far}),
        );
      } else {
        panic!("original driver thumbnail has no referenced geometry: row{row} index{index}");
      }
    }
  }
  std::fs::write(output, serde_json::to_vec(&cases).unwrap()).unwrap();
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn original_driver_selector_slots_camera_and_synthetic_sphere_fit() {
    let library = lrformats::library::Library::open(
      std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../extracted/Program_Files_Group/LEGO.JAM"),
    )
    .unwrap();
    let layout =
      lrformats::menu_layout::Layout::parse(library.find_in("EDITDRVR.MIB", "MENUDATA").unwrap())
        .unwrap();
    for (name, width) in [("partbox", 200.0), ("headbox", 201.0)] {
      let widget = &layout.widgets[name];
      let s = widget.selector.as_ref().unwrap();
      assert_eq!(s.selected_slot, 2);
      assert_eq!(s.option, 0.0);
      assert_eq!((s.fov_degrees, s.near, s.far), (4.0, 2.0, 275.0));
      for slot in 0..5 {
        let p = fit(s, Rect::new(0.0, 0.0, width, 64.0), slot, Vec3::ZERO, 0.5);
        if name == "partbox" {
          let depth = [-24.181713, -14.661092, -10.727222, -14.661092, -24.181713][slot];
          assert!((p.x - depth).abs() < 0.0001, "{name} slot{slot}: {p:?}");
        }
        assert_eq!(p.y, 0.0);
      }
    }
  }
}
