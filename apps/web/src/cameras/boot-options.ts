import type { BootCombination, BootHold, BootMode, PhysicalControls } from 'fujimod';
import type { DevicePanel, PowerCombo } from '@fujimod/emulator-ui/ui/layout';

export function bootCombination(document: PhysicalControls, mode: BootMode): BootCombination {
  const combination = document.boot?.[mode];
  if (!combination) throw new Error(`No ${mode} boot combination for ${document.body_id}`);
  return combination;
}

export function describeBootHold(document: PhysicalControls, input: BootHold): string {
  const [id, action] = input.id.split('.');
  const control = document.controls[id!];
  if (!control) throw new Error(`Unknown boot control ${input.id}`);
  if (control.behavior?.kind === 'pad' && action) {
    return `hold ${action.toUpperCase()} on the ${control.name.split('/')[0]!.toLowerCase()}`;
  }
  if (input.stage === 'full') return `press the ${control.name.toLowerCase()} all the way down`;
  if (action === 'press') return `press in and hold the ${control.name.toLowerCase()}`;
  return `hold ${control.face ?? control.name}`;
}

export function panelBootCombos(
  document: PhysicalControls,
  modes: BootMode[],
): DevicePanel['combos'] {
  const labels: Record<BootMode, string> = {
    sd: 'SD card boot',
    firmware_update: 'Firmware update',
    service: 'Service mode',
  };
  return modes.map((mode): PowerCombo => {
    const combination = bootCombination(document, mode);
    const inputs: PowerCombo['inputs'] = combination.hold.map(({ id, stage }) =>
      stage == null ? { id } : { id, stage },
    );
    const keys: PowerCombo['keys'] = combination.hold.map((input) => {
      const [id, action] = input.id.split('.');
      const control = document.controls[id!];
      if (!control) throw new Error(`Unknown boot control ${input.id}`);
      if (control.behavior?.kind === 'pad' && action) {
        const arrows: Record<string, string> = { up: '▲', down: '▼', left: '◀', right: '▶' };
        return { face: arrows[action] ?? action };
      }
      if (input.stage === 'full') return { face: 'SHUTTER' };
      if (action === 'press') return { face: 'CMD ●' };
      if (control.name.toLowerCase().includes('movie')) return { record: true };
      return { face: control.face ?? control.name };
    });
    if (combination.usb != null) {
      inputs.unshift({ id: 'usb', position: combination.usb ? 'on' : 'off' });
      keys.unshift({ face: combination.usb ? 'USB' : 'USB OFF' });
    }
    return { label: labels[mode], inputs, keys, ...(mode === 'sd' ? { boot: 'sd' as const } : {}) };
  });
}
