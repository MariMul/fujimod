import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { simulatorIssueURL } from '../src/ui/simulator-feedback.ts';

test('simulator feedback keeps attachment instructions, template and label with optional text', () => {
  const template = readFileSync(new URL('../../../.github/ISSUE_TEMPLATE/simulator.md', import.meta.url), 'utf8');
  for (const description of ['', 'Dial freezes & resets? + idea: a histogram 🦊']) {
    const url = new URL(simulatorIssueURL(description));
    assert.equal(url.origin + url.pathname, 'https://github.com/justjake/fujimod/issues/new');
    assert.equal(url.searchParams.get('template'), 'simulator.md');
    assert.equal(url.searchParams.get('labels'), 'Simulator');
    const body = url.searchParams.get('body');
    assert(body.includes(description));
    const instruction = template.split('## Simulator report\n\n')[1].trim();
    assert(body.includes(instruction));
  }
});
