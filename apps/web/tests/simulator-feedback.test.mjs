import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { simulatorIssueURL } from '../src/ui/simulator-feedback.ts';

test('simulator feedback keeps attachment instructions, template and label with optional text', () => {
  const template = readFileSync(new URL('../../../.github/ISSUE_TEMPLATE/simulator.md', import.meta.url), 'utf8');
  for (const description of ['', 'Dial freezes & resets? + idea: a histogram 🦊']) {
    const url = new URL(simulatorIssueURL(description, { event: "feedback" }));
    assert.equal(url.origin + url.pathname, 'https://github.com/justjake/fujimod/issues/new');
    assert.equal(url.searchParams.get('template'), 'simulator.md');
    assert.equal(url.searchParams.get('labels'), 'Simulator');
    const body = url.searchParams.get('body');
    assert(body.includes(description));
    const instruction = template.split('## Simulator report\n\n')[1].trim();
    assert(body.includes(instruction));
  }
});

test('camera and firmware prefill both pathways; only crashes include bug and fault details', () => {
  for (const model of ['X70', 'X-M1', 'X-T5']) {
    const camera = {model, version: '1.10', id: 'camera-110'};
    const fault = {kind:'guest-fault', message:'Execution fault', details:{device:'SDIO', pc:'0x1234', operation:'read', stack:'private long stack'}};
    for (const event of ['feedback', 'crash']) {
      const query = new URL(simulatorIssueURL('My report', {event, camera, fault})).searchParams;
      const labels = query.get('labels').split(',');
      assert(labels.includes(`Camera: ${model}`));
      assert.equal(labels.includes('bug'), event === 'crash');
      assert.equal(query.get('title'), `[Simulator · ${model}] ${event === 'crash' ? 'Crash' : 'Feedback'}`);
      const body = query.get('body');
      assert(body.includes(`Camera: ${model}`));
      assert(body.includes('Firmware: 1.10 (camera-110)'));
      assert.equal(body.includes('SDIO'), event === 'crash');
      assert(!body.includes('private long stack'));
    }
  }
});
