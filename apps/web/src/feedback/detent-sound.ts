/** Brief contact noise plus damped body resonances, rather than a pitched beep. */
const sounds = {
  soft:    { body: 780,  metal: 2400, decay: .0028, contact: .0014, seat: .0012, level: .07 },
  firm:    { body: 1040, metal: 3300, decay: .0032, contact: .0017, seat: .0017, level: .075 },
  crisp:   { body: 1420, metal: 4600, decay: .0025, contact: .0011, seat: .0010, level: .08 },
  fine:    { body: 1850, metal: 5400, decay: .0017, contact: .0009, seat: .0007, level: .07 },
  compact: { body: 1220, metal: 3700, decay: .0021, contact: .0012, seat: .0009, level: .075 },
};
export type DetentSound = keyof typeof sounds;

/** Deterministic PCM, cached by the player at its actual output sample rate. */
export function renderDetentSound(sound: DetentSound, sampleRate: number): Float32Array<ArrayBuffer> {
  const profile = sounds[sound];
  const samples = new Float32Array(Math.ceil(sampleRate * .024));
  const cutoff = Math.min(6500, sampleRate * .4);
  const smoothing = 1 - Math.exp(-2 * Math.PI * cutoff / sampleRate);
  let seed = 0x6d2b79f5, filtered = 0, previous = 0;
  const impact = (time: number, noise: number) => {
    if (time < 0) return 0;
    const attack = Math.min(1, time / .00015);
    return attack * (
      .65 * noise * Math.exp(-time / profile.contact) +
      .55 * Math.sin(2 * Math.PI * profile.body * time) * Math.exp(-time / profile.decay) +
      .18 * Math.sin(2 * Math.PI * profile.metal * time) * Math.exp(-time / (profile.decay * .55))
    );
  };
  for (let i = 0; i < samples.length; i++) {
    seed ^= seed << 13; seed ^= seed >>> 17; seed ^= seed << 5;
    const white = (seed >>> 0) / 0x80000000 - 1;
    filtered += smoothing * (white - filtered);
    // Remove the low rumble from the short contact burst.
    const noise = filtered - previous;
    previous = filtered;
    const time = i / sampleRate;
    const tail = Math.min(1, (samples.length - 1 - i) / (sampleRate * .001));
    // The softer second contact suggests a pawl seating in the next notch.
    samples[i] = profile.level * tail * (impact(time, noise) + .32 * impact(time - profile.seat, noise));
  }
  return samples;
}
