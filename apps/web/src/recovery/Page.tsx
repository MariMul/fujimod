import { For, Show, createSignal, onSettled } from 'solid-js';
import { downloadBlob } from '@fujimod/emulator-ui/ui/download';
import { zipBlob } from '@fujimod/emulator-ui/archive';
import { identify, type IdentifiedFirmware } from './identify';
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
  let dat: Uint8Array<ArrayBuffer> | undefined;

  onSettled(() => {
    setSystem(detectSystem(navigator.userAgent));
  });

  async function chooseDat(file: File | undefined) {
    if (!file) return;
    setError('');
    setIdentified(undefined);
    setCard(undefined);
    setBusy(true);
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      const result = await identify(bytes);
      dat = bytes;
      setIdentified(result);
      const files = await recoveryCard(result.firmware.id, bytes);
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
            <p>
              Download the firmware update for your camera from Fujifilm, then choose the .DAT file.
            </p>
            <input
              type="file"
              accept=".dat,.DAT"
              disabled={busy()}
              onChange={(event) => void chooseDat(event.currentTarget.files?.[0])}
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
                    class="recovery-download"
                    onClick={() => downloadBlob(built().zip, 'fujimod-recovery.zip')}
                  >
                    Download recovery files
                  </button>
                  <p>
                    Unzip onto the card's top level:{' '}
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
                  <p>
                    Copy the official .DAT onto the card as well, then insert it into the camera.
                  </p>
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
                  <ul class="recovery-holds">
                    <For each={result().sdBoot}>{(hold) => <li>{hold}</li>}</For>
                    <li>also hold DISP/BACK, to enter firmware update</li>
                  </ul>
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
