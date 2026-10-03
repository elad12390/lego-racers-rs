//! Unpaid local randomness; isolated captures still supply their own fixed clock.
use std::cell::Cell;
thread_local! {static SEED:Cell<u64>=const{Cell::new(0x9824_b93f_73cd_170b)};}
pub trait Range: Copy {
  fn sample(low: Self, high: Self, bits: u64) -> Self;
}
macro_rules! integer {($($t:ty),*)=>{$(impl Range for $t{fn sample(low:Self,high:Self,bits:u64)->Self{assert!(high>low);low+(bits%((high-low) as u64)) as Self}})*};}
integer!(i32, u32, u16, usize);
impl Range for f32 {
  fn sample(low: Self, high: Self, bits: u64) -> Self {
    low + (high - low) * ((bits >> 40) as f32 / 16777216.0)
  }
}
pub fn gen_range<T: Range>(low: T, high: T) -> T {
  SEED.with(|s| {
    let mut v = s.get();
    v ^= v << 13;
    v ^= v >> 7;
    v ^= v << 17;
    s.set(v);
    T::sample(low, high, v)
  })
}
pub fn srand(seed: u64) {
  SEED.with(|s| s.set(seed.max(1)));
}
