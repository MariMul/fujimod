# Fujimod

This app lives in `apps/web/` of the FujiMod repository. Run the commands
below from that directory.

Try camera firmware and build SD boot files in your browser. Firmware is
identified by its complete SHA-256 and never uploaded. The site is developed
on review branches; the Pages workflow deploys approved main builds only.

## Camera viewer

The Vite + TypeScript site uses three.js from npm, with no UI framework. Drop a
DAT anywhere on the viewer, or choose it there. The whole-file SHA-256 selects
`public/catalog.json`: firmware ID, body, model, optional engine artifact and
optional 3D model. Unrecognized files stay on the neutral camera silhouette;
recognized bodies without installed capabilities show unsupported. Patching
and emulation are independent capabilities. Release-only patch entries also
work without a site source change.

`src/engine/api.ts` is the website's emulator boundary: boot, frames,
press/release, setPosition, setScene and power. The shared Pyodide WorkerEngine adapter
owns pacing, request ordering and disposal; it wraps the existing Pyodide
session worker. No register addresses or firmware bindings appear in rendering
or panel code. The input artifact belongs to the emulator. Model nodes,
3D part mappings live in `devices/x70/parts.json`; panel layout, labels, faces
and power-on gestures live in `devices/x70/panel.json`.
The catalog selects the shared Pyodide engine and supplies its Python boot
protocol. Device directories contain model, panel and initial-position data;
adding a body does not add conditionals to the worker or viewer. `src/viewer/model.ts` ports LCD UV mapping,
picking, orbiting and LCD projection from the research viewer. The panel is
rendered from body data by shared dial, key, shutter and segmented controls.
The SD tree shows actual FAT files, offers downloads and format, and retains
media through power cycles. Generated patch files seed the initial card.
Power-on combinations apply external input levels through the checked body
graph; they do not patch guest RAM or guarantee that every ROM boot mode is
modeled.

With no image or webcam input, the app supplies the bundled abstract dunes
still through the ordinary RGB scene input. Clearing an input restores this
still, which is retained across power cycles and card restarts. The diagnostic
analytic chart remains available to the underlying emulator and its tests.
The initial image travels with boot/restart, and readiness waits for a new
scene frame so the chart is not published as the camera's first ready image.
The 1536×1024 PNG in `src/engine/assets/default-dunes.png` was generated with
OpenAI's built-in image generator and selected for the emulator background.

