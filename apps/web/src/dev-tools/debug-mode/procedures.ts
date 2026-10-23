import controls from '../../../../../catalog/bodies/x70/controls.json';
import { bootCombination, describeBootHold } from '../../cameras/boot-options';
import type { PhysicalControls } from 'fujimod';

/** Per-firmware support for the Enter Debug Mode wizard; body entry comes from the catalog. */
export interface DebugModeProcedure {
  firmwareId: string;
  model: string;
  version: string;
  /** Service-mode entry, in order, as shown to the owner. */
  entry: string[];
  /** Why the camera cannot use the wizard yet, when it cannot. */
  unsupported?: string;
}
const untraced = 'Its service-mode entry and its factory-reset guard have not been traced yet.';
const bodyControls = controls as PhysicalControls;
const service = bootCombination(bodyControls, 'service');
const holds = service.hold.map((input) => describeBootHold(bodyControls, input)).join(' and ');
export const serviceModeEntry = [
  ...(service.usb === true ? ['Connect the camera to this computer with a USB cable.'] : []),
  'Switch the camera OFF.',
  `${holds[0]!.toUpperCase()}${holds.slice(1)}.`,
  'Keep holding both and switch the camera ON.',
  'Release the buttons when the green lamp lights.',
];
export const procedures: DebugModeProcedure[] = [
  {
    firmwareId: 'x70-110',
    model: 'X70',
    version: '1.10',
    entry: serviceModeEntry,
  },
];
export function procedureFor(firmwareId: string): DebugModeProcedure | undefined {
  return procedures.find((procedure) => procedure.firmwareId === firmwareId);
}
export function unsupportedReason(firmwareId: string): string {
  const procedure = procedureFor(firmwareId);
  return procedure ? (procedure.unsupported ?? '') : untraced;
}
