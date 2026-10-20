import { validateCatalog, type Catalog } from '../../catalog';
import type { ToolSession } from '../camera-model';
import type { DebugModeOperations } from './Wizard';
import type { LeaveDebugModeOperation } from './LeaveDebugMode';

export type DebugModeSession = ToolSession['debug'];

declare const __FUJIMOD_CATALOG__: string;
async function definition(firmwareId: string) {
  const response = await fetch(new URL(__FUJIMOD_CATALOG__, document.baseURI));
  if (!response.ok) throw Error('Could not load the firmware catalog.');
  const catalog = validateCatalog((await response.json()) as Catalog);
  const firmware = catalog.firmwares.find((entry) => entry.id === firmwareId);
  if (!firmware?.camera_table) throw Error('The selected firmware has no camera table.');
  const table = await fetch(new URL(firmware.camera_table, document.baseURI));
  if (!table.ok) throw Error('Could not load the camera table.');
  return [await table.text(), firmware.id, firmware.sha256] as const;
}

/** Run each debug-mode step in its own session on the selected camera. */
export function debugModeOperations(
  run: <T>(tool: string, operation: (debug: DebugModeSession) => Promise<T>) => Promise<T>,
): DebugModeOperations & LeaveDebugModeOperation {
  async function step(
    tool: string,
    method: keyof DebugModeSession,
    firmwareId: string,
    progress: (line: string) => void,
  ) {
    const args = await definition(firmwareId);
    const result = await run(tool, async (debug) =>
      method === 'armResetGuard'
        ? debug.armResetGuard(...args)
        : method === 'enterDebugMode'
          ? debug.enterDebugMode(...args)
          : debug.leaveDebugMode(...args),
    );
    for (const line of result.progress) progress(line);
  }
  return {
    armResetGuard: (firmwareId, progress) =>
      step('Enable debug mode', 'armResetGuard', firmwareId, progress),
    enter: (firmwareId, progress) =>
      step('Enable debug mode', 'enterDebugMode', firmwareId, progress),
    leave: (progress) => step('Disable debug mode', 'leaveDebugMode', 'x70-110', progress),
  };
}
