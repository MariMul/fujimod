import { render } from '@solidjs/web';
import { afterEach, expect, test } from 'vite-plus/test';
import { ReleaseNotes } from './ReleaseNotes';
let dispose: (() => void) | undefined;
afterEach(() => {
  dispose?.();
  document.body.replaceChildren();
});
test('limited Markdown renders headings, lists, emphasis, code and safe links', () => {
  const host = document.createElement('div');
  document.body.append(host);
  dispose = render(
    () => (
      <ReleaseNotes
        text={
          '## Changes\n- **Fast** and *clear* `menus`\n- [Read more](https://example.com)\n\n1. Test\n2. Report\n\n```\n<code>\n```'
        }
      />
    ),
    host,
  );
  expect(host.querySelector('h4')?.textContent).toBe('Changes');
  expect(host.querySelectorAll('li')).toHaveLength(4);
  expect(host.querySelector('strong')?.textContent).toBe('Fast');
  expect(host.querySelector('em')?.textContent).toBe('clear');
  expect(host.querySelector('a')?.target).toBe('_blank');
  expect(host.querySelector('pre code')?.textContent).toBe('<code>');
});
test('release content cannot inject HTML, images or script links', () => {
  const host = document.createElement('div');
  document.body.append(host);
  dispose = render(
    () => (
      <ReleaseNotes
        text={
          '<img src=x onerror=alert(1)>\n![image](https://example.com/image)\n[unsafe](javascript:alert)\n[safe](/recovery/)'
        }
      />
    ),
    host,
  );
  expect(host.querySelector('img, script')).toBeNull();
  expect(host.querySelectorAll('a')).toHaveLength(1);
  expect(host.querySelector('a')?.getAttribute('href')).toBe('/recovery/');
});
