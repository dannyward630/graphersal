// When the Schema panel reads the schema again. Pure logic over a `SchemaFlow` (flow.js) and the
// panel's drawings; no Vue, no DOM (tested under Node).
//
// Inferring a schema walks the whole graph (on `large` ~0.7 s of engine time and ~18 MB through
// the dev server), so it never runs on its own after a change of the graph:
// - FIRST SHOW: the panel infers (when the graph stores no schema) the first time it is shown,
//   i.e. visible and expanded; a collapsed or hidden panel reads nothing.
// - REFRESH (the panel's refresh button): reads and infers again, clears "may be outdated".
// - A CHANGE of the graph (a query write, the property editor, a schema apply, a change made
//   elsewhere: `graphVersion` only): the stored schema is read again (cheap and authoritative,
//   so a new or changed stored schema shows at once); without one the panel keeps the schema
//   inferred last and the flow is `outdated` (the badge).
// - A REPLACED graph (a load, Stop's engine restart, a read-only view, back to live, a rollback,
//   an attic restore, a database switch: `graphEpoch` changes too) is treated as a first show:
//   the shown schema belongs to another graph, so it is dropped and the panel infers once, now
//   when it is shown, else when it is shown next.

export class SchemaPanelSync {
  /**
   * @param {import('./flow.js').SchemaFlow} flow
   * @param {{load: (force: boolean) => Promise<void>, drop: () => void}} views the drawings:
   *   `load(true)` reads them again (or drops them when no drawing tab shows), `load(false)` only
   *   when they are cheap to read (a stored schema) or not read yet, `drop()` forgets them.
   * @param {number} epoch the store's `graphEpoch` now
   */
  constructor(flow, views, epoch) {
    this.flow = flow;
    this.views = views;
    this.epoch = epoch;
    this.active = false;
    this.pending = true; // the next show reads and infers (first show, replaced graph)
  }

  /** The panel became visible and expanded (`true`) or hidden/collapsed (`false`). */
  setActive(active) {
    this.active = !!active;
    if (this.active && this.pending) return this.refresh();
    return Promise.resolve();
  }

  /** `graphVersion` changed; `epoch` is the store's `graphEpoch` now. */
  graphChanged(epoch) {
    if (epoch !== this.epoch) {
      this.epoch = epoch;
      this.pending = true;
      if (this.active) return this.refresh();
      this.views.drop();
      return Promise.resolve();
    }
    // Never shown: the first show reads everything.
    if (this.pending) return Promise.resolve();
    // The drawings decide on the stored schema the flow has just read.
    return this.flow.load({ infer: false }).then(() => this.views.load(false));
  }

  /** First show, the refresh button and a replaced graph: read and infer again. */
  refresh() {
    this.pending = false;
    return Promise.all([this.flow.load({ infer: true }), this.views.load(true)]).then(() => {});
  }
}
