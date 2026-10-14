// The global names of the DSL scope the highlighter colours as token classes (and completion
// offers as globals), without CodeMirror so node tests can load it. The fixed list below is the
// fallback while the engine's data is not loaded yet; `learnTokenClasses` adds every class the
// engine reports (`engine.tokenValues()`, called by `loadCompletionData`), so a token class the
// engine gains is highlighted without a change here.

/**
 * The sources, the token classes and the other global names of the DSL scope. A live set: the
 * classes of the engine are added to it once its data is loaded.
 */
export const TOKEN_CLASSES = new Set([
  'g', '__', 'P', 'TextP', 'T', 'Order', 'Scope', 'Column', 'Pop', 'Cardinality', 'Direction',
  'ProfileType', 'GType', 'Operator', 'Barrier', 'Merge', 'Pick', 'By', 'V', 'UUID', 'CastPolicy',
  'Set', 'GraphSource', 'SchemaMode', 'WithOptions', 'DT',
]);

/**
 * Adds the token classes of `tokens` (`engine.tokenValues()`: class -> values) to
 * `TOKEN_CLASSES`. Only identifiers are taken. Returns the number of classes added.
 * @param {Object<string, string[]>} tokens
 */
export function learnTokenClasses(tokens) {
  let added = 0;
  for (const name of Object.keys(tokens || {})) {
    if (/^[A-Za-z_][A-Za-z0-9_]*$/.test(name) && !TOKEN_CLASSES.has(name)) {
      TOKEN_CLASSES.add(name);
      added++;
    }
  }
  return added;
}
