// Deterministic host input for local fault inspection; all replies execute in firmware.
export const scenarios: Record<string, { firmware: string; commands: readonly string[] }> = {
  'console-jpeg': { firmware: 'x70-110', commands: ['help', 'dcfcc'] },
};
