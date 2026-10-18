import type { Firmware } from '../catalog';
import { icon } from './icons';
export interface Generation {
  name: string;
  years: string;
  models: string[];
}
export interface CameraSupport {
  firmware: Firmware;
  mod: boolean;
  emu: boolean;
}
function chips(camera: CameraSupport) {
  const nodes: HTMLElement[] = [];
  if (camera.mod) {
    const chip = document.createElement('span');
    chip.className = 'chip mod';
    const logo = document.createElement('img');
    logo.src = 'brand.png';
    logo.alt = 'Fujimod';
    chip.append(logo);
    nodes.push(chip);
  }
  if (camera.emu) {
    const chip = document.createElement('span');
    chip.className = 'chip emu';
    chip.textContent = 'EMU';
    nodes.push(chip);
  }
  return nodes;
}
function firmwareLink(camera: CameraSupport) {
  const link = document.createElement('a');
  link.className = 'getfw';
  link.href = camera.firmware.download_url;
  link.target = '_blank';
  link.rel = 'noopener noreferrer';
  link.append('Firmware', icon('external', 11));
  return link;
}
function emulateButton(camera: CameraSupport, emulate: (firmware: Firmware) => void) {
  const button = document.createElement('button');
  button.className = 'getfw';
  button.type = 'button';
  button.textContent = 'Try emulator';
  button.setAttribute('aria-label', `Try ${camera.firmware.model} emulator`);
  button.onclick = () => emulate(camera.firmware);
  return button;
}
export function cameraCard(camera: CameraSupport, emulate: (firmware: Firmware) => void) {
  const card = document.createElement('div');
  card.className = 'body';
  const name = document.createElement('span');
  name.className = 'nm';
  name.textContent = camera.firmware.model;
  card.append(name, ...chips(camera), firmwareLink(camera));
  if (camera.emu) card.append(emulateButton(camera, emulate));
  return card;
}
export function cameraTable(
  host: HTMLElement,
  columns: Generation[][],
  cameras: CameraSupport[],
  emulate: (firmware: Firmware) => void,
) {
  host.replaceChildren();
  for (const groups of columns) {
    const column = document.createElement('div');
    column.className = 'col';
    for (const group of groups) {
      const section = document.createElement('div');
      section.className = 'gen';
      const heading = document.createElement('h4'),
        years = document.createElement('span');
      years.textContent = group.years;
      heading.append(group.name, years);
      section.append(heading);
      const supported = cameras.filter((camera) => group.models.includes(camera.firmware.model));
      for (const camera of supported) {
        const row = document.createElement('div');
        row.className = 'cam';
        const name = document.createElement('div');
        name.className = 'n';
        name.append(camera.firmware.model, ...chips(camera));
        const link = firmwareLink(camera);
        link.style.marginTop = '8px';
        row.append(name, link);
        if (camera.emu) row.append(' ', emulateButton(camera, emulate));
        section.append(row);
      }
      const remaining = group.models.filter(
        (name) => !supported.some((camera) => camera.firmware.model === name),
      );
      if (remaining.length) {
        const rest = document.createElement('div');
        rest.className = 'rest';
        const caption = document.createElement('b');
        caption.textContent = 'NOT SUPPORTED YET';
        rest.append(caption, remaining.join(' · '));
        section.append(rest);
      }
      column.append(section);
    }
    host.append(column);
  }
}
