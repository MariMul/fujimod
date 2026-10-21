import { render } from '@solidjs/web';
import { flush } from 'solid-js';
import { afterEach, expect, test, vi } from 'vite-plus/test';
import type { ToolSession } from '../camera-model';
import { Wizard } from '../CameraTools';
import { Workbench } from '../Workbench';
import { createTestCameraModel, testCamera } from '../test-camera-model';
import type { DebugModeSession } from './operations';

afterEach(() => vi.unstubAllGlobals());

function page(debug?: Partial<DebugModeSession>, initialWizard?: Wizard) {
  const host = document.createElement('div');
  document.body.append(host);
  const next = vi.fn(() => testCamera('X70'));
  const test = createTestCameraModel({
    next,
    session: () => ({ debug: debug as ToolSession['debug'] }),
  });
  const dispose = render(
    () => <Workbench model={test.model} usbSupported initialWizard={initialWizard} />,
    host,
  );
  const button = (name: string) =>
    [...host.querySelectorAll('button')].find(
      (node) => (node.getAttribute('aria-label') ?? node.textContent?.trim()) === name,
    );
  return {
    host,
    next,
    button,
    dispose,
    unplug: () => test.update('X70', (camera) => ({ ...camera, present: false })),
  };
}

test('the wizard explains the procedure, then connects only after the service-mode check', async () => {
  const { host, next, button, dispose } = page(undefined, Wizard.Enable);
  try {
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
    host.querySelector<HTMLInputElement>('.dm-check input')!.click();
    flush();
    expect(allow.disabled).toBe(false);
    expect(next).not.toHaveBeenCalled();
    allow.click();
    await vi.waitFor(() => {
      flush();
      expect(next).toHaveBeenCalledTimes(1);
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

test('enabling arms the reset guard first, then the Write tool disables debug mode', async () => {
  const { session, table } = workflowSession();
  let armed!: (value: Awaited<ReturnType<DebugModeSession['armResetGuard']>>) => void;
  session.armResetGuard.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        armed = resolve;
      }),
  );
  const p = page(session, Wizard.Enable);
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
    p.host.querySelector<HTMLButtonElement>('.dm-huge')!.click();
    await vi.waitFor(() => {
      flush();
      expect(p.button('Done')).toBeTruthy();
    });
    expect(session.armResetGuard).toHaveBeenCalledWith(table, 'x70-110', 'a'.repeat(64));
    expect(session.enterDebugMode).toHaveBeenCalledWith(table, 'x70-110', 'a'.repeat(64));
    p.button('Done')!.click();
    flush();
    expect(p.button('Disable debug mode')!.disabled).toBe(false);
    p.button('Disable debug mode')!.click();
    flush();
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

test('a failed reset guard keeps enabling unavailable and can be retried', async () => {
  const { session } = workflowSession();
  session.armResetGuard.mockRejectedValueOnce(new Error('Service mode did not verify'));
  const p = page(session, Wizard.Enable);
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

test('unplugging during guard preparation does not offer enabling', async () => {
  const { session } = workflowSession();
  let armed!: (value: Awaited<ReturnType<DebugModeSession['armResetGuard']>>) => void;
  session.armResetGuard.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        armed = resolve;
      }),
  );
  const p = page(session, Wizard.Enable);
  try {
    await connectWizard(p);
    await vi.waitFor(() => {
      flush();
      expect(session.armResetGuard).toHaveBeenCalledTimes(1);
    });
    p.unplug();
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

test('a wizard deep link opens directly to its instructions', () => {
  const p = page(undefined, Wizard.Enable);
  try {
    flush();
    expect(p.button('Begin')).toBeTruthy();
    expect(p.host.querySelector('.td-enter')).toBeNull();
  } finally {
    p.dispose();
  }
});

test('the service guide connects without selecting firmware or writing anything', async () => {
  const { session } = workflowSession();
  const p = page(session);
  try {
    flush();
    p.button('Show instructions')!.click();
    flush();
    p.button('Begin')!.click();
    flush();
    expect(p.host.querySelector('.dm-camera')).toBeNull();
    expect(p.host.textContent).toContain('Hold UP');
    expect(p.host.textContent).toContain('do not use the OFF switch');
    p.host.querySelector<HTMLInputElement>('.dm-check input')!.click();
    flush();
    p.button('Allow camera access')!.click();
    await vi.waitFor(() => {
      flush();
      expect(p.button('Open workbench')).toBeTruthy();
    });
    p.button('Open workbench')!.click();
    flush();
    expect(p.host.querySelector('.wb-camera h2')!.textContent).toBe('X70');
    expect(session.armResetGuard).not.toHaveBeenCalled();
    expect(session.enterDebugMode).not.toHaveBeenCalled();
    expect(session.leaveDebugMode).not.toHaveBeenCalled();
  } finally {
    p.dispose();
  }
});
