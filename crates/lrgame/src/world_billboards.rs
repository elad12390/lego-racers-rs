//! Depth-tested original WDB billboards; cylindrical when an axis is declared.
use crate::platform::prelude::*;
use crate::race_view::RaceView;
use lrformats::{library::Library, scene::SceneBindings, world_billboards::Billboard};
use std::collections::HashMap;

struct Sprite {
  source: Billboard,
  texture: Texture2D,
  color: Color,
  pipeline: Material,
}
pub struct Billboards {
  sprites: Vec<Sprite>,
}
impl Billboards {
  pub fn load(
    library: &Library,
    table: &str,
    bytes: &[u8],
    bindings: &SceneBindings,
  ) -> Result<Self, String> {
    let mut textures = HashMap::new();
    let mut sprites = Vec::new();
    for source in lrformats::world_billboards::parse(bytes)? {
      let material = bindings.sprite_material(&source.material)?;
      let name = material
        .texture
        .as_ref()
        .ok_or("original billboard material has no texture")?;
      let key = name.to_ascii_lowercase();
      let texture = if let Some(texture) = textures.get(&key) {
        Texture2D::clone(texture)
      } else {
        let definition = bindings
          .textures
          .get(&key)
          .ok_or_else(|| format!("{table}: sprite texture {name} absent from declared TDB"))?;
        let file = format!("{name}.{}", if definition.targa { "TGA" } else { "BMP" });
        let bytes = library
          .find_in(&file, table)
          .or_else(|| library.find_in(&file, "COMMON"))
          .ok_or_else(|| format!("missing original sprite {table}/{file}"))?;
        let image = if definition.targa {
          lrformats::tga::decode(bytes)?
        } else {
          lrformats::bmp::decode(bytes).map_err(|e| e.to_string())?
        };
        let mut rgba = image.to_rgba();
        if let Some(key) = definition.color_key {
          for pixel in rgba.chunks_exact_mut(4) {
            if pixel[..3] == key {
              pixel[3] = 0;
            }
          }
        }
        let texture = Texture2D::from_rgba8(image.width, image.height, &rgba);
        texture.set_filter(FilterMode::Nearest);
        textures.insert(key, texture.clone());
        texture
      };
      let color = material.base_color();
      sprites.push(Sprite {
        source,
        texture,
        color: Color::from_rgba(color[0], color[1], color[2], color[3]),
        pipeline: crate::scene_material::load_blend(material.blend)?,
      });
    }
    Ok(Self { sprites })
  }
  pub fn len(&self) -> usize {
    self.sprites.len()
  }
  pub fn first(&self) -> Option<&Billboard> {
    self.sprites.first().map(|s| &s.source)
  }
  pub fn draw(&self, eye: [f32; 3], camera_up: [f32; 3], view: RaceView) {
    let eye = view.native(eye);
    let camera_up = view.native(camera_up).normalize_or_zero();
    for sprite in &self.sprites {
      let center = view.native(sprite.source.position);
      let to_eye = eye - center;
      if sprite.source.range >= 0.0 && to_eye.length() > sprite.source.range {
        continue;
      }
      let up = sprite
        .source
        .axis
        .map_or(camera_up, |axis| view.native(axis).normalize_or_zero());
      let mut right = up.cross(to_eye).normalize_or_zero();
      if right.length_squared() == 0.0 {
        right = up.cross(Vec3::X).normalize_or_zero();
        if right.length_squared() == 0.0 {
          right = up.cross(Vec3::Z).normalize_or_zero();
        }
      }
      let right = right * (sprite.source.width / 2.0);
      let vertical = up * (sprite.source.height / 2.0);
      let points = [
        center - right - vertical,
        center + right - vertical,
        center + right + vertical,
        center - right + vertical,
      ];
      gl_use_material(&sprite.pipeline);
      let vertices = points
        .into_iter()
        .zip([[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]])
        .map(|(point, uv)| Vertex::new(point.x, point.y, point.z, uv[0], uv[1], sprite.color))
        .collect();
      draw_mesh(&Mesh {
        vertices,
        indices: vec![0, 1, 2, 0, 2, 3],
        texture: Some(sprite.texture.clone()),
      });
    }
    gl_use_default_material();
  }
}
