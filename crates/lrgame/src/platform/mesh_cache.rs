//! Exact geometry reuse: moving cameras do not require re-uploading static roads.
use super::types;
use bevy::{
  asset::RenderAssetUsages,
  mesh::{Indices, PrimitiveTopology},
  prelude::*,
};

#[derive(Default)]
pub(super) struct Geometry {
  vertices: Vec<types::Vertex>,
  indices: Vec<u16>,
  overlay: Option<(f32, usize)>,
  initialized: bool,
}
impl Geometry {
  pub fn update(&mut self, source: &types::Mesh, overlay: Option<(f32, usize)>) -> Option<Mesh> {
    if self.initialized
      && self.overlay == overlay
      && self.vertices == source.vertices
      && self.indices == source.indices
    {
      return None;
    }
    self.vertices.clone_from(&source.vertices);
    self.indices.clone_from(&source.indices);
    self.overlay = overlay;
    self.initialized = true;
    let positions = self
      .vertices
      .iter()
      .map(|v| {
        let mut p = v.position;
        if let Some((height, order)) = overlay {
          p.y = height - p.y;
          p.z = order as f32 * 0.0001;
        }
        p.to_array()
      })
      .collect::<Vec<_>>();
    let mut mesh = Mesh::new(
      PrimitiveTopology::TriangleList,
      RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(
      Mesh::ATTRIBUTE_NORMAL,
      vec![[0.0, 1.0, 0.0]; self.vertices.len()],
    );
    mesh.insert_attribute(
      Mesh::ATTRIBUTE_UV_0,
      self
        .vertices
        .iter()
        .map(|v| v.uv.to_array())
        .collect::<Vec<_>>(),
    );
    mesh.insert_attribute(
      Mesh::ATTRIBUTE_COLOR,
      self
        .vertices
        .iter()
        .map(|v| v.color.map(|c| c as f32 / 255.0))
        .collect::<Vec<_>>(),
    );
    mesh.insert_indices(Indices::U16(self.indices.clone()));
    Some(mesh)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn bevy_boundary_static_geometry_reuses_gpu_asset_but_animation_uv_and_topology_refresh() {
    let mut cache = Geometry::default();
    let mut mesh = types::Mesh {
      vertices: vec![types::Vertex::new(1.0, 2.0, 3.0, 0.0, 0.0, types::WHITE); 3],
      indices: vec![0, 1, 2],
      texture: None,
    };
    assert!(cache.update(&mesh, None).is_some());
    assert!(cache.update(&mesh, None).is_none());
    mesh.vertices[0].position.x += 1.0;
    assert!(cache.update(&mesh, None).is_some());
    mesh.vertices[0].uv.x += 0.25;
    assert!(cache.update(&mesh, None).is_some());
    mesh.vertices[0].color[0] = 0;
    assert!(cache.update(&mesh, None).is_some());
    mesh.indices.swap(1, 2);
    assert!(cache.update(&mesh, None).is_some());
    assert!(cache.update(&mesh, Some((760.0, 1))).is_some());
    assert!(cache.update(&mesh, Some((760.0, 1))).is_none());
    assert!(cache.update(&mesh, Some((800.0, 1))).is_some());
    assert!(cache.update(&mesh, Some((800.0, 2))).is_some());
  }
}
