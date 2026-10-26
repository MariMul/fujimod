import { PowerComboKeys } from '@fujimod/emulator-ui/ui/power-combo';
import { For, Show, createMemo, createSignal, onSettled } from 'solid-js';
import { downloadBlob } from '@fujimod/emulator-ui/ui/download';
import { zipBlob } from '@fujimod/emulator-ui/archive';
import { catalog as reference, initialize } from 'fujimod';
import { Icon } from '@fujimod/emulator-ui/ui/icons';
import { SegmentedControl } from '@fujimod/emulator-ui/ui/SegmentedControl';

import { identify, recoveryCameras, recoveryPlan } from './identify';
import { CameraStore, type LibraryFirmware } from '../cameras/store';
import { FirmwareDownloads, type DownloadRow } from '../cameras/FirmwareDownloads';
import '../cameras/launcher.css';
import { recoveryCard, type RecoveryDownload } from './card';
import { SiteHeader } from '../ui/site-header';
import { SYSTEMS, FORMAT_STEPS, detectSystem, type System } from './format-guide';
import '../home/site.css';
import '../navigation.css';
import './recovery.css';
import { firmwareDownloadName } from '../firmware-download';

declare const __FUJIMOD_BUILD_SHA__: string;

function message(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}

export default function Recovery() {
  const store = new CameraStore();
  const [system, setSystem] = createSignal<System>('macOS');
  const [library, setLibrary] = createSignal<LibraryFirmware[]>([]);
  const [selected, setSelected] = createSignal<string>();
  const [catalogReady, setCatalogReady] = createSignal(false);
  const [error, setError] = createSignal('');
  const [adding, setAdding] = createSignal(false);
  const [building, setBuilding] = createSignal(false);
  const [buildError, setBuildError] = createSignal('');
  const [downloadType, setDownloadType] = createSignal<RecoveryDownload>('system');
  const [dragging, setDragging] = createSignal(false);
  const [cameras, setCameras] = createSignal<DownloadRow[]>([]);

  const chosen = () => library().find((firmware) => firmware.sha256 === selected());
  // The card's files and boot buttons come from the Wasm catalog, without building the card.
  const plan = createMemo(() => {
    const firmware = chosen();
    return catalogReady() && firmware ? recoveryPlan(firmware.firmwareId) : undefined;
  });

  let ready: Promise<unknown> = Promise.resolve();
  onSettled(() => {
    ready = initialize().then(() => {
      setCameras(recoveryCameras());
      setCatalogReady(true);
    });
    ready.catch((caught: unknown) => setError(message(caught)));
    setSystem(detectSystem(navigator.userAgent));
    store.library().then(setLibrary, (caught: unknown) => setError(message(caught)));
  });

  function select(sha256: string) {
    setBuildError('');
    setSelected(sha256);
    setDownloadType('system');
  }
  async function chooseFile(file: File | undefined) {
    if (!file) return;
    setError('');
    setAdding(true);
    try {
      await ready;
      const firmware = await identify(new Uint8Array(await file.arrayBuffer()));
      const entry = await store.addFirmware(file, firmware);
      setLibrary(await store.library());
      select(entry.sha256);
    } catch (caught) {
      setError(message(caught));
    } finally {
      setAdding(false);
    }
  }
  async function download(firmware: LibraryFirmware, kind: RecoveryDownload) {
    setBuildError('');
    setDownloadType(kind);
    setBuilding(true);
    try {
      const bytes = new Uint8Array(await store.firmwareBytes(firmware.sha256));
      const files = recoveryCard(firmware.firmwareId, bytes, kind);
      const zip = await zipBlob(
        files.map((entry) => ({ path: entry.name, data: new Blob([entry.data]) })),
      );
      const release = reference
        .firmwareVersions()
        .find((entry) => entry.firmware_id === firmware.firmwareId);
      if (!release) throw Error('Firmware is not in the catalog.');
      downloadBlob(
        zip,
        firmwareDownloadName(
          kind === 'green-led' ? 'green-led' : 'recovery',
          'sd-boot',
          { model: release.product, version: release.version },
          __FUJIMOD_BUILD_SHA__,
        ),
      );
    } catch (caught) {
      setBuildError(message(caught));
    } finally {
      setBuilding(false);
    }
  }

  return (
    <div class="shell recovery">
      <SiteHeader root="../" current="recovery" />
      <main>
        <h1>Recovery</h1>
        <p class="recovery-lead">
          If your camera is unresponsive after flashing custom firmware, boot it from an SD card and
          reflash the official firmware.
        </p>

        <ol class="recovery-steps">
          <li>
            <h2>Format an SD card as FAT32</h2>
            <SegmentedControl
              label="System"
              options={SYSTEMS.map((name) => ({ value: name, label: name }))}
              value={system()}
              onSelect={setSystem}
            />
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
                <Show when={library().length > 0}>
                  <div
                    class="recovery-saved"
                    role="radiogroup"
                    aria-label="Firmware in this browser"
                  >
                    <For each={library()}>
                      {(firmware) => (
                        <label class="recovery-saved-row">
                          <input
                            class="visually-hidden"
                            type="radio"
                            name="recovery-saved"
                            checked={selected() === firmware.sha256}
                            onChange={() => select(firmware.sha256)}
                          />
                          <span class="recovery-radio" aria-hidden="true">
                            <Icon name="check" size={12} />
                          </span>
                          <span class="launcher-row-text">
                            <strong>{firmware.name}</strong>
                            <span>In this browser</span>
                          </span>
                        </label>
                      )}
                    </For>
                  </div>
                </Show>
                <p class="recovery-dat">
                  Download your camera’s firmware update from Fujifilm.{' '}
                  <span class="launcher-drop">Drop the downloaded .DAT file here.</span>
                  <span class="launcher-touch">Choose the downloaded .DAT file.</span>
                </p>
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
                  disabled={adding()}
                  onChange={(event) => {
                    void chooseFile(event.currentTarget.files?.[0]);
                    event.currentTarget.value = '';
                  }}
                />
                <Show when={error()}>
                  <p class="recovery-error" role="alert">
                    {error()}
                  </p>
                </Show>
              </div>
              <FirmwareDownloads heading="Official firmware" rows={cameras()} />
            </div>
          </li>

          <li class={{ 'recovery-pending': !plan() }}>
            <h2>Copy the recovery files onto the card</h2>
            <Show
              when={plan()}
              fallback={
                <p class="recovery-dim">Choose a firmware above to build the recovery files.</p>
              }
            >
              {(card) => (
                <>
                  <button
                    class="launcher-button recovery-download"
                    disabled={building() || !card().canBuild}
                    onClick={() => {
                      const firmware = chosen();
                      if (firmware) void download(firmware, 'system');
                    }}
                  >
                    {building() && downloadType() === 'system'
                      ? 'Building…'
                      : 'Download full system image'}
                  </button>
                  <Show when={!card().canBuild}>
                    <p class="recovery-error">
                      Recovery files can't be built for the {card().model} yet. Missing:{' '}
                      {card().missing.join(', ')}.
                    </p>
                  </Show>
                  <Show when={buildError()}>
                    <p class="recovery-error" role="alert">
                      {buildError()}
                    </p>
                  </Show>
                  <p>
                    Unzip onto the card’s top level:{' '}
                    <For
                      each={downloadType() === 'green-led' ? card().greenLedFiles : card().files}
                    >
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

          <li class={{ 'recovery-pending': !plan() }}>
            <h2>Unplug the camera from USB</h2>
          </li>

          <li class={{ 'recovery-pending': !plan() }}>
            <h2>Boot from the SD card</h2>
            <Show
              when={plan()}
              fallback={
                <p class="recovery-dim">
                  Choose a firmware above to see instructions for your camera.
                </p>
              }
            >
              {(result) => (
                <>
                  <p>
                    With the camera off, hold these while switching it on, and keep holding until
                    {downloadType() === 'green-led'
                      ? 'the green LED lights:'
                      : 'the screen lights:'}
                  </p>
                  <Show
                    when={result().sdBoot}
                    fallback={
                      <p class="recovery-error">
                        The {result().model}’s SD boot buttons are not in the catalog yet.
                      </p>
                    }
                  >
                    {(controls) => (
                      <PowerComboKeys
                        controls={controls()}
                        modes={downloadType() === 'green-led' ? ['sd'] : ['sd', 'firmware_update']}
                      />
                    )}
                  </Show>
                  <aside class="recovery-led">
                    <p>
                      If the system image won’t boot, use this image to check your SD card boot
                      instructions. It only turns on the rear green LED. If the light comes on, your
                      camera is booting from the SD card. Remove the battery to turn the camera off.
                    </p>
                    <button
                      class="launcher-button recovery-download recovery-led-download"
                      disabled={building() || !result().greenLedFiles.length}
                      onClick={() => {
                        const firmware = chosen();
                        if (firmware) void download(firmware, 'green-led');
                      }}
                    >
                      {building() && downloadType() === 'green-led'
                        ? 'Building…'
                        : 'Download green LED image'}
                    </button>
                    <Show when={!result().greenLedFiles.length}>
                      <p class="recovery-error">
                        Green LED bindings are not available for the {result().model} yet.
                      </p>
                    </Show>
                  </aside>
                </>
              )}
            </Show>
          </li>

          <Show when={downloadType() === 'system'}>
            <li class={{ 'recovery-pending': !plan() }}>
              <h2>Flash the firmware as usual</h2>
              <p>Follow the on-screen firmware update to flash the official .DAT from the card.</p>
            </li>
          </Show>
        </ol>
      </main>
    </div>
  );
}
