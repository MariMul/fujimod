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
  snapshot?: ArrayBuffer;
  runtimeKey: string;
  poweredOn: boolean;
  positions: Record<string, string>;
  modEnabled: boolean;
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
  saved(id: string): Promise<CameraSave | undefined> {
    return transaction(['saves'], 'readonly', (tx, result) => {
      const request = tx.objectStore('saves').get(id);
      request.onsuccess = () => result(request.result);
    });
  }
  /** One transaction publishes storage and metadata together. Stale tabs cannot overwrite a newer save. */
  save(id: string, revision: number, save: CameraSave): Promise<CameraRecord> {
    return transaction<CameraRecord | undefined>(
      ['cameras', 'saves'],
      'readwrite',
      (tx, result, fail) => {
        const request = tx.objectStore('cameras').get(id);
        request.onsuccess = () => {
          try {
            const previous = request.result as CameraRecord | undefined;
            if (!previous || previous.revision !== revision) {
              result(undefined);
              return;
            }
            const record = {
              ...previous,
              savedAt: Date.now(),
              poweredOn: save.poweredOn,
              modEnabled: save.modEnabled,
              revision: revision + 1,
            };
            tx.objectStore('saves').put(save, id);
            tx.objectStore('cameras').put(record);
            result(record);
          } catch (error) {
            // IDB request callbacks run after transaction()'s synchronous body.
            fail(error);
          }
        };
      },
    ).then((record) => {
      if (!record) throw Error('This camera changed in another tab. Reload it before saving.');
      return record;
    });
  }
}
