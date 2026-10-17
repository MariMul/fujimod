import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { transformWithOxc } from 'vite';
const { code: soundSource } = await transformWithOxc(readFileSync(new URL('./detent-sound.ts', import.meta.url), 'utf8'), 'detent-sound.ts');
const soundURL = `data:text/javascript;base64,${Buffer.from(soundSource).toString('base64')}`;
const input = readFileSync(new URL('./detent.ts', import.meta.url), 'utf8').replace("'./detent-sound'", JSON.stringify(soundURL));
const { code: source } = await transformWithOxc(input, 'detent.ts');
const { DetentFeedback } = await import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`);

test('audio is gesture-only, reuses short PCM buffers, and disconnects finished sources', () => {
  let contexts = 0, buffers = 0, disconnected = 0;
  const played = [];
  Object.defineProperty(globalThis, 'navigator', { configurable: true, value: { audioSession: { type: 'auto' } } });
  globalThis.document = { hidden: false };
  globalThis.AudioContext = class {
    state = 'running'; sampleRate = 48000; destination = {};
    constructor() { contexts++; }
    createBuffer(channels, length, rate) {
      buffers++;
      assert.equal(channels, 1); assert.equal(rate, 48000);
      assert.equal(length, 1152);
      return { copyToChannel(samples, channel) {
        assert.equal(channel, 0); this.samples = samples;
      } };
    }
    createBufferSource() {
      return { playbackRate: {}, connect() {}, disconnect() { disconnected++; },
        start() { played.push(this); } };
    }
  };
  const feedback = new DetentFeedback(); feedback.step(); feedback.beginGesture({ isTrusted: false });
  assert.equal(contexts, 0);
  feedback.beginGesture({ isTrusted: true });
  feedback.step('soft'); feedback.step('soft'); feedback.step('firm');
  assert.equal(contexts, 1); assert.equal(played.length, 3); assert.equal(buffers, 2);
  assert.equal(played[0].buffer, played[1].buffer);
  assert.notDeepEqual(played[0].buffer.samples, played[2].buffer.samples);
  for (const source of played) {
    assert(source.playbackRate.value >= .985 && source.playbackRate.value <= 1.015);
    source.onended();
  }
  assert.equal(disconnected, 3);
  assert.equal(navigator.audioSession.type, 'transient');
  document.hidden = true; feedback.step(); assert.equal(played.length, 3);
});
test('supported haptics avoid audio, rejected vibration falls back to audio', () => {
  const vibrations = []; let contexts = 0;
  globalThis.navigator.vibrate = n => { vibrations.push(n); return true; };
  globalThis.AudioContext = class { constructor() { contexts++; } };
  const feedback = new DetentFeedback(); feedback.beginGesture({ isTrusted: true, pointerType: 'touch' }); feedback.step();
  assert.deepEqual(vibrations, [0, 8]); assert.equal(contexts, 0);
  navigator.vibrate = () => false;
  feedback.beginGesture({ isTrusted: true, pointerType: 'touch' }); assert.equal(contexts, 1);
  navigator.vibrate = () => { throw Error('Unavailable'); };
  new DetentFeedback().beginGesture({ isTrusted: true, pointerType: 'touch' }); assert.equal(contexts, 2);
});

test('mouse and keyboard use audio even when vibration requests are accepted', () => {
  const vibrations = []; let contexts = 0;
  navigator.vibrate = n => { vibrations.push(n); return true; };
  globalThis.AudioContext = class { constructor() { contexts++; } };
  new DetentFeedback().beginGesture({ isTrusted: true, pointerType: 'mouse' });
  new DetentFeedback().beginGesture({ isTrusted: true });
  assert.deepEqual(vibrations, []);
  assert.equal(contexts, 2);
});
