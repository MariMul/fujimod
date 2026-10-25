import controls from '../../../../catalog/bodies/x70/controls.json';
import { bootCombination, describeBootHold } from '../cameras/boot-options';
import type { PhysicalControls } from 'fujimod';

const bodyControls = controls as PhysicalControls;
const service = bootCombination(bodyControls, 'service');
const holds = service.hold.map((input) => describeBootHold(bodyControls, input)).join(' and ');
/** Service-mode entry, in order, as shown to the owner. */
export const serviceModeEntry = [
  ...(service.usb === true ? ['Connect the camera to this computer with a USB cable.'] : []),
  'Switch the camera OFF.',
  `${holds[0]!.toUpperCase()}${holds.slice(1)}.`,
  'Keep holding both and switch the camera ON.',
  'Release the buttons when the green lamp lights.',
];
