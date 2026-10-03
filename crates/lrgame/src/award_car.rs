//! Static original champion car assembly for original unlock scene substitution.
use crate::platform::prelude::*;
use lrformats::{appearance::Car, cmb::Chassis, library::Library, model::Model};
pub fn load(library: &Library, car: &Car, chassis: &Chassis) -> Result<Model, String> {
  let mut model =
    Model::load(library, &car.models[0], Some("COMMON")).map_err(|e| e.to_string())?;
  let wheel = &chassis
    .wheel_models
    .first()
    .ok_or("unlock champion has no wheel model")?
    .1;
  let wheels = Model::load(library, wheel, Some("COMMON")).map_err(|e| e.to_string())?;
  let rig = crate::skeleton::Skeleton::load(
    library
      .find_in(&format!("{wheel}.SDB"), "COMMON")
      .ok_or("missing unlock wheel rig")?,
    wheels.mesh.scale,
  )?
  .pose(None)?;
  let preserve_normals = !model.mesh.normals.is_empty();
  // Resolve per-corner bindings before flattening. Original wheel rotations
  // and axle positions belong to SDB; never fabricate tire positions.
  for surface in wheels.surfaces {
    let mut triangles = Vec::new();
    for (triangle, joints) in surface.triangles.iter().zip(&surface.joints) {
      let mut indices = [0; 3];
      for corner in 0..3 {
        let vertex = wheels.mesh.vertices[triangle[corner] as usize];
        let joint = rig
          .get(joints[corner] as usize)
          .ok_or("unlock wheel joint outside original rig")?;
        let position = joint
          .transform_point3(Vec3::from_array(vertex.position) * wheels.mesh.scale)
          / model.mesh.scale;
        indices[corner] = model.mesh.vertices.len() as u32;
        model.mesh.vertices.push(lrformats::gdb::Vertex {
          position: position.to_array(),
          ..vertex
        });
        if preserve_normals {
          let normal = wheels
            .mesh
            .normals
            .get(triangle[corner] as usize)
            .copied()
            .unwrap_or([0.0; 3]);
          model
            .mesh
            .normals
            .push(joint.transform_vector3(Vec3::from_array(normal)).to_array());
        }
      }
      triangles.push(indices);
    }
    let joints = vec![[0; 3]; triangles.len()];
    model.surfaces.push(lrformats::model::Surface {
      triangles,
      joints,
      ..surface
    });
  }
  model.images.extend(wheels.images);
  Ok(model)
}
