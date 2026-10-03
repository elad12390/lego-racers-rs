use std::path::PathBuf;

#[derive(Clone)]
pub struct Options {
  pub play: bool,
  pub scenery_smoke: bool,
  pub scenery_object: Option<String>,
  pub car: String,
  pub driver: String,
  pub race_name: Option<String>,
  pub time_trial: bool,
  pub profile: Option<PathBuf>,
  pub menu_smoke: bool,
  pub menu_views_only: bool,
  pub award_smoke: bool,
  pub reward_smoke: Option<String>,
  pub versus: bool,
  pub versus_smoke: bool,
  pub capture_clip: bool,
  pub camera_smoke: bool,
  pub intro_smoke: bool,
  pub turbo_smoke: bool,
  pub camera_mode: crate::camera_rig::Mode,
  pub menu_preview_smoke: bool,
  pub music: bool,
  pub sound: bool,
  pub audible_diagnostic: bool,
  pub mirrored: bool,
  pub smoke_race: Option<String>,
  pub builder_smoke: bool,
  pub driver_smoke: bool,
  pub custom_build: Option<lrsim::brick_build::Build>,
  pub custom_driver: Option<crate::custom_driver::Build>,
  pub drive: bool,
  pub jam: PathBuf,
  pub table: String,
  pub capture: Option<PathBuf>,
  pub frames: Option<u32>,
  pub scripted_camera: bool,
  pub scripted_drive: bool,
  pub scripted_route: bool,
  pub finish_tail_frames: u32,
}

