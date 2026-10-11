// The model of the start page's "Load from file" dialog (components/start-page.js): a graph file
// and an OPTIONAL schema file. "Load graph" always starts WITHOUT the previous graph's schema;
// with a schema file the schema is set on the new empty graph first and the data imported
// against it as one unit (GraphML's typed values come back, open/closed violations fail the
// load and nothing changes). A snapshot (.gsnap) brings its own schema, so it takes none.
// No DOM here: the Node tests (tests/load-dialog.test.mjs) run it.

import { graphFileFormat } from './engine/wasm-engine.js';

/** Whether `name` can be a schema file (JSON in the schema format). */
export function isSchemaFileName(name) {
  return /\.json$/i.test(name || '');
}

/**
 * Checks the dialog's choice: `{request}` for `loadGraph` (`{kind: 'file', file, schemaFile?}`),
 * or `{error}` saying what is wrong.
 * @param {{graphFile?: {name:string}|null, schemaFile?: {name:string}|null}} choice
 * @returns {{request?: {kind:'file', file:any, schemaFile?:any}, error?: string}}
 */
export function loadChoice({ graphFile = null, schemaFile = null } = {}) {
  if (!graphFile) return { error: 'Choose a graph file.' };
  const format = graphFileFormat(graphFile.name);
  if (!format) {
    return { error: `"${graphFile.name}" is not a graph file: choose a snapshot (.gsnap), GraphSON (.json, .jsonl, .graphson) or GraphML (.graphml, .xml).` };
  }
  if (!schemaFile) return { request: { kind: 'file', file: graphFile } };
  if (!isSchemaFileName(schemaFile.name)) {
    return { error: `"${schemaFile.name}" is not a schema file: a schema is JSON (.json).` };
  }
  if (format === 'snapshot') {
    return { error: 'A snapshot (.gsnap) brings its own schema: load it without a schema file (Load schema afterwards changes it).' };
  }
  return { request: { kind: 'file', file: graphFile, schemaFile } };
}

/**
 * What a drop on the "Load from file" card loads: ONE file loads without a schema; a graph data
 * file plus a `.json` schema pair up when it is unambiguous, i.e. the graph file is NOT `.json`
 * itself (GraphML, `.jsonl`, `.graphson`); two `.json` files could be either, so they are
 * refused (choose them in the dialog). Returns `{request}` or `{error}`.
 * @param {Array<{name:string}>} files
 */
export function droppedFiles(files) {
  const list = Array.from(files || []);
  if (list.length === 0) return { error: 'Nothing was dropped.' };
  if (list.length === 1) return loadChoice({ graphFile: list[0] });
  if (list.length === 2) {
    const schemas = list.filter((f) => isSchemaFileName(f.name));
    const graphs = list.filter((f) => !isSchemaFileName(f.name) && graphFileFormat(f.name));
    if (schemas.length === 1 && graphs.length === 1) {
      return loadChoice({ graphFile: graphs[0], schemaFile: schemas[0] });
    }
  }
  return { error: 'Drop one graph file, or a graph file and its schema (.json); to pair a .json graph with a schema, use the dialog.' };
}
