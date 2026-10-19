import { For, Show, createSignal, createEffect, untrack } from 'solid-js';
import type { Engine, CardEntry, CardState } from '../engine/api';
import { zipFiles } from '../archive';
import { Icon } from './icons';
import { downloadBlob } from './download';

function CardEntryRow(props: { entry: CardEntry; download: (entry: CardEntry) => Promise<void> }) {
  const [open, setOpen] = createSignal(true),
    [busy, setBusy] = createSignal(false);
  const size = () =>
    props.entry.bytes >= 1024 ** 2
      ? `${(props.entry.bytes / 1024 ** 2).toFixed(1)} MB`
      : props.entry.bytes >= 1024
        ? `${Number((props.entry.bytes / 1024).toFixed(1))} KB`
        : `${props.entry.bytes} B`;
  return (
    <Show
      when={props.entry.kind === 'directory'}
      fallback={
        <div class="r">
          <span class="tw" />
          <Icon name={/\.(jpe?g|png)$/i.test(props.entry.name) ? 'image' : 'file'} />
          <span class="n">{props.entry.name}</span>
          <span class="s">{size()}</span>
          <button
            class="dl"
            aria-label={`Download ${props.entry.name}`}
            disabled={busy()}
            onClick={async () => {
              setBusy(true);
              try {
                await props.download(props.entry);
              } finally {
                setBusy(false);
              }
            }}
          >
            <Icon name="download" size={15} />
          </button>
        </div>
      }
    >
      <div class="r folder">
        <button
          type="button"
          aria-expanded={open() ? 'true' : 'false'}
          aria-label={props.entry.name}
          onClick={() => setOpen((value) => !value)}
        >
          <span class={['tw', { closed: !open() }]}>
            <Icon name="chevron" size={13} />
          </span>
          <Icon name="folder" />
          <span class="n">{props.entry.name}</span>
        </button>
      </div>
      <div class="kids" hidden={!open()}>
        <For each={props.entry.children ?? []} keyed={(entry) => entry.path}>
          {(entry) => <CardEntryRow entry={entry()} download={props.download} />}
        </For>
      </div>
    </Show>
  );
}

/** All observations/downloads are bound to the engine and the card revision that supplied them. */
export function CardBrowser(props: {
  engine?: Engine;
  present: (value: boolean) => Promise<void>;
  format: () => Promise<void>;
}) {
  const [state, setState] = createSignal<CardState>();
  const [tree, setTree] = createSignal<{ revision: number; entries: CardEntry[] }>();
  const [status, setStatus] = createSignal('');
  let generation = 0,
    observation = 0;
  createEffect(
    () => props.engine,
    (engine) => {
      const current = ++generation;
      setState(undefined);
      setTree(undefined);
      setStatus('');
      let previous: CardState | undefined, pendingRevision: number | undefined;
      const stop = engine?.cardChanges((state) => {
        if (
          state.supported !== previous?.supported ||
          state.present !== previous?.present ||
          state.revision !== previous?.revision
        )
          ++observation;
        previous = state;
        const observed = observation;
        setState(state);
        setStatus(state.busy ? 'Camera is writing…' : '');
        if (!state.supported || !state.present) {
          setTree(undefined);
          return;
        }
        if (
          state.busy ||
          state.revision === null ||
          state.revision === untrack(tree)?.revision ||
          state.revision === pendingRevision
        )
          return;
        const revision = state.revision;
        pendingRevision = revision;
        void engine
          .cardTree(revision)
          .then((value) => {
            if (current !== generation || observed !== observation) return;
            if (value.revision !== revision) throw Error('Card revision is unavailable');
            setTree({ revision: value.revision, entries: value.entries });
          })
          .catch((error) => {
            if (current === generation && observed === observation) setStatus(String(error));
          })
          .finally(() => {
            if (pendingRevision === revision) pendingRevision = undefined;
          });
      });
      return () => {
        ++generation;
        stop?.();
      };
    },
  );
  async function perform(task: () => Promise<void>) {
    const current = generation;
    try {
      await task();
    } catch (error) {
      if (current === generation) setStatus(String(error));
    }
  }
  async function download(entry: CardEntry) {
    const engine = props.engine,
      current = generation,
      revision = tree()?.revision;
    if (!engine || revision === undefined) return;
    await perform(async () => {
      const bytes = await engine.readCardFile(entry.path, { revision });
      if (current === generation) downloadBlob(new Blob([bytes.slice().buffer]), entry.name);
    });
  }
  async function all() {
    const engine = props.engine,
      current = generation,
      listing = tree();
    if (!engine || !listing) return;
    await perform(async () => {
      const entries: CardEntry[] = [];
      const visit = (items: CardEntry[]) => {
        for (const item of items)
          if (item.kind === 'directory') visit(item.children ?? []);
          else entries.push(item);
      };
      visit(listing.entries);
      if (entries.reduce((sum, item) => sum + item.bytes, 0) > 64 * 1024 ** 2)
        throw Error('Card download exceeds the 64 MiB browser archive limit.');
      const files: [string, Uint8Array<ArrayBuffer>][] = [];
      for (const entry of entries) {
        const bytes = await engine.readCardFile(entry.path, { revision: listing.revision });
        if (current !== generation) return;
        files.push([entry.path.replace(/^[A-Za-z]:[\\/]/, '').replace(/^\//, ''), bytes.slice()]);
      }
      downloadBlob(zipFiles(files), 'sd-card.zip');
    });
  }
  return (
    <div class="sd" id="card-section" aria-label="SD card" hidden={state()?.supported === false}>
      <div class="head" id="card-header">
        <b>SD CARD</b>
        <span id="card-capacity">
          {state()
            ? `FAT32 · ${Number((state()!.capacity_bytes / 1024 ** 3).toFixed(1))} GB`
            : 'FAT32'}
        </span>
        <div class="seg" role="group" aria-label="SD card inserted">
          <For each={[false, true]}>
            {(present) => (
              <button
                data-card-present={String(present)}
                class={{ on: (state()?.present ?? true) === present }}
                aria-pressed={(state()?.present ?? true) === present ? 'true' : 'false'}
                disabled={!state()?.supported || state()?.busy}
                onClick={() => void perform(() => props.present(present))}
              >
                {present ? 'ON' : 'OFF'}
              </button>
            )}
          </For>
        </div>
      </div>
      <div class="tree" id="card-files">
        <For each={tree()?.entries ?? []} keyed={(entry) => entry.path}>
          {(entry) => <CardEntryRow entry={entry()} download={download} />}
        </For>
      </div>
      <div class="row" style={{ 'margin-top': '10px' }}>
        <button
          class="pill"
          id="card-download"
          disabled={!state()?.supported}
          onClick={() => void all()}
        >
          Download all
        </button>
        <button
          class="pill"
          id="card-format"
          disabled={!state()?.supported}
          onClick={() => void perform(props.format)}
        >
          Format
        </button>
      </div>
      <span class="visually-hidden" id="card-status" role="status">
        {status()}
      </span>
    </div>
  );
}
