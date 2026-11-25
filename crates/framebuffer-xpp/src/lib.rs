//! X-Processor Pro framebuffer composition from a read-only register snapshot.
use fujimod_framebuffer::raster::{self, PixelOrder, Plane};
use std::collections::BTreeMap;
pub const CONTROL: u64 = 0x000;
/// +0x000 bit 0 enables the output and its video plane, bit 2 the OSD layer group.
pub const VIDEO_ENABLE: u32 = 1 << 0;
pub const OSD_ENABLE: u32 = 1 << 2;
pub const VIDEO_FORMAT: u64 = 0x010;
pub const PACKED_422: u32 = 0x333;
pub const OUTPUT_SIZE: u64 = 0x014;
pub const VIDEO_SIZE: u64 = 0x02c;
pub const VIDEO_BUFFER: u64 = 0x030;
pub const OSD_GROUP_ATTRIBUTES: u64 = 0x200;
pub const OSD_WINDOW_FLAGS: u64 = 0x20c;
pub const LAYER_ORIGIN: u64 = 0x210;
pub const LAYER_SIZE: u64 = 0x220;
pub const LAYER_BUFFER: u64 = 0x230;
pub const LAYER_ROW_SKIP: u64 = 0x320;

pub const BASES: [u64; 2] = [0xfff59000, 0xfff64000];
pub fn register_ranges(output: usize) -> Result<Vec<(u64, usize)>, String> {
    let b = *BASES.get(output).ok_or("Unknown XPP output")?;
    Ok(vec![(b, 0x34), (b + 0x200, 0x40)])
}
pub const REGISTER_BYTES: usize = 0x240;
const WINDOW_FORMAT_MASK: u32 = 7;
const WINDOW_FORMAT_STRAIGHT_ABGR8888: u32 = 1;
/// Register layout that supplies the OSD window enables.
#[derive(Clone, Copy, Debug, Default)]
pub enum WindowEnables {
    /// The group word at +0x200 itself holds one bit per window.
    #[default]
    GroupAttributeMask,
    /// The low bit of each nibble at +0x20c enables that slot; +0x200 then
    /// holds the group format selector and +0x320 each window's row skip.
    SlotNibbles,
}

impl WindowEnables {
    pub fn from_offset(offset: u64) -> Result<Self, String> {
        match offset {
            OSD_GROUP_ATTRIBUTES => Ok(Self::GroupAttributeMask),
            OSD_WINDOW_FLAGS => Ok(Self::SlotNibbles),
            _ => Err(format!("Unsupported XPP OSD enable register {offset:#x}")),
        }
    }
}

/// Bytes and geometry that determine the rendered pixels.
#[derive(PartialEq)]
pub struct Capture {
    width: u32,
    height: u32,
    video: Option<raster::Snapshot>,
    /// Straight A,B,G,R window bytes instead of premultiplied B,G,R,A words.
    straight: bool,
    layers: Vec<Layer>,
}

#[derive(PartialEq)]
struct Layer {
    /// Bytes per stored row, including the window's skip.
    stride: usize,
    /// Visible pixels per row, clipped to the output.
    width: usize,
    pixels: Vec<u8>,
}

