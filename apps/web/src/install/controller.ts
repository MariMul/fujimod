export interface InstallTarget {
  id: string;
  model: string;
  version: string;
  serviceInstructions: string[];
  bootInstructions: string[];
}
export interface Connection {
  session: string;
  firmware: string;
  debug: boolean;
  mode: 'record' | 'service';
}
export interface Card {
  id: string;
  filesystem: 'FAT32' | 'exFAT' | 'other';
  capacity: number;
  free: number;
}
export interface BootFile {
  name: 'ffboot.bin' | 'ffsys.bin';
  data: Uint8Array<ArrayBuffer>;
  sha256: string;
}
export interface InstallerAdapter {
  available: boolean;
  canFormat: boolean;
  connect(target: InstallTarget): Promise<Connection>;
  guard(session: string): Promise<{ armed: boolean; settingsUnchanged: boolean }>;
  card(session: string): Promise<Card>;
  files(target: InstallTarget): Promise<{ firmware: string; files: BootFile[] }>;
  format(session: string, card: string): Promise<void>;
  write(session: string, card: string, file: BootFile): Promise<void>;
  read(session: string, card: string, name: BootFile['name']): Promise<Uint8Array<ArrayBuffer>>;
  abandon(): void;
  onDisconnect?(callback: () => void): () => void;
}
export type Step =
  | 'camera'
  | 'connect'
  | 'service'
  | 'card'
  | 'install'
  | 'manual-format'
  | 'battery'
  | 'boot'
  | 'confirm'
  | 'done'
  | 'error';
