# Image transfer registers

`ImageTransfer` owns the aligned 32-bit registers at `FFF64000..FFF64020`.
Reset (`00=0`) clears status and returns ready bit16 after two declared
virtual ticks. This clock is an approximation independent of host elapsed time.
The configuration admits control bits4–5, low30 address fields at04/08,
low15 pitch at0C, paired12-bit dimensions at10, status14, W1C18 and mask1C.

A request with control enable bit0 retains the submitted register state and
refuses execution. It publishes no DMA, completion or interrupt. The transfer
layout and format require support before any request can complete. Checkpoints
retain the register state and exact pending reset deadline; diagnostics are
observational.
