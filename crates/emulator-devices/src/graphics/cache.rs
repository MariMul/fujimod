//! Process-local rendering and receipt caches; never device state.
use super::{batch, render};
use std::{cell::RefCell, collections::HashMap, rc::Rc};
// Coverage of recent draws, keyed on every draw field except the paint ones
// (color, gradient, texture, blend, sequence), which coverage never reads.
type CoverageCache = (HashMap<Vec<u8>, Rc<render::Coverage>>, usize);
thread_local! {
    static COVERAGE: RefCell<CoverageCache> = RefCell::new((HashMap::new(), 0));
}
const COVERAGE_BUDGET: usize = 16 << 20;

pub fn coverage_key(draw: &batch::Draw) -> Vec<u8> {
    let mut key = Vec::with_capacity(64 + draw.geometry.len() * 40);
    let mut f = |v: f64| key.extend(v.to_bits().to_le_bytes());
    let (a, w, h, o) = draw.target;
    for m in draw.matrix {
        f(m);
    }
    f(draw.radius);
    for (code, flags, values) in &draw.geometry {
        f(*code as f64);
        f(*flags as f64);
        for v in values {
            f(*v);
        }
    }
    for (x0, y0, x1, y1) in &draw.clips {
        for v in [x0, y0, x1, y1] {
            key.extend(v.to_le_bytes());
        }
    }
    key.extend(a.to_le_bytes());
    key.extend(
        [w, h, o, draw.samples, draw.cap, draw.join]
            .iter()
            .flat_map(|v| v.to_le_bytes()),
    );
    key.extend([
        draw.nonzero as u8,
        draw.stroke as u8,
        draw.device_radius as u8,
        draw.geometry.len() as u8,
    ]);
    key.extend((draw.geometry.len() as u64).to_le_bytes());
    key
}

pub fn cached_coverage(draw: &batch::Draw) -> crate::record::Result<Rc<render::Coverage>> {
    let key = coverage_key(draw);
    if let Some(hit) = COVERAGE.with(|c| c.borrow().0.get(&key).cloned()) {
        return Ok(hit);
    }
    let cover = Rc::new(render::coverage(draw)?);
    COVERAGE.with(|c| {
        let mut cache = c.borrow_mut();
        let size = cover.values.len() * 8 + key.len();
        if size > COVERAGE_BUDGET {
            return;
        }
        if cache.1 + size > COVERAGE_BUDGET {
            cache.0.clear();
            cache.1 = 0;
        }
        cache.1 += size;
        cache.0.insert(key, cover.clone());
    });
    Ok(cover)
}

pub fn draws_key(draws: &[batch::Draw]) -> Vec<u8> {
    let mut key = Vec::new();
    for draw in draws {
        key.extend(coverage_key(draw));
        key.extend(draw.color);
        key.extend([draw.blend as u8]);
        key.extend(draw.sequence.to_le_bytes());
        if let Some(t) = &draw.texture {
            key.extend(t.source.to_le_bytes());
            key.extend([t.width, t.height].iter().flat_map(|v| v.to_le_bytes()));
            key.extend(
                [t.valid_width, t.valid_height]
                    .iter()
                    .flat_map(|v| v.to_le_bytes()),
            );
            key.extend(t.matrix.iter().flat_map(|v| v.to_bits().to_le_bytes()));
        }
        if let Some(g) = &draw.gradient {
            let (a, b, c, d) = g.coefficients;
            key.extend([a, b, c, d].iter().flat_map(|v| v.to_bits().to_le_bytes()));
            key.extend(g.colors.iter().flatten());
            key.extend([g.spread as u8, g.premultiplied as u8]);
            if let Some(radial) = &g.radial {
                key.push(0xfe);
                key.extend(radial.quadratics.iter().flat_map(|v| v.to_bits().to_le_bytes()));
                key.extend([radial.zero_x as u8, radial.zero_y as u8, radial.negative_denominator as u8]);
            }
        }
        key.push(0xff);
    }
    key
}

// GPU receipts repeatedly name immutable DMA snapshots. This cache changes
// only host work: every lookup compares the complete bytes, and the returned
// digest is always the standard SHA-256. It is never modeled/checkpoint state.
const DIGEST_BUDGET: usize = 8 << 20;
const DIGEST_ENTRIES: usize = 16;
#[derive(Default)]
struct DigestMemo {
    entries: std::collections::VecDeque<(Box<[u8]>, String)>,
    bytes: usize,
}
thread_local! {
    static DIGESTS: RefCell<DigestMemo> = RefCell::new(DigestMemo::default());
}
pub fn content_sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    DIGESTS.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(index) = cache
            .entries
            .iter()
            .position(|(saved, _)| &**saved == bytes)
        {
            let entry = cache.entries.remove(index).unwrap();
            let digest = entry.1.clone();
            cache.entries.push_back(entry);
            return digest;
        }
        let digest = format!("{:x}", Sha256::digest(bytes));
        if bytes.len() <= DIGEST_BUDGET {
            while cache.entries.len() >= DIGEST_ENTRIES || cache.bytes + bytes.len() > DIGEST_BUDGET
            {
                let (old, _) = cache.entries.pop_front().unwrap();
                cache.bytes -= old.len();
            }
            cache.bytes += bytes.len();
            cache.entries.push_back((bytes.into(), digest.clone()));
        }
        digest
    })
}

#[cfg(test)]
mod digest_tests {
    use super::*;
    use sha2::{Digest, Sha256};
    fn reset() {
        DIGESTS.with(|cache| *cache.borrow_mut() = DigestMemo::default());
    }
    #[test]
    fn complete_bytes_define_digest_in_cold_warm_and_evicted_caches() {
        reset();
        assert_eq!(
            content_sha256(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let mut input = vec![0xa5; 65536];
        let expected = format!("{:x}", Sha256::digest(&input));
        assert_eq!(content_sha256(&input), expected);
        input[65535] ^= 1;
        assert_ne!(content_sha256(&input), expected);
        assert_eq!(
            content_sha256(&input),
            format!("{:x}", Sha256::digest(&input))
        );
        assert_ne!(content_sha256(&input[..65535]), content_sha256(&input));
        input[65535] ^= 1;
        assert_eq!(content_sha256(&input), expected);
        for n in 0..32u8 {
            content_sha256(&[n; 11]);
        }
        assert_eq!(content_sha256(&input), expected);
    }
    #[test]
    fn receipt_cache_bounds_both_payload_bytes_and_entry_count() {
        reset();
        for n in 0..4 {
            content_sha256(&vec![n; 3 << 20]);
        }
        DIGESTS.with(|cache| {
            let cache = cache.borrow();
            assert_eq!(cache.entries.len(), 2);
            assert_eq!(cache.bytes, 6 << 20);
        });
        content_sha256(&vec![42; DIGEST_BUDGET + 1]);
        DIGESTS.with(|cache| assert_eq!(cache.borrow().bytes, 6 << 20));
        for n in 0..32u8 {
            content_sha256(&[n]);
        }
        DIGESTS.with(|cache| {
            let cache = cache.borrow();
            assert_eq!(cache.entries.len(), DIGEST_ENTRIES);
            assert!(cache.bytes <= DIGEST_BUDGET);
        });
    }
}
