import type { Firmware } from '../catalog';
import type { CameraChoice, ConsoleOutput, CameraDiagnostics } from './camera';
import type { NewEntry } from './log';
export enum EmulatorCall {
  Boot,
  Open,
  Submit,
  Poll,
  Abandon,
  Diagnostics,
}
export type EmulatorRequest =
  | { id: number; call: EmulatorCall.Boot; firmware: Firmware; dat: ArrayBuffer; baseUrl: string }
  | { id: number; call: EmulatorCall.Open; interface: number }
  | { id: number; call: EmulatorCall.Submit; data: Uint8Array }
  | { id: number; call: EmulatorCall.Poll | EmulatorCall.Abandon | EmulatorCall.Diagnostics };
export type EmulatorReply =
  | {
      type: 'result';
      id: number;
      result?: CameraChoice | ConsoleOutput | CameraDiagnostics;
      usable: boolean;
    }
  | { type: 'error'; id: number; message: string; usable: boolean }
  | { type: 'trace'; entries: NewEntry[] };