impl Options {
  pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
    let mut args = args.into_iter();
    let first = args.next();
    let play = first.is_none() || first.as_deref() == Some("--play");
    let drive = match first.as_deref() {
            None|Some("--play")=>true,
            Some("--preview") => false,
            Some("--drive") => true,
            _ => return Err("usage: lrracers <--preview|--drive> <LEGO.JAM> <RACE_TABLE> [--capture DIR --frames N --scripted-camera|--scripted-drive]".into()),
        };
    let jam = if first.is_none() {
      PathBuf::from("extracted/Program_Files_Group/LEGO.JAM")
    } else {
      args.next().ok_or("missing LEGO.JAM path")?.into()
    };
    let table = if play {
      "RACEC0R0".into()
    } else {
      args.next().ok_or("missing race table")?
    };
    let mut options = Self {
      play,
      scenery_smoke: false,
      scenery_object: None,
      car: "BK".into(),
      driver: "BK".into(),
      race_name: None,
      time_trial: false,
      profile: None,
      menu_smoke: false,
      menu_views_only: false,
      award_smoke: false,
      reward_smoke: None,
      versus: false,
      versus_smoke: false,
      capture_clip: false,
      camera_smoke: false,
      intro_smoke: false,
      turbo_smoke: false,
      camera_mode: crate::camera_rig::Mode::default(),
      menu_preview_smoke: false,
      music: true,
      sound: true,
      audible_diagnostic: false,
      mirrored: false,
      smoke_race: None,
      builder_smoke: false,
      driver_smoke: false,
      custom_build: None,
      custom_driver: None,
      jam,
      table,
      drive,
      capture: None,
      frames: None,
      scripted_camera: false,
      scripted_drive: false,
      scripted_route: false,
      finish_tail_frames: 0,
    };
    while let Some(arg) = args.next() {
      match arg.as_str() {
        "--capture" => {
          options.capture = Some(args.next().ok_or("missing capture directory")?.into())
        }
        "--profile" => options.profile = Some(args.next().ok_or("missing profile path")?.into()),
        "--menu-smoke" => options.menu_smoke = true,
        "--menu-views-only" => options.menu_views_only = true,
        "--award-smoke" => options.award_smoke = true,
        "--reward-smoke" => {
          options.reward_smoke = Some(args.next().ok_or("missing reward champion")?)
        }
        "--scenery-smoke" => options.scenery_smoke = true,
        "--scenery-object" => {
          options.scenery_object = Some(args.next().ok_or("missing world-detail object name")?)
        }
        "--versus-smoke" => {
          options.versus_smoke = true;
          options.versus = true;
        }
        "--capture-clip" => options.capture_clip = true,
        "--camera-smoke" => options.camera_smoke = true,
        "--intro-smoke" => options.intro_smoke = true,
        "--turbo-smoke" => options.turbo_smoke = true,
        "--menu-preview-smoke" => options.menu_preview_smoke = true,
        "--smoke-race" => options.smoke_race = Some(args.next().ok_or("missing smoke race name")?),
        "--builder-smoke" => options.builder_smoke = true,
        "--driver-smoke" => options.driver_smoke = true,
        "--time-trial" => options.time_trial = true,
        "--audible-diagnostic" => options.audible_diagnostic = true,
        "--car" => options.car = args.next().ok_or("missing car ID")?,
        "--driver" => options.driver = args.next().ok_or("missing driver ID")?,
        "--frames" => {
          let count = args
            .next()
            .ok_or("missing frame count")?
            .parse::<u32>()
            .map_err(|_| "frame count must be a positive integer")?;
          if count == 0 || count > 100_000 {
            return Err("frame count must be between1 and100000".into());
          }
          options.frames = Some(count);
        }
        "--scripted-camera" => options.scripted_camera = true,
        "--scripted-drive" => options.scripted_drive = true,
        "--scripted-route" => options.scripted_route = true,
        "--finish-tail-frames" => {
          options.finish_tail_frames = args
            .next()
            .ok_or("missing finish tail frame count")?
            .parse()
            .map_err(|_| "finish tail frames must be an integer")?;
          if options.finish_tail_frames > 600 {
            return Err("finish tail frames must be between0and600".into());
          }
        }
        _ => return Err(format!("unknown option: {arg}")),
      }
    }
    if options.capture.is_some() && options.frames.is_none() {
      return Err("bounded capture requires --frames".into());
    }
    if options.scripted_camera && options.frames.is_none() {
      return Err("scripted camera requires a bounded --frames run".into());
    }
    if options.scripted_drive && (!options.drive || options.frames.is_none()) {
      return Err("scripted drive requires --drive and a bounded --frames run".into());
    }
    if options.scripted_route && (!options.drive || options.frames.is_none()) {
      return Err("scripted route requires --drive and a bounded --frames run".into());
    }
    if options.finish_tail_frames > 0 && !options.scripted_route {
      return Err("finish tail capture requires --scripted-route".into());
    }
    if options.menu_smoke
      && (!options.play || options.profile.is_none() || options.capture.is_none())
    {
      return Err("menu smoke requires --play, --profile and --capture isolation".into());
    }
    if options.menu_preview_smoke
      && (!options.play
        || options.profile.is_none()
        || options.capture.is_none()
        || options.menu_smoke)
    {
      return Err(
        "menu preview smoke requires isolated --play, --profile, --capture and no race smoke"
          .into(),
      );
    }
    if options.builder_smoke && !options.menu_smoke {
      return Err("builder smoke requires isolated menu smoke".into());
    }
    if options.driver_smoke && !options.menu_smoke {
      return Err("driver smoke requires isolated menu smoke".into());
    }
    if options.menu_views_only && !options.menu_smoke {
      return Err("menu views require isolated menu smoke".into());
    }
    if options.award_smoke
      && (!options.play
        || options.profile.is_none()
        || options.capture.is_none()
        || options.menu_smoke)
    {
      return Err(
        "award smoke requires isolated --play, --profile, --capture and no race smoke".into(),
      );
    }
    if options.versus_smoke
      && (!options.play
        || options.profile.is_none()
        || options.capture.is_none()
        || options.menu_smoke
        || options.award_smoke
        || options.menu_preview_smoke)
    {
      return Err(
        "versus smoke requires isolated --play, --profile, --capture and no other smoke mode"
          .into(),
      );
    }
    if options.camera_smoke
      && (options.play
        || !options.scripted_route
        || options.capture.is_none()
        || options.frames.is_none_or(|frames| frames < 840))
    {
      return Err(
        "camera smoke requires bounded --drive --scripted-route, --capture and at least 840 frames"
          .into(),
      );
    }
    let driving_clip = !options.play
      && options.drive
      && options.scripted_route
      && options.capture.is_some()
      && options.frames.is_some_and(|frames| frames >= 600);
    let menu_race_clip = options.play
      && options.menu_smoke
      && !options.menu_views_only
      && !options.award_smoke
      && !options.menu_preview_smoke
      && options.profile.is_some()
      && options.capture.is_some()
      && options.frames.is_some_and(|frames| frames >= 600);
    if options.turbo_smoke && (!driving_clip || options.camera_smoke || options.intro_smoke) {
      return Err(
        "turbo smoke requires isolated bounded diagnostic driving and no camera/intro smoke".into(),
      );
    }
    if options.capture_clip
      && !options.versus_smoke
      && !options.camera_smoke
      && !options.scenery_smoke
      && !options.intro_smoke
      && !driving_clip
      && !menu_race_clip
    {
      return Err("progress clip capture requires isolated smoke or bounded diagnostic driving with --capture and at least 600 frames".into());
    }
    if options.scenery_smoke
      && (options.play
        || options.drive
        || options.capture.is_none()
        || options.frames.is_none_or(|f| f < 2))
    {
      return Err(
        "world-detail inspection requires --preview, --capture and at least two --frames".into(),
      );
    }
    if options.intro_smoke
      && (options.play
        || !options.scripted_route
        || options.capture.is_none()
        || options.frames.is_none_or(|frames| frames < 360))
    {
      return Err(
        "intro smoke requires bounded --drive --scripted-route, --capture and at least 360 frames"
          .into(),
      );
    }
    if options.scenery_object.is_some() && !options.scenery_smoke {
      return Err("world-detail object selection requires --scenery-smoke".into());
    }
    if options.reward_smoke.is_some()
      && (!options.play
        || options.profile.is_none()
        || options.capture.is_none()
        || options.menu_smoke
        || options.award_smoke
        || options.versus_smoke
        || options.menu_preview_smoke)
    {
      return Err("single reward smoke requires isolated --play, --profile, --capture and no other smoke mode".into());
    }
    Ok(options)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn driving_clip_accepts_the_bounded_pipeline_command() {
    let options = Options::parse(
      [
        "--drive",
        "LEGO.JAM",
        "RACEC0R0",
        "--scripted-route",
        "--capture-clip",
        "--capture",
        "isolated-capture",
        "--frames",
        "1200",
      ]
      .map(str::to_string),
    )
    .unwrap();
    assert!(!options.play);
    assert!(options.capture_clip && options.scripted_route);
  }

