import type { Engine, CardEntry, CardState } from "../engine/api";
import { zipFiles } from "../archive";
import { icon } from "./icons";

export function downloadBlob(blob: Blob, name: string) {
  if (import.meta.env.DEV) window.dispatchEvent(new CustomEvent('simulator-download', {
    detail: { name, bytes: blob.size },
  }));
  const url = URL.createObjectURL(blob),
    a = document.createElement("a");
  a.href = url;
  a.download = name;
  document.body.append(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 60000);
}

/** Revision-bound card observations and downloads; no guest-side file edits. */
export class CardBrowser {
  private engine?: Engine;
  private stop?: () => void;
  private generation = 0;
  private revision: number | null = null;
  private entries: CardEntry[] = [];
  constructor(
    private host: HTMLElement,
    private files: HTMLElement,
    private status: HTMLElement,
    present: (value: boolean) => Promise<void>,
    format: () => Promise<void>,
  ) {
    for (const button of host.querySelectorAll<HTMLButtonElement>(
      "[data-card-present]",
    ))
      button.onclick = () => {
        void present(button.dataset.cardPresent === "true").catch((error) =>
          this.problem(error),
        );
      };
    host.querySelector<HTMLButtonElement>("#card-download")!.onclick = () => {
      void this.all().catch((error) => this.problem(error));
    };
    host.querySelector<HTMLButtonElement>("#card-format")!.onclick = () => {
      void format().catch((error) => this.problem(error));
    };
  }
  connect(engine?: Engine) {
    this.stop?.();
    this.engine = engine;
    this.generation++;
    this.revision = null;
    this.entries = [];
    this.files.replaceChildren();
    this.status.textContent = "";
    for (const button of this.host.querySelectorAll<HTMLButtonElement>(
      "button",
    ))
      button.disabled = !engine;
    if (engine)
      this.stop = engine.cardChanges((state) => {
        void this.changed(state);
      });
  }
  private problem(error: unknown) {
    this.status.textContent = String(error);
  }
  private async changed(state: CardState) {
    this.host.hidden = !state.supported;
    if (!state.supported) return;
    const capacity = state.capacity_bytes / 1024 ** 3;
    this.host.querySelector("#card-capacity")!.textContent =
      `FAT32 · ${Number(capacity.toFixed(1))} GB`;
    for (const button of this.host.querySelectorAll<HTMLButtonElement>(
      "[data-card-present]",
    )) {
      const selected =
        (button.dataset.cardPresent === "true") === state.present;
      button.classList.toggle("on", selected);
      button.setAttribute("aria-pressed", String(selected));
      button.disabled = state.busy;
    }
    this.status.textContent = state.busy ? "Camera is writing…" : "";
    if (!state.present) {
      this.files.replaceChildren();
      this.revision = null;
      return;
    }
    if (
      state.busy ||
      state.revision === this.revision ||
      state.revision === null
    )
      return;
    const generation = this.generation;
    try {
      const tree = await this.engine!.cardTree(state.revision);
      if (generation !== this.generation) return;
      if (tree.revision === null) throw Error("Card revision is unavailable");
      this.revision = tree.revision;
      this.entries = tree.entries;
      this.files.replaceChildren(this.tree(tree.entries, tree.revision));
    } catch (error) {
      if (generation === this.generation) this.problem(error);
    }
  }
  private tree(entries: CardEntry[], revision: number): DocumentFragment {
    const fragment = document.createDocumentFragment();
    for (const entry of entries) {
      const row = document.createElement("div");
      row.className = "r";
      const twist = document.createElement("span");
      twist.className = "tw";
      const name = document.createElement("span");
      name.className = "n";
      name.textContent = entry.name;
      if (entry.kind === "directory") {
        row.classList.add("folder");
        const button = document.createElement("button");
        button.type = "button";
        button.setAttribute("aria-expanded", "true");
        button.setAttribute("aria-label", entry.name);
        twist.append(icon("chevron", 13));
        button.append(twist, icon("folder"), name);
        row.append(button);
        const children = document.createElement("div");
        children.className = "kids";
        children.append(this.tree(entry.children ?? [], revision));
        button.onclick = () => {
          children.hidden = !children.hidden;
          twist.classList.toggle("closed", children.hidden);
          button.setAttribute("aria-expanded", String(!children.hidden));
        };
        fragment.append(row, children);
      } else {
        const size = document.createElement("span");
        size.className = "s";
        size.textContent =
          entry.bytes >= 1024 * 1024
            ? `${(entry.bytes / (1024 * 1024)).toFixed(1)} MB`
            : entry.bytes >= 1024
              ? `${Number((entry.bytes / 1024).toFixed(1))} KB`
              : `${entry.bytes} B`;
        const button = document.createElement("button");
        button.className = "dl";
        button.setAttribute("aria-label", `Download ${entry.name}`);
        button.append(icon("download", 15));
        button.onclick = async () => {
          button.disabled = true;
          const generation = this.generation;
          try {
            const bytes = await this.engine!.readCardFile(entry.path, {
              revision,
            });
            if (generation === this.generation)
              downloadBlob(new Blob([bytes.slice().buffer]), entry.name);
          } catch (error) {
            this.problem(error);
          } finally {
            button.disabled = false;
          }
        };
        row.append(
          twist,
          icon(/\.(jpe?g|png)$/i.test(entry.name) ? "image" : "file"),
          name,
          size,
          button,
        );
        fragment.append(row);
      }
    }
    return fragment;
  }
  private async all() {
    if (!this.engine || this.revision === null) return;
    const engine = this.engine,
      revision = this.revision,
      generation = this.generation,
      entries: CardEntry[] = [];
    const visit = (items: CardEntry[]) => {
      for (const item of items)
        if (item.kind === "directory") visit(item.children ?? []);
        else entries.push(item);
    };
    visit(this.entries);
    if (entries.reduce((sum, item) => sum + item.bytes, 0) > 64 * 1024 * 1024)
      throw Error("Card download exceeds the 64 MiB browser archive limit.");
    const files: [string, Uint8Array<ArrayBuffer>][] = [];
    for (const entry of entries) {
      const bytes = await engine.readCardFile(entry.path, { revision });
      files.push([
        entry.path.replace(/^[A-Za-z]:[\\/]/, "").replace(/^\//, ""),
        bytes.slice(),
      ]);
    }
    if (generation === this.generation)
      downloadBlob(zipFiles(files), "sd-card.zip");
  }
}
