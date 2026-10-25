//! Catalog-selected camera state through the qualified stock bulk reader.
use crate::{Limits, bulk};
use fujimod_catalog::capture::{FirmwareState, StateRegion};
use fujimod_catalog_bulk::{BulkRange, FirmwareBulk};
use fujimod_usb::{ff80::Session, operations::checks::sha256_bytes};

#[derive(Debug)]
pub struct CapturedRegion {
    pub region: StateRegion,
    pub data: Vec<u8>,
    pub sha256: String,
}

/// Capture each region in catalog order. Constant regions are compared before
/// delivery. All-zero or all-FF constants are masked and omitted. A sink error
/// stops the capture and still restores the borrowed bulk mapping.
pub async fn dump(
    session: &mut Session,
    bulk: &FirmwareBulk,
    state: &FirmwareState,
    limits: Limits,
    sink: &mut dyn FnMut(CapturedRegion) -> std::result::Result<(), String>,
) -> bulk::Result<()> {
    state.validate().map_err(bulk::BulkError::Check)?;
    if state.firmware_id != bulk.firmware_id {
        return Err(bulk::BulkError::Check(
            "State and bulk firmware differ".into(),
        ));
    }
    // The state catalog explicitly authorizes additional apertures, including
    // boot ROM outside DRAM. Do not infer readability from a bulk capacity.
    let mut facts = bulk.clone();
    let mut start = facts.window.address;
    let mut end = start + facts.window.size;
    for region in &state.regions {
        let range = BulkRange {
            address: region.address,
            size: region.size,
        };
        super::bulk::memory_range(range)?;
        start = start.min(region.address);
        end = end.max(region.address + region.size);
    }
    facts.window = BulkRange {
        address: start,
        size: end - start,
    };
    super::bulk::memory_range(facts.window)?;
    let mut reader = bulk::Qualified::prepare(session, &facts, limits).await?;
    let outcome = async {
        for region in &state.regions {
            let range = BulkRange {
                address: region.address,
                size: region.size,
            };
            let data = reader.read(range).await?;
            if region.constant {
                if reader.read(range).await? != data {
                    return Err(bulk::BulkError::Check(format!(
                        "Constant state region {} changed between reads",
                        region.name
                    )));
                }
                if masked(&data) {
                    continue;
                }
            }
            let sha256 = super::bulk::hex(&sha256_bytes(&data));
            sink(CapturedRegion {
                region: region.clone(),
                data,
                sha256,
            })
            .map_err(bulk::BulkError::Check)?;
        }
        Ok(())
    }
    .await;
    let restored = reader.restore().await;
    bulk::finish(outcome, restored)
}
fn masked(bytes: &[u8]) -> bool {
    bytes.iter().all(|&b| b == 0) || bytes.iter().all(|&b| b == 0xff)
}

#[cfg(test)]
mod tests;
