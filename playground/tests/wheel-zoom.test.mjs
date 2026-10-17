// Node tests of the Detailed tier's wheel zoom (js/graph/wheel-zoom.js).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { wheelZoomRatio, wheelPixels, WHEEL_ZOOM } from '../js/graph/wheel-zoom.js';

test('one mouse notch is one step, whatever the delta size', () => {
  for (const deltaY of [100, 4, 1.5, 300]) {
    assert.equal(wheelZoomRatio({ deltaY }, {}, 0), 1 / WHEEL_ZOOM.step, `out ${deltaY}`);
    assert.equal(wheelZoomRatio({ deltaY: -deltaY }, {}, 0), WHEEL_ZOOM.step, `in ${deltaY}`);
  }
});

test('repeated events in one direction within the interval are one step', () => {
  const memo = {};
  assert.equal(wheelZoomRatio({ deltaY: -3 }, memo, 0), WHEEL_ZOOM.step);
  assert.equal(wheelZoomRatio({ deltaY: -3 }, memo, 20), null);
  assert.equal(wheelZoomRatio({ deltaY: -3 }, memo, 60), WHEEL_ZOOM.step);
  // a change of direction is never held back
  assert.equal(wheelZoomRatio({ deltaY: 3 }, memo, 61), 1 / WHEEL_ZOOM.step);
});

test('a pinch follows its delta and is bounded', () => {
  const r = wheelZoomRatio({ deltaY: -10, ctrlKey: true }, {}, 0);
  assert.ok(r > 1 && r < 1.2, String(r));
  assert.equal(wheelZoomRatio({ deltaY: -5000, ctrlKey: true }, {}, 0), WHEEL_ZOOM.pinchMaxRatio);
  assert.equal(wheelZoomRatio({ deltaY: 0 }, {}, 0), null);
});

test('line and page deltas are converted to pixels', () => {
  assert.equal(wheelPixels({ deltaY: 3, deltaMode: 1 }), 48);
  assert.equal(wheelPixels({ deltaY: 1, deltaMode: 2 }), 400);
});
