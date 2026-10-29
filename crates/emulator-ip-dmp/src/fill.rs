//! Pattern DMA: 16-bit repetition and independently masked 32-bit byte lanes.

/// Decoded fill control. Bit 20 is retained by the submission interface;
/// both forms use the same pixel pattern and byte lanes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Control {
    pixel_bytes: usize,
    lane_mask: u8,
}
impl Control {
    pub fn decode(word: u32) -> Result<Self, String> {
        if word & !0x0f10_0201 != 0 || word & 1 == 0 {
            return Err(format!("DMP unsupported fill control {word:#x}"));
        }
        let lane_mask = (word >> 24) as u8;
        let pixel_bytes = if word & 0x200 == 0 { 2 } else { 4 };
        if lane_mask == 0 || (pixel_bytes == 2 && lane_mask != 15) {
            return Err(format!("DMP unsupported fill control lanes {word:#x}"));
        }
        Ok(Self {
            pixel_bytes,
            lane_mask,
        })
    }
    pub fn preserves_destination(self) -> bool {
        self.lane_mask != 15
    }
    /// Partial fills require the original destination. Full fills also use
    /// this span so DMA bounds are checked before a completion is scheduled.
    pub fn apply(self, bytes: &mut [u8], pattern: u32) -> Result<(), String> {
        if bytes.is_empty() || !bytes.len().is_multiple_of(self.pixel_bytes) {
            return Err("DMP fill extent does not contain whole pixels".into());
        }
        let pattern = pattern.to_le_bytes();
        for pixel in bytes.chunks_exact_mut(self.pixel_bytes) {
            for (lane, byte) in pixel.iter_mut().enumerate() {
                if self.lane_mask & (1 << lane) != 0 {
                    *byte = pattern[lane];
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn depth_and_stencil_preserve_each_others_lanes() {
        for mask in 1..16 {
            let mut bytes = [0xa5; 16];
            Control::decode((mask << 24) | 0x201)
                .unwrap()
                .apply(&mut bytes, 0x44332211)
                .unwrap();
            for (i, byte) in bytes.iter().enumerate() {
                assert_eq!(
                    *byte,
                    if mask & (1 << (i % 4)) != 0 {
                        [0x11, 0x22, 0x33, 0x44][i % 4]
                    } else {
                        0xa5
                    }
                );
            }
        }
    }
    #[test]
    fn sixteen_bit_fill_repeats_low_halfword() {
        let mut bytes = [0xa5; 6];
        Control::decode(0x0f000001)
            .unwrap()
            .apply(&mut bytes, 0x44332211)
            .unwrap();
        assert_eq!(bytes, [0x11, 0x22, 0x11, 0x22, 0x11, 0x22]);
    }
    #[test]
    fn unknown_controls_and_partial_pixels_refuse() {
        for word in [0, 0x201, 0x0f000003, 0x0f000101, 0x01000001, 0x0f200201] {
            assert!(Control::decode(word).is_err(), "{word:#x}");
        }
        let control = Control::decode(0x0f100201).unwrap();
        assert!(control.apply(&mut [], 0).is_err());
        assert!(control.apply(&mut [0; 3], 0).is_err());
    }
}
