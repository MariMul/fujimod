import type { Firmware } from '../catalog';
import type { CameraChoice, ConsoleOutput, CameraDiagnostics } from './camera';
import type { Descriptor, Operation, Completion } from './usb-sharing';
import type { NewEntry } from './log';
export enum EmulatorCall {
  Boot = 'Boot',
  Open = 'Open',
  Submit = 'Submit',
  Poll = 'Poll',
  Abandon = 'Abandon',
  Diagnostics = 'Diagnostics',
  ExportUsb = 'ExportUsb',
  Usb = 'Usb',
  CloseUsb = 'CloseUsb',
  Close = 'Close',
}
export type EmulatorRequest =
  | { id: number; call: EmulatorCall.Boot; firmware: Firmware; dat: ArrayBuffer; baseUrl: string }
  | { id: number; call: EmulatorCall.Open; interface: number }
  | { id: number; call: EmulatorCall.Submit; data: Uint8Array }
  | { id: number; call: EmulatorCall.Usb; operation: Operation; timeout: number }
  | {
      id: number;
      call:
        | EmulatorCall.Poll
        | EmulatorCall.Abandon
        | EmulatorCall.Diagnostics
        | EmulatorCall.ExportUsb
        | EmulatorCall.CloseUsb
        | EmulatorCall.Close;
    };
export type EmulatorReply =
  | {
      type: 'result';
      id: number;
      result?: CameraChoice | ConsoleOutput | CameraDiagnostics | Descriptor | Completion;
      usable: boolean;
    }
  | { type: 'error'; id: number; message: string; usable: boolean }
  | { type: 'trace'; entries: NewEntry[] };
