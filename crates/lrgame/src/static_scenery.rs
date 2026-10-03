//! Original static WDB placements with independently timed material channels.
use crate::platform::prelude::*;
use crate::{
  gpu::{instance_transform, TrackGpu},
  race_view::RaceView,
  world_material::Player,
};
use lrformats::{library::Library, model::Model, scene::SceneBindings};

pub struct Object {
  pub name: String,
  gpu: TrackGpu,
  transform: Mat4,
  materials: Player,
  time: f32,
  uv: lrsim::uv_scroll::Clock,
}
impl Object {
  pub fn load(
    library: &Library,
    table: &str,
    world: &[u8],
    bindings: &SceneBindings,
  ) -> Result<Vec<Self>, String> {
    let material_objects = lrformats::world_material::parse(world)?;
    let mut out = Vec::new();
    let track = lrformats::world::track_model(world);
    for source in lrformats::world::named_instances(world).map_err(|e| e.to_string())? {
      if track
        .as_ref()
        .is_some_and(|n| n.eq_ignore_ascii_case(&source.placement.model))
      {
        continue;
      }
      let model = Model::load_with_materials(
        library,
        &source.placement.model,
        Some(table),
        &bindings.materials,
      )
      .map_err(|e| e.to_string())?;
      let references = material_objects
        .iter()
        .find(|o| o.name.eq_ignore_ascii_case(&source.name))
        .map_or(&[][..], |o| o.references.as_slice());
      let mut gpu = TrackGpu::upload(&model)?;
      gpu.enable_scene_render(&model)?;
      let mut materials = Player::load(library, table, world, &model, references, bindings)?;
      materials.update(0.0, |name, surface| gpu.set_scene_surface(name, surface))?;
      let uv =
        lrsim::uv_scroll::Clock::new(lrformats::world_uv::for_object(world, &source.name)?.rate);
      out.push(Self {
        name: source.name,
        gpu,
        transform: instance_transform(&source.placement),
        materials,
        time: 0.0,
        uv,
      });
    }
    Ok(out)
  }
  pub fn update(&mut self, dt: f32) -> Result<(), String> {
    self.time += dt.max(0.0);
    self.gpu.set_uv_offset(self.uv.advance(dt));
    self.materials.update(self.time, |name, surface| {
      self.gpu.set_scene_surface(name, surface)
    })
  }
  pub fn reset(&mut self) -> Result<(), String> {
    self.time = 0.0;
    self.uv.reset();
    self.materials.reset();
    self.update(0.0)
  }
  pub fn bounds(&self) -> (Vec3, f32) {
    (
      self.transform.transform_point3(self.gpu.center),
      self.gpu.radius * self.transform.x_axis.truncate().length(),
    )
  }
  pub fn draw(&self, view: RaceView) {
    self.gpu.draw_at(view.world_matrix() * self.transform);
    gl_use_default_material();
  }
  /// SkyDatabase replaces the child position, retaining the declared basis.
  pub fn draw_sky(&self, camera_relative_position: Vec3) {
    let mut transform = self.transform;
    transform.w_axis = camera_relative_position.extend(1.0);
    self.gpu.draw_at(transform);
    gl_use_default_material();
  }
}
