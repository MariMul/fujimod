import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { simulatorIssueURL } from '../src/ui/simulator-feedback.ts';

const template = event => readFileSync(new URL(`../../../.github/ISSUE_TEMPLATE/simulator-${event}.yml`, import.meta.url), 'utf8');
test('crashes require a ZIP upload; general feedback permits an optional upload', () => {
  for (const event of ['feedback', 'crash']) {
    const form = template(event);
    assert.match(form, /type: upload\n    id: simulator-report/);
    assert.match(form, /accept: ".zip"/);
    assert.match(form, new RegExp(`required: ${event === 'crash'}\\n      accept:`));
    assert(form.includes('Drag and drop the downloaded simulator report ZIP'));
    assert.match(form, /id: mod-build/);
  }
});
test('both pathways prefill separate fields and preserve Unicode and punctuation', () => {
  for (const [model, id] of [['X70', 'x70-110'], ['X-M1', 'xm1-150']]) {
    const camera = {model, id};
    const fault = {kind:'guest-fault', message:'Execution fault', details:{device:'SDIO', pc:'0x1234', stack:'private long stack'}};
    for (const event of ['feedback', 'crash']) {
      for (const description of ['', 'Dial freezes & resets? + idea 🦊']) {
        const modBuild = 'v42 / 2026-10-17T12:00:00Z';
        const query = new URL(simulatorIssueURL(description, {event, camera, fault, modBuild})).searchParams;
        assert.equal(query.get('template'), `simulator-${event}.yml`);
        const labels = query.get('labels').split(',');
        assert(labels.includes(`Camera: ${model}`));
        assert.equal(labels.includes('bug'), event === 'crash');
        assert(!query.has('camera'));
        assert(!template(event).includes('id: camera\n'));
        assert.equal(query.get('firmware'), id);
        assert.equal(query.get('feedback'), description);
        assert.equal(query.get('mod-build'), modBuild);
        assert.equal(query.has('crash'), event === 'crash');
        assert(!query.toString().includes('private+long+stack'));
        assert(!query.has('simulator-report'), 'a report attachment cannot be prefilled');
        for (const field of ['firmware','feedback','mod-build'])
          assert(template(event).includes(`id: ${field}\n`));
      }
    }
  }
});
test('unknown camera and stock firmware remain explicit without asking users to supply metadata', () => {
  const query = new URL(simulatorIssueURL('', {event:'feedback', modBuild:'Stock firmware (no mods)'})).searchParams;
  assert(!query.has('camera'));
  assert.equal(query.get('firmware'), 'Not identified');
  assert.equal(query.get('mod-build'), 'Stock firmware (no mods)');
  assert.equal(query.get('labels'), 'Simulator');
});
