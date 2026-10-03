use std::collections::HashMap;
use std::fmt;

use crate::bmp::{self, Image};
use crate::gdb::{self, Mesh};
use crate::library::Library;
use crate::mdb::{self, Material};

#[derive(Debug)]
pub enum ModelError {
  NotFound(String),
  Gdb(gdb::GdbError),
  Invalid(String),
}

impl fmt::Display for ModelError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      ModelError::NotFound(name) => write!(f, "{name} not found in the archive"),
      ModelError::Gdb(e) => write!(f, "{e}"),
      ModelError::Invalid(why) => write!(f, "{why}"),
    }
  }
}

impl std::error::Error for ModelError {}

/// One drawable group: triangles (absolute vertex indices) with the surface they use.
#[derive(Debug, Clone)]
pub struct Surface {
  pub material: Option<String>,
  pub color: [u8; 4],
  pub texture: Option<String>,
  pub triangles: Vec<[u32; 3]>,
  /// Rigid-joint bindings retained per corner from the persistent GDB cache.
  pub joints: Vec<[u16; 3]>,
  pub color_key: Option<[u8; 3]>,
  pub blend: Option<[u8; 2]>,
}

#[derive(Debug)]
pub struct Model {
  pub mesh: Mesh,
  pub surfaces: Vec<Surface>,
  pub images: HashMap<String, Image>,
}

impl Model {
  /// Loads `<name>.GDB` with its `.MDB` materials and the `.BMP` images they reference.
  /// `table` is the archive table to prefer when names exist in several.
  pub fn load(library: &Library, name: &str, table: Option<&str>) -> Result<Self, ModelError> {
    Self::load_with_materials(library, name, table, &HashMap::new())
  }

  pub fn load_with_materials(
    library: &Library,
    name: &str,
    table: Option<&str>,
    scene_materials: &HashMap<String, Material>,
  ) -> Result<Self, ModelError> {
    let bytes = scoped_find(library, &format!("{name}.GDB"), table)
      .ok_or_else(|| ModelError::NotFound(format!("{name}.GDB")))?;
    let mesh = gdb::parse(bytes).map_err(ModelError::Gdb)?;
    let draws = mesh.resolved_draws().map_err(ModelError::Invalid)?;
    let mut materials: HashMap<String, Material> =
      scoped_find(library, &format!("{name}.MDB"), table)
        .map(|data| mdb::parse(data).map_err(|e| ModelError::Invalid(format!("{name}.MDB:{e}"))))
        .transpose()?
        .unwrap_or_default()
        .into_iter()
        .map(|m| (m.name.to_ascii_lowercase(), m))
        .collect();
    materials.extend(scene_materials.iter().map(|(n, m)| (n.clone(), m.clone())));
    if let Some(bytes) = scoped_find(library, &format!("{name}.TDB"), table) {
      let textures = crate::texture_catalog::parse(bytes).map_err(ModelError::Invalid)?;
      crate::texture_catalog::bind(&mut materials, &textures);
    }

    let mut images = HashMap::new();
    let mut surfaces = Vec::new();
    for (part, triangles) in mesh.parts.iter().zip(draws) {
      let material_name = mesh.textures.get(usize::from(part.texture)).cloned();
      let material = material_name
        .as_ref()
        .and_then(|n| materials.get(&n.to_ascii_lowercase()));
      // A material that names no texture of its own may still be an image of the same name.
      let texture = material
        .and_then(|m| m.texture.clone())
        .or_else(|| material_name.clone())
        .filter(|t| load_image(library, t, table, &mut images));
      if let Some(expected) = material.and_then(|m| m.texture.as_ref()) {
        if texture.is_none() {
          return Err(ModelError::NotFound(format!(
            "declared texture {expected}.BMP/.TGA for {name}"
          )));
        }
      }
      surfaces.push(Surface {
        color_key: material.and_then(|m| m.color_key),
        blend: material.and_then(|m| m.blend),
        color: material.map_or([200, 200, 200, 255], Material::base_color),
        material: material_name,
        texture,
        triangles: triangles.iter().map(|t| t.map(|v| v.0)).collect(),
        joints: triangles.iter().map(|t| t.map(|v| v.1)).collect(),
      });
    }
    Ok(Model {
      mesh,
      surfaces,
      images,
    })
  }
}

fn load_image(
  library: &Library,
  name: &str,
  table: Option<&str>,
  images: &mut HashMap<String, Image>,
) -> bool {
  let key = name.to_ascii_lowercase();
  if images.contains_key(&key) {
    return true;
  }
  match scoped_find(library, &format!("{name}.BMP"), table)
    .and_then(|b| bmp::decode(b).ok())
    .or_else(|| {
      scoped_find(library, &format!("{name}.TGA"), table).and_then(|b| crate::tga::decode(b).ok())
    }) {
    Some(image) => {
      images.insert(key, image);
      true
    }
    None => false,
  }
}

fn scoped_find<'a>(library: &'a Library, name: &str, table: Option<&str>) -> Option<&'a [u8]> {
  match table {
    Some(table) => library
      .find_in(name, table)
      .or_else(|| library.find_in(name, "COMMON"))
      .or_else(|| {
        // Menu champion/build scenes bind resources from the shared part
        // databases initialized by PartDatabase_LoadBodyParts. Do not
        // broaden race-world lookup to unrelated owners.
        library
          .jam()
          .tables
          .iter()
          .any(|t| t.group.eq_ignore_ascii_case("MENUDATA") && t.name.eq_ignore_ascii_case(table))
          .then(|| {
            library
              .find_at(name, "MENUDATA", "PARTDB")
              .or_else(|| library.find_at(name, "MENUDATA", "PIECEDB"))
          })
          .flatten()
      }),
    None => library.find(name, None),
  }
}
