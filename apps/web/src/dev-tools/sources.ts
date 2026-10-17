import { CameraTarget, WebUsbCamera, type CameraBackend } from './camera';
import { EmulatorCamera } from './emulator-camera';
/** Select a device source; both execute the same USB/protocol implementation. */
export class CameraSources implements CameraBackend {
  private physical = new WebUsbCamera();
  private emulator = new EmulatorCamera();
  private selected: CameraBackend = this.physical;
  private removeDisconnect: () => void = () => {};
  async initialize() {
    // Safari has no physical WebUSB backend. Its simulator initializes USB
    // in the worker; avoid creating an unused Wasm instance in this page.
    if ('usb' in navigator) await this.physical.initialize();
  }
  async pick(target = CameraTarget.Usb) {
    this.selected = target === CameraTarget.Emulator ? this.emulator : this.physical;
    return this.selected.pick();
  }
  open(id: number) {
    return this.selected.open(id);
  }
  drain() {
    return [...this.physical.drain(), ...this.emulator.drain()];
  }
  onDisconnect(callback: () => void) {
    this.removeDisconnect = this.physical.onDisconnect(() => {
      if (this.selected === this.physical) callback();
    });
    return () => this.removeDisconnect();
  }
  cancelPending() {
    this.emulator.cancelPending();
  }
  dispose() {
    this.removeDisconnect();
    this.physical.dispose();
    this.emulator.dispose();
  }
}
