import {
  writeDurable,
  readDurable,
  removeDurable,
  type DurableReference,
} from '../storage/durable';
/** Owned cameras are independent of the disposable firmware boot cache. */
export interface CameraRecord {
  id: string;
  name: string;
  firmwareId: string;
  firmwareHash: string;
  fileName: string;
  createdAt: number;
  savedAt?: number;
  poweredOn?: boolean;
  revision: number;
  modEnabled: boolean;
}
export interface CameraSave {
  durable: ArrayBuffer;
  runtimeKey: string;
  poweredOn: boolean;
  positions: Record<string, string>;
  modEnabled: boolean;
}
interface StoredSave extends Omit<CameraSave, 'durable'> {
  schema: 1;
  durable: DurableReference;
}
const DB = 'fujimod-cameras';
async function database() {
  return new Promise<IDBDatabase>((resolve, reject) => {
    const request = indexedDB.open(DB, 1);
    request.onupgradeneeded = () => {
      request.result.createObjectStore('cameras', { keyPath: 'id' });
      request.result.createObjectStore('firmware');
      request.result.createObjectStore('saves');
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}
async function transaction<T>(
  stores: string[],
  mode: IDBTransactionMode,
  run: (tx: IDBTransaction, result: (value: T) => void, fail: (error: unknown) => void) => void,
): Promise<T> {
  for (let attempt = 0; ; attempt++) {
    try {
      return await transactionAttempt(stores, mode, run);
    } catch (error) {
      // WebKit can transiently abort an initial open/read. Reopen once for
      // read-only operations; never replay a write or hide persistent failures.
      if (
        mode !== 'readonly' ||
        attempt > 0 ||
        !(error instanceof DOMException) ||
        error.name !== 'AbortError'
      )
        throw error;
    }
  }
}
async function transactionAttempt<T>(
  stores: string[],
  mode: IDBTransactionMode,
  run: (tx: IDBTransaction, result: (value: T) => void, fail: (error: unknown) => void) => void,
): Promise<T> {
  const db = await database();
  try {
    return await new Promise<T>((resolve, reject) => {
      const tx = db.transaction(stores, mode);
      let value: T;
      let failure: DOMException | null = null;
      tx.oncomplete = () => resolve(value);
      tx.onerror = (event) => {
        failure = (event.target as IDBRequest).error;
      };
      tx.onabort = () => reject(tx.error ?? failure ?? Error('Camera storage transaction failed.'));
      const fail = (error: unknown) => {
        try {
          tx.abort();
        } catch {
          // A browser may already have aborted the transaction.
        }
        reject(error);
      };
      try {
        run(
          tx,
          (result) => {
            value = result;
          },
          fail,
        );
      } catch (error) {
        fail(error);
      }
    });
  } finally {
    db.close();
  }
}
export class CameraStore {
  list(): Promise<CameraRecord[]> {
    return transaction(['cameras'], 'readonly', (tx, result) => {
      const request = tx.objectStore('cameras').getAll();
      request.onsuccess = () =>
        result((request.result as CameraRecord[]).sort((a, b) => b.createdAt - a.createdAt));
    });
  }
  async create(
    file: File,
    firmware: { id: string; model: string; version: string; sha256: string },
    modEnabled: boolean,
  ): Promise<CameraRecord> {
    // Store bytes rather than File/Blob handles: WebKit can fail their IndexedDB clone.
    const bytes = await file.arrayBuffer();
    const record: CameraRecord = {
      id: crypto.randomUUID(),
      name: `${firmware.model} · ${firmware.version}`,
      firmwareId: firmware.id,
      firmwareHash: firmware.sha256,
      fileName: file.name,
      createdAt: Date.now(),
      revision: 0,
      modEnabled,
    };
    await transaction(['cameras', 'firmware'], 'readwrite', (tx, result) => {
      tx.objectStore('cameras').add(record);
      // A copy per camera keeps ownership/deletion independent, even for the same DAT.
      tx.objectStore('firmware').add(bytes, record.id);
      result(undefined);
    });
    return record;
  }
  firmware(id: string): Promise<Blob> {
    return transaction(['firmware'], 'readonly', (tx, result) => {
      const request = tx.objectStore('firmware').get(id);
      request.onsuccess = () => result(request.result);
    }).then((value) => {
      if (!(value instanceof ArrayBuffer) && !(value instanceof Blob))
        throw Error('This camera’s firmware is missing from browser storage.');
      return new Blob([value]);
    });
  }
  async saved(id: string): Promise<CameraSave | undefined> {
    for (let attempt = 0; ; attempt++) {
      const saved = await transaction<StoredSave | undefined>(
        ['saves'],
        'readonly',
        (tx, result) => {
          const request = tx.objectStore('saves').get(id);
          request.onsuccess = () => result(request.result);
        },
      );
      if (!saved) return undefined;
      if (saved.schema !== 1) throw Error('Unknown durable camera save format.');
      try {
        const { schema: _, durable, ...metadata } = saved;
        return { ...metadata, durable: await readDurable(durable) };
      } catch (error) {
        if (attempt === 0 && error instanceof DOMException && error.name === 'NotFoundError')
          continue;
        throw error;
      }
    }
  }
  /** A closed immutable file becomes current in the same optimistic metadata transaction. */
  async save(id: string, revision: number, save: CameraSave): Promise<CameraRecord> {
    const durable = await writeDurable(save.durable);
    let published: { record: CameraRecord; previous?: StoredSave } | undefined;
    try {
      published = await transaction(['cameras', 'saves'], 'readwrite', (tx, result, fail) => {
        const cameras = tx.objectStore('cameras'),
          saves = tx.objectStore('saves');
        const request = cameras.get(id);
        request.onsuccess = () => {
          const previous = request.result as CameraRecord | undefined;
          if (!previous || previous.revision !== revision) {
            result(undefined);
            return;
          }
          const old = saves.get(id);
          old.onsuccess = () => {
            try {
              const record = {
                ...previous,
                savedAt: Date.now(),
                poweredOn: save.poweredOn,
                modEnabled: save.modEnabled,
                revision: revision + 1,
              };
              saves.put({ ...save, schema: 1, durable } satisfies StoredSave, id);
              cameras.put(record);
              result({ record, previous: old.result });
            } catch (error) {
              fail(error);
            }
          };
        };
      });
      if (!published) throw Error('This camera changed in another tab. Reload it before saving.');
    } catch (error) {
      await removeDurable(durable).catch(() => {});
      throw error;
    }
    if (published.previous?.schema === 1)
      await removeDurable(published.previous.durable).catch(() => {});
    return published.record;
  }
}
