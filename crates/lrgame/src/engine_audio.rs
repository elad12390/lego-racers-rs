//! Original PCM clips on the existing native backend, no synthesized engine.
//! Discrete sample-rate pitch bank is a modern mixer adaptation, provisional.
use crate::platform::audio::{self, PlaySoundParams, Sound};
use lrformats::{library::Library, pcm, wav};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Rules {
  engine_file: String,
  idle_file: String,
  skid_file: String,
  pub effect_files: [[String; 4]; 4],
  pub pickup_file: String,
  pub white_files: [String; 3],
  engine_pitch_steps: usize,
  engine_pitch_min: f32,
  engine_pitch_max: f32,
  engine_gain: f32,
  idle_gain: f32,
  skid_gain: f32,
  pub effect_gain: f32,
  reference_speed: f32,
}
impl Rules {
  pub fn load() -> Result<Self, String> {
    let rules: Self = serde_json::from_str(include_str!("../../../assets/native/audio.json"))
      .map_err(|e| e.to_string())?;
    if !(2..=32).contains(&rules.engine_pitch_steps)
      || rules.engine_pitch_min <= 0.0
      || rules.engine_pitch_max < rules.engine_pitch_min
      || rules.reference_speed <= 0.0
    {
      return Err("invalid native audio configuration".into());
    }
    Ok(rules)
  }
}
pub async fn sound(library: &Library, name: &str) -> Result<Sound, String> {
  let decoded = pcm::decode(
    library
      .find_in(name, "COMMON")
      .ok_or_else(|| format!("missing original sound {name}"))?,
  )
  .map_err(|e| e.to_string())?;
  if decoded.sample_rate == 0 || decoded.samples.is_empty() {
    return Err(format!("invalid original sound {name}"));
  }
  audio::load_sound_from_bytes(&wav::encode_mono_16(decoded.sample_rate, &decoded.samples))
    .await
    .map_err(|e| e.to_string())
}
pub struct EngineAudio {
  sounds: Vec<Sound>,
  idle: Sound,
  coast: Sound,
  brake: Sound,
  skid: Sound,
  active: Vec<bool>,
  mix: [f32; 3],
  revs: f32,
  enabled: bool,
  rules: Rules,
  started: bool,
}
impl EngineAudio {
  pub async fn load(library: &Library, enabled: bool) -> Result<Self, String> {
    let rules = Rules::load()?;
    let decoded = pcm::decode(
      library
        .find_in(&rules.engine_file, "COMMON")
        .ok_or("missing original engine")?,
    )
    .map_err(|e| e.to_string())?;
    let mut sounds = Vec::new();
    for index in 0..rules.engine_pitch_steps {
      let pitch = rules.engine_pitch_min
        + (rules.engine_pitch_max - rules.engine_pitch_min) * index as f32
          / (rules.engine_pitch_steps - 1) as f32;
      sounds.push(
        audio::load_sound_from_bytes(&wav::encode_mono_16(
          (decoded.sample_rate as f32 * pitch) as u32,
          &decoded.samples,
        ))
        .await
        .map_err(|e| e.to_string())?,
      );
    }
    Ok(Self {
      sounds,
      idle: sound(library, &rules.idle_file).await?,
      // COMMON sound-bank slots 0x3d and 3, selected by 00437d00.
      coast: sound(library, "SPUT.PCM").await?,
      brake: sound(library, "BRAKE2.PCM").await?,
      skid: sound(library, &rules.skid_file).await?,
      active: vec![false; rules.engine_pitch_steps],
      mix: [1.0, 0.0, 0.0],
      revs: 0.0,
      enabled,
      rules,
      started: false,
    })
  }
  pub fn update(&mut self, speed: f32, steer: f32, paused: bool) {
    self.update_driving(
      speed.abs(),
      speed,
      1.0,
      steer.abs(),
      true,
      paused,
      1.0 / 60.0,
    );
  }
  pub fn update_driving(
    &mut self,
    speed: f32,
    forward_speed: f32,
    throttle: f32,
    slip: f32,
    grounded: bool,
    paused: bool,
    dt: f32,
  ) {
    if !self.enabled {
      return;
    }
    if !self.started {
      for sound in [&self.idle, &self.coast, &self.brake, &self.skid] {
        audio::play_sound(
          sound,
          PlaySoundParams {
            looped: true,
            volume: 0.0,
          },
        );
      }
      self.started = true;
    }
    let dt = if paused || !dt.is_finite() {
      0.0
    } else {
      dt.clamp(0.0, 0.1)
    };
    let amount = (speed.abs() / self.rules.reference_speed).clamp(0.0, 1.0);
    // Original selects idle below 0.015 world units/ms with no pedal,
    // engine while on the pedal, and SPUT while coasting. Its blend clock
    // advances by elapsed_ms * .03 * .06 (1.8 per second).
    let state = engine_state(speed, throttle);
    for (index, gain) in self.mix.iter_mut().enumerate() {
      let target = f32::from(index == state);
      *gain += (target - *gain).clamp(-dt * 1.8, dt * 1.8);
    }
    // Pitch-bank interpolation is a modern backend adaptation, not gears:
    // the original continuously varies frequency, it does not shift gears.
    self.revs += (amount - self.revs) * (1.0 - (-dt * 18.0).exp());
    let cursor = self.revs * (self.sounds.len() - 1) as f32;
    let gain = if paused { 0.0 } else { 1.0 };
    for (index, sound) in self.sounds.iter().enumerate() {
      let weight = (1.0 - (cursor - index as f32).abs()).max(0.0);
      if weight > 0.0 && !self.active[index] {
        audio::play_sound(
          sound,
          PlaySoundParams {
            looped: true,
            volume: 0.0,
          },
        );
        self.active[index] = true;
      }
      if self.active[index] {
        audio::set_sound_volume(sound, gain * self.rules.engine_gain * self.mix[1] * weight);
        if weight == 0.0 {
          audio::stop_sound(sound);
          self.active[index] = false;
        }
      }
    }
    audio::set_sound_volume(&self.idle, gain * self.rules.idle_gain * self.mix[0]);
    audio::set_sound_volume(&self.coast, gain * self.rules.engine_gain * self.mix[2]);
    // Original braking loop starts only with positive forward speed > .01/ms.
    audio::set_sound_volume(
      &self.brake,
      gain * self.rules.skid_gain * f32::from(grounded && forward_speed > 10.0 && throttle < 0.0),
    );
    audio::set_sound_volume(
      &self.skid,
      gain * self.rules.skid_gain * slip.clamp(0.0, 1.0) * f32::from(grounded),
    );
  }
}
impl Drop for EngineAudio {
  fn drop(&mut self) {
    for sound in self
      .sounds
      .iter()
      .chain([&self.idle, &self.coast, &self.brake, &self.skid])
    {
      audio::stop_sound(sound);
    }
  }
}

