import { render } from '@solidjs/web';
import { flush } from 'solid-js';
import { expect, test } from 'vite-plus/test';
import type { PhysicalControls } from 'fujimod';
import x70 from '../../../../catalog/bodies/x70/controls.json';
import { CommandFace, PadFace } from './control-faces';
import { PowerComboKeys } from './power-combo';

test('X70 recovery draws the panel faces with command center, record, DISP/BACK and ON', () => {
  const host = document.createElement('div');
  const dispose = render(
    () => <PowerComboKeys controls={x70 as PhysicalControls} modes={['sd', 'firmware_update']} />,
    host,
  );
  flush();
  expect([...host.querySelectorAll('[data-part]')].map((e) => e.getAttribute('data-part'))).toEqual(
    ['command', 'key', 'key', 'positions'],
  );
  expect(host.querySelector('.dial-press.glow')).toBeTruthy();
  expect(host.querySelector('.knurl')).toBeNull();
  expect(host.querySelector('.control-short-name')?.textContent).toBe('COMMAND');
  expect(host.querySelector('.key.rec i')).toBeTruthy();
  expect(host.querySelectorAll('.key.glow')).toHaveLength(2);
  expect(host.textContent).toContain('DISP\nBACK');
  expect(host.querySelector('.seg .on')?.textContent).toBe('ON');
  expect(host.querySelectorAll('button')).toHaveLength(0);
  dispose();
});

test('service sequence draws USB, the held selector direction, and full shutter stage', () => {
  const host = document.createElement('div');
  const dispose = render(
    () => <PowerComboKeys controls={x70 as PhysicalControls} modes={['service']} />,
    host,
  );
  flush();
  expect(host.querySelector('[aria-label="USB connected"]')).toBeTruthy();
  expect(host.querySelector('.dpad .u.glow')).toBeTruthy();
  expect(host.querySelector('.slide.full')).toBeTruthy();
  dispose();
});

for (const held of ['decrease', 'press', 'increase'] as const) {
  test(`command mini highlights only its held ${held} part`, () => {
    const host = document.createElement('div');
    const dispose = render(
      () => (
        <CommandFace part={{ kind: 'command', marking: { shortName: 'COMMAND' }, held }} size="mini" />
      ),
      host,
    );
    flush();
    expect(host.querySelectorAll('.glow')).toHaveLength(1);
    expect(host.querySelector('.glow')?.getAttribute('data-action')).toBe(held);
    expect(host.querySelector('.active')).toBeNull();
    dispose();
  });
}
for (const held of ['up', 'down', 'left', 'right', 'press'] as const) {
  test(`pad mini highlights only its held ${held} part`, () => {
    const host = document.createElement('div');
    const dispose = render(
      () => <PadFace part={{ kind: 'pad', marking: {}, held }} size="mini" />,
      host,
    );
    flush();
    expect(host.querySelectorAll('.glow')).toHaveLength(1);
    expect(host.querySelector('.glow')?.getAttribute('data-action')).toBe(held);
    expect(host.querySelector('.active')).toBeNull();
    dispose();
  });
}
