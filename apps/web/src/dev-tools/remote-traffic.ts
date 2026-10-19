import type { Descriptor, Operation } from './usb-sharing';
import type { ShareTrafficEvent } from '../../../../crates/usb-proxy/web/share-traffic.ts';
import { Direction, Layer, Level, describeCommand, type NewEntry } from './log';

const CAPTURE = 4096;
const MAX_QUEUE = 128;
const MAX_GROUPS = 64;
const MAX_CONTROLLERS = 32;
const SUMMARY_MS = 250;
const envelopeNames: Record<number, string> = {
  0xc0: 'Open session',
  0xc2: 'Ping',
  0xc4: 'Close session',
  0xd0: 'Write command',
  0xd1: 'Read command',
  0xe0: 'Write data',
  0xe1: 'Read data',
  0xe8: 'Empty response',
};
const text = (value: unknown, limit = 128) =>
  (typeof value === 'string' ? value : typeof value === 'number' ? String(value) : '').slice(
    0,
    limit,
  );
const number = (value: unknown) =>
  typeof value === 'number' && Number.isFinite(value) ? Math.max(0, value) : 0;
const hex = (value: number) => `0x${value.toString(16)}`;
const size = (data: unknown) => (data instanceof Uint8Array ? data.byteLength : 0);
function bytes(data: unknown, limit = CAPTURE): Uint8Array {
  return data instanceof Uint8Array ? data.slice(0, limit) : new Uint8Array();
}
const word = (data: Uint8Array, offset: number) =>
  new DataView(data.buffer, data.byteOffset, data.byteLength).getUint32(offset, true);
interface Frame {
  kind: number;
  expected: number;
  received: number;
  capture: Uint8Array;
  endpoint?: number;
}
interface Controller {
  frame?: Frame;
  command?: { code: number; fn?: number; exchange: number };
}
interface Group {
  controller: string;
  label: string;
  time: number;
  first: number;
  last: number;
  requests: number;
  completions: number;
  requested: number;
  transferred: number;
  errors: number;
  error: string;
  elapsed: number;
  timeout: number;
  direction: Direction;
  sample: Uint8Array;
}

/** Passive decoding only: bounded prefixes, queues and controller state. Raw
 * transfers are summarized per operation/controller in 250 ms windows. */
