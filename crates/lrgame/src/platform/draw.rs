use super::{
  state::{self, Batch},
  types::*,
};
use bevy::math::{Mat4, Vec2, Vec3, Vec4};
pub fn clear_background(color: Color) {
  state::with(|s| {
    s.background = color;
    s.dirty = true;
  });
}
pub fn set_camera(camera: &Camera3D) {
  state::with(|s| s.camera = Some(camera.clone()));
}
pub fn set_default_camera() {
  state::with(|s| s.camera = None);
}
pub fn gl_use_material(material: &Material) {
  state::with(|s| s.material = Some(*material));
}
pub fn gl_use_default_material() {
  state::with(|s| s.material = None);
}
pub fn push_model_matrix(matrix: Mat4) {
  state::with(|s| s.transforms.push(*s.transforms.last().unwrap() * matrix));
}
pub fn pop_model_matrix() {
  state::with(|s| {
    assert!(s.transforms.len() > 1);
    s.transforms.pop();
  });
}
pub fn draw_mesh(mesh: &Mesh) {
  state::with(|s| {
    let mut mesh = mesh.clone();
    let transform = *s.transforms.last().unwrap();
    if transform != Mat4::IDENTITY {
      for vertex in &mut mesh.vertices {
        vertex.position = transform_position(transform, vertex.position);
      }
    }
    let material = s.material.unwrap_or_else(|| {
      if s.camera.is_some() {
        Material::scene(None)
      } else {
        Material::overlay()
      }
    });
    // Bitmap fonts submit one quad per glyph and original bricks one mesh per
    // face. Merge adjacent overlays/opaque faces with identical state, preserving
    // triangle order. Translucent world submissions keep their separate sorting.
    if s.camera.is_none() || (material.blend.is_none() && material.depth_write) {
      if let Some(last) = s.batches.last_mut().filter(|last| {
        last.camera == s.camera
          && last.material == material
          && last.mesh.texture.as_ref().map(|t| t.id) == mesh.texture.as_ref().map(|t| t.id)
          && last.mesh.vertices.len() + mesh.vertices.len() <= u16::MAX as usize
      }) {
        let offset = last.mesh.vertices.len() as u16;
        last.mesh.vertices.extend(mesh.vertices);
        last
          .mesh
          .indices
          .extend(mesh.indices.into_iter().map(|index| index + offset));
        s.dirty = true;
        return;
      }
    }
    s.batches.push(Batch {
      mesh,
      camera: s.camera.clone(),
      material,
    });
    s.dirty = true;
  });
}
fn quad(
  rect: Rect,
  uv: Rect,
  color: Color,
  texture: Option<Texture2D>,
  rotation: f32,
  pivot: Option<Vec2>,
) {
  let mut points = [
    Vec2::new(rect.x, rect.y),
    Vec2::new(rect.x + rect.w, rect.y),
    Vec2::new(rect.x + rect.w, rect.y + rect.h),
    Vec2::new(rect.x, rect.y + rect.h),
  ];
  if rotation != 0.0 {
    let origin = pivot.unwrap_or(Vec2::new(rect.x + rect.w * 0.5, rect.y + rect.h * 0.5));
    let (sin, cos) = rotation.sin_cos();
    for p in &mut points {
      let q = *p - origin;
      *p = origin + Vec2::new(q.x * cos - q.y * sin, q.x * sin + q.y * cos);
    }
  }
  let coords = [
    Vec2::new(uv.x, uv.y),
    Vec2::new(uv.x + uv.w, uv.y),
    Vec2::new(uv.x + uv.w, uv.y + uv.h),
    Vec2::new(uv.x, uv.y + uv.h),
  ];
  draw_mesh(&Mesh {
    vertices: points
      .into_iter()
      .zip(coords)
      .map(|(p, uv)| Vertex {
        position: p.extend(0.0),
        uv,
        color: color.into(),
        normal: Vec4::ZERO,
      })
      .collect(),
    indices: vec![0, 1, 2, 0, 2, 3],
    texture,
  });
}
pub fn draw_rectangle(x: f32, y: f32, w: f32, h: f32, color: Color) {
  quad(
    Rect::new(x, y, w, h),
    Rect::new(0.0, 0.0, 1.0, 1.0),
    color,
    None,
    0.0,
    None,
  );
}
pub fn draw_circle(x: f32, y: f32, r: f32, color: Color) {
  let mut vertices = vec![Vertex::new(x, y, 0.0, 0.5, 0.5, color)];
  let mut indices = vec![];
  for i in 0..32 {
    let angle = i as f32 * std::f32::consts::TAU / 32.0;
    vertices.push(Vertex::new(
      x + r * angle.cos(),
      y + r * angle.sin(),
      0.0,
      0.0,
      0.0,
      color,
    ));
    indices.extend([0, i + 1, (i + 1) % 32 + 1]);
  }
  draw_mesh(&Mesh {
    vertices,
    indices,
    texture: None,
  });
}
pub fn draw_plane(center: Vec3, size: Vec2, texture: Option<&Texture2D>, color: Color) {
  let offsets = [
    Vec3::new(-size.x / 2.0, 0.0, -size.y / 2.0),
    Vec3::new(size.x / 2.0, 0.0, -size.y / 2.0),
    Vec3::new(size.x / 2.0, 0.0, size.y / 2.0),
    Vec3::new(-size.x / 2.0, 0.0, size.y / 2.0),
  ];
  let uv = [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y];
  draw_mesh(&Mesh {
    vertices: offsets
      .into_iter()
      .zip(uv)
      .map(|(p, uv)| Vertex {
        position: p + center,
        uv,
        color: color.into(),
        normal: Vec4::ZERO,
      })
      .collect(),
    indices: vec![0, 2, 1, 0, 3, 2],
    texture: texture.cloned(),
  });
}
pub fn draw_rectangle_lines(x: f32, y: f32, w: f32, h: f32, t: f32, color: Color) {
  for r in [
    Rect::new(x, y, w, t),
    Rect::new(x, y + h - t, w, t),
    Rect::new(x, y, t, h),
    Rect::new(x + w - t, y, t, h),
  ] {
    draw_rectangle(r.x, r.y, r.w, r.h, color);
  }
}
pub fn draw_texture_ex(
  texture: &Texture2D,
  x: f32,
  y: f32,
  color: Color,
  params: DrawTextureParams,
) {
  let source = params
    .source
    .unwrap_or(Rect::new(0.0, 0.0, texture.width(), texture.height()));
  let size = params.dest_size.unwrap_or(Vec2::new(source.w, source.h));
  let mut uv = Rect::new(
    source.x / texture.width(),
    source.y / texture.height(),
    source.w / texture.width(),
    source.h / texture.height(),
  );
  if params.flip_x {
    uv.x += uv.w;
    uv.w = -uv.w;
  }
  if params.flip_y {
    uv.y += uv.h;
    uv.h = -uv.h;
  }
  quad(
    Rect::new(x, y, size.x, size.y),
    uv,
    color,
    Some(texture.clone()),
    params.rotation,
    params.pivot,
  );
}
pub fn measure_text(text: &str, _font: Option<&()>, size: u16, scale: f32) -> TextDimensions {
  state::with(|s| {
    let width = text
      .chars()
      .map(|c| s.font.metrics(c, size as f32).advance_width)
      .sum::<f32>()
      * scale;
    TextDimensions {
      width,
      height: size as f32 * scale,
      offset_y: size as f32 * scale,
    }
  })
}
pub fn draw_text(text: &str, x: f32, y: f32, size: f32, color: Color) -> TextDimensions {
  let mut cursor = x;
  for c in text.chars() {
    let metrics = state::with(|s| s.font.metrics(c, size));
    if metrics.width > 0 && metrics.height > 0 {
      let key = (c, size.to_bits());
      let texture = state::with(|s| s.glyphs.get(&key).cloned()).unwrap_or_else(|| {
        let (metrics, mask) = state::with(|s| s.font.rasterize(c, size));
        let rgba = mask
          .into_iter()
          .flat_map(|a| [255, 255, 255, a])
          .collect::<Vec<_>>();
        let texture = Texture2D::from_rgba8(metrics.width as u16, metrics.height as u16, &rgba);
        state::with(|s| s.glyphs.insert(key, texture.clone()));
        texture
      });
      draw_texture_ex(
        &texture,
        cursor + metrics.xmin as f32,
        y - metrics.height as f32 - metrics.ymin as f32,
        color,
        DrawTextureParams::default(),
      );
    }
    cursor += metrics.advance_width;
  }
  TextDimensions {
    width: cursor - x,
    height: size,
    offset_y: size,
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn bevy_boundary_original_opaque_faces_merge_but_translucent_receivers_stay_separate() {
    state::with(|s| {
      s.batches.clear();
      s.transforms = vec![Mat4::IDENTITY];
      s.camera = None;
      s.material = None;
    });
    set_camera(&Camera3D::default());
    draw_rectangle(0.0, 0.0, 1.0, 1.0, RED);
    draw_rectangle(2.0, 0.0, 1.0, 1.0, BLUE);
    gl_use_material(&Material::scene(Some([1, 1])));
    draw_rectangle(4.0, 0.0, 1.0, 1.0, WHITE);
    draw_rectangle(6.0, 0.0, 1.0, 1.0, WHITE);
    state::with(|s| {
      assert_eq!(s.batches.len(), 3);
      assert_eq!(s.batches[0].mesh.vertices.len(), 8);
      assert_eq!(s.batches[0].mesh.vertices[4].color, <[u8; 4]>::from(BLUE));
      assert_eq!(s.batches[1].mesh.vertices.len(), 4);
      assert_eq!(s.batches[2].mesh.vertices.len(), 4);
    });
  }
  #[test]
  fn bevy_boundary_adjacent_font_quads_keep_ui_triangle_order_and_state_boundaries() {
    state::with(|s| {
      s.batches.clear();
      s.transforms = vec![Mat4::IDENTITY];
      s.camera = None;
      s.material = None;
    });
    let font = Texture2D::from_rgba8(1, 1, &[255, 255, 255, 255]);
    draw_texture_ex(&font, 10.0, 20.0, WHITE, DrawTextureParams::default());
    draw_texture_ex(&font, 30.0, 20.0, RED, DrawTextureParams::default());
    draw_rectangle(0.0, 0.0, 10.0, 10.0, BLACK);
    draw_texture_ex(&font, 50.0, 20.0, WHITE, DrawTextureParams::default());
    state::with(|s| {
      assert_eq!(s.batches.len(), 3);
      assert_eq!(
        s.batches[0].mesh.indices,
        vec![0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7]
      );
      assert_eq!(
        s.batches[0].mesh.vertices[4].position,
        Vec3::new(30.0, 20.0, 0.0)
      );
      assert_eq!(s.batches[0].mesh.vertices[4].color, <[u8; 4]>::from(RED));
      assert!(s.batches[1].mesh.texture.is_none());
    });
  }
  #[test]
  fn bevy_boundary_preserves_nested_original_model_transforms_and_uvs() {
    state::with(|s| {
      s.batches.clear();
      s.transforms = vec![Mat4::IDENTITY];
      s.camera = None;
      s.material = None;
    });
    let mesh = Mesh {
      vertices: vec![Vertex {
        position: Vec3::X,
        uv: Vec2::new(2.0, -3.0),
        color: [12, 34, 56, 78],
        normal: Vec4::ZERO,
      }],
      indices: vec![0, 0, 0],
      texture: None,
    };
    push_model_matrix(Mat4::from_translation(Vec3::new(10.0, 20.0, 30.0)));
    push_model_matrix(Mat4::from_scale(Vec3::splat(2.0)));
    draw_mesh(&mesh);
    pop_model_matrix();
    pop_model_matrix();
    state::with(|s| {
      let vertex = s.batches[0].mesh.vertices[0];
      assert_eq!(vertex.position, Vec3::new(12.0, 20.0, 30.0));
      assert_eq!(vertex.uv, Vec2::new(2.0, -3.0));
      assert_eq!(vertex.color, [12, 34, 56, 78]);
      assert_eq!(s.transforms, vec![Mat4::IDENTITY]);
    });
    assert_eq!(mesh.vertices[0].position, Vec3::X);
  }
  #[test]
  fn bevy_boundary_retains_both_split_screen_views_and_pixel_overlay() {
    state::with(|s| {
      s.batches.clear();
      s.camera = None;
      s.material = None;
    });
    let first = Camera3D {
      viewport: Some((0, 380, 1000, 380)),
      ..Default::default()
    };
    let second = Camera3D {
      viewport: Some((0, 0, 1000, 380)),
      ..Default::default()
    };
    set_camera(&first);
    draw_rectangle(1.0, 2.0, 3.0, 4.0, WHITE);
    set_camera(&second);
    draw_rectangle(5.0, 6.0, 7.0, 8.0, RED);
    set_default_camera();
    draw_rectangle(0.0, 0.0, 1000.0, 2.0, BLACK);
    state::with(|s| {
      assert_eq!(s.batches.len(), 3);
      assert_eq!(s.batches[0].camera, Some(first));
      assert_eq!(s.batches[1].camera, Some(second));
      assert_eq!(s.batches[2].camera, None);
      assert!(!s.batches[2].material.depth_write);
    });
  }
}
