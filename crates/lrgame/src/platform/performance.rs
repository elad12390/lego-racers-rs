//! Render-update timings measured in the native app, excluding screenshot waits.
use bevy::prelude::*;
use std::{collections::VecDeque, time::Instant};
#[derive(Resource, Default)]
pub struct Performance {
  last: Option<(Instant, bool)>,
  samples: VecDeque<(f64, bool)>,
  cpu: VecDeque<(f64, f64)>,
  pub capture_updates: u64,
}
impl Performance {
  pub fn tick(&mut self, capturing: bool, focused: bool) {
    let now = Instant::now();
    if capturing {
      self.last = None;
      self.capture_updates += 1;
      return;
    }
    if let Some((last, previous_focus)) = self
      .last
      .filter(|(_, previous_focus)| *previous_focus == focused)
    {
      // Keep at most one minute of 60 Hz samples in an indefinitely running game.
      if self.samples.len() == 3600 {
        self.samples.pop_front();
      }
      self
        .samples
        .push_back((now.duration_since(last).as_secs_f64(), previous_focus));
    }
    self.last = Some((now, focused));
  }
  pub fn record_cpu(&mut self, application: f64, submission: f64) {
    if self.cpu.len() == 3600 {
      self.cpu.pop_front();
    }
    self.cpu.push_back((application, submission));
  }
  pub fn report(&self) {
    let mut samples = self
      .samples
      .iter()
      .skip(20)
      .map(|(seconds, _)| *seconds)
      .filter(|v| *v > 0.0)
      .collect::<Vec<_>>();
    if samples.is_empty() {
      return;
    }
    samples.sort_by(f64::total_cmp);
    let median = samples[samples.len() / 2];
    let p95 = samples[(samples.len() * 95 / 100).min(samples.len() - 1)];
    let mean = samples.iter().sum::<f64>() / samples.len() as f64;
    println!("bevy native pacing: samples={} mean_fps={:.2} median_ms={:.3} p95_ms={:.3} screenshot_wait_updates={} (render updates, not physical-input or feel acceptance)",samples.len(),1.0/mean,median*1000.0,p95*1000.0,self.capture_updates);
    for focused in [true, false] {
      let values = self
        .samples
        .iter()
        .skip(20)
        .filter(|(_, active)| *active == focused)
        .map(|(seconds, _)| *seconds)
        .collect::<Vec<_>>();
      if !values.is_empty() {
        let seconds = values.iter().sum::<f64>();
        println!(
          "bevy focus pacing: focused={focused} samples={} mean_fps={:.2}",
          values.len(),
          values.len() as f64 / seconds
        );
      }
    }
    let cpu = self.cpu.iter().skip(20).collect::<Vec<_>>();
    if !cpu.is_empty() {
      println!("bevy main-thread CPU: samples={} mean_application_ms={:.3} mean_submission_ms={:.3} (excludes Bevy post-update/render-world and screenshot waits)",cpu.len(),cpu.iter().map(|v|v.0).sum::<f64>()*1000.0/cpu.len()as f64,cpu.iter().map(|v|v.1).sum::<f64>()*1000.0/cpu.len()as f64);
    }
  }
}
