import { CameraTarget, WebUsbCamera, RemoteUsbCamera, type CameraBackend } from './camera';
/** Select a device source; both execute the same USB/protocol implementation. */
export class CameraSources implements CameraBackend {
  private physical = new WebUsbCamera();
  private remote = new RemoteUsbCamera();
  private selected: CameraBackend = this.physical;
  private removeDisconnect: () => void = () => {};
  async initialize() {
    if ('usb' in navigator) await this.physical.initialize();
  }
  async pick(target = CameraTarget.Usb) {
    this.selected = target === CameraTarget.Remote ? this.remote : this.physical;
    return this.selected.pick();
  }
  async diagnostics() {
    return (
      this.selected.diagnostics?.() ?? {
        snapshot: { unavailable: 'Physical USB connections have no emulator snapshot.' },
      }
    );
  }
  canShare() {
    return this.selected !== this.remote;
  }
  exporter() {
    if (!this.selected.exporter) throw Error('A remote camera cannot be shared again');
    return this.selected.exporter();
  }
  async disconnect() { await this.selected.disconnect?.(); }
  open(id: number) {
    return this.selected.open(id);
  }
  drain() {
    return [...this.physical.drain(), ...this.remote.drain()];
  }
  onDisconnect(callback: () => void) {
    this.removeDisconnect = this.physical.onDisconnect(() => {
      if (this.selected === this.physical) callback();
    });
    return () => this.removeDisconnect();
  }
  dispose() {
    this.removeDisconnect();
    this.physical.dispose();
    this.remote.dispose();
  }
}
