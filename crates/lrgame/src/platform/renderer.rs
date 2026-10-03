//! Persistent Bevy mesh/material slots and ordered scene/overlay cameras.
use super::{material::OriginalSurface, state, types};
use bevy::{
  asset::RenderAssetUsages,
  camera::{visibility::RenderLayers, ScalingMode, Viewport},
  core_pipeline::tonemapping::Tonemapping,
  image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor},
  prelude::*,
  render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use std::collections::HashMap;
#[derive(Resource, Default)]
pub struct Renderer {
  pub(super) textures: Vec<Handle<Image>>,
  pub(super) slots: Vec<(Entity, Handle<Mesh>)>,
  geometry: Vec<super::mesh_cache::Geometry>,
  cameras: Vec<Entity>,
  pub(super) materials: HashMap<(Option<usize>, types::Material), Handle<OriginalSurface>>,
}
pub fn flush(world: &mut World) {
  world.resource_scope(|world, mut gpu: Mut<Renderer>| {
    upload_textures(world, &mut gpu);
    if state::with(|s| std::mem::take(&mut s.dirty)) {
      render(world, &mut gpu);
    }
  });
}
fn upload_textures(world: &mut World, gpu: &mut Renderer) {
  state::with(|s| {
    let mut images = world.resource_mut::<Assets<Image>>();
    for texture in s.textures.iter().skip(gpu.textures.len()) {
      let mut image = Image::new(
        Extent3d {
          width: texture.width as u32,
          height: texture.height as u32,
          depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        texture.bytes.clone(),
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
      );
      let filter = match texture.filter {
        types::FilterMode::Nearest => ImageFilterMode::Nearest,
        types::FilterMode::Linear => ImageFilterMode::Linear,
      };
      image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: filter,
        min_filter: filter,
        mipmap_filter: filter,
        ..default()
      });
      gpu.textures.push(images.add(image));
    }
  });
}
fn render(world: &mut World, gpu: &mut Renderer) {
  let (batches, background, width, height, scale) =
    state::with(|s| (s.batches.clone(), s.background, s.width, s.height, s.scale));
  let target = super::target::ensure(world, width, height, scale);
  let mut groups: Vec<(Option<types::Camera3D>, bool)> = vec![];
  for (i, batch) in batches.iter().enumerate() {
    let group = (batch.camera.clone(), batch.material.depth_test);
    if groups.last() != Some(&group) {
      groups.push(group);
    }
    let layer = groups.len() - 1;
    if i == gpu.geometry.len() {
      gpu.geometry.push(super::mesh_cache::Geometry::default());
    }
    let mesh = gpu.geometry[i].update(&batch.mesh, batch.camera.is_none().then_some((height, i)));
    let key = (batch.mesh.texture.as_ref().map(|t| t.id), batch.material);
    let material = gpu
      .materials
      .entry(key)
      .or_insert_with(|| {
        world
          .resource_mut::<Assets<OriginalSurface>>()
          .add(OriginalSurface {
            texture: key.0.map(|id| gpu.textures[id].clone()),
            settings: batch.material,
          })
      })
      .clone();
    if let Some((entity, handle)) = gpu.slots.get(i) {
      if let Some(mesh) = mesh {
        *world
          .resource_mut::<Assets<Mesh>>()
          .get_mut(handle)
          .unwrap() = mesh;
      }
      // Do not mark static render components changed merely because the game
      // submitted them again. Bevy can retain their extracted material/layer.
      if world
        .get::<MeshMaterial3d<OriginalSurface>>(*entity)
        .unwrap()
        .0
        != material
      {
        world.entity_mut(*entity).insert(MeshMaterial3d(material));
      }
      let layers = RenderLayers::layer(layer);
      if world.get::<RenderLayers>(*entity) != Some(&layers) {
        world.entity_mut(*entity).insert(layers);
      }
      if world.get::<Visibility>(*entity) != Some(&Visibility::Visible) {
        world.entity_mut(*entity).insert(Visibility::Visible);
      }
    } else {
      let handle = world
        .resource_mut::<Assets<Mesh>>()
        .add(mesh.expect("new slot geometry"));
      // Positions are CPU-transformed/animated every submission. Static bounds
      // and indirect batching were observed to produce blank moving race output.
      let entity = world
        .spawn((
          Mesh3d(handle.clone()),
          MeshMaterial3d(material),
          Transform::IDENTITY,
          RenderLayers::layer(layer),
          bevy::camera::visibility::NoFrustumCulling,
          bevy::render::batching::NoAutomaticBatching,
        ))
        .id();
      gpu.slots.push((entity, handle));
    }
  }
  for (entity, _) in gpu.slots.iter().skip(batches.len()) {
    if world.get::<Visibility>(*entity) != Some(&Visibility::Hidden) {
      world.entity_mut(*entity).insert(Visibility::Hidden);
    }
  }
  if groups.is_empty() {
    groups.push((None, false));
  }
  for (layer, (camera, _)) in groups.iter().enumerate() {
    assert!(layer < 63, "presentation camera layer reserved");
    let mut native_camera = Camera {
      order: layer as isize,
      clear_color: if layer == 0 {
        ClearColorConfig::Custom(bevy::color::Color::srgba(
          background.r,
          background.g,
          background.b,
          background.a,
        ))
      } else {
        ClearColorConfig::None
      },
      ..default()
    };
    let (projection, transform) = if let Some(c) = camera {
      if let Some((x, y, w, h)) = c.viewport {
        native_camera.viewport = Some(Viewport {
          physical_position: UVec2::new(
            (x.max(0) as f32 * scale).round() as u32,
            ((height - y as f32 - h as f32).max(0.0) * scale).round() as u32,
          ),
          physical_size: UVec2::new(
            (w as f32 * scale).round() as u32,
            (h as f32 * scale).round() as u32,
          ),
          ..default()
        });
      }
      (
        Projection::Perspective(PerspectiveProjection {
          fov: c.fovy,
          aspect_ratio: c.aspect.unwrap_or(width / height),
          near: c.z_near,
          far: c.z_far,
          ..default()
        }),
        Transform::from_translation(c.position).looking_at(c.target, c.up),
      )
    } else {
      (
        Projection::Orthographic(OrthographicProjection {
          scaling_mode: ScalingMode::Fixed { width, height },
          near: 0.0,
          far: 2000.0,
          ..OrthographicProjection::default_3d()
        }),
        Transform::from_xyz(width / 2.0, height / 2.0, 1000.0),
      )
    };
    if let Some(entity) = gpu.cameras.get(layer) {
      // Camera caches target size and projection internally. Replacing it
      // with Default each submitted frame loses those caches during readback.
      let mut camera = world.get_mut::<Camera>(*entity).unwrap();
      camera.order = native_camera.order;
      camera.clear_color = native_camera.clear_color;
      camera.viewport = native_camera.viewport;
      camera.is_active = true;
      world.entity_mut(*entity).insert((
        projection,
        transform,
        RenderLayers::layer(layer),
        super::target::camera_target(&target),
      ));
    } else {
      gpu.cameras.push(
        world
          .spawn((
            Camera3d::default(),
            native_camera,
            super::target::camera_target(&target),
            projection,
            transform,
            RenderLayers::layer(layer),
            Tonemapping::None,
            Msaa::Off,
            bevy::render::view::NoIndirectDrawing,
          ))
          .id(),
      );
    }
  }
  for entity in gpu.cameras.iter().skip(groups.len()) {
    world.get_mut::<Camera>(*entity).unwrap().is_active = false;
  }
}
