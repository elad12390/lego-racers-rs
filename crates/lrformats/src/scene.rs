//! WDB resource bindings. The world names material databases independently of GDB.
use crate::{
  library::Library,
  mdb::{self, Material},
  tok::{self, Node},
};
use std::collections::{HashMap, HashSet};

pub struct SceneBindings {
  pub materials: HashMap<String, Material>,
  pub textures: HashMap<String, crate::texture_catalog::Texture>,
  pub material_tables: Vec<Vec<Material>>,
  pub unavailable_texture_catalogs: Vec<String>,
}

impl SceneBindings {
  /// Race worlds share the renderer's named resource context (GolDP
  /// 10026110/10028c20), but indexed MDB references remain world-local.
  /// Only declarations in this owning race table are considered.
  pub fn race_resources(library: &Library, table: &str) -> Result<Self, String> {
    let owner = library
      .jam()
      .tables
      .iter()
      .find(|t| t.group.eq_ignore_ascii_case("GAMEDATA") && t.name.eq_ignore_ascii_case(table))
      .ok_or("missing race resource owner")?;
    let mut shared = Self {
      materials: HashMap::new(),
      textures: HashMap::new(),
      material_tables: Vec::new(),
      unavailable_texture_catalogs: Vec::new(),
    };
    let mut ambiguous_materials = HashSet::new();
    let mut ambiguous_textures = HashSet::new();
    for entry in owner
      .entries
      .iter()
      .filter(|e| e.name.to_ascii_uppercase().ends_with(".WDB"))
    {
      let bindings = Self::load(
        library,
        table,
        library.jam().bytes(entry).map_err(|e| e.to_string())?,
      )?;
      for (name, material) in bindings.materials {
        if ambiguous_materials.contains(&name) {
          continue;
        }
        if shared
          .materials
          .get(&name)
          .is_some_and(|old| old != &material)
        {
          shared.materials.remove(&name);
          ambiguous_materials.insert(name);
          continue;
        }
        shared.materials.insert(name, material);
      }
      for (name, texture) in bindings.textures {
        if ambiguous_textures.contains(&name) {
          continue;
        }
        if shared
          .textures
          .get(&name)
          .is_some_and(|old| old != &texture)
        {
          shared.textures.remove(&name);
          ambiguous_textures.insert(name);
          continue;
        }
        shared.textures.insert(name, texture);
      }
      shared
        .unavailable_texture_catalogs
        .extend(bindings.unavailable_texture_catalogs);
    }
    Ok(shared)
  }
  pub fn inherit_named_resources(&mut self, shared: &Self) {
    for (name, material) in &shared.materials {
      self
        .materials
        .entry(name.clone())
        .or_insert_with(|| material.clone());
    }
    for (name, texture) in &shared.textures {
      self
        .textures
        .entry(name.clone())
        .or_insert_with(|| texture.clone());
    }
  }
  pub fn load(library: &Library, table: &str, data: &[u8]) -> Result<Self, String> {
    let nodes = tok::parse(data).map_err(|e| e.to_string())?;
    let mut materials = HashMap::new();
    let mut material_tables = Vec::new();
    let mut textures = HashMap::new();
    let mut unavailable_texture_catalogs = Vec::new();
    if let Some(index) = nodes.iter().position(|n| *n == Node::Keyword(0x27)) {
      let body = nodes[index + 1..]
        .iter()
        .find_map(|n| {
          if let Node::Block(body) = n {
            Some(body)
          } else {
            None
          }
        })
        .ok_or("WDB material database list missing")?;
      let names = body.iter().flat_map(|n| match n {
        Node::Str(name) => vec![name.as_str()],
        Node::PackedStrings(names) => names.iter().map(String::as_str).collect(),
        _ => Vec::new(),
      });
      for name in names {
        let filename = format!("{name}.MDB");
        let bytes = library
          .find_in(&filename, table)
          .or_else(|| library.find_in(&filename, "COMMON"))
          .ok_or_else(|| format!("{table}: declared material database{filename}missing"))?;
        let database = mdb::parse(bytes).map_err(|e| format!("{filename}:{e}"))?;
        material_tables.push(database.clone());
        for material in database {
          materials.insert(material.name.to_ascii_lowercase(), material);
        }
      }
    }
    // Texture catalogs are independently named by WDB 0x28. A material's
    // texture flags do not specify its transparent RGB color.
    if let Some(body) = nodes.windows(3).find_map(|v| {
      if let [Node::Keyword(0x28), Node::Count(_), Node::Block(body)] = v {
        Some(body)
      } else {
        None
      }
    }) {
      for name in body.iter().flat_map(|n| match n {
        Node::Str(name) => vec![name.as_str()],
        Node::PackedStrings(names) => names.iter().map(String::as_str).collect(),
        _ => Vec::new(),
      }) {
        let Some(bytes) = library
          .find_in(&format!("{name}.TDB"), table)
          .or_else(|| library.find_in(&format!("{name}.TDB"), "COMMON"))
        else {
          unavailable_texture_catalogs.push(format!("{table}/{name}.TDB"));
          continue;
        };
        let catalog = crate::texture_catalog::parse(bytes)?;
        crate::texture_catalog::bind(&mut materials, &catalog);
        for texture in catalog {
          textures.insert(texture.name.to_ascii_lowercase(), texture);
        }
      }
    }
    for database in &mut material_tables {
      for material in database {
        if let Some(texture) = material
          .texture
          .as_ref()
          .and_then(|n| textures.get(&n.to_ascii_lowercase()))
        {
          material.color_key = texture.color_key;
        }
      }
    }
    Ok(Self {
      materials,
      material_tables,
      textures,
      unavailable_texture_catalogs,
    })
  }
  pub fn sprite_material(
    &self,
    reference: &crate::world_billboards::MaterialRef,
  ) -> Result<&Material, String> {
    match reference {
      crate::world_billboards::MaterialRef::Name(name) => self
        .materials
        .get(&name.to_ascii_lowercase())
        .ok_or_else(|| format!("missing declared sprite material {name}")),
      crate::world_billboards::MaterialRef::Index { database, slot } => self
        .material_tables
        .get(*database)
        .and_then(|table| table.get(*slot))
        .ok_or_else(|| format!("sprite material outside declared MDB: {database}/{slot}")),
    }
  }
}
