import { expect, test } from 'vite-plus/test';
import type { BodyBootOptions } from 'fujimod';
import x70 from '../../../catalog/bodies/x70/boot.json';
import xm5 from '../../../catalog/bodies/xm5/boot.json';
import xpro2 from '../../../catalog/bodies/xpro2/boot.json';
import controls from '../../../catalog/bodies/x70/controls.json';
import { bootOption, panelBootCombos } from '../src/cameras/boot-options';
import { serviceModeEntry } from '../src/dev-tools/debug-mode/procedures';

test('catalog startup inputs preserve the existing X70 panel and service guide', () => {
  const combos = panelBootCombos(x70 as BodyBootOptions, controls.controls, [
    'sd-boot',
    'firmware-update',
    'service-mode',
  ]);
  expect(combos.map((combo) => combo.inputs)).toEqual([
    [{ id: 'command_control.press' }, { id: 'movie_record_button' }],
    [{ id: 'disp_back_button' }],
    [{ id: 'usb', position: 'on' }, { id: 'selector.up' }, { id: 'shutter_button', stage: 'full' }],
  ]);
  expect(serviceModeEntry).toEqual(x70.boot_options['service-mode'].entry.instructions);
});

test('owner candidates and deleted manual procedures cannot become active options', () => {
  expect(() => bootOption(xm5 as BodyBootOptions, 'sd-entry-candidate')).toThrow(
    'not an active procedure',
  );
  expect(() => bootOption(xpro2 as BodyBootOptions, 'sd-download')).toThrow(
    'not an active procedure',
  );
});
