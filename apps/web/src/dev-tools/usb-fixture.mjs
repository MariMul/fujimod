// Test-only WebUSB device. The page still uses production Wasm and nusb.
import { installWebUsb } from '../../../../bindings/javascript/tests/webusb-fixture.mjs';
export function installConsoleFixture() {
  const fixture = installWebUsb();
  const original = fixture.device.transferOut.bind(fixture.device);
  fixture.device.transferOut = async (endpoint, data) => {
    const result = await original(endpoint, data);
    const bytes = new Uint8Array(data),
      view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    if (
      fixture.state.kind === 0xd0 &&
      bytes.length >= 20 &&
      view.getUint32(0, true) === 0x200001 &&
      view.getUint32(16, true) === 7
    ) {
      const text = new TextEncoder().encode(
        'Fuji console ready\n' + '0123456789abcdef '.repeat(28) + '\n',
      );
      const reply = new Uint8Array(32 + text.length),
        header = new DataView(reply.buffer);
      header.setUint32(0, 0x200001, true);
      header.setUint32(16, 7, true);
      header.setUint32(28, text.length, true);
      reply.set(text, 32);
      fixture.state.pending = reply;
    }
    return result;
  };
  return fixture;
}
