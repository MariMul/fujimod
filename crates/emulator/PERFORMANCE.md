# Emulator speed

The goal is three times camera speed in release builds: an X70 that boots to
REC in about one second on the camera boots in 0.33 s here, and every later
interaction keeps the same ratio. Speed is always camera wall time for an event
divided by the emulator's time for the same event. Emulated (virtual) time is
not a speed; when a milestone takes more emulated time than the camera takes,
the emulator does extra work and that is a correctness bug.

## Measurements

The X70 runs with saved settings and a set clock: it reaches REC at 0.34 s of
emulated time and accepts input at 1.35 s, after 53.6M instructions. Board
construction from the run configuration takes 0.15 s. REC live view with a RAW
scene costs 1.44 CPU seconds per emulated second; the ISP statistics producer
alone takes 134 ms per frame and the image output channels about 47 ms more.

| Change | CPU seconds to input readiness |
|---|---|
| Before | 1.09–1.14 |
| Memory DMA status as a fast read | 0.81–0.82 |
| Interactive runs test exits only at block entry | 0.72–0.74 |

## Where the time goes

Samples of three boots to input readiness, as shares of emulation time:

| Cost | Share |
|---|---|
| Translating guest code (`tb_gen_code`) | 21% |
| Device register accesses (`load_helper`, `store_helper`) | 19% |
| Per-block hook helper (`helper_uc_tracecode`) | 12% |
| Device deadlines, mostly firmware LZ decompression (`method4::Stream::feed`) | 10% |
| Instruction callbacks, half of them exclusives (`Execution::instruction`) | 8% |
| Per-access and per-block exit checks (`helper_check_exit_request`) | 7% |
| JIT write-protect switches | 5% |

Before the DMA change, 81% of all device accesses during boot were one status
register polled in a tight loop, each a full exit into the board.

## Plan

In expected order of return, each measured by CPU time to input readiness and
host instructions retired (load barely changes them):

1. Polled status registers as fast reads. Each register a boot loop polls is
   published as an atomic the CPU reads without entering the device. Then let
   interactive runs skip a polling loop whose only input is such a register
   until the device's next deadline, as exact runs already do.
2. Block hook calls. Debiting the budget in translated code instead of the
   hook helper left host instructions unchanged (30.9G for three boots either
   way): most blocks reach the hook because the interactive budget is short or
   the accounting mode changed. Find why, and widen the interactive window.
3. Done: interactive runs test exits at block entry with QEMU's inline test
   and no check after guest loads and stores.
4. Done: exclusives without leaving translated code (below).
5. Firmware decompression in whole tokens and slices instead of per byte.
6. Translation cost: fewer generated helper calls per block (items 2 and 3)
   shrink every block; then measure what remains.
7. Device access bookkeeping: wake only the dependents an access can affect and
   update deadlines incrementally.

The ISP statistics producer and image output channels, which dominate live view,
are optimized with the scene and ISP work.

## Walkthroughs

`controls/hero/ev_dial` on the X70 (11.2 camera seconds, 1.03G guest
instructions after boot, exact policy) is the reference guide. On the shared
machine it ran at 0.12–0.15x camera speed and failed the 60 CPU second limit.
Each Python wait poll ran the board once with that poll's read set as store
watches, and installing and removing about 500 completed-store hooks per run
cost 1.5–2 ms against 3 us for an unwatched empty window. 96% of consecutive
polls watch the same set, so a continuation now keeps its hooks for the next
one (`board/continuation.rs`): the empty watched window costs 25 us and the
guide passes at 11.2 camera seconds in 22.0 CPU seconds (0.51x).

Engine time in that guide, as shares of the whole run sampled after boot (the
remainder is Python observation, PNG decoding and live-view scene rendering):

| Cost | Share |
|---|---|
| Stores on the slow path (`store_helper`), including state restores for MMIO inside a block | 16% |
| Translating guest code (`tb_gen_code`) | 13% |
| Instruction callbacks: exclusives 5%, failed loop proofs 2%, register reads that restore state | 11% |
| Per-access exit checks (`helper_check_exit_request`) | 7% |
| Per-block hook helper (`helper_uc_tracecode`) | 7% |

Core 0 left translated code 1.73M times ("check exit, no sync"), once per
600 guest instructions: an emulated LDREX, STREX or CP14 identity read wrote
the PC from its callback, which leaves `cpu_exec`. The translation filter now
replaces those ARM instructions (word exclusives, CLREX, the CP14 identity
read and PMU transfers) with their instruction observer
(`InstructionDisposition::Replace`): the block carries the hook and no body,
ends after it, and chains to the next block. The observer writes the PC only
when a stop was requested while it ran. A successful software STREX also
publishes to completed-store observers, as a native store does. Thumb has no
translation filter and keeps the PC write.

| Change | ev_dial CPU seconds | Boot exits, core 0 |
|---|---|---|
| Before | 21.6 | 112,725 |
| Replaced exclusives and identity reads | 15.0 | 1,951 |

Boot exits count "check exit, no sync" over the first 65M instructions in
exact policy. Guest instruction counts, board fingerprints and live-view
frames are identical before and after. Boot host instructions did not change
measurably (19.1–19.2G for process start, construction and 65M
instructions), because boot leaves translated code rarely; the walkthrough's
waits run firmware that polls exclusives and the identity register.
