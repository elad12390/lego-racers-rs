//! Original BVB compressed plane topology (GolDP1001bd00, keyword8e).
//! The two child links precede three signed2^-30normal components and a range.
use crate::{tok::{self,Node,Value},world::{self,CollisionMesh}};

#[derive(Clone,Debug)]
pub struct PlaneNode {pub normal:[f32;3],pub first:u16,pub count:u16,pub children:[u16;2]}
pub struct CollisionTree {pub mesh:CollisionMesh,pub planes:Vec<PlaneNode>,pub depth:usize}

pub fn parse(data:&[u8])->Result<CollisionTree,String> {
    let mesh=world::collision_mesh(data).map_err(|e|e.to_string())?;
    let nodes=tok::parse(data).map_err(|e|e.to_string())?;
    let index=nodes.iter().position(|n|*n==Node::Keyword(0x8e)).ok_or("BVB compressed plane tree missing")?;
    let Some(Node::Count(count))=nodes.get(index+1) else {return Err("BVB plane count missing".into());};
    let Some(Node::Block(body))=nodes.get(index+2) else {return Err("BVB plane block missing".into());};
    let integer=|v:&Value|match *v {Value::U8(v)=>Ok(i32::from(v)),Value::I8(v)=>Ok(i32::from(v)),Value::U16(v)=>Ok(i32::from(v)),Value::I16(v)=>Ok(i32::from(v)),Value::I32(v)=>Ok(v),Value::F32(_)=>Err("BVB plane integer expected".to_string())};
    let mut values=Vec::new();
    for node in body {
        match node {
            Node::Int(v)=>values.push(*v),
            Node::Record {fields,..}=>for value in fields {values.push(integer(value)?);},
            Node::Packed {rows,..}=>for value in rows.iter().flatten() {values.push(integer(value)?);},
            _=>return Err("unexpected compressed BVB plane field".into()),
        }
    }
    if *count==0 || *count>=0xfffe || values.len()!=*count as usize*7 {return Err("BVB compressed plane count mismatch".into());}
    let mut planes=Vec::new();
    for row in values.chunks_exact(7) {
        let child=|v:i32|match v {-1=>Ok(0xffff),-2=>Ok(0xfffe),v if v>=0 && v<*count as i32=>Ok(v as u16),_=>Err("BVB plane child out of range".to_string())};
        let first=u16::try_from(row[5]).map_err(|_|"BVB plane triangle range invalid")?;
        let count=u16::try_from(row[6]).map_err(|_|"BVB plane triangle count invalid")?;
        if usize::from(first)>=mesh.triangles.len() || usize::from(first)+usize::from(count)>mesh.triangles.len() {return Err("BVB plane triangle range out of bounds".into());}
        // FILD integer * exact GolDP10056c38 (2^-30), then FSTP binary32.
        let normal=std::array::from_fn(|i|(f64::from(row[i+2])/1073741824.0) as f32);
        planes.push(PlaneNode {normal,first,count,children:[child(row[0])?,child(row[1])?]});
    }
    let mut active=vec![false;planes.len()];let mut visited=vec![false;planes.len()];let mut depth=0;
    let mut stack=vec![(0usize,1usize,false)];
    while let Some((index,level,exit))=stack.pop() {
        if exit {active[index]=false;visited[index]=true;continue;}
        if active[index] {return Err("BVB plane topology cycle".into());}
        depth=depth.max(level);
        if visited[index] {continue;}
        active[index]=true;stack.push((index,level,true));
        for child in planes[index].children {if child<0xfffe {stack.push((usize::from(child),level+1,false));}}
    }
    if visited.iter().any(|v|!*v) {return Err("BVB unreachable plane record".into());}
    Ok(CollisionTree {mesh,planes,depth})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_original_checkpoint_and_track_colliders_have_valid_compressed_trees() {
        let library=crate::library::Library::open(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extracted/Program_Files_Group/LEGO.JAM")).unwrap();
        let mut meshes=0;
        for table in library.jam().tables.iter().filter(|t|t.name.starts_with("RACEC")) {
            for entry in table.entries.iter().filter(|e|e.name.ends_with(".BVB")) {
                let tree=parse(library.jam().bytes(entry).unwrap()).unwrap_or_else(|e|panic!("{}/{}:{e}",table.name,entry.name));
                assert!(tree.depth>0);meshes+=1;
            }
        }
        assert_eq!(meshes,43);
    }
}
