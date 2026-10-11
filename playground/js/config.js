// Site-level settings: links and the example queries offered in the editor.

/**
 * External links. `null` renders as "coming soon" until the public location exists.
 */
export const LINKS = {
  book: 'https://graphersal.dev/',
  repository: 'https://github.com/graphersal/graphersal',
};

/**
 * Example scripts, grouped (`group`), each written for one sample graph (`graph`: a sample kind, or
 * `"any"`). `expect: "error"` marks the examples that demonstrate an error. The list lives in
 * `examples.json` so that `graphersal-wasm`'s tests run every example against the engine.
 */
export { default as EXAMPLES } from './examples.json' with { type: 'json' };
