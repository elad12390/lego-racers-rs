//! Original TDB image formats and explicit RGB transparency keys.
use crate::{named_records::Records,tok::{Node,Value},mdb::Material};
use std::collections::HashMap;
pub struct Texture {pub name:String,pub targa:bool,pub color_key:Option<[u8;3]>}
pub fn parse(bytes:&[u8])->Result<Vec<Texture>,String> {
    let records=Records::parse(bytes)?;let mut textures=Vec::new();
    for (name,fields) in records.entries {
        let color_key=fields.iter().find_map(|v|if let Node::Record {kind:0x17,fields}=v {Some(fields)}else {None}).map(|fields| {
            if fields.len()!=3 {return Err("invalid TDB transparency color".into());}
            fields.iter().map(|v|match v {Value::U8(i)=>Ok(*i),_=>v.as_u32().and_then(|i|u8::try_from(i).ok()).ok_or("invalid TDB transparency component")}).collect::<Result<Vec<_>,_>>().map(|v|v.try_into().unwrap())
        }).transpose()?;
        textures.push(Texture {name,targa:fields.contains(&Node::Keyword(0x2b)),color_key});
    }Ok(textures)
}
pub fn bind(materials:&mut HashMap<String,Material>,textures:&[Texture]) {
    for material in materials.values_mut() {if let Some(texture)=material.texture.as_ref().and_then(|name|textures.iter().find(|t|t.name.eq_ignore_ascii_case(name))) {material.color_key=texture.color_key;}}
}
