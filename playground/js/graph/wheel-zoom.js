// The Detailed tier's mouse-wheel zoom (js/graph/cytoscape-renderer.js). Pure: no DOM, tested
// under Node.
//
// Cytoscape's own wheel zoom takes a mouse wheel for an "inaccurate scroll device" and moves about
// 2.8 % per notch (0.7 % with the sensitivity the renderer used before): unusable on a large
// drawing. The Fast tier (sigma.js) zooms by a fixed ratio per wheel event, at most one step per
// direction every 50 ms, whatever the delta's size, and feels right on mice and trackpads alike;
// this does the same for scrolling, and follows a trackpad pinch (a wheel event with ctrlKey)
// proportionally, so a pinch stays smooth.

export const WHEEL_ZOOM = Object.freeze({
  /** The zoom ratio of one wheel step (one mouse notch). */
  step: 1.3,
  /** Steps in the same direction closer than this (ms) are one step: a trackpad's stream of small
   *  scroll events, or a fast-spinning wheel, do not race. */
  minIntervalMs: 50,
  /** A pinch's zoom per pixel of delta (natural log): 100 px ~ x2.7. */
  pinchPerPixel: 0.01,
  /** The largest ratio one pinch event may apply. */
  pinchMaxRatio: 2,
});

/** A wheel event's vertical delta in pixels (`deltaMode` lines or pages converted). */
export function wheelPixels(event) {
  const dy = Number(event.deltaY) || 0;
  if (event.deltaMode === 1) return dy * 16;
  if (event.deltaMode === 2) return dy * 400;
  return dy;
}

/**
 * The zoom ratio to apply for one wheel event (> 1 zooms in), or null to ignore it (no delta, or a
 * repeated step within `minIntervalMs`). `memo` ({last, direction}) carries the throttle state
 * between calls; `now` is the time in ms.
 * @param {{deltaY:number, deltaMode?:number, ctrlKey?:boolean}} event
 * @param {{last?:number, direction?:number}} memo
 * @param {number} now
 * @returns {number|null}
 */
export function wheelZoomRatio(event, memo, now) {
  const dy = wheelPixels(event);
  if (!dy) return null;
  if (event.ctrlKey) {
    const ratio = Math.exp(-dy * WHEEL_ZOOM.pinchPerPixel);
    return Math.min(WHEEL_ZOOM.pinchMaxRatio, Math.max(1 / WHEEL_ZOOM.pinchMaxRatio, ratio));
  }
  const direction = Math.sign(dy);
  if (memo.direction === direction && memo.last !== undefined && now - memo.last < WHEEL_ZOOM.minIntervalMs) return null;
  memo.direction = direction;
  memo.last = now;
  return direction > 0 ? 1 / WHEEL_ZOOM.step : WHEEL_ZOOM.step;
}
