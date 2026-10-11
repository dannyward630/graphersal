// Node tests of the completion data (js/editor/completion-data.js): the token values come from the
// engine (`tokenValues()`, here through the WebAssembly engine's worker protocol with a stub
// worker), and the choices after `Class.` are built from them.
// Run: node --test playground/tests/*.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { loadCompletionData, tokenChoices, tokenClassNames } from '../js/editor/completion-data.js';
import { WasmEngine } from '../js/engine/wasm-engine.js';

// What `Playground.tokenValues()` answers (a part of it; the session test pins the full set).
const TOKENS = {
  By: ['Count', 'Id', 'Label', 'Value'],
  Cardinality: ['list', 'set', 'single'],
  Order: ['asc', 'desc', 'shuffle'],
  T: ['id', 'key', 'label', 'value'],
  V: ['Table', 'Tree'],
  SchemaMode: ['closed', 'none', 'open'],
};
const CATALOGUE = [
  { name: 'Property', aliases: [], receiver: 'By', arities: [1], signatures: ['(key: string)'], doc: 'By a property.' },
  { name: 'list', aliases: [], receiver: 'Cardinality', arities: [2], signatures: ['(k, v)'], doc: 'A list value.' },
  { name: 'Table', aliases: [], receiver: 'V', arities: [0], signatures: ['()'], doc: 'The table visualizer.' },
  { name: 'has_label', aliases: ['hasLabel'], receiver: 'traversal', arities: [1], signatures: ['(l)'], doc: 'x' },
];

/** A stand-in for the module worker: answers the protocol of worker.js from fixed data. */
function stubWorker(answers) {
  return class FakeWorker {
    constructor() {
      this.onmessage = null;
      this.onerror = null;
    }
    postMessage({ id, method }) {
      const known = Object.prototype.hasOwnProperty.call(answers, method);
      queueMicrotask(() => this.onmessage({
        data: known ? { id, ok: true, result: answers[method] } : { id, ok: false, error: `no ${method}` },
      }));
    }
    terminate() {}
  };
}

test('the completion data comes from the engine through the worker', async (t) => {
  const saved = globalThis.Worker;
  t.after(() => { globalThis.Worker = saved; });
  globalThis.Worker = stubWorker({
    start: { version: 'test', memoryTracked: false, samples: [], methods: [] },
    completions: CATALOGUE,
    tokenValues: TOKENS,
  });
  const data = await loadCompletionData(new WasmEngine());
  assert.deepEqual(data.tokens, TOKENS);
  assert.equal(data.catalogue.length, CATALOGUE.length);
});

test('a failing call leaves its part empty', async (t) => {
  const saved = globalThis.Worker;
  t.after(() => { globalThis.Worker = saved; });
  globalThis.Worker = stubWorker({ start: { version: 'old', samples: [] }, completions: CATALOGUE });
  const data = await loadCompletionData(new WasmEngine());
  assert.deepEqual(data.tokens, {});
  assert.equal(data.catalogue.length, CATALOGUE.length);
});

test('the choices after a token class are its values, then its other functions', () => {
  const by = tokenChoices('By', TOKENS, CATALOGUE);
  assert.deepEqual(by.constants.map((c) => c.label), ['Count', 'Id', 'Label', 'Value']);
  assert.deepEqual(by.entries.map((e) => e.name), ['Property']);
  // A value with a function of the same name stays a constant and carries its doc.
  const cardinality = tokenChoices('Cardinality', TOKENS, CATALOGUE);
  assert.deepEqual(cardinality.constants.map((c) => c.label), ['list', 'set', 'single']);
  assert.equal(cardinality.constants[0].entry.doc, 'A list value.');
  assert.deepEqual(cardinality.entries, []);
  assert.equal(tokenChoices('V', TOKENS, CATALOGUE).constants[0].entry.name, 'Table');
  // Not a token class: the caller falls back to the receiver's functions.
  assert.equal(tokenChoices('P', TOKENS, CATALOGUE), null);
  assert.equal(tokenChoices('g', TOKENS, CATALOGUE), null);
  assert.equal(tokenChoices('constructor', TOKENS, CATALOGUE), null);
  assert.equal(tokenChoices('Order', {}, CATALOGUE), null);
});

test('the engine adds token classes the highlighter does not list', () => {
  assert.deepEqual(tokenClassNames(['P', 'Order'], TOKENS),
    ['By', 'Cardinality', 'Order', 'P', 'SchemaMode', 'T', 'V']);
});
