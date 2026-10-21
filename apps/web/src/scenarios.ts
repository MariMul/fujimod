import type { ReplayStep } from '@fujimod/emulator-ui/engine/api';

export interface ReplayScenario {
  firmwareId: string;
  firmwarePath: string;
  clockSeconds2000?: number;
  interactive: boolean;
  steps: readonly ReplayStep[];
}

// Camera-specific input policy lives here, outside the reusable execution adapter.
const scenarios: Record<string, ReplayScenario> = {
  'x70-movie': {
    firmwareId: 'x70-110',
    firmwarePath: '/local/firmware.dat',
    clockSeconds2000: 845000000, // Fixed checked graph RTC input.
    interactive: false,
    steps: [
      { action: 'press', id: 'movie_record_button' },
      { action: 'release', id: 'movie_record_button' },
      { action: 'advance', ticks: 120000000 },
    ],
  },
  'x70-lcd': {
    firmwareId: 'x70-110',
    firmwarePath: '/local/firmware.dat',
    clockSeconds2000: 845000000,
    interactive: true,
    steps: [],
  },
};

export function scenarioFromUrl(search: string): ReplayScenario {
  const params = new URLSearchParams(search);
  const id = params.get('scenario') ?? '';
  if (id === 'camera-rec') {
    const firmwareId = params.get('firmware') ?? '';
    if (!/^[a-z0-9]+-[0-9]+$/.test(firmwareId)) throw Error('Invalid simulator firmware ID.');
    return {
      firmwareId,
      firmwarePath: `/local/firmware/${firmwareId}.dat`,
      interactive: true,
      steps: [],
    };
  }
  if (!Object.hasOwn(scenarios, id)) throw Error('Unknown simulator scenario: ' + id);
  return scenarios[id];
}
