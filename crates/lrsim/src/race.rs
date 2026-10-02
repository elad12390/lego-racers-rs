//! Solo race clock and original spatial lap-state integration.
use crate::{
    lap_state::LapState,
    lap_zones::{LapEvent, LapZones},
};
use serde::Serialize;

#[derive(Clone, Serialize)]
pub struct RaceEvent {
    pub time: f64,
    pub position: [f32; 3],
    pub event: LapEvent,
    pub completed_laps: u32,
    pub counter: i32,
    /// Spatial re-entry can redispatch an already selected mode; the original
    /// lap effect then does nothing. Preserve both raw hit and effective state.
    pub state_changed: bool,
}

pub struct Race {
    pub laps: LapState,
    pub lap_times: Vec<f64>,
    pub elapsed: f64,
    pub finished: bool,
    pub events: Vec<RaceEvent>,
    total_laps: u32,
    lap_start: f64,
}

impl Race {
    pub fn new(total_laps: u32) -> Result<Self, String> {
        if !(1..=5).contains(&total_laps) {
            return Err("lap count must be1..5".into());
        }
        Ok(Self {
            laps: LapState::default(),
            lap_times: Vec::new(),
            elapsed: 0.0,
            finished: false,
            events: Vec::new(),
            total_laps,
            lap_start: 0.0,
        })
    }

    pub fn advance(&mut self, old: [f32; 3], position: [f32; 3], zones: &LapZones, dt: f64) {
        if self.finished || dt <= 0.0 || !dt.is_finite() {
            return;
        }
        self.elapsed += dt;
        for event in zones.crossings(old, position) {
            let history = self.laps.history;
            if self.laps.event(event.mode) {
                self.lap_times.push(self.elapsed - self.lap_start);
                self.lap_start = self.elapsed;
            }
            self.events.push(RaceEvent {
                time: self.elapsed,
                position,
                event,
                completed_laps: self.laps.completed_laps,
                counter: self.laps.counter,
                state_changed: history != self.laps.history,
            });
            if self.laps.completed_laps >= self.total_laps {
                self.finished = true;
                break;
            }
        }
    }
}
