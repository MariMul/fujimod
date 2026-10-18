import { For, Match, Show, Switch, createSignal, onSettled, untrack } from 'solid-js';
import { Installer, initialState, type InstallerAdapter, type InstallerState } from './controller';
import { x70Installer } from '../../devices/x70/installer';
import '../style.css';
import '../navigation.css';
import './style.css';

const unavailable: InstallerAdapter = {
  available: false,
  canFormat: false,
  async connect() {
    throw Error('The USB SD-card installer is not available yet.');
  },
  async guard() {
    throw Error('Service-exit guard adapter unavailable.');
  },
  async card() {
    throw Error('SD-card adapter unavailable.');
  },
  async files() {
    throw Error('No verified installation package is available.');
  },
  async format() {
    throw Error('SD-card formatting adapter unavailable.');
  },
  async write() {
    throw Error('SD-card transfer adapter unavailable.');
  },
  async read() {
    throw Error('SD-card transfer adapter unavailable.');
  },
  abandon() {},
};
export default function InstallerPage(
  props: { adapter?: InstallerAdapter; preview?: boolean } = {},
) {
  const [state, setState] = createSignal<InstallerState>(initialState());
  const [camera, setCamera] = createSignal('');
  const [consent, setConsent] = createSignal(false);
  const [preview, setPreview] = createSignal(false);
  const [adapter, setAdapter] = createSignal<InstallerAdapter>(
    untrack(() => props.adapter ?? unavailable),
  );
  let installer: Installer;
  onSettled(() => {
    let live = true;
    const attach = (value: InstallerAdapter, isPreview: boolean) => {
      if (!live) return;
      installer?.dispose();
      installer = new Installer(value);
      installer.subscribe(setState);
      setAdapter(value);
      setPreview(isPreview);
      setState(installer.state);
    };
    attach(
      untrack(() => props.adapter ?? unavailable),
      untrack(() => props.preview ?? false),
    );
    if (import.meta.env.DEV && !untrack(() => props.adapter)) {
      const requested = new URLSearchParams(location.search).get('preview');
      if (requested === 'fat32' || requested === 'exfat')
        void import('./preview').then(({ previewAdapter }) =>
          attach(previewAdapter(requested === 'fat32' ? 'FAT32' : 'exFAT'), true),
        );
    }
    return () => {
      live = false;
      installer?.dispose();
    };
  });
  const stepIndex = () => {
    const step = state().step;
    if (step === 'camera') return 0;
    if (step === 'connect' || step === 'service') return 1;
    if (step === 'card' || step === 'manual-format') return 2;
    if (step === 'install') return 3;
    if (step === 'battery' || step === 'boot') return 4;
    return 5;
  };
  const choose = () => {
    setConsent(false);
    installer.choose(x70Installer);
  };
  return (
    <div class="shell installer">
      <header>
        <a href="./" aria-label="FujiMod">
          <img src="brand.png" alt="FujiMod" width="110" height="28" />
        </a>
        <nav class="site-nav" aria-label="Pages">
          <a href="dev-tools/">Dev Tools</a>
        </nav>
        <Show when={preview()}>
          <span class="installer-preview">Preview · no camera connected</span>
        </Show>
      </header>
      <main>
        <div class="installer-heading">
          <p>SD CARD DUAL BOOT</p>
          <h1>Install FujiMod</h1>
        </div>
        <ol class="installer-steps" aria-label="Installation steps">
          <For each={['Camera', 'USB', 'SD card', 'Install', 'Restart', 'Confirm']}>
            {(label, index) => (
              <li aria-current={index() === stepIndex() ? 'step' : undefined}>{label}</li>
            )}
          </For>
        </ol>
        <section
          class="installer-panel"
          aria-live="polite"
          aria-busy={state().busy ? 'true' : 'false'}
        >
          <Switch>
            <Match when={state().step === 'camera'}>
              <h2>Choose your camera</h2>
              <label for="install-camera">Camera</label>
              <select
                id="install-camera"
                value={camera()}
                onChange={(event) => setCamera(event.currentTarget.value)}
              >
                <option value="" disabled>
                  Select a camera
                </option>
                <option value={x70Installer.id}>
                  {x70Installer.model} · firmware {x70Installer.version}
                </option>
              </select>
              <button class="primary" disabled={!camera()} onClick={choose}>
                Continue
              </button>
            </Match>
            <Match when={state().step === 'connect'}>
              <h2>Connect your {state().target?.model}</h2>
              <p>Insert the SD card and connect the camera to this device with a USB cable.</p>
              <Show when={!adapter().available}>
                <p role="status">
                  USB SD-card installation is not available for this firmware yet.
                </p>
              </Show>
              <button
                class="primary"
                disabled={state().busy || !adapter().available}
                onClick={() => installer.connect()}
              >
                {state().busy ? 'Checking camera…' : 'Connect camera'}
              </button>
            </Match>
            <Match when={state().step === 'service'}>
              <h2>Reboot into service mode</h2>
              <p>The camera is not in debug mode.</p>
              <ol>
                <For each={state().target?.serviceInstructions ?? []}>
                  {(text) => <li>{text}</li>}
                </For>
              </ol>
              <button class="primary" disabled={state().busy} onClick={() => installer.connect()}>
                {state().busy ? 'Checking camera…' : 'Reconnect camera'}
              </button>
            </Match>
            <Match when={state().step === 'card'}>
              <h2>Check the SD card</h2>
              <div class="installer-card">
                <strong>{state().card?.filesystem}</strong>
                <span>{Math.round((state().card?.capacity ?? 0) / 1024 ** 3)} GB</span>
              </div>
              <Show when={state().card?.filesystem === 'FAT32'}>
                <p>
                  Ready to copy <code>ffboot.bin</code> and <code>ffsys.bin</code>.
                </p>
                <button class="primary" disabled={state().busy} onClick={() => installer.install()}>
                  Install FujiMod
                </button>
              </Show>
              <Show when={state().card?.filesystem !== 'FAT32'}>
                <p>Dual boot requires FAT32.</p>
                <button disabled={state().busy} onClick={() => installer.manualFormat()}>
                  Format on my computer
                </button>
                <Show when={state().card?.filesystem === 'exFAT' && adapter().canFormat}>
                  <div class="installer-erase">
                    <h3>Or erase and format this card</h3>
                    <label>
                      <input
                        type="checkbox"
                        checked={consent()}
                        disabled={state().busy}
                        onChange={(event) => setConsent(event.currentTarget.checked)}
                      />{' '}
                      Delete all photos and files on this SD card.
                    </label>
                    <button
                      class="danger"
                      disabled={!consent() || state().busy}
                      onClick={() => {
                        void installer.format(consent());
                        setConsent(false);
                      }}
                    >
                      {state().busy ? 'Formatting…' : 'Erase card and format FAT32'}
                    </button>
                  </div>
                </Show>
              </Show>
            </Match>
            <Match when={state().step === 'manual-format'}>
              <h2>Format the card on your computer</h2>
              <p>
                {state().guard === 'armed'
                  ? 'Settings-reset guard verified.'
                  : 'Normal recording mode: service-exit guard not needed.'}
              </p>
              <ol>
                <li>
                  Turn the camera off while USB remains connected. Remove the battery, then
                  disconnect USB.
                </li>
                <li>
                  Remove the SD card and format it as FAT32 on your computer. Formatting deletes all
                  data on the card.
                </li>
                <li>Put the card and battery back in the camera, then reconnect USB.</li>
              </ol>
              <button class="primary" onClick={() => installer.reconnectFormatted()}>
                Card formatted — reconnect
              </button>
            </Match>
            <Match when={state().step === 'install'}>
              <h2>Copying and verifying</h2>
              <p>{state().message}</p>
              <progress max="100" value={state().progress} aria-label="File transfer" />
              <ul>
                <For each={state().verified}>{(name) => <li>{name} · SHA-256 verified</li>}</For>
              </ul>
              <p>Keep the camera powered on and USB connected.</p>
            </Match>
            <Match when={state().step === 'battery'}>
              <h2>Files verified</h2>
              <ul>
                <For each={state().verified}>{(name) => <li>{name} · SHA-256 verified</li>}</For>
              </ul>
              <p>
                {state().guard === 'armed'
                  ? 'Settings-reset guard verified.'
                  : 'Normal recording mode: service-exit guard not needed.'}
              </p>
              <ol>
                <li>Turn the camera off while USB is still connected.</li>
                <li>Remove the battery.</li>
                <li>Disconnect USB, then reinsert the battery.</li>
              </ol>
              <button class="primary" onClick={() => installer.advance()}>
                Battery reinserted
              </button>
            </Match>
            <Match when={state().step === 'boot'}>
              <h2>Start dual boot</h2>
              <ol>
                <For each={state().target?.bootInstructions ?? []}>{(text) => <li>{text}</li>}</For>
              </ol>
              <button class="primary" onClick={() => installer.advance()}>
                Camera is on
              </button>
            </Match>
            <Match when={state().step === 'confirm'}>
              <h2>Confirm FujiMod is running</h2>
              <p>Press MENU on the camera and check that the FujiMod menu appears.</p>
              <button class="primary" onClick={() => installer.advance()}>
                I can see the FujiMod menu
              </button>
              <details>
                <summary>The FujiMod menu did not appear</summary>
                <p>
                  Turn the camera off. Check the FAT32 card and both files, then try the dual-boot
                  button combination again.
                </p>
              </details>
            </Match>
            <Match when={state().step === 'done'}>
              <h2>FujiMod is ready</h2>
            </Match>
            <Match when={state().step === 'error'}>
              <h2>Installation stopped</h2>
              <p role="alert">{state().message}</p>
              <a href="dev-tools/">Open Dev Tools</a>
            </Match>
          </Switch>
          <Show
            when={
              state().guard === 'armed' &&
              !['battery', 'boot', 'confirm', 'done'].includes(state().step)
            }
          >
            <p class="installer-guard">Settings-reset guard armed</p>
          </Show>
        </section>
      </main>
    </div>
  );
}
