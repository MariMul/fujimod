export interface Descriptor {
  id: string;
  vendor_id: number;
  product_id: number;
  manufacturer: string | null;
  product: string | null;
  serial: string | null;
  interfaces: unknown[];
}
export interface Operation {
  op: string;
  [key: string]: unknown;
}
export interface Completion {
  devices?: Descriptor[];
  data?: string;
  transferred?: number;
  error?: string | null;
}
export interface SharedDevice {
  descriptor: Descriptor;
  execute(operation: Operation, timeout: number): Promise<Completion>;
  close(): Promise<void>;
}
export interface Ticket {
  v: 1;
  signal: string;
  room: string;
  key: string;
  stun: string[];
}
export interface Callbacks {
  connected?(id: string): void;
  disconnected?(id: string): void;
  error?(error: Error): void;
}
export function newTicket(signal: string, stun?: string[]): Ticket;
export function parseShare(url: string): Ticket;
export function shareUrl(current: string, ticket: Ticket): string;
export function connectPeer(url: string): Promise<PeerChannel>;
export class PeerChannel {
  send(text: string): Promise<void>;
  receive(): Promise<string>;
  close(): void;
}
export class ShareHost {
  static create(ticket: Ticket, device: SharedDevice, callbacks?: Callbacks): Promise<ShareHost>;
  stop(): void;
}
