//! Named MIB widgets. Rectangles are retained as original inset values;
//! interpretation (including parent-relative anchoring) belongs to the UI.
use crate::tok::{self,Node,Value};
use std::collections::BTreeMap;

#[derive(Debug)]
pub struct Widget {pub kind:u8,pub rect:Option<[i32;4]>,pub parent:Option<String>}
pub struct Layout {pub widgets:BTreeMap<String,Widget>}
impl Layout {
    pub fn parse(bytes:&[u8])->Result<Self,String> {
        let nodes=tok::parse(bytes).map_err(|e|e.to_string())?;
        let mut widgets=BTreeMap::new();
        for section in nodes.windows(3) {
            let [Node::Keyword(kind),Node::Count(count),Node::Block(body)]=section else {continue;};
            let mut found=0;
            for row in body.chunks_exact(3) {
                let [Node::Keyword(tag),Node::Str(name),Node::Block(fields)]=row else {return Err("invalid MIB widget".into());};
                if tag!=kind {return Err("MIB widget type mismatch".into());}
                let style=fields.windows(2).find_map(|pair|if let [Node::Keyword(0x36),Node::Block(style)]=pair {Some(style)} else {None}).unwrap_or(fields);
                let packed=style.windows(2).find_map(|pair|if let [Node::Keyword(0x2f),Node::Packed {kind:4,rows}]=pair {Some(rows)} else {None});
                let rect=packed.map(|packed| {
                    let values=packed.iter().flatten().map(|v|if let Value::I32(v)=v {Ok(*v)} else {Err("MIB rectangle is not integer")}).collect::<Result<Vec<_>,_>>()?;
                    values.try_into().map_err(|_|"MIB rectangle needs four insets")
                }).transpose()?;
                let parent=style.windows(2).find_map(|pair|if let [Node::Keyword(0x31),Node::Str(name)]=pair {Some(name.clone())} else {None});
                if widgets.insert(name.to_ascii_lowercase(),Widget {kind:*kind,rect,parent}).is_some() {return Err(format!("duplicate MIB widget {name}"));}
                found+=1;
            }
            if body.len()%3!=0||found!=*count {return Err("MIB widget count mismatch".into());}
        }
        if widgets.is_empty() {return Err("MIB layout contains no widgets".into());}
        Ok(Self {widgets})
    }
}
