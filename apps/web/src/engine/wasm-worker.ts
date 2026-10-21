import { WasmWorkerRuntime } from './wasm-worker-runtime';
import type { WasmWorkerRequest } from './wasm-worker-protocol';
const scope = self as unknown as DedicatedWorkerGlobalScope;
const runtime = new WasmWorkerRuntime((event, transfer = []) => scope.postMessage(event, transfer));
scope.onmessage = (event: MessageEvent<WasmWorkerRequest>) => runtime.receive(event.data);
