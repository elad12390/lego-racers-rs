//! WDB resource bindings. The world names material databases independently of GDB.
use crate::{
    library::Library,
    mdb::{self, Material},
    tok::{self, Node},
};
use std::collections::HashMap;

pub struct SceneBindings {
    pub materials: HashMap<String, Material>,
    pub unavailable_texture_catalogs:Vec<String>,
}

impl SceneBindings {
    pub fn load(library: &Library, table: &str, data: &[u8]) -> Result<Self, String> {
        let nodes = tok::parse(data).map_err(|e| e.to_string())?;
        let mut materials = HashMap::new();let mut unavailable_texture_catalogs=Vec::new();
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
                    .ok_or_else(|| {
                        format!("{table}: declared material database{filename}missing")
                    })?;
                for material in mdb::parse(bytes).map_err(|e| format!("{filename}:{e}"))? {
                    materials.insert(material.name.to_ascii_lowercase(), material);
                }
            }
        }
        // Texture catalogs are independently named by WDB 0x28. A material's
        // texture flags do not specify its transparent RGB color.
        if let Some(body)=nodes.windows(3).find_map(|v|if let [Node::Keyword(0x28),Node::Count(_),Node::Block(body)]=v {Some(body)}else {None}) {
            for name in body.iter().flat_map(|n|match n {Node::Str(name)=>vec![name.as_str()],Node::PackedStrings(names)=>names.iter().map(String::as_str).collect(),_=>Vec::new()}) {
                let Some(bytes)=library.find_in(&format!("{name}.TDB"),table).or_else(||library.find_in(&format!("{name}.TDB"),"COMMON")) else {unavailable_texture_catalogs.push(format!("{table}/{name}.TDB"));continue;};
                crate::texture_catalog::bind(&mut materials,&crate::texture_catalog::parse(bytes)?);
            }
        }
        Ok(Self { materials,unavailable_texture_catalogs })
    }
}
