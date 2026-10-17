import { Show, For, createSignal, onSettled, untrack } from 'solid-js';
import { DevTools } from '../dev-tools/Page';
import type { CameraBackend, CameraChoice } from '../dev-tools/camera';
import { downloadBlob } from '../ui/card';
import { deliverReport } from '../ui/report-delivery';
import {
  parseReportLink,
  decode64,
  validateReport,
  reportName,
  cameraIssue,
  type ReportLink,
} from './report-link';
import { prepareDownload } from './download';
import { ReportUsb, type ReportConnection } from './usb';
import '../style.css';
import './style.css';
export function BugReport(props: {
  connection?: ReportConnection;
  hash?: string;
  supported?: boolean;
}) {
  const connection = untrack(() => props.connection) ?? new ReportUsb();
  const supported = untrack(() => props.supported) ?? (isSecureContext && 'usb' in navigator);
  const [link, setLink] = createSignal<ReportLink>();
  const [archive, setArchive] = createSignal<Blob>();
  const [download, setDownload] = createSignal<{ blob: Blob; name: string }>();
  const [choice, setChoice] = createSignal<CameraChoice>();
  const [selected, setSelected] = createSignal(0);
  const [status, setStatus] = createSignal(''),
    [error, setError] = createSignal('');
  const [busy, setBusy] = createSignal(false),
    [ready, setReady] = createSignal(false);
  const [connected, setConnected] = createSignal(false),
    [tools, setTools] = createSignal<CameraBackend>();
  const [description, setDescription] = createSignal('');
  let handedOff = false;
  onSettled(() => {
    try {
      const value = parseReportLink(untrack(() => props.hash) ?? location.hash);
      setLink(value);
      if (value.archive) {
        void acceptArchive(validateReport(decode64(value.archive), value), value)
          .then(() => setStatus('Report decoded from QR code.'))
          .catch(problem);
      } else if (supported)
        void connection
          .initialize()
          .then(() => setReady(true))
          .catch(problem);
    } catch (cause) {
      problem(cause);
    }
    const stop = connection.onDisconnect?.(() => {
      setConnected(false);
      setStatus('Camera disconnected. Your report remains available.');
    });
    return () => {
      stop?.();
      if (!handedOff) connection.dispose();
    };
  });
  function problem(cause: unknown) {
    setError(cause instanceof Error ? cause.message : String(cause));
    setBusy(false);
  }
  async function acceptArchive(blob: Blob, current: ReportLink) {
    setArchive(blob);
    setDownload(undefined);
    setStatus('Preparing download…');
    setDownload(await prepareDownload(blob, reportName(current)));
  }
  async function read(id: number) {
    setBusy(true);
    setError('');
    setStatus('Reading report…');
    const current = link()!;
    try {
      await acceptArchive(
        await connection.read(id, current, (bytes) =>
          setStatus(`Reading report… ${Math.floor((bytes / current.bytes) * 100)}%`),
        ),
        current,
      );
      setConnected(true);
      setStatus('Report retrieved from camera.');
    } catch (cause) {
      problem(cause);
    } finally {
      setBusy(false);
    }
  }
  async function connect() {
    if (busy()) return;
    setBusy(true);
    setError('');
    try {
      const value = await connection.choose();
      setChoice(value);
      if (value?.interfaces.length === 1) await read(value.interfaces[0].id);
      else if (value && !value.interfaces.length) throw Error('No debug interface is available.');
    } catch (cause) {
      problem(cause);
    } finally {
      setBusy(false);
    }
  }
  async function chooseFile(event: Event) {
    const file = (event.currentTarget as HTMLInputElement).files?.[0];
    if (!file || !link()) return;
    setError('');
    setBusy(true);
    try {
      if (file.size !== link()!.bytes) throw Error('Choose the TAR file named in this report.');
      await acceptArchive(
        validateReport(new Uint8Array(await file.arrayBuffer()), link()!),
        link()!,
      );
      setStatus('Report opened from file.');
    } catch (cause) {
      problem(cause);
    } finally {
      setBusy(false);
    }
  }
  function submit(event: SubmitEvent) {
    event.preventDefault();
    if (!download() || !link()) return;
    deliverReport({ ...download()!, issueURL: cameraIssue(link()!, description()) }, downloadBlob);
    setStatus('Report downloaded. Attach it to the GitHub issue.');
  }
  return (
    <Show
      when={tools()}
      fallback={
        <main class="shell bug-report">
          <header>
            <a href="../">
              <img src="../brand.png" alt="FujiMod" width="110" height="28" />
            </a>
            <h1>Report a camera bug</h1>
          </header>
          <section class="bug-privacy" aria-label="Privacy">
            <h2>Nothing is uploaded automatically.</h2>
            <p>Your report stays local and private on this device until you choose to share it.</p>
          </section>
          <Show when={link()}>
            {(value) => (
              <section class="bug-card">
                <Show
                  when={archive()}
                  fallback={
                    <>
                      <h2>Connect your camera</h2>
                      <p>
                        Leave the camera’s QR screen open, plug in USB, then connect below.
                        Reconnect USB if it was already plugged in.
                      </p>
                      <p>
                        If you restarted the camera, turn on SET UP → FUJIMOD DEVTOOLS → USB
                        DEBUGGING before connecting.
                      </p>
                      <p class="bug-file">
                        {reportName(value())} · {value().bytes.toLocaleString()} bytes
                      </p>
                      <Show
                        when={supported}
                        fallback={
                          <p>
                            USB is unavailable in this browser. Open this link on a computer with
                            USB support, or choose the TAR from your SD card.
                          </p>
                        }
                      >
                        <button
                          class="pill primary"
                          disabled={!ready() || busy()}
                          onClick={() => void connect()}
                        >
                          Connect camera
                        </button>
                      </Show>
                      <Show when={choice() && choice()!.interfaces.length > 1}>
                        <label>
                          Debug interface
                          <select
                            value={selected()}
                            onChange={(event) => setSelected(Number(event.currentTarget.value))}
                          >
                            <For each={choice()!.interfaces}>
                              {(item) => <option value={item.id}>{item.label}</option>}
                            </For>
                          </select>
                        </label>
                        <button
                          class="pill"
                          disabled={busy()}
                          onClick={() => void read(selected())}
                        >
                          Read report
                        </button>
                      </Show>
                      <label class="bug-file-picker">
                        Choose report from SD card
                        <input
                          type="file"
                          accept=".tar,application/x-tar"
                          disabled={busy()}
                          onChange={(event) => void chooseFile(event)}
                        />
                      </label>
                    </>
                  }
                >
                  <h2>Report ready</h2>
                  <p class="bug-file">
                    {reportName(value())} · {archive()!.size.toLocaleString()} bytes
                  </p>
                  <form onSubmit={submit}>
                    <label>
                      What happened?
                      <textarea
                        rows="5"
                        value={description()}
                        onInput={(event) => setDescription(event.currentTarget.value)}
                      />
                    </label>
                    <button class="pill primary" type="submit" disabled={!download()}>
                      Download and report
                    </button>
                  </form>
                  <Show when={connected()}>
                    <button
                      class="pill"
                      onClick={() => {
                        try {
                          const backend = connection.devTools();
                          handedOff = true;
                          setTools(backend);
                        } catch (cause) {
                          problem(cause);
                        }
                      }}
                    >
                      Open Dev Tools
                    </button>
                  </Show>
                </Show>
              </section>
            )}
          </Show>
          <p role="status" aria-live="polite">
            {status()}
          </p>
          <Show when={error()}>
            <p role="alert">{error()}</p>
          </Show>
        </main>
      }
    >
      {(backend) => <DevTools backend={backend()} supported={true} connectOnMount={true} />}
    </Show>
  );
}
