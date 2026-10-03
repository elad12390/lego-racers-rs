//! Converts decoded original brick polygons to the existing GPU model format.
use lrformats::{
  bmp,
  gdb::{Mesh, Part, Vertex},
  library::Library,
  mdb,
  model::{Model, Surface},
};
use lrsim::brick_build::{rotated_point, Build, BuilderData, PlacedBrick};
use std::collections::HashMap;

pub fn model(library: &Library, data: &BuilderData, build: &Build) -> Result<Model, String> {
  data.validate(build)?;
  geometry(
    library,
    data,
    data.origin(build)?,
    std::iter::once(PlacedBrick {
      name: build.chassis.clone(),
      color: build.color.clone(),
      x: 0,
      y: 0,
      z: 0,
      rotation: 0,
    })
    .chain(build.bricks.iter().cloned()),
  )
}
/// Candidate geometry is deliberately independent of attachment validity, so
/// an invalid location remains visible rather than hiding the piece being moved.
pub fn candidate(
  library: &Library,
  data: &BuilderData,
  build: &Build,
  placed: &PlacedBrick,
) -> Result<Model, String> {
  geometry(
    library,
    data,
    data.origin(build)?,
    std::iter::once(placed.clone()),
  )
}
fn geometry(
  library: &Library,
  data: &BuilderData,
  origin: [f32; 3],
  placed: impl Iterator<Item = PlacedBrick>,
) -> Result<Model, String> {
  let materials = mdb::parse(
    library
      .find_at(&data.rules.materials, "MENUDATA", "PIECEDB")
      .ok_or("missing original brick materials")?,
  )
  .map_err(|e| e.to_string())?;
  let mut mesh = Mesh {
    textures: Vec::new(),
    scale: data.rules.geometry_scale,
    vertices: Vec::new(),
    normals: Vec::new(),
    triangles: Vec::new(),
    parts: Vec::new(),
  };
  let mut surfaces = Vec::new();
  let mut images = HashMap::new();
  let mut parts = Vec::new();
  for placed in placed {
    let brick = data.database.find(&placed.name)?;
    // Stud caps are generated from the original Cylinder entry, not new
    // primitive geometry. Original grid flags select stud-bearing cells.
    for x in 0..brick.width {
      for y in 0..brick.depth {
        let [top, _] = brick.cells[x as usize * brick.depth as usize + y as usize];
        if top & 0x80 != 0 {
          let point = rotated_point(brick, [x as f32, y as f32, 0.0], placed.rotation);
          let offset = match placed.rotation {
            0 => [0.0, 0.0],
            1 => [0.0, -1.0],
            2 => [-1.0, -1.0],
            _ => [-1.0, 0.0],
          };
          parts.push(PlacedBrick {
            name: "Cylinder".into(),
            color: placed.color.clone(),
            x: placed
              .x
              .saturating_add((point[0] + offset[0]).max(0.0) as u8),
            y: placed
              .y
              .saturating_add((point[1] + offset[1]).max(0.0) as u8),
            z: placed.z + (top & 63),
            rotation: 0,
          });
        }
      }
    }
    parts.push(placed);
  }
  for placed in parts {
    let brick = data.database.find(&placed.name)?;
    for face in &brick.faces {
      let color_material = materials
        .iter()
        .find(|m| m.name.eq_ignore_ascii_case(&placed.color))
        .ok_or_else(|| format!("original color {} not found", placed.color))?;
      let material = if face.material < 3 {
        color_material
      } else {
        materials
          .get(face.material as usize)
          .unwrap_or(color_material)
      };
      let texture = material.texture.clone();
      if let Some(name) = &texture {
        if !images.contains_key(name) {
          let bytes = library
            .find_at(&format!("{name}.BMP"), "MENUDATA", "PIECEDB")
            .or_else(|| library.find_in(&format!("{name}.BMP"), "COMMON"))
            .ok_or_else(|| format!("missing original brick texture {name}"))?;
          images.insert(name.clone(), bmp::decode(bytes).map_err(|e| e.to_string())?);
        }
      }
      let vertex_start = mesh.vertices.len() as u32;
      let triangle_start = mesh.triangles.len() as u32;
      for ((point, uv), normal) in face.points.iter().zip(&face.uvs).zip(&face.normals) {
        let point = rotated_point(brick, *point, placed.rotation);
        let normal = match placed.rotation {
          0 => *normal,
          1 => [normal[1], -normal[0], normal[2]],
          2 => [-normal[0], -normal[1], normal[2]],
          _ => [-normal[1], normal[0], normal[2]],
        };
        let base = if texture.is_some() {
          material.base_color()
        } else {
          [255; 4]
        };
        // Retain real original polygon normals, not a garage light baked into
        // geometry that is also used by racing and the saved ghost.
        mesh.normals.push(normal);
        mesh.vertices.push(Vertex {
          position: [
            point[0] + placed.x as f32 - origin[0],
            point[1] + placed.y as f32 - origin[1],
            (point[2] + placed.z as f32 - origin[2]) * data.rules.vertical_step,
          ],
          uv: *uv,
          rgba: base,
        });
      }
      // Original packed quads order0,1,2,3 with diagonal0-3 equivalence:
      // vertex3 is the implied fourth corner following the first triangle.
      let local = if face.points.len() == 4 {
        vec![[0, 1, 2], [1, 3, 2]]
      } else {
        vec![[0, 1, 2]]
      };
      mesh.triangles.extend(&local);
      let absolute: Vec<_> = local
        .iter()
        .map(|t| t.map(|i| vertex_start + i as u32))
        .collect();
      mesh.parts.push(Part {
        texture: 0,
        joint: 0,
        flag: 0,
        vertices: vertex_start..mesh.vertices.len() as u32,
        triangles: triangle_start..mesh.triangles.len() as u32,
      });
      let joints = vec![[0; 3]; absolute.len()];
      surfaces.push(Surface {
        material: Some(material.name.clone()),
        color: material.base_color(),
        texture,
        triangles: absolute,
        joints,
        color_key: material.color_key,
        blend: material.blend,
      });
    }
  }
  Ok(Model {
    mesh,
    surfaces,
    images,
  })
}
