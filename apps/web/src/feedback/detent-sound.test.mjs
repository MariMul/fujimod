import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { transformWithOxc } from 'vite';
const { code } = await transformWithOxc(readFileSync(new URL('./detent-sound.ts', import.meta.url), 'utf8'), 'detent-sound.ts');
const { renderDetentSound } = await import(`data:text/javascript;base64,${Buffer.from(code).toString('base64')}`);

for (const sampleRate of [44100, 48000]) {
  test(`mechanical clicks are short, quiet transients with distinct dial timbres at ${sampleRate} Hz`, () => {
    const signals = ['soft', 'firm', 'crisp', 'fine', 'compact'].map(sound => renderDetentSound(sound, sampleRate));
    const energy = signal => signal.reduce((sum, value) => sum + value * value, 0);
    for (const signal of signals) {
      assert.equal(signal.length, Math.ceil(sampleRate * .024));
      assert.equal(Math.abs(signal[0]), 0); assert.equal(Math.abs(signal.at(-1)), 0);
      assert(signal.every(Number.isFinite));
      const peak = Math.max(...signal.map(Math.abs));
      assert(peak > .02 && peak < .2, `unexpected peak ${peak}`);
      // Most energy is contact/body impact; the sound cannot sustain a beep.
      assert(energy(signal.slice(Math.floor(sampleRate * .012))) < energy(signal) * .01);
      assert(peak / Math.sqrt(energy(signal) / signal.length) > 4);
    }
    for (let i = 0; i < signals.length; i++)
      for (let j = 0; j < i; j++)
        assert(energy(signals[i].map((value, n) => value - signals[j][n])) > .1 * energy(signals[i]));
    assert.deepEqual(renderDetentSound('soft', sampleRate), signals[0]);
  });
}
