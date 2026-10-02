//! Normal driving force model in seconds/original world units.
//! References: 00445500 UpdateForces, 004483b0 drive drag,
//! 00446ef0 radius selection, 00447cf0 collision velocity response.
//! Suspension/effects are separate; these fixtures do not prove whole-game feel.
use lrformats::cmb::Chassis;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug)]
pub struct Handling {
    pub acceleration: f32,
    pub forward_limit: f32,
    pub steering_scale: f32,
}

impl Handling {
    pub fn from_chassis(chassis: &Chassis) -> Self {
        // 0042ad70/ada0/add0 rating modifiers; Driver::Update's 80 and 120
        // are AI driving values. Player input at00420130 instead uses54 and
        // doubles it when input opposes current motion; ref speed remains120.
        let acceleration_scale = 1.0 - (50.0 - f32::from(chassis.rating_c)) * 0.001;
        let speed_scale = 1.0 - (50.0 - f32::from(chassis.rating_b)) * 0.001;
        Self {
            acceleration: 54.0 * acceleration_scale,
            forward_limit: 120.0 * speed_scale,
            steering_scale: f32::from(chassis.rating_a) * 0.003 + 0.7,
        }
    }

    pub fn signed_radius(&self, steer: f32) -> f32 {
        let rate = steer.clamp(-1.0, 1.0) * self.steering_scale;
        // 0041fe60: reciprocal of 0.00025 + (0.025-0.00025)*rate.
        clamp_radius(if rate > 0.0 {
            1.0 / (0.00025 + 0.02475 * rate)
        } else if rate < 0.0 {
            1.0 / (0.02475 * rate - 0.00025)
        } else {
            0.0
        })
    }

    pub fn player_throttle(&self, input: f32, forward_speed: f32) -> f32 {
        player_drive_strength(input, forward_speed) * self.acceleration / 54.0
    }

    pub fn player_radius(&self, input: f32, speed: f32) -> f32 {
        let radius = self.signed_radius(input);
        // Driver::UpdateSteering scales the requested radius at <0.04units/ms.
        clamp_radius(if speed < 40.0 { radius * 0.2 } else { radius })
    }
}

pub fn player_drive_strength(input: f32, forward_speed: f32) -> f32 {
    let input = input.clamp(-1.0, 1.0);
    54.0 * input
        * if input * forward_speed < 0.0 {
            2.0
        } else {
            1.0
        }
}

#[derive(Deserialize)]
pub struct ControlCase {
    pub input: f32,
    pub forward_speed: f32,
    pub steer: f32,
    pub speed: f32,
}

#[derive(Serialize)]
pub struct ControlResult {
    pub throttle: f32,
    pub radius: f32,
}

pub fn player_controls(case: &ControlCase) -> ControlResult {
    let handling = Handling {
        acceleration: 54.0,
        forward_limit: 120.0,
        steering_scale: 1.0,
    };
    ControlResult {
        throttle: handling.player_throttle(case.input, case.forward_speed),
        radius: handling.player_radius(case.steer, case.speed),
    }
}

pub fn clamp_radius(radius: f32) -> f32 {
    if radius == 0.0 || radius.abs() > 4096.0 {
        0.0
    } else {
        radius.signum() * radius.abs().max(40.0)
    }
}

///00445dc0 grounded force after complete wheel support selection/alignment.
/// Mass cancels through inverse mass; gravity uses original seconds conversion.
pub fn ground_acceleration(normal:[f32;3])->[f32;3] {
    let component=39.0*normal[2];
    [normal[0]*component,normal[1]*component,-39.0+normal[2]*component]
}

#[derive(Deserialize)]
pub struct ForceCase {
    pub velocity: [f32; 3],
    pub throttle: f32,
    pub reference_speed: f32,
    pub radius: f32,
    pub grounded: bool,
}

#[derive(Serialize)]
pub struct ForceResult {
    pub acceleration: [f32; 3],
    pub radius: f32,
    pub yaw_rate: f32,
}

