import type { BodyBootOptions, Catalog_BootOption } from 'fujimod';
import type { DevicePanel, PowerCombo } from '@fujimod/emulator-ui/ui/layout';

/** Resolve an explicitly recorded option without promoting provisional instructions. */
export function bootOption(document: BodyBootOptions, id: string): Catalog_BootOption {
  const option = document.boot_options[id];
  if (!option || option.status === 'candidate' || option.status === 'superseded') {
    throw new Error(`Boot option ${document.body_id}/${id} is not an active procedure`);
  }
  return option;
}

/** Convert selected, implemented startup combinations into panel input declarations. */
export function panelBootCombos(
  document: BodyBootOptions,
  controls: Record<string, { behavior: { kind: string } | null }>,
  ids: string[],
): DevicePanel['combos'] {
  return ids.map((id): PowerCombo => {
    const option = bootOption(document, id);
    if (option.status !== 'implemented') throw new Error(`Boot option ${id} is not implemented`);
    const inputs: PowerCombo['inputs'] = [];
    const keys: PowerCombo['keys'] = [];
    if (option.entry.usb === 'connected') {
      inputs.push({ id: 'usb', position: 'on' });
      keys.push({ face: 'USB' });
    }
    for (const key of option.entry.hold) {
      if (!key.control_id) throw new Error(`Boot control ${key.label} has no resolved identity`);
      const control = controls[key.control_id];
      if (!control) throw new Error(`Boot control ${key.control_id} is missing`);
      const rotary = ['dial', 'ring'].includes(control.behavior?.kind ?? '');
      const inputId =
        key.action === 'up'
          ? `${key.control_id}.up`
          : rotary
            ? `${key.control_id}.press`
            : key.control_id;
      inputs.push(key.action === 'full_press' ? { id: inputId, stage: 'full' } : { id: inputId });
      keys.push(
        key.action === 'up'
          ? { face: '▲' }
          : key.action === 'full_press'
            ? { face: 'SHUTTER' }
            : key.control_id === 'movie_record_button'
              ? { record: true }
              : rotary
                ? { face: 'CMD ●' }
                : { face: key.label },
      );
    }
    return {
      label: option.label,
      inputs,
      keys,
      ...(option.group === 'sd_boot' ? { boot: 'sd' as const } : {}),
    };
  });
}
