import type { ModelViewer } from '../viewer/model';
import type { DetentSound } from '../feedback/detent-sound';
import type { Engine } from '../engine/api';
declare global {
  interface Window {
    requests: { id: string; value: string }[];
    clicks: number;
    clickTimbres: number[];
    vibrations: (number | number[])[];
    haptics: boolean;
    modelSounds: Record<string, DetentSound>;
    panel: ControlSession;
    fixtureReady: boolean;
  }
}
import { ControlSession, Controls } from './controls';
import { render, createComponent } from '@solidjs/web';
import { flush } from 'solid-js';
import panelData from '../../devices/x70/panel.json';
window.requests = [];
window.clicks = 0;
window.clickTimbres = [];
window.vibrations = [];
window.haptics = false;
Object.defineProperty(navigator, 'vibrate', {
  configurable: true,
  value: (duration: number | number[]) => {
    window.vibrations.push(duration);
    return window.haptics;
  },
});
const createBufferSource = AudioContext.prototype.createBufferSource;
const buffers = new Map<AudioBuffer | null, number>();
AudioContext.prototype.createBufferSource = function (...args) {
  const source = createBufferSource.apply(this, args),
    start = source.start.bind(source);
  source.start = (...args) => {
    if (!buffers.has(source.buffer)) buffers.set(source.buffer, buffers.size);
    window.clicks++;
    window.clickTimbres.push(buffers.get(source.buffer)!);
    start(...args);
  };
  return source;
};
const viewer = new Proxy(
  {
    configureRotary(_inputs: unknown, _step: unknown, sounds: Record<string, DetentSound>) {
      window.modelSounds = sounds;
    },
  },
  { get: (target, key) => Reflect.get(target, key) ?? (() => {}) },
);
window.panel = new ControlSession(
  viewer as unknown as ModelViewer,
  (error) => {
    throw error;
  },
  async () => {},
);
window.panel.render(
  [
    {
      id: 'aperture',
      label: 'Aperture',
      kind: 'ring',
      supported: true,
      positions: ['2.8', '4', '5.6', '8', '11', '16'].map((id) => ({ id, label: id })),
      initial: '5.6',
    },
    {
      id: 'shutter_speed',
      label: 'Speed',
      kind: 'dial',
      supported: true,
      positions: ['30', '60', '125', '250', '500', '1000'].map((id) => ({ id, label: id })),
      initial: '125',
    },
    { id: 'control_ring', label: 'Command', kind: 'dial', supported: true, relative: true },
  ],
  { setPosition: async (id: string, value: string) => { window.requests.push({ id, value }); } } as unknown as Engine,
  {
    detentSounds: panelData.detentSounds as Record<string, DetentSound>,
    layout: [
      { kind: 'control', id: 'aperture', variant: 'dial' },
      { kind: 'control', id: 'shutter_speed', variant: 'dial' },
      { kind: 'control', id: 'control_ring', variant: 'ticks' },
    ],
    combos: [],
  },
);
render(() => createComponent(Controls, { session: window.panel }), document.querySelector('#controls')!);
flush();
window.fixtureReady = true;
