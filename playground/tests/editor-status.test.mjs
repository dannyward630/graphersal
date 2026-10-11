// Node tests of the Query panel's status bar texts (js/editor/editor-status.js) and of the parent
// of the floating editor tooltips (js/editor/tooltip-parent.js).
// Run: node --test playground/tests/*.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { runStatusText, cursorTexts } from '../js/editor/editor-status.js';
import { tooltipParent } from '../js/editor/tooltip-parent.js';

const ok = (extra = {}) => ({ run_id: 3, ok: true, elapsed_ms: 1.25, display: { total_rows: 6 }, execution: { profile: {} }, ...extra });

test('the status of a tab that never ran is Ready', () => {
  assert.equal(runStatusText({ result: null }), 'Ready');
  assert.equal(runStatusText({ result: null, elsewhere: 'Tab 2', elapsedMs: 2000 }), 'Ready · running in "Tab 2" · 2.00 s');
});

test('a finished run shows its number, rows and time, never whether it was profiled', () => {
  assert.equal(runStatusText({ result: ok() }), 'Run #3 · 6 results · 1.3 ms');
  assert.equal(runStatusText({ result: ok({ display: { total_rows: 1 } }) }), 'Run #3 · 1 result · 1.3 ms');
  assert.equal(runStatusText({ result: ok({ display: null }) }), 'Run #3 · done · 1.3 ms');
  assert.equal(runStatusText({ result: ok(), stale: true }), 'Run #3 · 6 results · 1.3 ms · possibly stale');
});

test('a failed and a stopped run', () => {
  assert.equal(runStatusText({ result: ok({ ok: false }) }), 'Run #3 · error · 1.3 ms');
  assert.equal(runStatusText({ result: ok({ stopped: true }), stale: true }), 'Run #3 · stopped');
});

test('a run of the selection says so after the run number', () => {
  const sel = { selection: { lines: 2, offset: { line: 3, column: 1 } } };
  assert.equal(runStatusText({ result: ok(sel) }), 'Run #3 · selection (2 lines) · 6 results · 1.3 ms');
  assert.equal(runStatusText({ result: ok({ ...sel, ok: false }) }), 'Run #3 · selection (2 lines) · error · 1.3 ms');
});

test('the cursor position has a long and a compact form', () => {
  assert.deepEqual(cursorTexts({ line: 1, col: 30 }), { long: 'Ln 1, Col 30', short: '1:30', title: 'Line 1, column 30' });
});

test('editor tooltips go into the enclosing dialog, else into the body', () => {
  const body = { tag: 'body' };
  const dialog = { tag: 'dialog' };
  const inDialog = { closest: (sel) => (sel === 'dialog' ? dialog : null), ownerDocument: { body } };
  const inPanel = { closest: () => null, ownerDocument: { body } };
  assert.equal(tooltipParent(inDialog), dialog);
  assert.equal(tooltipParent(inPanel), body);
});
