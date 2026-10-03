//! A small CPU rasterizer used to check decoded models by eye. It is a verification tool,
//! not the game renderer.

use crate::bmp::Image;
use crate::model::{Model, Surface};

pub struct Camera {
  pub azimuth_deg: f32,
  pub elevation_deg: f32,
}

pub struct Frame {
  pub width: usize,
  pub height: usize,
  pub rgba: Vec<u8>,
}

impl Frame {
  pub fn dot(&mut self, x: f32, y: f32, radius: i32, rgb: [u8; 3]) {
    for dy in -radius..=radius {
      for dx in -radius..=radius {
        if dx * dx + dy * dy <= radius * radius {
          self.put(x as i32 + dx, y as i32 + dy, rgb);
        }
      }
    }
  }

  pub fn line(&mut self, from: [f32; 2], to: [f32; 2], rgb: [u8; 3]) {
    let steps = (to[0] - from[0])
      .abs()
      .max((to[1] - from[1]).abs())
      .ceil()
      .max(1.0) as i32;
    for k in 0..=steps {
      let t = k as f32 / steps as f32;
      self.put(
        (from[0] + (to[0] - from[0]) * t) as i32,
        (from[1] + (to[1] - from[1]) * t) as i32,
        rgb,
      );
    }
  }

  fn put(&mut self, x: i32, y: i32, rgb: [u8; 3]) {
    if x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height {
      let at = (y as usize * self.width + x as usize) * 4;
      self.rgba[at..at + 3].copy_from_slice(&rgb);
    }
  }
}

/// Maps model space to pixels for an orthographic camera fitted to a model's bounding box.
/// +Z is up; `azimuth` yaws the model, `elevation` is the camera's angle above the horizon.
pub struct Projector {
  scale: f32,
  azimuth: f32,
  elevation: f32,
  center: [f32; 2],
  fit: f32,
  size: f32,
}

impl Projector {
  pub fn fit(model: &Model, camera: &Camera, size: usize) -> Self {
    let mut projector = Projector {
      scale: model.mesh.scale,
      azimuth: camera.azimuth_deg.to_radians(),
      elevation: camera.elevation_deg.to_radians(),
      center: [0.0; 2],
      fit: 1.0,
      size: size as f32,
    };
    let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
    for v in &model.mesh.vertices {
      let p = projector.rotate(v.position);
      for k in 0..2 {
        lo[k] = lo[k].min(p[k]);
        hi[k] = hi[k].max(p[k]);
      }
    }
    let span = (hi[0] - lo[0]).max(hi[1] - lo[1]).max(1e-6);
    projector.fit = (size as f32 * 0.9) / span;
    projector.center = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
    projector
  }

  fn rotate(&self, p: [f32; 3]) -> [f32; 3] {
    let (az, el) = (self.azimuth, self.elevation);
    let (x, y, z) = (p[0] * self.scale, p[1] * self.scale, p[2] * self.scale);
    let (x1, y1) = (x * az.cos() - y * az.sin(), x * az.sin() + y * az.cos());
    // After yaw, +Y points away from the viewer. The camera sits above looking down by
    // `elevation`: `up` is the screen-up axis, `depth` the distance along the view direction.
    let up = y1 * el.sin() + z * el.cos();
    let depth = y1 * el.cos() - z * el.sin();
    [x1, up, depth]
  }

  /// Pixel position (x right, y down) and view depth of a model-space point.
  pub fn project(&self, p: [f32; 3]) -> [f32; 3] {
    let r = self.rotate(p);
    [
      (r[0] - self.center[0]) * self.fit + self.size / 2.0,
      self.size / 2.0 - (r[1] - self.center[1]) * self.fit,
      r[2],
    ]
  }
}

/// Renders with an orthographic camera looking at the model's bounding box.
pub fn render(model: &Model, camera: &Camera, size: usize) -> Frame {
  let projector = Projector::fit(model, camera, size);
  render_with(model, &projector, size)
}

