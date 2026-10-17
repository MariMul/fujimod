// Browser-local data only. Keys bind the user's DAT, runtime bytes and storage inputs.
const DB = 'fujimod-checkpoints-v1';
const MAX_BYTES = 128 * 1024 * 1024;
export async function digest(data) {
  return Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', data)), b => b.toString(16).padStart(2, '0')).join('');
}
export async function cacheKey(datHash, bundleVersion, storageProfileHash) {
  return digest(new TextEncoder().encode(JSON.stringify([1, datHash, bundleVersion, storageProfileHash])));
}
async function directory() { return (await navigator.storage.getDirectory()).getDirectoryHandle(DB, { create: true }); }
async function database() {
  return new Promise((resolve, reject) => {
    const open = indexedDB.open(DB, 1);
    open.onupgradeneeded = () => open.result.createObjectStore('checkpoints');
    open.onsuccess = () => resolve(open.result); open.onerror = () => reject(open.error);
  });
}
async function indexed(action, key, value) {
  const db = await database();
  try {
    return await new Promise((resolve, reject) => {
      const tx = db.transaction('checkpoints', action === 'get' ? 'readonly' : 'readwrite');
      const request = value ? tx.objectStore('checkpoints')[action](value, key) : tx.objectStore('checkpoints')[action](key);
      tx.oncomplete = () => resolve(request.result); tx.onerror = () => reject(tx.error); tx.onabort = () => reject(tx.error);
    });
  } finally { db.close(); }
}
async function pack(bytes) {
  const header = new TextEncoder().encode(JSON.stringify({ schema: 1, bytes: bytes.byteLength, sha256: await digest(bytes) }) + '\n');
  return new Blob([header, bytes]);
}
async function unpack(blob) {
  if (!(blob instanceof Blob) || blob.size > MAX_BYTES) throw Error('Invalid checkpoint size');
  const head = new Uint8Array(await blob.slice(0, 256).arrayBuffer()), end = head.indexOf(10);
  if (end < 0) throw Error('Invalid checkpoint envelope');
  const header = JSON.parse(new TextDecoder().decode(head.subarray(0, end)));
  const bytes = await blob.slice(end + 1).arrayBuffer();
  if (header.schema !== 1 || header.bytes !== bytes.byteLength || await digest(bytes) !== header.sha256) throw Error('Checkpoint integrity differs');
  return bytes;
}
export class CheckpointStore {
  backend = 'unavailable';
  async remove(key) {
    await Promise.allSettled([
      directory().then(dir => dir.removeEntry(key)), indexed('delete', key),
    ]);
  }
  async read(key) {
    try {
      const file = await (await directory()).getFileHandle(key);
      const bytes = await unpack(await file.getFile()); this.backend = 'opfs'; return bytes;
    } catch { /* Unavailable, absent or corrupt: try the transactional fallback. */ }
    try {
      const bytes = await unpack(new Blob([await indexed('get', key)])); this.backend = 'indexeddb'; return bytes;
    } catch { return undefined; }
  }
  async write(key, bytes) {
    if (bytes.byteLength > MAX_BYTES) return false;
    const blob = await pack(bytes);
    try {
      const file = await (await directory()).getFileHandle(key, { create: true });
      const writer = await file.createWritable();
      try { await writer.write(blob); await writer.close(); }
      catch (error) { await writer.abort().catch(() => {}); throw error; }
      this.backend = 'opfs'; return true;
    } catch { /* OPFS is optional; IndexedDB commits the complete blob atomically. */ }
    // ArrayBuffer has consistent structured-clone support in Safari workers.
    try { await indexed('put', key, await blob.arrayBuffer()); this.backend = 'indexeddb'; return true; }
    catch { return false; }
  }
}
