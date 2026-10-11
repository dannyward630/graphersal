// The engine layouts running on this page (pure: no DOM, no Vue; tested under Node).
//
// Each drawing whose engine layout runs registers a cancel function here; the toolbar's Stop
// (store.js `stopRun`) cancels them when no query runs, so the drawings keep the positions reached
// so far (like the panel's own Stop) instead of the engine being restarted. `onChange(count)`
// reports how many run (the store mirrors it in `state.layoutsRunning`, which enables Stop).

export class LayoutJobs {
  /** @param {(count:number) => void} [onChange] */
  constructor(onChange = () => {}) {
    this.onChange = onChange;
    /** @type {Map<number, () => void>} */
    this.jobs = new Map();
    this.next = 0;
  }

  /** How many layouts run. */
  get size() {
    return this.jobs.size;
  }

  /**
   * Registers a running layout; returns its token for `remove`.
   * @param {() => void} cancel   Stops it (keeping the positions reached so far).
   */
  add(cancel) {
    const token = ++this.next;
    this.jobs.set(token, cancel);
    this.onChange(this.jobs.size);
    return token;
  }

  /** The layout of `token` ended (finished, cancelled or superseded). Unknown tokens are ignored. */
  remove(token) {
    if (this.jobs.delete(token)) this.onChange(this.jobs.size);
  }

  /** Cancels every running layout; returns how many were asked to stop. */
  cancelAll() {
    const cancels = [...this.jobs.values()];
    for (const cancel of cancels) {
      try {
        cancel();
      } catch (err) {
        // A drawing that is gone: nothing left to stop.
      }
    }
    return cancels.length;
  }
}