pub fn render_with(model: &Model, projector: &Projector, size: usize) -> Frame {
  let projected: Vec<[f32; 3]> = model
    .mesh
    .vertices
    .iter()
    .map(|v| projector.project(v.position))
    .collect();
  let to_screen = |p: &[f32; 3]| *p;
  let mut frame = Frame {
    width: size,
    height: size,
    rgba: vec![0; size * size * 4],
  };
  for pixel in frame.rgba.chunks_exact_mut(4) {
    pixel.copy_from_slice(&[40, 40, 48, 255]);
  }
  let mut depth = vec![f32::MAX; size * size];
  for surface in &model.surfaces {
    let image = surface
      .texture
      .as_ref()
      .and_then(|t| model.images.get(&t.to_ascii_lowercase()));
    for tri in &surface.triangles {
      let idx = tri.map(|i| i as usize);
      let screen = idx.map(|i| to_screen(&projected[i]));
      let verts = idx.map(|i| &model.mesh.vertices[i]);
      raster(&mut frame, &mut depth, &screen, &verts, surface, image);
    }
  }
  frame
}

fn raster(
  frame: &mut Frame,
  depth: &mut [f32],
  s: &[[f32; 3]; 3],
  v: &[&crate::gdb::Vertex; 3],
  surface: &Surface,
  image: Option<&Image>,
) {
  let area = (s[1][0] - s[0][0]) * (s[2][1] - s[0][1]) - (s[2][0] - s[0][0]) * (s[1][1] - s[0][1]);
  if area.abs() < 1e-6 {
    return;
  }
  let min_x = s
    .iter()
    .map(|p| p[0])
    .fold(f32::MAX, f32::min)
    .floor()
    .max(0.0) as usize;
  let max_x =
    (s.iter().map(|p| p[0]).fold(f32::MIN, f32::max).ceil() as usize).min(frame.width - 1);
  let min_y = s
    .iter()
    .map(|p| p[1])
    .fold(f32::MAX, f32::min)
    .floor()
    .max(0.0) as usize;
  let max_y =
    (s.iter().map(|p| p[1]).fold(f32::MIN, f32::max).ceil() as usize).min(frame.height - 1);
  for y in min_y..=max_y {
    for x in min_x..=max_x {
      let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
      let w0 = ((s[1][0] - px) * (s[2][1] - py) - (s[2][0] - px) * (s[1][1] - py)) / area;
      let w1 = ((s[2][0] - px) * (s[0][1] - py) - (s[0][0] - px) * (s[2][1] - py)) / area;
      let w2 = 1.0 - w0 - w1;
      if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
        continue;
      }
      let z = w0 * s[0][2] + w1 * s[1][2] + w2 * s[2][2];
      let at = y * frame.width + x;
      if z >= depth[at] {
        continue;
      }
      let u = w0 * v[0].uv[0] + w1 * v[1].uv[0] + w2 * v[2].uv[0];
      let t = w0 * v[0].uv[1] + w1 * v[1].uv[1] + w2 * v[2].uv[1];
      let tint = |k: usize| {
        w0 * f32::from(v[0].rgba[k]) + w1 * f32::from(v[1].rgba[k]) + w2 * f32::from(v[2].rgba[k])
      };
      let base = match image {
        Some(img) => sample(img, u, t),
        None => [surface.color[0], surface.color[1], surface.color[2]],
      };
      let out = &mut frame.rgba[at * 4..at * 4 + 4];
      for k in 0..3 {
        out[k] = (f32::from(base[k]) * tint(k) / 255.0).clamp(0.0, 255.0) as u8;
      }
      depth[at] = z;
    }
  }
}

