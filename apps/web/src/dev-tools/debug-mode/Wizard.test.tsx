import { render } from '@solidjs/web';
import { flush } from 'solid-js';
import { afterEach, expect, test, vi } from 'vite-plus/test';
import DevTools from '../Page';
import { Progress, type CameraBackend, type DebugModeSession } from '../camera';

afterEach(() => vi.unstubAllGlobals());

function page(debugSession?: DebugModeSession) {
  const host = document.createElement('div');
  document.body.append(host);
  const pick = vi.fn(async () => ({ name: 'X70', interfaces: [{ id: 0, label: 'Debug' }] }));
  let disconnect = () => {};
  const backend: CameraBackend = {
    initialize: async () => {},
    pick,
    open: async () => ({
      debugSession,
      submit: async () => {},
      poll: async () => ({ progress: Progress.Complete, status: 0, output: new Uint8Array() }),
      usable: () => true,
      close: async () => {},
      abandon: () => {},
    }),
    drain: () => [],
    onDisconnect: (callback) => {
      disconnect = callback;
      return () => {};
    },
    dispose: () => {},
  };
  const dispose = render(() => <DevTools backend={backend} supported={true} />, host);
  const button = (name: string) =>
    [...host.querySelectorAll('button')].find((node) => node.textContent?.trim() === name);
  return { host, pick, button, dispose, disconnect: () => disconnect() };
}

test('Dev Tools opens on the tool deck with debug-only tools locked', async () => {
  const { host, button, dispose } = page();
  try {
    await vi.waitFor(() => {
      flush();
      expect(host.textContent).toContain('Enter debug mode');
    });
    expect(host.querySelector('input[aria-label="Console command"]')).toBeNull();
    const cards = [...host.querySelectorAll('.td-card')];
    const card = (title: string) =>
      cards.find((node) => node.querySelector('h2')?.textContent === title)!;
    expect(card('Leave debug mode').textContent).toContain('Needs a camera in debug mode');
    expect(card('Share camera').textContent).toContain('Needs a camera in debug mode');
    expect(card('Leave debug mode').querySelector('button')).toBeNull();
    expect(button('Show instructions')).toBeTruthy();
    expect(button('Connect camera')).toBeTruthy();
    const simulator = host.querySelector<HTMLAnchorElement>('.td-sim a');
    expect(simulator?.textContent?.trim()).toBe('Open simulator');
    expect(simulator?.getAttribute('href')).toBe('../?scenario=x70-lcd#workspace');
  } finally {
    dispose();
  }
});

test('the hero Connect camera CTA starts a USB connection', async () => {
  const { pick, button, dispose } = page();
  try {
    await vi.waitFor(() => {
      flush();
      expect(button('Connect camera')).toBeTruthy();
    });
    expect(pick).not.toHaveBeenCalled();
    button('Connect camera')!.click();
    await vi.waitFor(() => {
      flush();
      expect(pick).toHaveBeenCalledTimes(1);
    });
  } finally {
    dispose();
  }
});

test('the wizard explains the procedure, then connects only after the service-mode check', async () => {
  const { host, pick, button, dispose } = page();
  try {
    await vi.waitFor(() => {
      flush();
      expect(button('Show instructions')).toBeTruthy();
    });
    button('Show instructions')!.click();
    flush();
    expect(host.textContent).toContain('Restart the camera into service mode.');
    expect(host.textContent).toContain('Remove the battery to restart normally.');
    button('Begin')!.click();
    await vi.waitFor(() => {
      flush();
      expect(host.querySelector('.dm-camera')).toBeTruthy();
    });
    const x70 = [...host.querySelectorAll<HTMLButtonElement>('.dm-camera')].find((node) =>
      node.textContent?.includes('X70'),
    )!;
    expect(x70.disabled).toBe(false);
    x70.click();
    flush();
    expect(host.textContent).toContain('do not use the OFF switch');
    const allow = button('Allow camera access') as HTMLButtonElement;
    expect(allow.disabled).toBe(true);
    const check = host.querySelector<HTMLInputElement>('.dm-check input')!;
    check.click();
    flush();
    expect(allow.disabled).toBe(false);
    expect(pick).not.toHaveBeenCalled();
    allow.click();
    await vi.waitFor(() => {
      flush();
      expect(pick).toHaveBeenCalledTimes(1);
    });
  } finally {
    dispose();
  }
});

function workflowSession() {
  const table = JSON.stringify({ firmware_id: 'x70-110', firmware_sha256: 'a'.repeat(64) });
  vi.stubGlobal(
    'fetch',
    vi.fn(
      async (url: URL) =>
        new Response(
          url.pathname.endsWith('camera.json')
            ? table
            : JSON.stringify({
                schema: 1,
                firmwares: [
                  {
                    id: 'x70-110',
                    body: 'x70',
                    model: 'X70',
                    version: '1.10',
                    sha256: 'a'.repeat(64),
                    bytes: 42,
                    download_url: 'https://example.org/firmware',
                    camera_table: 'runtime/x70-110/camera.json',
                  },
                ],
              }),
        ),
    ),
  );
  const session = {
    armResetGuard: vi.fn(async () => ({
      guardWasSet: false,
      debugFlagBefore: 0,
      progress: ['Reset guard armed'],
    })),
    enterDebugMode: vi.fn(async () => ({
      alreadyPersisted: false,
      progress: ['Debug mode enabled'],
    })),
    leaveDebugMode: vi.fn(async () => ({
      alreadyPersisted: false,
      progress: ['Debug mode disabled'],
    })),
  };
  return { session, table };
}

