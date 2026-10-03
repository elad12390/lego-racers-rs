//! Opt-in bounded native camera evidence, separate from the race implementation.
use crate::platform::prelude::*;
use crate::{
  camera_rig::{ChaseRig, Mode},
  capture_video::Video,
  options::Options,
};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Serialize)]
struct Sample {
  frame: u32,
  mode: Mode,
  rear: bool,
  eye: [f32; 3],
  direction: [f32; 3],
  position: [f32; 3],
  image: PathBuf,
}

pub struct Check {
  directory: PathBuf,
  encode: bool,
  video: Option<Video>,
  samples: Vec<Sample>,
}

impl Check {
  pub fn from_options(options: &Options) -> Option<Self> {
    options.camera_smoke.then(|| Self {
      directory: options
        .capture
        .clone()
        .expect("validated camera check directory"),
      encode: options.capture_clip,
      video: None,
      samples: Vec::new(),
    })
  }

  pub fn select(&self, frame: u32, rig: &mut ChaseRig) -> bool {
    if frame < 240 {
      return false;
    }
    let phase = (frame - 240) / 120;
    rig.set_mode(match phase {
      0 | 1 => Mode::Chase,
      2 => Mode::Far,
      3 => Mode::Near,
      _ => Mode::Cockpit,
    });
    phase == 1
  }

  pub async fn capture(
    &mut self,
    frame: u32,
    rig: &ChaseRig,
    rear: bool,
    position: [f32; 3],
  ) -> Result<(), String> {
    draw_text(
      &format!(
        "{}{} / SCRIPTED CAMERA CHECK",
        rig.mode().label(),
        if rear { " rear" } else { "" }
      ),
      20.0,
      32.0,
      22.0,
      WHITE,
    );
    if matches!(frame, 300 | 420 | 540 | 660 | 780) {
      let image = self.directory.join(format!(
        "camera-{}{}.png",
        rig.mode().label().to_ascii_lowercase(),
        if rear { "-rear" } else { "" },
      ));
      crate::capture::save_frame(&image, &crate::capture::screen_data().await)?;
      let (eye, direction, _) = rig.view_pose(rear);
      self.samples.push(Sample {
        frame,
        mode: rig.mode(),
        rear,
        eye,
        direction,
        position,
        image,
      });
    }
    if self.encode && (240..840).contains(&frame) && frame % 2 == 0 {
      let image = crate::capture::screen_data().await;
      if self.video.is_none() {
        self.video = Some(Video::start(
          &self.directory.join("camera-modes.mp4"),
          image.width,
          image.height,
        )?);
      }
      self.video.as_mut().unwrap().frame(&image)?;
    }
    if frame == 839 {
      let video_frames = self.video.take().map(Video::finish).transpose()?;
      #[derive(Serialize)]
      struct Report<'a> {
        mode: &'static str,
        samples: &'a [Sample],
        video_frames: Option<u32>,
        physical_input_verified: bool,
      }
      let report = Report {
        mode: "scripted_camera_check_not_physical_input_or_parity",
        samples: &self.samples,
        video_frames,
        physical_input_verified: false,
      };
      std::fs::write(
        self.directory.join("camera-modes.json"),
        serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
      )
      .map_err(|e| e.to_string())?;
    }
    Ok(())
  }
}
