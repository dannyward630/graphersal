// Saved queries in the playground: the pure models behind the Catalog manager, the editor dialog and
// the run form (no Vue, no DOM, tested under Node: playground/tests/saved-queries.test.mjs).
//
// The engine's forms (crates/graphersal-session/src/queries.rs):
// - an ENTRY (listQueries/getQuery): {name, folder, description, params: [{name, schema,
//   default?, required, description?}], status: 'ok'|'error', error?}, getQuery adds `body`
//   (the stored function text), `dialect`, `meta`;
// - a SPEC (defineQuery): {name, folder, description, params: [{name, schema, default?}], body,
//   replaces?}: `body` is the text of ONE Rhai function named like the query whose parameters
//   are the declared ones, in that order; `replaces` names the query an edit started from;
// - FIELD ERRORS (a refused defineQuery): [{field, message, help?}] with `field` one of name,
//   folder, description, meta, body, params, params.<parameter>, general.
//
// The editor shows the function's BODY only (what is inside its braces): the signature is
// written from the parameter list, so adding, removing, renaming or reordering a parameter
// never needs a second edit. A stored source this model cannot split (comments before `fn`, an
// unusual layout) is edited as the whole function text (`raw`).

/**
 * The string `format`s the editor offers as types of their own: a uuid and the date and time
 * types (book "Dates and Times"); an argument is written as its text, the format reads it back.
 */
export const FORMAT_OF = Object.freeze({
  uuid: 'uuid', datetime: 'date-time', localdatetime: 'local-date-time', date: 'date', time: 'time', duration: 'duration',
});
const TYPE_OF_FORMAT = Object.fromEntries(Object.entries(FORMAT_OF).map(([t, f]) => [f, t]));
/** The parameter types written as text: plain strings and the formats. */
const TEXT_PARAM_TYPES = ['string', ...Object.keys(FORMAT_OF)];

/** The parameter types the editor offers; 'custom' is a schema edited as JSON. */
export const PARAM_TYPES = ['string', 'integer', 'number', 'boolean', ...Object.keys(FORMAT_OF), 'array', 'object', 'custom'];

/** Item types of an 'array' parameter. */
export const ITEM_TYPES = ['string', 'integer', 'number', 'boolean', ...Object.keys(FORMAT_OF)];

/** An example value of a format type (a field's placeholder), '' for the others. */
export function formatExample(type) {
  return {
    uuid: '01234567-89ab-cdef-0123-456789abcdef', datetime: '2023-08-02T09:30:00+02:00',
    localdatetime: '2023-08-02T09:30:00', date: '2023-08-02', time: '09:30:00', duration: 'P1DT2H',
  }[type] || '';
}

/** Schema keywords the editor's fields cover (anything else makes a parameter 'custom'). */
const FIELD_KEYWORDS = new Set([
  'type', 'format', 'items', 'minimum', 'maximum', 'minLength', 'maxLength', 'pattern', 'enum', 'description',
]);

const NAME_RE = /^[A-Za-z_][A-Za-z0-9_]*$/;
const UUID_RE = /^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$/;

// ------------------------------------------------------------------ the tree

/**
 * The saved queries grouped by folder, for the Catalog manager and the completion: `[{folder,
 * queries}]`, the root (`''`) first, then the folders by path; queries by name in each.
 * @param {Array<{name:string, folder:?string}>} list
 */
export function queryTree(list) {
  const groups = new Map();
  for (const entry of list || []) {
    const folder = entry.folder || '';
    if (!groups.has(folder)) groups.set(folder, []);
    groups.get(folder).push(entry);
  }
  return [...groups.entries()]
    .sort(([a], [b]) => (a === '' ? -1 : b === '' ? 1 : a.localeCompare(b)))
    .map(([folder, queries]) => ({
      folder,
      depth: folder ? folder.split('/').length : 0,
      queries: [...queries].sort((a, b) => a.name.localeCompare(b.name)),
    }));
}

/** A one-line signature for a menu row's tooltip: `top(country, limit = 20)`. */
export function signatureText(entry) {
  const params = (entry.params || []).map((p) =>
    ('default' in p ? `${p.name} = ${JSON.stringify(p.default)}` : p.name));
  return `${entry.name}(${params.join(', ')})`;
}

// ------------------------------------------------------------------ source <-> body

/**
 * Splits a stored function text `fn name(a, b) { ... }` into its parameter names and its body
 * (inside the braces, without the newline after `{` and before `}`). `null` when the text has
 * anything before `fn` (a doc comment) or another shape: it is then edited raw.
 */
