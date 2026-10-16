import type { Engine, CardEntry, CardState } from '../engine/api';

/** Revision-bound downloads from the inserted medium; reads never advance the camera. */
export class CardBrowser {
  private engine?: Engine;
  private stop?: () => void;
  private generation = 0;
  private revision: number | null = null;
  constructor(private section: HTMLElement, private files: HTMLElement, private status: HTMLElement) {}
  connect(engine?: Engine) {
    this.stop?.(); this.engine = engine; this.generation++; this.revision = null;
    this.section.hidden = !engine; this.files.replaceChildren(); this.status.textContent = '';
    if (engine) this.stop = engine.cardChanges(state => { void this.changed(state); });
  }
  private async changed(state: CardState) {
    this.section.hidden = !state.supported;
    if (!state.supported) return;
    this.status.textContent = !state.present ? 'No card inserted' : state.busy ? 'Camera is writing…' : '';
    if (!state.present) { this.files.replaceChildren(); this.revision = null; return; }
    if (state.busy || state.revision === this.revision || state.revision === null) return;
    const revision = state.revision, engine = this.engine!, generation = this.generation;
    try {
      const tree = await engine.cardTree(revision);
      if (generation !== this.generation) return;
      this.revision = tree.revision; this.files.replaceChildren();
      this.files.append(this.tree(tree.entries, revision));
      if (!tree.entries.length) this.status.textContent = 'Empty card';
    } catch { if (generation === this.generation) this.status.textContent = 'Files are changing. Waiting for the camera…'; }
  }
  private tree(entries: CardEntry[], revision: number): HTMLUListElement {
    const list = document.createElement('ul');
    for (const entry of entries) {
      const item = document.createElement('li');
      if (entry.kind === 'directory') {
        const details = document.createElement('details'), summary = document.createElement('summary');
        details.open = true; summary.textContent = entry.name;
        details.append(summary, this.tree(entry.children ?? [], revision)); item.append(details);
      } else {
        const row = document.createElement('div'), name = document.createElement('span'), size = document.createElement('span'), download = document.createElement('button');
        row.className = 'card-file'; name.textContent = entry.name; size.textContent = `${entry.bytes.toLocaleString()} bytes`; download.textContent = 'Download';
        download.setAttribute('aria-label', `Download ${entry.name}`);
        download.onclick = async () => {
          download.disabled = true;
          const generation = this.generation;
          try {
            const bytes = await this.engine!.readCardFile(entry.path, { revision });
            if (generation !== this.generation) return;
            const url = URL.createObjectURL(new Blob([bytes.slice().buffer]));
            const link = document.createElement('a'); link.href = url; link.download = entry.name; link.click();
            window.setTimeout(() => URL.revokeObjectURL(url), 60000);
          } catch { if (generation === this.generation) this.status.textContent = 'This file changed. Try its updated entry.'; }
          finally { download.disabled = false; }
        };
        row.append(name, size, download); item.append(row);
      }
      list.append(item);
    }
    return list;
  }
}
