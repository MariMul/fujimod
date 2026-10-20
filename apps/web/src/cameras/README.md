# Browser camera library

Each supported firmware upload creates a separate virtual camera record.
The homepage lists those records without importing the emulator or loading its
runtime. Selecting a record reads its DAT from IndexedDB and verifies it through
the ordinary catalog path.

`store.ts` owns the versioned `fujimod-cameras` database. Metadata, firmware and
execution saves use separate stores. Atomic save transactions use optimistic
revisions, and failed writes preserve the previous successful save.

The native engine currently provides live execution and owner-bound Undo.
Durable machine saves and restoration require a separate C ABI format and are
unavailable; the engine reports that limitation explicitly. Stored firmware and
existing saved execution data are retained.

Run store and component tests with `npm test`. Check lazy startup independently
in Chromium and WebKit with a development server:

```sh
FUJIMOD_WEB_URL=https://your-host.example/ node src/cameras/startup.browser.ts
```

This refreshes empty and populated libraries three times, asserting that no
firmware is selected, no emulator starts and records remain unchanged.
Screenshots go under ignored `build/camera-library/`.
