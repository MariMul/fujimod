/** Capture the emulator worker, including its native Wasm and generated guest blocks. */
import { chromium, type CDPSession } from "playwright";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { parseArgs } from "node:util";
import { createHash } from "node:crypto";
import { summarizeProfile, type CpuProfile } from "./profile-summary.ts";
import type { EmulatorModule } from "../../../bindings/javascript/emulator/module.ts";

const { values } = parseArgs({
  options: {
    url: { type: "string", default: "http://127.0.0.1:5190/" },
    update: { type: "string" },
    out: { type: "string", default: "build/boot-profile" },
    module: { type: "string" },
    policy: { type: "string" },
    "sample-ticks": { type: "string", default: "0" },
    "no-profile": { type: "boolean", default: false },
  },
});
if (!values.update) throw Error("--update must name a firmware update accepted by the page.");
if (values.policy && !["interactive", "exact"].includes(values.policy))
  throw Error("--policy must be interactive or exact.");
const sampleTicks = Number(values["sample-ticks"]);
if (!Number.isSafeInteger(sampleTicks) || sampleTicks < 0)
  throw Error("--sample-ticks must be a nonnegative integer.");
const output = resolve(values.out);
const runtime: Record<string, unknown> = {};
const hash = (bytes: string | Uint8Array) => createHash("sha256").update(bytes).digest("hex");
await mkdir(output, { recursive: true });

/** Serialized into the generated loader response, only for this diagnostic browser. */
function traceModule(module: EmulatorModule, sampleTicks: number) {
  const emit = (event: string, data: object) =>
    console.debug(
      "emulator-boot-profile " + JSON.stringify({ event, wall: performance.now(), ...data }),
    );
  const configure = module.configure;
  module.configure = (options) => {
    emit("configure", { options });
    return configure(options);
  };
  const prototype = module.Camera.prototype;
  const status = (camera: EmulatorModule["Camera"]["prototype"]) => {
    const current = camera.status();
    const result = { ticks: String(current.ticks), instructions: String(current.instructions) };
    current.free();
    return result;
  };
  const run = prototype.runUntil;
  prototype.runUntil = function (...args) {
    const end = args[0];
    const advance = (tick: bigint) => {
      const step: Parameters<typeof run> = [...args];
      step[0] = tick;
      return run.apply(this, step);
    };
    const start = performance.now();
    emit("run-start", status(this));
    if (sampleTicks) {
      const quantum = BigInt(sampleTicks);
      while (BigInt(status(this).ticks) < end) {
        const next = BigInt(status(this).ticks) + quantum;
        advance(next < end ? next : end);
        emit("run-sample", status(this));
      }
    } else advance(end);
    emit("run-end", { ...status(this), milliseconds: performance.now() - start });
  };
  const boot = prototype.bootToRec;
  prototype.bootToRec = function (...args) {
    const start = performance.now();
    emit("rec-start", status(this));
    const report = boot.apply(this, args);
    emit("rec-end", { ...status(this), milliseconds: performance.now() - start, report });
    emit("jit", module.jitStatistics());
    return report;
  };
}

/** CDP routes worker sessions through the page's Target domain. */
class WorkerSession {
  private serial = 0;
  private pending = new Map<
    number,
    { resolve(value: unknown): void; reject(error: Error): void }
  >();
  private cdp: CDPSession;
  readonly id: string;
  constructor(cdp: CDPSession, id: string) {
    this.cdp = cdp;
    this.id = id;
    cdp.on("Target.receivedMessageFromTarget", (event) => {
      if (event.sessionId !== id) return;
      const message: { id?: number; result?: unknown; error?: { message: string } } = JSON.parse(
        event.message,
      );
      if (message.id === undefined) return;
      const request = this.pending.get(message.id);
      if (!request) return;
      this.pending.delete(message.id);
      if (message.error) request.reject(Error(message.error.message));
      else request.resolve(message.result);
    });
  }
  async send(method: string, params: object = {}): Promise<unknown> {
    const id = ++this.serial;
    const result = new Promise<unknown>((resolve, reject) =>
      this.pending.set(id, { resolve, reject }),
    );
    await this.cdp.send("Target.sendMessageToTarget", {
      sessionId: this.id,
      message: JSON.stringify({ id, method, params }),
    });
    return result;
  }
}

