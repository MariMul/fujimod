import { usb, type UsbError, type Session, type Device, type InfoReply } from '../src/index.js';
declare const session: Session;
declare const device: Device;
const bytes: Promise<Uint8Array> = session.readMemory(0x1000, 4);
const info: Promise<InfoReply> = session.deviceInfo(usb.InfoSelector.Product);
const opened: Promise<Session> = usb.Session.open(device, new usb.InterfaceSelection(1,0,0), new usb.FirmwareLimits(256,256,256,256),new usb.TransferOptions(1000,1024));
type NotAny<T> = 0 extends (1 & T) ? never : T;
async function inspect(): Promise<void> {
  const reply=await info;
  const raw: NotAny<typeof reply.raw> = reply.raw;
  const array: Uint8Array = raw;
  for await (const part of usb.memory.readChunks(session,0x1000,8)) { const output: Uint8Array = part; void output; }
  void array;
}
function errors(value: unknown): void {
  if(usb.isError(value)){const error: UsbError=value;const kind=error.kind;void kind;}
}
// @ts-expect-error Each choice group has its own enum type.
session.deviceInfo(usb.DataReadMode.Echo);
// @ts-expect-error Protocol choices are enums, not strings.
session.readConfiguration('Ram',0,0,1);
// @ts-expect-error Sessions come from explicit opens.
new usb.Session();
// @ts-expect-error Observational state cannot be assigned.
session.state=usb.SessionState.Ready;
void bytes; void opened; void inspect; void errors;
