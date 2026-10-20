/** Per-firmware facts for the Enter Debug Mode wizard. The wizard itself is generic. */
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
export const procedures: DebugModeProcedure[] = [
  {
    firmwareId: 'x70-110',
    model: 'X70',
    version: '1.10',
    entry: [
      'Connect the camera to this computer with a USB cable.',
      'Switch the camera OFF.',
      'Hold UP on the selector and press the shutter button all the way down.',
      'Keep holding both and switch the camera ON.',
      'Release the buttons when the green lamp lights.',
    ],
  },
];
export function procedureFor(firmwareId: string): DebugModeProcedure | undefined {
  return procedures.find((procedure) => procedure.firmwareId === firmwareId);
}
export function unsupportedReason(firmwareId: string): string {
  const procedure = procedureFor(firmwareId);
  return procedure ? (procedure.unsupported ?? '') : untraced;
}