export function splitSource(source, name) {
  const escaped = name.replace(/[$]/g, '\\$&');
  const m = new RegExp(`^\\s*fn\\s+${escaped}\\s*\\(([^)]*)\\)\\s*\\{([\\s\\S]*)\\}\\s*$`).exec(source || '');
  if (!m) return null;
  const params = m[1].split(',').map((p) => p.trim()).filter(Boolean);
  let body = m[2];
  if (!body.includes('\n')) body = body.trim();
  else {
    if (body.startsWith('\n')) body = body.slice(1);
    else if (body.startsWith('\r\n')) body = body.slice(2);
    body = body.replace(/\r?\n[ \t]*$/, '');
  }
  return { params, body };
}

/** The function text of a draft: `fn name(p1, p2) {\n<body>\n}` (a raw draft: its text). */
export function composeSource(draft) {
  if (draft.raw) return draft.body;
  const params = draft.params.map((p) => p.name.trim()).filter(Boolean).join(', ');
  return `fn ${draft.name.trim()}(${params}) {\n${draft.body}\n}`;
}

// ------------------------------------------------------------------ parameters

/** An empty editor parameter. */
export function newParam(name = '') {
  return {
    name, type: 'string', itemsType: 'string', minimum: '', maximum: '', minLength: '', maxLength: '',
    pattern: '', enumText: '', description: '', defaultText: '', schemaText: '',
  };
}

/** The editor's simple type of a schema node, or null when the fields cannot show it. */
function simpleType(schema) {
  if (!schema || typeof schema !== 'object' || Array.isArray(schema)) return null;
  if (Object.keys(schema).some((k) => !FIELD_KEYWORDS.has(k))) return null;
  if (schema.type === 'string' && TYPE_OF_FORMAT[schema.format]) return TYPE_OF_FORMAT[schema.format];
  if (schema.format !== undefined) return null;
  if (['string', 'integer', 'number', 'boolean', 'object'].includes(schema.type)) {
    return schema.items === undefined ? schema.type : null;
  }
  if (schema.type === 'array') {
    const items = schema.items;
    if (!items) return null;
    const t = simpleType(items);
    const plain = Object.keys(items).every((k) => k === 'type' || k === 'format');
    return plain && ITEM_TYPES.includes(t) ? 'array' : null;
  }
  return null;
}

/** The text of a default value in its field ('' without one). */
function defaultTextOf(type, entry) {
  if (!('default' in entry)) return '';
  const value = entry.default;
  if (TEXT_PARAM_TYPES.includes(type) && typeof value === 'string') return value;
  if (['integer', 'number', 'boolean'].includes(type)) return String(value);
  return JSON.stringify(value);
}

/** An editor parameter from an entry's parameter (`{name, schema, default?}`). */
export function paramFromEntry(entry) {
  const schema = entry.schema || {};
  const param = newParam(entry.name);
  const type = simpleType(schema);
  if (!type) {
    param.type = 'custom';
    param.schemaText = JSON.stringify(schema, null, 2);
    param.defaultText = 'default' in entry ? JSON.stringify(entry.default) : '';
    return param;
  }
  param.type = type;
  if (type === 'array') param.itemsType = simpleType(schema.items);
  for (const key of ['minimum', 'maximum', 'minLength', 'maxLength', 'pattern']) {
    if (schema[key] !== undefined) param[key] = String(schema[key]);
  }
  if (Array.isArray(schema.enum)) {
    param.enumText = schema.enum.every((v) => typeof v === 'string' && !v.includes(','))
      ? schema.enum.join(', ')
      : JSON.stringify(schema.enum);
  }
  param.description = schema.description || '';
  param.defaultText = defaultTextOf(type, entry);
  return param;
}

function numberField(text, label) {
  const t = String(text ?? '').trim();
  if (t === '') return { value: undefined };
  const n = Number(t);
  return Number.isFinite(n) ? { value: n } : { error: `${label} is not a number: ${t}` };
}

/** The enum list of a field: a JSON array, or comma-separated values of the type. */
function enumOf(text, type) {
  const t = String(text ?? '').trim();
  if (!t) return { value: undefined };
  if (t.startsWith('[')) {
    try {
      const list = JSON.parse(t);
      return Array.isArray(list) ? { value: list } : { error: 'allowed values: a JSON array' };
    } catch (err) {
      return { error: `allowed values are not JSON: ${err.message}` };
    }
  }
  const items = t.split(',').map((s) => s.trim()).filter((s) => s !== '');
  if (type === 'integer' || type === 'number') {
    const numbers = items.map(Number);
    if (numbers.some((n) => !Number.isFinite(n))) return { error: 'allowed values: numbers separated by commas' };
    return { value: numbers };
  }
  return { value: items };
}

