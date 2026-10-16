# Fujimod

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
physical panel order and plain display labels live in `devices/x70/parts.json`.
The catalog selects the shared Pyodide engine and supplies its Python boot
protocol. Device directories contain model/part/initial-position data only;
adding a body does not add conditionals to the worker or viewer. `src/viewer/model.ts` ports LCD UV mapping,
picking, orbiting, visible leader lines and sizing from the research viewer.

The model is **CAMERA ( Fujifilm X70 )** by
[McCLOUD2101](https://sketchfab.com/McCLOUD2101),
[original model](https://sketchfab.com/3d-models/camera-fujifilm-x70-5fd32c3c004f4fe4aecaaab8e2b89783),
[CC BY 4.0](https://creativecommons.org/licenses/by/4.0/).
The model uses Git LFS. Its hidden filter attachment is selected by node data;
the glTF itself is unchanged. The page repeats attribution when loaded.

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

This serves the built `dist/` and enables automatic boot using `FUJIMOD_DAT` in
ignored `.env.local`. The preview binds only `127.0.0.1` and permits the
`cozycat.tail2b5ca.ts.net` Host header for `tailscale serve`. The ordinary public
`npm run build` excludes the local automatic-boot setting.

With a runtime that implements capture and Playback, verify the built preview:

```sh
PLAYWRIGHT_BROWSERS_PATH=/path/to/browsers python tests/capture.py \
  --url http://127.0.0.1:5190/ --output /fresh/ignored/capture-proof
```

This WebKit iPhone gate checks exact English MENU pixels, touch dial input,
source/LCD frame cadence, a Full shutter tap, a JPEG download through the card
UI, and the same saved file decoded by stock Playback. It saves viewport and
LCD screenshots without modifying served source or installing firmware patches.

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
