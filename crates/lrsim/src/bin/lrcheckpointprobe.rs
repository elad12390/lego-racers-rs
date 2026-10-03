//! Actual checkpoint fixture points, original placement and native tree dispatch.
use lrformats::library::Library;
use lrsim::checkpoint_contacts::CheckpointContacts;
fn main() -> Result<(), Box<dyn std::error::Error>> {
  let args: Vec<_> = std::env::args().skip(1).collect();
  let [jam] = args.as_slice() else {
    return Err("usage: lrcheckpointprobe LEGO.JAM".into());
  };
  let library = Library::open(jam)?;
  let mut output = Vec::new();
  for owner in library
    .jam()
    .tables
    .iter()
    .filter(|t| t.name.starts_with("RACEC"))
  {
    let checkpoints = CheckpointContacts::load(&library, &owner.name)?;
    let records: Vec<_> = checkpoints
      .table
      .records
      .iter()
      .map(|c| serde_json::json!({"plane":c.plane,"progress":c.progress}))
      .collect();
    let collider = checkpoints.query_collider();
    let wheel_materials:Vec<_>=collider.tree.mesh.names.iter().enumerate().filter(|(_,name)|name.as_bytes().first().is_some_and(u8::is_ascii_digit)).map(|(i,_)| {
            let surface=checkpoints.wheel_surface(i as u32);
            serde_json::json!({"flags":surface.flags,"friction":surface.slope_friction,"drag":surface.quadratic_drag,"force":surface.force})
        }).collect();
    output.push(serde_json::json!({"table":owner.name,"collider":checkpoints.collider,"transform":checkpoints.transform,"records":records,"wheel_materials":wheel_materials,"fixtures":checkpoints.probe_fixtures()?}));
  }
  println!("{}", serde_json::to_string(&output)?);
  Ok(())
}