export interface InstallerState {
  step: Step;
  busy: boolean;
  message: string;
  target?: InstallTarget;
  card?: Card;
  guard: 'unchecked' | 'armed' | 'not-needed';
  progress: number;
  verified: string[];
}
export const initialState = (): InstallerState => ({
  step: 'camera',
  busy: false,
  message: '',
  guard: 'unchecked',
  progress: 0,
  verified: [],
});
export async function sha256(data: Uint8Array<ArrayBuffer>): Promise<string> {
  return [...new Uint8Array(await crypto.subtle.digest('SHA-256', data))]
    .map((value) => value.toString(16).padStart(2, '0'))
    .join('');
}
export class Installer {
  state = initialState();
  private connection?: Connection;
  private epoch = 0;
  private listeners = new Set<(state: InstallerState) => void>();
  private stop?: () => void;
  constructor(readonly adapter: InstallerAdapter) {
    this.stop = adapter.onDisconnect?.(() => this.disconnected());
  }
  subscribe(listener: (state: InstallerState) => void) {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }
  private update(value: Partial<InstallerState>) {
    this.state = { ...this.state, ...value };
    for (const listener of this.listeners) listener(this.state);
  }
  choose(target: InstallTarget) {
    if (this.state.busy || !['camera', 'connect', 'service'].includes(this.state.step)) return;
    this.epoch++;
    this.adapter.abandon();
    this.connection = undefined;
    this.update({ ...initialState(), target, step: 'connect' });
  }
  private async operation(run: (current: () => void) => Promise<void>) {
    if (this.state.busy) return;
    const epoch = this.epoch;
    const current = () => {
      if (epoch !== this.epoch) throw Error('Camera disconnected. Installation stopped.');
    };
    this.update({ busy: true, message: '' });
    try {
      await run(current);
    } catch (error) {
      if (epoch === this.epoch)
        this.update({
          step: 'error',
          message: String(error instanceof Error ? error.message : error),
        });
    } finally {
      if (epoch === this.epoch) this.update({ busy: false });
    }
  }
  async connect() {
    if (!this.state.target || !['connect', 'service', 'card'].includes(this.state.step)) return;
    await this.operation(async (current) => {
      const target = this.state.target!;
      const connection = await this.adapter.connect(target);
      current();
      if (connection.firmware !== target.id)
        throw Error('The connected camera or firmware does not match your selection.');
      this.connection = connection;
      if (!connection.debug) {
        this.update({ step: 'service' });
        return;
      }
      await this.guard(current);
      const card = await this.adapter.card(connection.session);
      current();
      this.update({ step: 'card', card });
    });
  }
  private async guard(current: () => void) {
    const connection = this.connection!;
    if (connection.mode === 'record') {
      this.update({ guard: 'not-needed' });
      return;
    }
    const proof = await this.adapter.guard(connection.session);
    current();
    if (!proof.armed || !proof.settingsUnchanged)
      throw Error('The settings-reset guard could not be verified. Keep USB connected.');
    this.update({ guard: 'armed' });
  }
  private async checkCard(current: () => void, expected: Card) {
    const actual = await this.adapter.card(this.connection!.session);
    current();
    if (actual.id !== expected.id || actual.capacity !== expected.capacity)
      throw Error('The SD card changed. Reconnect and check it before continuing.');
    if (expected.filesystem === 'FAT32' && actual.filesystem !== 'FAT32')
      throw Error('The SD card is no longer FAT32.');
    return actual;
  }
  async format(consent: boolean) {
    if (
      this.state.step !== 'card' ||
      this.state.card?.filesystem !== 'exFAT' ||
      !consent ||
      !this.adapter.canFormat
    )
      return;
    await this.operation(async (current) => {
      const card = this.state.card!,
        session = this.connection!.session;
      const actual = await this.checkCard(current, card);
      if (actual.filesystem !== 'exFAT')
        throw Error('The card format changed. Reconnect before formatting.');
      await this.adapter.format(session, card.id);
      current();
      const formatted = await this.checkCard(current, card);
      if (formatted.filesystem !== 'FAT32')
        throw Error('Formatting did not produce a verified FAT32 card.');
      this.update({ card: formatted });
    });
  }
  async manualFormat() {
    if (this.state.step !== 'card' || this.state.card?.filesystem === 'FAT32') return;
    await this.operation(async (current) => {
      await this.guard(current);
      this.update({ step: 'manual-format' });
    });
  }
  reconnectFormatted() {
    if (this.state.step !== 'manual-format' || this.state.busy) return;
    this.epoch++;
    this.adapter.abandon();
    this.connection = undefined;
    this.update({ step: 'connect', card: undefined, guard: 'unchecked' });
  }
  async install() {
    if (this.state.step !== 'card' || this.state.card?.filesystem !== 'FAT32') return;
    await this.operation(async (current) => {
      const target = this.state.target!,
        card = this.state.card!,
        session = this.connection!.session;
      const packageData = await this.adapter.files(target);
      current();
      if (packageData.firmware !== target.id)
        throw Error('The installation package is for a different camera firmware.');
      const files = packageData.files;
      if (
        files.length !== 2 ||
        new Set(files.map((file) => file.name)).size !== 2 ||
        files.some(
          (file) =>
            !['ffboot.bin', 'ffsys.bin'].includes(file.name) ||
            !file.data.length ||
            !/^[a-f0-9]{64}$/.test(file.sha256),
        )
      )
        throw Error(
          'The installation package must contain ffboot.bin and ffsys.bin with SHA-256 hashes.',
        );
      for (const file of files) {
        if ((await sha256(file.data)) !== file.sha256)
          throw Error(`The source hash does not match for ${file.name}.`);
        current();
      }
      const checked = await this.checkCard(current, card);
      if (checked.filesystem !== 'FAT32') throw Error('The SD card is no longer FAT32.');
      if (checked.free < files.reduce((sum, file) => sum + file.data.length, 0))
        throw Error('There is not enough free space on the SD card.');
      this.update({ step: 'install', verified: [], progress: 0 });
      for (const [index, file] of files.entries()) {
        await this.checkCard(current, card);
        this.update({ message: `Copying ${file.name}`, progress: index * 50 });
        await this.adapter.write(session, card.id, file);
        current();
        await this.checkCard(current, card);
        this.update({ message: `Reading back ${file.name}`, progress: index * 50 + 25 });
        const readback = await this.adapter.read(session, card.id, file.name);
        current();
        if (readback.length !== file.data.length || (await sha256(readback)) !== file.sha256)
          throw Error(`Read-back verification failed for ${file.name}. Keep USB connected.`);
        current();
        this.update({ verified: [...this.state.verified, file.name], progress: (index + 1) * 50 });
      }
      await this.guard(current);
      this.update({ step: 'battery', message: '' });
    });
  }
  advance() {
    if (this.state.busy) return;
    if (this.state.step === 'battery') {
      this.epoch++;
      this.adapter.abandon();
      this.connection = undefined;
      this.update({ step: 'boot' });
    } else if (this.state.step === 'boot') this.update({ step: 'confirm' });
    else if (this.state.step === 'confirm') this.update({ step: 'done' });
  }
  private disconnected() {
    if (
      ['battery', 'boot', 'confirm', 'done', 'camera', 'manual-format'].includes(this.state.step) ||
      (!this.state.busy && ['connect', 'service'].includes(this.state.step))
    )
      return;
    this.epoch++;
    this.connection = undefined;
    this.adapter.abandon();
    this.update({
      step: 'error',
      busy: false,
      message: 'Camera disconnected. Installation stopped; file verification is incomplete.',
    });
  }
  dispose() {
    this.epoch++;
    this.stop?.();
    this.adapter.abandon();
    this.listeners.clear();
  }
}
