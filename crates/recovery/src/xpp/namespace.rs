//! A single, exact source namespace with erased runs represented as constants.
use crate::image::{Result, require};

pub(super) const FILL: usize = 0xffff_ffff;
#[derive(Debug)]
pub(super) struct Range {
    pub offset: usize,
    pub size: usize,
    pub address: usize,
    pub value: u32,
}
pub(super) struct Namespace {
    pub ranges: Vec<Range>,
    pub bytes: Vec<u8>,
    pub logical_size: usize,
}
impl Namespace {
    fn retain(&mut self, offset: usize, data: &[u8], tail: usize) {
        if data.is_empty() {
            return;
        }
        self.ranges.push(Range {
            offset,
            size: data.len(),
            address: tail + self.bytes.len(),
            value: 0,
        });
        self.bytes.extend_from_slice(data);
    }
}
pub(super) fn build(data: &[u8], limit: usize, tail: usize) -> Result<Namespace> {
    let logical_size = data.len().max(limit);
    require(
        logical_size <= 0x8000000 && tail < 0x40000000,
        "Agate source namespace exceeds bounds",
    )?;
    let mut n = Namespace {
        ranges: vec![],
        bytes: vec![],
        logical_size,
    };
    let mut cursor = 0;
    let mut at = 0;
    while at < data.len() {
        if data[at] != 255 {
            at += 1;
            continue;
        }
        let start = at;
        while at < data.len() && data[at] == 255 {
            at += 1;
        }
        if at - start < 512 {
            continue;
        }
        n.retain(cursor, &data[cursor..start], tail);
        n.ranges.push(Range {
            offset: start,
            size: at - start,
            address: FILL,
            value: 255,
        });
        cursor = at;
    }
    n.retain(cursor, &data[cursor..], tail);
    if data.len() < logical_size {
        n.ranges.push(Range {
            offset: data.len(),
            size: logical_size - data.len(),
            address: FILL,
            value: 255,
        });
    }
    require(
        tail.checked_add(n.bytes.len())
            .is_some_and(|end| end < 0x40000000),
        "Agate compact source escapes RAM",
    )?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn interior_erased_runs_and_padding_round_trip_without_a_second_source_copy() {
        let mut data = vec![17; 73];
        data.extend([255; 512]);
        data.extend([29; 23]);
        data.extend([255; 511]);
        data.extend([42; 11]);
        data.extend([255; 4096]);
        data.extend([9; 7]);
        let n = build(&data, data.len() + 101, 0x4000).unwrap();
        assert_eq!(n.bytes.len(), data.len() - 512 - 4096);
        let mut actual = Vec::new();
        for r in n.ranges {
            assert_eq!(r.offset, actual.len());
            if r.address == FILL {
                actual.extend(std::iter::repeat_n(r.value as u8, r.size));
            } else {
                actual.extend_from_slice(&n.bytes[r.address - 0x4000..r.address - 0x4000 + r.size]);
            }
        }
        data.extend([255; 101]);
        assert_eq!(actual, data);
    }
}
