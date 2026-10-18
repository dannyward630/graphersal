# Release 0.2.1

A patch release of the 0.2 line. It changes no format: stores, snapshots and journals of 0.2.0 are
read and written exactly as before. Its additions are additive API (a new `GraphStorage` method
with a default, new `#[non_exhaustive]` types).

## Added

- **Estimate a compression rule before defining it**
  ([#12](https://github.com/graphersal/graphersal/issues/12);
  [Estimating the gain](../users_guide/compressed_properties.md#estimating-the-gain)):
  `g.estimate_compression(#{label, path, element?, codec?, min_bytes?, dictionary?})` measures the
  candidate rules (`lz4` without and with a dictionary at `min_bytes` 64 to 1024, plus the rule that
  covers the path today) on the current values and reports per candidate the values it would
  compress, plain, stored and dictionary bytes, the net saving, the ratio and the measured
  compression and read (decompression) speed, plus the values found at the path, how many are
  strings, a size histogram to choose `min_bytes`, the best candidate and warnings for a label
  that does not exist, a path without values and values that are not strings. The graph is not
  changed (no commit, no catalog change). The trial dictionary is built by the rule's own code, so
  the estimate of a candidate is what the rule holds after it is defined on the same data; above
  100,000 strings or 16 MiB of text a deterministic sample is measured and extrapolated
  (`sampled: true`). It asks for `Read` on the label's data, so read-only policies, the MCP
  read-only mode and saved queries may estimate. Rust
  `GraphTraversalSource::estimate_compression(&CompressionEstimateRequest)` (`graphersal::catalog`;
  storages implement `GraphStorage::estimate_compression`, default `Unsupported`), Python
  `Graph.estimate_compression(label, path, ...)`, the playground's **Estimate** button in the
  compression rule form (a table of the candidates with saving bars, the best highlighted, **Use**
  fills the form), the dev server's `POST /api/compression/estimate` and the MCP tool
  `estimate_compression`. The help of an out-of-range `min_bytes` names it.
