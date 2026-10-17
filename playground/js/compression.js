// The models of the Catalog dropdown and its Compression manager: what the engine's
// `listCompressions()` returns (crates/graphersal-session `compression.rs`: `[{name, element,
// label, path, codec, minBytes, dictionary, stats: {compressedValues, plainBytes, storedBytes,
// dictionaryBytes}}]`) turned into rows to show, the form's draft and its spec, and the items of
// the Catalog dropdown with their counts. Pure functions, no Vue, no DOM (tested under Node:
// tests/compression.test.mjs).

import { formatBytes } from './statistics.js';

/** The default threshold of a rule (the engine's `DEFAULT_MIN_BYTES`). */
export const DEFAULT_MIN_BYTES = 256;
/** The largest threshold (the engine's `MAX_MIN_BYTES`, the default `traversal.max_string_bytes`). */
export const MAX_MIN_BYTES = 16 * 1024 * 1024;

/**
 * The kinds of the catalog the Catalog dropdown lists, in its order, with what each manager
 * opens. The property indexes come after them: js/indexes.js `catalogMenuItems` appends their
 * item (and the dropdown uses that list).
 */
export const CATALOG_KINDS = Object.freeze([
  Object.freeze({ kind: 'query', id: 'queries', title: 'Saved queries', hint: 'Named, parameterized, read-only queries stored in the graph' }),
  Object.freeze({ kind: 'compression', id: 'compression', title: 'Compression', hint: 'Rules that keep large strings compressed in memory' }),
]);

/**
 * The items of the Catalog dropdown: one per kind with its count, from the graph info's
 * `catalog` (`{query: 2, compression: 1}`; older answers have only `savedQueries`).
 * @param {object|null} info the graph info
 * @returns {Array<{id:string, title:string, hint:string, count:number}>}
 */
export function catalogItems(info) {
  const counts = (info && info.catalog) || {};
  return CATALOG_KINDS.map((k) => {
    let count = counts[k.kind];
    if (count === undefined && k.kind === 'query') count = info && info.savedQueries;
    if (count === undefined && k.kind === 'compression') count = info && info.compressionRules;
    return { id: k.id, title: k.title, hint: k.hint, count: Number(count) || 0 };
  });
}

/** How many definitions of every kind the catalog holds (unknown kinds included). */
export function catalogTotal(info) {
  const counts = (info && info.catalog) || {};
  return Object.values(counts).reduce((sum, n) => sum + (Number(n) || 0), 0);
}

/**
 * The text of the Store menu's catalog line from `StoreInfo`'s `definitionsByKind`
 * (`{query: 2, compression: 1}`): `"2 saved queries, 1 compression rule"`.
 */
export function catalogCountsText(byKind, total) {
  const names = {
    query: ['saved query', 'saved queries'],
    compression: ['compression rule', 'compression rules'],
    property_index: ['property index', 'property indexes'],
  };
  const order = CATALOG_KINDS.map((k) => k.kind);
  const rank = (kind) => (order.includes(kind) ? order.indexOf(kind) : order.length);
  const parts = Object.entries(byKind || {})
    .filter(([, n]) => Number(n) > 0)
    .sort(([a], [b]) => rank(a) - rank(b) || a.localeCompare(b))
    .map(([kind, n]) => {
      const [one, many] = names[kind] || [`${kind} definition`, `${kind} definitions`];
      return `${n} ${Number(n) === 1 ? one : many}`;
    });
  if (parts.length) return parts.join(', ');
  return total ? `${total} definition${total === 1 ? '' : 's'}` : 'empty';
}

/** `0.42` -> `42 %` (the stored share of the plain size); `—` without values. */
export function ratioText(stats) {
  if (!stats || !stats.plainBytes) return '—';
  return `${Math.round((100 * stats.storedBytes) / stats.plainBytes)} %`;
}

/**
 * One row of the rules table: the rule and its stats as text.
 * @param {object} rule an entry of `listCompressions()`
 */
