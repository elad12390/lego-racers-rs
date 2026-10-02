//! Original COMMON CHAMPS.CCB and DRIVERS.DDB model/chassis references.
use crate::named_records::{self,Records};

pub struct Car {pub name:String,pub models:[String;3],pub chassis:String,pub mass:f32}
pub struct Driver {pub name:String,pub models:[String;3],pub car:String,pub parts:[usize;4]}

pub fn cars(bytes:&[u8])->Result<Vec<Car>,String> {
    Records::parse(bytes)?.entries.into_iter().map(|(name,f)|Ok(Car {
        name,models:[named_records::string(&f,0x28)?,named_records::string(&f,0x29)?,named_records::string(&f,0x2a)?],
        chassis:named_records::string(&f,0x2b)?,
        mass:match named_records::value(&f,0x2c) {Some(crate::tok::Node::Float(m)) if m.is_finite()&&*m>0.0=>*m,_=>return Err("invalid original car mass".into())},
    })).collect()
}

pub fn drivers(bytes:&[u8])->Result<Vec<Driver>,String> {
    Records::parse(bytes)?.entries.into_iter().map(|(name,f)|Ok(Driver {
        name,models:[named_records::string(&f,0x28)?,named_records::string(&f,0x29)?,named_records::string(&f,0x2a)?],
        car:named_records::string(&f,0x2b)?,
        parts:[0x35,0x36,0x37,0x38].map(|key|named_records::integer(&f,key).and_then(|i|usize::try_from(i).map_err(|_|"negative driver part index".into()))).into_iter().collect::<Result<Vec<_>,_>>()?.try_into().map_err(|_|"driver part count")?,
    })).collect()
}
