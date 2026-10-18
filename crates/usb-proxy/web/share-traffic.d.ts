import type { Operation, Completion } from './peer.js';
export interface RemoteRequest {
  id: number;
  device?: string;
  operation: Operation;
  timeout_ms: number;
}
interface Identity {
  controller: string;
  id: number;
  device?: string;
  operation: Readonly<Operation>;
  timeoutMs: number;
}
export type ShareTrafficEvent = Identity &
  (
    | { phase: 'submitted' }
    | {
        phase: 'completed';
        elapsedMs: number;
        completion: Readonly<
          Pick<Completion, 'data' | 'dataLength' | 'transferred' | 'error'> & {
            deviceCount: number;
          }
        >;
      }
  );
export function observeRequest(
  controller: string,
  request: RemoteRequest,
  execute: () => Promise<Completion>,
  callback?: (event: ShareTrafficEvent) => void,
): Promise<Completion>;
