import type { ReplayStep } from './engine/api';

export interface ReplayScenario {
  firmwareId: string;
  clockSeconds2000: number;
  steps: readonly ReplayStep[];
}

// Camera-specific input policy lives here, outside the reusable execution adapter.
const scenarios: Record<string, ReplayScenario> = {
  'x70-movie': {
    firmwareId: 'x70-110',
    clockSeconds2000: 845510400, // 2026-10-17 00:00:00, fixed RTC input.
    steps: [
      { action: 'press', id: 'movie' },
      { action: 'release', id: 'movie' },
      { action: 'advance', ticks: 120000000 },
    ],
  },
};

export function scenarioFromUrl(search: string): ReplayScenario {
  const id = new URLSearchParams(search).get('scenario') ?? '';
  if (!Object.hasOwn(scenarios, id)) throw Error('Unknown simulator scenario: ' + id);
  return scenarios[id];
}