pub struct Decoder<'a> {
    pub words: &'a BTreeMap<u64, u32>,
    pub window_enables: WindowEnables,
}
impl Decoder<'_> {
    fn word(&self, o: u64) -> u32 {
        self.words.get(&o).copied().unwrap_or(0)
    }
    /// `(width, height)` of a size word, or `None` for unsupported bits.
    pub fn extent(word: u32) -> Option<(u32, u32)> {
        (word & 0xf000_f000 == 0).then_some(((word >> 16) + 1, (word & 0xfff) + 1))
    }
    /// Whether the video plane is shown. A plane whose words are all zero
    /// (format, size and buffer) is not yet programmed: the firmware enables
    /// the output (0x1010101) before REC sets it up.
    pub fn video_shown(&self) -> bool {
        self.word(CONTROL) & VIDEO_ENABLE != 0
            && (self.word(VIDEO_FORMAT) != 0
                || self.word(VIDEO_SIZE) != 0
                || self.word(VIDEO_BUFFER) != 0)
    }
    /// The raw window enables normalized to one bit per slot.
    pub fn window_mask(&self) -> u32 {
        match self.window_enables {
            WindowEnables::GroupAttributeMask => self.word(OSD_GROUP_ATTRIBUTES),
            WindowEnables::SlotNibbles => {
                let flags = self.word(OSD_WINDOW_FLAGS);
                (0..4).fold(0, |mask, slot| mask | (((flags >> (slot * 4)) & 1) << slot))
            }
        }
    }
    /// The enabled OSD layers, or none while the OSD group is off.
    pub fn layers_shown(&self) -> u32 {
        if self.word(CONTROL) & OSD_ENABLE != 0 {
            self.window_mask()
        } else {
            0
        }
    }
    /// Byte pitch of a window with the supplied visible width.
    pub fn layer_stride(&self, slot: u64, width: u32) -> usize {
        let skip = match self.window_enables {
            WindowEnables::GroupAttributeMask => 0,
            WindowEnables::SlotNibbles => self.word(LAYER_ROW_SKIP + 4 * slot) & 0x3fff,
        };
        (width + skip) as usize * 4
    }

    fn straight_abgr(&self) -> Result<bool, ()> {
        match self.window_enables {
            WindowEnables::GroupAttributeMask => Ok(false),
            WindowEnables::SlotNibbles => {
                let format = self.word(OSD_GROUP_ATTRIBUTES) & WINDOW_FORMAT_MASK;
                if format != WINDOW_FORMAT_STRAIGHT_ABGR8888 {
                    return Err(());
                }
                Ok(true)
            }
        }
    }
    /// Composite the video layer and the enabled OSD layers; `Ok(None)` when
    /// no layer is enabled.
    ///
    /// The output size (+0x014) is the active area the panel reads each frame.
    /// A plane larger than it shows its top-left part: rows and columns past the
    /// active area are not fetched. The X-T20 2.11 VIEW MODE switch from the EVF
    /// back to the rear screen produces this: the firmware writes the rear
    /// screen's timing (720 x 480) and re-enables the OSD group in one step,
    /// while the plane sizes still hold the EVF's 1024 x 768 (OSD) and
    /// 1024 x 682 (video) until its next commit, and a frame is scanned in
    /// between. The law is inferred from that sequence, not measured on the
    /// panel.
    // Err(()) only says the layer setup is unsupported; callers fall back without a reason.
    #[allow(clippy::result_unit_err)]
    pub fn compose(
        &self,
        mut read: impl FnMut(u64, usize) -> Result<Vec<u8>, String>,
    ) -> Result<Option<(u32, u32, Vec<u8>, u64)>, ()> {
        let Some((capture, top)) = self.capture(&mut read)? else {
            return Ok(None);
        };
        Ok(Some((
            capture.width,
            capture.height,
            capture.render()?,
            top,
        )))
    }

    /// Sample the planes once. The reported source address is separate from
    /// the pixel inputs so a buffer flip can reuse identical rendered pixels.
    #[allow(clippy::result_unit_err)]
    pub fn capture(
        &self,
        mut read: impl FnMut(u64, usize) -> Result<Vec<u8>, String>,
    ) -> Result<Option<(Capture, u64)>, ()> {
        if self.window_mask() & !0xf != 0 {
            return Err(());
        }
        let video = self.video_shown();
        let enable = self.layers_shown();
        if enable == 0 && !video {
            return Ok(None);
        }
        let straight = enable != 0 && self.straight_abgr()?;
        let (width, height) = Self::extent(self.word(OUTPUT_SIZE)).ok_or(())?;
        let mut capture = Capture {
            width,
            height,
            video: None,
            straight,
            layers: Vec::new(),
        };
        let mut top = 0;
        if video {
            let (w, h) = Self::extent(self.word(VIDEO_SIZE)).ok_or(())?;
            let buffer = u64::from(self.word(VIDEO_BUFFER));
            // The video path can be on with no plane committed: the X-T20's
            // update page switches bit 2 of +0x000 on while +0x02c and +0x030
            // both stay zero, and the layers then compose over black. A plane
            // with only one of them set is still refused.
            // A plane larger than the output is clipped to the output's active
            // area (see `compose`).
            let committed = self.word(VIDEO_SIZE) != 0 || buffer != 0;
            if committed && (self.word(VIDEO_FORMAT) != PACKED_422 || w & 1 != 0 || buffer & 3 != 0)
            {
                return Err(());
            }
            if committed {
                let plane = Plane {
                    address: 0,
                    stride: w * 2,
                    source_x: 0,
                    source_y: 0,
                    source_width: w,
                    source_height: h,
                    sampling: None,
                    work_x: 0,
                    work_y: 0,
                    work_width: w,
                    work_height: h,
                    order: PixelOrder::YyCbCr,
                };
                capture.video = Some(
                    raster::capture(
                        |offset, count| read(buffer + offset, count),
                        plane,
                        width,
                        height,
                    )
                    .map_err(|_| ())?,
                );
                top = buffer;
            }
        }
        for layer in (0..4).filter(|n| enable & 1 << n != 0) {
            let at = |offset: u64| offset + 4 * layer as u64;
            let (w, h) = Self::extent(self.word(at(LAYER_SIZE))).ok_or(())?;
            let buffer = u64::from(self.word(at(LAYER_BUFFER)));
            if self.word(at(LAYER_ORIGIN)) != 0 || buffer & 3 != 0 {
                return Err(());
            }
            let shown = (w.min(width) as usize, h.min(height) as usize);
            let stride = self.layer_stride(layer, w);
            let pixels = read(buffer, stride * shown.1).map_err(|_| ())?;
            capture.layers.push(Layer {
                stride,
                width: shown.0,
                pixels,
            });
            top = buffer;
        }
        Ok(Some((capture, top)))
    }
}

