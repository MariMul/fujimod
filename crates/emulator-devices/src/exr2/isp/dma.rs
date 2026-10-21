//! Shared image DMA: bounded, nonoverlapping addressed rows, then status,
//! then the physical IRQ. No host read, digest or instruction drives effects.
use super::{Irq, due};
use crate::Cx;
use serde::{Deserialize, Serialize};

/// An explicit transfer clock, independent of the external frame cadence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Clock {
    Fixed { ticks: u64 },
    Throughput { bytes_per_tick: u64 },
}
impl Clock {
    pub fn delay(self, bytes: usize) -> Result<u64, String> {
        let ticks = match self {
            Self::Fixed { ticks } => ticks,
            Self::Throughput { bytes_per_tick } if bytes_per_tick != 0 => {
                (bytes as u64).div_ceil(bytes_per_tick).max(1)
            }
            _ => 0,
        };
        if ticks == 0 {
            Err("image DMA requires a positive transfer clock".into())
        } else {
            Ok(ticks)
        }
    }
}

pub const MAX_BYTES: usize = 64 * 1024 * 1024;

/// Normalize the uncached DRAM alias within its 30-bit aperture and reject
/// vector/ROM space. The runtime bus enforces the selected body's backing;
/// a shared image engine must not impose a smaller body's DRAM population.
pub fn physical(address: u64, count: usize) -> Result<u64, String> {
    let start = match address {
        0x40000000..0x80000000 => address - 0x40000000,
        _ => address,
    };
    if count == 0
        || count > MAX_BYTES
        || start < 0x4000
        || start
            .checked_add(count as u64)
            .is_none_or(|end| end > 0x40000000)
    {
        return Err("image DMA span escapes bounded DRAM".into());
    }
    Ok(start)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Raster {
    pub address: u64,
    pub width: usize,
    pub height: usize,
    pub pitch: usize,
}
impl Raster {
    pub fn validate(self, row_bytes: usize) -> Result<(), String> {
        if self.width == 0
            || self.width > 8192
            || self.height == 0
            || self.height > 8192
            || row_bytes == 0
            || self.pitch < row_bytes
            || self.pitch > MAX_BYTES
        {
            return Err("image DMA raster geometry is unsupported".into());
        }
        physical(
            self.address,
            self.pitch
                .checked_mul(self.height)
                .ok_or("image DMA size overflow")?,
        )?;
        Ok(())
    }
    pub fn overlaps(self, other: Self) -> Result<bool, String> {
        let a = physical(self.address, self.pitch * self.height)?;
        let b = physical(other.address, other.pitch * other.height)?;
        Ok(
            a < b + (other.pitch * other.height) as u64
                && b < a + (self.pitch * self.height) as u64,
        )
    }
    /// Compare addressed active bytes, excluding pitch padding on both sides.
    pub fn active_rows_overlap(
        self,
        row_bytes: usize,
        other: Self,
        other_row_bytes: usize,
    ) -> Result<bool, String> {
        self.validate(row_bytes)?;
        other.validate(other_row_bytes)?;
        let a = physical(self.address, self.pitch * self.height)?;
        let b = physical(other.address, other.pitch * other.height)?;
        let (mut i, mut j) = (0, 0);
        while i < self.height && j < other.height {
            let start_a = a + (i * self.pitch) as u64;
            let start_b = b + (j * other.pitch) as u64;
            if start_a < start_b + other_row_bytes as u64 && start_b < start_a + row_bytes as u64 {
                return Ok(true);
            }
            if start_a + row_bytes as u64 <= start_b {
                i += 1;
            } else {
                j += 1;
            }
        }
        Ok(false)
    }
}

/// Reused output storage. The owning model prepares pixels at START, writes
/// active rows only at `deadline`, publishes status, then calls `interrupt`.
#[derive(Default, Serialize, Deserialize)]
pub struct Transfer {
    #[serde(with = "crate::checkpoint::bytes")]
    pub pixels: Vec<u8>,
    raster: Option<Raster>,
    row_bytes: usize,
    deadline: Option<u64>,
}
impl Transfer {
    pub fn fingerprint(&self) -> serde_json::Value {
        let Self {
            pixels,
            raster,
            row_bytes,
            deadline,
        } = self;
        serde_json::json!({"pixels":crate::fingerprint::bytes(pixels),"raster":raster,
            "row_bytes":row_bytes,"deadline":deadline})
    }
    pub fn prepare(
        &mut self,
        now: u64,
        delay: u64,
        raster: Raster,
        row_bytes: usize,
    ) -> Result<(), String> {
        if self.deadline.is_some() {
            return Err("image DMA is already pending".into());
        }
        raster.validate(row_bytes)?;
        let deadline = due(now, delay)?;
        self.pixels.resize(row_bytes * raster.height, 0);
        self.raster = Some(raster);
        self.row_bytes = row_bytes;
        self.deadline = Some(deadline);
        Ok(())
    }
    pub fn deadline(&self) -> Option<u64> {
        self.deadline
    }
    pub fn ready(&self, now: u64) -> bool {
        self.deadline.is_some_and(|at| now >= at)
    }
    pub fn cancel(&mut self) {
        self.deadline = None;
        self.raster = None;
        self.row_bytes = 0;
        // Completed bytes live in guest RAM. Keep allocation capacity for the
        // next transfer without retaining a second full image in saved state.
        self.pixels.clear();
    }
    pub fn write(&mut self, cx: &mut Cx<'_>) -> Result<bool, String> {
        if !self.ready(cx.now()) {
            return Ok(false);
        }
        let r = self.raster.ok_or("image DMA has no destination")?;
        // Every physical row span and nonoverlap were checked at preparation;
        // the runtime bus additionally enforces the selected body backing.
        if self.row_bytes == r.pitch {
            write(cx, r.address, &self.pixels)?;
        } else {
            for y in 0..r.height {
                write(
                    cx,
                    r.address + (y * r.pitch) as u64,
                    &self.pixels[y * self.row_bytes..(y + 1) * self.row_bytes],
                )?;
            }
        }
        self.cancel();
        Ok(true)
    }
    pub fn interrupt(cx: &mut Cx<'_>, irq: Irq) {
        if let Err(e) = irq.raise(cx) {
            cx.fail(e);
        }
    }
}

use std::sync::{Arc, Mutex};
/// The interrupt controller publishes its register-derived owner. None
/// means disabled, ambiguous or unsupported routing. Graph wiring must wake
/// consumers when a route changes, as well as when their source edge changes.
#[derive(Clone)]
pub struct IrqRoutes(Arc<Mutex<Routing>>);
#[derive(serde::Serialize, serde::Deserialize)]
struct Routing {
    #[serde(with = "crate::checkpoint::array")]
    owners: [Option<usize>; 128],
    enabled: [[u32; 4]; 2],
}
impl Default for IrqRoutes {
    fn default() -> Self {
        let result = Self(Arc::new(Mutex::new(Routing {
            owners: [None; 128],
            enabled: [[0; 4]; 2],
        })));
        drop(result.0.lock().unwrap());
        result
    }
}
impl IrqRoutes {
    pub fn checkpoint(&self) -> Result<crate::checkpoint::Checkpoint, String> {
        crate::checkpoint::Checkpoint::encode(
            "image-irq-routes",
            &*self.0.lock().map_err(|_| "image-irq-routes poisoned")?,
        )
    }
    pub fn validate_checkpoint(&self, saved: &crate::checkpoint::Checkpoint) -> Result<(), String> {
        let _: Routing = saved.decode("image-irq-routes")?;
        Ok(())
    }
    pub fn restore_checkpoint(&self, saved: &crate::checkpoint::Checkpoint) -> Result<(), String> {
        *self.0.lock().map_err(|_| "image-irq-routes poisoned")? =
            saved.decode::<Routing>("image-irq-routes")?;
        Ok(())
    }

    pub fn fingerprint(&self) -> serde_json::Value {
        let state = self.0.lock().unwrap();
        serde_json::json!({"owners":state.owners.as_slice(),"enabled":state.enabled})
    }
    pub fn set(&self, line: u32, owner: Option<usize>) -> Result<(), String> {
        if line >= 128 || owner.is_some_and(|v| v > 1) {
            return Err("invalid image IRQ route".into());
        }
        let mut routes = self.0.lock().unwrap();
        routes.owners[line as usize] = owner;
        for core in 0..2 {
            let word = &mut routes.enabled[core][line as usize / 32];
            *word = (*word & !(1 << (line % 32))) | (u32::from(owner == Some(core)) << (line % 32));
        }
        Ok(())
    }
    pub fn owner(&self, line: u32) -> Option<usize> {
        self.0
            .lock()
            .unwrap()
            .owners
            .get(line as usize)
            .copied()
            .flatten()
    }
    pub fn latch(&self, line: u32) -> Result<Irq, String> {
        Ok(Irq {
            line,
            core: self
                .owner(line)
                .ok_or("image DMA needs one programmed IRQ owner")?,
        })
    }
    pub fn validate(&self, route: Irq) -> Result<(), String> {
        if self.owner(route.line) != Some(route.core) {
            return Err("image DMA IRQ owner changed during transfer".into());
        }
        Ok(())
    }
}

/// One controller's physical enable-state output, joined with its peer's
/// output. Ambiguous or disabled source ownership remains unavailable.
pub struct RouteOutput {
    routes: IrqRoutes,
    core: usize,
}
impl RouteOutput {
    pub fn new(routes: IrqRoutes, core: usize) -> Result<Self, String> {
        if core > 1 {
            return Err("image route output targets an absent EXR II controller".into());
        }
        Ok(Self { routes, core })
    }
}
impl crate::exr2::irq::EnableOutput for RouteOutput {
    fn publish(&self, enabled: [u32; 4]) {
        let mut routes = self.routes.0.lock().unwrap();
        if routes.enabled[self.core] == enabled {
            return;
        }
        routes.enabled[self.core] = enabled;
        for line in 0..128 {
            let bit = 1 << (line % 32);
            routes.owners[line] = match (
                routes.enabled[0][line / 32] & bit != 0,
                routes.enabled[1][line / 32] & bit != 0,
            ) {
                (true, false) => Some(0),
                (false, true) => Some(1),
                _ => None,
            };
        }
    }
}

/// Physical bounds used to reject overlap before issuing a multi-span job.
pub fn span(address: u64, count: usize) -> Result<std::ops::Range<u64>, String> {
    let start = physical(address, count)?;
    Ok(start..start + count as u64)
}
/// Write one preflighted span. Device status and IRQ publication follow all
/// writes in the owning controller's completion, including a paired batch.
pub fn write(cx: &mut Cx<'_>, address: u64, bytes: &[u8]) -> Result<(), String> {
    physical(address, bytes.len())?;
    cx.ram_write(address, bytes)
}

#[cfg(test)]
mod fingerprint_tests {
    use super::*;
    #[test]
    fn active_row_aliases_distinguish_interleaved_padding_and_later_row_collisions() {
        let source = Raster {
            address: 0x10000,
            width: 368,
            height: 240,
            pitch: 1440,
        };
        let output = Raster {
            address: 0x400102e0,
            width: 192,
            height: 120,
            pitch: 1440,
        };
        assert!(source.overlaps(output).unwrap());
        assert!(!source.active_rows_overlap(736, output, 384).unwrap());
        assert!(!output.active_rows_overlap(384, source, 736).unwrap());
        assert!(
            source
                .active_rows_overlap(
                    736,
                    Raster {
                        address: output.address - 1,
                        ..output
                    },
                    384
                )
                .unwrap()
        );
        // First rows are disjoint; different pitches collide on a later row.
        assert!(
            source
                .active_rows_overlap(
                    736,
                    Raster {
                        pitch: 1424,
                        ..output
                    },
                    384
                )
                .unwrap()
        );
        assert!(
            source
                .active_rows_overlap(
                    736,
                    Raster {
                        height: 0,
                        ..output
                    },
                    384
                )
                .is_err()
        );
        assert!(source.active_rows_overlap(1441, output, 384).is_err());
    }
    #[test]
    fn pending_image_identity_is_bounded_and_preserves_payload_and_deadline() {
        let mut transfer = Transfer::default();
        transfer
            .prepare(
                10,
                4,
                Raster {
                    address: 0x10000,
                    width: 1024,
                    height: 2048,
                    pitch: 2048,
                },
                2048,
            )
            .unwrap();
        transfer.pixels.fill(0xa5);
        let identity = transfer.fingerprint();
        assert!(serde_json::to_vec(&identity).unwrap().len() < 500);
        let saved = crate::checkpoint::Checkpoint::encode("transfer", &transfer).unwrap();
        let mut restored: Transfer = saved.decode("transfer").unwrap();
        assert_eq!(restored.fingerprint(), identity);
        *restored.pixels.last_mut().unwrap() ^= 1;
        assert_ne!(restored.fingerprint(), identity);
        assert_eq!(*transfer.pixels.last().unwrap(), 0xa5);
        restored = saved.decode("transfer").unwrap();
        restored.deadline = Some(15);
        assert_ne!(restored.fingerprint(), identity);
        assert_eq!(transfer.fingerprint(), identity);
    }
}
