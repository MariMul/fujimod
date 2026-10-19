# Browser camera library

Each firmware upload or drop creates an independent virtual camera. The homepage
opens the library without starting an emulator; selecting a saved camera reads
its DAT from IndexedDB and verifies it through the ordinary catalog path.

`store.ts` owns the versioned `fujimod-cameras` database. Camera metadata, firmware
bytes and execution saves use separate object stores so listing cameras does not
read firmware or machine memory. A save transaction replaces durable media and
its optional live snapshot together, and increments an optimistic revision.
A stale tab cannot overwrite a newer revision. Storage failures are reported in
the library while the previous transaction remains available.

The engine's checked boot transport exports committed internal storage and sparse
SD sectors, including ejected media. Pending device operations are excluded from
a power-cut save. Power-off, switching cameras and returning to the library save
before releasing the worker. Live sessions save approximately every 30 seconds,
and hiding the document requests an additional best-effort save. Closing a tab
cannot await an asynchronous write; the most recent committed save is retained.

Live snapshots are taken only at safe emulator boundaries. They resume only when
the DAT, runtime, input profile, protocol and boot-media identities match. A cold
boot uses the saved durable media when a snapshot is unavailable or incompatible.
Physical switch positions and mod selection belong to the owned camera too.
An OFF camera reopens OFF, and the viewer defers scene input until power-on.
The firmware boot cache used by unowned sessions is separate from this library.

Run component and engine tests with `npm test`. With a development server and an
emulation-supported DAT, exercise actual browser storage and native inputs with:

```sh
FUJIMOD_WEB_URL=https://your-host.example/ \
FUJIMOD_TEST_DAT=/path/to/FPUPDATE.DAT \
node tests/camera-library.browser.ts
```

Screenshots are written under ignored `build/camera-library/`. The browser test
uses a fresh context and never connects to a physical camera. It exercises both
upload and drop, resumes a live snapshot, captures a photo through the camera
controls, and compares its downloaded SHA-256 after power-off and reload. A
second camera created from the same DAT must have its own empty photo library.
It also checks that stale revisions and failed storage transactions preserve
the previous successful save.

With a local development DAT configured, check startup independently in Chromium
and WebKit:

```sh
FUJIMOD_WEB_URL=https://your-host.example/ node src/cameras/startup.browser.ts
```

This refreshes both empty and populated libraries three times, asserting that
no firmware is selected, no emulator starts, and all camera records stay unchanged.