export function ruleRow(rule) {
  const s = rule.stats || {};
  return {
    name: rule.name,
    target: `${rule.element === 'edge' ? 'edge' : 'vertex'} ${rule.label}`,
    path: rule.path,
    threshold: `≥ ${formatBytes(rule.minBytes)}`,
    dictionary: rule.dictionary ? (s.dictionaryBytes ? formatBytes(s.dictionaryBytes) : 'yes (none yet)') : 'no',
    values: s.compressedValues || 0,
    sizes: s.compressedValues ? `${formatBytes(s.plainBytes)} → ${formatBytes(s.storedBytes)}` : '—',
    ratio: ratioText(s),
    saved: s.plainBytes > s.storedBytes ? formatBytes(s.plainBytes - s.storedBytes) : '—',
  };
}

/** The totals of all rules: values, plain, stored and dictionary bytes. */
export function totals(rules) {
  const t = { compressedValues: 0, plainBytes: 0, storedBytes: 0, dictionaryBytes: 0 };
  for (const r of rules || []) {
    const s = r.stats || {};
    t.compressedValues += s.compressedValues || 0;
    t.plainBytes += s.plainBytes || 0;
    t.storedBytes += s.storedBytes || 0;
    t.dictionaryBytes += s.dictionaryBytes || 0;
  }
  return t;
}

/** A new rule's draft (the form). */
export function newDraft() {
  return {
    name: '',
    element: 'vertex',
    label: '',
    path: '',
    minBytesText: String(DEFAULT_MIN_BYTES),
    dictionary: true,
    replaces: null,
  };
}

/** The draft of a stored rule (Edit): its path without the leading `$.` when it is plain names. */
export function draftFromRule(rule) {
  const path = String(rule.path || '');
  const plain = /^\$\.[A-Za-z_][A-Za-z0-9_]*(\.[A-Za-z_][A-Za-z0-9_]*)*$/.test(path);
  return {
    name: rule.name,
    element: rule.element === 'edge' ? 'edge' : 'vertex',
    label: rule.label,
    path: plain ? path.slice(2) : path,
    minBytesText: String(rule.minBytes ?? DEFAULT_MIN_BYTES),
    dictionary: rule.dictionary !== false,
    replaces: rule.name,
  };
}

/**
 * The spec `defineCompression` takes, or the field errors the form shows without asking the
 * engine (empty fields, a threshold that is not a whole number in range).
 * @returns {{spec: object} | {errors: Array<{field:string, message:string}>}}
 */
export function draftToSpec(draft) {
  const errors = [];
  const name = String(draft.name || '').trim();
  const label = String(draft.label || '').trim();
  const path = String(draft.path || '').trim();
  if (!name) errors.push({ field: 'name', message: 'A rule needs a name (an identifier such as customer_cv).' });
  if (!label) errors.push({ field: 'label', message: 'Name the vertex or edge label the rule applies to.' });
  if (!path) errors.push({ field: 'path', message: 'Name the property path, for example cv or details.career.cv.' });
  const minText = String(draft.minBytesText ?? '').trim();
  let minBytes = DEFAULT_MIN_BYTES;
  if (minText) {
    minBytes = Number(minText);
    if (!Number.isInteger(minBytes) || minBytes < 1 || minBytes > MAX_MIN_BYTES) {
      errors.push({ field: 'minBytes', message: `A whole number of bytes from 1 to ${MAX_MIN_BYTES}.` });
    }
  }
  if (errors.length) return { errors };
  const spec = {
    name,
    element: draft.element === 'edge' ? 'edge' : 'vertex',
    label,
    path,
    minBytes,
    dictionary: !!draft.dictionary,
  };
  if (draft.replaces) spec.replaces = draft.replaces;
  return { spec };
}

/**
 * The label suggestions of the form: the graph's labels of the chosen element kind, from the
 * engine's `statistics()` answer (`{statistics: {vertex_labels: [{label}], edge_labels:
 * [{label}]}}`), sorted, without duplicates.
 */
export function labelSuggestions(statistics, element) {
  const s = statistics && statistics.statistics;
  if (!s) return [];
  const rows = element === 'edge' ? s.edge_labels : s.vertex_labels;
  return [...new Set((rows || []).map((r) => r.label).filter((l) => typeof l === 'string' && l))].sort();
}