  #[test]
  fn menu_race_clip_requires_bounded_isolated_race_not_interactive_play() {
    let arguments = [
      "--play",
      "LEGO.JAM",
      "--menu-smoke",
      "--profile",
      "isolated/profile.json",
      "--capture",
      "isolated",
      "--capture-clip",
      "--frames",
      "12000",
    ];
    let options = Options::parse(arguments.map(str::to_string)).unwrap();
    assert!(options.play && options.menu_smoke && options.capture_clip);
    for missing in ["--menu-smoke", "--profile", "--capture", "--frames"] {
      let mut args = arguments.to_vec();
      let index = args.iter().position(|arg| *arg == missing).unwrap();
      args.remove(index);
      if missing != "--menu-smoke" {
        args.remove(index);
      }
      assert!(Options::parse(args.into_iter().map(str::to_string)).is_err());
    }
    let mut short = arguments.to_vec();
    *short.last_mut().unwrap() = "599";
    assert!(Options::parse(short.into_iter().map(str::to_string)).is_err());
    let mut views = arguments.to_vec();
    views.push("--menu-views-only");
    assert!(Options::parse(views.into_iter().map(str::to_string)).is_err());
  }

  #[test]
  fn driving_clip_rejects_interactive_missing_or_short_isolation() {
    for arguments in [
      vec![
        "--drive",
        "LEGO.JAM",
        "RACEC0R0",
        "--capture-clip",
        "--capture",
        "isolated-capture",
        "--frames",
        "1200",
      ],
      vec![
        "--drive",
        "LEGO.JAM",
        "RACEC0R0",
        "--scripted-route",
        "--capture-clip",
        "--frames",
        "1200",
      ],
      vec![
        "--drive",
        "LEGO.JAM",
        "RACEC0R0",
        "--scripted-route",
        "--capture-clip",
        "--capture",
        "isolated-capture",
        "--frames",
        "599",
      ],
      vec![
        "--play",
        "LEGO.JAM",
        "--scripted-route",
        "--capture-clip",
        "--capture",
        "isolated-capture",
        "--frames",
        "1200",
      ],
    ] {
      assert!(Options::parse(arguments.into_iter().map(str::to_string)).is_err());
    }
  }

  #[test]
  fn turbo_smoke_is_limited_to_the_isolated_diagnostic_race() {
    assert!(Options::parse(
      [
        "--drive",
        "LEGO.JAM",
        "RACEC0R0",
        "--scripted-route",
        "--turbo-smoke",
        "--capture",
        "isolated",
        "--frames",
        "600"
      ]
      .map(str::to_string)
    )
    .is_ok());
    assert!(Options::parse(
      [
        "--play",
        "LEGO.JAM",
        "--scripted-route",
        "--turbo-smoke",
        "--capture",
        "isolated",
        "--frames",
        "600"
      ]
      .map(str::to_string)
    )
    .is_err());
    assert!(Options::parse(
      [
        "--drive",
        "LEGO.JAM",
        "RACEC0R0",
        "--scripted-route",
        "--turbo-smoke",
        "--frames",
        "600"
      ]
      .map(str::to_string)
    )
    .is_err());
  }
}
