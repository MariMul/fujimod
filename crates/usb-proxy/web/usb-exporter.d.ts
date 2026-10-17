import type { Descriptor, Operation, Completion } from './peer.js';
export class Exporter {
  constructor(usb: { getDevices(): Promise<unknown[]> }, changed?: () => void);
  onreply?: (frame: { type: string; id: number; completion: Completion }) => void;
  inventory(): Promise<Descriptor[]>;
  execute(id: string, operation: Operation): Promise<Completion>;
  request(frame: {
    id: number;
    device: string;
    operation: Operation;
    timeout_ms: number;
  }): Promise<void>;
  stop(): Promise<void>;
}
