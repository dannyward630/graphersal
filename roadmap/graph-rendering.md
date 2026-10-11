# Graph rendering at scale in the playground: design

Status: IMPLEMENTED in 0.1.0 in three steps (GR1-GR3 below): the Detailed and Fast tiers, the engine's
force layout, the meaningful selections. The optional Overview tier is not planned. DECIDED items are
settled design choices. The measured tuning lives in `playground/README.md` ("Graph drawing").

## 1. Starting point and steps

- Cytoscape.js 3.34 (cdnjs), Canvas 2D (`playground/js/components/cy-graph.js`).
- `graph_view` (graphersal-session `views.rs`) takes the FIRST N vertices (`vertices().take(n)`,
  default 300 vertices / 800 edges, Settings) and the edges between them; every vertex comes with
  ALL its properties as JSON (worker -> `postMessage` -> Vue).
- Layout `cose` on the main thread: 400-1000 iterations, quadratic per iteration.
- Labels with text outline on every vertex and edge, bezier edges with arrows.

Bottlenecks, largest first: the `cose` layout (seconds of frozen page from ~1000 vertices), Canvas
2D drawing of outlined text and curves per frame, shipping every property of every vertex, and
"first N" being an arbitrary slice of a large graph.

After GR1 (measured, `playground/README.md` "Graph drawing"): the drawing and a result's subgraph
are columns without properties (the inspector fetches one element), `cose` runs for at most 200
vertices (grid above), Cytoscape draws in three size tiers (full / reduced from 2000 / minimal from
5000 elements), defaults 1000 vertices and 2000 edges, confirmation above 5000. 3000 elements: 7.1 s
frozen before, 0.25 s to the first frame after. Cytoscape's WebGL option is not enabled (no gain
in the Detailed range).

After GR2: the panels draw through the renderer interface (`playground/js/graph/renderer.js`;
Cytoscape is the Detailed adapter, also for the schema graph); above the `cose` cap (and on
request at any size) the engine's Barnes-Hut layout (`graphersal-session` `layout.rs`) runs in
slices in the worker / on the dev server with progress and Stop, cached by the drawn topology;
labels get collision-free slots of a validated 12-colour palette. Layout of 1k / 10k / 50k
vertices: 0.34 / 2.4 / 8.8 s in wasm, 0.19 / 1.5 / 5.5 s native; the page's main thread stays
free meanwhile. SVG export is not there yet (`capabilities.svg: false`).

After GR3: the Fast tier exists (`playground/js/graph/sigma-renderer.js`: sigma.js 3.0.3 +
graphology 0.26.0 as cdnjs UMD builds with SRI, injected on first use, because their ESM builds
import the CommonJS `events` package by bare name). Calibrated thresholds: Fast from 2000 elements,
Detailed again below 1600 (`playground/js/graph/tiers.js`; Detailed's long frames start where its
`reduced` plan begins). Fast first frame 30-90 ms up to 50k elements, 215 ms at 100k; pan/zoom
within one or two 120 Hz frames up to 100k. Auto/Detailed/Fast switch kept in the settings,
positions survive a switch, banner and disabled controls from `capabilities`, no-WebGL fallback
(Detailed, 10k cap), drill down "Open in Detailed" on a neighbourhood of the drawing (no lasso: sigma
has none), PNG in both tiers. Edge labels in Fast show on vertex hover (no edge hover: sigma's edge
picking costs an extra pass; edges are clickable up to 20k edges). The toolbar Stop cancels a
running layout without restarting the engine.

## 2. Tiers (DECIDED: a detailed engine for small graphs, a fast one with fewer features above)

| Tier | Size (initial guess, calibrated by measurement) | Engine | Keeps | Loses |
|---|---|---|---|---|
| **Detailed** | up to ~2-3k elements | Cytoscape (optimized) | every style, vertex and edge labels, curved edges, arrows, every layout, the schema graph, SVG export | - |
| **Fast** | ~3k-200k | sigma.js v3 + graphology (WebGL) | vertex labels by zoom level, click + inspector, hover, colour by label, force layout | edge labels (hover only), curved edges, tree/concentric layouts, SVG export |
| **Overview** (optional, not queued) | > 200k | cosmos.gl (GPU) | points coloured by label, GPU layout, hover tooltip | labels, arrows, inspector |

The schema graph always uses Detailed.

## 3. Principles

1. **One renderer interface** (`setData`, `setPositions`, `setLayout`, `fit`, `select`, `onClick`,
   `onDoubleClick`, `exportPng`, `setTheme`, `destroy`, `capabilities`): the graph panel never
   knows which engine draws.
2. **Compact columnar data** from the engine: vertex ids, a label index per vertex, the label
   table, edge source/target as vertex indices, edge label indices (typed arrays, transferable).
   **Properties are fetched lazily** for the clicked element (inspector), the same in every tier.
3. **Layout separate from the renderer, computed in Rust** (Barnes-Hut force layout,
   ForceAtlas2-style, O(n log n)) in the engine: in the playground's Web Worker (wasm) and on the
   dev server, with progress and cancellation; positions come back as a `Float32Array`. Switching
   tiers keeps the positions (Cytoscape gets a preset layout). The main thread never lays out a
   large graph.
4. **Same palette and gestures** in every tier: colours from the same function, click = inspector,
   double click = expand neighbours.
5. **Threshold with hysteresis** (e.g. Fast from 3000 elements, back to Detailed below 2500), so
   the mode does not flap.
6. **Honest indicator and manual choice**: a "Detailed / Fast" switch in the panel and a banner
   naming what the current tier lacks; unavailable controls are disabled with a tooltip saying why;
   Detailed can be forced above the threshold with a warning.
7. **Drill down**: in Fast, select a vertex (or a lasso area) -> "Open in Detailed" loads that
   neighbourhood.
8. **Lazy loading**: sigma/graphology are loaded with `import()` only when needed; small graphs pay
   nothing.
9. **Fallback**: no WebGL -> Cytoscape with a hard limit and a notice.
10. **Meaningful selection instead of "first N"**: draw a query result / subgraph, the neighbourhood
    of a vertex with expansion on demand, and for a graph above the Fast range first the meta-graph
    (labels as nodes sized by count, from the statistics) with drill-down.

## 4. Constraints

- No bundler: every library must load as ESM or a classic script from the allowed CDNs
  (cdnjs, jsdelivr, unpkg) with exact versions and SRI where cdnjs offers it.
- No new Rust dependency for the layout without justification (rule 10): a quadtree Barnes-Hut is
  a few hundred lines.
- wasm32: no threads; the layout runs in iterations with a deadline and cancellation (the session's
  cancellation token), reporting progress.
- Measured, not guessed: each change is measured once (layout time and frame time for 1k, 10k,
  50k elements on the `large` graph or a `Fake` generated graph), no benchmark rounds.