/// Normal driving with forward +X, no surface/effect modifiers. No guessed grip.
pub fn normal_force(case: &ForceCase) -> ForceResult {
    let radius = clamp_radius(case.radius);
    let [vx, vy, vz] = case.velocity;
    let speed = (vx * vx + vy * vy + vz * vz).sqrt();
    let drag = if case.reference_speed > 0.0 {
        case.throttle.abs() / case.reference_speed.powi(2) * speed
    } else {
        0.0
    };
    let mut acceleration = [-vx * drag, -vy * drag, -vz * drag];
    acceleration[0] += if case.grounded && case.throttle == 0.0 {
        -vx
    } else {
        case.throttle
    };
    if case.grounded {
        acceleration[1] -= 10.0 * vy;
        acceleration[2] -= 10.0 * vz;
        if radius != 0.0 {
            acceleration[1] += vx * vx / radius;
        }
    } else {
        acceleration[2] -= 39.0 * 4.0;
    }
    let yaw_speed = if case.grounded && vx > 0.5 && vx < 30.0 {
        30.0
    } else {
        vx
    };
    ForceResult {
        acceleration,
        radius,
        yaw_rate: if radius == 0.0 {
            0.0
        } else {
            yaw_speed / radius
        },
    }
}

/// Millisecond-based original rebound rewritten in seconds. Normal must be unit.
pub fn wall_response(velocity: [f32; 3], normal: [f32; 3]) -> [f32; 3] {
    let dot = velocity.iter().zip(normal).map(|(v, n)| v * n).sum::<f32>();
    let mut out = velocity;
    for i in 0..3 {
        if dot < 0.0 {
            out[i] -= dot * normal[i];
            out[i] -= dot * normal[i] * if i == 2 { 0.15 } else { 0.3 };
        }
        out[i] += 4.0 * normal[i];
    }
    out[2] = out[2].min(300.0);
    out
}

#[derive(serde::Deserialize)]
pub struct LandingCase {
    pub velocity:[f32;3],
    pub normal:[f32;3],
    pub airborne_ms:u32,
    pub was_airborne:bool,
    pub wheels:u32,
}

#[derive(serde::Serialize)]
pub struct LandingResponse {pub velocity:[f32;3],pub wheels:u32}

#[derive(serde::Deserialize)]
pub struct SupportTorqueCase {
    pub points:[[f32;3];4],
    pub center:[f32;3],
    pub supported:[bool;4],
    pub wheels:u32,
}

///00445dc0 support force and00445500/AddTorqueAtPoint, divided by mass.
pub fn support_torque(case:SupportTorqueCase)->[f32;3] {
    if case.wheels==0 || case.wheels>=3 {return [0.0;3];}
    let lift=39.0/(case.wheels+8) as f32;
    let mut torque=[0.0;3];
    for (point,active) in case.points.into_iter().zip(case.supported) {
        if active {torque[0]+=(point[1]-case.center[1])*lift;torque[1]-=(point[0]-case.center[0])*lift;}
    }
    torque
}

/// Original00445dc0 landing-only branch, seconds-based velocity convention.
pub fn landing_response(case:LandingCase)->LandingResponse {
    let mut result=LandingResponse {velocity:case.velocity,wheels:case.wheels};
    if case.wheels>0 && case.was_airborne && case.airborne_ms>400 {
        let speed=crate::contact::dot(case.velocity,case.normal);
        if speed<0.0 {
            if speed< -50.0 {
                result.velocity[2]-=1.15*speed;
                result.wheels=0;
            } else {result.velocity[2]-=speed;}
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_rebound_retains_tangential_velocity() {
        // Unchanged-original fixture from tools/test_x86_oracle.py.
        let out = wall_response([-2000.0, 3000.0, -4000.0], [1.0, 0.0, 0.0]);
        assert_eq!(out, [604.0, 3000.0, -4000.0]);
        assert_eq!(
            wall_response([-2000.0, 3000.0, -4000.0], [0.0, 0.0, 1.0]),
            [-2000.0, 3000.0, 300.0]
        );
    }
}
