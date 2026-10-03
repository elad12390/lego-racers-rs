//! Numeric Game state music routing; gain/backend/stream mixing are separate.
pub fn race_index(count: usize, circuit: bool, noise: u16) -> usize {
  if count > 4 && !circuit {
    match usize::from(noise) % (count - 3) {
      1 => 4,
      2 => 5,
      3 => 6,
      _ => 1,
    }
  } else {
    1
  }
}
pub fn finish_index(rank: u32) -> usize {
  if rank == 1 {
    3
  } else {
    2
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn circuits_use_the_primary_theme_and_noncircuits_use_only_race_variants() {
    for count in 4..=7 {
      for noise in 0..1024 {
        assert_eq!(race_index(count, true, noise), 1);
        let index = race_index(count, false, noise);
        assert!(index == 1 || (index >= 4 && index < count));
      }
    }
    assert_eq!(finish_index(1), 3);
    for rank in 2..=6 {
      assert_eq!(finish_index(rank), 2);
    }
  }
}
