# Dev Tools: tool deck and debug mode

Agreed with the owner on 2026-10-19. This records the design; implementation
follows it. This wizard is part of Dev Tools and independent of the animated
setup wizard (`../../SETUP-WIZARD.md`), by owner decision. The camera
operations (service-mode check, reset guard, debug-mode save and rollback) go
in a Rust workflow crate beside `fujimod-usb`, shared by every caller.

## Start screen: the tool deck

Dev Tools opens on a service-mode entry guide, a grouped camera/simulator
connection box, and tool cards. Service mode and debug mode both expose the
ff80 connection; neither a model list nor the Debug Mode setting gates a
connection. A usable ff80 session is sufficient.

The service-mode guide covers the power-on chord, the OFF-switch warning and
battery-removal exit. It stops at connecting and never changes a setting.
The separate Enter Debug Mode wizard retains the verified persistent workflow.

Once connection activity starts, a 350px tool column sits beside the persistent
console, layer/level filters and command composer. Console, Inspector, Enter/Leave
Debug Mode, Share Camera, Bug Report and a back-to-deck link live on the left.
The log remains visible through tool changes and sharing. On phones the log
stacks below the tools.

Inspector exposes only identity selectors, configuration geometry and reads,
and bounded memory reads. Configuration reads use the camera's reported RAM
capacity and read size, with a 64 MiB host allocation budget and 16 KiB transfer
budget. The diagnostic JSON bundle retains successful reads, raw reply bytes,
and failed requests. No Inspector operation writes memory or configuration.

Every tool forms its command from the information it has and verifies the
camera's reply. It refuses when the command cannot be formed or verified.
Connection does not establish that a particular operation will succeed.

## Enter Debug Mode wizard

Before step 1, a brief **overview** tells the user what they are getting into:
what debug mode is, what it changes (USB plugged twice for file transfer), the
steps ahead (service mode, a setting change, battery removal), that service
mode factory-resets on a normal shutdown, and roughly how long it takes. The
user starts the wizard from there.

Steps, on a numbered rail that fills as each completes:

1. **Camera.** Saved emulator cameras as tiles, X70 1.10 first. Other bodies
   are listed as "Not supported yet": their service-mode entry and reset guard
   are untraced.
2. **Service mode.** Connect USB, switch off, then hold UP and press the
   shutter fully while switching ON; the green LED lights (X70: the USB, UP and
   full-shutter key states are read at power-on; see `../../SETUP-WIZARD.md`). A fixed warning: in service mode
   do not use the OFF switch, because shutdown factory-resets the camera;
   restart by removing the battery. A large checkbox ("I'm in service mode,
   the green LED is on, and USB is connected") enables **Allow Camera Access**.
3. **Connect.** WebUSB chooser; the page points up to where the chooser opens
   (top center, under the address bar) and names the device to pick (the
   unknown FUJIFILM device, `04cb:ff80`; a screenshot from the owner will
   follow). After access: confirm service mode (UI 12, system 7), then arm the
   RAM reset guard `a3=1` at once so an accidental OFF does not reset the camera.
4. **Enter Debug Mode.** One large button: persist `f7=1` with the range save
   that leaves stored `a3` at 0, then read
   it back. A terminal-style log shows the real steps at their real pace.
5. **Restart.** Only after the readback confirms the save: remove the battery,
   reinsert it and switch on normally. Then: the camera works as usual; plug USB
   in twice for file transfer.

Exit is by **battery removal** (owner decision). It bypasses the shutdown
reset path; it has not yet been exercised on a camera.

## Leave Debug Mode

A separate tool available with a connected ff80 session. It
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
