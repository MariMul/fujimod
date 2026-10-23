//! Retained R-profile counter law: the cycle counter advances on the explicitly
//! supplied guest clock, gated by PMCR.E and PMCNTENSET.C. No host clock or
//! instruction-observation cadence contributes to it.
use super::*;
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Counter {
    control: u32,
    enabled: u32,
    frozen: u64,
    start: u64,
}

pub(super) fn selected(r: arm::Coprocessor) -> bool {
    r.cp == 15
        && ((r.wide && r.crm == 9)
            || (r.opc1 == 0
                && ((r.crn == 9 && (12..=14).contains(&r.crm))
                    || (r.crn == 14 && (8..=15).contains(&r.crm)))))
}
impl Counter {
    pub(super) fn fingerprint(self) -> serde_json::Value {
        serde_json::json!({"control":self.control,"enabled":self.enabled,"frozen":self.frozen,"start":self.start})
    }
    fn check(crm: u32, opc2: u32) -> Result<(), String> {
        if matches!((crm, opc2), (12, 0..=2) | (13, 0)) {
            Ok(())
        } else {
            Err("unmodeled R-profile PMU selector".into())
        }
    }
    fn count(self, now: u64) -> u64 {
        if self.control & 1 != 0 && self.enabled & 0x80000000 != 0 {
            now.wrapping_sub(self.start)
        } else {
            self.frozen
        }
    }
    fn access(&mut self, crm: u32, opc2: u32, value: Option<u32>, now: u64) -> Result<u32, String> {
        Self::check(crm, opc2)?;
        let count = self.count(now);
        if let Some(value) = value {
            match (crm, opc2) {
                (12, 0) => {
                    self.frozen = count;
                    self.start = now.wrapping_sub(count);
                    self.control = value & !6;
                    if value & 4 != 0 {
                        self.start = now;
                        self.frozen = 0;
                    }
                }
                (12, 1) => {
                    self.start = now.wrapping_sub(count);
                    self.enabled |= value;
                }
                (12, 2) => {
                    self.frozen = count;
                    self.enabled &= !value;
                }
                (13, 0) => {
                    self.frozen = value as u64;
                    self.start = now.wrapping_sub(value as u64);
                }
                _ => unreachable!(),
            }
            Ok(0)
        } else {
            Ok(if crm == 13 {
                count as u32
            } else if opc2 == 0 {
                self.control
            } else {
                self.enabled
            })
        }
    }
}
impl Execution {
    pub(super) fn pmu_access(
        &self,
        crm: u32,
        opc2: u32,
        value: Option<u32>,
    ) -> Result<u32, String> {
        Counter::check(crm, opc2)?;
        let clock = self
            .clock
            .get()
            .ok_or("R-profile PMU requires an explicit guest clock")?;
        let now = clock
            .after(if self.active.get() {
                self.progress()
                    .ok_or("missing R-profile PMU instruction progress")?
            } else {
                0
            })?
            .now;
        let mut counter = self.pmu.get();
        let result = counter.access(crm, opc2, value, now)?;
        if value.is_some() {
            self.pmu.set(counter);
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn register_gates_reset_write_and_wrap_follow_only_the_supplied_clock() {
        let mut c = Counter::default();
        assert_eq!(c.access(13, 0, None, 100).unwrap(), 0);
        c.access(12, 0, Some(1), 100).unwrap();
        c.access(12, 1, Some(0x80000000), 100).unwrap();
        assert_eq!(c.access(13, 0, None, 125).unwrap(), 25);
        c.access(12, 2, Some(0x80000000), 125).unwrap();
        assert_eq!(c.access(13, 0, None, 1000).unwrap(), 25);
        c.access(12, 1, Some(0x80000000), 1000).unwrap();
        assert_eq!(c.access(13, 0, None, 1010).unwrap(), 35);
        c.access(12, 0, Some(5), 1010).unwrap();
        assert_eq!(c.access(12, 0, None, 1010).unwrap(), 1);
        assert_eq!(c.access(13, 0, None, 1011).unwrap(), 1);
        c.access(13, 0, Some(u32::MAX), 1011).unwrap();
        assert_eq!(c.access(13, 0, None, 1013).unwrap(), 1);
        let before = c;
        assert!(c.access(12, 3, Some(0), 1013).is_err());
        assert_eq!(c, before);
        c.access(13, 0, None, 1013).unwrap();
        assert_eq!(c, before);
    }
}
