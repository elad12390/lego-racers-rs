//! Original initialized query set: primary slot0and owning CPB collider slot1.
//! Owner callbacks happen on each trial, before physical selection/retry.
//! Extra effect colliders and primary event side effects are separate work.
use crate::{
  chassis_dispatch::{self, Collider, Selection},
  checkpoint_contacts::{CheckpointContacts, State},
  mesh_query::Hit,
};

pub struct ChassisWorld<'a> {
  colliders: [Collider; 2],
  checkpoints: &'a CheckpointContacts,
}
impl<'a> ChassisWorld<'a> {
  pub fn new(primary: Collider, checkpoints: &'a CheckpointContacts) -> Self {
    Self {
      colliders: [primary, checkpoints.query_collider()],
      checkpoints,
    }
  }

  pub fn secondary(&self) -> &[Collider] {
    &self.colliders[1..]
  }

  pub fn wheel_surface(
    &self,
    contacts: &crate::contact::Contacts,
    collider: usize,
    surface: u32,
  ) -> lrformats::collision_materials::Surface {
    match collider {
      0 => contacts.surface(surface),
      1 => self.checkpoints.wheel_surface(surface),
      _ => panic!("unbound wheel collider {collider}"),
    }
  }
  pub fn dispatch(
    &self,
    starts: [[f32; 3]; 4],
    ends: [[f32; 3]; 4],
    include_primary: bool,
    state: &mut State,
    mut ordinary: impl FnMut(usize, usize, &Hit) -> bool,
    mut observe: impl FnMut(usize, usize, &Hit),
  ) -> (Selection, u32) {
    let mut count = 0;
    let selected = chassis_dispatch::dispatch(
      &self.colliders,
      starts,
      ends,
      include_primary,
      |collider, probe, hit| {
        observe(collider, probe, hit);
        if collider == 1 && self.checkpoints.touch_hit(state, hit) {
          count += 1;
          false
        } else {
          ordinary(collider, probe, hit)
        }
      },
    );
    (selected, count)
  }

  pub fn material_selection(
    &self,
    contacts: &crate::contact::Contacts,
    starts: [[f32; 3]; 4],
    ends: [[f32; 3]; 4],
    include_primary: bool,
    state: &mut State,
  ) -> (Selection, u32) {
    self.dispatch(
      starts,
      ends,
      include_primary,
      state,
      |collider, _, hit| {
        let surface = if collider == 0 {
          contacts.surface(hit.surface)
        } else {
          self.checkpoints.ordinary_surface(hit.surface)
        };
        // Material event/finish overrides are not swallowed by CPB binding,
        // but their effect tables remain to be integrated separately.
        lrformats::collision_materials::blocks_chassis(surface.flags)
      },
      |_, _, _| {},
    )
  }

  pub fn sweep(
    &self,
    contacts: &crate::contact::Contacts,
    starts: [[f32; 3]; 4],
    ends: [[f32; 3]; 4],
    state: &mut State,
  ) -> (Option<crate::collision_step::Hit>, u32) {
    let (selection, count) = self.material_selection(contacts, starts, ends, true, state);
    (
      selection.probe.map(|probe| crate::collision_step::Hit {
        fraction: selection.fraction,
        normal: selection.normal,
        probe,
      }),
      count,
    )
  }
}
