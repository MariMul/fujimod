import type { InstallTarget } from '../../src/install/controller';
// X70 1.10 only: research/service-exit/README.md and tools/x70_sd_boot_image.py.
export const x70Installer: InstallTarget = {
  id: 'x70-110',
  model: 'X70',
  version: '1.10',
  serviceInstructions: [
    'Turn the camera off.',
    'Reboot the camera into service mode, then reconnect USB.',
  ],
  bootInstructions: [
    'With the camera off, press and hold the rear command dial and the red record button.',
    'Turn the camera on while holding both buttons.',
  ],
};
