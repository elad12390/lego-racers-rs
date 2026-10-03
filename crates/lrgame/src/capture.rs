//! Capture reads the actual native GPU framebuffer, not the offline rasterizer.
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use crate::platform::Image;
use serde::Serialize;

/// Await the actual Bevy GPU framebuffer, not a previous frame or CPU image.
pub async fn screen_data() -> Image {
  crate::platform::screen_data().await
}

#[derive(Serialize)]
pub struct Report {
  pub mode: &'static str,
  pub native_frames_presented: u32,
  pub triangles: usize,
  pub textures: usize,
  pub frame_hash_first: u64,
  pub frame_hash_last: u64,
  pub non_background_pixels_first: usize,
  pub non_background_pixels_last: usize,
  pub physical_keyboard_verified: bool,
  pub rendering_discrepancies: Vec<&'static str>,
}

pub fn save_frame(path: &Path, image: &Image) -> Result<(u64, usize), String> {
  let file = File::create(path).map_err(|error| format!("{}: {error}", path.display()))?;
  let mut encoder = png::Encoder::new(
    BufWriter::new(file),
    u32::from(image.width),
    u32::from(image.height),
  );
  encoder.set_color(png::ColorType::Rgba);
  encoder.set_depth(png::BitDepth::Eight);
  // Native OpenGL readback is bottom-up, matching Macroquad's export_png
  // implementation. Normalize only PNG row order, not rendered game content.
  let stride = usize::from(image.width) * 4;
  let top_down: Vec<u8> = image
    .bytes
    .chunks_exact(stride)
    .rev()
    .flatten()
    .copied()
    .collect();
  encoder
    .write_header()
    .and_then(|mut writer| writer.write_image_data(&top_down))
    .map_err(|error| error.to_string())?;
  let hash = image
    .bytes
    .iter()
    .fold(0xcbf29ce484222325u64, |hash, byte| {
      (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    });
  let populated = image
    .bytes
    .chunks_exact(4)
    .filter(|pixel| pixel[..3] != [24, 31, 45])
    .count();
  Ok((hash, populated))
}

pub fn save_report(path: &Path, report: &Report) -> Result<(), String> {
  let file = File::create(path).map_err(|error| error.to_string())?;
  serde_json::to_writer_pretty(file, report).map_err(|error| error.to_string())
}
