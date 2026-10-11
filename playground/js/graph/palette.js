// Label colours of the graph drawings (pure: no DOM; tested under Node).
//
// Collision-free: a label gets the next free SLOT the first time it is drawn in this page (the
// labels of a view are assigned in the order of its label table), and keeps it for the page's
// life, so two labels of one view never share a colour (up to `PALETTE_SIZE` labels) and a label
// has the same colour in the whole-graph drawing, a result drawing, the schema drawing and every
// tier. Labels beyond the palette share the neutral "other" colour; the legend says so.
//
// The 12 colours are a validated categorical palette (the first eight are the data-viz reference
// palette, four more hues appended), checked with the palette validator for lightness band,
// chroma floor, colour-blind separation and the normal-vision floor between neighbouring slots,
// in light mode against the light surface (#ffffff) and in dark mode against the dark surface
// (#181d25): every check passes in both modes, contrast against the dark surface >= 3:1 for all;
// three light-mode slots stay below 3:1 on white (aqua, yellow, pink), which the captions and
// the legend next to every drawing relieve (colour is never the only cue: each vertex shows its
// caption, the legend names the label). Each mode has its own steps of the same hues.

/** The light-mode colour of each slot (the light surface #ffffff). */
export const PALETTE_LIGHT = Object.freeze([
  '#2a78d6', // blue
  '#eb6834', // orange
  '#1baf7a', // aqua
  '#eda100', // yellow
  '#e87ba4', // pink
  '#008300', // green
  '#4a3aa7', // violet
  '#e34948', // red
  '#0097a7', // teal
  '#b5651d', // brown
  '#a43fb5', // purple
  '#6f8f00', // olive
]);

/** The dark-mode colour of each slot (the dark surface #181d25): the same hues, stepped. */
export const PALETTE_DARK = Object.freeze([
  '#3987e5',
  '#d95926',
  '#199e70',
  '#c98500',
  '#d55181',
  '#008300',
  '#9085e9',
  '#e66767',
  '#0097a7',
  '#b5651d',
  '#a43fb5',
  '#6f8f00',
]);

/** Distinct slots; more labels than this share `OTHER`. */
export const PALETTE_SIZE = PALETTE_LIGHT.length;

/** The colour of labels beyond the palette (light, dark). */
export const OTHER = Object.freeze({ light: '#8b94a3', dark: '#6b7585' });

/** The slot of the labels beyond the palette. */
export const OTHER_SLOT = PALETTE_SIZE;

/** The colour of `slot` in a theme (`OTHER_SLOT` and anything out of range: the other colour). */
export function slotColor(slot, dark = false) {
  const colors = dark ? PALETTE_DARK : PALETTE_LIGHT;
  return slot >= 0 && slot < colors.length ? colors[slot] : (dark ? OTHER.dark : OTHER.light);
}

/**
 * Label -> slot, assigned on first use and kept. One instance per page (`labelPalette`); tests
 * make their own.
 */
export class LabelPalette {
  constructor(size = PALETTE_SIZE) {
    this.size = size;
    /** @type {Map<string, number>} */
    this.slots = new Map();
  }

  /**
   * Gives every label of `labels` that has none yet the next free slot, in the given order (a
   * view's label table order); labels past the palette get `OTHER_SLOT`. Returns `this`.
   * @param {Iterable<string>} labels
   */
  assign(labels) {
    for (const raw of labels) {
      const label = String(raw ?? '');
      if (this.slots.has(label)) continue;
      const used = this.usedSlots();
      this.slots.set(label, used < this.size ? used : OTHER_SLOT);
    }
    return this;
  }

  /** How many distinct slots are taken. */
  usedSlots() {
    let n = 0;
    for (const slot of this.slots.values()) if (slot !== OTHER_SLOT) n++;
    return n;
  }

  /** The slot of `label` (assigned now when it has none). */
  slot(label) {
    const key = String(label ?? '');
    if (!this.slots.has(key)) this.assign([key]);
    return this.slots.get(key);
  }

  /** The colour of `label` in a theme. */
  color(label, dark = false) {
    return slotColor(this.slot(label), dark);
  }
}

/** The page's palette: every drawing (graph, result, schema, every tier) colours through it. */
export const labelPalette = new LabelPalette();

/**
 * The vertex labels of a compact graph view in the order of its label table (each once): what
 * `LabelPalette.assign` takes, so a view's labels get consecutive slots.
 * @param {{labels:string[], vertexLabel:ArrayLike<number>}} view
 * @returns {string[]}
 */
export function vertexLabelsInTableOrder(view) {
  if (!view || !view.labels) return [];
  const used = new Uint8Array(view.labels.length);
  for (let i = 0; i < view.vertexLabel.length; i++) used[view.vertexLabel[i]] = 1;
  const out = [];
  for (let k = 0; k < view.labels.length; k++) if (used[k]) out.push(view.labels[k]);
  return out;
}