export class RemoteTrafficLog {
  private queue: NewEntry[] = [];
  private groups = new Map<string, Group>();
  private controllers = new Map<string, Controller>();
  private bulk = new Set<number>();
  private discarded = 0;
  private exchange = 0;
  constructor(private now = () => Date.now()) {}
  begin(descriptor: Descriptor): NewEntry[] {
    const previous = this.end();
    this.bulk.clear();
    for (const iface of descriptor.interfaces) {
      const endpoints = (iface as { endpoints?: { address: number; kind: string }[] })?.endpoints;
      for (const endpoint of Array.isArray(endpoints) ? endpoints : []) {
        if (endpoint.kind === 'bulk') this.bulk.add(endpoint.address);
      }
    }
    return previous;
  }
  private enqueue(entry: NewEntry) {
    if (this.queue.length === MAX_QUEUE) {
      this.queue.shift();
      this.discarded++;
    }
    this.queue.push(entry);
  }
  private emit(key: string, group: Group) {
    this.groups.delete(key);
    const status = group.errors
      ? `${group.errors} failed: ${group.error}`
      : group.completions < group.requests
        ? 'pending'
        : 'ok';
    this.enqueue({
      layer: Layer.Connection,
      level: group.errors ? Level.Error : Level.Info,
      direction: group.direction,
      time: group.time,
      message: `Remote ${text(group.controller, 48)} · ${group.label} · ${group.requests} submitted, ${group.completions} completed · requested ${group.requested} B, transferred ${group.transferred} B · ${status}`,
      detail: `Controller ${text(group.controller)}; requests ${group.first}–${group.last}; last timeout ${group.timeout} ms; completed in ${group.elapsed.toFixed(1)} ms total. Captured prefix: ${group.sample.length} B.`,
      data: group.sample.length ? group.sample : undefined,
      correlation: `Remote ${text(group.controller)} #${group.first}–${group.last}`,
    });
  }
  private summarize(event: ShareTrafficEvent) {
    const op = event.operation;
    const envelope = this.envelope(op);
    const label = [
      text(op.op, 64),
      typeof op.endpoint === 'number' ? `ep ${hex(op.endpoint)}` : '',
      envelope ? envelopeNames[envelope.kind] : '',
      op.setup && !envelope ? text(JSON.stringify(op.setup), 256) : '',
      op.op === 'open'
        ? `configuration ${text(op.configuration)} interface ${text(op.interface)} alternate ${text(op.alternate)}`
        : '',
    ]
      .filter(Boolean)
      .join(' ');
    const key = JSON.stringify([text(event.controller), label]);
    let group = this.groups.get(key);
    if (!group) {
      if (this.groups.size === MAX_GROUPS) {
        const [oldKey, old] = this.groups.entries().next().value!;
        this.emit(oldKey, old);
      }
      group = {
        controller: event.controller,
        label,
        time: this.now(),
        first: event.id,
        last: event.id,
        requests: 0,
        completions: 0,
        requested: 0,
        transferred: 0,
        errors: 0,
        error: '',
        elapsed: 0,
        timeout: event.timeoutMs,
        direction: op.op.endsWith('_in')
          ? Direction.In
          : op.op.endsWith('_out')
            ? Direction.Out
            : Direction.Host,
        sample: new Uint8Array(),
      };
      this.groups.set(key, group);
    }
    group.last = event.id;
    group.timeout = event.timeoutMs;
    if (event.phase === 'submitted') {
      group.requests++;
      group.requested += op.op.endsWith('_in')
        ? number(op.length)
        : (op.dataLength ?? size(op.data));
      if (!group.sample.length) group.sample = bytes(op.data, 256);
    } else {
      group.completions++;
      group.transferred += number(event.completion.transferred);
      group.elapsed += number(event.elapsedMs);
      if (!group.sample.length) group.sample = bytes(event.completion.data, 256);
      if (event.completion.error) {
        group.errors++;
        group.error = text(event.completion.error, 512);
        this.emit(key, group);
      }
    }
  }
  private envelope(op: Readonly<Operation>) {
    const setup = op.setup as Record<string, unknown> | undefined;
    if (
      op.op !== 'control_out' ||
      setup?.request_type !== 'vendor' ||
      setup.recipient !== 'other' ||
      setup.request !== 0 ||
      setup.value !== 0 ||
      setup.index !== 0 ||
      (op.dataLength ?? size(op.data)) !== 12
    )
      return;
    const data = bytes(op.data, 12);
    if (data.length !== 12 || data[1] !== 0 || !envelopeNames[data[0]]) return;
    const view = new DataView(data.buffer, data.byteOffset, data.byteLength);
    return { kind: data[0], expected: view.getUint32(6), data };
  }
  private controller(id: string) {
    let state = this.controllers.get(id);
    if (!state) {
      if (this.controllers.size === MAX_CONTROLLERS)
        this.controllers.delete(this.controllers.keys().next().value!);
      state = {};
      this.controllers.set(id, state);
    }
    return state;
  }
  record(event: ShareTrafficEvent): void {
    this.summarize(event);
    if (event.phase !== 'completed') return;
    const state = this.controller(event.controller),
      op = event.operation;
    if (event.completion.error) {
      state.frame = undefined;
      state.command = undefined;
      return;
    }
    const envelope = this.envelope(op);
    if (op.op === 'open' || op.op === 'close') {
      state.frame = undefined;
      state.command = undefined;
    }
    if (op.op === 'control_out') {
      state.frame = undefined;
      // Only a successfully transferred full Fuji vendor envelope arms decoding.
      if (!envelope || event.completion.transferred !== 12) {
        state.command = undefined;
        return;
      }
      if (envelope.kind !== 0xd1) state.command = undefined;
      state.frame = {
        kind: envelope.kind,
        expected: envelope.expected,
        received: 0,
        capture: new Uint8Array(),
      };
      if ([0xc0, 0xc2, 0xc4].includes(envelope.kind)) {
        this.protocol(
          event,
          Direction.Out,
          describeCommand(envelope.data, envelope.kind),
          envelope.data,
          12,
        );
        state.command = undefined;
      }
      return;
    }
    const frame = state.frame;
    if (!frame || !this.bulk.has(number(op.endpoint))) return;
    const inbound = op.op === 'transfer_in';
    if (
      (!inbound && op.op !== 'transfer_out') ||
      inbound !== (frame.kind !== 0xd0 && frame.kind !== 0xe0)
    )
      return;
    if (frame.endpoint !== undefined && frame.endpoint !== op.endpoint) return;
    frame.endpoint = number(op.endpoint);
    // Data windows are opaque; their bulk chunks remain in the raw summary.
    if (![0xd0, 0xd1, 0xc0, 0xc2, 0xc4].includes(frame.kind)) return;
    const data = inbound ? event.completion.data : op.data;
    const available = inbound
      ? (event.completion.dataLength ?? size(data))
      : (op.dataLength ?? size(data));
    const transferred = number(event.completion.transferred);
    if (transferred > available) {
      state.frame = undefined;
      return;
    }
    const capture = bytes(data, Math.min(transferred, CAPTURE - frame.capture.length));
    const joined = new Uint8Array(frame.capture.length + capture.length);
    joined.set(frame.capture);
    joined.set(capture, frame.capture.length);
    frame.capture = joined;
    frame.received += transferred;
    if (frame.received > frame.expected) {
      state.frame = undefined;
      return;
    }
    if (frame.received < frame.expected && !(inbound && transferred < number(op.length))) return;
    state.frame = undefined;
    this.decode(event, state, frame);
  }
  private protocol(
    event: ShareTrafficEvent,
    direction: Direction,
    message: string,
    data: Uint8Array,
    total: number,
    status = '',
  ) {
    this.enqueue({
      layer: Layer.Ff80,
      level: status ? Level.Error : Level.Debug,
      direction,
      message: `${message}${status ? ' · failed' : ''}`,
      data,
      detail: `Controller ${text(event.controller)}; request #${event.id}${status ? `; ${status}` : ''}${total > data.length ? `; captured ${data.length} of ${total} B` : ''}`,
      correlation: `Remote ${text(event.controller)} FF80 #${this.controller(event.controller).command?.exchange ?? event.id}`,
    });
  }
  private decode(event: ShareTrafficEvent, state: Controller, frame: Frame) {
    const data = frame.capture,
      inbound = frame.kind !== 0xd0;
    if ([0xc0, 0xc2, 0xc4].includes(frame.kind)) {
      if (frame.received !== frame.expected) return;
      this.protocol(event, Direction.In, describeCommand(data, frame.kind), data, frame.received);
      return;
    }
    if (data.length < 16) return;
    const code = word(data, 0);
    if (inbound && (!state.command || state.command.code !== code || word(data, 12) !== 0)) return;
    if (
      inbound &&
      state.command?.fn !== undefined &&
      (data.length < 20 || word(data, 16) !== state.command.fn)
    )
      return;
    if (!inbound)
      state.command = {
        code,
        fn: code === 0x200001 && data.length >= 20 ? word(data, 16) : undefined,
        exchange: ++this.exchange,
      };
    const primary = inbound ? word(data, 4) : 0,
      secondary = inbound ? word(data, 8) : 0;
    const status = primary || secondary ? `Device status ${hex(primary)}/${hex(secondary)}` : '';
    this.protocol(
      event,
      inbound ? Direction.In : Direction.Out,
      describeCommand(data, code),
      data,
      frame.received,
      status,
    );
    if (inbound) state.command = undefined;
    if (code !== 0x200001 || data.length < 24 || status) return;
    const fn = word(data, 16);
    let offset: number,
      length: number,
      detail = '';
    if (!inbound && fn === 6) {
      offset = 24;
      length = word(data, 20);
      if (length > frame.received - 28) return;
    } else if (inbound && fn === 7 && data.length >= 32) {
      offset = 32;
      length = word(data, 28);
      if (length > frame.received - 32) return;
      detail = `Progress ${word(data, 20)}; status ${hex(word(data, 24))}. `;
    } else return;
    if (!length) return;
    const payload = data.slice(offset, offset + length);
    this.enqueue({
      layer: Layer.Console,
      level: Level.Info,
      direction: inbound ? Direction.In : Direction.Out,
      message: new TextDecoder().decode(payload).trimEnd(),
      data: payload,
      detail: `${detail}Controller ${text(event.controller)}; request #${event.id}${payload.length < length ? `; captured ${payload.length} of ${length} B` : ''}`,
      correlation: `Remote ${text(event.controller)} #${event.id}`,
    });
  }
  clear(): void {
    this.queue = [];
    this.groups.clear();
    this.discarded = 0;
  }
  drain(force = false): NewEntry[] {
    const now = this.now();
    for (const [key, group] of this.groups) {
      if (force || now - group.time >= SUMMARY_MS) this.emit(key, group);
    }
    const entries = this.queue.splice(0);
    if (this.discarded) {
      entries.push({
        layer: Layer.Connection,
        level: Level.Warning,
        direction: Direction.Host,
        message: `Remote traffic recorder discarded ${this.discarded} entries`,
      });
      this.discarded = 0;
    }
    return entries;
  }
  end(controller?: string): NewEntry[] {
    for (const [key, group] of this.groups) {
      if (controller === undefined || group.controller === controller) this.emit(key, group);
    }
    if (controller === undefined) this.controllers.clear();
    else this.controllers.delete(controller);
    return this.drain();
  }
}
