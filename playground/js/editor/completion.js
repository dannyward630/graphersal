// Autocompletion for the DSL: steps after '.', predicates after `P.`/`TextP.`, token values after
// `T.`/`Order.`/..., and the global names elsewhere. Every spelling (snake_case and camelCase)
// is offered; the info tooltip shows the overloads and the step's documentation.
//
// The catalogue comes from `engine.completions()`: the Rhai engine's own function metadata (names,
// overloads with parameter types, registered doc comments); the token values from
// `engine.tokenValues()` (the getters the engine registers; the CLI REPL uses the same source).
// Both are loaded by `loadCompletionData` (completion-data.js).

import { TOKEN_CLASSES } from './dsl-language.js';
import { splitExample } from './completion-doc.js';
import { tokenChoices, tokenClassNames } from './completion-data.js';
import { queryTree, signatureText, queryNameContext } from '../saved-queries.js';

const RECEIVER_OF = {
  P: 'P', TextP: 'TextP', By: 'By', UUID: 'UUID', Set: 'Set', V: 'V', CastPolicy: 'CastPolicy',
  GraphSource: 'GraphSource',
};

const KEYWORDS = ['let', 'fn', 'if', 'else', 'for', 'while', 'loop', 'return', 'true', 'false'];

function globals(tokens) {
  return [
    { label: 'g', detail: 'traversal source', info: 'The traversal source over the loaded graph: g.v(), g.e(), g.inject(...)' },
    { label: '__', detail: 'anonymous traversal', info: 'Starts a child traversal: __.out(), __.values("name")' },
    ...tokenClassNames(TOKEN_CLASSES, tokens).filter((n) => n !== 'g' && n !== '__').map((n) => ({ label: n, detail: 'token class' })),
  ];
}

function docNode(entry, spelling, constant = false) {
  const root = document.createElement('div');
  root.className = 'completion-doc';
  const h = document.createElement('h5');
  h.textContent = constant ? spelling : spelling + '()';
  root.appendChild(h);
  const others = [entry.name, ...entry.aliases].filter((n) => n !== spelling);
  if (others.length) {
    const a = document.createElement('div');
    a.className = 'aliases';
    a.textContent = 'also: ' + others.join(', ');
    root.appendChild(a);
  }
  if (entry.signatures && entry.signatures.length) {
    const ul = document.createElement('ul');
    ul.className = 'sigs';
    for (const s of entry.signatures) {
      const li = document.createElement('li');
      li.textContent = spelling + s;
      ul.appendChild(li);
    }
    root.appendChild(ul);
  }
  // `doc` is the engine's text: one sentence, an empty line, `Example: ...`.
  const doc = entry.doc || 'See the signatures above; the step is described in the Graphersal book.';
  const [summary, example] = splitExample(doc);
  const p = document.createElement('p');
  p.textContent = summary.length > 1400 ? summary.slice(0, 1400) + ' …' : summary;
  root.appendChild(p);
  if (example) {
    const ex = document.createElement('pre');
    ex.className = 'example';
    ex.textContent = example;
    root.appendChild(ex);
  }
  const cat = document.createElement('div');
  cat.className = 'cat';
  cat.textContent = [entry.category, entry.receiver !== 'traversal' ? entry.receiver : null, `arity ${arityText(entry.arities)}`]
    .filter(Boolean).join(' · ');
  root.appendChild(cat);
  return root;
}

function arityText(arities) {
  if (!arities.length) return '?';
  const sorted = [...arities].sort((a, b) => a - b);
  const contiguous = sorted.every((n, i) => i === 0 || n === sorted[i - 1] + 1);
  return contiguous && sorted.length > 2 ? `${sorted[0]}–${sorted[sorted.length - 1]}` : sorted.join(', ');
}

function applyCall(spelling, entry) {
  return (view, completion, from, to) => {
    const next = view.state.sliceDoc(to, to + 1);
    if (next === '(') {
      view.dispatch({ changes: { from, to, insert: spelling }, selection: { anchor: from + spelling.length + 1 } });
      return;
    }
    const takesNone = entry.arities.length === 1 && entry.arities[0] === 0;
    const insert = spelling + '()';
    view.dispatch({
      changes: { from, to, insert },
      selection: { anchor: from + spelling.length + (takesNone ? 2 : 1) },
    });
  };
}

