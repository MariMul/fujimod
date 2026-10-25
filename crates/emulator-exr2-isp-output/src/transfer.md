# Image transfer

`ImageTransfer` owns the aligned 32-bit registers at `FFF64000..FFF64020`.
Reset (`00=0`) cancels pending work, clears status and returns ready bit 16
after two declared virtual ticks. The configuration uses control bits 4–5,
low 30 source/destination fields at 04/08, low 15 pitch in luma samples at 0C,
paired 12-bit dimensions at 10, status 14, W1C 18 and mask 1C.

Control `10101 | alignment<<4` starts a bounded packed Y0,Y1,Cb,Cr copy.
Alignment is 3/2/1/0 for widths divisible by 128/64/32/other. It selects transfer
grouping without resizing. Only positive even widths, positive heights and
tightly packed rows (`pitch=width`) are supported. Partial/in-place overlap,
unmapped DMA, unknown register bits and other layouts refuse before DMA.

Source bytes and destination geometry latch at START. Output remains unchanged
until the virtual deadline, when all addressed bytes copy before success status
bit 0 and the configured physical interrupt. The declared clock is 256 bytes
per tick with a two-tick minimum, independent of host elapsed time. Status and
mask are independent; W1C lowers the interrupt only after all enabled status
bits are acknowledged. Reset is an explicit cancellation boundary.

The transfer counter and `last_output` receipt publish atomically at completion.
The receipt contains physical source/destination addresses, dimensions, byte
pitch/count, packing, latched source/output hashes and start/completion ticks.
Pending metadata cannot relabel the prior completed receipt. Checkpoints retain
the exact deadline and immutable binary source payload; diagnostics expose only
metadata and hashes. Restore validates the payload shape and digest before any
successor DMA.
