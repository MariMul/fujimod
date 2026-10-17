//! Network-only timing: USB execution is never included in RTT samples.
use std::{collections::VecDeque, time::Duration};

#[cfg(not(target_arch = "wasm32"))]
pub(crate) struct Clock(std::time::Instant);
#[cfg(target_arch = "wasm32")]
pub(crate) struct Clock(f64);
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(js_namespace = performance, js_name = now)]
    fn now() -> f64;
}
impl Clock {
    pub fn new() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        {
            Self(std::time::Instant::now())
        }
        #[cfg(target_arch = "wasm32")]
        {
            Self(now())
        }
    }
    pub fn elapsed(&self) -> Duration {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.0.elapsed()
        }
        #[cfg(target_arch = "wasm32")]
        {
            Duration::from_secs_f64(((now() - self.0) / 1000.0).max(0.0))
        }
    }
}

#[derive(Default)]
pub(crate) struct Latency {
    samples: VecDeque<Duration>,
}
impl Latency {
    pub fn record(&mut self, sample: Duration) {
        if self.samples.len() == 8 {
            self.samples.pop_front();
        }
        self.samples.push_back(sample);
    }
    pub fn allowance(&self) -> Duration {
        if self.samples.is_empty() {
            return Duration::from_secs(2);
        }
        let mean = self.samples.iter().sum::<Duration>() / self.samples.len() as u32;
        let deviation = self
            .samples
            .iter()
            .map(|sample| sample.abs_diff(mean))
            .sum::<Duration>()
            / self.samples.len() as u32;
        mean + (deviation * 4).max(Duration::from_millis(100))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mean_jitter_and_recent_window() {
        let mut latency = Latency::default();
        assert_eq!(latency.allowance(), Duration::from_secs(2));
        for _ in 0..8 {
            latency.record(Duration::from_millis(150));
        }
        assert_eq!(latency.allowance(), Duration::from_millis(250));
        latency.record(Duration::from_millis(950));
        assert_eq!(latency.allowance(), Duration::from_millis(950));
        for _ in 0..8 {
            latency.record(Duration::from_millis(20));
        }
        assert_eq!(latency.allowance(), Duration::from_millis(120));
    }
}
