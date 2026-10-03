//! Native original one-sided geometry; pipeline specialization lives in Bevy.
use crate::platform::Material;
use std::{cell::RefCell, collections::HashMap};
thread_local! {static PIPELINES:RefCell<HashMap<Option<[u8;2]>,Material>>=RefCell::new(HashMap::new());}
pub fn load() -> Result<Material, String> {
  load_blend(None)
}
pub fn load_blend(blend: Option<[u8; 2]>) -> Result<Material, String> {
  PIPELINES.with(|cache| {
    Ok(
      *cache
        .borrow_mut()
        .entry(blend)
        .or_insert_with(|| Material::scene(blend)),
    )
  })
}
