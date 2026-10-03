//! Decoded original external music, shared by menu and race playback.
use crate::platform::audio::{self, Sound};
use std::path::Path;
pub async fn tune(directory: &Path, name: &str) -> Result<Sound, String> {
  if name.contains(['/', '\\']) || !name.to_ascii_lowercase().ends_with(".tun") {
    return Err("invalid original tune name".into());
  }
  let entries = std::fs::read_dir(directory)
    .map_err(|e| e.to_string())?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| e.to_string())?;
  let mut matches = entries
    .iter()
    .filter(|entry| {
      entry
        .file_name()
        .to_string_lossy()
        .eq_ignore_ascii_case(name)
    })
    .map(|entry| entry.path());
  let path = matches
    .next()
    .ok_or_else(|| format!("missing original tune {name}"))?;
  if matches.next().is_some() {
    return Err(format!("ambiguous original tune {name}"));
  }
  let decoded = lrformats::tun::decode(&std::fs::read(&path).map_err(|e| e.to_string())?)
    .map_err(|e| format!("{name}: {e}"))?;
  if decoded.sample_rate == 0 || decoded.samples.is_empty() {
    return Err(format!("empty original tune {name}"));
  }
  audio::load_sound_from_bytes(&lrformats::wav::encode_16(
    decoded.sample_rate,
    decoded.channels,
    &decoded.samples,
  ))
  .await
}
