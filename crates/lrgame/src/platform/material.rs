//! Original unlit vertex/texture multiplication and authored blending on wgpu.
use super::types;
use bevy::{
  mesh::MeshVertexBufferLayoutRef,
  pbr::{MaterialPipeline, MaterialPipelineKey},
  prelude::*,
  reflect::TypePath,
  render::render_resource::*,
  shader::ShaderRef,
};
pub const SHADER_PATH: &str = "shaders/original_surface.wgsl";
pub const SHADER: Handle<bevy::shader::Shader> =
  bevy::asset::uuid_handle!("e9716878-5b47-4ac9-9a0f-e2d7ee13aa32");
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
#[bind_group_data(SurfaceKey)]
pub struct OriginalSurface {
  #[texture(0)]
  #[sampler(1)]
  pub texture: Option<Handle<Image>>,
  pub settings: types::Material,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SurfaceKey(types::Material);
impl From<&OriginalSurface> for SurfaceKey {
  fn from(m: &OriginalSurface) -> Self {
    Self(m.settings)
  }
}
fn factor(n: u8) -> BlendFactor {
  match n {
    0 => BlendFactor::Zero,
    1 => BlendFactor::One,
    2 => BlendFactor::Src,
    3 => BlendFactor::Dst,
    4 => BlendFactor::OneMinusSrc,
    5 => BlendFactor::OneMinusDst,
    6 => BlendFactor::SrcAlpha,
    7 => BlendFactor::DstAlpha,
    8 => BlendFactor::OneMinusSrcAlpha,
    9 => BlendFactor::OneMinusDstAlpha,
    _ => BlendFactor::SrcAlphaSaturated,
  }
}
fn blending(settings: types::Material) -> Option<BlendState> {
  settings.blend.map(|[src, dst]| {
    let component = BlendComponent {
      src_factor: factor(src),
      dst_factor: factor(dst),
      operation: BlendOperation::Add,
    };
    BlendState {
      color: component,
      alpha: component,
    }
  })
}
impl Material for OriginalSurface {
  fn fragment_shader() -> ShaderRef {
    SHADER.clone().into()
  }
  fn alpha_mode(&self) -> AlphaMode {
    // Opaque/cutout geometry belongs in the depth-tested mask phase, not the
    // per-view translucent sorting list. The shader keeps its original key cut.
    if self.settings.blend.is_none() && self.settings.depth_write {
      AlphaMode::Mask(0.001)
    } else {
      AlphaMode::Blend
    }
  }
  fn specialize(
    _: &MaterialPipeline,
    descriptor: &mut RenderPipelineDescriptor,
    _: &MeshVertexBufferLayoutRef,
    key: MaterialPipelineKey<Self>,
  ) -> Result<(), SpecializedMeshPipelineError> {
    let m = key.bind_group_data.0;
    descriptor.primitive.cull_mode = m.cull.then_some(Face::Back);
    descriptor.primitive.front_face = FrontFace::Ccw;
    if let Some(depth) = &mut descriptor.depth_stencil {
      depth.depth_write_enabled = Some(m.depth_write);
      depth.depth_compare = Some(if m.depth_test {
        CompareFunction::GreaterEqual
      } else {
        CompareFunction::Always
      });
    }
    if let Some(fragment) = &mut descriptor.fragment {
      for target in fragment.targets.iter_mut().flatten() {
        target.blend = blending(m);
      }
    }
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn bevy_boundary_maps_all_original_blend_factors_and_depth_modes() {
    let factors = [
      BlendFactor::Zero,
      BlendFactor::One,
      BlendFactor::Src,
      BlendFactor::Dst,
      BlendFactor::OneMinusSrc,
      BlendFactor::OneMinusDst,
      BlendFactor::SrcAlpha,
      BlendFactor::DstAlpha,
      BlendFactor::OneMinusSrcAlpha,
      BlendFactor::OneMinusDstAlpha,
      BlendFactor::SrcAlphaSaturated,
    ];
    for (i, expected) in factors.into_iter().enumerate() {
      assert_eq!(factor(i as u8), expected);
    }
    assert!(types::Material::scene(None).depth_write);
    assert!(!types::Material::scene(Some([1, 1])).depth_write);
    assert!(!types::Material::sky().depth_test);
    assert!(!types::Material::overlay().cull);
    assert!(blending(types::Material::scene(None)).is_none());
    assert_eq!(
      OriginalSurface {
        texture: None,
        settings: types::Material::scene(None)
      }
      .alpha_mode(),
      AlphaMode::Mask(0.001)
    );
    assert_eq!(
      OriginalSurface {
        texture: None,
        settings: types::Material::scene(Some([1, 1]))
      }
      .alpha_mode(),
      AlphaMode::Blend
    );
    assert_eq!(
      OriginalSurface {
        texture: None,
        settings: types::Material::sky()
      }
      .alpha_mode(),
      AlphaMode::Blend
    );
    assert_eq!(
      blending(types::Material::scene(Some([1, 1])))
        .unwrap()
        .color
        .src_factor,
      BlendFactor::One
    );
  }
}