/** A default's field text as a value of the type: `{value}` (undefined: none) or `{error}`. */
export function parseDefault(type, text) {
  const t = String(text ?? '');
  if (t.trim() === '') return { value: undefined };
  switch (type) {
    case 'string':
      return { value: t };
    case 'uuid':
      return UUID_RE.test(t.trim()) ? { value: t.trim() } : { error: `default is not a UUID: ${t}` };
    case 'datetime':
    case 'localdatetime':
    case 'date':
    case 'time':
    case 'duration':
      // The text; the engine reads it as the format when the query is stored.
      return { value: t.trim() };
    case 'integer':
      return /^[-+]?\d+$/.test(t.trim()) ? { value: Number(t.trim()) } : { error: `default is not an integer: ${t}` };
    case 'number': {
      const n = Number(t.trim());
      return Number.isFinite(n) ? { value: n } : { error: `default is not a number: ${t}` };
    }
    case 'boolean':
      if (t.trim() === 'true') return { value: true };
      if (t.trim() === 'false') return { value: false };
      return { error: 'default is true or false' };
    default:
      try {
        return { value: JSON.parse(t) };
      } catch (err) {
        return { error: `default is not JSON: ${err.message}` };
      }
  }
}

/**
 * The declaration of an editor parameter: `{declaration: {name, schema, default?}}` or
 * `{error}` (a field that does not parse; the engine checks everything else).
 */
export function paramDeclaration(param) {
  const name = param.name.trim();
  let schema;
  if (param.type === 'custom') {
    try {
      schema = JSON.parse(param.schemaText || '{}');
    } catch (err) {
      return { error: `schema is not JSON: ${err.message}` };
    }
    if (!schema || typeof schema !== 'object' || Array.isArray(schema)) return { error: 'schema is a JSON object' };
  } else {
    schema = FORMAT_OF[param.type] ? { type: 'string', format: FORMAT_OF[param.type] } : { type: param.type };
    if (param.type === 'array') {
      const it = param.itemsType || 'string';
      schema.items = FORMAT_OF[it] ? { type: 'string', format: FORMAT_OF[it] } : { type: it };
    }
    const numeric = param.type === 'integer' || param.type === 'number';
    const keys = numeric ? ['minimum', 'maximum'] : param.type === 'string' ? ['minLength', 'maxLength'] : [];
    for (const key of keys) {
      const n = numberField(param[key], key);
      if (n.error) return { error: n.error };
      if (n.value !== undefined) schema[key] = n.value;
    }
    if (param.type === 'string' && String(param.pattern || '').trim()) schema.pattern = param.pattern.trim();
    if (['string', 'integer', 'number'].includes(param.type)) {
      const e = enumOf(param.enumText, param.type);
      if (e.error) return { error: e.error };
      if (e.value !== undefined && e.value.length) schema.enum = e.value;
    }
    if (String(param.description || '').trim()) schema.description = param.description.trim();
  }
  const declaration = { name, schema };
  const parsed = parseDefault(param.type === 'custom' ? 'json' : param.type, param.defaultText);
  if (parsed.error) return { error: parsed.error };
  if (parsed.value !== undefined) declaration.default = parsed.value;
  return { declaration };
}

// ------------------------------------------------------------------ the editor draft

/** A draft for "New saved query" (in `folder`, optional). */
export function newDraft(folder = '') {
  return { name: '', folder, description: '', params: [], body: 'g.v()', raw: false, replaces: null };
}

/** A draft for editing a stored query (`getQuery` entry). */
export function draftFromQuery(entry) {
  const split = splitSource(entry.body || '', entry.name);
  const params = (entry.params || []).map(paramFromEntry);
  const sameSignature = split && split.params.join(',') === params.map((p) => p.name).join(',');
  return {
    name: entry.name,
    folder: entry.folder || '',
    description: entry.description || '',
    params,
    body: split && sameSignature ? split.body : entry.body || '',
    raw: !(split && sameSignature),
    replaces: entry.name,
  };
}

/**
 * The spec `defineQuery` takes, from a draft: `{spec}`, or `{errors}` (field errors found
 * before the engine is asked: a parameter field that does not parse, a missing name).
 */
