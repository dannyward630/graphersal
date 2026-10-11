// A one-shot readiness gate (pure, no Vue/DOM; tested under Node). The store opens it once the
// engine is created (or found missing); every engine-dependent action awaits it first, so an
// early call (a hash navigation to #/play while the WebAssembly module still loads) waits for
// the engine instead of failing on a null one.

/**
 * @returns {{ready: Promise<void>, open: () => void, isOpen: () => boolean}}
 */
export function createReadyGate() {
  let opened = false;
  let resolve;
  const ready = new Promise((r) => {
    resolve = r;
  });
  return {
    ready,
    open() {
      if (opened) return;
      opened = true;
      resolve();
    },
    isOpen: () => opened,
  };
}
