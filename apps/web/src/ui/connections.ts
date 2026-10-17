import type { Connection } from '../engine/api';

/** Connection state and peripheral support are separate observations. */
export class ConnectionsPanel {
  private previous = '';
  constructor(private host: HTMLElement) {}

  update(rows: Connection[]) {
    const key = JSON.stringify(rows);
    if (key === this.previous) return;
    this.previous = key;
    const open = this.host.querySelector('details')?.open ?? false;
    this.host.replaceChildren();
    this.host.hidden = rows.length === 0;
    if (!rows.length) return;
    const details = document.createElement('details');
    details.open = open;
    const summary = document.createElement('summary');
    summary.textContent = 'Connections';
    details.append(summary);
    const list = document.createElement('ul');
    for (const row of rows) {
      const item = document.createElement('li');
      item.dataset.connection = row.id;
      const name = document.createElement('b');
      name.textContent = row.label;
      const status = document.createElement('span');
      const connection = {connected: 'Connected', disconnected: 'Disconnected', unknown: 'Unknown'}[row.connection];
      const support = {modeled: 'Modeled', partial: 'Partly modeled', unmodeled: 'Unmodeled'}[row.support];
      status.textContent = `${connection} · ${support}`;
      const limits = document.createElement('small');
      limits.textContent = row.limits;
      item.append(name, status, limits);
      list.append(item);
    }
    details.append(list);
    this.host.append(details);
  }
}
