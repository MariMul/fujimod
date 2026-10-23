import bootData from '../../../../../catalog/bodies/x70/boot.json';
import { bootOption } from '../../cameras/boot-options';
import type { BodyBootOptions } from 'fujimod';

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
export const serviceModeEntry = bootOption(bootData as BodyBootOptions, 'service-mode').entry
  .instructions;
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