fn engine_state(speed: f32, throttle: f32) -> usize {
  if throttle != 0.0 {
    1
  } else if speed.abs() <= 15.0 {
    0
  } else {
    2
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn original_pedal_idle_and_coasting_selection() {
    assert_eq!(engine_state(0.0, 1.0), 1);
    assert_eq!(engine_state(-20.0, -1.0), 1);
    assert_eq!(engine_state(15.0, 0.0), 0);
    assert_eq!(engine_state(15.01, 0.0), 2);
    assert_eq!(engine_state(-20.0, 0.0), 2);
  }
  #[test]
  fn original_coasting_and_braking_clips_decode() {
    let library = Library::open(
      std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../extracted/Program_Files_Group/LEGO.JAM"),
    )
    .unwrap();
    for name in ["SPUT.PCM", "BRAKE2.PCM"] {
      let clip = pcm::decode(library.find_in(name, "COMMON").unwrap()).unwrap();
      assert!(clip.sample_rate > 0 && !clip.samples.is_empty());
    }
    let bank =
      lrformats::cinematic_audio::bank(library.find_in("GENERAL.SBK", "COMMON").unwrap()).unwrap();
    for (slot, file) in [
      (3, "brake2.pcm"),
      (10, "engine.pcm"),
      (32, "idle.pcm"),
      (0x3d, "sput.pcm"),
      (0x38, "skrenv.pcm"),
      (0x19, "hitenv.pcm"),
    ] {
      assert!(
        bank[slot].eq_ignore_ascii_case(file),
        "original sound slot {slot}"
      );
    }
  }
  #[test]
  fn real_engine_bank_crossfades_without_constant_speed_restarts_and_mutes_when_paused() {
    use crate::platform::{audio::AudioCommand, test_state};
    let library = Library::open(
      std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../extracted/Program_Files_Group/LEGO.JAM"),
    )
    .unwrap();
    let mut load = std::pin::pin!(EngineAudio::load(&library, true));
    let mut engine = match std::future::Future::poll(
      load.as_mut(),
      &mut std::task::Context::from_waker(std::task::Waker::noop()),
    ) {
      std::task::Poll::Ready(Ok(engine)) => engine,
      _ => panic!("original PCM engine bank must load synchronously"),
    };
    for _ in 0..120 {
      engine.update_driving(80.0, 80.0, 1.0, 0.0, true, false, 1.0 / 60.0);
    }
    assert!(engine.active.iter().filter(|active| **active).count() <= 2);
    test_state(|state| state.audio_commands.clear());
    engine.update_driving(80.0, 80.0, 1.0, 0.0, true, false, 1.0 / 60.0);
    test_state(|state| {
      assert!(
        state
          .audio_commands
          .iter()
          .all(|command| matches!(command, AudioCommand::Volume(_, _)))
      )
    });
    engine.update_driving(80.0, 80.0, 0.0, 0.0, true, false, 0.1);
    assert!(engine.mix[2] > 0.0 && engine.mix[1] < 1.0);
    test_state(|state| state.audio_commands.clear());
    engine.update_driving(80.0, 80.0, 0.0, 1.0, true, true, 0.1);
    test_state(|state| {
      assert!(
        state
          .audio_commands
          .iter()
          .all(|command| matches!(command, AudioCommand::Volume(_, volume) if *volume == 0.0))
      )
    });
  }
}
