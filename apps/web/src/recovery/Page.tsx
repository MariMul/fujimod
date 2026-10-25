import { For, Show, createSignal, onSettled } from 'solid-js';
import { downloadBlob } from '@fujimod/emulator-ui/ui/download';
import { zipBlob } from '@fujimod/emulator-ui/archive';
import { initialize } from 'fujimod';
import { Icon } from '@fujimod/emulator-ui/ui/icons';

import { identify, recoveryCameras, type IdentifiedFirmware } from './identify';
import { CameraStore, type CameraRecord } from '../cameras/store';
import { FirmwareDownloads, type DownloadRow } from '../cameras/FirmwareDownloads';
import '../cameras/launcher.css';
import { recoveryCard } from './card';
import { SYSTEMS, FORMAT_STEPS, detectSystem, type System } from './format-guide';
import '../home/site.css';
import '../navigation.css';
import './recovery.css';

function message(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}

export default function Recovery() {
  const [system, setSystem] = createSignal<System>('macOS');
  const [identified, setIdentified] = createSignal<IdentifiedFirmware>();
  const [card, setCard] = createSignal<{ names: string[]; zip: Blob }>();
  const [error, setError] = createSignal('');
  const [busy, setBusy] = createSignal(false);

  const [dragging, setDragging] = createSignal(false);
  const [saved, setSaved] = createSignal<CameraRecord[]>([]);
  const [cameras, setCameras] = createSignal<DownloadRow[]>([]);
  // The catalog's boot combinations come from the Wasm catalog.
  let ready: Promise<unknown> = Promise.resolve();
  onSettled(() => {
    ready = initialize().then(() => setCameras(recoveryCameras()));
    ready.catch((caught: unknown) => setError(message(caught)));
    setSystem(detectSystem(navigator.userAgent));
    // One row per stored firmware, from cameras saved in the emulator.
    new CameraStore().list().then(
      (records) =>
        setSaved(
          records.filter(
            (record, index) =>
              records.findIndex((other) => other.firmwareHash === record.firmwareHash) === index,
          ),
        ),
      () => setSaved([]),
    );
  });

  async function chooseFile(file: File | undefined) {
    if (file) await chooseDat(new Uint8Array(await file.arrayBuffer()));
  }
  async function chooseSaved(camera: CameraRecord) {
    const blob: Blob = await new CameraStore().firmware(camera.id);
    await chooseDat(new Uint8Array(await blob.arrayBuffer()));
  }
  async function chooseDat(bytes: Uint8Array<ArrayBuffer>) {
    setError('');
    setIdentified(undefined);
    setCard(undefined);
    setBusy(true);
    try {
      await ready;
      const result = await identify(bytes);
      setIdentified(result);
      const files = recoveryCard(result.firmware.id, bytes);
      const zip = await zipBlob(
        files.map((entry) => ({ path: entry.name, data: new Blob([entry.data]) })),
      );
      setCard({ names: files.map((entry) => entry.name), zip });
    } catch (caught) {
      setError(message(caught));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div class="shell recovery">
      <header>
        <a href="../" aria-label="Fujimod">
          <img src="../brand.png" alt="Fujimod" width="110" height="28" />
        </a>
        <nav class="site-nav" aria-label="Pages">
          <a href="../">Emulator</a>
          <a href="../dev-tools/">Dev Tools</a>
          <a href="./" aria-current="page">
            Recovery
          </a>
        </nav>
      </header>
      <main>
        <h1>Recovery</h1>
        <p class="recovery-lead">
          If your camera is unresponsive after flashing custom firmware, boot it from an SD card and
          reflash the official firmware.
        </p>

        <ol class="recovery-steps">
          <li>
            <h2>Format an SD card as FAT32</h2>
            <label class="recovery-system">
              System
              <select
                value={system()}
                onChange={(event) => setSystem(event.currentTarget.value as System)}
              >
                <For each={SYSTEMS}>{(name) => <option value={name}>{name}</option>}</For>
              </select>
            </label>
            <ol class="recovery-sub">
              <For each={FORMAT_STEPS[system()]}>{(step) => <li>{step}</li>}</For>
            </ol>
          </li>

          <li>
            <h2>Choose the official Fujifilm firmware</h2>
            <div
              class={{ launcher: true, dragging: dragging() }}
              onDragOver={(event) => {
                event.preventDefault();
                setDragging(true);
              }}
              onDragLeave={() => setDragging(false)}
              onDrop={(event) => {
                event.preventDefault();
                setDragging(false);
                void chooseFile(event.dataTransfer?.files[0]);
              }}
            >
              <div class="launcher-main">
                <Show when={saved().length > 0}>
                  <div class="launcher-list" role="list" aria-label="Firmware in this browser">
                    <For each={saved()}>
                      {(camera) => (
                        <button
                          class="launcher-row"
                          role="listitem"
                          disabled={busy()}
                          onClick={() => void chooseSaved(camera)}
                        >
                          <span class="launcher-row-text">
                            <strong>{camera.name}</strong>
                            <span>In this browser</span>
                          </span>
                          <Icon name="forward" size={16} />
                        </button>
                      )}
                    </For>
                  </div>
                </Show>
                <ol class="launcher-steps">
                  <li>Download your camera’s firmware update from Fujifilm.</li>
                  <li>
                    <span class="launcher-drop">Drop the downloaded .DAT file here.</span>
                    <span class="launcher-touch">Choose the downloaded .DAT file.</span>
                  </li>
                </ol>
                <div class="launcher-actions">
                  <label class="launcher-button" for="recovery-file">
                    Choose File…
                  </label>
                </div>
                <input
                  class="visually-hidden"
                  id="recovery-file"
                  type="file"
                  accept=".dat,.DAT,application/octet-stream"
                  disabled={busy()}
                  onChange={(event) => {
                    void chooseFile(event.currentTarget.files?.[0]);
                    event.currentTarget.value = '';
                  }}
                />
                <Show when={identified()}>
                  {(result) => (
                    <p class="recovery-ok">
                      {result().firmware.model} · {result().firmware.version}
                    </p>
                  )}
                </Show>
                <Show when={error()}>
                  <p class="recovery-error" role="alert">
                    {error()}
                  </p>
                </Show>
              </div>
              <FirmwareDownloads heading="Official firmware" rows={cameras()} />
            </div>
          </li>

          <li class={{ 'recovery-pending': !card() }}>
            <h2>Copy the recovery files onto the card</h2>
            <Show
              when={card()}
              fallback={
                <p class="recovery-dim">Choose a firmware above to build the recovery files.</p>
              }
            >
              {(built) => (
                <>
                  <button
                    class="launcher-button recovery-download"
                    onClick={() => downloadBlob(built().zip, 'fujimod-recovery.zip')}
                  >
                    Download recovery files
                  </button>
                  <p>
                    Unzip onto the card’s top level:{' '}
                    <For each={built().names}>
                      {(name, i) => (
                        <span>
                          {i() ? ', ' : ''}
                          <code>{name}</code>
                        </span>
                      )}
                    </For>
                    .
                  </p>
                  <p>Insert the card into the camera.</p>
                </>
              )}
            </Show>
          </li>

          <li class={{ 'recovery-pending': !identified() }}>
            <h2>Unplug the camera from USB</h2>
          </li>

          <li class={{ 'recovery-pending': !identified() }}>
            <h2>Boot from the SD card</h2>
            <Show
              when={identified()}
              fallback={<p class="recovery-dim">Choose a firmware above.</p>}
            >
              {(result) => (
                <>
                  <p>
                    With the camera off, hold these while switching it on, and keep holding until
                    the screen lights:
                  </p>
                  <Show
                    when={result().sdBoot}
                    fallback={
                      <p class="recovery-error">
                        The {result().firmware.model}’s SD boot buttons are not in the catalog yet.
                      </p>
                    }
                  >
                    {(holds) => (
                      <ul class="recovery-holds">
                        <For each={holds()}>{(hold) => <li>{hold}</li>}</For>
                        <li>also hold DISP/BACK, to enter firmware update</li>
                      </ul>
                    )}
                  </Show>
                </>
              )}
            </Show>
          </li>

          <li class={{ 'recovery-pending': !identified() }}>
            <h2>Flash the firmware as usual</h2>
            <p>Follow the on-screen firmware update to flash the official .DAT from the card.</p>
          </li>
        </ol>
      </main>
    </div>
  );
}
