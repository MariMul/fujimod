# Dev Tools: tool deck and debug mode

Agreed with the owner on 2026-10-19. This records the design; implementation
follows it. The service-mode walkthrough itself already exists as the setup
wizard (`../../SETUP-WIZARD.md`, route `/setup/`); how the two combine is not
yet decided.

## Start screen: the tool deck

Most visitors arrive without a camera in debug mode, so Dev Tools opens on a
deck of tool cards, not on the console.

- **Enter Debug Mode** is the large primary card: title, one-line summary
  ("In debug mode the camera works normally, but USB can inspect and control
  its software. Plug USB in twice for ordinary file transfer. Required for
  other Dev Tools."), and **Show Instructions**.
- Tools that need a debug-mode camera stay visible, locked, with the reason
  ("Needs a camera in debug mode"): Console, Share Camera, **Leave Debug
  Mode**, and later tools.
- Always available: Try It in the Emulator (saved firmware), Bug Reports.
- When a debug-mode camera connects, the deck collapses to a top rail and the
  console takes the space.
- Every new tool is a card in one of three states: available, locked (with its
  reason) or connected.

## Enter Debug Mode wizard

Before step 1, a brief **overview** tells the user what they are getting into:
what debug mode is, what it changes (USB plugged twice for file transfer), the
steps ahead (service mode, a setting change, battery removal), that service
mode factory-resets on a normal shutdown, and roughly how long it takes. The
user starts the wizard from there.

Steps, on a numbered rail that fills as each completes:

1. **Camera.** Saved emulator cameras as tiles, X70 1.10 first. Other bodies
   are listed as "Not supported yet": their service-mode entry and reset guard
   are untraced (`research/debug-mode-generations`).
2. **Service mode.** Connect USB, switch off, then hold UP and press the
   shutter fully while switching ON; the green LED lights (X70: the USB, UP and
   full-shutter key states are read at power-on; see `../../SETUP-WIZARD.md`). A fixed warning: in service mode
   do not use the OFF switch, because shutdown factory-resets the camera;
   restart by removing the battery. A large checkbox ("I'm in service mode,
   the green LED is on, and USB is connected") enables **Allow Camera Access**.
3. **Connect.** WebUSB chooser; the page points up to where the chooser opens
   (top centre, under the address bar) and names the device to pick (the
   unknown FUJIFILM device, `04cb:ff80`; a screenshot from the owner will
   follow). After access: confirm service mode (UI 12, system 7), then arm the
   RAM reset guard `a3=1` at once, as `tools/x70_service_exit.py --execute`
   does, so an accidental OFF does not reset the camera.
4. **Enter Debug Mode.** One large button: persist `f7=1` with the range save
   that leaves stored `a3` at 0 (`research/persistent-debug-save`), then read
   it back. A terminal-style log shows the real steps at their real pace.
5. **Restart.** Only after the readback confirms the save: remove the battery,
   reinsert it and switch on normally. Then: the camera works as usual; plug USB
   in twice for file transfer.

Exit is by **battery removal** (owner decision). It bypasses the shutdown
reset path; it has not yet been exercised on a camera.

## Leave Debug Mode

A separate tool, locked unless a camera in debug mode is connected. It
restores stored `f7=0`. The rollback has not been validated on hardware.

## Look and motion

- Current dark Dev Tools palette; monospace uppercase labels and step numbers;
  body text 18–20px; buttons 56px tall, full width on phones.
- One phosphor-green accent matching the camera's green LED; amber only for
  the OFF-switch warning; red only for errors.
- Steps change with a short sideways slide and scan-line wipe (~250ms); the
  rail segment fills as a step completes; the step-4 log types at the real
  pace. A slow pulse marks only the next button to press. Everything is still
  under `prefers-reduced-motion`.