export function draftToSpec(draft) {
  const errors = [];
  const name = draft.name.trim();
  if (!name) errors.push({ field: 'name', message: 'A saved query needs a name (letters, digits and _, e.g. top_customers).' });
  else if (!NAME_RE.test(name)) errors.push({ field: 'name', message: `"${name}" is not a valid name: letters, digits and _, not starting with a digit.` });
  const params = [];
  const seen = new Set();
  draft.params.forEach((param, index) => {
    const pname = param.name.trim();
    const field = `params.${pname || index}`;
    if (!pname) {
      errors.push({ field, message: `Parameter ${index + 1} needs a name.` });
      return;
    }
    if (seen.has(pname)) {
      errors.push({ field, message: `The parameter "${pname}" is declared twice.` });
      return;
    }
    seen.add(pname);
    const result = paramDeclaration(param);
    if (result.error) errors.push({ field, message: result.error });
    else params.push(result.declaration);
  });
  if (errors.length) return { errors };
  const spec = {
    name,
    folder: draft.folder.trim() || null,
    description: draft.description.trim() || null,
    params,
    body: composeSource({ ...draft, name }),
  };
  if (draft.replaces) spec.replaces = draft.replaces;
  return { spec };
}

/** Field errors by field: `{name: [..], 'params.limit': [..], ...}`. */
export function errorsByField(errors) {
  const out = {};
  for (const e of errors || []) {
    const text = e.help ? `${e.message} ${e.help}` : e.message;
    (out[e.field] = out[e.field] || []).push(text);
  }
  return out;
}

// ------------------------------------------------------------------ the run form

/** The base type of a schema node without `null` (`['integer', 'null']` -> 'integer'). */
function baseType(schema) {
  const t = schema && schema.type;
  if (Array.isArray(t)) {
    const rest = t.filter((x) => x !== 'null');
    return rest.length === 1 ? rest[0] : null;
  }
  return t || null;
}

/**
 * The fields of the run form, one per parameter: `{name, kind, required, hint, schema, min,
 * max, options, itemsKind, defaultValue, hasDefault}`. `kind`: 'select' (enum), 'integer',
 * 'number', 'checkbox' (boolean), 'uuid', a date and time type ('datetime', 'localdatetime',
 * 'date', 'time', 'duration': its ISO 8601 text), 'list' (array of simple items, one per line),
 * 'json' (objects and anything else), 'text' (string).
 */
export function formFields(entry) {
  return (entry.params || []).map((p) => {
    const schema = p.schema || {};
    const type = baseType(schema);
    let kind = 'json';
    let itemsKind = null;
    if (Array.isArray(schema.enum)) kind = 'select';
    else if (type === 'string' && TYPE_OF_FORMAT[schema.format]) kind = TYPE_OF_FORMAT[schema.format];
    else if (type === 'string') kind = 'text';
    else if (type === 'integer' || type === 'number') kind = type;
    else if (type === 'boolean') kind = 'checkbox';
    else if (type === 'array' && schema.items) {
      const it = baseType(schema.items);
      itemsKind = it === 'string' && TYPE_OF_FORMAT[schema.items.format] ? TYPE_OF_FORMAT[schema.items.format] : it;
      kind = ITEM_TYPES.includes(itemsKind) ? 'list' : 'json';
    }
    return {
      name: p.name,
      kind,
      required: !!p.required,
      hint: p.description || schema.description || '',
      schema,
      min: schema.minimum,
      max: schema.maximum,
      options: Array.isArray(schema.enum) ? schema.enum : null,
      itemsKind,
      hasDefault: 'default' in p,
      defaultValue: p.default,
    };
  });
}

/** A value as the text of its field (a select option is the value's JSON). */
function fieldText(field, value) {
  if (value === undefined) return '';
  switch (field.kind) {
    case 'select':
      return JSON.stringify(value);
    case 'checkbox':
      return value === true;
    case 'text':
    case 'uuid':
    case 'datetime':
    case 'localdatetime':
    case 'date':
    case 'time':
    case 'duration':
      return typeof value === 'string' ? value : JSON.stringify(value);
    case 'integer':
    case 'number':
      return String(value);
    case 'list':
      return Array.isArray(value) ? value.map((v) => (typeof v === 'string' ? v : JSON.stringify(v))).join('\n') : JSON.stringify(value);
    default:
      return JSON.stringify(value, null, 2);
  }
}

