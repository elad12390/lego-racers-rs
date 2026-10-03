//! Static original WDB cameras, expressed in the runtime's right-handed basis.
use crate::platform::prelude::*;
use crate::{gpu::world_position, menu_ui::MenuUi};
use lrformats::tok::{self, Node};

pub struct MenuCamera {
  pub position: Vec3,
  pub forward: Vec3,
  pub up: Vec3,
  pub fov: f32,
  pub near: f32,
  pub far: f32,
}

#[cfg(test)]
mod tests {
  use super::*;
  use lrformats::library::Library;

  #[test]
  fn original_license_scene_camera_preserves_authored_projection_and_gallery_loading() {
    let library = Library::open(
      std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../extracted/Program_Files_Group/LEGO.JAM"),
    )
    .unwrap();
    let camera =
      MenuCamera::load(library.find_at("CAM.WDB", "MENUDATA", "MENUDATA").unwrap()).unwrap();
    assert!((camera.position - vec3(2.281, 2.558, 3.632)).length() < 0.001);
    assert!((camera.fov.to_degrees() - 17.460205).abs() < 0.001);
    assert_eq!((camera.near, camera.far), (5.0, 800.0));
    let gallery =
      MenuCamera::load(library.find_at("RACER.WDB", "MENUDATA", "RS_SET").unwrap()).unwrap();
    assert!(gallery.fov > 0.0 && gallery.near < gallery.far);
  }
}
impl MenuCamera {
  pub fn load(bytes: &[u8]) -> Result<Self, String> {
    let nodes = tok::parse(bytes).map_err(|e| e.to_string())?;
    let list = nodes
      .windows(3)
      .find_map(|p| match p {
        [Node::Keyword(0x43), Node::Count(_), Node::Block(b)] => Some(b),
        _ => None,
      })
      .ok_or("original menu camera list missing")?;
    let fields = list
      .windows(3)
      .find_map(|p| match p {
        [Node::Keyword(0x43), Node::Str(_), Node::Block(b)] => Some(b),
        _ => None,
      })
      .ok_or("original menu camera missing")?;
    let record = |kind| {
      fields
        .iter()
        .find_map(|n| match n {
          Node::Record { kind: k, fields } if *k == kind => Some(fields),
          _ => None,
        })
        .ok_or("menu camera transform missing")
    };
    let position = record(0x17)?
      .iter()
      .map(|v| v.as_f32().ok_or("invalid camera position"))
      .collect::<Result<Vec<_>, _>>()?;
    let axes = record(0x18)?
      .iter()
      .map(|v| v.as_f32().ok_or("invalid camera axes"))
      .collect::<Result<Vec<_>, _>>()?;
    if position.len() != 3 || axes.len() != 6 {
      return Err("invalid menu camera transform dimensions".into());
    }
    let fov = match lrformats::named_records::value(fields, 0x47) {
      Some(Node::Float(v)) if v.is_finite() && *v > 0.0 && *v < 180.0 => v.to_radians(),
      _ => return Err("original menu camera FOV missing".into()),
    };
    let depth = |kind| match lrformats::named_records::value(fields, kind) {
      Some(Node::Float(v)) if v.is_finite() && *v > 0.0 => Ok(*v),
      _ => Err("original menu camera clip distance missing"),
    };
    let near = depth(0x45)?;
    let far = depth(0x46)?;
    if near >= far {
      return Err("original menu camera clip distances reversed".into());
    }
    Ok(Self {
      position: world_position(position.try_into().unwrap(), 1.0),
      forward: world_position(axes[..3].try_into().unwrap(), 1.0),
      up: world_position(axes[3..].try_into().unwrap(), 1.0),
      fov,
      near,
      far,
    })
  }
  pub fn draw(&self, rect: Rect, offset: Vec3, orbit: f32) {
    // Existing gallery previews retain their provisional near plane; changing
    // unrelated gallery geometry/clipping is outside the license slice.
    self.draw_with_depth(rect, offset, orbit, 0.05, 800.0);
  }
  pub fn draw_original(&self, rect: Rect, offset: Vec3, orbit: f32) {
    self.draw_with_depth(rect, offset, orbit, self.near, self.far);
  }
  fn draw_with_depth(&self, rect: Rect, offset: Vec3, orbit: f32, near: f32, far: f32) {
    let rotation = Mat4::from_rotation_y(orbit);
    let position = rotation.transform_point3(self.position - offset);
    let forward = rotation.transform_vector3(self.forward);
    let up = rotation.transform_vector3(self.up);
    let s = MenuUi::scale();
    let o = MenuUi::origin();
    set_camera(&Camera3D {
      position,
      target: position + forward,
      up,
      aspect: Some(rect.w / rect.h),
      fovy: self.fov,
      z_near: near,
      z_far: far,
      viewport: Some((
        (o.x + rect.x * s) as i32,
        (screen_height() - o.y - (rect.y + rect.h) * s) as i32,
        (rect.w * s) as i32,
        (rect.h * s) as i32,
      )),
      ..Default::default()
    });
  }
}
