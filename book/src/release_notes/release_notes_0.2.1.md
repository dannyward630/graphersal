# Release 0.2.1

> **Warning: do not use 0.2.0 on a store written by 0.2.1.** Graphersal 0.2.0 reads a store that
> holds a compression rule with the codec `dedup` (new in 0.2.1) as **damaged** from the commit
> that defined the rule on: it opens it in maintenance mode, reports the commits from there on as
> lost, and `graphersal store repair` run from 0.2.0 would really drop them, although nothing on
> disk is damaged. Do not open such a store with 0.2.0 and never run `graphersal store repair`
> from 0.2.0 on it. Upgrade to 0.2.1; 0.2.0 is not recommended. From 0.2.1 on, what a newer
> version writes is never read as damage (see "Changed" below).

A patch release of the 0.2 line. It changes no format: stores, snapshots and journals of 0.2.0 are
read and written exactly as before. Its additions are additive API (a new `GraphStorage` method
with a default, new `#[non_exhaustive]` types).

## Added

- **Saved queries that write**
  ([#11](https://github.com/graphersal/graphersal/issues/11);
  [Queries that read and queries that write](../users_guide/saved_queries.md#queries-that-read-and-queries-that-write)):
  a saved query may now change data. Its **effects** (which elements of which labels it may
  create, update or delete) are inferred from its body when it is saved, never declared; a call
  asks the permission check for the whole effect set first and runs as one unit, so a failing
  call changes nothing. Schema, catalog and file writes, and writes the inference cannot see
  (behind `Fn`, `call`, `eval`, `import` or a computed query name), are refused when the query is
  saved. `g.queries()` lists `writes` and `effects`; the playground marks queries R/O or R/W and
  previews a writing call before it runs; the MCP server lists the effects, hides writing queries
  in its read-only mode, and the AI Chat runs one only after the user confirmed it. A writing query
  is stored in the dialect `graphersal-rhai/2`; read-only queries stay `graphersal-rhai/1`, so
  0.2.0 still runs them and lists a writing one as not runnable
  ([Unavailable queries and older versions](../users_guide/saved_queries.md#unavailable-queries-and-older-versions)).
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
- **A `dedup` codec for repeated values**
  ([#13](https://github.com/graphersal/graphersal/issues/13);
  [Deduplicating repeated values](../users_guide/compressed_properties.md#deduplicating-repeated-values)):
  `g.define_compression(#{name, label, path, codec: "dedup"})` stores every distinct value at the
  rule's path once, in a table of the rule, and every element keeps only a reference. For long
  values repeated across many elements (file paths, URLs, addresses, category texts); strings of at
  most 24 bytes are stored inline and never shared (the default `min_bytes` of a dedup rule is 25),
  and `dictionary: true` with `dedup` is refused with a help that names the fix. Reads stay plain
  and cost nothing extra, query results never change, change sets, exports and files hold plain
  values (no format change: a load shares the values again). `g.compressions()` reports the shared
  values (`compressed_values`), `distinct_values`, plain and stored bytes (each distinct text once
  plus 32 bytes); `g.memory_usage()` adds `deduped_values`, `deduped_plain_bytes` and
  `dedup_table_bytes` (each shared text once, part of `property_bytes`). The estimate measures
  dedup with `codec: "dedup"` or `codec: ["lz4", "dedup"]` (exact, never sampled; per candidate
  `distinct_values`; the warning `high_cardinality` when the values are mostly distinct) and
  includes a dedup rule that covers the path today. Rust `Codec::Dedup`,
  `CompressionCandidate::dedup`/`matching_codecs`, Python `define_compression(..., codec="dedup")`
  and `estimate_compression(..., codec=[...])`, the playground's codec choice and dedup rows in the
  estimate table, the MCP tools `define_compression`/`estimate_compression`. A 0.2.0 reader cannot
  decode a dedup rule (it reports the definition as damage, see the warning at the top): drop dedup
  rules before opening a store with 0.2.0.

## Changed

- **What a newer version writes is never damage**
  ([format spec 16 rule 9](../persistence/format.md#16-extensibility)): content behind a valid
  checksum that this version cannot interpret is kept or refused, never reported as damage and
  never offered for a repair that would drop it.
  - A compression rule whose `codec` or `element` this version does not know, or whose `path` it
    cannot parse, is kept and written back byte for byte through every store path (journal,
    checkpoint, fork, backup, ZIP, repair), not applied (its values stay plain), and listed with
    `served: false` and a `reason` by `g.compressions()`, Python, the session, MCP's
    `list_compressions` and the playground (a *not applied* badge); `g.recompress` refuses it
    with the reason. New: `CompressionDefinition::unserved_reason`, `codec_name`,
    `element_name`.
  - A property index of an unknown `element` kind is handled like one of an unknown `type`: kept,
    not served, listed with the reason, refused when created
    (`PropertyIndexDefinition::element_name`).
  - A saved query whose stored parameters this version cannot read (a schema keyword of a newer
    version) is kept, listed with the reason and refused when called
    (`QueryDefinition::unsupported_reason`).
  - A definition of a known kind whose payload fields this version cannot read is kept opaque,
    byte for byte, like a definition of an unknown kind.
  - A graph schema this version cannot parse (in a snapshot or a WAL record) opens the store
    read-only with the reason (the data without the schema); a checkpoint, fork or repair of it
    is refused, a backup copies it as it is.
  - An unknown interrupted operation (`INTENT`), an unknown `GRAPH` state value and a
    well-formed backup archive entry this version does not know are refused as unsupported.
  - A block compressed with zstd, read by a build without the `persist-zstd` feature, is
    reported as unsupported (format spec 2.1), no longer as damage.
  - New error `PersistError::UnsupportedContent` (*written by a newer Graphersal, not damage*);
    `graphersal store verify` lists such content as a `note:`, not as damage.

## Fixed

- **A traversal where a step takes values is refused**
  ([#9](https://github.com/graphersal/graphersal/issues/9)): an anonymous traversal given to an
  argument that takes ids, labels, property keys or values (`has_value(__.is("marko"))`,
  `has_label(__..)`, `out(__..)`, `values(__..)`, ...) was silently turned into a string and
  matched nothing. It is now an error whose help names the step that runs a traversal there.
  New: `has(T.id, traversal)` and `has(T.label, traversal)`, as in TinkerPop.
- **`g.memory_usage()` counts the hash index of every map.** It counted a property map, a nested
  object, a value map and the id and label indexes by their entries only and missed the hash
  table beside them, so `property_bytes` and `index_bytes` were 10-20 % low. They now match the
  bytes the allocator measures (the `large` sample graph: 74.2 MiB before, 92.0 MiB now). Nothing
  grew: the figure was wrong, not the memory.
