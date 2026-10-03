//! Asynchronous readback of the actual Bevy GPU image presented to the window.
//! Hold the submitted scene until completion, independently of window occlusion.
use super::{renderer::Renderer, state, types};
use bevy::{
  asset::UntypedAssetId,
  pbr::PreparedMaterial,
  prelude::*,
  render::{
    erased_render_asset::ErasedRenderAssets,
    extract_resource::{ExtractResource, ExtractResourcePlugin},
    mesh::RenderMesh,
    render_asset::RenderAssets,
    render_resource::PipelineCache,
    texture::GpuImage,
    view::screenshot::{Screenshot, ScreenshotCaptured},
    Render, RenderApp, RenderSystems,
  },
};
use std::sync::{
  atomic::{AtomicU64, Ordering},
  Arc,
};
#[derive(Resource, Default)]
pub struct Readback {
  serial: u64,
  pending: Option<u64>,
}
#[derive(Resource, Clone, Default, ExtractResource)]
struct Readiness {
  serial: u64,
  meshes: Vec<AssetId<Mesh>>,
  images: Vec<AssetId<Image>>,
  materials: Vec<UntypedAssetId>,
  completed: Arc<AtomicU64>,
}
pub fn install(app: &mut App) {
  app
    .init_resource::<Readiness>()
    .add_plugins(ExtractResourcePlugin::<Readiness>::default());
  app
    .sub_app_mut(RenderApp)
    .add_systems(Render, acknowledge.in_set(RenderSystems::PostCleanup));
}
fn acknowledge(
  request: Res<Readiness>,
  meshes: Res<RenderAssets<RenderMesh>>,
  images: Res<RenderAssets<GpuImage>>,
  materials: Res<ErasedRenderAssets<PreparedMaterial>>,
  pipelines: Res<PipelineCache>,
  mut consecutive: Local<(u64, u8)>,
) {
  if request.serial == 0 {
    return;
  }
  if consecutive.0 != request.serial {
    *consecutive = (request.serial, 0);
  }
  let ready = request.meshes.iter().all(|id| meshes.get(*id).is_some())
    && request.images.iter().all(|id| images.get(*id).is_some())
    && request
      .materials
      .iter()
      .all(|id| materials.get(*id).is_some())
    && pipelines.waiting_pipelines().next().is_none();
  // One ready pass may precede material queue/visibility propagation. Require
  // a second completed render with the same prepared submission, not a delay.
  consecutive.1 = if ready {
    consecutive.1.saturating_add(1)
  } else {
    0
  };
  if consecutive.1 >= 2 {
    request.completed.store(request.serial, Ordering::Release);
  }
}
pub fn flush(world: &mut World) {
  let capture = state::with(|s| {
    if s.capture_requested && !s.capture_inflight {
      s.capture_requested = false;
      s.capture_inflight = true;
      true
    } else {
      false
    }
  });
  if capture {
    let serial = {
      let mut clock = world.resource_mut::<Readback>();
      clock.serial += 1;
      clock.pending = Some(clock.serial);
      clock.serial
    };
    let gpu = world.resource::<Renderer>();
    let meshes = gpu
      .slots
      .iter()
      .take(state::with(|s| s.batches.len()))
      .map(|(_, h)| h.id())
      .collect();
    let mut images: Vec<_> = gpu.textures.iter().map(Handle::id).collect();
    images.push(
      world
        .resource::<super::target::Target>()
        .image
        .as_ref()
        .unwrap()
        .id(),
    );
    let materials = gpu.materials.values().map(|h| h.id().untyped()).collect();
    let mut request = world.resource_mut::<Readiness>();
    request.serial = serial;
    request.meshes = meshes;
    request.images = images;
    request.materials = materials;
  }
  let completed = world
    .resource::<Readiness>()
    .completed
    .load(Ordering::Acquire);
  let ready = {
    let mut clock = world.resource_mut::<Readback>();
    if clock.pending == Some(completed) {
      clock.pending = None;
      true
    } else {
      false
    }
  };
  if ready {
    let cameras = world
      .query::<(&Camera, &bevy::camera::visibility::VisibleEntities)>()
      .iter(world)
      .filter(|(c, _)| c.is_active)
      .map(|(c, v)| {
        (
          c.order,
          c.physical_target_size(),
          v.entities.values().map(Vec::len).sum::<usize>(),
        )
      })
      .collect::<Vec<_>>();
    println!(
      "bevy readback submission={completed} cameras={cameras:?} batches={}",
      state::with(|s| s.batches.len())
    );
    let image = world
      .resource::<super::target::Target>()
      .image
      .clone()
      .unwrap();
    world
      .spawn(Screenshot::image(image))
      .observe(|event: On<ScreenshotCaptured>| {
        let rgba = event
          .image
          .clone()
          .try_into_dynamic()
          .expect("native screenshot format")
          .to_rgba8();
        let (w, h) = rgba.dimensions();
        let mut bytes = rgba.into_raw();
        let row = w as usize * 4;
        // Existing evidence writers consume bottom-up framebuffer bytes.
        for y in 0..h as usize / 2 {
          let split = (h as usize - 1 - y) * row;
          let (a, b) = bytes.split_at_mut(split);
          a[y * row..(y + 1) * row].swap_with_slice(&mut b[..row]);
        }
        state::with(|s| {
          s.readback = Some(types::Image {
            width: w as u16,
            height: h as u16,
            bytes,
          });
          s.capture_inflight = false;
        });
      });
  }
}
