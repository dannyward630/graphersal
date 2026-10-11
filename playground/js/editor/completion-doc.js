// The doc text of a completion entry, as the engine's function docs write it: one sentence, an
// empty line, then `Example: `query`` with ` returns `result`` or ` fails with `message``.
// Kept free of browser and CodeMirror imports so the node tests can load it.

/**
 * Splits a doc text into its description and its example line (`query  → result`).
 * @param {string} doc
 * @returns {[string, string | null]}
 */
export function splitExample(doc) {
  const at = doc.indexOf('\n\nExample: `');
  if (at < 0) return [doc, null];
  const rest = doc.slice(at + '\n\nExample: '.length);
  const m = rest.match(/^`(.*?)`(?: returns `(.*)`| fails with `(.*)`)?$/s);
  if (!m) return [doc.slice(0, at), rest];
  const outcome = m[2] !== undefined ? `  → ${m[2]}` : m[3] !== undefined ? `  → error: ${m[3]}` : '';
  return [doc.slice(0, at), m[1] + outcome];
}
