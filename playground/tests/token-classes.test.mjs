// Node tests of the highlighter's token classes (js/editor/token-classes.js): a fixed fallback
// before the engine's data is loaded, the engine's classes added by `learnTokenClasses` (called
// by `loadCompletionData`).
// Run: node --test playground/tests/*.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { TOKEN_CLASSES, learnTokenClasses } from '../js/editor/token-classes.js';
import { loadCompletionData } from '../js/editor/completion-data.js';

test('the fallback holds every token class of the engine today, SchemaMode and WithOptions included', () => {
  // What `GET /api/token-values` reports on 2026-10-11 (the session's `token_values`).
  const engine = ['Barrier', 'By', 'Cardinality', 'CastPolicy', 'Column', 'Direction', 'GType', 'Merge',
    'Operator', 'Order', 'Pick', 'Pop', 'ProfileType', 'SchemaMode', 'Scope', 'T', 'V', 'WithOptions'];
  for (const name of engine) assert.ok(TOKEN_CLASSES.has(name), name);
  for (const name of ['g', '__', 'P', 'TextP', 'UUID', 'Set', 'GraphSource']) assert.ok(TOKEN_CLASSES.has(name), name);
});

test('learnTokenClasses adds the classes the engine reports (identifiers only), once', () => {
  assert.equal(TOKEN_CLASSES.has('FutureToken'), false);
  assert.equal(learnTokenClasses({ FutureToken: ['a'], T: ['id'], 'not a name': ['x'] }), 1);
  assert.ok(TOKEN_CLASSES.has('FutureToken'));
  assert.equal(TOKEN_CLASSES.has('not a name'), false);
  assert.equal(learnTokenClasses({ FutureToken: ['a'] }), 0);
  assert.equal(learnTokenClasses(null), 0);
  TOKEN_CLASSES.delete('FutureToken');
});

test('loadCompletionData teaches the highlighter the engine classes; a failing engine keeps the fallback', async () => {
  const engine = {
    completions: async () => [],
    tokenValues: async () => ({ NewClass: ['x'] }),
  };
  await loadCompletionData(engine);
  assert.ok(TOKEN_CLASSES.has('NewClass'));
  TOKEN_CLASSES.delete('NewClass');
  const before = TOKEN_CLASSES.size;
  await loadCompletionData({
    completions: async () => { throw new Error('down'); },
    tokenValues: async () => { throw new Error('down'); },
  });
  assert.equal(TOKEN_CLASSES.size, before);
  assert.ok(TOKEN_CLASSES.has('SchemaMode'));
});
