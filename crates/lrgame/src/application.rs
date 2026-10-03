//! Persistent launch/menu/garage/race/results/circuit loop.
//! Original assets/catalogs; native widgets/save encoding are not parity claims.
use crate::platform::prelude::*;
use crate::{
  game_catalog::Catalog, gpu::TrackGpu, menu_ui::MenuUi, options::Options, profile::Profile,
};
use lrformats::library::Library;
#[cfg(test)]
#[path = "career_loop_test.rs"]
mod career_loop_test;

enum Screen {
  Main,
  Races,
  Circuits,
  RacerSelect,
  Garage,
  EditRacer,
  Driver,
  License,
  DeleteRacer,
  Settings,
  Results,
}
struct CircuitRun {
  index: usize,
  races: Vec<usize>,
  next: usize,
  scores: [u32; 6],
  names: [String; 6],
  awards: [u32; 6],
  final_rank: Option<u32>,
  new_parts: bool,
}

pub async fn run(mut options: Options) -> Result<(), String> {
  let library = Library::open(&options.jam).map_err(|e| e.to_string())?;
  let catalog = Catalog::load(&library)?;
  let driver_names = racer_labels(&library)?;
  let path = options.profile.clone().unwrap_or_else(Profile::path);
  let mut profile = Profile::load(&path)?;
  if let Some(saved) = &profile.career {
    let index = catalog
      .circuits
      .iter()
      .position(|c| c.name == saved.circuit)
      .ok_or("saved career circuit missing from original catalog; save preserved")?;
    let points = catalog.rules.finish_points.iter().sum::<u32>();
    if index as u32 > profile.unlocked_circuit
      || saved.completed >= catalog.circuit_races(&saved.circuit).len()
      || saved.scores.iter().map(|v| *v as u64).sum::<u64>()
        != saved.completed as u64 * points as u64
    {
      return Err("invalid saved career standings; save preserved".into());
    }
  }
  if let Some(build) = &profile.custom_build {
    lrsim::brick_build::BuilderData::load(&library)?.validate(build)?;
  }
  if let Some(build) = &profile.custom_driver {
    crate::custom_driver::Data::load(&library)?.validate(build)?;
  }
  profile.sync_racer();
  if !catalog
    .cars
    .iter()
    .any(|c| c.name.eq_ignore_ascii_case(&profile.car))
    || !catalog
      .drivers
      .iter()
      .any(|d| d.name.eq_ignore_ascii_case(&profile.driver))
  {
    return Err("saved appearance is not in the original catalog; save preserved".into());
  }
  let mut ui = MenuUi::load(&library)?;
  ui.audio = Some(
    crate::menu_audio::MenuAudio::load(
      &library,
      options
        .jam
        .parent()
        .ok_or("missing original music directory")?,
    )
    .await?,
  );
  let audible_menu = options.audible_diagnostic
    || !(options.menu_smoke
      || options.menu_preview_smoke
      || options.versus_smoke
      || options.award_smoke
      || options.reward_smoke.is_some());
  let mut screen = Screen::Main;
  let mut circuit: Option<CircuitRun> = None;
  let mut result: Option<crate::driving::RaceResult> = None;
  let mut result_race = 0;
  let mut message = String::new();
  let mut preview_name = String::new();
  let mut preview = None;
  let mut frame = 0u32;
  let mut smoke_stage = 0;
  let mut circuit_menu = crate::circuit_menu::CircuitMenu::new();
  let mut track_menu = crate::track_menu::TrackMenu::new();
  let mut pending_race = None;
  let mut versus_players = None;
  let mut garage_edit: Option<crate::garage_edit::Draft> = None;
  if options.menu_preview_smoke {
    screen = Screen::Circuits;
  }
  if let Some(dir) = &options.capture {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
  }
  if let Some(host) = &options.reward_smoke {
    let before = std::fs::read(&path).ok();
    let dir = options.capture.as_ref().unwrap();
    crate::circuit_award::unlock(
      &library,
      &catalog,
      &profile,
      host,
      &ui,
      Some((dir, format!("reward-{}", host.to_ascii_lowercase()))),
    )
    .await?;
    if std::fs::read(&path).ok() != before {
      return Err("single reward smoke mutated isolated profile".into());
    }
    return Ok(());
  }
  if options.award_smoke {
    let before = std::fs::read(&path).ok();
    let dir = options.capture.as_ref().unwrap();
    for rank in 1..=4 {
      crate::circuit_award::run(
        &library,
        &catalog,
        &profile,
        rank,
        (rank == 1).then_some(0),
        &ui,
        Some(dir),
      )
      .await?;
    }
    for (i, circuit) in catalog.circuits.iter().enumerate().skip(1) {
      crate::circuit_award::unlock(
        &library,
        &catalog,
        &profile,
        &circuit.racers[1],
        &ui,
        Some((dir, format!("unlock-{}", i + 1))),
      )
      .await?;
    }
    crate::circuit_award::unlock(
      &library,
      &catalog,
      &profile,
      "VV",
      &ui,
      Some((dir, "unlock-veronica".into())),
    )
    .await?;
    let unchanged = std::fs::read(&path).ok() == before;
    if !unchanged {
      return Err("award scene smoke mutated isolated profile".into());
    }
    std::fs::write(dir.join("award-scenes.json"),serde_json::to_vec_pretty(&serde_json::json!({"mode":"isolated_original_scene_loading_not_completed_circuits","ranks":[1,2,3,4],"profile_unchanged":unchanged})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
    return Ok(());
  }
  if options.versus_smoke {
    let race = catalog
      .races
      .iter()
      .find(|r| {
        r.name
          .eq_ignore_ascii_case(options.smoke_race.as_deref().unwrap_or("rkr"))
      })
      .ok_or("missing versus smoke race")?;
    options.table = race.table.to_ascii_uppercase();
    options.mirrored = race.mirrored;
    let mut second = Profile::default();
    second.name = "Player 2".into();
    second.car = "KK".into();
    second.driver = "KK".into();
    let track = TrackGpu::upload(&crate::assets::load_track(&options)?)?;
    crate::versus_race::run(&options, &library, &[profile, second], track).await?;
    return Ok(());
  }
  loop {
    if is_quit_requested() {
      break;
    }
    if let Some(audio) = &mut ui.audio {
      audio.update(
        matches!(
          screen,
          Screen::Garage | Screen::EditRacer | Screen::Driver | Screen::License
        ),
        audible_menu && profile.music,
        audible_menu && profile.sound,
      );
      if is_key_pressed(KeyCode::Escape) {
        audio.cancelled();
      }
    }
    if is_key_pressed(KeyCode::Escape) {
      if matches!(screen, Screen::Main) {
        break;
      }
      if let Some(draft) = garage_edit.take() {
        draft.cancel(&mut profile);
        preview_name.clear();
      }
      screen = match screen {
        Screen::Settings => {
          profile.save(&path)?;
          if pending_race.is_some() {
            Screen::RacerSelect
          } else {
            Screen::Main
          }
        }
        Screen::RacerSelect => {
          pending_race = None;
          if circuit.take().is_some() {
            Screen::Circuits
          } else {
            Screen::Races
          }
        }
        Screen::Results => {
          circuit = None;
          Screen::Main
        }
        Screen::EditRacer | Screen::DeleteRacer => Screen::Garage,
        _ => Screen::Main,
      };
      ui.selected = 0;
      next_frame().await;
      continue;
    }
    let mut launch = None;
    // Upload textures before drawing the UI, not halfway through a queued
    // text batch: a newly created preview can invalidate its GL binding.
    if matches!(
      screen,
      Screen::Main | Screen::Garage | Screen::EditRacer | Screen::RacerSelect
    ) && preview_name != format!("{}/{}", profile.car, profile.driver)
    {
      preview = Some(crate::racer_preview::RacerPreview::load(
        &library, &catalog, &profile,
      )?);
      preview_name = format!("{}/{}", profile.car, profile.driver);
    }
    #[cfg(test)]
    crate::playable_loop_test::observe_menu(match screen {
      Screen::Main => "main",
      Screen::Races => "tracks",
      Screen::RacerSelect => "racer_select",
      Screen::Results => "results",
      Screen::Circuits => "circuits",
      _ => "other",
    });
    #[cfg(test)]
    career_loop_test::observe(
      match screen {
        Screen::Main => "main",
        Screen::Circuits => "circuits",
        Screen::RacerSelect => "racer_select",
        Screen::Results => "results",
        _ => "other",
      },
      &profile,
      circuit.as_ref().map(|c| (c.next, c.scores)),
    );
    match screen {
      Screen::Main => {
        ui.original_background("", true);
        let labels = [
          ("garage", 37, true),
          ("circuit", 33, true),
          ("single", 34, true),
          ("vs", 35, true),
          ("time", 36, true),
          ("options", 38, true),
          ("quit", 39, true),
        ]
        .into_iter()
        .map(|(key, id, enabled)| Ok((key, ui.strings.get(id)?.to_owned(), enabled)))
        .collect::<Result<Vec<_>, String>>()?;
        let selected = ui.original_buttons("mainmenu", &labels);
        ui.text(
          "Circuit Race: championship career",
          250.0,
          429.0,
          18.0,
          YELLOW,
        );
        if let Some(saved) = &profile.career {
          ui.text(
            &format!("Saved circuit: {} races complete", saved.completed),
            250.0,
            455.0,
            16.0,
            WHITE,
          );
        }
        if let Some(selected) = selected {
          ui.selected = 0;
          options.versus = selected == 3;
          screen = match selected {
            0 => Screen::Garage,
            1 => {
              circuit_menu.selected = profile
                .career
                .as_ref()
                .and_then(|s| catalog.circuits.iter().position(|c| c.name == s.circuit))
                .unwrap_or(profile.unlocked_circuit as usize);
              Screen::Circuits
            }
            2 | 3 => {
              options.time_trial = false;
              Screen::Races
            }
            4 => {
              options.time_trial = true;
              Screen::Races
            }
            5 => Screen::Settings,
            _ => break,
          };
        }
        if let Some(racer) = &mut preview {
          racer.main_driver()?;
        }
      }
      Screen::Races => {
        match track_menu.draw(&library, &catalog, &profile, &ui, options.time_trial)? {
          crate::track_menu::Action::Back => screen = Screen::Main,
          crate::track_menu::Action::Start(index) => {
            circuit = None;
            pending_race = Some(index);
            screen = Screen::RacerSelect;
          }
          crate::track_menu::Action::None => {}
        }
      }
      Screen::Circuits => match circuit_menu.draw(&library, &catalog, &profile, &mut ui)? {
        crate::circuit_menu::Action::Back => {
          screen = Screen::Main;
          ui.selected = 0;
        }
        crate::circuit_menu::Action::Start(index) => {
          let races = catalog.circuit_races(&catalog.circuits[index].name);
          let saved = profile
            .career
            .as_ref()
            .filter(|c| c.circuit == catalog.circuits[index].name && c.completed < races.len());
          let next = saved.map_or(0, |c| c.completed);
          let scores = saved.map_or([0; 6], |c| c.scores);
          let mut names = saved.map_or_else(
            || catalog.circuits[index].racers.clone(),
            |c| c.names.clone(),
          );
          names[0] = profile.name.clone();
          pending_race = races.get(next).copied();
          screen = Screen::RacerSelect;
          circuit = Some(CircuitRun {
            index,
            races,
            next,
            scores,
            names,
            awards: [0; 6],
            final_rank: None,
            new_parts: false,
          });
          options.time_trial = false;
          options.versus = false;
        }
        crate::circuit_menu::Action::None => {}
      },
      Screen::RacerSelect => {
        if options.versus {
          if let Some(players) =
            crate::versus_selection::run(&library, &catalog, &profile, &mut ui).await?
          {
            versus_players = Some(players);
            launch = pending_race.take();
          } else {
            pending_race = None;
            screen = Screen::Races;
          }
        } else {
          ui.original_background("Racer Select", false);
          ui.centered_text(&profile.name, 420.0, 119.0, 26.0, YELLOW);
          let delta = if ui.icon_button("arrowlu", 239.0, 87.0) || is_key_pressed(KeyCode::Left) {
            -1
          } else if ui.icon_button("arrowru", 572.0, 87.0) || is_key_pressed(KeyCode::Right) {
            1
          } else {
            0
          };
          if delta != 0 {
            let index = (profile.active_racer as isize + delta)
              .rem_euclid(profile.racers.len() as isize) as usize;
            profile.select_racer(index)?;
            profile.save(&path)?;
            preview_name.clear();
          }
          if let Some(racer) = &mut preview {
            racer.garage(&ui)?;
          }
          if ui.action("Config", 35.0, 366.0, None) {
            screen = Screen::Settings;
          }
          if ui.action("OK", 35.0, 406.0, Some("chck")) || is_key_pressed(KeyCode::Enter) {
            launch = pending_race.take();
          }
          if ui.action("Cancel", 35.0, 446.0, Some("txtx")) {
            screen = if circuit.is_some() {
              Screen::Circuits
            } else {
              Screen::Races
            };
            circuit = None;
            pending_race = None;
          }
        }
      }
      Screen::Garage => {
        ui.original_background("Build Menu", false);
        let labels = [
          ("newracer", "New Racer".into(), profile.racers.len() < 8),
          ("editracr", "Edit Racer".into(), true),
          ("copyracr", "Copy Racer".into(), profile.racers.len() < 8),
          ("delracer", "Delete Racer".into(), profile.racers.len() > 1),
          ("testtrck", "Test Drive".into(), true),
          ("goback", "Main Menu".into(), true),
        ];
        if let Some(choice) = ui.original_buttons("garage", &labels) {
          match choice {
            0 => {
              garage_edit = Some(crate::garage_edit::Draft::new_racer(&mut profile)?);
              preview_name.clear();
              screen = Screen::Driver;
              ui.selected = 0;
            }
            1 => {
              screen = Screen::EditRacer;
              ui.selected = 0;
            }
            2 => {
              profile.copy_racer()?;
              profile.save(&path)?;
              preview_name.clear();
            }
            3 => {
              screen = Screen::DeleteRacer;
              ui.selected = 0;
            }
            4 => {
              circuit = None;
              options.time_trial = false;
              launch = Some(
                catalog
                  .races
                  .iter()
                  .position(|r| r.name.eq_ignore_ascii_case("test"))
                  .ok_or("missing original test track")?,
              );
            }
            5 => {
              screen = Screen::Main;
              ui.selected = 0;
            }
            _ => {}
          }
        }
        ui.text(
          &profile.name,
          365.0,
          121.0,
          26.0,
          Color::new(0.65, 0.65, 0.15, 1.0),
        );
        let delta = if ui.icon_button("arrowlu", 235.0, 92.0) || is_key_pressed(KeyCode::Left) {
          -1
        } else if ui.icon_button("arrowru", 572.0, 92.0) || is_key_pressed(KeyCode::Right) {
          1
        } else {
          0
        };
        if delta != 0 {
          let index = (profile.active_racer as isize + delta)
            .rem_euclid(profile.racers.len() as isize) as usize;
          profile.select_racer(index)?;
          profile.save(&path)?;
          preview_name.clear();
        }
        if let Some(racer) = &mut preview {
          racer.garage(&ui)?;
        }
      }
      Screen::EditRacer => {
        ui.original_background("Edit Racer", false);
        let labels = [
          ("newracer", "Select Racer".into(), false),
          ("editdrvr", "Build Driver".into(), true),
          ("editlice", "Make License".into(), true),
          ("editcar", "Build Car".into(), true),
          ("testtrck", "Test Drive".into(), true),
          ("goback", "Build Menu".into(), true),
        ];
        if let Some(choice) = ui.original_buttons("garage", &labels) {
          match choice {
            1 => {
              garage_edit = Some(crate::garage_edit::Draft::edit(&profile));
              screen = Screen::Driver;
            }
            2 => {
              garage_edit = Some(crate::garage_edit::Draft::edit(&profile));
              screen = Screen::License;
            }
            3 => {
              if let Some((build, car)) =
                crate::garage_builder::run(&library, &catalog, &profile, &mut ui, None).await?
              {
                profile.custom_build = Some(build);
                profile.custom_enabled = true;
                profile.car = car;
                profile.save(&path)?;
                preview_name.clear();
              }
              ui.selected = 0;
            }
            4 => {
              circuit = None;
              options.time_trial = false;
              launch = Some(
                catalog
                  .races
                  .iter()
                  .position(|r| r.name.eq_ignore_ascii_case("test"))
                  .ok_or("missing test track")?,
              );
            }
            _ => {
              screen = Screen::Garage;
              ui.selected = 0;
            }
          }
        }
        if let Some(racer) = &mut preview {
          racer.garage(&ui)?;
        }
      }
      Screen::Driver => {
        let creating = garage_edit.as_ref().is_some_and(|d| d.creating);
        if let Some(build) =
          crate::driver_editor::run_with_creation(&library, &profile, &catalog, &ui, None, creating)
            .await?
        {
          profile.custom_driver = Some(build);
          profile.license_photo = None;
          preview_name.clear();
          screen = Screen::License;
        } else {
          if let Some(draft) = garage_edit.take() {
            draft.cancel(&mut profile);
          }
          preview_name.clear();
          screen = if creating {
            Screen::Garage
          } else {
            Screen::EditRacer
          };
        }
        ui.selected = 0;
      }
      Screen::DeleteRacer => {
        ui.original_background("Delete Racer", false);
        ui.centered_text(
          &format!("Delete {}?", profile.name),
          320.0,
          190.0,
          28.0,
          WHITE,
        );
        if let Some(choice) = ui.buttons(
          &["Cancel".into(), "Delete Racer".into()],
          230.0,
          265.0,
          50.0,
        ) {
          if choice == 1 {
            profile.delete_racer()?;
            profile.save(&path)?;
            preview_name.clear();
          }
          screen = Screen::Garage;
          ui.selected = 0;
        }
      }
      Screen::License => {
        if crate::license_editor::run(&library, &mut profile, &ui, None).await? {
          if let Some((build, car)) =
            crate::garage_builder::run(&library, &catalog, &profile, &mut ui, None).await?
          {
            profile.custom_build = Some(build);
            profile.custom_enabled = true;
            profile.car = car;
            if let Some(draft) = garage_edit.take() {
              draft.commit(&mut profile, &path)?;
            } else {
              profile.save(&path)?;
            }
            screen = Screen::Garage;
          } else {
            screen = Screen::License;
          }
          preview_name.clear();
        } else {
          screen = Screen::Driver;
        }
        ui.selected = 0;
      }
      Screen::Settings => {
        ui.background("Options");
        let labels = vec![
          format!("Music: {}", if profile.music { "On" } else { "Off" }),
          format!("Sound: {}", if profile.sound { "On" } else { "Off" }),
          format!("Camera: {}", profile.camera_mode.label()),
          "Save / Back".into(),
        ];
        if let Some(choice) = ui.buttons(&labels, 24.0, 150.0, 60.0) {
          match choice {
            0 => {
              profile.music = !profile.music;
              profile.save(&path)?;
            }
            1 => {
              profile.sound = !profile.sound;
              profile.save(&path)?;
            }
            2 => {
              profile.camera_mode = profile.camera_mode.next();
              profile.save(&path)?;
            }
            _ => {
              profile.save(&path)?;
              screen = if pending_race.is_some() {
                Screen::RacerSelect
              } else {
                Screen::Main
              };
              ui.selected = 0;
            }
          }
        }
        ui.text(
          "Arrows: steer / accelerate / brake",
          275.0,
          170.0,
          20.0,
          WHITE,
        );
        ui.text("Space: use powerup   Esc: pause", 275.0, 210.0, 20.0, WHITE);
        ui.text("R: restart race", 275.0, 250.0, 20.0, WHITE);
        ui.text("C: change camera   V: look back", 275.0, 290.0, 20.0, WHITE);
        ui.text(
          "Player 2: Q camera / E look back",
          275.0,
          330.0,
          18.0,
          WHITE,
        );
      }
      Screen::Results => {
        #[cfg(test)]
        if let Some(result) = &result {
          crate::playable_loop_test::observe_result(result);
        }
        ui.background(if circuit.is_some() {
          "Circuit Standings"
        } else {
          "Race Results"
        });
        if let Some(run) = &circuit {
          ui.text(
            &format!(
              "Circuit {} - Race {} / {}",
              run.index + 1,
              run.next + 1,
              run.races.len()
            ),
            245.0,
            93.0,
            22.0,
            YELLOW,
          );
          ui.text(
            &catalog.title(&catalog.races[result_race]),
            245.0,
            122.0,
            17.0,
            WHITE,
          );
          ui.text("RACER", 280.0, 168.0, 17.0, WHITE);
          ui.text("RACE", 478.0, 168.0, 17.0, WHITE);
          ui.text("TOTAL", 551.0, 168.0, 17.0, WHITE);
          let mut order = (0..6).collect::<Vec<_>>();
          order.sort_by_key(|i| std::cmp::Reverse(run.scores[*i]));
          for (row, index) in order.into_iter().enumerate() {
            let rank = 1
              + run
                .scores
                .iter()
                .filter(|v| **v > run.scores[index])
                .count();
            let color = if index == 0 { YELLOW } else { WHITE };
            let y = 208.0 + row as f32 * 32.0;
            ui.text(&format!("{rank}."), 250.0, y, 21.0, color);
            let name = if index == 0 {
              profile.name.clone()
            } else {
              racer_label(&driver_names, &run.names[index])
            };
            ui.fitted_text(&name, 280.0, y, 18.0, 190.0, color);
            ui.text(&format!("+{}", run.awards[index]), 487.0, y, 21.0, color);
            ui.text(&run.scores[index].to_string(), 559.0, y, 21.0, color);
          }
          let footer = if let Some(rank) = run.final_rank {
            format!("Circuit complete! Place {rank} - progress saved")
          } else {
            format!(
              "Next: {}",
              catalog.title(&catalog.races[run.races[run.next + 1]])
            )
          };
          ui.fitted_text(&footer, 245.0, 423.0, 17.0, 365.0, YELLOW);
          ui.text(
            "30 / 20 / 10 / 3 / 2 / 1 points per race",
            245.0,
            451.0,
            15.0,
            WHITE,
          );
        } else if let Some(result) = &result {
          ui.text(
            &catalog.title(&catalog.races[result_race]),
            245.0,
            88.0,
            20.0,
            WHITE,
          );
          let mut order = (0..result.names.len()).collect::<Vec<_>>();
          order.sort_by_key(|i| result.ranks[*i]);
          for (row, index) in order.into_iter().enumerate() {
            ui.text(
              &format!(
                "{}. {}",
                result.ranks[index],
                if index == 0 {
                  profile.name.clone()
                } else {
                  racer_label(&driver_names, &result.names[index])
                }
              ),
              285.0,
              145.0 + row as f32 * 35.0,
              24.0,
              if index == 0 { YELLOW } else { WHITE },
            );
          }
          ui.text(
            &format!("Your time: {:.2} s", result.time),
            280.0,
            383.0,
            22.0,
            YELLOW,
          );
          if let Some(time) = result.original_ghost_time {
            ui.text(&format!("Veronica: {time:.2} s"), 280.0, 412.0, 18.0, WHITE);
            ui.text(
              if result.time < time {
                "VERONICA BEATEN"
              } else {
                "TRY TO BEAT VERONICA"
              },
              280.0,
              441.0,
              17.0,
              YELLOW,
            );
          }
        }
        let labels = vec![
          if circuit.is_some() {
            if circuit.as_ref().is_some_and(|c| c.final_rank.is_some()) {
              "View Award".into()
            } else {
              "Next Race".into()
            }
          } else {
            "Race Again".into()
          },
          if circuit.as_ref().is_some_and(|c| c.final_rank.is_none()) {
            "Save / Main Menu".into()
          } else {
            "Main Menu".into()
          },
        ];
        if let Some(choice) = ui.buttons(&labels, 24.0, 160.0, 60.0) {
          if choice == 1 {
            screen = Screen::Main;
            circuit = None;
            ui.selected = 0;
          } else if let Some(run) = &mut circuit {
            run.next += 1;
            if run.next < run.races.len() {
              launch = Some(run.races[run.next]);
            } else {
              let rank = run.final_rank.ok_or("completed circuit missing award")?;
              if let Some(audio) = &mut ui.audio {
                audio.stop_music();
              }
              crate::circuit_award::run(
                &library,
                &catalog,
                &profile,
                rank,
                run.new_parts.then_some(run.index),
                &ui,
                None,
              )
              .await?;
              message = format!(
                "Circuit {} complete! Place {rank}, {} points - saved",
                run.index + 1,
                run.scores[0]
              );
              if rank <= 2 && run.index + 1 < catalog.circuits.len() {
                circuit_menu.selected = run.index + 1;
              }
              screen = Screen::Circuits;
              circuit = None;
              ui.selected = 0;
            }
          } else {
            launch = Some(result_race);
          }
        }
      }
    }
    if !message.is_empty() {
      ui.text(&message, 245.0, 460.0, 15.0, YELLOW);
    }
    if options.menu_preview_smoke && frame % 8 == 7 {
      let dir = options.capture.as_ref().unwrap();
      crate::capture::save_frame(
        &dir.join(format!("circuit-{}.png", circuit_menu.selected + 1)),
        &crate::capture::screen_data().await,
      )?;
      circuit_menu.selected += 1;
      if circuit_menu.selected == catalog.circuits.len() {
        break;
      }
    }
    if options.menu_smoke {
      if frame == 2 && smoke_stage == 0 {
        crate::capture::save_frame(
          &options.capture.as_ref().unwrap().join("menu.png"),
          &crate::capture::screen_data().await,
        )?;
        screen = Screen::Garage;
        ui.selected = 0;
        smoke_stage = 1;
      }
      if frame == 5 && smoke_stage == 1 {
        crate::capture::save_frame(
          &options.capture.as_ref().unwrap().join("garage.png"),
          &crate::capture::screen_data().await,
        )?;
        let draft = if options.driver_smoke {
          Some(crate::garage_edit::Draft::new_racer(&mut profile)?)
        } else {
          None
        };
        let before = std::fs::read(&path).ok();
        if options.driver_smoke {
          profile.custom_driver = crate::driver_editor::run(
            &library,
            &profile,
            &catalog,
            &ui,
            options.capture.as_deref(),
          )
          .await?;
          crate::license_editor::run(&library, &mut profile, &ui, options.capture.as_deref())
            .await?;
          preview_name.clear();
        }
        if options.builder_smoke {
          let (build, car) = crate::garage_builder::run(
            &library,
            &catalog,
            &profile,
            &mut ui,
            options.capture.as_deref(),
          )
          .await?
          .ok_or("builder smoke canceled")?;
          profile.custom_build = Some(build);
          profile.car = car;
          profile.custom_enabled = true;
        }
        if std::fs::read(&path).ok() != before {
          return Err("garage draft changed the profile before completion".into());
        }
        if let Some(draft) = draft {
          draft.commit(&mut profile, &path)?;
        } else {
          profile.save(&path)?;
        }
        if options.driver_smoke {
          let saved = std::fs::read(&path).map_err(|e| e.to_string())?;
          let old = serde_json::to_vec(&profile).map_err(|e| e.to_string())?;
          let canceled = crate::garage_edit::Draft::new_racer(&mut profile)?;
          profile.name = "Canceled Racer".into();
          profile.license_photo = None;
          canceled.cancel(&mut profile);
          if serde_json::to_vec(&profile).map_err(|e| e.to_string())? != old
            || std::fs::read(&path).map_err(|e| e.to_string())? != saved
          {
            return Err("garage cancellation changed saved/current racer".into());
          }
        }
        if Profile::load(&path)?.custom_build != profile.custom_build {
          return Err("custom brick car save/reload mismatch".into());
        }
        if Profile::load(&path)?.custom_driver != profile.custom_driver
          || Profile::load(&path)?.license_photo != profile.license_photo
        {
          return Err("custom driver/license save/reload mismatch".into());
        }
        screen = Screen::Races;
        ui.selected = 0;
        smoke_stage = 2;
      }
      if frame == 8 && smoke_stage == 2 {
        crate::capture::save_frame(
          &options.capture.as_ref().unwrap().join("selection.png"),
          &crate::capture::screen_data().await,
        )?;
        if options.menu_views_only {
          screen = Screen::RacerSelect;
          preview_name.clear();
          smoke_stage = 4;
        } else {
          launch = Some(
            catalog
              .races
              .iter()
              .position(|r| {
                r.name
                  .eq_ignore_ascii_case(options.smoke_race.as_deref().unwrap_or("rkr"))
              })
              .ok_or("missing smoke race")?,
          );
          smoke_stage = 3;
        }
      }
      if frame == 11 && smoke_stage == 4 {
        crate::capture::save_frame(
          &options.capture.as_ref().unwrap().join("racer-select.png"),
          &crate::capture::screen_data().await,
        )?;
        options.time_trial = true;
        screen = Screen::Races;
        smoke_stage = 5;
      }
      if frame == 14 && smoke_stage == 5 {
        let dir = options.capture.as_ref().unwrap();
        crate::capture::save_frame(
          &dir.join("time-selection.png"),
          &crate::capture::screen_data().await,
        )?;
        std::fs::write(dir.join("menu-views.json"),serde_json::to_vec_pretty(&serde_json::json!({"mode":"isolated_native_menu_views_not_race_or_physical_input_acceptance","build_restored":Profile::load(&path)?.custom_build==profile.custom_build,"driver_restored":Profile::load(&path)?.custom_driver==profile.custom_driver,"photo_restored":Profile::load(&path)?.license_photo==profile.license_photo})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
        break;
      }
      if matches!(screen, Screen::Results) && smoke_stage == 3 {
        crate::capture::save_frame(
          &options.capture.as_ref().unwrap().join("results.png"),
          &crate::capture::screen_data().await,
        )?;
        let restored = Profile::load(&path)?;
        if restored.best_times.get(&catalog.races[result_race].name)
          != profile.best_times.get(&catalog.races[result_race].name)
        {
          return Err("save restoration failed".into());
        }
        if let Some(ghost) = profile.trial_ghosts.get(&catalog.races[result_race].name) {
          let saved = restored
            .trial_ghosts
            .get(&catalog.races[result_race].name)
            .ok_or("time-race ghost missing after save")?;
          if serde_json::to_vec(ghost).map_err(|e| e.to_string())?
            != serde_json::to_vec(saved).map_err(|e| e.to_string())?
          {
            return Err("time-race ghost save/reload mismatch".into());
          }
        }
        std::fs::write(options.capture.as_ref().unwrap().join("menu-smoke.json"),serde_json::to_vec_pretty(&serde_json::json!({"mode":"native_scripted_menu_race_save_flow_not_physical_input_or_full_game_parity","profile":path,"race":catalog.races[result_race].name,"result":result,"restored_best_times":restored.best_times})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
        break;
      }
    }
    next_frame().await;
    frame += 1;
    if let Some(index) = launch {
      message.clear();
      let race = &catalog.races[index];
      options.table = race.table.to_ascii_uppercase();
      options.race_name = Some(race.name.clone());
      options.car = profile.car.clone();
      options.driver = profile.driver.clone();
      options.mirrored = race.mirrored;
      options.custom_build = if profile.custom_enabled {
        profile.custom_build.clone()
      } else {
        None
      };
      options.custom_driver = profile.custom_driver.clone();
      if options.menu_smoke {
        options.scripted_route = true;
        options.frames = Some(options.frames.unwrap_or(12000));
      }
      options.music = profile.music;
      options.sound = profile.sound;
      options.camera_mode = profile.camera_mode;
      if let Some(run) = &circuit {
        profile.career = Some(crate::profile::CareerProgress {
          circuit: catalog.circuits[run.index].name.clone(),
          completed: run.next,
          scores: run.scores,
          names: run.names.clone(),
        });
        profile.save(&path)?;
      }
      if let Some(audio) = &mut ui.audio {
        audio.stop_music();
      }
      let model = crate::assets::load_track(&options)?;
      let track = TrackGpu::upload(&model)?;
      result = if options.versus {
        crate::versus_race::run(
          &options,
          &library,
          versus_players
            .as_ref()
            .ok_or("versus players not selected")?,
          track,
        )
        .await?
        .map(|outcome| {
          message = format!("Player 2: {:.2} s", outcome.times[1]);
          crate::driving::RaceResult {
            names: outcome.names.to_vec(),
            ranks: outcome.ranks.to_vec(),
            time: outcome.times[0],
            laps: 3,
            trial_ghost: None,
            original_ghost_time: None,
          }
        })
      } else {
        crate::driving::run_race(options.clone(), track).await?
      };
      result_race = index;
      if options.menu_smoke && result.is_none() {
        return Err(
          "native menu smoke race did not finish within the frame budget; evidence preserved"
            .into(),
        );
      }
      if let Some(outcome) = &result {
        let previous_ghost_unlock = profile.driver_part_allowed(128, &catalog);
        if let Some(ghost) = &outcome.trial_ghost {
          if profile
            .trial_ghosts
            .get(&race.name)
            .is_none_or(|best| ghost.run.total() < best.run.total())
          {
            profile
              .trial_ghosts
              .insert(race.name.clone(), ghost.clone());
          }
          if outcome
            .original_ghost_time
            .is_some_and(|time| outcome.time < time)
          {
            profile.trial_wins.insert(race.name.clone(), true);
          }
        }
        if !options.time_trial && !options.versus {
          profile.record_time(&race.name, outcome.time);
        }
        if !options.versus {
          profile.save(&path)?;
        }
        if !previous_ghost_unlock && profile.driver_part_allowed(128, &catalog) {
          crate::circuit_award::unlock(&library, &catalog, &profile, "VV", &ui, None).await?;
        }
        if let Some(run) = &mut circuit {
          if outcome.ranks.len() != 6 || outcome.names.len() != 6 {
            return Err("career race requires the original six-racer roster".into());
          }
          for i in 0..6 {
            run.names[i] = if i == 0 {
              profile.name.clone()
            } else {
              outcome.names[i].clone()
            };
            let rank = outcome.ranks[i];
            if !(1..=6).contains(&rank) {
              return Err("invalid career race placement".into());
            }
            run.awards[i] = catalog.rules.finish_points[rank as usize - 1];
            run.scores[i] += run.awards[i];
          }
          if run.next + 1 == run.races.len() {
            let old_gold = profile
              .circuit_medals
              .get(&catalog.circuits[run.index].name)
              == Some(&1);
            let rank = profile.record_circuit(&catalog, run.index, run.scores)?;
            run.final_rank = Some(rank);
            run.new_parts = rank == 1 && !old_gold;
            profile.career = None;
          } else {
            profile.career = Some(crate::profile::CareerProgress {
              circuit: catalog.circuits[run.index].name.clone(),
              completed: run.next + 1,
              scores: run.scores,
              names: run.names.clone(),
            });
          }
          profile.save(&path)?;
        }
        screen = Screen::Results;
      } else {
        screen = Screen::Main;
        circuit = None;
      }
      ui.selected = 0;
      next_frame().await;
    }
  }
  Ok(())
}

fn racer_labels(library: &Library) -> Result<std::collections::BTreeMap<String, String>, String> {
  let text = lrformats::string_table::StringTable::parse(
    library
      .find_at("DRIVERS.SRF", "GAMEDATA", "ENGLISH")
      .ok_or("missing original driver names")?,
  )?;
  // DRIVERS.DDB keyword33 is the original localized driver-name index.
  lrformats::named_records::Records::parse(
    library
      .find_in("DRIVERS.DDB", "COMMON")
      .ok_or("missing original driver catalog")?,
  )?
  .entries
  .into_iter()
  .map(|(name, fields)| {
    let index = lrformats::named_records::integer(&fields, 0x33)?;
    let index = usize::try_from(index).map_err(|_| "negative driver-name index")?;
    Ok((name.to_ascii_uppercase(), text.get(index)?.to_owned()))
  })
  .collect()
}

fn racer_label(names: &std::collections::BTreeMap<String, String>, code: &str) -> String {
  names
    .get(&code.to_ascii_uppercase())
    .cloned()
    .unwrap_or_else(|| code.to_owned())
}
