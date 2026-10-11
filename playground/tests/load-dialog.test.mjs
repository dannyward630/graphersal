// Node tests of the start page's "Load from file" dialog model (js/load-dialog.js): a graph file
// with an OPTIONAL schema file, and what a drop loads.
// Run: node --test playground/tests/*.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { loadChoice, droppedFiles, isSchemaFileName } from '../js/load-dialog.js';

const f = (name) => ({ name });

test('dialog: a graph file alone loads without a schema', () => {
  const graph = f('g.graphml');
  assert.deepEqual(loadChoice({ graphFile: graph }), { request: { kind: 'file', file: graph } });
  assert.deepEqual(loadChoice({ graphFile: graph, schemaFile: null }), { request: { kind: 'file', file: graph } });
});

test('dialog: a graph file with a schema file loads both', () => {
  const graph = f('g.json');
  const schema = f('graphersal-schema-20261010.json');
  assert.deepEqual(loadChoice({ graphFile: graph, schemaFile: schema }), {
    request: { kind: 'file', file: graph, schemaFile: schema },
  });
});

test('dialog: no graph file, a wrong graph or schema file, a schema with a snapshot are refused', () => {
  assert.match(loadChoice({}).error, /Choose a graph file/);
  assert.match(loadChoice({ schemaFile: f('s.json') }).error, /Choose a graph file/);
  assert.match(loadChoice({ graphFile: f('g.csv') }).error, /not a graph file/);
  assert.match(loadChoice({ graphFile: f('g.graphml'), schemaFile: f('s.yaml') }).error, /not a schema file/);
  assert.match(loadChoice({ graphFile: f('g.gsnap'), schemaFile: f('s.json') }).error, /brings its own schema/);
  assert.deepEqual(loadChoice({ graphFile: f('g.gsnap') }), { request: { kind: 'file', file: { name: 'g.gsnap' } } });
});

test('drop: one file loads without a schema', () => {
  const graph = f('g.json');
  assert.deepEqual(droppedFiles([graph]), { request: { kind: 'file', file: graph } });
  assert.match(droppedFiles([]).error, /Nothing/);
  assert.match(droppedFiles(null).error, /Nothing/);
  assert.match(droppedFiles([f('notes.txt')]).error, /not a graph file/);
});

test('drop: a non-JSON graph file and a .json schema pair up, in either order', () => {
  for (const name of ['g.graphml', 'g.xml', 'g.jsonl', 'g.graphson']) {
    const graph = f(name);
    const schema = f('schema.json');
    const expected = { request: { kind: 'file', file: graph, schemaFile: schema } };
    assert.deepEqual(droppedFiles([graph, schema]), expected);
    assert.deepEqual(droppedFiles([schema, graph]), expected);
  }
});

test('drop: ambiguous or unpairable sets are refused', () => {
  // Two .json files: either could be the GraphSON.
  assert.match(droppedFiles([f('g.json'), f('schema.json')]).error, /use the dialog/);
  // Two graph files, three files, a snapshot with a schema.
  assert.match(droppedFiles([f('a.graphml'), f('b.graphml')]).error, /Drop one graph file/);
  assert.match(droppedFiles([f('a.graphml'), f('s.json'), f('t.json')]).error, /Drop one graph file/);
  assert.match(droppedFiles([f('g.gsnap'), f('s.json')]).error, /brings its own schema/);
});

test('schema file names are .json', () => {
  assert.equal(isSchemaFileName('s.json'), true);
  assert.equal(isSchemaFileName('S.JSON'), true);
  assert.equal(isSchemaFileName('s.jsonl'), false);
  assert.equal(isSchemaFileName(''), false);
});
