export { credit as previewCredit } from './parts.json';
import panelData from './panel.json';
import bootData from '../../../../catalog/bodies/x70/boot.json';
import controls from '../../../../catalog/bodies/x70/controls.json';
import { panelBootCombos } from '../../src/cameras/boot-options';
import type { BodyBootOptions } from 'fujimod';
import type { DevicePanel } from '@fujimod/emulator-ui/ui/layout';
export const panel: DevicePanel = {
  ...(panelData as DevicePanel),
  combos: panelBootCombos(bootData as BodyBootOptions, controls.controls, [
    'sd-boot',
    'firmware-update',
    'service-mode',
  ]),
};
export const preview = new URL('./preview.png', import.meta.url).href;

export const sdBoot = true;
