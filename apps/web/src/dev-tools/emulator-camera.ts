import { validateCatalog, type Catalog, type Firmware } from '../catalog';
import type {
  CameraBackend,
  CameraChoice,
  CameraLink,
  ConsoleOutput,
  CameraDiagnostics,
} from './camera';
import { EmulatorCall, type EmulatorRequest, type EmulatorReply } from './emulator-protocol';
import type { NewEntry } from './log';
declare const __FUJIMOD_CATALOG__: string;
declare const __FUJIMOD_LOCAL_DAT__: boolean;
function chooseFirmware(): Promise<File | undefined> {
  return new Promise((resolve) => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = '.dat';
    input.hidden = true;
    document.body.append(input);
    const finish = (file?: File) => {
      input.remove();
      resolve(file);
    };
    input.addEventListener('change', () => finish(input.files?.[0]), { once: true });
    input.addEventListener('cancel', () => finish(), { once: true });
    input.click();
  });
}
export class EmulatorCamera implements CameraBackend {
  private worker?: Worker;
  private errorListeners = new Set<(error: Error) => void>();
  errors(listener: (error: Error) => void) {
    this.errorListeners.add(listener);
    return () => {
      this.errorListeners.delete(listener);
    };
  }
  private serial = 0;
  private firmware?: Firmware;
  private pending = new Map<
    number,
    { resolve: (result: unknown) => void; reject: (error: Error) => void }
  >();
  private entries: NewEntry[] = [];
  private ready = false;
  private generation = 0;
  async initialize() {}
  async diagnostics(): Promise<CameraDiagnostics> {
    const identity = this.firmware && {
      firmwareId: this.firmware.id,
      model: this.firmware.model,
      firmwareSha256: this.firmware.sha256,
    };
    if (!this.worker)
      return { identity, snapshot: { unavailable: 'Firmware emulator worker is unavailable.' } };
    try {
      const result = (await this.request({ call: EmulatorCall.Diagnostics })) as CameraDiagnostics;
      return { ...result, identity: { ...(result.identity as object), ...identity } };
    } catch (error) {
      return { identity, snapshot: { unavailable: String(error) } };
    }
  }
  private request(
    request:
      | Omit<Extract<EmulatorRequest, { call: EmulatorCall.Boot }>, 'id'>
      | Omit<Extract<EmulatorRequest, { call: EmulatorCall.Open }>, 'id'>
      | Omit<Extract<EmulatorRequest, { call: EmulatorCall.Submit }>, 'id'>
      | Omit<
          Extract<
            EmulatorRequest,
            { call: EmulatorCall.Poll | EmulatorCall.Abandon | EmulatorCall.Diagnostics }
          >,
          'id'
        >,
  ): Promise<unknown> {
    const worker = this.worker;
    if (!worker) return Promise.reject(Error('Firmware emulator is not running.'));
    const id = ++this.serial;
    return new Promise((resolve, reject) => {
      const timeout =
        request.call === EmulatorCall.Diagnostics
          ? setTimeout(() => {
              this.pending.delete(id);
              reject(Error('Firmware diagnostic capture timed out.'));
            }, 30000)
          : undefined;
      this.pending.set(id, {
        resolve: (value) => {
          if (timeout) clearTimeout(timeout);
          resolve(value);
        },
        reject: (error) => {
          if (timeout) clearTimeout(timeout);
          reject(error);
        },
      });
      worker.postMessage(
        { ...request, id },
        request.call === EmulatorCall.Boot ? [request.dat] : [],
      );
    });
  }
  async pick(): Promise<CameraChoice | undefined> {
    const file = __FUJIMOD_LOCAL_DAT__ ? undefined : chooseFirmware();
    this.stop();
    const generation = this.generation;
    const baseUrl = new URL('../', document.baseURI).href;
    const response = await fetch(new URL(__FUJIMOD_CATALOG__, baseUrl));
    if (!response.ok) throw Error('Firmware catalog unavailable.');
    const catalog = validateCatalog((await response.json()) as Catalog);
    let firmware: Firmware | undefined, dat: ArrayBuffer;
    if (__FUJIMOD_LOCAL_DAT__) {
      const selected = new URLSearchParams(location.search).get('firmware');
      firmware = catalog.firmwares.find((f) => f.emulate?.usb && (!selected || f.id === selected));
      if (!firmware) throw Error('No USB firmware emulator is configured for this selection.');
      const input = await fetch(new URL(`local/firmware/${firmware.id}.dat`, baseUrl));
      if (!input.ok) throw Error('Configured firmware DAT is unavailable.');
      dat = await input.arrayBuffer();
    } else {
      const chosen = await file!;
      if (!chosen) return;
      dat = await chosen.arrayBuffer();
      const sha = [...new Uint8Array(await crypto.subtle.digest('SHA-256', dat))]
        .map((b) => b.toString(16).padStart(2, '0'))
        .join('');
      firmware = catalog.firmwares.find((f) => f.sha256 === sha);
      if (!firmware?.emulate?.usb) throw Error('This firmware has no configured USB emulator.');
    }
    if (generation !== this.generation) return;
    this.firmware = firmware;
    this.worker = new Worker(new URL('./emulator-worker.ts', import.meta.url), { type: 'module' });
    this.worker.onmessage = ({ data }: MessageEvent<EmulatorReply>) => {
      if (data.type === 'trace') {
        this.entries.push(...data.entries);
        return;
      }
      this.ready = data.usable;
      const pending = this.pending.get(data.id);
      this.pending.delete(data.id);
      if (data.type === 'error') pending?.reject(Error(data.message));
      else pending?.resolve(data.result);
    };
    this.worker.onerror = (event) => {
      const error = Error(event.message || 'Firmware emulator worker failed.');
      this.stop(error);
      for (const listener of this.errorListeners) listener(error);
    };
    try {
      return (await this.request({
        call: EmulatorCall.Boot,
        firmware,
        dat,
        baseUrl,
      })) as CameraChoice;
    } catch (error) {
      this.ready = false;
      throw error;
    }
  }
  async open(id: number): Promise<CameraLink> {
    await this.request({ call: EmulatorCall.Open, interface: id });
    return {
      submit: async (data) => {
        await this.request({ call: EmulatorCall.Submit, data });
      },
      poll: async () => (await this.request({ call: EmulatorCall.Poll })) as ConsoleOutput,
      usable: () => this.ready,
      abandon: () => this.stop(),
    };
  }
  drain(): NewEntry[] {
    return this.entries.splice(0);
  }
  onDisconnect(): () => void {
    return () => {};
  }
  cancelPending() {
    this.stop();
  }
  private stop(error = Error('Firmware emulator disconnected.')) {
    this.generation++;
    this.worker?.terminate();
    this.worker = undefined;
    this.ready = false;
    for (const pending of this.pending.values()) pending.reject(error);
    this.pending.clear();
  }
  dispose() {
    this.stop();
  }
}
