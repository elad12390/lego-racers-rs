//! CDB scene timelines: original object placements, camera and clip ranges.
//! Sound/text/effect tracks remain retained by the source, not synthesized here.
use crate::{named_records,tok::Node};

/// CameraTrigger::Read 00404a10 and FindModel 00404c90:
/// world child, its MAB table, channel record, destination slot, model level.
pub struct MaterialTrack {pub world:usize,pub table:usize,pub channel:usize,pub slot:usize,pub level:usize}
pub struct Object {pub name:String,pub animated:bool,pub sprite:Option<(usize,usize)>,pub position:[f32;3],pub forward:[f32;3],pub up:[f32;3],pub start:u32,pub duration:u32,pub clip:usize,pub materials:Vec<MaterialTrack>}
pub struct Camera {pub name:String,pub start:u32,pub duration:u32}
#[derive(Clone)]
pub struct Event {pub name:String,pub start:u32,pub duration:u32}
pub struct Timeline {pub name:String,pub worlds:Vec<String>,pub fps:f32,pub duration:u32,pub cameras:Vec<Camera>,pub objects:Vec<Object>,pub events:Vec<Event>}

fn section(nodes:&[Node],key:u8)->Option<&[Node]> {
    nodes.windows(3).find_map(|v|if let [Node::Keyword(k),Node::Count(_),Node::Block(b)]=v {(*k==key).then_some(b.as_slice())}else {None})
}
fn values(node:&Node)->Result<Vec<f32>,String> {
    let fields=match node {Node::Packed {rows,..}=>rows.iter().flatten().collect::<Vec<_>>(),Node::Record {fields,..}=>fields.iter().collect(),_=>return Err("invalid CDB vector".into())};
    fields.into_iter().map(|v|v.as_f32().filter(|v|v.is_finite()).ok_or("invalid CDB numeric value".into())).collect()
}
impl Timeline {
    pub fn parse(bytes:&[u8])->Result<Vec<Self>,String> {
        let nodes=crate::tok::parse(bytes).map_err(|e|e.to_string())?;
        let worlds=section(&nodes,0x28).into_iter().flatten().flat_map(|n|match n {Node::Str(name)=>vec![name.clone()],Node::PackedStrings(names)=>names.clone(),_=>Vec::new()}).collect::<Vec<_>>();
        let table=named_records::Records::parse_with_prefix(bytes)?;let mut out=Vec::new();
        for (name,fields) in table.entries {
            let fps=named_records::integer(&fields,0x3b)? as f32;
            let duration=u32::try_from(named_records::integer(&fields,0x2c)?).map_err(|_|"negative CDB duration")?;
            if fps<=0.0||duration==0 {return Err("invalid CDB frame timing".into());}
            let body=section(&fields,0x29).ok_or("missing CDB camera track")?;let mut cameras=Vec::new();
            for row in body.chunks_exact(3) {let [Node::Keyword(0x29),Node::Str(_),Node::Block(fields)]=row else {return Err("invalid CDB camera track".into());};
                cameras.push(Camera {name:named_records::string(fields,0x2a)?,start:u32::try_from(named_records::integer(fields,0x2b)?).map_err(|_|"negative camera start")?,duration:u32::try_from(named_records::integer(fields,0x2c)?).map_err(|_|"negative camera duration")?});
            }
            if cameras.is_empty()||body.len()%3!=0 {return Err("missing or incomplete CDB camera track".into());}
            let body=section(&fields,0x2e).ok_or("missing CDB objects")?;let mut objects=Vec::new();
            for row in body.chunks_exact(3) {
                let [Node::Keyword(0x2e),Node::Str(track),Node::Block(fields)]=row else {return Err("invalid CDB object track".into());};
                let animated=named_records::value(fields,0x30).is_some();
                let sprite=if let Some(at)=fields.iter().position(|v|*v==Node::Keyword(0x32)) {let [Node::Int(world),Node::Int(index)]=fields.get(at+1..at+3).ok_or("incomplete CDB sprite reference")? else {return Err("invalid CDB sprite reference".into());};Some((usize::try_from(*world).map_err(|_|"negative CDB sprite world")?,usize::try_from(*index).map_err(|_|"negative CDB sprite index")?))}else {None};
                let name=if sprite.is_some() {track.clone()}else {named_records::string(fields,if animated {0x30}else {0x2f})?};
                let at=fields.iter().position(|v|*v==Node::Keyword(0x33)).ok_or("missing CDB position")?;
                let p=fields.get(at+1..at+4).ok_or("incomplete CDB position")?.iter().map(|v|match v {Node::Float(f) if f.is_finite()=>Ok(*f),_=>Err("invalid CDB position")}).collect::<Result<Vec<_>,_>>()?;
                let basis=if sprite.is_some() {vec![1.0,0.0,0.0,0.0,0.0,1.0]}else {values(named_records::value(fields,0x34).ok_or("missing CDB orientation")?)?};
                if basis.len()!=6 {return Err("invalid CDB orientation length".into());}
                let start=u32::try_from(named_records::integer(fields,0x2b)?).map_err(|_|"negative CDB start")?;
                let duration=u32::try_from(named_records::integer(fields,0x2c)?).map_err(|_|"negative CDB object duration")?;
                let clip=if animated {usize::try_from(named_records::integer(fields,0x2d)?).map_err(|_|"negative CDB animation index")?}else {0};
                let mut materials=Vec::new();if let Some(body)=section(fields,0x36) {let values=body.iter().flat_map(|n|match n {Node::Packed {rows,..}=>rows.iter().flatten().map(|v|v.as_u32().and_then(|v|i32::try_from(v).ok())).collect::<Vec<_>>(),Node::Int(i)=>vec![Some(*i)],_=>vec![None]}).collect::<Option<Vec<_>>>().ok_or("invalid CDB material animation row")?;
                    for row in values.chunks_exact(5) {materials.push(MaterialTrack {world:usize::try_from(row[0]).map_err(|_|"negative CDB material world")?,table:usize::try_from(row[1]).map_err(|_|"negative CDB material table")?,channel:usize::try_from(row[2]).map_err(|_|"negative CDB material channel")?,slot:usize::try_from(row[3]).map_err(|_|"negative CDB material slot")?,level:usize::try_from(row[4]).map_err(|_|"negative CDB material level")?});}if values.len()%5!=0 {return Err("incomplete CDB material animation row".into());}}
                objects.push(Object {name,animated,sprite,position:[p[0],p[1],p[2]],forward:[basis[0],basis[1],basis[2]],up:[basis[3],basis[4],basis[5]],start,duration,clip,materials});
            }
            if body.len()%3!=0 {return Err("incomplete CDB object track".into());}
            let mut events=Vec::new();
            if let Some(body)=section(&fields,0x37) {for row in body.chunks_exact(3) {
                let [Node::Keyword(0x37),Node::Str(name),Node::Block(fields)]=row else {return Err("invalid CDB effect event".into());};
                events.push(Event {name:name.clone(),start:u32::try_from(named_records::integer(fields,0x2b)?).map_err(|_|"negative CDB event start")?,duration:u32::try_from(named_records::integer(fields,0x2c)?).map_err(|_|"negative CDB event duration")?});
            }if body.len()%3!=0 {return Err("incomplete CDB effect event".into());}}
            out.push(Self {name,worlds:worlds.clone(),fps,duration,cameras,objects,events});
        }
        Ok(out)
    }
}
