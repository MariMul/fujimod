import { cacheKey, CheckpointStore, digest } from './checkpoint-store.js';

export interface RecoveryRuntime {
  globals: { set(name: string, value: unknown): void; delete(name: string): void };
  runPython<T = string>(source: string): T;
  toPy(value: Uint8Array): { destroy(): void };
}
interface BytesProxy {
  getBuffer(kind: string): { data: Uint8Array; release(): void };
  destroy(): void;
}
/** This owns persistence and history only; execution/profiling remain in the worker. */
export class WorkerRecovery {
  private store = new CheckpointStore();
  private key = '';
  private saved = false;
  private saving = false;
  private cacheFailure?: string;
  private restoreSeconds = 0;
  identity: Record<string, string> = {};
  mode = 'cold';
  private persistenceDisabled = false;
  bypassPersistence() {
    this.persistenceDisabled = true;
    this.saved = false;
    this.mode = 'cold';
  }
  async identify(
    dat: ArrayBuffer,
    sources: ArrayBuffer,
    native: ArrayBuffer,
    inputs: ArrayBuffer,
    protocol: unknown,
    seeds: { path: string; sha256: string }[] = [],
  ) {
    const [sourcesSha256, nativeSha256, firmwareSha256] = await Promise.all([
      digest(sources),
      digest(native),
      digest(dat),
    ]);
    const version = await digest(
      new TextEncoder().encode(
        JSON.stringify([sourcesSha256, nativeSha256, protocol, 'recovery-v2']),
      ),
    );
    const profile = await digest(
      new TextEncoder().encode(
        JSON.stringify([await digest(inputs), seeds.map((file) => [file.path, file.sha256])]),
      ),
    );
    this.key = await cacheKey(firmwareSha256, version, profile);
    this.identity = {
      firmwareSha256,
      bundleVersion: version,
      sourcesSha256,
      nativeSha256,
      storageProfileSha256: profile,
    };
  }
  async restore(py: RecoveryRuntime, restart = false) {
    if (this.persistenceDisabled) return false;
    if (restart) {
      await this.store.remove(this.key);
      return false;
    }
    const started = performance.now();
    const bytes = await this.store.read(this.key);
    if (!bytes) return false;
    const proxy = py.toPy(new Uint8Array(bytes));
    py.globals.set('browser_checkpoint', proxy);
    try {
      py.runPython(
        'from native.web.recovery import restore_session\nbrowser_restore_timings = []\nsession = restore_session(bytes(browser_checkpoint), timings=browser_restore_timings, clock_seconds_2000=clock_seconds_2000)',
      );
      this.restoreSeconds = (performance.now() - started) / 1000;
      this.mode = 'resume';
      this.saved = true;
      return true;
    } catch (error) {
      this.cacheFailure = String(error);
      await this.store.remove(this.key);
      return false;
    } finally {
      py.globals.delete('browser_checkpoint');
      proxy.destroy();
    }
  }
  start(py: RecoveryRuntime) {
    py.runPython(
      'from native.web.recovery import History, export_snapshot\nbrowser_history = History()',
    );
  }
  async capture(py: RecoveryRuntime) {
    // History checks the two-second deadline and refuses unsafe boundaries without execution.
    let value: BytesProxy | undefined;
    let ownsSave = false;
    try {
      const captured = py.runPython<boolean>('browser_history.capture(session)');
      if (!captured || this.saved || this.saving || this.persistenceDisabled) return;
      this.saving = ownsSave = true;
      value = py.runPython<BytesProxy>('memoryview(export_snapshot(browser_history.points[-1]))');
      const buffer = value.getBuffer('u8');
      let bytes: Uint8Array<ArrayBuffer>;
      try {
        bytes = Uint8Array.from(buffer.data);
      } finally {
        buffer.release();
      }
      value.destroy();
      value = undefined;
      this.saved = await this.store.write(this.key, bytes);
      if (!this.saved) this.persistenceDisabled = true;
      return this.saved;
    } catch (error) {
      this.cacheFailure = String(error);
      this.persistenceDisabled = true;
      return true;
    } finally {
      value?.destroy();
      if (ownsSave) this.saving = false;
    }
  }
  rollback(py: RecoveryRuntime, seconds: number) {
    py.globals.set('browser_undo_seconds', seconds);
    py.runPython('browser_history.rollback(session, browser_undo_seconds)');
  }
  canRollback(py: RecoveryRuntime | undefined, seconds: number): boolean {
    if (!py) return false;
    try {
      py.globals.set('browser_undo_seconds', seconds);
      return py.runPython<boolean>(
        "browser_history.can_rollback(session, browser_undo_seconds) if globals().get('browser_history') is not None and globals().get('session') is not None else False",
      );
    } catch {
      return false;
    }
  }
  errorSnapshot(py: RecoveryRuntime): {
    checkpoint?: ArrayBuffer;
    snapshot: Record<string, unknown>;
  } {
    let value: BytesProxy | undefined;
    try {
      py.runPython(`from native.web.recovery import error_snapshot
browser_report_data, browser_report_meta = error_snapshot(globals().get('browser_history'), session) if globals().get('session') is not None else (None, {'unavailable': 'No camera session was created'})`);
      const snapshot = JSON.parse(py.runPython<string>('json.dumps(browser_report_meta)'));
      if (!py.runPython<boolean>('browser_report_data is not None')) return { snapshot };
      value = py.runPython<BytesProxy>('memoryview(browser_report_data)');
      const buffer = value.getBuffer('u8');
      try {
        return { checkpoint: Uint8Array.from(buffer.data).buffer, snapshot };
      } finally {
        buffer.release();
      }
    } catch (error) {
      return { snapshot: { unavailable: String(error) } };
    } finally {
      value?.destroy();
      py.globals.delete('browser_report_data');
      py.globals.delete('browser_report_meta');
    }
  }
  status(py: RecoveryRuntime) {
    return {
      mode: this.mode,
      backend: this.store.backend,
      saved: this.saved,
      cacheFailure: this.cacheFailure,
      restoreSeconds: this.restoreSeconds,
      restoreSteps:
        this.mode === 'resume'
          ? JSON.parse(py.runPython('json.dumps(browser_restore_timings)'))
          : [],
      history: JSON.parse(py.runPython('json.dumps(browser_history.status(session))')),
    };
  }
}
