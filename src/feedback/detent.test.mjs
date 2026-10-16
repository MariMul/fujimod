import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import ts from 'typescript';
const source = ts.transpileModule(readFileSync(new URL('./detent.ts', import.meta.url), 'utf8'), { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext } }).outputText;
const { DetentFeedback } = await import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`);

test('audio is gesture-only, uses transient session and bounded click envelopes', () => {
  let contexts = 0, starts = 0, stopped;
  Object.defineProperty(globalThis, 'navigator', { configurable: true, value: { audioSession: { type: 'auto' } } });
  globalThis.document = { hidden: false };
  const parameter = { setValueAtTime() {}, exponentialRampToValueAtTime() {} };
  globalThis.AudioContext = class {
    state = 'running'; currentTime = 1; destination = {};
    constructor() { contexts++; }
    createOscillator() { return { frequency: parameter, connect() {}, disconnect() {}, start() { starts++; }, stop(at) { stopped = at; } }; }
    createGain() { return { gain: parameter, connect() {}, disconnect() {} }; }
  };
  const feedback = new DetentFeedback(); feedback.step(); feedback.beginGesture({ isTrusted: false });
  assert.equal(contexts, 0);
  feedback.beginGesture({ isTrusted: true }); feedback.step();
  assert.equal(contexts, 1); assert.equal(starts, 1); assert.equal(stopped, 1.018);
  assert.equal(navigator.audioSession.type, 'transient');
  document.hidden = true; feedback.step(); assert.equal(starts, 1);
});
test('supported haptics avoid audio, rejected vibration falls back to audio', () => {
  const vibrations = []; let contexts = 0;
  globalThis.navigator.vibrate = n => { vibrations.push(n); return true; };
  globalThis.AudioContext = class { constructor() { contexts++; } };
  const feedback = new DetentFeedback(); feedback.beginGesture({ isTrusted: true }); feedback.step();
  assert.deepEqual(vibrations, [0, 8]); assert.equal(contexts, 0);
  navigator.vibrate = () => false;
  feedback.beginGesture({ isTrusted: true }); assert.equal(contexts, 1);
  navigator.vibrate = () => { throw Error('Unavailable'); };
  new DetentFeedback().beginGesture({ isTrusted: true }); assert.equal(contexts, 2);
});
