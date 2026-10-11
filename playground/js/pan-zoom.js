// Small pan/zoom controller for a static drawing (the Mermaid SVG): the content element is
// transformed (translate + scale) inside a clipping container.
//   - pinch on a trackpad (wheel events with ctrlKey) or Ctrl/Cmd + wheel: zoom around the pointer
//   - Safari trackpad pinch (gesture events): zoom
//   - two-finger scroll / wheel: pan
//   - drag with the primary button or one finger: pan
//   - double click: fit
// No library: a few dozen lines are enough and need no extra CDN dependency.

const MIN_SCALE = 0.05;
const MAX_SCALE = 8;
const MAX_FIT_SCALE = 1.6; // a tiny diagram is not blown up beyond this when fitted
const FIT_PADDING = 24;

/**
 * @param {HTMLElement} container  Clipping viewport (overflow hidden, position relative).
 * @param {HTMLElement} content    Element to transform; its untransformed size is the drawing size.
 * @returns {{fit: () => void, destroy: () => void}}
 */
export function attachPanZoom(container, content) {
  let x = 0;
  let y = 0;
  let s = 1;
  let userMoved = false;
  let drag = null;
  let gestureStart = 1;

  content.style.transformOrigin = '0 0';
  const apply = () => { content.style.transform = `translate(${x}px, ${y}px) scale(${s})`; };
  const clamp = (v) => Math.min(MAX_SCALE, Math.max(MIN_SCALE, v));

  function zoomAt(px, py, next) {
    const ns = clamp(next);
    x = px - (px - x) * (ns / s);
    y = py - (py - y) * (ns / s);
    s = ns;
    userMoved = true;
    apply();
  }

  function size() {
    return { w: content.offsetWidth, h: content.offsetHeight, cw: container.clientWidth, ch: container.clientHeight };
  }

  function fit() {
    const { w, h, cw, ch } = size();
    if (!w || !h || !cw || !ch) return; // not laid out yet (hidden or collapsed panel)
    s = Math.min(MAX_FIT_SCALE, Math.max(MIN_SCALE, Math.min((cw - 2 * FIT_PADDING) / w, (ch - 2 * FIT_PADDING) / h)));
    x = (cw - w * s) / 2;
    y = (ch - h * s) / 2;
    userMoved = false;
    apply();
  }

  function onWheel(e) {
    e.preventDefault();
    const rect = container.getBoundingClientRect();
    if (e.ctrlKey || e.metaKey) {
      zoomAt(e.clientX - rect.left, e.clientY - rect.top, s * Math.exp(-e.deltaY * 0.01));
    } else {
      x -= e.deltaX;
      y -= e.deltaY;
      userMoved = true;
      apply();
    }
  }

  function onGestureStart(e) { e.preventDefault(); gestureStart = s; }
  function onGestureChange(e) {
    e.preventDefault();
    const rect = container.getBoundingClientRect();
    zoomAt(e.clientX - rect.left, e.clientY - rect.top, gestureStart * e.scale);
  }

  function onPointerDown(e) {
    if (e.button !== 0) return;
    drag = { id: e.pointerId, sx: e.clientX, sy: e.clientY, x, y };
    container.setPointerCapture(e.pointerId);
    container.classList.add('dragging');
  }
  function onPointerMove(e) {
    if (!drag || e.pointerId !== drag.id) return;
    x = drag.x + (e.clientX - drag.sx);
    y = drag.y + (e.clientY - drag.sy);
    userMoved = true;
    apply();
  }
  function onPointerUp(e) {
    if (!drag || e.pointerId !== drag.id) return;
    drag = null;
    container.classList.remove('dragging');
  }

  // Refit when the panel is resized or becomes visible, unless the user moved the view by hand.
  let wasHidden = !container.clientWidth || !container.clientHeight;
  const observer = new ResizeObserver(() => {
    const hidden = !container.clientWidth || !container.clientHeight;
    if (!hidden && (wasHidden || !userMoved)) fit();
    wasHidden = hidden;
  });

  container.addEventListener('wheel', onWheel, { passive: false });
  container.addEventListener('gesturestart', onGestureStart);
  container.addEventListener('gesturechange', onGestureChange);
  container.addEventListener('pointerdown', onPointerDown);
  container.addEventListener('pointermove', onPointerMove);
  container.addEventListener('pointerup', onPointerUp);
  container.addEventListener('pointercancel', onPointerUp);
  container.addEventListener('dblclick', fit);
  observer.observe(container);

  return {
    fit,
    destroy() {
      observer.disconnect();
      container.removeEventListener('wheel', onWheel);
      container.removeEventListener('gesturestart', onGestureStart);
      container.removeEventListener('gesturechange', onGestureChange);
      container.removeEventListener('pointerdown', onPointerDown);
      container.removeEventListener('pointermove', onPointerMove);
      container.removeEventListener('pointerup', onPointerUp);
      container.removeEventListener('pointercancel', onPointerUp);
      container.removeEventListener('dblclick', fit);
    },
  };
}
