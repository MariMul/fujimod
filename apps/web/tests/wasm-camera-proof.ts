import { createEngine } from '../src/engine';
import type { EngineConfig } from '../src/engine/api';
import { engineReport } from './wasm-report';
const output = document.createElement('pre');
document.body.append(output);
let frames = 0,
  metrics = 0;
const faultMessages: unknown[] = [];
async function proveCamera() {
  const manifestResponse = await fetch('/__wasm-camera/wasm.json');
  if (!manifestResponse.ok) throw Error('Configure the camera construction artifacts.');
  const manifest = (await manifestResponse.json()) as {
    session: { firmwareId: string; clockSeconds2000: number };
  };
  const config: EngineConfig = {
    firmwareId: manifest.session.firmwareId,
    bundle: '/__wasm-camera/',
    inputs: '/runtime/wasm-proof/inputs.json',
    execution: { mode: 'replay', clockSeconds2000: manifest.session.clockSeconds2000 },
    boot: {
      datPath: '',
      prepare: { module: '', function: '', options: {} },
      module: '',
      sessionModule: '',
      limit: 0,
      step: 0,
      readyLimit: 0,
      readyWhen: [],
      sceneInput: false,
    },
  };
  const engine = createEngine({ ...config, adapter: 'wasm' }, () => {});
  engine.frames(() => {
    frames++;
  });
  engine.metrics(() => {
    metrics++;
  });
  engine.errors((fault) => faultMessages.push(fault));
  try {
    const response = await fetch('/__wasm-camera/firmware.dat');
    if (!response.ok) throw Error('Camera DAT is unavailable.');
    const dat = await response.arrayBuffer(),
      datBytes = dat.byteLength;
    const datSha256 = [...new Uint8Array(await crypto.subtle.digest('SHA-256', dat))]
      .map((b) => b.toString(16).padStart(2, '0'))
      .join('');
    const rejection = await engine
      .boot(dat, config, (message) => {
        output.textContent = message;
      })
      .then(
        () => undefined,
        (error) => error as Error,
      );
    if (!rejection)
      throw Error(
        'Construction now succeeds: update the browser gate to exercise guest execution.',
      );
    const report = await engineReport(await engine.errorReport());
    if (
      report.created !== false ||
      report.status !== null ||
      report.frame !== null ||
      frames ||
      metrics
    )
      throw Error('Construction refusal was reported as execution.');
    output.textContent = JSON.stringify(
      {
        firmwareId: config.firmwareId,
        datBytes,
        datSha256,
        created: false,
        status: null,
        frame: null,
        frames,
        metrics,
        faults: faultMessages,
        report,
      },
      null,
      2,
    );
    document.body.dataset.proof = 'refused';
  } catch (error) {
    output.textContent = String(error);
    document.body.dataset.proof = 'failed';
    throw error;
  } finally {
    engine.dispose();
  }
}
void proveCamera().catch((error) => {
  output.textContent = String(error);
  document.body.dataset.proof = 'failed';
});
