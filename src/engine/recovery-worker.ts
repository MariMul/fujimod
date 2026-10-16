import { cacheKey, CheckpointStore, digest } from './checkpoint-store.js';

export interface RecoveryRuntime {
  globals: { set(name: string, value: unknown): void; delete(name: string): void };
  runPython<T = string>(source: string): T;
  toPy(value: Uint8Array): { destroy(): void };
}
interface BytesProxy { getBuffer(kind: string): { data: Uint8Array; release(): void }; destroy(): void }
/** This owns persistence and history only; execution/profiling remain in the worker. */
export class WorkerRecovery {
  private store = new CheckpointStore();
  private key = '';
  private saved = false;
  private cacheFailure?: string;
  private restoreSeconds = 0;
  mode = 'cold';
  private persistenceDisabled = false;
  bypassPersistence() { this.persistenceDisabled = true; this.saved = false; this.mode = 'cold'; }
  async identify(dat: ArrayBuffer, sources: ArrayBuffer, native: ArrayBuffer, inputs: ArrayBuffer, protocol: unknown, seeds: { path: string; bytes: ArrayBuffer }[] = []) {
    const version = await digest(new TextEncoder().encode(JSON.stringify([await digest(sources), await digest(native), protocol, 'recovery-v1'])));
    const profile = await digest(new TextEncoder().encode(JSON.stringify([await digest(inputs), await Promise.all(seeds.map(async file => [file.path, await digest(file.bytes)]))])));
    this.key = await cacheKey(await digest(dat), version, profile);
  }
  async restore(py: RecoveryRuntime, restart = false) {
    if (this.persistenceDisabled) return false;
    if (restart) { await this.store.remove(this.key); return false; }
    const started = performance.now();
    const bytes = await this.store.read(this.key);
    if (!bytes) return false;
    const proxy = py.toPy(new Uint8Array(bytes)); py.globals.set('browser_checkpoint', proxy);
    try {
      py.runPython('from native.web.recovery import restore_initial\nbrowser_restore_timings = []\nsession = restore_initial(bytes(browser_checkpoint), timings=browser_restore_timings, clock_seconds_2000=clock_seconds_2000)');
      this.restoreSeconds = (performance.now()-started)/1000;
      this.mode = 'resume'; this.saved = true; return true;
    } catch (error) {
      this.cacheFailure = String(error); await this.store.remove(this.key); return false;
    } finally { py.globals.delete('browser_checkpoint'); proxy.destroy(); }
  }
  start(py: RecoveryRuntime) {
    py.runPython('from native.web.recovery import History, export_initial\nbrowser_history = History()');
  }
  async capture(py: RecoveryRuntime) {
    // History checks the two-second deadline and refuses unsafe boundaries without execution.
    let value: BytesProxy | undefined;
    try {
      const captured = py.runPython<boolean>('browser_history.capture(session)');
      if (!captured || this.saved || this.persistenceDisabled) return;
      value = py.runPython<BytesProxy>('memoryview(export_initial(session))');
      const buffer = value.getBuffer('u8');
      let bytes: Uint8Array<ArrayBuffer>;
      try { bytes = Uint8Array.from(buffer.data); } finally { buffer.release(); }
      this.saved = await this.store.write(this.key, bytes);
      return this.saved;
    } catch (error) { this.cacheFailure = String(error); }
    finally { value?.destroy(); }
  }
  rollback(py: RecoveryRuntime, seconds: number) {
    py.globals.set('browser_undo_seconds', seconds);
    py.runPython('browser_history.rollback(session, browser_undo_seconds)');
  }
  status(py: RecoveryRuntime) {
    return { mode: this.mode, backend: this.store.backend, saved: this.saved, cacheFailure: this.cacheFailure,
      restoreSeconds: this.restoreSeconds, restoreSteps: this.mode === 'resume' ? JSON.parse(py.runPython('json.dumps(browser_restore_timings)')) : [],
      history: JSON.parse(py.runPython('json.dumps(browser_history.status(session))')) };
  }
}
