# Release downloads (design)

Agreed 2026-10-26. Replaces the compile-time download flags described in
[DOWNLOADS.md](DOWNLOADS.md) once implemented.

## Releases without a website deployment

The website reads the current release at runtime instead of bundling
`artifacts/patches.json` at deploy time. GitHub Releases stays the only place
releases live.

Browser JavaScript can read GitHub's release list
(`api.github.com/repos/justjake/fujimod/releases` sends
`access-control-allow-origin: *`, 60 unauthenticated requests per hour per
visitor), but not release files: every download path ends at
`release-assets.githubusercontent.com`, which sends no CORS header. The
`fujimod-web` Worker therefore adds a stateless pass-through:

- `/releases/current` names the newest published release that has a
  `patches.json` (pre-releases included).
- `/releases/<tag>/<file>` fetches that release file from GitHub server-side.
- Cloudflare's edge cache keeps a short lifetime. The browser still verifies
  every file's size and SHA-256 as it does today.

## Release contents

`patches.json` gains, per firmware entry, two download switches, both false
unless the release sets them:

- `sd_card`: offer FujiMod SD boot for this body.
- `installable`: offer FujiMod installable (SD boot files plus `FPUPDATE.DAT`).
  Accepted only when the release contains that body's `FPUPDATE.DAT` output.

An optional `notes.md` (one per release, not per body) is referenced from
`patches.json` by name, size and SHA-256. Without it no notes are shown. The
site renders it with a restricted Markdown renderer: headings, lists,
emphasis, code and links (opening in a new tab); no raw HTML or images.

## Emulator section

Desktop: two columns inside the existing emulator panel; the inner launcher
card is removed. Phone: one column directly on the page.

**Left: the release panel.** One rounded panel headed with the release name
and date contains the rendered notes (tall on desktop, scrolling) and four
rows styled like the camera list rows (icon, title, subtitle, chevron):

| Row | Subtitle with a chosen DAT | Unavailable |
| --- | --- | --- |
| Download FujiMod installable | `X70 • 1.10 • Includes SD boot version` | *Not available for this camera yet* |
| Download FujiMod SD boot | `X70 • 1.10 base firmware` | *Not available for this camera yet* |
| Run FujiMod in your browser | `X70 • 1.10 with FujiMod 0.0.1` | |
| Run unmodified in your browser | `X70 • 1.10 official firmware` | |

Rows are always shown on desktop; an unavailable row is dimmed without a
chevron. Before any firmware is chosen, every row is disabled with a
descriptive subtitle (Includes SD boot version / Dual boot from an SD card /
FujiMod on an emulated camera / Official firmware, emulated) and a tinted
banner above them reads "Add base firmware to continue" with an arrow toward
Base firmware. On a phone, the banner and rows are hidden until firmware is
chosen.

**Right, top: Base firmware.** Heading "Base firmware", description
"FujiMod is assembled in your browser by patching official firmware you
provide". Saved firmware uses Recovery's radio rows with the subtitle
"Saved in your browser • Remove". Below the group, a separate rounded drop
zone with a dashed stroke: "Add official firmware", "Drop a Fujifilm .DAT file
here, or choose a file". With no saved firmware it is the only element and is
tinted blue. The most recently added firmware is selected automatically.

**Right, below: Get from fujifilm.com.** The existing search and camera list,
filling the remaining height with a minimum of 4.5 rows. The body badge reads
"Moddable" and sits beside the camera name instead of in its own grid column,
so processor names stay on one line (this also fixes Recovery's list).

## Download modals

Each download row opens a modal built from the same step components as the
Recovery page (numbered steps, file list, `PowerComboKeys` from the body's
boot data):

- **FujiMod SD boot:** 1. Download the SD card files, with the disclaimer
  below and a blue button. 2. Copy them to a FAT32 SD card (`ffsys.bin`,
  `ffboot.bin`). 3. Boot from the SD card with the `sd` combination only.
- **FujiMod installable:** 1. Download the files, orange button.
  2. Copy all files including `FPUPDATE.DAT`. 3. Try it from the SD card first
  (`sd` combination) and shoot normally until satisfied. 4. Install it: turn
  the camera off and power on holding the `firmware_update` combination.
  5. Optional: SET-UP → FUJIMOD DEVTOOLS → BUILD OVERLAY → OFF.

Disclaimer, in bold above each download button:

> *No custom firmware is completely safe*. FujiMod is tested in our emulator
> and spot-checked on real camera hardware before each release, and we believe
> SD card dual boot is very low risk. We still do not guarantee this software
> for any purpose expressed or implied.

Followed by: "[Recovery tools](/recovery/) are available in the very unlikely
event this bricks your camera."

## Download filenames

Archives use `fujimod-{recovery,green-led,modified}-{sd-boot,installable}-{model}-{firmware}-build-{build}.zip`.
The model is lowercase with hyphens instead of spaces; firmware versions retain
periods, and the build is the website's eight-character Git commit SHA.
For example: `fujimod-recovery-sd-boot-x-t1-5.52-build-1234abcd.zip`.
Camera-required filenames inside each archive stay intact. Modified installable
archives include the SD boot files and `FPUPDATE.DAT`.
