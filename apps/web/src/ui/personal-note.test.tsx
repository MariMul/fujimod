import { render } from '@solidjs/web';
import { flush } from 'solid-js';
import { afterEach, expect, test, vi } from 'vite-plus/test';
import { PersonalNote, projectAge } from './personal-note';

let dispose: (() => void) | undefined;
afterEach(() => {
  dispose?.();
  dispose = undefined;
  document.body.replaceChildren();
  vi.useRealTimers();
});

test('project age counts calendar days from October 3, including across daylight saving time', () => {
  expect(projectAge(new Date(2026, 9, 2, 23, 59))).toBe(0);
  expect(projectAge(new Date(2026, 9, 3, 23, 59))).toBe(0);
  expect(projectAge(new Date(2026, 9, 4))).toBe(1);
  expect(projectAge(new Date(2026, 9, 27))).toBe(24);
  expect(projectAge(new Date(2026, 10, 2))).toBe(30);
});

test.each([26, 27])('the note calculates its age when opened on October %i', (day) => {
  vi.useFakeTimers({ toFake: ['Date'] });
  vi.setSystemTime(new Date(2026, 9, day, 12));
  const host = document.createElement('div');
  document.body.append(host);
  dispose = render(() => <PersonalNote />, host);
  flush();
  expect(host.querySelector('#project-age')?.textContent).toBe(String(day - 3));
  expect(host.textContent).toContain(`${day - 3}-day-old vibe-coded firmware modification`);
});