async function connectWizard(p: ReturnType<typeof page>) {
  await vi.waitFor(() => {
    flush();
    expect(p.button('Show instructions')).toBeTruthy();
  });
  p.button('Show instructions')!.click();
  flush();
  p.button('Begin')!.click();
  await vi.waitFor(() => {
    flush();
    expect(p.host.querySelector('.dm-camera')).toBeTruthy();
  });
  p.host.querySelector<HTMLButtonElement>('.dm-camera')!.click();
  flush();
  p.host.querySelector<HTMLInputElement>('.dm-check input')!.click();
  flush();
  p.button('Allow camera access')!.click();
}

test('USB workflows arm the guard before enabling Enter, then enable Leave and show results', async () => {
  const { session, table } = workflowSession();
  let armed!: (value: Awaited<ReturnType<DebugModeSession['armResetGuard']>>) => void;
  session.armResetGuard.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        armed = resolve;
      }),
  );
  const p = page(session);
  try {
    await connectWizard(p);
    await vi.waitFor(() => {
      flush();
      expect(session.armResetGuard).toHaveBeenCalledTimes(1);
    });
    expect(p.button('Continue')!.disabled).toBe(true);
    expect(session.enterDebugMode).not.toHaveBeenCalled();
    armed({ guardWasSet: false, debugFlagBefore: 0, progress: ['Reset guard armed'] });
    await vi.waitFor(() => {
      flush();
      expect(p.button('Continue')!.disabled).toBe(false);
    });
    expect(p.host.textContent).toContain('Reset guard armed');
    p.button('Continue')!.click();
    flush();
    expect(p.button('Enter debug mode')!.disabled).toBe(false);
    p.button('Enter debug mode')!.click();
    await vi.waitFor(() => {
      flush();
      expect(p.button('Done')).toBeTruthy();
    });
    expect(session.armResetGuard).toHaveBeenCalledWith(table, 'x70-110', 'a'.repeat(64));
    expect(session.enterDebugMode).toHaveBeenCalledWith(table, 'x70-110', 'a'.repeat(64));
    p.button('Done')!.click();
    flush();
    [...p.host.querySelectorAll('.td-card')]
      .find((card) => card.querySelector('h2')?.textContent === 'Leave debug mode')!
      .querySelector<HTMLButtonElement>('button')!
      .click();
    flush();
    expect(p.host.querySelector<HTMLButtonElement>('.dm-huge')!.disabled).toBe(false);
    p.host.querySelector<HTMLButtonElement>('.dm-huge')!.click();
    await vi.waitFor(() => {
      flush();
      expect(p.host.textContent).toContain('Debug mode is off.');
    });
    expect(p.host.textContent).toContain('Debug mode disabled');
    expect(session.leaveDebugMode).toHaveBeenCalledWith(table, 'x70-110', 'a'.repeat(64));
  } finally {
    p.dispose();
  }
});

test('a failed reset guard keeps Enter disabled and can be retried', async () => {
  const { session } = workflowSession();
  session.armResetGuard.mockRejectedValueOnce(new Error('Service mode did not verify'));
  const p = page(session);
  try {
    await connectWizard(p);
    await vi.waitFor(() => {
      flush();
      expect(p.host.textContent).toContain('Service mode did not verify');
    });
    expect(p.button('Continue')!.disabled).toBe(true);
    expect(session.enterDebugMode).not.toHaveBeenCalled();
    p.button('Try again')!.click();
    await vi.waitFor(() => {
      flush();
      expect(p.button('Continue')!.disabled).toBe(false);
    });
    expect(session.armResetGuard).toHaveBeenCalledTimes(2);
  } finally {
    p.dispose();
  }
});

test('disconnecting during guard preparation does not enable Enter', async () => {
  const { session } = workflowSession();
  let armed!: (value: Awaited<ReturnType<DebugModeSession['armResetGuard']>>) => void;
  session.armResetGuard.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        armed = resolve;
      }),
  );
  const p = page(session);
  try {
    await connectWizard(p);
    await vi.waitFor(() => {
      flush();
      expect(session.armResetGuard).toHaveBeenCalledTimes(1);
    });
    p.disconnect();
    flush();
    armed({ guardWasSet: false, debugFlagBefore: 0, progress: ['Reset guard armed'] });
    await Promise.resolve();
    flush();
    expect(p.button('Continue')).toBeUndefined();
    expect(session.enterDebugMode).not.toHaveBeenCalled();
  } finally {
    p.dispose();
  }
});

test('the wizard preview URL opens directly to its instructions', async () => {
  const previous = location.href;
  history.replaceState({}, '', '/dev-tools/?tool=enter-debug-mode');
  const p = page();
  try {
    await vi.waitFor(() => {
      flush();
      expect(p.button('Begin')).toBeTruthy();
    });
    expect(p.button('Show instructions')).toBeUndefined();
  } finally {
    p.dispose();
    history.replaceState({}, '', previous);
  }
});
