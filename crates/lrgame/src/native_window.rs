//! Read-only focus state from Bevy's owning winit Window; never activates it.
pub fn focused() -> bool {
  crate::platform::focused()
}
