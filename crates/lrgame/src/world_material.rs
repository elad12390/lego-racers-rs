//! Preloaded race-world material keys, assigned only to declared mesh slots.
use crate::cinematic_material::Surface;
use crate::platform::prelude::*;
use lrformats::{
  library::Library,
  material_animation::Animation,
  model::Model,
  scene::SceneBindings,
  world_material::{self, Reference},
};
use std::collections::HashMap;

struct Track {
  animation: Animation,
  channel: usize,
  target: String,
  last: Option<String>,
  surfaces: HashMap<String, Surface>,
}
pub struct Player {
  tracks: Vec<Track>,
}
impl Player {
  pub fn main_track(
    library: &Library,
    table: &str,
    gpu: &mut crate::gpu::TrackGpu,
  ) -> Result<Self, String> {
    let owner = library
      .jam()
      .tables
      .iter()
      .find(|t| t.group.eq_ignore_ascii_case("GAMEDATA") && t.name.eq_ignore_ascii_case(table))
      .ok_or("missing main track owner")?;
    for entry in owner
      .entries
      .iter()
      .filter(|e| e.name.to_ascii_uppercase().ends_with(".WDB"))
    {
      let world = library.jam().bytes(entry).map_err(|e| e.to_string())?;
      let Some(name) = lrformats::world::track_model(world) else {
        continue;
      };
      let mut bindings = SceneBindings::load(library, table, world)?;
      bindings.inherit_named_resources(&SceneBindings::race_resources(library, table)?);
      let model = Model::load_with_materials(library, &name, Some(table), &bindings.materials)
        .map_err(|e| e.to_string())?;
      gpu.enable_scene_render(&model)?;
      let objects = world_material::parse(world)?;
      let references = objects
        .iter()
        .find(|o| o.name.eq_ignore_ascii_case(&name))
        .map_or(&[][..], |o| o.references.as_slice());
      let mut player = Self::load(library, table, world, &model, references, &bindings)?;
      player.update(0.0, |name, surface| gpu.set_scene_surface(name, surface))?;
      return Ok(player);
    }
    Err("original main track world missing".into())
  }
  pub fn load(
    library: &Library,
    owner: &str,
    world: &[u8],
    model: &Model,
    references: &[Reference],
    bindings: &SceneBindings,
  ) -> Result<Self, String> {
    let mut tracks = Vec::new();
    for reference in references {
      if reference.level != 0 {
        return Err("race-world material LOD reference requires alternate model renderer".into());
      }
      let animation = world_material::load(library, owner, world, reference)?;
      let target = model
        .mesh
        .textures
        .get(reference.slot)
        .ok_or("WDB animated material slot outside GDB")?
        .clone();
      if !model.surfaces.iter().any(|s| {
        s.material
          .as_deref()
          .is_some_and(|name| name.eq_ignore_ascii_case(&target))
      }) {
        return Err(format!(
          "WDB animated slot {target} has no rendered surface"
        ));
      }
      let channel = &animation.channels[reference.channel];
      let mut surfaces = HashMap::new();
      for key in &animation.keys[channel.start..channel.start + channel.count] {
        if surfaces.contains_key(&key.name) {
          continue;
        }
        let material = bindings
          .materials
          .get(&key.name.to_ascii_lowercase())
          .ok_or_else(|| format!("missing original world animated material {}", key.name))?;
        let texture = material
          .texture
          .as_ref()
          .map(|name| {
            let definition = bindings
              .textures
              .get(&name.to_ascii_lowercase())
              .ok_or_else(|| format!("world animated texture {name} has no owning TDB entry"))?;
            let filename = format!("{name}.{}", if definition.targa { "TGA" } else { "BMP" });
            let bytes = library
              .find_in(&filename, owner)
              .or_else(|| library.find_in(&filename, "COMMON"))
              .ok_or_else(|| format!("missing original animated texture {owner}/{filename}"))?;
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
            // Repeated sampling is configured by the Bevy image backend.
            Ok::<_, String>(texture)
          })
          .transpose()?;
        let color = if texture.is_some() {
          [255; 4]
        } else {
          material.base_color()
        };
        let pipeline = crate::scene_material::load_blend(material.blend)?;
        surfaces.insert(
          key.name.clone(),
          Surface {
            texture,
            pipeline,
            color,
          },
        );
      }
      tracks.push(Track {
        animation,
        channel: reference.channel,
        target,
        last: None,
        surfaces,
      });
    }
    Ok(Self { tracks })
  }
  pub fn reset(&mut self) {
    for track in &mut self.tracks {
      track.last = None;
    }
  }
  pub fn update(
    &mut self,
    seconds: f32,
    mut apply: impl FnMut(&str, &Surface),
  ) -> Result<(), String> {
    for track in &mut self.tracks {
      let name = track.animation.sample(track.channel, seconds)?;
      if track.last.as_deref() == Some(name) {
        continue;
      }
      apply(
        &track.target,
        track
          .surfaces
          .get(name)
          .ok_or("missing preloaded world material key")?,
      );
      track.last = Some(name.to_owned());
    }
    Ok(())
  }
}
