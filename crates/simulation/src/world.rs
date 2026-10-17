//! Physical state and observer controls. No browser, renderer, or firmware knowledge.
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionMode {
    Walk,
    Inspect,
}

/// Declared optical inputs, supplied by the selected camera/scenario.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Optics {
    pub focus_distance: f32,
    pub aperture: f32,
    pub focal_length: f32,
    pub sensor_height: f32,
}
impl Default for Optics {
    fn default() -> Self {
        Self {
            focus_distance: 3.,
            aperture: 2.8,
            focal_length: 0.05,
            sensor_height: 0.024,
        }
    }
}
impl Optics {
    pub fn is_valid(&self) -> bool {
        [
            self.focus_distance,
            self.aperture,
            self.focal_length,
            self.sensor_height,
        ]
        .iter()
        .all(|v| v.is_finite() && *v > 0.)
    }
}

#[derive(Resource, Clone, Debug)]
pub struct Simulation {
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub height: f32,
    pub camera_height: f32,
    pub mode: InteractionMode,
    pub elapsed: f64,
    pub movement: Vec2,
    pub orbit: Vec2,
    pub distance: f32,
    pub transition: f32,
    pub optics: Optics,
}
impl Default for Simulation {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            yaw: 0.,
            pitch: 0.,
            height: 1.75,
            camera_height: 1.45,
            mode: InteractionMode::Inspect,
            elapsed: 0.,
            movement: Vec2::ZERO,
            orbit: Vec2::new(0., 0.08),
            distance: 0.13,
            transition: 1.,
            optics: Optics::default(),
        }
    }
}
impl Simulation {
    /// Advance explicitly: real elapsed time interactively, scenario time in tests.
    /// Guest execution speed never scales this interval.
    pub fn advance(&mut self, seconds: f32) {
        assert!(seconds.is_finite() && seconds >= 0.);
        self.elapsed += f64::from(seconds);
        if self.mode == InteractionMode::Walk {
            let direction = self.movement.clamp_length_max(1.);
            self.position += Quat::from_rotation_y(self.yaw)
                * Vec3::new(direction.x, 0., -direction.y)
                * 1.4
                * seconds;
        }
        let target = if self.mode == InteractionMode::Inspect {
            1.
        } else {
            0.
        };
        self.transition += (target - self.transition).clamp(-seconds / 0.25, seconds / 0.25);
    }
    pub fn toggle(&mut self) {
        self.mode = match self.mode {
            InteractionMode::Walk => InteractionMode::Inspect,
            InteractionMode::Inspect => InteractionMode::Walk,
        };
        self.movement = Vec2::ZERO;
        if self.mode == InteractionMode::Inspect {
            self.orbit = Vec2::new(0., 0.08);
        }
    }
    pub fn look(&mut self, delta: Vec2) {
        if self.mode == InteractionMode::Walk {
            self.yaw -= delta.x * 0.0025;
            self.pitch = (self.pitch - delta.y * 0.0025).clamp(-1.35, 1.35);
        }
    }
    pub fn orbit(&mut self, delta: Vec2) {
        if self.mode == InteractionMode::Inspect {
            self.orbit.x -= delta.x * 0.006;
            self.orbit.y = (self.orbit.y + delta.y * 0.006).clamp(-1.4, 1.4);
        }
    }
    pub fn rotation(&self) -> Quat {
        Quat::from_euler(EulerRot::YXZ, self.yaw, self.pitch, 0.)
    }
    pub fn camera_pose(&self) -> Transform {
        Transform::from_translation(
            self.position
                + Vec3::Y * self.camera_height
                + Quat::from_rotation_y(self.yaw) * Vec3::new(0., 0., -0.30),
        )
        .with_rotation(self.rotation())
    }
    pub fn observer_pose(&self) -> Transform {
        let physical = self.camera_pose();
        let eye = self.position + Vec3::Y * self.height;
        let offset = Quat::from_euler(EulerRot::YXZ, self.orbit.x, -self.orbit.y, 0.)
            * Vec3::Z
            * self.distance;
        let close = physical.translation + physical.rotation * offset;
        let inspecting =
            Transform::from_translation(close).looking_at(physical.translation, Vec3::Y);
        let a = self.transition * self.transition * (3. - 2. * self.transition);
        Transform::from_translation(eye.lerp(inspecting.translation, a))
            .with_rotation(self.rotation().slerp(inspecting.rotation, a))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inspecting_changes_only_observer_even_during_transition() {
        let mut s = Simulation {
            mode: InteractionMode::Walk,
            transition: 0.,
            ..default()
        };
        s.look(Vec2::new(100., -40.));
        let camera = s.camera_pose();
        let observer = s.observer_pose();
        s.toggle();
        s.advance(0.1);
        s.orbit(Vec2::new(80., 30.));
        assert_eq!(camera, s.camera_pose());
        assert_ne!(observer, s.observer_pose());
        s.advance(1.);
        assert_eq!(camera, s.camera_pose());
    }
    #[test]
    fn explicit_time_replays_independently_of_guest_speed_and_display_rate() {
        let mut a = Simulation {
            mode: InteractionMode::Walk,
            movement: Vec2::Y,
            ..default()
        };
        let mut b = a.clone();
        for _ in 0..60 {
            a.advance(1. / 60.);
        }
        for _ in 0..30 {
            b.advance(1. / 30.);
        }
        assert!(a.position.distance(b.position) < 0.00001);
        assert!((a.position.z + 1.4).abs() < 0.00001);
    }
    #[test]
    fn diagonal_speed_and_inspect_movement_are_bounded() {
        let mut s = Simulation {
            mode: InteractionMode::Walk,
            movement: Vec2::ONE,
            ..default()
        };
        s.advance(1.);
        assert!((s.position.length() - 1.4).abs() < 0.00001);
        s.toggle();
        let p = s.position;
        s.movement = Vec2::ONE;
        s.advance(10.);
        assert_eq!(s.position, p);
    }
}
