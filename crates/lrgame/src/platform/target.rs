//! Render the game into a persistent GPU image, then present that exact image.
//! Readback must not depend on the macOS window surface being drawable/visible.
use bevy::{
  asset::RenderAssetUsages,
  camera::{visibility::RenderLayers, RenderTarget},
  prelude::*,
  render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
};
const PRESENT_LAYER: usize = 63;
#[derive(Resource, Default)]
pub struct Target {
  pub image: Option<Handle<Image>>,
  size: UVec2,
  presenter: Option<Entity>,
}
pub fn ensure(world: &mut World, width: f32, height: f32, scale: f32) -> Handle<Image> {
  world.resource_scope(|world, mut target: Mut<Target>| {
    let size = UVec2::new(
      (width * scale).round().max(1.0) as u32,
      (height * scale).round().max(1.0) as u32,
    );
    let resized = target.size != size || target.image.is_none();
    if resized {
      let mut image = Image::new_fill(
        Extent3d {
          width: size.x,
          height: size.y,
          depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
      );
      image.texture_descriptor.usage = TextureUsages::RENDER_ATTACHMENT
        | TextureUsages::TEXTURE_BINDING
        | TextureUsages::COPY_SRC
        | TextureUsages::COPY_DST;
      if let Some(handle) = &target.image {
        *world
          .resource_mut::<Assets<Image>>()
          .get_mut(handle)
          .unwrap() = image;
      } else {
        target.image = Some(world.resource_mut::<Assets<Image>>().add(image));
      }
      target.size = size;
    }
    let image = target.image.clone().unwrap();
    let sprite = Sprite {
      image: image.clone(),
      custom_size: Some(Vec2::new(width, height)),
      ..default()
    };
    if let Some(entity) = target.presenter {
      if resized {
        world.entity_mut(entity).insert(sprite);
      }
    } else {
      target.presenter = Some(
        world
          .spawn((
            sprite,
            Transform::default(),
            RenderLayers::layer(PRESENT_LAYER),
          ))
          .id(),
      );
      world.spawn((
        Camera2d,
        Camera {
          order: 1000,
          ..default()
        },
        RenderLayers::layer(PRESENT_LAYER),
        Msaa::Off,
      ));
    }
    image
  })
}
pub fn camera_target(image: &Handle<Image>) -> RenderTarget {
  RenderTarget::Image(image.clone().into())
}