const browser = await chromium.launch();
try {
  const context = await browser.newContext();
  await context.route("**/wasm.json", async (route) => {
    const response = await route.fetch();
    const manifest: unknown = await response.json();
    if (!manifest || typeof manifest !== "object") throw Error("Invalid runtime manifest.");
    const selected = values.policy ? { ...manifest, executionPolicy: values.policy } : manifest;
    for (const key of [
      "construction",
      "executionPolicy",
      "ticksPerSecond",
      "bootTicks",
      "bootInstructions",
    ])
      runtime[key] = Reflect.get(selected, key);
    await route.fulfill({ response, json: selected });
  });
  await context.route("**/camera.mjs", async (route) => {
    const response = await route.fetch();
    const source = values.module
      ? await readFile(join(values.module, "camera.mjs"), "utf8")
      : await response.text();
    runtime.loaderSha256 = hash(source);
    const marker = "export default createEmulator;";
    if (!source.includes(marker))
      throw Error("Generated loader has no createEmulator default export.");
    const instrumented = source.replace(
      marker,
      `export default async function(options) { const module = await createEmulator(options); (${traceModule.toString()})(module, ${sampleTicks}); return module; };`,
    );
    await route.fulfill({ response, body: instrumented, contentType: "text/javascript" });
  });
  await context.route("**/camera.wasm", async (route) => {
    const response = await route.fetch();
    const bytes = values.module
      ? await readFile(join(values.module, "camera.wasm"))
      : await response.body();
    runtime.wasmSha256 = hash(bytes);
    runtime.wasmBytes = bytes.length;
    await route.fulfill({ response, body: bytes, contentType: "application/wasm" });
  });
  const page = await context.newPage();
  const events: unknown[] = [];
  page.on("console", (message) => {
    const prefix = "emulator-boot-profile ";
    if (!message.text().startsWith(prefix)) return;
    const event: unknown = JSON.parse(message.text().slice(prefix.length));
    events.push(event);
    console.log(message.text().slice(prefix.length));
  });
  page.on("pageerror", (error) => console.error(error));
  const cdp = await context.newCDPSession(page);
  let worker: WorkerSession | undefined;
  let attachError: unknown;
  const attached: Promise<void>[] = [];
  cdp.on("Target.attachedToTarget", (event) => {
    attached.push(
      (async () => {
        const session = new WorkerSession(cdp, event.sessionId);
        if (event.targetInfo.type === "worker" && event.targetInfo.url.includes("wasm-worker")) {
          worker = session;
          if (!values["no-profile"]) {
            await session.send("Profiler.enable");
            await session.send("Profiler.setSamplingInterval", { interval: 1000 });
            await session.send("Profiler.start");
          }
        }
        await session.send("Runtime.runIfWaitingForDebugger");
      })().catch((error: unknown) => {
        attachError = error;
      }),
    );
  });
  await cdp.send("Target.setAutoAttach", {
    autoAttach: true,
    waitForDebuggerOnStart: true,
    flatten: false,
  });
  await page.goto(values.url, { waitUntil: "networkidle" });
  await page.locator("input[type=file]").first().setInputFiles(values.update);
  await page.waitForFunction(
    () => performance.getEntriesByName("camera-engine-ready").length > 0,
    null,
    { timeout: 240_000 },
  );
  await Promise.all(attached);
  if (attachError) throw attachError;
  if (!worker) throw Error("No emulator worker attached.");
  if (!values["no-profile"]) {
    const stopped = await worker.send("Profiler.stop");
    if (!stopped || typeof stopped !== "object" || !("profile" in stopped))
      throw Error("CDP returned no worker CPU profile.");
    await writeFile(join(output, "worker.cpuprofile"), JSON.stringify(stopped.profile));
    await writeFile(
      join(output, "summary.json"),
      JSON.stringify(summarizeProfile(stopped.profile as CpuProfile), null, 2) + "\n",
    );
  }
  const marks = await page.evaluate(() =>
    performance.getEntriesByType("mark").map(({ name, startTime }) => ({ name, startTime })),
  );
  await writeFile(
    join(output, "timings.json"),
    JSON.stringify(
      {
        url: values.url,
        policy: values.policy ?? "manifest/default",
        sampleTicks,
        sampled: !values["no-profile"],
        module: values.module,
        runtime,
        marks,
        events,
      },
      null,
      2,
    ) + "\n",
  );
  console.log(JSON.stringify({ output, marks }));
} finally {
  await browser.close();
}
