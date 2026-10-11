// Node tests of what Run executes (the editor's selection or the whole script) and of how the
// engine's error positions map back into the full editor text (js/editor/run-selection.js).
// Run: node --test playground/tests/*.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { runText, mapPosition, mapErrorPosition, positionAt, selectionText } from '../js/editor/run-selection.js';

const DOC = 'let a = 1;\n  g.v().count()\ng.v().foo()\n';

test('no selection runs the whole script, trimmed', () => {
  const r = runText(DOC, 5, 5);
  assert.equal(r.selection, false);
  assert.equal(r.text, DOC.trim());
  assert.equal(r.lines, 3);
  assert.deepEqual(r.offset, { line: 1, column: 1 });
});

test('a selection runs only the selected text', () => {
  const from = DOC.indexOf('g.v().count()');
  const r = runText(DOC, from, from + 'g.v().count()'.length);
  assert.equal(r.selection, true);
  assert.equal(r.text, 'g.v().count()');
  assert.equal(r.lines, 1);
  assert.deepEqual(r.offset, { line: 2, column: 3 });
});

test('a selection made backwards (anchor after head) is the same range', () => {
  const from = DOC.indexOf('g.v().foo()');
  assert.deepEqual(runText(DOC, from + 11, from), runText(DOC, from, from + 11));
});

test('the selection is trimmed and its offset is its first non-blank character', () => {
  // From the end of line 1 to the end of line 2: starts with "\n  ".
  const r = runText(DOC, DOC.indexOf('\n'), DOC.indexOf('\ng.v().foo'));
  assert.equal(r.text, 'g.v().count()');
  assert.deepEqual(r.offset, { line: 2, column: 3 });
  const two = runText(DOC, DOC.indexOf('  g.v'), DOC.length);
  assert.equal(two.text, 'g.v().count()\ng.v().foo()');
  assert.equal(two.lines, 2);
});

test('a selection of only whitespace runs the whole script', () => {
  const r = runText(DOC, DOC.indexOf('\n'), DOC.indexOf('g.v().count'));
  assert.equal(r.selection, false);
  assert.equal(r.text, DOC.trim());
});

test('leading blank lines of the whole script move the offset', () => {
  const r = runText('\n\n  g.v()', 0, 0);
  assert.equal(r.text, 'g.v()');
  assert.deepEqual(r.offset, { line: 3, column: 3 });
});

test('an empty script runs nothing', () => {
  const r = runText('  \n ', 0, 0);
  assert.equal(r.text, '');
  assert.equal(r.selection, false);
  assert.equal(r.lines, 0);
});

test('positions on the first line shift by line and column, later lines only by line', () => {
  const offset = { line: 2, column: 3 };
  assert.deepEqual(mapPosition({ line: 1, column: 7 }, offset), { line: 2, column: 9 });
  assert.deepEqual(mapPosition({ line: 2, column: 4 }, offset), { line: 3, column: 4 });
  assert.deepEqual(mapPosition({ line: 1, column: 1 }, { line: 1, column: 1 }), { line: 1, column: 1 });
  assert.equal(mapPosition(undefined, offset), undefined);
  assert.deepEqual(mapPosition({ line: 1, column: 1 }, null), { line: 1, column: 1 });
});

test('positionAt counts lines and UTF-16 columns', () => {
  assert.deepEqual(positionAt('ab\ncd', 0), { line: 1, column: 1 });
  assert.deepEqual(positionAt('ab\ncd', 4), { line: 2, column: 2 });
});

test('an error moves its position and the position written in its texts', () => {
  const error = {
    message: 'Function not found: foo () (line 1, position 7)',
    diagnostic: 'Error: Function not found: foo () (line 1, position 7)',
    help: 'unchanged (line 1, position 7)',
    position: { line: 1, column: 7 },
  };
  const moved = mapErrorPosition(error, { line: 3, column: 1 });
  assert.deepEqual(moved.position, { line: 3, column: 7 });
  assert.equal(moved.message, 'Function not found: foo () (line 3, position 7)');
  assert.equal(moved.diagnostic, 'Error: Function not found: foo () (line 3, position 7)');
  assert.equal(moved.help, error.help);
  assert.deepEqual(error.position, { line: 1, column: 7 }, 'the input is not changed');
  const plain = { message: 'no position' };
  assert.equal(mapErrorPosition(plain, { line: 3, column: 1 }), plain);
});

test('the partial-run text', () => {
  assert.equal(selectionText(1), 'selection (1 line)');
  assert.equal(selectionText(3), 'selection (3 lines)');
});
