import type { Input } from 'fujimod/emulator';
import { DEVICE_PANEL_SCHEMA, type ControlNode, type DevicePanel } from './layout';

/** A catalog supplies capabilities even when the body has no custom presentation. */
export function inputPanel(inputs: readonly Input[], body: string): DevicePanel {
  return {
    schema: DEVICE_PANEL_SCHEMA,
    body,
    combos: [],
    layout: inputs
      .filter((input) => input.kind !== 'touch')
      .map((input): ControlNode => ({
        kind: 'control',
        id: input.id,
        variant:
          input.relative || (input.positions?.length ?? 0) > 5
            ? 'dial'
            : input.positions?.length
              ? 'segment'
              : input.stages?.some((stage) => stage.id === 'half') &&
                  input.stages.some((stage) => stage.id === 'full')
                ? 'shutter'
                : 'key',
        labels:
          input.positions &&
          Object.fromEntries(input.positions.map((position) => [position.id, position.label])),
      })),
  };
}