/**
 * The form's first values: the last values used for this query (`saved`, from localStorage)
 * where they still fit the fields, else the defaults (prefilled), else empty.
 */
export function initialValues(fields, saved = {}) {
  const values = {};
  for (const f of fields) {
    const last = saved && Object.prototype.hasOwnProperty.call(saved, f.name) ? saved[f.name] : undefined;
    if (last !== undefined && (f.kind === 'checkbox' ? typeof last === 'boolean' : typeof last === 'string')) {
      values[f.name] = last;
    } else if (f.kind === 'checkbox') {
      values[f.name] = f.hasDefault ? f.defaultValue === true : false;
    } else {
      values[f.name] = f.hasDefault ? fieldText(f, f.defaultValue) : '';
    }
  }
  return values;
}

function parseItem(kind, text) {
  switch (kind) {
    case 'integer':
      return /^[-+]?\d+$/.test(text) ? { value: Number(text) } : { error: `"${text}" is not an integer` };
    case 'number': {
      const n = Number(text);
      return text !== '' && Number.isFinite(n) ? { value: n } : { error: `"${text}" is not a number` };
    }
    case 'boolean':
      return text === 'true' || text === 'false' ? { value: text === 'true' } : { error: `"${text}" is not true or false` };
    case 'uuid':
      return UUID_RE.test(text) ? { value: text } : { error: `"${text}" is not a UUID (8-4-4-4-12 hex digits)` };
    default:
      return { value: text };
  }
}

/**
 * The named arguments of a filled form: `{args, errors}` (`errors`: field name -> message).
 * An empty field is left out, so the parameter's default applies; an empty required field is
 * an error. Values are converted to the declared type here (the engine checks them again).
 */
export function formArgs(fields, values) {
  const args = {};
  const errors = {};
  for (const f of fields) {
    const raw = values[f.name];
    if (f.kind === 'checkbox') {
      args[f.name] = raw === true;
      continue;
    }
    const text = String(raw ?? '').trim();
    if (text === '') {
      if (f.required) errors[f.name] = 'required';
      continue;
    }
    let result;
    switch (f.kind) {
      case 'select':
        try {
          result = { value: JSON.parse(text) };
        } catch (err) {
          result = { value: text };
        }
        break;
      case 'integer':
      case 'number':
        result = parseItem(f.kind, text);
        if (result.value !== undefined && f.kind === 'integer' && !Number.isSafeInteger(result.value)) {
          result = { error: `${text} is outside the exact integer range of the form; write the call by hand` };
        }
        if (result.value !== undefined && f.min !== undefined && result.value < f.min) result = { error: `at least ${f.min}` };
        if (result.value !== undefined && f.max !== undefined && result.value > f.max) result = { error: `at most ${f.max}` };
        break;
      case 'uuid':
        result = parseItem('uuid', text);
        break;
      case 'list':
        if (text.startsWith('[')) {
          try {
            result = { value: JSON.parse(text) };
          } catch (err) {
            result = { error: `not a JSON array: ${err.message}` };
          }
        } else {
          const items = [];
          for (const line of text.split('\n').map((l) => l.trim()).filter((l) => l !== '')) {
            const item = parseItem(f.itemsKind, line);
            if (item.error) {
              result = item;
              break;
            }
            items.push(item.value);
          }
          if (!result) result = { value: items };
        }
        break;
      case 'json':
        try {
          result = { value: JSON.parse(text) };
        } catch (err) {
          result = { error: `not JSON: ${err.message}` };
        }
        break;
      default:
        result = { value: String(raw) };
    }
    if (result.error) errors[f.name] = result.error;
    else args[f.name] = result.value;
  }
  return { args, errors };
}

/** The values to remember for a query (localStorage, per query name). */
export function rememberValues(all, name, values, max = 100) {
  const next = { ...(all && typeof all === 'object' ? all : {}) };
  delete next[name];
  next[name] = { ...values };
  const names = Object.keys(next);
  for (const old of names.slice(0, Math.max(0, names.length - max))) delete next[old];
  return next;
}

// ------------------------------------------------------------------ completion

/**
 * The name being typed as the first argument of `g.query(`, or null: `before` is the text of
 * the line up to the cursor. `{prefix, quote}`: the typed part of the name and its quote.
 */
export function queryNameContext(before) {
  const m = /\bquery\s*\(\s*(["'`])([A-Za-z0-9_]*)$/.exec(before || '');
  return m ? { quote: m[1], prefix: m[2] } : null;
}
