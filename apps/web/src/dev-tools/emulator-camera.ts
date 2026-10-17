import { validateCatalog, type Catalog, type Firmware } from '../catalog';
import type { CameraBackend, CameraChoice, CameraLink, ConsoleOutput } from './camera';
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
  private serial = 0;
  private pending = new Map<
    number,
    { resolve: (result: unknown) => void; reject: (error: Error) => void }
  >();
  private entries: NewEntry[] = [];
  private ready = false;
  private generation = 0;
  async initialize() {}
  private request(
    request:
      | Omit<Extract<EmulatorRequest, { call: EmulatorCall.Boot }>, 'id'>
      | Omit<Extract<EmulatorRequest, { call: EmulatorCall.Open }>, 'id'>
      | Omit<Extract<EmulatorRequest, { call: EmulatorCall.Submit }>, 'id'>
      | Omit<Extract<EmulatorRequest, { call: EmulatorCall.Poll | EmulatorCall.Abandon }>, 'id'>,
  ): Promise<unknown> {
    const worker = this.worker;
    if (!worker) return Promise.reject(Error('Firmware emulator is not running.'));
    const id = ++this.serial;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
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
      this.stop(Error(event.message || 'Firmware emulator worker failed.'));
    };
    try {
      return (await this.request({
        call: EmulatorCall.Boot,
        firmware,
        dat,
        baseUrl,
      })) as CameraChoice;
    } catch (error) {
      this.stop();
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
