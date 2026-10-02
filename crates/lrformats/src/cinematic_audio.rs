//! Original CEB named sound effects resolved through owning SBK/PCM resources.
use crate::{library::Library,cinematic::Event,named_records,tok::{self,Node}};
#[derive(Clone,Debug)]
pub struct Cue {pub file:String,pub frame:u32,pub gain:f32}
pub struct Plan {pub cues:Vec<Cue>,pub unavailable_banks:Vec<String>}
pub fn bank(bytes:&[u8])->Result<Vec<String>,String> {
    let text=std::str::from_utf8(bytes).map_err(|_|"invalid original sound bank text")?;let mut lines=text.lines().map(str::trim).filter(|s|!s.is_empty());
    let count=lines.next().ok_or("empty sound bank")?.parse::<usize>().map_err(|_|"invalid sound bank count")?;
    let names=lines.map(str::to_owned).collect::<Vec<_>>();if names.len()!=count {return Err("sound bank count mismatch".into());}Ok(names)
}
pub(crate) fn records(nodes:&[Node],key:u8)->Result<Vec<(&str,&[Node])>,String> {
    let Some(body)=nodes.windows(3).find_map(|v|if let [Node::Keyword(k),Node::Count(_),Node::Block(body)]=v {(*k==key).then_some(body)}else {None}) else {return Ok(Vec::new());};
    let mut records=Vec::new();for row in body.chunks_exact(3) {let [Node::Keyword(k),Node::Str(name),Node::Block(fields)]=row else {return Err("invalid CEB record".into());};if *k!=key {return Err("CEB record type mismatch".into());}records.push((name.as_str(),fields.as_slice()));}if body.len()%3!=0 {return Err("incomplete CEB record".into());}Ok(records)
}
pub fn cues(library:&Library,table:&str,cdb:&str,events:&[Event])->Result<Vec<Cue>,String> {
    Ok(plan(library,table,cdb,events)?.cues)
}
pub fn plan(library:&Library,table:&str,cdb:&str,events:&[Event])->Result<Plan,String> {
    let empty=||Plan {cues:Vec::new(),unavailable_banks:Vec::new()};
    let Some(bytes)=library.find_at(&format!("{cdb}.CEB"),"MENUDATA",table) else {return Ok(empty());};let nodes=tok::parse(bytes).map_err(|e|e.to_string())?;
    let sounds=records(&nodes,0x2f)?;if sounds.is_empty() {return Ok(empty());}
    let bindings=records(&nodes,0x56)?;
    let sbk=if let Some(bytes)=library.find_at(&format!("{cdb}.SBK"),"MENUDATA",table) {bytes}else {
        let owner=library.jam().tables.iter().find(|t|t.group.eq_ignore_ascii_case("MENUDATA")&&t.name.eq_ignore_ascii_case(table)).ok_or("missing cinematic owning table")?;
        let banks=owner.entries.iter().filter(|e|e.name.to_ascii_uppercase().ends_with(".SBK")).collect::<Vec<_>>();
        if banks.is_empty() {return Ok(Plan {cues:Vec::new(),unavailable_banks:vec![format!("{table}/{cdb}.SBK (declared by original CEB but absent from owning archive)")]});}
        if banks.len()!=1 {return Err("CEB sounds require one unambiguous owning sound bank".into());}library.jam().bytes(banks[0]).map_err(|e|e.to_string())?
    };
    let bank=bank(sbk)?;let mut cues=Vec::new();
    for event in events {
        for (_,binding) in bindings.iter().filter(|(name,_)|name.eq_ignore_ascii_case(&event.name)) {
            if !binding.contains(&Node::Keyword(0x2f)) {continue;}
            let name=named_records::string(binding,0x4e)?;let fields=sounds.iter().find(|(n,_)|n.eq_ignore_ascii_case(&name)).ok_or("missing named CEB sound")?.1;
            let reference=fields.windows(3).find_map(|v|if let [Node::Keyword(0x30),Node::Int(bank),Node::Int(index)]=v {Some((*bank,*index))}else {None}).ok_or("missing CEB sound bank reference")?;
            if reference.0!=0 {return Err("cinematic uses an unsupported secondary sound bank".into());}
            let index=usize::try_from(reference.1).map_err(|_|"negative CEB sound index")?;let file=bank.get(index).ok_or("CEB sound outside owning bank")?.clone();
            let gain=match named_records::value(fields,0x33) {Some(Node::Float(v)) if v.is_finite()&&*v>=0.0=>*v,None=>1.0,_=>return Err("invalid CEB sound gain".into())};
            cues.push(Cue {file,frame:event.start,gain});
        }
    }
    cues.sort_by_key(|c|c.frame);Ok(Plan {cues,unavailable_banks:Vec::new()})
}
