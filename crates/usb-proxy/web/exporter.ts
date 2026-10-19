import { VERSION, encodeMessage, decodeMessage } from './wire.ts';
import { Exporter } from './usb-exporter.ts';
import { Pipeline } from './pipeline.ts';
import type { Message } from './types.ts';
if (typeof document !== 'undefined') {
  const $ = (id: string) => {
    const element = document.getElementById(id);
    if (!element) throw Error(`Missing element ${id}`);
    return element;
  };
  const button = (id: string) => {
    const element = $(id);
    if (!(element instanceof HTMLButtonElement)) throw Error(`Expected button ${id}`);
    return element;
  };
  let socket: WebSocket | undefined, exporter: Exporter | undefined;
  const status = (text: string) => {
    $('status').textContent = text;
  };
  const update = () => {
    $('devices').replaceChildren(
      ...[...(exporter?.devices || [])].map(([id, d]) => {
        const row = document.createElement('tr');
        for (const value of [
          d.productName || d.manufacturerName || 'USB device',
          `${d.vendorId.toString(16).padStart(4, '0')}:${d.productId.toString(16).padStart(4, '0')}`,
          exporter!.fenced.has(id)
            ? 'Completion unknown'
            : exporter!.claims.has(id)
              ? 'In use'
              : 'Available',
        ]) {
          const cell = document.createElement('td');
          cell.textContent = value;
          row.append(cell);
        }
        return row;
      }),
    );
  };
  const inventory = async () => {
    const active = exporter;
    if (!active) return;
    const devices = await active.inventory();
    if (exporter === active && socket?.readyState === WebSocket.OPEN)
      socket.send(encodeMessage({ type: 'inventory', devices }));
    update();
  };
  const disconnected = async () => {
    const stopping = exporter;
    const active = socket;
    socket = undefined;
    active?.close();
    for (const id of ['add', 'refresh', 'disconnect']) button(id).disabled = true;
    button('connect').disabled = true;
    status('Disconnecting');
    await stopping?.stop();
    if (exporter === stopping) {
      button('connect').disabled = false;
      status('Disconnected');
      update();
    }
  };
  $('connect-form').onsubmit = (event) => {
    event.preventDefault();
    if (!navigator.usb) {
      status('WebUSB unavailable');
      return;
    }
    exporter = new Exporter(navigator.usb, update);
    socket = new WebSocket(
      `${location.protocol === 'https:' ? 'wss:' : 'ws:'}//${location.host}/usb-proxy/ws`,
    );
    socket.binaryType = 'arraybuffer';
    const active = socket;
    button('connect').disabled = true;
    status('Connecting');
    socket.onopen = () => {
      active.send(
        encodeMessage({
          type: 'hello',
          version: VERSION,
          role: 'exporter',
          token: ($('token') as HTMLInputElement).value,
          reason: null,
        }),
      );
    };
    // One pipeline per controller stream the relay routes here, each run in
    // order on its own chain.
    const streams = new Map<number, { pipeline: Pipeline; chain: Promise<void> }>();
    const reply = (stream: number, message: Message) => {
      if (active.readyState === WebSocket.OPEN)
        active.send(encodeMessage({ type: 'routed', stream, message }));
      update();
    };
    socket.onmessage = async ({ data }) => {
      const message = decodeMessage(data);
      if (message.type === 'ready' && message.version === VERSION) {
        status(`Connected · ${message.id}`);
        for (const id of ['add', 'refresh', 'disconnect']) button(id).disabled = false;
        await inventory();
        return;
      }
      if (message.type !== 'routed') return;
      const { stream, message: inner } = message;
      if (inner.type === 'goodbye') {
        streams.delete(stream);
        return;
      }
      if (!Pipeline.carries(inner) || !exporter) return;
      const active = exporter;
      let entry = streams.get(stream);
      if (!entry) {
        entry = {
          pipeline: new Pipeline((m) => active.run(m.device, m.op, m.timeout_ms)),
          chain: Promise.resolve(),
        };
        streams.set(stream, entry);
      }
      const { pipeline } = entry;
      entry.chain = entry.chain.then(async () => {
        const answer = await pipeline.handle(inner);
        if (answer) reply(stream, answer);
      });
    };
    socket.onclose = () => {
      if (socket === active) void disconnected();
    };
    socket.onerror = () => status('Connection failed');
  };
  $('add').onclick = async () => {
    try {
      await navigator.usb?.requestDevice({ filters: [] });
      await inventory();
    } catch (error) {
      if (!(error instanceof Error) || error.name !== 'NotFoundError')
        status(error instanceof Error ? error.message : String(error));
    }
  };
  $('refresh').onclick = () => inventory().catch((error) => status(error.message));
  $('disconnect').onclick = disconnected;
  navigator.usb?.addEventListener('connect', () => {
    if (socket?.readyState === WebSocket.OPEN) void inventory();
  });
  navigator.usb?.addEventListener('disconnect', () => {
    if (socket?.readyState === WebSocket.OPEN) void inventory();
  });
}
