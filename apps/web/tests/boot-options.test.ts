import { expect, test } from 'vite-plus/test';
import type { PhysicalControls } from 'fujimod';
import x70 from '../../../catalog/bodies/x70/controls.json';
import xm5 from '../../../catalog/bodies/xm5/controls.json';
import { bootCombination, panelBootCombos } from '../src/cameras/boot-options';
import { serviceModeEntry } from '../src/dev-tools/service-mode-entry';

test('controls preserve the X70 startup inputs and service guide', () => {
  const combos = panelBootCombos(x70 as PhysicalControls, ['sd', 'firmware_update', 'service']);
  expect(combos.map((combo) => combo.inputs)).toEqual([
    [{ id: 'command_control.press' }, { id: 'movie_record_button' }],
    [{ id: 'disp_back_button' }],
    [{ id: 'usb', position: 'on' }, { id: 'selector.up' }, { id: 'shutter_button', stage: 'full' }],
  ]);
  expect(serviceModeEntry).toEqual([
    'Connect the camera to this computer with a USB cable.',
    'Switch the camera OFF.',
    'Hold UP on the selector and press the shutter button all the way down.',
    'Keep holding both and switch the camera ON.',
    'Release the buttons when the green lamp lights.',
  ]);
});

test('an experimental X-M5 entry is absent from the confirmed controls', () => {
  expect(() => bootCombination(xm5 as PhysicalControls, 'sd')).toThrow('No sd boot combination');
});
