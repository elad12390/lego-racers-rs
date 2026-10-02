//! TMB collision flags: original CarRecord_Read00443cf0. Names are not hints.
use crate::tok::{self, Node, TokError};
use std::collections::HashMap;

pub const NONBLOCKING_CHASSIS: u32 = 0x10000;
pub const NONSUPPORTING_WHEELS: u32 = 0x20000;

/// Effective ordinary wheel values from CarBody0042aad0/0042acb0.
/// Rendering, sound/event IDs and power/effect locks are handled separately.
#[derive(Clone,Copy,Debug)]
pub struct Surface {
    pub flags:u32,
    pub slope_friction:f32,
    pub quadratic_drag:f32,
    pub force:[f32;3],
}

impl Default for Surface {
    fn default()->Self {Self {flags:1,slope_friction:0.5,quadratic_drag:0.0,force:[0.0;3]}}
}

pub fn surfaces(data:&[u8],mirrored:bool)->Result<HashMap<String,Surface>,TokError> {
    let nodes=tok::parse(data)?;
    let Some(Node::Block(body))=nodes.iter().find(|n|matches!(n,Node::Block(_))) else {return Ok(HashMap::new());};
    let mut result=HashMap::new();
    for row in body.windows(3) {
        let [Node::Keyword(0x27),Node::Str(name),Node::Block(fields)]=row else {continue;};
        let mut surface=Surface::default();
        for (i,node) in fields.iter().enumerate() {
            if let Node::Keyword(k)=node {
                if (0x28..=0x39).contains(k) {surface.flags|=1<<(k-0x27);}
                match (*k,fields.get(i+1)) {
                    (0x32,Some(Node::Float(v)))=>surface.slope_friction=*v,
                    (0x36,Some(Node::Float(v)))=>surface.quadratic_drag=*v,
                    (0x2d,_)=> {
                        if let Some([Node::Float(x),Node::Float(y),Node::Float(z)])=fields.get(i+1..i+4) {
                            surface.force=[*x,if mirrored {-*y} else {*y},*z];
                        }
                    }
                    _=>{},
                }
            }
        }
        result.insert(name.to_ascii_lowercase(),surface);
    }
    Ok(result)
}

pub fn blocks_chassis(flags: u32) -> bool {
    flags & NONBLOCKING_CHASSIS == 0
}
pub fn supports_wheels(flags: u32) -> bool {
    flags & NONSUPPORTING_WHEELS == 0
}

pub fn parse(data: &[u8]) -> Result<HashMap<String, u32>, TokError> {
    Ok(surfaces(data,false)?.into_iter().map(|(name,surface)|(name,surface.flags)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_surface_defaults_do_not_invent_a_force_or_drag() {
        let table=surfaces(&[5,0x27,2,b'R',b'O',b'A',b'D',0,5,6,6],false).unwrap();
        let road=table["road"];
        assert_eq!(road.flags,1);
        assert_eq!(road.slope_friction,0.5);
        assert_eq!(road.quadratic_drag,0.0);
        assert_eq!(road.force,[0.0;3]);
    }

    #[test]
    fn explicit_coefficients_and_mirrored_surface_force_keep_their_original_flags() {
        let mut bytes=vec![5,0x27,2,b'I',b'C',b'E',0,5];
        for (key,values) in [(0x32,vec![0.05]),(0x36,vec![900.0]),(0x2d,vec![1.0,2.0,3.0])] {
            bytes.push(key);
            for value in values {bytes.push(3);bytes.extend_from_slice(&f32::to_le_bytes(value));}
        }
        bytes.extend([6,6]);
        let ordinary=surfaces(&bytes,false).unwrap()["ice"];
        let mirrored=surfaces(&bytes,true).unwrap()["ice"];
        assert_eq!(ordinary.flags,1|0x800|0x8000|0x40);
        assert_eq!(parse(&bytes).unwrap()["ice"],ordinary.flags);
        assert_eq!(ordinary.slope_friction,0.05);
        assert_eq!(ordinary.quadratic_drag,900.0);
        assert_eq!(ordinary.force,[1.0,2.0,3.0]);
        assert_eq!(mirrored.force,[1.0,-2.0,3.0]);
        assert_eq!(mirrored.flags,ordinary.flags);
    }
}
