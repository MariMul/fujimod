import { Exporter } from './usb-exporter.js';
if (typeof document !== 'undefined') {
  const $ = (id) => document.getElementById(id);
  let socket, exporter;
  const status = (text) => { $('status').textContent = text; };
  const update = () => {
    $('devices').replaceChildren(...[...(exporter?.devices || [])].map(([id, d]) => {
      const row = document.createElement('tr');
      for (const value of [d.productName || d.manufacturerName || 'USB device', `${d.vendorId.toString(16).padStart(4, '0')}:${d.productId.toString(16).padStart(4, '0')}`, exporter.fenced.has(id) ? 'Completion unknown' : exporter.claims.has(id) ? 'In use' : 'Available']) {
        const cell = document.createElement('td'); cell.textContent = value; row.append(cell);
      }
      return row;
    }));
  };
  const inventory = async () => {
    const active = exporter;
    const devices = await active.inventory();
    if (exporter === active && socket?.readyState === WebSocket.OPEN) socket.send(JSON.stringify({ type: 'inventory', devices }));
    update();
  };
  const disconnected = async () => {
    const stopping = exporter;
    const active = socket; socket = undefined; active?.close();
    for (const id of ['add', 'refresh', 'disconnect']) $(id).disabled = true;
    $('connect').disabled = true; status('Disconnecting');
    await stopping?.stop();
    if (exporter === stopping) { $('connect').disabled = false; status('Disconnected'); update(); }
  };
  $('connect-form').onsubmit = (event) => {
    event.preventDefault();
    if (!navigator.usb) { status('WebUSB unavailable'); return; }
    exporter = new Exporter(navigator.usb, update);
    socket = new WebSocket(`${location.protocol === 'https:' ? 'wss:' : 'ws:'}//${location.host}/usb-proxy/ws`);
    const active = socket;
    $('connect').disabled = true; status('Connecting');
    socket.onopen = () => { active.send(JSON.stringify({ type: 'hello', version: 1, role: 'exporter', token: $('token').value })); };
    exporter.onreply = (frame) => { if (active.readyState === WebSocket.OPEN) active.send(JSON.stringify(frame)); update(); };
    socket.onmessage = async ({ data }) => {
      const frame = JSON.parse(data);
      if (frame.type === 'ready' && frame.version === 1) {
        status(`Connected · ${frame.id}`);
        for (const id of ['add', 'refresh', 'disconnect']) $(id).disabled = false;
        await inventory();
      } else if (frame.type === 'request') exporter.request(frame);
    };
    socket.onclose = () => { if (socket === active) disconnected(); };
    socket.onerror = () => status('Connection failed');
  };
  $('add').onclick = async () => {
    try { await navigator.usb.requestDevice({ filters: [] }); await inventory(); }
    catch (error) { if (error.name !== 'NotFoundError') status(error.message); }
  };
  $('refresh').onclick = () => inventory().catch((error) => status(error.message));
  $('disconnect').onclick = disconnected;
  navigator.usb?.addEventListener('connect', () => { if (socket?.readyState === WebSocket.OPEN) inventory(); });
  navigator.usb?.addEventListener('disconnect', () => { if (socket?.readyState === WebSocket.OPEN) inventory(); });
}