The model is **CAMERA ( Fujifilm X70 )** by
[McCLOUD2101](https://sketchfab.com/McCLOUD2101),
[original model](https://sketchfab.com/3d-models/camera-fujifilm-x70-5fd32c3c004f4fe4aecaaab8e2b89783),
[CC BY 4.0](https://creativecommons.org/licenses/by/4.0/).
The model uses Git LFS. Its hidden filter attachment is selected by node data;
the glTF itself is unchanged. The page repeats attribution when loaded.
Model taps show the selected button name and a marker that remains briefly after
release. Touch picking accepts nearby visible buttons while preserving direct
LCD contacts; the four-way selector marks one direction without lighting its
entire shared mesh. `tests/model_touch.py` checks real WebKit touch delivery on
the model against a Vite development server, without a DAT or emulator runtime.

X-M1 1.50 uses the same Wasm runtime with its checked REC graph, native LCD
and calibrated menu/sub-command controls. It has no 3D model, host RGB scene,
shutter-capture binding or main command dial. Its virtual card starts ejected;
inserted-card boot has not passed the browser gate. “Try emulator” selects a
local configured DAT, or opens the DAT chooser on the public site.

### Develop

Use Node 22.18 or later, `git lfs install --local`, `git lfs pull`, then:

```sh
npm ci
npm run dev -- --port 5190
```

Vite binds to `127.0.0.1`. To use existing research artifacts locally, put
absolute paths in ignored `.env.local`:

```text
FUJIMOD_RUNTIME=/path/to/staged/runtime
FUJIMOD_PATCHES=/path/to/patch-release/artifacts
FUJIMOD_DAT=/path/to/local/FPUPDATE.DAT
```

The staged runtime directory contains `catalog.json` and firmware-ID folders
with `_native.so`, `sources.zip`, `inputs.json`, `pyodide/` and
`firmware-inputs.zip`. A release preparation stages this tree into
`public/runtime/`, which Vite also serves directly without a mount. These mounts exist only in
the dev and preview servers. Setting `FUJIMOD_DAT` enables automatic local boot; without it,
the viewer remains a DAT drop target. Public builds omit the automatic DAT fetch. The local DAT is served from its
opt-in mount and is never copied into any build artifact.

```sh
npm test
npm run build
PLAYWRIGHT_BROWSERS_PATH=/path/to/browsers python tests/browser.py \
  --output /fresh/ignored/proof --expected /path/to/private/sd-card
```

The browser check boots the real local DAT on desktop Chromium and WebKit's
iPhone 13 emulation, checks live frames and a menu tap, saves page/model/LCD
screenshots and verifies downloaded ZIP members against private builder output.
WebKit emulation is a desktop browser check, not an iPhone hardware measurement.

### Local production preview

```sh
npm run build:local-preview
npm run preview -- --mode local-preview --port 5190 --strictPort
```

This serves `dist-local-preview/` and enables automatic boot using `FUJIMOD_DAT` in
ignored `.env.local`. The preview binds only `127.0.0.1` and permits the
`cozycat.tail2b5ca.ts.net` Host header for `tailscale serve`. The ordinary public
`npm run build` writes `dist/`, excludes automatic boot, and cannot overwrite
the local preview. Restart the preview server after changing its environment.

To switch cameras from a phone on the local preview, add an explicit map of
firmware IDs to DAT files in `.env.local` (only configured files are served):

```text
FUJIMOD_LOCAL_FIRMWARES={"x70-110":"/path/to/FPUPDATE.DAT","xm1-150":"/path/to/FWUP0002.DAT"}
```

With a runtime that implements capture and Playback, verify the built preview:

```sh
PLAYWRIGHT_BROWSERS_PATH=/path/to/browsers python tests/capture.py \
  --url http://127.0.0.1:5190/ --output /fresh/ignored/capture-proof
```

This WebKit iPhone gate checks exact English MENU pixels, touch dial input,
source/LCD frame cadence, a Full shutter tap, a JPEG download through the card
UI, and the same saved file decoded by stock Playback. It saves viewport and
LCD screenshots without modifying served source or installing firmware patches.

The performance gate measures 30 wall seconds with Undo enabled, requires at
least 0.5× virtual/wall speed and 9.5 source/presented frames per guest second,
times an exact English MENU pixel match, and requires a real reload below five
seconds. The capture gate requires a Wasm peak below 1024 MiB and also records
whole-browser RSS and macOS physical footprint, which are separate quantities.

```sh
python tests/performance.py --persistent --output /fresh/ignored/performance
python tests/capture.py --persistent --cold --output /fresh/ignored/capture
python -m unittest discover -s tests -p test_source_archive.py
```

Persistent WebKit contexts support OPFS; private contexts can use IndexedDB.
`--cold` explicitly clears this app's test-origin caches before loading. A fresh
profile directory alone does not guarantee empty WebKit origin storage. The
process sampler measures the host browser, including new WebKit XPC processes;
run only one instance of that browser bundle while sampling.

The engine prepares runtime downloads in parallel with DAT verification. Initial
card bytes are requested only on a cold miss. ETags revalidate browser-local
asset caches; catalog, runtime and package checks still verify their bytes.
OPFS writes stream into atomic replacement files. The source archive loader
materializes imported source/checked-hash bytecode while preserving ordinary
source paths for inspection. The allocator hook is specific to the pinned
Pyodide factory ABI and fails if that ABI changes.

Compare the built page with an approved HTML design reference:

```sh
PLAYWRIGHT_BROWSERS_PATH=/path/to/browsers python tests/design.py \
  --reference /path/to/mockup.html --output /fresh/ignored/design-proof
```

This captures idle, running and forced-fault states at 1300×900 and WebKit
iPhone 390×844, saves side-by-side composites and checks CSS-pixel geometry.
Faults pause the engine, paint a Mac-style Undo dialog into the LCD, and expose
raw diagnostics plus a report ZIP containing the preceding RAM snapshot.

### Release artifacts and deployment

`npm run prepare:release` fetches a release index and verifies patches. Optional
`browser.json` supplies a hash/size inventory of generated runtime artifacts and
a capability catalog. `scripts/stage-browser.mjs` verifies each artifact before
staging it under ignored `public/runtime/`. A patch-only release removes the
emulation capability from the installed catalog. `npm run build` then bundles
this repository into `dist/` and verifies every GLB is a real glTF 2 binary with
identical source/deployment hashes. Pages checks out and pulls the LFS model
objects before building. No private repository source is copied over the
site, and neither a stock DAT nor boot ROM is a public input.

The Pages workflow runs only from main or a release targeting main. Review
branches never deploy. The research repository pins the reviewed public commit
as its `web` submodule. The private patch builder creates/verifies release
artifacts and never copies, commits or pushes website source.

## JavaScript API

```js
import { patch } from './src/patch/patcher.ts';
const { ffboot, ffsys, metadata } = await patch(baseStockDatFile, compiledPatch);
```

Inputs are a File, Blob, ArrayBuffer or Uint8Array. Outputs are Uint8Arrays.
This API does no network or filesystem I/O. It verifies the stock input,
command stream and both final files with SHA-256. The page additionally checks
the downloaded patch against the index. Work runs in a module worker; the
page provides a ZIP download and separate download links for both binaries.

## Patch format: FJMODP01

All integers are unsigned little-endian. Eight ASCII magic bytes `FJMODP01`,
then a u32 JSON metadata length, then UTF-8 JSON, then a binary command stream.
Metadata declares schema 1, source byte count and SHA-256, output sizes and
SHA-256 (`outputs.ffboot`, `outputs.ffsys`), command count and stream SHA-256.
It also identifies the firmware, release and included mods.

Each command has a 20-byte header:

| Byte | Field |
| --- | --- |
| 0 | Opcode: 1 COPY, 2 FUJI_LZ, 3 WRITE |
| 1 | Output: 0 ffboot, 1 ffsys |
| 2 | Flags: bit 0 XOR source with 0xff; all other bits zero |
| 3 | Reserved, zero |
| 4 | Destination offset, u32 |
| 8 | Output byte count, u32 |
| 12 | DAT source offset, u32 (zero for WRITE) |
| 16 | DAT source byte count, u32 (zero for WRITE) |

COPY reads DAT bytes, optionally XOR-decoding them. FUJI_LZ uses Fuji's
method-4 stream (not the LZ4 codec): a five-u32 header then literal runs and
11-bit-distance/4-bit-length matches, including overlapping matches. WRITE
is followed immediately by its literal bytes, with flags zero. Outputs start
zero-filled; commands execute in order, so later WRITE operations replace
reconstructed stock bytes. Stock firmware comes from the supplied DAT;
patch literals contain compiled changes and short gaps coalesced between them.

Both parsers reject invalid commands, bounds, malformed streams, wrong inputs,
trailing bytes and mismatched output hashes. Versioned magic permits future
format changes while existing pages handle any firmware represented by these
operations. A genuinely new codec would require an interpreter update.

`patches.json` has schema 1, a release tag, `patches: {SHA256: filename}` and a
`firmwares` array. Each entry supplies model/version, source hash/size, Fuji
download page and original release date, patch hash/size, included mods and model-specific boot instructions. Patch filenames are simple
local filenames. Pages serves index and patches from the same origin.
