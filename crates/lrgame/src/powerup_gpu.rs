//! Original COMMON geometry/materials for pickups and active powerups.
use crate::gpu::TrackGpu;
use crate::platform::prelude::*;
use crate::powerup_animation::Player;
use lrformats::{library::Library, model::Model, scene::SceneBindings};
use lrsim::power_weapons::Attack;
use lrsim::powerups::{Powerups, Racer};

pub struct PowerupGpu {
  bricks: Vec<Player>,
  missile: Player,
  cannon: TrackGpu,
  grapple: TrackGpu,
  barrel: TrackGpu,
  mine: Player,
  curse: Player,
  beam: TrackGpu,
  tether: TrackGpu,
  oil: Texture2D,
  shields: Vec<(Player, Player)>,
  warp: TrackGpu,
  turbos: Vec<(Player, Player, Player)>,
}
impl PowerupGpu {
  pub fn load(library: &Library, rules: &lrsim::powerups::Rules) -> Result<Self, String> {
    let bindings = SceneBindings::load(
      library,
      "COMMON",
      library
        .find_in("POWERUP.WDB", "COMMON")
        .ok_or("missing powerup world")?,
    )?;
    let load = |name: &str| {
      let model = Model::load_with_materials(library, name, Some("COMMON"), &bindings.materials)
        .map_err(|e| e.to_string())?;
      let mut gpu = TrackGpu::upload(&model)?;
      gpu.enable_scene_render(&model)?;
      Ok::<_, String>(gpu)
    };
    let animated = |name: &str| Player::load(library, name, &bindings);
    let bricks = (0..5)
      .map(|kind| Player::pickup(library, kind, &bindings))
      .collect::<Result<_, _>>()?;
    let image = lrformats::bmp::decode(
      library
        .find_in(&rules.models.oil, "COMMON")
        .ok_or("missing original oil image")?,
    )
    .map_err(|e| e.to_string())?;
    let mut rgba = image.to_rgba();
    for (pixel, index) in rgba.chunks_exact_mut(4).zip(&image.indices) {
      if *index == 0 {
        pixel[3] = 0;
      }
    }
    let oil = Texture2D::from_rgba8(image.width, image.height, &rgba);
    Ok(Self {
      bricks,
      missile: animated("dmissil")?,
      cannon: load(&rules.models.cannon)?,
      grapple: load(&rules.models.grapple)?,
      barrel: load(&rules.models.barrel)?,
      mine: animated("magnet")?,
      curse: animated("curse")?,
      beam: load(&rules.models.beam)?,
      tether: load(&rules.models.tether)?,
      oil,
      shields: (0..4)
        .map(|tier| {
          Ok((
            animated(&format!("shield{tier}"))?,
            animated(&format!("shldin{tier}"))?,
          ))
        })
        .collect::<Result<_, String>>()?,
      warp: load(&rules.models.warp)?,
      turbos: (0..3)
        .map(|tier| {
          Ok((
            animated(&format!("TurboL{tier}"))?,
            animated(&format!("turb{tier}f1"))?,
            animated(&format!("turb{tier}f2"))?,
          ))
        })
        .collect::<Result<_, String>>()?,
    })
  }
  pub fn draw(
    &mut self,
    state: &Powerups,
    racers: &[Racer],
    view: crate::race_view::RaceView,
  ) -> Result<(), String> {
    for pickup in &state.pickups {
      let opacity = pickup.opacity(&state.rules);
      if opacity == 0 {
        continue;
      }
      self.bricks[pickup.kind as usize].draw_playback(
        Mat4::from_translation(view.native(pickup.source.position)) * view.world_matrix(),
        state.elapsed_seconds(),
        0,
        true,
        opacity,
        (state.elapsed_seconds() * 1000.0) as u32,
      )?;
    }
    for projectile in &state.weapons.projectiles {
      let position = view.native(projectile.position);
      let direction = view.native(lrsim::contact::normalized(projectile.velocity));
      let transform = if direction.length_squared() > 0.0 {
        Mat4::from_cols(
          direction.extend(0.0),
          Vec3::Y.extend(0.0),
          direction.cross(Vec3::Y).extend(0.0),
          position.extend(1.0),
        )
      } else {
        Mat4::from_translation(position)
      };
      if projectile.attack == Attack::Missile {
        self.missile.draw(
          transform,
          (state.rules.projectile_lifetime_ms as f32 - projectile.remaining_ms) / 1000.0,
          0,
        )?;
        continue;
      }
      if projectile.attack == Attack::Curse {
        self.curse.draw(
          transform,
          (state.rules.projectile_lifetime_ms as f32 - projectile.remaining_ms) / 1000.0,
          0,
        )?;
        continue;
      }
      let model = match projectile.attack {
        Attack::Cannon => &self.cannon,
        Attack::Grapple => &self.grapple,
        Attack::Barrel => &self.barrel,
        _ => &self.cannon,
      };
      model.draw_at(transform);
      gl_use_default_material();
    }
    for zone in &state.weapons.zones {
      let position = view.native(zone.position);
      if zone.attack == Attack::MagneticMine {
        self.mine.draw(
          Mat4::from_translation(position),
          (state.rules.hazard_lifetime_ms as f32 - zone.remaining_ms) / 1000.0,
          0,
        )?;
      } else {
        draw_plane(
          position + Vec3::Y * 0.1,
          vec2(state.rules.hit_radius * 2.0, state.rules.hit_radius * 2.0),
          Some(&self.oil),
          WHITE,
        );
      }
    }
    let beam = |model: &TrackGpu, from: [f32; 3], to: [f32; 3]| {
      let from = view.native(from);
      let to = view.native(to);
      let direction = (to - from).normalize_or_zero();
      if direction.length_squared() > 0.0 {
        let up = if direction.dot(Vec3::Y).abs() > 0.99 {
          Vec3::Z
        } else {
          Vec3::Y
        };
        let side = direction.cross(up).normalize();
        let up = side.cross(direction);
        model.draw_at(Mat4::from_cols(
          (direction * (to - from).length() / model.radius.max(1.0) / 2.0).extend(0.0),
          up.extend(0.0),
          side.extend(0.0),
          ((from + to) * 0.5).extend(1.0),
        ));
        gl_use_default_material();
      }
    };
    for flash in &state.weapons.flashes {
      if let (Some(owner), Some(target)) = (racers.get(flash.owner), racers.get(flash.target)) {
        beam(&self.beam, owner.position, target.position);
      }
    }
    for (racer, inventory) in racers.iter().zip(&state.inventories) {
      let transform = view.car(racer.position, racer.forward, racer.up);
      if inventory.shield.active() {
        let tier = inventory.shield_tier as usize;
        let seconds = inventory.shield.elapsed_ms as f32 / 1000.0;
        let (outer, inner) = &mut self.shields[tier];
        // Scene_StartEffect_Multi starts both clips without repeat. 0x10000
        // enables Advance_Clips; only 0x40000 requests cyclic playback.
        outer.draw_playback(
          transform,
          seconds,
          0,
          false,
          255,
          inventory.shield.elapsed_ms,
        )?;
        inner.draw_playback(
          transform,
          seconds,
          0,
          false,
          inventory.shield.inner_alpha(),
          inventory.shield.elapsed_ms,
        )?;
      }
      if inventory.warp_ms > 0.0 {
        self.warp.draw_at(transform);
        gl_use_default_material();
      }
      if let Some(clip) = inventory.turbo.clip() {
        let tier = inventory.turbo_tier.min(2) as usize;
        let seconds = inventory.turbo.clip_ms as f32 / 1000.0;
        let basis = [
          racer.forward,
          lrsim::contact::cross(racer.up, racer.forward),
          racer.up,
        ]
        .concat()
        .try_into()
        .unwrap();
        let mount = lrsim::turbo_mount::solve(
          &lrsim::turbo_mount::Case {
            position: racer.position,
            basis,
          },
          state.rules.turbo_mount_offset,
        );
        let transform = view.car(mount.position, mount.forward, mount.up);
        let (burner, flame_a, flame_b) = &mut self.turbos[tier];
        burner.draw_playback(
          transform,
          seconds,
          clip,
          false,
          255,
          inventory.turbo.elapsed_ms,
        )?;
        let alpha = inventory.turbo.flame_alpha();
        if alpha > 0 {
          flame_b.draw_playback(
            transform,
            seconds,
            clip,
            false,
            alpha,
            inventory.turbo.elapsed_ms,
          )?;
          flame_a.draw_playback(
            transform,
            seconds,
            clip,
            false,
            alpha,
            inventory.turbo.elapsed_ms,
          )?;
        }
      }
      if inventory.curse_ms > 0.0 {
        self.curse.draw(
          transform,
          (state.rules.weapons.curse_ms as f32 - inventory.curse_ms) / 1000.0,
          0,
        )?;
      }
      if let Some(target) = inventory
        .grapple_target
        .filter(|_| inventory.grapple_ms > 0.0)
        .and_then(|i| racers.get(i))
      {
        beam(&self.tether, racer.position, target.position);
      }
    }
    Ok(())
  }
}
