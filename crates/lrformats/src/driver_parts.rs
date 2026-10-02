//! Original BODYPART.PCB selection order, geometry variants and reward gates.
use crate::tok::{self,Node};
#[derive(Clone)]
pub struct Part {pub name:String,pub variant:usize,pub unlock:u8}
pub struct Catalog {pub hats:Vec<Part>,pub faces:Vec<Part>,pub torsos:Vec<Part>,pub legs:Vec<Part>,pub heads:Vec<String>}
impl Catalog {
    pub fn parse(bytes:&[u8])->Result<Self,String> {
        let nodes=tok::parse(bytes).map_err(|e|e.to_string())?;
        let section=|key|nodes.windows(3).find_map(|p|if let [Node::Keyword(k),Node::Count(n),Node::Block(body)]=p {(*k==key).then_some((*n as usize,body.as_slice()))} else {None}).ok_or("missing driver part table");
        let read=|key,defaults,stride|->Result<Vec<Part>,String> {
            let (count,body)=section(key)?;let body=body.get(defaults..).ok_or("missing default part models")?;let mut out=Vec::new();
            for row in body.chunks_exact(stride) {
                let Node::Str(name)=&row[0] else {return Err("missing driver part name".into());};
                let Node::Int(unlock)=row[stride-1] else {return Err("missing driver part unlock".into());};
                let variant=if stride==3 {if let Node::Int(v)=row[1] {usize::try_from(v).map_err(|_|"negative geometry variant")?} else {return Err("missing geometry variant".into());}} else {0};
                out.push(Part {name:name.clone(),variant,unlock:u8::try_from(unlock).map_err(|_|"invalid part unlock")?});
            }
            if body.len()%stride!=0||out.len()!=count {return Err("driver part count mismatch".into());}Ok(out)
        };
        let (count,body)=section(0x2d)?;let mut heads=Vec::new();
        for node in body {match node {Node::Str(n)=>heads.push(n.clone()),Node::PackedStrings(n)=>heads.extend(n.clone()),_=>return Err("invalid hat-head table".into())}}
        if heads.len()!=count {return Err("hat-head count mismatch".into());}
        let hats=read(0x29,0,2)?;if hats.len()!=heads.len() {return Err("hat-head mapping mismatch".into());}
        Ok(Self {hats,faces:read(0x2a,1,2)?,torsos:read(0x2b,2,3)?,legs:read(0x2c,2,3)?,heads})
    }
}
