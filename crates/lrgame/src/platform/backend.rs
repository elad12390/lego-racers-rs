//! Bevy owns the native main loop; the existing application future is non-Send.
use super::{
  audio, input,
  material::{OriginalSurface, SHADER, SHADER_PATH},
  performance::Performance,
  readback, renderer, state,
};
use bevy::{prelude::*, window::WindowResolution};
use std::{
  future::Future,
  pin::Pin,
  sync::Arc,
  task::{Context, Poll, Wake, Waker},
};
struct Runner(Pin<Box<dyn Future<Output = Result<(), String>>>>);
struct Noop;
impl Wake for Noop {
  fn wake(self: Arc<Self>) {}
}
pub fn run(
  title: String,
  show_window: bool,
  future: impl Future<Output = Result<(), String>> + 'static,
) -> std::process::ExitCode {
  let mut app = App::new();
  app.add_plugins(
    DefaultPlugins
      .set(bevy::render::RenderPlugin {
        synchronous_pipeline_compilation: true,
        ..default()
      })
      .set(WindowPlugin {
        primary_window: Some(Window {
          title,
          visible: show_window,
          resolution: WindowResolution::new(1000, 760),
          ..default()
        }),
        ..default()
      }),
  );
  install_presentation(&mut app);
  app.init_resource::<Performance>();
  if !show_window {
    app.insert_resource(bevy::winit::WinitSettings {
      focused_mode: bevy::winit::UpdateMode::Continuous,
      unfocused_mode: bevy::winit::UpdateMode::Continuous,
    });
  }
  app.world_mut().insert_non_send(Runner(Box::pin(future)));
  app.add_systems(Update, advance);
  println!("native engine: Bevy 0.19 / wgpu / Metal; shader {SHADER_PATH}");
  let exit = app.run();
  if exit.is_success() {
    std::process::ExitCode::SUCCESS
  } else {
    std::process::ExitCode::FAILURE
  }
}
fn install_presentation(app: &mut App) {
  app.add_plugins(MaterialPlugin::<OriginalSurface>::default());
  app
    .world_mut()
    .resource_mut::<Assets<bevy::shader::Shader>>()
    .insert(
      SHADER.id(),
      bevy::shader::Shader::from_wgsl(
        include_str!("../../../../assets/shaders/original_surface.wgsl"),
        SHADER_PATH,
      ),
    )
    .expect("embedded original-surface shader");
  app.init_resource::<renderer::Renderer>();
  app.init_resource::<super::target::Target>();
  app.init_resource::<input::InputEvents>();
  app.init_resource::<readback::Readback>();
  readback::install(app);
  app.init_resource::<audio::Backend>();
}

/// Real offscreen GPU integration, without an OS window or speaker playback.
#[cfg(test)]
pub(crate) fn offscreen_test_app() -> App {
  let mut app = App::new();
  app.add_plugins(
    DefaultPlugins
      .build()
      .disable::<bevy::winit::WinitPlugin>()
      .disable::<bevy::audio::AudioPlugin>()
      .set(bevy::render::RenderPlugin {
        synchronous_pipeline_compilation: true,
        ..default()
      })
      .set(WindowPlugin {
        primary_window: None,
        ..default()
      }),
  );
  install_presentation(&mut app);
  app.finish();
  app.cleanup();
  app
}

#[cfg(test)]
pub(crate) fn submit_test_frame(app: &mut App) {
  renderer::flush(app.world_mut());
  // The test uses the exact game's GPU target and readback, not a window surface.
  for mut camera in app
    .world_mut()
    .query::<&mut Camera>()
    .iter_mut(app.world_mut())
  {
    if camera.order == 1000 {
      camera.is_active = false;
    }
  }
  readback::flush(app.world_mut());
}
fn advance(world: &mut World) {
  if !world.contains_non_send::<Runner>() {
    return;
  }
  input::sync(world);
  let (capturing, focused) = state::with(|s| (s.capture_inflight, s.focused));
  world.resource_mut::<Performance>().tick(capturing, focused);
  let application_start = std::time::Instant::now();
  if !state::with(|s| s.capture_inflight) {
    let mut runner = world.remove_non_send::<Runner>().unwrap();
    let waker = Waker::from(Arc::new(Noop));
    let mut cx = Context::from_waker(&waker);
    match runner.0.as_mut().poll(&mut cx) {
      Poll::Ready(result) => {
        world.resource::<Performance>().report();
        let exit = match result {
          Ok(()) => AppExit::Success,
          Err(error) => {
            eprintln!("{error}");
            AppExit::error()
          }
        };
        world.write_message(exit);
      }
      Poll::Pending => world.insert_non_send(runner),
    }
  }
  let application_seconds = application_start.elapsed().as_secs_f64();
  let submission_start = std::time::Instant::now();
  renderer::flush(world);
  audio::flush(world);
  readback::flush(world);
  if !capturing {
    world.resource_mut::<Performance>().record_cpu(
      application_seconds,
      submission_start.elapsed().as_secs_f64(),
    );
  }
}
