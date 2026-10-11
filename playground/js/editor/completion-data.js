// The data behind the DSL completion (js/editor/completion.js), without CodeMirror so node tests
// can load it: the function catalogue (`engine.completions()`) and the token values
// (`engine.tokenValues()`, the members of `T.`, `Order.`, `GType.`, ... read from the getters the
// engine registers; the CLI REPL completes from the same source,
// `graphersal_session::views::token_values`). Loading it also teaches the highlighter the
// engine's token classes (`learnTokenClasses`).

import { learnTokenClasses } from './token-classes.js';

/**
 * Loads the completion data from `engine`. A call that fails leaves its part empty (completion
 * then offers less, the editor keeps working).
 * @returns {Promise<{catalogue: Array<object>, tokens: Object<string, string[]>}>}
 */
export async function loadCompletionData(engine) {
  const [catalogue, raw] = await Promise.all([
    engine.completions().catch(() => []),
    engine.tokenValues().catch(() => ({})),
  ]);
  const tokens = raw && typeof raw === 'object' && !Array.isArray(raw) ? raw : {};
  // The highlighter colours the engine's token classes from now on (token-classes.js).
  learnTokenClasses(tokens);
  return {
    catalogue: Array.isArray(catalogue) ? catalogue : [],
    tokens,
  };
}

/**
 * The choices after `ident.` when `ident` is a token class: its values as constants (each with
 * the catalogue entry of the same name and receiver, when there is one, for its doc tooltip), and
 * the class's other functions (`By.Property(..)` beside `By.Id`). `null` when `ident` is no token
 * class.
 * @param {string} ident
 * @param {Object<string, string[]>} tokens
 * @param {Array<object>} catalogue
 * @returns {{constants: Array<{label: string, entry: object|null}>, entries: Array<object>}|null}
 */
export function tokenChoices(ident, tokens, catalogue) {
  const values = tokens && Object.prototype.hasOwnProperty.call(tokens, ident) ? tokens[ident] : null;
  if (!Array.isArray(values) || !values.length) return null;
  const own = (catalogue || []).filter((e) => e.receiver === ident);
  const spellings = (e) => [e.name, ...(e.aliases || [])];
  const constants = values.map((label) => ({
    label,
    entry: own.find((e) => spellings(e).includes(label)) || null,
  }));
  const entries = own.filter((e) => !spellings(e).some((s) => values.includes(s)));
  return { constants, entries };
}

/**
 * The token class names to offer as globals: the fixed list of the highlighter plus every class
 * the engine reports, sorted.
 * @param {Iterable<string>} fixed
 * @param {Object<string, string[]>} tokens
 * @returns {string[]}
 */
export function tokenClassNames(fixed, tokens) {
  return [...new Set([...fixed, ...Object.keys(tokens || {})])].sort();
}
