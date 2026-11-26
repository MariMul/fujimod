//! The native session behind the emulator UI.
//!
//! A camera is served in a share [`Room`]: every controller, the desktop
//! window included, joins it over WebRTC, sends commands on a data channel
//! and receives the LCD as video. Guest execution stays on the host's session
//! thread; a [`Viewer`] applies the controllers' input to any [`Run`] between
//! guest slices.

pub mod launch;
mod protocol;
mod server;
mod usb;

pub use server::{Endpoint, Room, Run, Viewer, serve};

use fujimod_emulator_boot::xp5::RunHost;

impl Run for RunHost {
    fn session(&self) -> &fujimod_emulator_session::Session {
        RunHost::session(self)
    }
    fn session_mut(&mut self) -> &mut fujimod_emulator_session::Session {
        RunHost::session_mut(self)
    }
    fn step(&mut self) -> Result<(), String> {
        RunHost::step(self).map(drop)
    }
    fn settle(&mut self) -> Result<(), String> {
        RunHost::step(self).map(drop)
    }
    fn frames(
        &mut self,
    ) -> Result<Vec<fujimod_emulator_devices::graphics::scanout::Frame>, String> {
        Ok(self.take_frames())
    }
    fn finish(self) -> Result<serde_json::Value, String> {
        RunHost::finish(self, Ok(()))
    }
}

/// Shortest wall time between LCD reads, about 30 images a second.
const FRAME_INTERVAL: std::time::Duration = std::time::Duration::from_millis(33);

/// How fast a live camera's guest time runs against the host's wall time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Speed {
    /// Guest seconds per wall second; 1 is real time.
    Paced(f64),
    /// As fast as the host runs the guest.
    Unpaced,
}

impl Speed {
    /// Guest time follows wall time, as on the camera.
    pub const REAL_TIME: Self = Self::Paced(1.0);

    /// `max` for unpaced, or a positive factor of real time such as `1`, `0.5` or `4`.
    pub fn parse(text: &str) -> Result<Self, String> {
        if text == "max" {
            return Ok(Self::Unpaced);
        }
        match text.parse::<f64>() {
            Ok(factor) if factor.is_finite() && factor > 0.0 => Ok(Self::Paced(factor)),
            _ => Err(format!(
                "--speed must be a positive factor of real time or max, not {text}"
            )),
        }
    }
}

/// Wall time a paced run waits at most before its window services input again.
const PACE_WAIT: std::time::Duration = std::time::Duration::from_millis(4);

/// The wall instant at which a paced run's guest clock read `ticks`.
struct Pace {
    wall: std::time::Instant,
    ticks: u64,
}

/// An opened camera of any processor, run live for its window.
pub struct Live {
    camera: fujimod_emulator_boot::Camera,
    selection: fujimod_catalog::reference::ResolvedSelection,
    read_at: std::time::Instant,
    speed: Speed,
    pace: Option<Pace>,
}

impl Live {
    /// A camera run in real time.
    pub fn new(
        camera: fujimod_emulator_boot::Camera,
        selection: fujimod_catalog::reference::ResolvedSelection,
    ) -> Self {
        Self {
            camera,
            selection,
            read_at: std::time::Instant::now() - FRAME_INTERVAL,
            speed: Speed::REAL_TIME,
            pace: None,
        }
    }

    /// Run at `speed` instead of real time.
    pub fn with_speed(mut self, speed: Speed) -> Self {
        self.speed = speed;
        self.pace = None;
        self
    }

    /// The guest tick the next slice may run to, or `None` while the guest is
    /// ahead of the wall clock. A run that falls more than a quarter second
    /// behind drops that backlog instead of racing to catch up.
    fn paced_until(&mut self, now: u64, slice: u64) -> Option<u64> {
        let Speed::Paced(factor) = self.speed else {
            return Some(now + slice);
        };
        let rate = self.camera.ticks_per_second as f64 * factor;
        let pace = self.pace.get_or_insert_with(|| Pace {
            wall: std::time::Instant::now(),
            ticks: now,
        });
        let earned = pace.ticks + (pace.wall.elapsed().as_secs_f64() * rate) as u64;
        if earned > now + self.camera.ticks_per_second / 4 {
            *pace = Pace {
                wall: std::time::Instant::now(),
                ticks: now,
            };
            return Some(now + slice);
        }
        if earned <= now {
            let ahead = (now + 1 - earned) as f64 / rate;
            std::thread::sleep(PACE_WAIT.min(std::time::Duration::from_secs_f64(ahead)));
            return None;
        }
        Some(earned.min(now + slice))
    }
}

impl Run for Live {
    fn session(&self) -> &fujimod_emulator_session::Session {
        &self.camera.session
    }
    fn session_mut(&mut self) -> &mut fujimod_emulator_session::Session {
        &mut self.camera.session
    }
    fn step(&mut self) -> Result<(), String> {
        // At most one LCD frame of guest time per slice keeps the window responsive.
        let now = self.camera.session.board.now();
        let slice = (self.camera.ticks_per_second / 60).max(1);
        let Some(until) = self.paced_until(now, slice) else {
            return Ok(());
        };
        let session = &mut self.camera.session;
        session.run_window(u64::MAX, self.camera.quantum, None, Some(until))
    }
    fn settle(&mut self) -> Result<(), String> {
        let session = &mut self.camera.session;
        let until = session.board.now() + (self.camera.ticks_per_second / 1000).max(1);
        session.run_window(u64::MAX, self.camera.quantum, None, Some(until))
    }
    fn frames(
        &mut self,
    ) -> Result<Vec<fujimod_emulator_devices::graphics::scanout::Frame>, String> {
        if self.read_at.elapsed() < FRAME_INTERVAL {
            return Ok(Vec::new());
        }
        self.read_at = std::time::Instant::now();
        // The wiring is located once the firmware has loaded its configuration.
        if self.camera.session.controls.is_none() {
            let _ = fujimod_emulator_controls::connect(
                &mut self.camera.session,
                &self.selection,
                self.camera.ticks_per_second,
            );
        }
        fujimod_emulator_boot::lcd_frames(&mut self.camera.session)
    }
    fn finish(self) -> Result<serde_json::Value, String> {
        Ok(serde_json::json!({
            "guest_ticks": self.camera.session.board.now(),
            "instructions": self.camera.session.board.instruction_counts(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::Speed;

    #[test]
    fn speed_is_a_positive_factor_of_real_time_or_max() {
        assert_eq!(Speed::parse("1"), Ok(Speed::REAL_TIME));
        assert_eq!(Speed::parse("0.5"), Ok(Speed::Paced(0.5)));
        assert_eq!(Speed::parse("max"), Ok(Speed::Unpaced));
        for refused in ["0", "-1", "inf", "NaN", "fast", ""] {
            assert!(Speed::parse(refused).is_err(), "{refused}");
        }
    }
}
