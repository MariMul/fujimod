import type { EngineFault } from "../engine/fault";
import { icon } from "./icons";
export interface FaultContext {
  tick?: number;
  firmware?: string;
  build: string;
}
export class FaultPanel {
  constructor(private host: HTMLElement) {
    host.querySelector("h4")!.prepend(icon("alert"), " ");
    host.querySelector("#error-report")!.prepend(icon("download", 14), " ");
  }
  show(fault: EngineFault, context: FaultContext) {
    this.host.hidden = false;
    this.host.querySelector("#fault-message")!.textContent =
      fault.kind === "unsupported-device"
        ? "We don't know how to simulate this camera hardware feature yet. It works fine on a real camera."
        : fault.message;
    const d = fault.details;
    const kind = String(d.type ?? d.error_type ?? fault.kind);
    const access = [
      d.operation,
      d.address,
      d.value == null ? "" : `← ${d.value}`,
      d.device ? `(${d.device})` : "",
    ]
      .filter(Boolean)
      .join(" ");
    const raw = String(d.error ?? d.message ?? "");
    const last = raw.trim().split("\n").at(-1) ?? "";
    const ua = navigator.userAgent,
      browser =
        ua.match(/(?:Chrome|Firefox)\/[\d.]+/)?.[0] ??
        `Safari ${ua.match(/Version\/([\d.]+)/)?.[1] ?? ""}`;
    const os =
      ua.match(/iPhone OS ([\d_]+)/)?.[1]?.replaceAll("_", ".") ??
      ua.match(/Mac OS X ([\d_]+)/)?.[1]?.replaceAll("_", ".") ??
      navigator.platform;
    const lines = [
      `${kind}: ${access || last}`,
      `pc ${d.pc ?? "unknown"} · core ${d.cpu ?? "unknown"} · tick ${context.tick ?? d.tick ?? "unknown"} · firmware ${context.firmware ?? "unknown"} · build ${context.build} · ${browser} / ${os}`,
    ];
    this.host.querySelector("#fault-raw")!.textContent = lines.join("\n");
  }
  hide() {
    this.host.hidden = true;
  }
}
