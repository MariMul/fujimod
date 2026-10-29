import { RemoteEngine } from '@fujimod/emulator-ui/engine/remote';
import type { EngineConfig, Frame } from '@fujimod/emulator-ui/engine/api';

declare global {
  interface Window {
    nativeFrameCount: number;
    nativeTouchCount: number;
    nativeEngine?: RemoteEngine;
  }
}

function required<T>(value: T | null, name: string): T {
  if (value === null) throw Error(`Native LCD preview is missing ${name}.`);
  return value;
}

const canvas = required(document.querySelector<HTMLCanvasElement>('#lcd'), 'the canvas');
const status = required(document.querySelector<HTMLOutputElement>('#status'), 'the status field');
const context = required(canvas.getContext('2d'), 'the canvas context');

window.nativeFrameCount = 0;
window.nativeTouchCount = 0;
const parameters = new URLSearchParams(location.search);
const endpoint =
  parameters.get('ws') ??
  `${location.protocol === 'https:' ? 'wss:' : 'ws:'}//${location.hostname}:${
    location.hostname.endsWith('.ts.net') ? '8803' : '5208'
  }/engine`;
const engine = new RemoteEngine(endpoint, (error) => {
  status.textContent = error.message;
  status.dataset.state = 'ERROR';
});
window.nativeEngine = engine;

function draw(frame: Frame) {
  if (!('bitmap' in frame)) return;
  if (canvas.width !== frame.width || canvas.height !== frame.height) {
    canvas.width = frame.width;
    canvas.height = frame.height;
  }
  context.drawImage(frame.bitmap, 0, 0, frame.width, frame.height);
  window.nativeFrameCount++;
  canvas.dataset.sequence = String(frame.sequence);
}

engine.frames(draw);
const config: EngineConfig = {
  firmwareId: 'x70-110',
  bundle: '/',
  sceneInput: false,
  execution: { mode: 'live', clockSeconds2000: 845000000 },
};
void engine
  .boot(new ArrayBuffer(0), config, (message) => {
    status.textContent = message;
    status.dataset.state = 'BOOTING';
  })
  .then(() => {
    status.textContent = 'LIVE';
    status.dataset.state = 'LIVE';
  })
  .catch((error: unknown) => {
    status.textContent = error instanceof Error ? error.message : String(error);
    status.dataset.state = 'ERROR';
  });

let pointer: number | undefined;
let touchPosition = { x: 0, y: 0 };
function locate(event: PointerEvent) {
  const bounds = canvas.getBoundingClientRect();
  touchPosition = {
    x: Math.max(0, Math.min(720, Math.round(((event.clientX - bounds.left) * 720) / bounds.width))),
    y: Math.max(0, Math.min(480, Math.round(((event.clientY - bounds.top) * 480) / bounds.height))),
  };
}
function sendTouch(phase: 'press' | 'drag' | 'release') {
  void engine
    .touch('touchscreen', phase, touchPosition.x, touchPosition.y)
    .then((result) => {
      window.nativeTouchCount++;
      canvas.dataset.lastTouch = JSON.stringify({ accepted: true, result });
    })
    .catch((error: unknown) => {
      status.textContent = error instanceof Error ? error.message : String(error);
      status.dataset.state = 'ERROR';
    });
}
function release(event?: PointerEvent) {
  if (pointer === undefined) return;
  if (event) locate(event);
  const id = pointer;
  pointer = undefined;
  sendTouch('release');
  if (canvas.hasPointerCapture(id)) canvas.releasePointerCapture(id);
}
canvas.addEventListener('pointerdown', (event) => {
  if (event.button !== 0 || pointer !== undefined) return;
  event.preventDefault();
  locate(event);
  pointer = event.pointerId;
  canvas.setPointerCapture(pointer);
  sendTouch('press');
});
canvas.addEventListener('pointermove', (event) => {
  if (pointer !== event.pointerId) return;
  event.preventDefault();
  locate(event);
  sendTouch('drag');
});
canvas.addEventListener('pointerup', release);
canvas.addEventListener('pointercancel', release);
canvas.addEventListener('lostpointercapture', release);
window.addEventListener('blur', () => release());
document.addEventListener('visibilitychange', () => {
  if (document.hidden) release();
});
window.addEventListener('pagehide', () => engine.dispose(), { once: true });
