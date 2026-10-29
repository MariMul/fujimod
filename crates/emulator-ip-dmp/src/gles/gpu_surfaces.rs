use fujimod_emulator_gpu::{
    PixelWriteMask, SurfaceExtent, SurfaceFormat, SurfaceKey, SurfaceLayout, context,
};

const EXTENT: SurfaceExtent = SurfaceExtent {
    width: 16,
    height: 8,
};

fn context_or_skip() -> Option<&'static fujimod_emulator_gpu::GpuContext> {
    match context() {
        Ok(context) => Some(context),
        Err(error) if error.starts_with("host GPU adapter:") => {
            eprintln!(
                "skipping DMP GPU surface test because no host GPU adapter is available: {error}"
            );
            None
        }
        Err(error) => panic!("DMP GPU surface setup failed: {error}"),
    }
}

#[test]
fn surface_upload_and_readback_round_trip_every_format() {
    let Some(context) = context_or_skip() else {
        return;
    };
    let formats = [
        SurfaceFormat::Rgba8,
        SurfaceFormat::Rgb565,
        SurfaceFormat::Rgb5A1,
        SurfaceFormat::Rgba4,
        SurfaceFormat::D16,
        SurfaceFormat::D24,
        SurfaceFormat::D24S8,
    ];
    for (index, format) in formats.into_iter().enumerate() {
        let key = SurfaceKey {
            guest_address: 0x160000 + index as u64 * 0x1000,
            format,
            extent: EXTENT,
            layout: SurfaceLayout::Morton8x8,
        };
        let byte_len =
            EXTENT.width as usize * EXTENT.height as usize * format.bytes_per_pixel() as usize;
        let bytes: Vec<u8> = (0..byte_len)
            .map(|offset| offset.wrapping_mul(37).wrapping_add(index * 11) as u8)
            .collect();
        context.get_or_create_target(key, Some(&bytes)).unwrap();
        let readback = context.readback(key).unwrap();
        assert_eq!(readback.bytes, bytes, "{format:?}");
        assert!(readback.written_ranges.is_empty(), "{format:?}");
    }
}

#[test]
fn surface_write_masks_report_exact_morton_guest_byte_ranges() {
    let Some(context) = context_or_skip() else {
        return;
    };
    let key = SurfaceKey {
        guest_address: 0x168000,
        format: SurfaceFormat::D24S8,
        extent: EXTENT,
        layout: SurfaceLayout::Morton8x8,
    };
    let bytes = vec![0xa5; EXTENT.width as usize * EXTENT.height as usize * 4];
    context.get_or_create_target(key, Some(&bytes)).unwrap();
    context
        .record_write_masks(
            key,
            &[
                PixelWriteMask {
                    x: 0,
                    y: 0,
                    byte_mask: 0b1111,
                },
                PixelWriteMask {
                    x: 1,
                    y: 0,
                    byte_mask: 0b0101,
                },
                PixelWriteMask {
                    x: 0,
                    y: 1,
                    byte_mask: 0b1111,
                },
            ],
        )
        .unwrap();
    let readback = context.readback(key).unwrap();
    assert_eq!(readback.bytes, bytes);
    assert_eq!(
        readback.written_ranges,
        vec![
            (key.guest_address, 5),
            (key.guest_address + 6, 1),
            (key.guest_address + 8, 4),
        ]
    );
}
