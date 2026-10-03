//! Original car/wheel/driver geometry in menu preview viewports.
use crate::platform::prelude::*;
use crate::{
  game_catalog::Catalog, gpu::TrackGpu, menu_camera::MenuCamera, menu_driver::MenuDriver,
  menu_ui::MenuUi, profile::Profile,
};
use lrformats::{library::Library, model::Model};

pub struct RacerPreview {
  body: TrackGpu,
  wheels: crate::wheels::Wheels,
  driver: MenuDriver,
  camera: MenuCamera,
  room: Vec<(TrackGpu, Mat4)>,
  car: Mat4,
  person: Mat4,
}
impl RacerPreview {
  pub fn load(library: &Library, catalog: &Catalog, profile: &Profile) -> Result<Self, String> {
    let car = catalog
      .cars
      .iter()
      .find(|c| c.name.eq_ignore_ascii_case(&profile.car))
      .ok_or("missing preview car")?;
    let model = if profile.custom_enabled {
      crate::brick_model::model(
        library,
        &lrsim::brick_build::BuilderData::load(library)?,
        profile
          .custom_build
          .as_ref()
          .ok_or("missing custom build")?,
      )?
    } else {
      Model::load(library, &car.models[0], Some("COMMON")).map_err(|e| e.to_string())?
    };
    let chassis = lrformats::cmb::parse(
      library
        .find_in("CHASSIS.CMB", "COMMON")
        .ok_or("missing chassis")?,
    )
    .map_err(|e| e.to_string())?
    .into_iter()
    .find(|c| c.name.eq_ignore_ascii_case(&car.chassis))
    .ok_or("missing preview chassis")?;
    let wheels = crate::wheels::Wheels::load(
      library,
      &chassis
        .wheel_models
        .first()
        .ok_or("missing preview wheels")?
        .1,
    )?;
    let appearance = catalog
      .drivers
      .iter()
      .find(|d| d.name.eq_ignore_ascii_case(&profile.driver))
      .ok_or("missing preview driver")?;
    let build =
      profile.custom_driver.clone().map(Ok).unwrap_or_else(|| {
        crate::custom_driver::Data::load(library)?.original_driver(appearance)
      })?;
    let driver = MenuDriver::load(library, &build)?;
    let world = library
      .find_at("RACER.WDB", "MENUDATA", "RS_SET")
      .ok_or("missing original racer stage")?;
    let camera = MenuCamera::load(world)?;
    let bindings = lrformats::scene::SceneBindings::load(library, "RS_SET", world)?;
    let mut room = Vec::new();
    let mut car = None;
    let mut person = None;
    for instance in lrformats::world::instances(world).map_err(|e| e.to_string())? {
      let transform = crate::gpu::instance_transform(&instance);
      match instance.model.as_str() {
        "carp" => car = Some(transform),
        "gyplac" => person = Some(transform),
        name => room.push((
          TrackGpu::upload(
            &Model::load_with_materials(library, name, Some("RS_SET"), &bindings.materials)
              .map_err(|e| e.to_string())?,
          )?,
          transform,
        )),
      }
    }
    Ok(Self {
      body: TrackGpu::upload(&model)?,
      wheels,
      driver,
      camera,
      room,
      car: car.ok_or("racer stage car placement missing")?,
      person: person.ok_or("racer stage driver placement missing")?,
    })
  }
  pub fn main_driver(&mut self) -> Result<(), String> {
    self.driver.advance(get_frame_time().min(0.1))?;
    camera(
      Rect::new(312.0, 120.0, 315.0, 360.0),
      vec3(8.0, 2.3, 2.0),
      vec3(0.0, 1.5, 0.0),
    );
    self.driver.draw_at(Mat4::IDENTITY);
    set_default_camera();
    Ok(())
  }
  pub fn garage(&mut self, ui: &MenuUi) -> Result<(), String> {
    self.driver.advance(get_frame_time().min(0.1))?;
    ui.preview_frame(230.0, 152.0, 384.0, 302.0);
    self
      .camera
      .draw(Rect::new(233.0, 155.0, 378.0, 296.0), Vec3::ZERO, 0.0);
    for (gpu, transform) in &self.room {
      gpu.draw_at(*transform);
    }
    self.body.draw_at(self.car);
    self.wheels.draw_at(self.car);
    self.driver.draw_at(self.person);
    set_default_camera();
    Ok(())
  }
}
pub fn camera(rect: Rect, position: Vec3, target: Vec3) {
  let s = MenuUi::scale();
  let o = MenuUi::origin();
  set_camera(&Camera3D {
    position,
    target,
    up: Vec3::Y,
    aspect: Some(rect.w / rect.h),
    viewport: Some((
      (o.x + rect.x * s) as i32,
      (screen_height() - o.y - (rect.y + rect.h) * s) as i32,
      (rect.w * s) as i32,
      (rect.h * s) as i32,
    )),
    fovy: 45.0f32.to_radians(),
    ..Default::default()
  });
}
