use crate::{Frame, Result, badges, video::Encoders};
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    time::Instant,
};

#[derive(Default, Clone, Serialize)]
pub struct Metrics {
    pub samples: u64,
    pub changed_frames: u64,
    pub annotated_frames: u64,
    pub written_frames: u64,
    pub png_seconds: f64,
    pub annotation_seconds: f64,
    pub encoder_write_seconds: [f64; 2],
    pub encoder_finish_seconds: f64,
}
struct Badge {
    name: String,
    released: Option<u64>,
}
pub struct Recorder {
    output: PathBuf,
    pub fps: u32,
    pub video: bool,
    pub idle_limit: f64,
    pub skipped_seconds: f64,
    pub frames: u64,
    pub metrics: Metrics,
    last_time: Option<f64>,
    last_sample: Option<f64>,
    last: Option<Frame>,
    fractional: f64,
    hold_frames: f64,
    idle_kept: f64,
    latest_shown: bool,
    finished: bool,
    badges: Vec<Badge>,
    encoders: Option<Encoders>,
    size: Option<(u32, u32)>,
    fitted: Option<Frame>,
    annotated: Vec<u8>,
    labels: Vec<(String, u8)>,
    has_poster: bool,
}
pub fn seconds(value: f64) -> Result<f64> {
    if value.is_finite() && value >= 0. {
        Ok(value)
    } else {
        Err("Time must be a finite nonnegative number of seconds".into())
    }
}
impl Recorder {
    pub fn new(output: PathBuf, fps: u32, video: bool) -> Result<Self> {
        if !(1..=60).contains(&fps) {
            return Err("fps must be 1..60".into());
        }
        if video {
            Encoders::check()?;
        }
        Ok(Self {
            output,
            fps,
            video,
            idle_limit: 0.4,
            skipped_seconds: 0.,
            frames: 0,
            metrics: Metrics::default(),
            last_time: None,
            last_sample: None,
            last: None,
            fractional: 0.,
            hold_frames: 0.,
            idle_kept: 0.,
            latest_shown: false,
            finished: false,
            badges: Vec::new(),
            encoders: None,
            size: None,
            fitted: None,
            annotated: Vec::new(),
            labels: Vec::new(),
            has_poster: false,
        })
    }
    pub fn seconds(&self) -> f64 {
        self.frames as f64 / f64::from(self.fps)
    }
    pub fn has_poster(&self) -> bool {
        self.has_poster
    }
    pub fn due(&self, time: f64, force: bool) -> Result<bool> {
        seconds(time)?;
        if self.finished {
            return Err("Recording is already finished".into());
        }
        Ok((self.video || !self.has_poster)
            && (force
                || self
                    .last_sample
                    .is_none_or(|last| time - last >= 1. / f64::from(self.fps))))
    }
    fn advance(&mut self, time: f64) -> Result<()> {
        seconds(time)?;
        if self.finished {
            return Err("Recording is already finished".into());
        }
        if let Some(last) = self.last_time {
            if time < last {
                return Err("Frame and input timestamps must be monotonic".into());
            }
            if self.last.is_some() {
                let elapsed = time - last;
                let duration = if self.badges.is_empty() {
                    elapsed.min((self.idle_limit - self.idle_kept).max(0.))
                } else {
                    elapsed
                };
                if self.badges.is_empty() {
                    self.idle_kept += duration;
                }
                self.skipped_seconds += elapsed - duration;
                self.fractional += duration * f64::from(self.fps);
                let count = (self.fractional + 1e-8).floor() as u64;
                self.fractional -= count as f64;
                for _ in 0..count {
                    self.write()?;
                }
                if count > 0 {
                    self.latest_shown = true;
                }
            }
        }
        self.last_time = Some(time);
        Ok(())
    }
    fn select(&mut self, frame: Frame) -> bool {
        if !self
            .last
            .as_ref()
            .is_some_and(|last| last.same_pixels(&frame))
        {
            self.latest_shown = false;
            self.fitted = None;
            self.metrics.changed_frames += 1;
            self.last = Some(frame);
            return true;
        }
        false
    }
    pub fn frame(&mut self, frame: Frame, time: f64) -> Result<()> {
        self.advance(time)?;
        if !self.has_poster {
            let start = Instant::now();
            frame.save(&self.output.join("poster.png"))?;
            self.metrics.png_seconds += start.elapsed().as_secs_f64();
            self.has_poster = true;
        }
        if self.select(frame) {
            self.idle_kept = 0.;
        }
        self.last_sample = Some(time);
        self.metrics.samples += 1;
        Ok(())
    }
    pub fn input(&mut self, name: &str, pressed: bool, time: f64) -> Result<()> {
        badges::check(name)?;
        self.advance(time)?;
        self.idle_kept = 0.;
        if pressed {
            self.badges.push(Badge {
                name: name.into(),
                released: None,
            });
            if self.badges.len() > 5 {
                self.badges.remove(0);
            }
        } else if let Some(badge) = self
            .badges
            .iter_mut()
            .rev()
            .find(|b| b.name == name && b.released.is_none())
        {
            badge.released = Some(self.frames);
        }
        Ok(())
    }
    pub fn still(&mut self, frame: Frame, duration: f64) -> Result<()> {
        seconds(duration)?;
        if self.finished {
            return Err("Recording is already finished".into());
        }
        self.select(frame);
        self.hold_frames += duration * f64::from(self.fps);
        let count = (self.hold_frames + 1e-8).floor() as u64;
        self.hold_frames -= count as f64;
        for _ in 0..count {
            self.write()?;
        }
        if count > 0 {
            self.latest_shown = true;
            self.idle_kept = self.idle_limit;
        }
        Ok(())
    }
    pub fn screenshot(&mut self, frame: &Frame, name: &str) -> Result<String> {
        let relative = Path::new(name);
        if relative.is_absolute()
            || relative
                .components()
                .any(|c| !matches!(c, std::path::Component::Normal(_)))
        {
            return Err("Screenshot path must stay inside the recording directory".into());
        }
        let path = self.output.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let start = Instant::now();
        let hash = frame.save(&path)?;
        self.metrics.png_seconds += start.elapsed().as_secs_f64();
        Ok(hash)
    }
    fn current_labels(&mut self) -> Vec<(String, u8)> {
        let lifetime = 5 * u64::from(self.fps);
        let fade = badges::fade_frames(self.fps);
        self.badges
            .retain(|b| b.released.is_none_or(|at| self.frames - at < lifetime));
        self.badges
            .iter()
            .map(|b| {
                (
                    b.name.clone(),
                    b.released.map_or(255, |at| {
                        (255. * ((lifetime - (self.frames - at)) as f64 / fade as f64).min(1.))
                            .round() as u8
                    }),
                )
            })
            .collect()
    }
    fn write(&mut self) -> Result<()> {
        if !self.video {
            self.frames += 1;
            return Ok(());
        }
        let frame = self.last.as_ref().ok_or("No LCD frames recorded")?;
        let size = *self.size.get_or_insert((frame.width(), frame.height()));
        let changed = self.fitted.is_none();
        if changed {
            self.fitted = Some(frame.fit(size.0, size.1)?);
        }
        let labels = self.current_labels();
        let fitted = self.fitted.as_ref().unwrap();
        if changed || labels != self.labels {
            let start = Instant::now();
            self.annotated = badges::draw(fitted, &labels);
            self.metrics.annotation_seconds += start.elapsed().as_secs_f64();
            self.metrics.annotated_frames += 1;
            self.labels = labels;
        }
        if self.encoders.is_none() {
            self.encoders = Some(Encoders::open(&self.output, fitted, self.fps)?);
        }
        let elapsed = self
            .encoders
            .as_mut()
            .unwrap()
            .write(&self.annotated, fitted.pixels())?;
        for (total, part) in self.metrics.encoder_write_seconds.iter_mut().zip(elapsed) {
            *total += part;
        }
        self.frames += 1;
        self.metrics.written_frames += 1;
        Ok(())
    }
    pub fn finish(&mut self) -> Result<()> {
        if self.finished {
            return Ok(());
        }
        let result = (|| {
            if self.last.is_some() && !self.latest_shown {
                self.write()?;
            }
            if self.video && self.encoders.is_none() {
                return Err("No LCD frames recorded".into());
            }
            Ok(())
        })();
        self.finished = true;
        let start = Instant::now();
        let encoders = self.encoders.as_mut().map_or(Ok(()), Encoders::finish);
        self.metrics.encoder_finish_seconds += start.elapsed().as_secs_f64();
        result.and(encoders)
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
