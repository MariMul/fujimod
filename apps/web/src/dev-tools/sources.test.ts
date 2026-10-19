import { afterEach, test, vi } from 'vite-plus/test';
import assert from 'node:assert/strict';
import { WebUsbCamera } from './camera';
import { CameraSources } from './sources';

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

test('without WebUSB, page startup leaves USB Wasm to the simulator worker', async () => {
  vi.stubGlobal('navigator', {});
  const initialize = vi.spyOn(WebUsbCamera.prototype, 'initialize').mockResolvedValue();
  await new CameraSources().initialize();
  assert.equal(initialize.mock.calls.length, 0);
});

test('physical WebUSB is initialized before a device picker needs user activation', async () => {
  vi.stubGlobal('navigator', { usb: new EventTarget() });
  const initialize = vi.spyOn(WebUsbCamera.prototype, 'initialize').mockResolvedValue();
  await new CameraSources().initialize();
  assert.equal(initialize.mock.calls.length, 1);
});
