//! RacerTable0043c1b0 numeric ranking, not checkpoint contact-dispatch or audio.
use lrformats::checkpoints::Checkpoint;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Deserialize, Serialize)]
pub struct Racer {
  pub checkpoint: Option<usize>,
  pub contact_count: i32,
  pub position: [f32; 3],
  pub flags: u32,
  pub previous_rank: u32,
}

/// Preserve the original strict selection scans, including their tie order.
/// Geared here means race position, not a mechanical transmission gear.
pub fn ranks(racers: &[Racer], checkpoints: &[Checkpoint]) -> Vec<u32> {
  let mut order: Vec<_> = (0..racers.len()).collect();
  let score = |index: usize| {
    racers[index].checkpoint.map_or(0.0, |c| {
      racers[index].contact_count as f32 + checkpoints[c].progress
    })
  };
  for i in 0..order.len().saturating_sub(1) {
    let mut selected = i;
    for j in i + 1..order.len() {
      if score(order[j]) < score(order[selected]) {
        selected = j;
      }
    }
    order.swap(i, selected);
  }
  let mut i = 0;
  while i + 1 < order.len() {
    let mut end = i + 1;
    while end < order.len() && score(order[i]) == score(order[end]) {
      end += 1;
    }
    if end > i + 1 {
      if let Some(c) = racers[order[i]].checkpoint {
        // The original uses the first tied racer's next-plane links
        // for the whole tied run, not one checkpoint per racer.
        let distance = |index: usize| {
          checkpoints[c]
            .next
            .into_iter()
            .filter(|next| *next != 0xff)
            .map(|next| {
              let plane = checkpoints[next as usize].plane;
              (plane[..3]
                .iter()
                .zip(racers[index].position)
                .map(|(n, p)| f64::from(*n) * f64::from(p))
                .sum::<f64>()
                + f64::from(plane[3])) as f32
            })
            .fold(f32::MAX, f32::min)
        };
        for at in i..end {
          let mut selected = at;
          for j in at + 1..end {
            if distance(order[selected]) < distance(order[j]) {
              selected = j;
            }
          }
          order.swap(at, selected);
        }
      }
    }
    i = end;
  }
  let mut result: Vec<_> = racers.iter().map(|r| r.previous_rank).collect();
  for (index, racer) in order.into_iter().enumerate() {
    if racers[racer].flags & 0x1000 == 0 {
      result[racer] = (racers.len() - index) as u32;
    }
  }
  result
}
