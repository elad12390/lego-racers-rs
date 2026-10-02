//! Original BVB tree fixtures and native full-tree segment-query adapter.
use std::io::{self,Read};
use serde::Deserialize;
use lrformats::{library::Library,collision_tree};
#[derive(Deserialize)]
struct Case {jam:String,table:String,mesh:String,#[serde(default)]resolve_primary:bool,#[serde(default)]segments:Vec<[[f32;3];2]>,#[serde(default)]containments:Vec<([f32;3],usize,usize)>,#[serde(default)]observe:bool}
fn main()->Result<(),Box<dyn std::error::Error>> {
    let mut input=String::new();io::stdin().read_to_string(&mut input)?;
    let cases:Vec<Case>=serde_json::from_str(&input)?;let mut output=Vec::new();
    for case in cases {
        let library=Library::open(&case.jam)?;
        let mesh=if case.resolve_primary {
            let binding=lrformats::race_archive::collisions(library.find_in(&format!("{}.RAB",case.table),&case.table).ok_or("original RAB missing")?)?;
            format!("{}.BVB",binding.primary).to_ascii_uppercase()
        } else {case.mesh};
        let data=library.find_in(&mesh,&case.table).ok_or("original collider missing")?;
        let tree=collision_tree::parse(data)?;
        let planes:Vec<_>=tree.planes.iter().map(|p|serde_json::json!({"normal":p.normal,"first":p.first,"count":p.count,"children":p.children})).collect();
        let triangles:Vec<_>=tree.mesh.triangles.iter().map(|t|serde_json::json!({"indices":t.indices,"surface":t.surface})).collect();
        let mut traversals=Vec::new();
        let hits:Vec<_>=case.segments.iter().map(|[a,b]| {
            let mut steps=Vec::new();
            let hit=lrsim::mesh_query::trace_observed(&tree,*a,*b,|node,phase,distance|if case.observe {steps.push((node,phase,distance));});
            if case.observe {traversals.push(steps);}hit
        }).collect();
        let containments:Vec<_>=case.containments.iter().map(|(point,plane,triangle)|lrsim::mesh_query::contains(*point,tree.mesh.triangles[*triangle].indices.map(|i|tree.mesh.vertices[i as usize]),tree.planes[*plane].normal)).collect();
        output.push(serde_json::json!({"mesh":mesh,"planes":planes,"triangles":triangles,"vertices":tree.mesh.vertices,"names":tree.mesh.names,"depth":tree.depth,"hits":hits,"containments":containments,"traversals":traversals}));
    }
    println!("{}",serde_json::to_string(&output)?);Ok(())
}
