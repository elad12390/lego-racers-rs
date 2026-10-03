//! Original PST named track vignette and its selectable minifigure material set.
use crate::platform::Rect;
use crate::{cinematic_scene::Scene, custom_driver::Data, game_catalog::Catalog};
use lrformats::library::Library;
pub fn load(
  library: &Library,
  catalog: &Catalog,
  index: usize,
  time_trial: bool,
) -> Result<Scene, String> {
  let race = catalog
    .races
    .get(index)
    .ok_or("track preview outside race catalog")?;
  let driver = if time_trial {
    "VV"
  } else {
    race
      .driver
      .as_deref()
      .ok_or("race preview driver missing")?
  };
  let driver = catalog
    .drivers
    .iter()
    .find(|d| d.name.eq_ignore_ascii_case(driver))
    .ok_or("preview driver outside original catalog")?;
  let build = Data::load(library)?.original_driver(driver)?;
  Scene::load_named(
    library,
    "SINGRACE",
    "PST",
    Some(
      race
        .image
        .as_deref()
        .ok_or("missing original race preview scene")?,
    ),
    Some(&build),
  )
}
pub const VIEWPORT: Rect = Rect {
  x: 260.0,
  y: 135.0,
  w: 320.0,
  h: 255.0,
};
