import type { EngineConfig } from './api';
export function testConfig(sceneInput = true): EngineConfig {
  return { firmwareId: 'test', bundle: 'runtime/', sceneInput };
}
