//! Shared strict counted named-record framing used by the original table loaders.
use crate::tok::{self,Node};

pub struct Records {pub entries:Vec<(String,Vec<Node>)>}

impl Records {
    /// CDBs prefix their counted named table with a WDB resource list.
    pub fn parse_with_prefix(bytes:&[u8])->Result<Self,String> {
        let nodes=tok::parse(bytes).map_err(|e|e.to_string())?;
        let at=nodes.windows(3).position(|v|matches!(v,[Node::Keyword(0x27),Node::Count(_),Node::Block(_)])).ok_or("missing counted named table")?;
        Self::from_nodes(&nodes[at..at+3])
    }
    pub fn parse(bytes:&[u8])->Result<Self,String> {
        let nodes=tok::parse(bytes).map_err(|e|e.to_string())?;
        Self::from_nodes(&nodes)
    }
    fn from_nodes(nodes:&[Node])->Result<Self,String> {
        let [Node::Keyword(0x27),Node::Count(count),Node::Block(body)]=nodes else {return Err("expected counted named table".into());};
        let mut entries=Vec::new();let mut seen=std::collections::HashSet::new();
        for row in body.chunks_exact(3) {
            let [Node::Keyword(0x27),Node::Str(name),Node::Block(fields)]=row else {return Err("malformed named table entry".into());};
            if !seen.insert(name.to_ascii_lowercase()) {return Err(format!("duplicate table entry{name}"));}
            entries.push((name.clone(),fields.clone()));
        }
        if body.len()%3!=0 || entries.len()!=*count as usize {return Err("named table count mismatch".into());}
        Ok(Self {entries})
    }
}

pub fn value(fields:&[Node],key:u8)->Option<&Node> {
    fields.windows(2).find_map(|p|if p[0]==Node::Keyword(key) {Some(&p[1])} else {None})
}

pub fn string(fields:&[Node],key:u8)->Result<String,String> {
    match value(fields,key) {Some(Node::Str(s))=>Ok(s.clone()),_=>Err(format!("missing string field{key:02x}"))}
}

pub fn integer(fields:&[Node],key:u8)->Result<i32,String> {
    match value(fields,key) {Some(Node::Int(i))=>Ok(*i),_=>Err(format!("missing integer field{key:02x}"))}
}
