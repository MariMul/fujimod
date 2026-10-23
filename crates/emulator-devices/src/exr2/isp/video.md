# AVC encoder

`VideoEncoder` consumes aligned register writes and packed Y0,Y1,Cb,Cr
input DMA. It averages adjacent chroma rows to planar 8-bit 4:2:0 and uses
[OpenH264](https://github.com/ralfbiedert/openh264-rs) for real Baseline AVC
compression. Cargo builds the BSD-2-Clause codec from its locked sources.
Wrapper 0.9.3 supports the workspace's Rust 1.88 minimum; the locked native
backend is `openh264-sys2` 0.9.9. Native targets support encoding; Wasm
explicitly refuses image-backed starts until a software backend is available.

| Offset | Modeled meaning |
| --- | --- |
| `400` | Packed input DMA address, tightly packed two bytes per luma sample |
| `490/494`, `498/49C` | First/second bitstream base and byte capacity |
| `4A4` bits 2/3/4 | First/second/common output enables |
| `4A4` bit 5 | Current output bank, read-only |
| `4AC` | Running byte offset in the current bank |
| `C00` | `800` initialize; `8000` encode; `1000` pause; `2000` finish |
| `C20` bits 2–3 | Primary operation mode (1) after completion |
| `C24` | W1C status: bank start bit 9, frame bit 12, pause bit 14, finish bit 15 |
| `D88/D8C` | Low/high halves of the actual frame bit count |
| `E00/E04` | Luma width and padded height; progressive packed input only |
| `E08` | Software target rate in 1024 bits/s units |
| `E0C` | Frame rate (`23/29/59` select `24/30/60`) |
| `E10` | Maximum software quantizer |
| `E34` | Maximum GOP length |

The target-rate, quantizer and GOP mappings define the software compression
approximation; they do not reproduce silicon rate control or identical
compressed bytes. The supported register mode is checked before input is
read. Crop/field modes, alternate codec commands, overlapping/unmapped DMA,
invalid geometry and frames larger than a ring bank refuse. Other coefficient
or program uploads do not authorize an encoding mode.

All input is latched at START. Compression runs without guest effects. A due
virtual deadline writes the encoded bytes, zero-pads only to a word boundary,
updates the cursor and exact bit count, then raises the configured IRQ. A
frame can cross from either bank into the other; crossing sets the bank-start
status. Acknowledging one of several pending status bits schedules another
IRQ at the current virtual tick. Early advancement is inert. The declared
throughput is 64 luma samples per virtual tick; setup, pause and finish take
two ticks. These are model clocks, independent of host elapsed time.

GOPs start with a fresh IDR and constant SPS/PPS IDs. Single-threaded encoding
with scene-change, background and adaptive-quantization decisions disabled
makes checkpoint reconstruction repeatable. Checkpoints share immutable
binary input blobs for only the current GOP (at most 120 frames), plus any
pending encoded payload. Restore lazily rebuilds the host codec by replaying
those inputs, without reading guest memory or replaying guest effects.

The colocated video tests decode an eight-frame DMA clip across P frames and
GOP restarts, check decoded luma/chroma, compare restored successor bytes,
cross ring boundaries, check exact deadlines and DMA-before-IRQ order, and
refuse invalid modes and spans without publishing a completion.
