import type { ShareTrafficEvent } from './share-traffic.js';
export type { ShareTrafficEvent } from './share-traffic.js';
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
  /** `reason`: why the controller connected (native tools send their command line), or ''. */
  connected?(id: string, reason: string): void;
  /** `reason`: the controller's goodbye ("done" or its error), or 'connection lost without goodbye'. */
  disconnected?(id: string, reason: string): void;
  /** Passive owner-side request and completion observation, including failed operations. */
  traffic?(event: ShareTrafficEvent): void;
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