function entryOptions(entries) {
  const options = [];
  for (const entry of entries) {
    for (const spelling of [entry.name, ...entry.aliases]) {
      options.push({
        label: spelling,
        type: 'method',
        detail: entry.category || entry.receiver,
        info: () => docNode(entry, spelling),
        apply: applyCall(spelling, entry),
        boost: spelling === entry.name ? 1 : 0,
      });
    }
  }
  return options;
}

/** The saved queries as completion options of `g.query("`, grouped by folder. */
function savedQueryOptions(list) {
  return queryTree(list).flatMap((group) => group.queries.map((q) => ({
    label: q.name,
    type: 'function',
    detail: q.status === 'error' ? 'fails' : signatureText(q).slice(q.name.length),
    info: [q.description, q.status === 'error' ? `It currently fails: ${q.error}` : null].filter(Boolean).join('\n') || undefined,
    section: group.folder ? { name: group.folder, rank: 1 } : { name: 'saved queries', rank: 0 },
  })));
}

/**
 * @param {() => ({catalogue: Array<object>, tokens: Object<string, string[]>}|null)} getData  Returns the
 *   current completion data (`loadCompletionData`; may be null or empty while loading).
 * @param {() => Array<object>} [getQueries]  Returns the saved queries (`engine.listQueries()`), for `g.query("`.
 */
export function dslCompletionSource(getData, getQueries = () => []) {
  return (context) => {
    // The name of a saved query: `g.query("` lists them, grouped by folder.
    const line = context.state.doc.lineAt(context.pos);
    const named = queryNameContext(context.state.sliceDoc(line.from, context.pos));
    if (named) {
      const list = getQueries() || [];
      if (!list.length) return null;
      return {
        from: context.pos - named.prefix.length,
        options: savedQueryOptions(list),
        validFor: /^[A-Za-z0-9_]*$/,
      };
    }
    const word = context.matchBefore(/[A-Za-z_][A-Za-z0-9_]*/);
    const from = word ? word.from : context.pos;
    if (!word && !context.explicit) {
      // Only open on '.' / '::' without a typed prefix.
      const before = context.state.sliceDoc(Math.max(0, context.pos - 2), context.pos);
      if (!/(\.|::)$/.test(before)) return null;
    }
    const before = context.state.sliceDoc(Math.max(0, from - 120), from);
    if (countQuotes(before.split('\n').pop() || '') % 2 === 1) return null; // inside a string literal
    const access = before.match(/([A-Za-z_][A-Za-z0-9_]*)?\s*(\.|::)\s*$/);
    const data = getData() || {};
    const catalogue = data.catalogue || [];
    const tokens = data.tokens || {};

    if (access) {
      const ident = access[1];
      const choices = ident ? tokenChoices(ident, tokens, catalogue) : null;
      if (choices) {
        return {
          from,
          options: [
            ...choices.constants.map((c) => ({
              label: c.label,
              type: 'constant',
              detail: ident,
              info: c.entry ? () => docNode(c.entry, c.label, true) : undefined,
            })),
            ...entryOptions(choices.entries),
          ],
          validFor: /^[A-Za-z0-9_]*$/,
        };
      }
      const receiver = (ident && RECEIVER_OF[ident]) || 'traversal';
      let entries = catalogue.filter((e) => e.receiver === receiver);
      if (receiver === 'TextP') entries = entries.concat(catalogue.filter((e) => e.receiver === 'P' && /ing|not/.test(e.name)));
      return { from, options: entryOptions(entries), validFor: /^[A-Za-z0-9_]*$/ };
    }

    if (!word && !context.explicit) return null;
    return {
      from,
      options: [
        ...globals(tokens).map((o) => ({ ...o, type: 'variable', boost: o.label === 'g' ? 2 : 0 })),
        ...KEYWORDS.map((k) => ({ label: k, type: 'keyword' })),
        // Free functions: jpath(..), UUID(), bare predicates such as gt(30).
        ...entryOptions(catalogue.filter((e) => e.receiver === 'global')).map((o) => ({ ...o, type: 'function', boost: -1 })),
      ],
      validFor: /^[A-Za-z0-9_]*$/,
    };
  };
}

function countQuotes(s) {
  let n = 0;
  for (let i = 0; i < s.length; i++) if (s[i] === '"' && s[i - 1] !== '\\') n++;
  return n;
}
