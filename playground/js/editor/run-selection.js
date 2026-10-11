// Which text a Run executes, and how the engine's error positions map back into the editor.
//
// Run executes the editor's main selection when it holds more than whitespace, else the whole
// script. The engine sees the text trimmed, so its line 1, column 1 is the first non-blank
// character of what ran; `offset` is where that character stands in the full editor text, and
// `mapPosition` moves a reported `{line, column}` (1-based) by it. Pure: no CodeMirror, no DOM.

/** The 1-based `{line, column}` of the character at `index` in `text` (columns in UTF-16 units, like the editor). */
export function positionAt(text, index) {
  let line = 1;
  let lineStart = 0;
  for (let i = 0; i < index; i++) {
    if (text.charCodeAt(i) === 10) {
      line++;
      lineStart = i + 1;
    }
  }
  return { line, column: index - lineStart + 1 };
}

/**
 * The text a Run executes. `doc` is the full editor text, `from`/`to` the main selection's range.
 * Returns `{ text, selection, lines, offset }`: `text` trimmed (what the engine gets), `selection`
 * true when it is the selection rather than the whole script, `lines` the number of lines of
 * `text`, `offset` the `{line, column}` of its first character in `doc`.
 */
export function runText(doc, from = 0, to = 0) {
  const lo = Math.max(0, Math.min(from, to, doc.length));
  const hi = Math.min(doc.length, Math.max(from, to));
  const picked = hi > lo && doc.slice(lo, hi).trim() !== '';
  const start = picked ? lo : 0;
  const raw = picked ? doc.slice(lo, hi) : doc;
  const text = raw.trim();
  const lead = raw.length - raw.trimStart().length;
  const lines = text ? text.split('\n').length : 0;
  return { text, selection: picked, lines, offset: positionAt(doc, start + lead) };
}

/**
 * A position the engine reported for the text that ran, moved into the full editor text: lines
 * shift by the offset's line, and columns on the first line by its column. A missing position or
 * offset passes through unchanged.
 */
export function mapPosition(position, offset) {
  if (!position || !offset) return position;
  const line = Number(position.line);
  const column = Number(position.column);
  if (!Number.isFinite(line) || !Number.isFinite(column)) return position;
  return {
    ...position,
    line: line + offset.line - 1,
    column: line === 1 ? column + offset.column - 1 : column,
  };
}

/**
 * An engine error of the text that ran, moved into the full editor text: its `position` and the
 * `(line L, position C)` Rhai writes into `message` and `diagnostic`. Returns a new object (or
 * `error` itself when there is nothing to move).
 */
export function mapErrorPosition(error, offset) {
  if (!error || !error.position || !offset) return error;
  const moveText = (text) => (typeof text !== 'string' ? text
    : text.replace(/\(line (\d+), position (\d+)\)/g, (all, l, c) => {
      const p = mapPosition({ line: Number(l), column: Number(c) }, offset);
      return `(line ${p.line}, position ${p.column})`;
    }));
  const out = { ...error, position: mapPosition(error.position, offset) };
  if ('message' in error) out.message = moveText(error.message);
  if ('diagnostic' in error) out.diagnostic = moveText(error.diagnostic);
  return out;
}

/** The status text of a partial run: `selection (2 lines)`. */
export function selectionText(lines) {
  return `selection (${lines} line${lines === 1 ? '' : 's'})`;
}
