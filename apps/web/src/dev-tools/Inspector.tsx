import type { JSX } from '@solidjs/web';
import { createSignal, untrack } from 'solid-js';
import type { DevToolsController } from './controller';
import { ConnectionState, ConsoleState } from './controller';
import {
  infoSelectors,
  inspectorReadLimit,
  memoryRange,
  type InspectionRequest,
  type InspectorRead,
} from './inspection';

interface Reading {
  camera: string;
  time: string;
  request: InspectionRequest;
  data?: number[];
  text?: string;
  capacity?: number;
  pageBytes?: number;
  error?: string;
}
export function Inspector(props: { tools: DevToolsController }): JSX.Element {
  const [address, setAddress] = createSignal('0x00000000');
  const [length, setLength] = createSignal('256');
  const [busy, setBusy] = createSignal(false);
  const [status, setStatus] = createSignal('');
  const [readings, setReadings] = createSignal<Reading[]>([]);
  const available = () =>
    props.tools.connection() === ConnectionState.Connected &&
    props.tools.consoleState() === ConsoleState.Idle &&
    !props.tools.inspectionBusy() &&
    !busy();
  async function run(action: (read: InspectorRead) => Promise<void>) {
    if (!available()) return;
    setBusy(true);
    setStatus('Reading…');
    try {
      await props.tools.readInspection(async (read) => {
        const camera = untrack(() => props.tools.camera()?.name ?? 'Camera');
        await action(async (request) => {
          const record = { camera, time: new Date().toISOString(), request };
          try {
            const reply = await read(request);
            setReadings((previous) => [
              ...previous,
              { ...record, ...reply, data: Array.from(reply.data) },
            ]);
            return reply;
          } catch (error) {
            setReadings((previous) => [...previous, { ...record, error: String(error) }]);
            throw error;
          }
        });
      });
      setStatus('Read complete');
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }
  function identity() {
    void run(async (read) => {
      const failures: string[] = [];
      // Query every selector even if one is unsupported by the connected body.
      for (const selector of infoSelectors) {
        try {
          await read({ kind: 'identity', selector });
        } catch (error) {
          failures.push(`${selector}: ${String(error)}`);
        }
      }
      if (failures.length) throw Error(failures.join('\n'));
    });
  }
  function configuration() {
    void run(async (read) => {
      const geometry = await read({ kind: 'geometry' });
      const { capacity, pageBytes } = geometry;
      // Host allocation budget, independent of camera identity. Never infer geometry.
      if (
        !Number.isInteger(capacity) ||
        !capacity ||
        capacity < 0 ||
        capacity > 64 * 1024 * 1024 ||
        !Number.isInteger(pageBytes) ||
        !pageBytes ||
        pageBytes < 1
      )
        throw Error('Cannot verify configuration geometry within the 64 MiB host budget.');
      const chunk = Math.min(pageBytes, inspectorReadLimit);
      for (let offset = 0; offset < capacity; offset += chunk)
        await read({ kind: 'configuration', offset, length: Math.min(chunk, capacity - offset) });
    });
  }
  function memory(event: SubmitEvent) {
    event.preventDefault();
    let range;
    try {
      range = memoryRange(address(), length());
    } catch (error) {
      setStatus(String(error));
      return;
    }
    void run(async (read) => {
      await read({ kind: 'memory', ...range });
    });
  }
  function download() {
    const blob = new Blob(
      [
        JSON.stringify(
          {
            schema: 'fujimod-inspector-v1',
            createdAt: new Date().toISOString(),
            readings: readings(),
          },
          null,
          2,
        ),
      ],
      { type: 'application/json' },
    );
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = `fujimod-diagnostics-${new Date().toISOString().replaceAll(':', '-')}.json`;
    anchor.click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  }
  return (
    <section class="dt-inspector" aria-labelledby="inspector-title">
      <h2 id="inspector-title">Inspector</h2>
      <button class="td-action" disabled={!available()} onClick={identity}>
        Read device identity
      </button>
      <button class="td-action" disabled={!available()} onClick={configuration}>
        Dump configuration
      </button>
      <form onSubmit={memory}>
        <label>
          Address
          <input
            value={address()}
            onInput={(event) => setAddress(event.currentTarget.value)}
            spellcheck={false}
          />
        </label>
        <label>
          Length (bytes)
          <input
            value={length()}
            onInput={(event) => setLength(event.currentTarget.value)}
            inputmode="numeric"
          />
        </label>
        <button class="td-action" type="submit" disabled={!available()}>
          Read memory
        </button>
      </form>
      <button class="td-action" disabled={busy() || !readings().length} onClick={download}>
        Download diagnostic bundle
      </button>
      <p role="status">{status()}</p>
    </section>
  );
}
