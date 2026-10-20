import { validateCatalog, type Catalog } from '../../catalog';
import type { DebugModeSession } from '../camera';
import type { DebugModeOperations } from './Wizard';
import type { LeaveDebugModeOperation } from './LeaveDebugMode';

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

export function sessionDebugModeOperations(
  session: DebugModeSession,
  currentSession: () => DebugModeSession | undefined,
): DebugModeOperations & LeaveDebugModeOperation {
  async function run(
    method: keyof DebugModeSession,
    firmwareId: string,
    progress: (line: string) => void,
  ) {
    const args = await definition(firmwareId);
    if (currentSession() !== session) throw Error('The camera disconnected. Connect it again.');
    const result = await session[method](...args);
    for (const line of result.progress) progress(line);
  }
  return {
    armResetGuard: (firmwareId, progress) => run('armResetGuard', firmwareId, progress),
    enter: (firmwareId, progress) => run('enterDebugMode', firmwareId, progress),
    leave: (progress) => run('leaveDebugMode', 'x70-110', progress),
  };
}
