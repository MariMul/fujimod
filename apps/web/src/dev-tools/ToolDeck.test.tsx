import { render } from '@solidjs/web';
import { createSignal, flush } from 'solid-js';
import { expect, test, vi } from 'vite-plus/test';
import { DeckTool, ToolDeck } from './ToolDeck';

test('the whole tile invokes its named action and debug instructions stay separate', () => {
  const host = document.createElement('div');
  const onOpen = vi.fn();
  const dispose = render(() => <ToolDeck debugCamera usbSupported onOpen={onOpen} />, host);
  try {
    flush();
    expect(host.querySelector('.td-enter p')!.textContent?.replace(/\s+/g, ' ').trim()).toBe(
      'In debug mode, the camera operates normally, except its USB connection can be used to inspect and control the camera’s internal software. USB must be plugged in twice for regular file transfer.',
    );
    expect(host.querySelector('.td-note')!.textContent).toBe('Required for other Dev Tools.');
    host.querySelector<HTMLButtonElement>('.td-enter button')!.click();
    expect(onOpen).toHaveBeenLastCalledWith(DeckTool.EnterDebugMode);
    for (const [title, tool] of [
      ['Connect camera', DeckTool.ConnectDebugCamera],
      ['Connect simulator', DeckTool.ConnectSimulator],
      ['Console', DeckTool.Console],
      ['Share camera', DeckTool.Share],
      ['Leave debug mode', DeckTool.LeaveDebugMode],
    ] as const) {
      const tile = host.querySelector<HTMLButtonElement>(`button[aria-label="${title}"]`)!;
      expect(tile.classList.contains('td-action')).toBe(true);
      expect(tile.querySelector('.td-title')!.tagName).toBe('SPAN');
      expect(tile.querySelector('.td-title')!.textContent).toBe(title);
      expect(tile.querySelector('h2, button, a')).toBeNull();
      tile.click();
      expect(onOpen).toHaveBeenLastCalledWith(tool);
    }
    const bug = host.querySelector<HTMLAnchorElement>('a[aria-label="Bug reports"]')!;
    expect(bug.getAttribute('href')).toBe('../bug/');
    expect(bug.classList.contains('td-action')).toBe(true);
    expect(bug.querySelector('button, a')).toBeNull();
    expect(host.querySelectorAll('.td-connects > button')).toHaveLength(2);
    expect(host.querySelectorAll('.td-grid > .td-action')).toHaveLength(4);
  } finally {
    dispose();
  }
});

test('locked and busy tiles are disabled, display the reason, and cannot invoke actions', () => {
  const host = document.createElement('div');
  const onOpen = vi.fn();
  const [debugCamera, setDebugCamera] = createSignal(false);
  const [busy, setBusy] = createSignal(false);
  const dispose = render(
    () => (
      <ToolDeck
        debugCamera={debugCamera()}
        busy={busy()}
        usbSupported={false}
        shareLocked="Sharing isn’t available here"
        onOpen={onOpen}
      />
    ),
    host,
  );
  const tile = (title: string) =>
    host.querySelector<HTMLButtonElement>(`button[aria-label="${title}"]`)!;
  try {
    flush();
    expect(tile('Connect camera').disabled).toBe(true);
    expect(tile('Connect camera').textContent).toContain(
      'This browser can’t connect to USB. Use Chrome or Edge.',
    );
    for (const title of ['Share camera', 'Leave debug mode']) {
      expect(tile(title).disabled).toBe(true);
      expect(tile(title).textContent).toContain('Needs a camera in debug mode');
      tile(title).click();
    }
    tile('Connect camera').click();
    setDebugCamera(true);
    setBusy(true);
    flush();
    expect(tile('Share camera').disabled).toBe(true);
    expect(tile('Share camera').textContent).toContain('Sharing isn’t available here');
    expect(tile('Leave debug mode').disabled).toBe(false);
    for (const title of ['Connect camera', 'Connect simulator']) {
      expect(tile(title).disabled).toBe(true);
      expect(tile(title).getAttribute('aria-busy')).toBe('true');
      expect(tile(title).textContent).toContain('Connecting…');
      tile(title).click();
    }
    expect(onOpen).not.toHaveBeenCalled();
    setBusy(false);
    flush();
    expect(tile('Connect simulator').disabled).toBe(false);
    expect(tile('Connect simulator').textContent).not.toContain('Connecting…');
    tile('Connect simulator').click();
    expect(onOpen).toHaveBeenCalledExactlyOnceWith(DeckTool.ConnectSimulator);
  } finally {
    dispose();
  }
});
