# Emulator speed

The goal is three times camera speed in release builds: an X70 that boots to
REC in about one second on the camera boots in 0.33 s here, and every later
interaction keeps the same ratio. Speed is always camera wall time for an event
divided by the emulator's time for the same event. Emulated (virtual) time is
not a speed; when a milestone takes more emulated time than the camera takes,
the emulator does extra work and that is a correctness bug.

## Where the time goes

A release X70 boot was sampled at 1 ms intervals with `sample` (12 s, main thread
only; both cores run on it). Shares are inclusive.

| Cost | Share |
|---|---|
| Inside `uc_emu_start` (translated code, helpers and device callbacks) | 95% |
| Slow-path loads and stores (`load_helper`, `store_helper`) | 32% |
| — device register accesses (`Bus::access`) | 11% |
| — of which waking dependent devices (`Bus::wake_dependents`) | 4.4% |
| — recovering the exact instruction for an access (`cpu_restore_state`) | 6% |
| Exclusive loads and stores handled in Rust (`exclusive_instruction`) | 11% |
| Per-block exit-request helper (`helper_check_exit_request`) | 6.6% |
| Entering and leaving translated code (`tb_exec_lock`/`unlock`) | 8.7% |
| — of which JIT write-protect switches (`pthread_jit_write_protect_np`) | 5.1% |
| Deadline recomputation (`State::refresh_deadlines`) | 6.1% |
| SipHash in device maps | 3% |
| SHA-256 during execution | 1.7% |

At most a seventh of the samples fall in translated guest code itself. The rest
is the cost of leaving it, finding the exact instruction again, and
bookkeeping that runs per access or per slice.

## Plan

In expected order of return, each measured by host instructions retired for
the boot and the button scenario (load barely changes them) and by wall time:

1. Exclusives without leaving translated code. LDREX, STREX and CLREX become
   TCG helpers that call the shared monitor directly with the physical
   address and register values, instead of stopping, decoding the word,
   reading registers through the API and restarting the instruction. Monitor
   semantics stay as they are.
2. Fewer exits and switches. Keep the JIT in execute mode across exec-loop
   iterations and switch only when code is generated or invalidated; replace
   the per-block exit-request helper with an inline flag test.
3. Device accesses without `cpu_restore_state`. Supply the instruction count
   the access needs from the translation (the position within the block is
   known when the access is generated) rather than searching the block tree.
4. Scheduler bookkeeping. Wake only the dependents an access can affect and
   update deadlines incrementally instead of recomputing them.
5. Hashing. Device maps keyed by addresses use a fast non-cryptographic hasher;
   SHA-256 runs only where a digest is reported.
6. Then re-profile with the camera's power-on state (saved settings, set clock)
   and continue down the new list.
