//! Original LEGORACE.RCB race definitions and CRB six-entry circuit rosters.
use crate::{named_records::{self,Records},tok::Node};

pub struct Race {
    pub name:String,pub table:String,pub circuit:Option<String>,pub slot:u32,
    /// Original keyword2b stores a short at CircuitRaceDef+34.
    pub text_id:i16,pub mirrored:bool,pub image:Option<String>,pub driver:Option<String>,
}
pub struct Circuit {pub name:String,pub index:u32,pub racers:[String;6],pub next:Option<String>}

pub fn races(bytes:&[u8])->Result<Vec<Race>,String> {
    Records::parse(bytes)?.entries.into_iter().map(|(name,f)| {
        let optional=|key|match named_records::value(&f,key) {
            None=>Ok(None),Some(Node::Str(s))=>Ok(Some(s.clone())),_=>Err(format!("invalid optional string field{key:02x}")),
        };
        Ok(Race {name,table:named_records::string(&f,0x29)?,circuit:optional(0x2a)?,
            slot:match named_records::value(&f,0x28) {None=>0,Some(Node::Int(i))=>u32::try_from(*i).map_err(|_|"negative circuit slot")?,_=>return Err("invalid circuit slot".into())},
            text_id:i16::try_from(named_records::integer(&f,0x2b)?).map_err(|_|"race text id exceeds short")?,
            mirrored:f.contains(&Node::Keyword(0x2c)),image:optional(0x2d)?,driver:optional(0x2e)?})
    }).collect()
}

pub fn circuits(bytes:&[u8])->Result<Vec<Circuit>,String> {
    Records::parse(bytes)?.entries.into_iter().map(|(name,f)| {
        // Original28[count] { packed strings }; preserve player template slot0.
        let list=f.windows(3).find_map(|p|match p {
            [Node::Keyword(0x28),Node::Count(6),Node::Block(b)]=>Some(b),_=>None,
        }).ok_or("circuit must contain6racer entries")?;
        let [Node::PackedStrings(names)]=list.as_slice() else {return Err("invalid circuit roster".into());};
        let racers:[String;6]=names.clone().try_into().map_err(|_|"circuit roster count mismatch")?;
        Ok(Circuit {name,index:u32::try_from(named_records::integer(&f,0x29)?).map_err(|_|"negative circuit index")?,
            racers,next:match named_records::value(&f,0x2b) {Some(Node::Str(s))=>Some(s.clone()),None=>None,_=>return Err("invalid next circuit".into())}})
    }).collect()
}
