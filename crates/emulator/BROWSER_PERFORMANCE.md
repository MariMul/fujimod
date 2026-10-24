# Profiling a browser boot

Capture the dedicated emulator worker, not the page's renderer or the firmware
verification worker. The web app's marks separate file receipt, DAT verification,
engine boot and engine readiness. A new browser context gives each measurement
fresh storage and a cold camera.

## Build with function names

From the repository root, install the pinned Mise tools and JavaScript package
dependencies, then build the optimized emulator with its Wasm name section:

```sh
mise exec -- node bindings/javascript/emulator/build.mts \
  --out "$PWD/target/emulator" --profiling
```

This keeps the optimized Rust and C profiles. `--profiling` adds function names;
it does not select a debug build. Keep `build.json` with the capture: it records
compiler versions, optimization settings, JIT source identity and output hashes.
Use the same output directory for subsequent builds so Cargo reuses its output.

Serve the web app locally with its camera runtime available. Replace both
`camera.mjs` and `camera.wasm` in that served runtime bundle with the profiling
build. Nested Emscripten workers must receive the same loader as their parent;
intercepting only the parent worker's fetch can produce mismatched Wasm imports.
Keep the profiling build local, and use camera data matching the catalog
compiled into that module. A prepared-document identity failure means the bundle
and module need to be rebuilt together.

## Capture the worker

From `apps/web`, with a firmware update accepted by the page:

```sh
node scripts/profile-boot.ts \
  --url https://your-preview.example/ \
  --update /path/to/UPDATE.DAT \
  --out ../../target/boot-profile \
  --sample-ticks 1200000
```

The script attaches through CDP before a worker starts, starts the sampling
profiler only for `wasm-worker`, then releases the debugger startup pause. It
stops sampling at `camera-engine-ready` and writes:

- `worker.cpuprofile`: open in Chrome DevTools **Performance → Load profile**.
- `summary.json`: weighted self and inclusive function totals and broad runtime
  categories. Recursive calls count once per sample in inclusive totals.
- `timings.json`: page marks, selected manifest settings, module hashes,
  configuration, guest instruction/tick samples, boot call times and JIT counters.

The loader wrapper records `configure`, `runUntil` and `bootToRec` calls only in
this browser. `--sample-ticks` divides each `runUntil` into bounded synchronous
calls and records their counters. Choose a quantum near 0.1 guest seconds using
the manifest's `ticksPerSecond`; 1,200,000 ticks is appropriate at 12 MHz. The
wrapper adds no sleeps. It can alter scheduler boundaries and block overshoot,
so use it to locate slow phases, then measure the fix without it.

For each neighboring pair of samples, compute:

```
guest instructions / host second = Δinstructions / (Δwall milliseconds / 1000)
guest seconds / host second = Δticks / ticksPerSecond / (Δwall milliseconds / 1000)
```

Ticks and instructions have different meanings. Inspect both: a run can be slow
because it executes more guest instructions, spends more host time per
instruction, or waits without executing. Do not infer camera speed from MIPS.

## Read the capture

Start with page marks. Subtract `camera-file-received` from
`camera-dat-verified` for verification, and `camera-engine-boot` from
`camera-engine-ready` for the full engine startup. Within startup, the loader's
`run-start`/`run-end` and `rec-start`/`rec-end` events separate guest boot calls
from construction, scene setup, checkpoint capture and presentation.

In the worker profile, `camera.wasm` contains the emulator's Rust/C runtime;
`wasm://` modules contain generated guest translation blocks and their chain
dispatcher. Generated-body samples include inline memory and accounting guards,
so that category is not a measure of useful ARM computation alone. Expand the
native Wasm stacks to distinguish:

- translation and JIT preparation from execution;
- instruction/block hooks and accounting from guest semantics;
- slow memory access dispatch from the device work invoked through it;
- device callbacks, deadlines, DMA and decompression;
- checkpointing, hashing and rendering;
- JavaScript glue, garbage collection and idle time.

Self time identifies where the processor spent its samples. Inclusive time
identifies which call path owns that cost; inclusive rows overlap and must not
be added. `timeDeltas` weight samples in microseconds. Compare weighted time with
sample counts when long sampling gaps or host contention are present.

JIT `compileMilliseconds` and `prepareMilliseconds` are measured separately.
Check `enabled`, failures, unsupported operations, capacity refusals, flushes,
invalidations and compiled/chained executions before assuming a JIT problem.
A small compilation total does not establish that execution stays in generated
code; helper and callback stacks reveal repeated exits.

For a prepared camera, `--boot-ticks 0` removes only the preliminary fixed-tick
run and lets `bootToRec` observe readiness from reset. Compare its final
instruction count with the ordinary boot: similar counts mean the extra work
precedes REC. Record the boot entry and held inputs too. ROM/SD boot and a
loader-handoff run execute different startup code; their instruction counts
cannot serve as interchangeable readiness baselines.

`configure` records the actual module options. An omitted `executionPolicy`
selects `interactive`; the manifest can override it. Interactive policy permits
block-boundary instruction overshoot and controls scheduling/accounting. It does
not itself sleep to pace a cold boot. The web worker starts its live pacing loop
after cold boot. Inspect that distinction before changing the clock rate or
removing event boundaries. Compare a policy with `--policy exact` or
`--policy interactive`; the override affects only the diagnostic context.

## Validate the improvement

Repeat without CPU sampling or diagnostic slicing:

```sh
node scripts/profile-boot.ts \
  --url https://your-preview.example/ \
  --update /path/to/UPDATE.DAT \
  --out ../../target/boot-timing --no-profile
```

Run comparisons sequentially, keep the update, runtime inputs, scene, policy and
module identity fixed, and record host load. Use multiple fresh contexts and
report the range. Confirm REC and usable controls, then repeat the ordinary boot
in Chromium and WebKit. Sampling builds and captures locate costs; unsampled
runs establish wall-time improvements. Keep generated modules, profiles and
receipts in ignored build output.

For an unsampled WebKit comparison, add `--browser webkit --no-profile`.
The script records the browser identity and isolation state. Its prepared-camera
and card-boot events also record the boot entry, instruction/tick ratio, held
controls and boot filenames, so a ROM/SD boot is distinguishable from a loader
handoff without inferring the start from instruction counts.