fn sample(image: &Image, u: f32, v: f32) -> [u8; 3] {
  let wrap = |t: f32, n: u16| ((t.rem_euclid(1.0)) * f32::from(n)) as usize % usize::from(n.max(1));
  let (x, y) = (wrap(u, image.width), wrap(v, image.height));
  image.rgb(y * usize::from(image.width) + x)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::gdb::{Mesh, Part, Vertex};
  use std::collections::HashMap;

  fn vertex(x: f32, y: f32) -> Vertex {
    Vertex {
      position: [x, 0.0, y],
      uv: [0.0, 0.0],
      rgba: [255; 4],
    }
  }

  #[test]
  fn a_flat_triangle_facing_the_camera_fills_pixels_with_its_color() {
    let mesh = Mesh {
      normals: vec![],
      textures: vec![],
      scale: 1.0,
      vertices: vec![vertex(0.0, 0.0), vertex(1.0, 0.0), vertex(0.0, 1.0)],
      triangles: vec![[0, 1, 2]],
      parts: vec![Part {
        texture: 0,
        joint: 0,
        flag: 0,
        vertices: 0..3,
        triangles: 0..1,
      }],
    };
    let model = Model {
      mesh,
      surfaces: vec![Surface {
        material: None,
        color: [200, 0, 0, 255],
        texture: None,
        triangles: vec![[0, 1, 2]],
        joints: vec![[0; 3]],
        color_key: None,
        blend: None,
      }],
      images: HashMap::new(),
    };
    let frame = render(
      &model,
      &Camera {
        azimuth_deg: 0.0,
        elevation_deg: 0.0,
      },
      64,
    );
    let red = frame
      .rgba
      .chunks_exact(4)
      .filter(|p| p[..3] == [200, 0, 0])
      .count();
    assert!(
      red > 400,
      "expected a filled triangle, got {red} red pixels"
    );
  }
}

#[cfg(test)]
mod camera_tests {
  use super::*;
  use crate::gdb::{Mesh, Part, Vertex};
  use std::collections::HashMap;

  fn tri(size: f32, z: f32, rgb: u8) -> (Vec<Vertex>, Surface) {
    let v = |x: f32, y: f32| Vertex {
      position: [x, y, z],
      uv: [0.0; 2],
      rgba: [255; 4],
    };
    (
      vec![v(0.0, 0.0), v(size, 0.0), v(0.0, size)],
      Surface {
        material: None,
        color: [rgb, rgb, rgb, 255],
        texture: None,
        triangles: vec![],
        joints: vec![],
        color_key: None,
        blend: None,
      },
    )
  }

  #[test]
  fn top_down_view_shows_the_higher_surface_over_the_lower_one() {
    // A big low triangle and a small high one that sits inside it.
    let (mut vertices, mut low) = tri(12.0, 0.0, 50);
    let (upper_vertices, mut high) = tri(4.0, 5.0, 200);
    vertices.extend(upper_vertices);
    low.triangles = vec![[0, 1, 2]];
    high.triangles = vec![[3, 4, 5]];
    let mesh = Mesh {
      normals: vec![],
      textures: vec![],
      scale: 1.0,
      vertices,
      triangles: vec![[0, 1, 2], [3, 4, 5]],
      parts: vec![Part {
        texture: 0,
        joint: 0,
        flag: 0,
        vertices: 0..6,
        triangles: 0..2,
      }],
    };
    // Draw the high one first so draw order cannot be what makes it win.
    let model = Model {
      mesh,
      surfaces: vec![high, low],
      images: HashMap::new(),
    };
    let frame = render(
      &model,
      &Camera {
        azimuth_deg: 0.0,
        elevation_deg: 90.0,
      },
      64,
    );
    // Model point (1, 1) lands at pixel (8, 56): x grows right, +Y goes up the screen.
    let pixel = |x: usize, y: usize| frame.rgba[(y * 64 + x) * 4];
    assert_eq!(
      pixel(8, 56),
      200,
      "the high triangle should cover the low one"
    );
    assert_eq!(
      pixel(30, 40),
      50,
      "the low triangle shows where nothing covers it"
    );
  }
}
