//! Original GHOST.GHB absolute 10-byte poses, Rumble_LoadFile00423160.
use crate::tok::{self,Node,Value};
pub struct Pose {pub position:[f32;3],pub rotation:[f32;4]}
pub struct Ghost {pub lap_ms:[u32;3],pub start:Pose,pub samples:Vec<Pose>,pub flags:u32}
impl Ghost {
    pub fn load(bytes:&[u8],mirrored:bool)->Result<Self,String> {
        let nodes=tok::parse(bytes).map_err(|e|e.to_string())?;
        let body=nodes.iter().find_map(|n|if let Node::Block(b)=n {Some(b)} else {None}).ok_or("missing original ghost body")?;
        let at=|key|body.iter().position(|n|matches!(n,Node::Keyword(k) if *k==key)).ok_or_else(||format!("missing ghost field {key:02x}"));
        let ms=at(0x2a)?;let lap_ms:[Option<u32>;3]=std::array::from_fn(|i|if let Some(Node::Int(v))=body.get(ms+i+1) {u32::try_from(*v).ok()} else {None});
        let lap_ms=lap_ms.into_iter().collect::<Option<Vec<_>>>().ok_or("invalid ghost lap times")?.try_into().unwrap();
        let p=at(0x28)?;let position:[Option<f32>;3]=std::array::from_fn(|i|if let Some(Node::Float(v))=body.get(p+i+1) {Some(*v)} else {None});
        let position=position.into_iter().collect::<Option<Vec<_>>>().ok_or("invalid ghost start")?.try_into().unwrap();
        let r=at(0x29)?;let rotation=match body.get(r+1) {Some(Node::Packed {kind:3,rows})=>rows.iter().flatten().filter_map(|v|if let Value::F32(v)=v {Some(*v)} else {None}).collect::<Vec<_>>(),_=>return Err("invalid ghost start rotation".into())};
        let rotation=rotation.try_into().map_err(|_|"invalid ghost start rotation count")?;
        let f=at(0x2b)?;let flags=match body.get(f+1) {Some(Node::Int(v))=>*v as u32,_=>return Err("invalid ghost flags".into())};
        let s=at(0x27)?;let (count,samples)=match (body.get(s+1),body.get(s+2)) {(Some(Node::Count(c)),Some(Node::Block(b)))=>(*c,b),_=>return Err("invalid ghost sample array".into())};
        if count>961 {return Err("original ghost exceeds 240-second recorder capacity".into());}
        let mut poses=Vec::new();
        for node in samples {
            let Node::Packed {kind:0x17,rows}=node else {return Err("unsupported ghost sample encoding".into());};
            for row in rows {
                if row.len()!=7 {return Err("invalid ghost sample width".into());}
                let number=|i|match row[i] {Value::I16(v)=>Ok(v as f32),Value::U16(v)=>Ok(v as i16 as f32),Value::I8(v)=>Ok(v as f32),Value::U8(v)=>Ok(v as i8 as f32),_=>Err("invalid ghost sample type")};
                poses.push(Pose {position:[number(0)?/32.0,number(1)?/32.0,number(2)?/32.0],rotation:[number(3)?/127.0,number(4)?/127.0,number(5)?/127.0,number(6)?/127.0]});
            }
        }
        if poses.len()!=count as usize {return Err("ghost sample count mismatch".into());}
        let mut ghost=Self {lap_ms,start:Pose {position,rotation},samples:poses,flags};
        if mirrored {for pose in std::iter::once(&mut ghost.start).chain(&mut ghost.samples) {pose.position[1]=-pose.position[1];pose.rotation[1]=-pose.rotation[1];pose.rotation[3]=-pose.rotation[3];}}
        Ok(ghost)
    }
}
