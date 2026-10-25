import { render } from '@solidjs/web';
import { flush } from 'solid-js';
import { expect, test } from 'vite-plus/test';
import type { PhysicalControls } from 'fujimod';
import x70 from '../../../../catalog/bodies/x70/controls.json';
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
  expect(host.querySelector('.knurl .tab.active')).toBeTruthy();
  expect(host.querySelector('.control-short-name')?.textContent).toBe('CMD');
  expect(host.querySelector('.key.rec i')).toBeTruthy();
  expect(host.querySelectorAll('.key.active')).toHaveLength(3);
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
  expect(host.querySelector('.dpad .u.active')).toBeTruthy();
  expect(host.querySelector('.slide.full')).toBeTruthy();
  dispose();
});
