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
  data?: Uint8Array;
  /** Full byte count when this is a bounded diagnostic snapshot. */
  dataLength?: number;
  [key: string]: unknown;
}
export interface Completion {
  devices?: Descriptor[];
  data?: Uint8Array;
  transferred?: number;
  dataLength?: number;
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
  /** A controller reported the camera's battery level (firmware level name). */
  battery?(id: string, level: string): void;
  /** A controller asks the owner's user to fix something only it can see. */
  attention?(id: string, message: string): void;
  /** Neutral tool progress, including an expected refusal to continue. */
  toolStatus?(id: string, message: string): void;
  /** Resolves only after the owner's spoken ten-second countdown. */
  countdown?(id: string, message: string): Promise<void>;
  /** Online state or battery changed. */
  state?(status: ShareStatus): void;
  error?(error: Error): void;
}
export interface ShareStatus {
  online: boolean;
  /** Why the camera is offline: 'battery', 'unplugged' or 'not responding'. */
  reason?: string;
  battery?: string;
  battery_age_ms?: number;
}
export function newTicket(signal: string, stun?: string[]): Ticket;
export function parseShare(url: string): Ticket;
export function shareUrl(current: string, ticket: Ticket): string;
export function connectPeer(url: string): Promise<PeerChannel>;
export class PeerChannel {
  send(bytes: Uint8Array): Promise<void>;
  receive(): Promise<Uint8Array>;
  close(): void;
}
export class ShareHost {
  static create(ticket: Ticket, device: SharedDevice, callbacks?: Callbacks): Promise<ShareHost>;
  status(): ShareStatus;
  /** The camera went away; keep the link and report it offline with `reason`. */
  cameraLost(reason: string): void;
  /** The same camera is back (caller checked its identity); share it again. */
  cameraReturned(device: SharedDevice): void;
  stop(): void;
}