impl Capture {
    pub fn extent(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    #[allow(clippy::result_unit_err)]
    pub fn render(&self) -> Result<Vec<u8>, ()> {
        let (width, height) = self.extent();
        let mut rgba = vec![0; width as usize * height as usize * 4];
        for pixel in rgba.as_chunks_mut::<4>().0 {
            pixel[3] = 255;
        }
        if let Some(video) = &self.video {
            raster::composite_snapshot(video, width, height, &mut rgba).map_err(|_| ())?;
            for pixel in rgba.as_chunks_mut::<4>().0 {
                pixel.swap(0, 2);
            }
        }
        let (alpha_index, color_offset) = if self.straight { (0, 1) } else { (3, 0) };
        for Layer {
            stride,
            width: shown,
            pixels,
        } in &self.layers
        {
            let shown_w = (*shown).min(width as usize);
            for (y, row) in pixels.chunks_exact(*stride).enumerate() {
                let row = &row[..shown_w * 4];
                let line = &mut rgba[y * width as usize * 4..][..shown_w * 4];
                let (source, tail) = row.as_chunks::<4>().0.as_chunks::<16>();
                let (destination, tail_out) = line.as_chunks_mut::<4>().0.as_chunks_mut::<16>();
                for (out, src) in destination.iter_mut().zip(source) {
                    if *src != [[0; 4]; 16] {
                        blend_osd(out, src, alpha_index, color_offset);
                    }
                }
                blend_osd(tail_out, tail, alpha_index, color_offset);
            }
        }
        Ok(rgba)
    }
}

fn blend_osd(output: &mut [[u8; 4]], source: &[[u8; 4]], alpha_index: usize, color_offset: usize) {
    for (out, src) in output.iter_mut().zip(source) {
        if *src == [0; 4] {
            continue;
        }
        let alpha = u32::from(src[alpha_index]);
        if alpha == 255 && alpha_index == 3 {
            *out = *src;
            continue;
        }
        let keep = 255 - alpha;
        for n in 0..3 {
            let color = u32::from(src[color_offset + n]);
            let contribution = if alpha_index == 0 {
                (color * alpha + 127) / 255
            } else {
                color
            };
            let value = contribution + (u32::from(out[n]) * keep + 127) / 255;
            out[n] = value.min(255) as u8;
        }
    }
}
