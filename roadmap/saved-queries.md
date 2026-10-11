# Saved queries: design

Status: IMPLEMENTED in 0.1.0 (user documentation: `book/src/users_guide/saved_queries.md`; format:
`book/src/persistence/format.md` sections 6.3, 9.3 and 16). Items marked DECIDED are settled design
choices; the rest describes the design as proposed, refined by the implementation. When this design
and the code disagree, the code wins.

## 1. Summary

A **saved query** is a named, parameterized piece of DSL stored IN the database, next to the
schema: written once, called by name from any front end.

```rhai
g.query("top_customers", #{country: "DE", limit: 50}).values("name")
```

- One concept for both shapes (DECIDED: one name, `query`): the call runs the stored function
  and returns what it returns. A traversal result can be continued (like a SQL view); a value
  result (number, list, map) is terminal (like a stored procedure).
- **Read-only, always** (DECIDED): a saved query never writes, not even through a
  nested saved query; enforced by the authorizer (section 4). Writing stored code is a separate
  definition kind for later ("procedure").
- Parameters are named and typed (section 3); the playground builds a form from them.
- Organized in folders (DECIDED: variant B, folder = attribute), with an optional description
  (DECIDED).
- Persisted losslessly in the native binary format (DECIDED: requirement), through a catalog of
  definitions (section 6).

Why not the names considered:

| Name | Rejected because |
|---|---|
| `view` | Promises SQL-view semantics (a continuable SELECT); a saved query may also return a plain value |
| `call` | A TinkerPop 3.7+ step (provider services) |
| `run`, `exec` | Collide with our `run()` / `execute()` |
| `procedure` | Kept for the future WRITING kind; a saved query's traversal result can be continued |
| `macro` | Suggests text substitution; parameters are values |

In the UI the entity is called **Saved queries** (the playground already has query tabs).

## 2. Definition

The body is the text of ONE Rhai function; its parameters are declared beside it (Rhai
functions have no default values, so the signature cannot carry them).

```rhai
g.define_query(#{
    name: "top_customers",
    folder: "reports/sales",
    description: "Customers of a country by revenue.",
    params: #{
        country: #{ schema: #{type: "string", enum: ["DE", "FR", "US"],
                              description: "ISO country code"} },
        limit:   #{ schema: #{type: "integer", minimum: 1, maximum: 1000},
                    default: 20 }
    },
    body: `fn top_customers(country, limit) {
        g.v().has_label("customer").has("country", country)
         .order().by("revenue", Order.desc).limit(limit)
    }`
})
```

Checked at definition (the definition is refused otherwise):

- the body parses and defines exactly one function named like the query;
- the parameter list equals the function's signature;
- parameter names are not reserved (`RESERVED_PARAM_NAMES`: `g`, `__`, `P`, `T`, ...);
- every parameter schema passes the schema parser; every `default` validates against its schema;
- saved queries called by the body exist, and the call graph has no cycle (a call depth bound
  applies at run time too).

Alternative spelling for several queries at once, descriptions from doc comments (Rhai's
`metadata` feature, already on):

```rhai
g.define_queries("reports/sales", `
    /// Customers with an order in the last N days, newest first.
    fn active_customers(days) { ... }
`, #{ active_customers: #{ days: #{ schema: #{type: "integer"}, default: 30 } } })
```

Management: `g.queries()` / `g.queries("reports/")` (list with folder, description, parameters,
status), `g.get_query(name)`, `g.drop_query(name)`, `g.move_query(name, folder)`,
`g.describe_query(name, text)`. Both spellings as usual (`define_query`/`defineQuery`).

## 3. Parameters

- **Typed**, with the node format of the schema (`PropertySchema`): `type`, `enum`, `minimum`,
  `maximum`, `pattern`, `minLength`, `items`, ..., annotations `title`/`description`/`examples`.
  Short form `params: #{limit: "integer"}` for a bare type.
- **Default outside the schema**, on the parameter (`{schema, default}`): in a data schema
  `default` is annotation-only (project rule), and its meaning must not be mixed. A parameter
  default IS substituted when the argument is omitted. A parameter without a default is required.
- **Named arguments**: `g.query("x", #{limit: 5})`. Reordering parameters does not break callers,
  and a form produces a named set naturally. Named only (section 9).
- **Values only**: an argument is a data value (never a traversal, closure or code), bound like a
  prepared-statement parameter, never spliced into text: no injection.
- **Errors** with `help()`: unknown parameter, missing required parameter, wrong type or a
  violated constraint (`limit expects int64 from 1 to 1000, got 5000`). The only implicit
  conversion inside the DSL is `int64` -> `float64` (as on a schema write).
- **Coercion at the boundary**: the playground form, HTTP and MCP deliver text or JSON; they
  convert to the declared type before the call. Inside the DSL types are exact.
- A `limit` parameter inside the query differs from a caller's `.limit()`: it matters after an
  `order()` or for paging (`range(offset, offset + limit)`); a caller can always add its own.

## 4. Read-only (DECIDED)

A saved query never writes the graph or the schema. The reasoning: a query may call other
saved queries, so whether a call writes cannot be known in general; making the whole kind
read-only removes the question.

- **Enforced, not trusted**: the call runs under a read-only authorizer (Read only; Create,
  Update, Delete on Data and Schema and the catalog denied). Nested saved queries inherit it, so
  no chain of calls can write.
- A traversal **returned** by a query carries the restriction: the caller cannot continue it with
  a mutating step (`g.query("x").property("k", 1)` fails with `PermissionDenied`); the caller
  writes in a separate traversal.
- The pre-run analysis stays exact and simple: `is_mutating()` of anything built from a query
  result is false, a read-only host (MCP `--mcp-read-only`) can offer every saved query, the
  playground needs no confirmation before running one.
- Authorization: calling asks `Execute` on the query (new resource, section 7) plus the body's
  own read requests. "Definer rights" (run a query without raw data access) is out of scope.
- **Later**: a writing kind ("procedure", its own definition kind in the catalog, section 6) with
  explicit unit semantics. Not part of 0.1.0.

## 5. Folders and descriptions (DECIDED)

- **Folder is an attribute, not part of the name** (variant B): names are unique per database,
  a call uses the name only. Moving a query or renaming a folder never breaks a caller; a folder
  rename is several catalog changes in one commit.
- Folders are implicit (like directories in git): a folder exists while something is in it. A
  folder description, if ever wanted, becomes a definition kind of its own.
- Path rules: `/`-separated, non-empty segments, no leading or trailing `/`, trimmed, bounded
  depth and length. Case sensitivity of names and folders: section 9.
- **Description**: optional, the same keyword as in the schema (`description`, maybe `title`).
  Shown in the playground tree and completion, as the MCP tool description, and in `queries()`.
- Tags (several groups per query) can come later in `meta`; they do not conflict with folders.

## 6. Persistence: a catalog of definitions

The manifest's variable part already reserves item 3, **Definitions** (`uvar` count, "reserved for
property index definitions; MUST be 0 in version 1", `persist/manifest.rs`). It becomes a typed,
self-describing table: one mechanism for saved queries now and for indexes, folder descriptions,
procedures or anything else of that sort later (DECIDED: extensible without format migrations).

### 6.1 Layout

```text
definitions = uvar count | definition*
definition  = kind u8 | flags u8 | name string | length uvar | payload (length bytes)
flags       = bit 0: critical (a reader that does not know `kind` must not write the store);
              bits 1-7 reserved, zero, preserved
payload     = property map (the codec's tagged value encoding, keys inline)
```

Kinds: `1` saved query; `2` reserved for property indexes; `3` compression rules
(`roadmap/compressed-properties.md`); `4`-`127` unassigned; `128`-`255` host kinds, never assigned by the
library (a host application's own definitions, stored opaque).

A saved query's payload map:

| Key | Value |
|---|---|
| `folder` | string, absent = root |
| `description` | string, absent = none |
| `dialect` | string, `"graphersal-rhai/1"` |
| `source` | string: the Rhai function text |
| `params` | string: canonical JSON, name -> `{schema, default?}` |
| `meta` | string, free, never interpreted |

Payload keys MUST NOT be `kind`, `name` or `flags`: those are the envelope keys of a
definition's flat JSON form (DECIDED). Unknown keys are preserved when read from
stored or exchanged data and rejected in user input (typos are caught) (DECIDED).

### 6.2 Extensibility rules (become a section of the format spec)

1. **Length before payload, payload as a map.** A reader skips an unknown kind by its length and
   ignores unknown keys inside a known kind's map; a new field is a new key, never a new layout.
2. **Unknown, not critical**: kept as opaque bytes (kind, flags, name, payload) and written back
   unchanged at the next checkpoint and in every copy (fork, backup, repair), so an older binary
   never drops a newer definition. Unknown keys of a known kind are preserved the same way.
3. **Unknown and critical**: the store opens read-only (a definition that must stay consistent
   with the data, such as an index, would be corrupted by a writer that does not maintain it).
4. **WAL**: ONE commit-payload op for every kind, so new kinds need no new op and no new version:
   `10 | SetDefinition | kind u8 | name string | before opt(definition) | after opt(definition)`
   (define: no before; drop: no after; change, move, describe: both). Replay of an unknown kind
   follows rules 2 and 3.
5. **Large data** belongs in files of its own, not in the manifest (whose section is read whole
   and bounded at 256 MiB): a definition in the catalog plus files in the snapshot directory,
   referenced from the segment table by a new segment `kind`, with the same critical/ignorable
   rule for unknown segment kinds. Packed snapshots get a generic section kind "named extra
   file" (name + bytes), so a new file type needs no new section kind. (Specified in the format
   spec; no file type uses it yet.)
6. **Two version axes**: the format version (byte structure; with rules 1-5 it rarely moves) and
   content versions such as `dialect` (DSL text evolves by compiling the text, never by
   re-encoding files).
7. **No migration tools**: a reader reads its own and every older version; an older store opens
   read-only until an upgrade writes a checkpoint in the current version (spec section 15).
   Migration is a checkpoint.

Pre-release, so all of this extends format version 1 in place.

### 6.3 Everything else

- **Engine**: `Mutation::SetDefinition` in `ChangeSet` (CDC sees catalog changes; serde), an
  `UndoEntry` (rollback, savepoints, `dry_run`), a case in `graph/capture.rs`, replay in
  `TraversalGraph::replay`, `apply_change_set`. A catalog change is a unit like `set_schema`.
- **Recovery and repair**: the catalog comes from the base snapshot's manifest, else from an older
  snapshot plus the WAL's `SetDefinition` ops (as the schema does, format spec 13.4); `verify`
  covers it. Rollback, attic, fork, backups (incremental too) and ZIP need nothing new beyond
  reading and writing the manifest and the WAL.
- **Store the source text, never a compiled plan**: internal steps, step ids and optimizer rules
  change between versions; the text is the stable contract, and every call plans anew (and gets
  new optimizations). A query that no longer compiles does NOT make the store unopenable: it is
  listed with its error and fails when called.
- **Exchange formats**: GraphML and GraphSON carry no catalog (like the schema); `.gsnap` carries
  it losslessly. The playground's **Catalog** menu saves and loads the whole catalog (every kind)
  as one JSON file.
- Spec: `book/src/persistence/format.md` sections 6.2, 7/8 (segment and section kinds), 9.3, 15
  and a new section "Extensibility".

## 7. Engine and API surface

- `GraphStorage`: `get_definitions`/`set_definition` with default `Unsupported` (rule 15: a new
  trait method has a default). `TraversalGraph` keeps the catalog beside the schema.
- `Resource::Query { name }` (or a general `Resource::Definition`) in the authorization model:
  `Execute` to call, `Create`/`Update`/`Delete` to manage, `Read` to list.
- DSL: `g.query(name, params)` is a source method that evaluates the stored function in a scope
  with `g`, `__` and the parameters, then returns its result (a traversal is retargeted to the
  caller's source, like a returned traversal of a whole-script unit). A compiled-function cache
  per graph, invalidated by catalog changes.
- Profile and errors name the origin: `query("top_customers") › has("country", "DE")`; step ids
  stay plan positions.
- Mid-traversal use (`g.v(1).query("friends_of_friends")`) is a follow-up (section 9). Its natural
  form splices the body's anonymous traversal into the caller's plan at lowering time (parameters
  are constants of the call), so the pre-run analysis sees every step.
- **Mutation analysis**: saved queries are read-only (section 4); a returned traversal carries the
  read-only restriction, so nothing built from a query result can write.

## 8. Front ends

- **Playground: full management (DECIDED)**: create, edit, run and delete saved
  queries in the UI. An editor dialog (name, folder, description, the parameter list
  with type, constraints, default and description per parameter, and the body in the code editor
  with the DSL completion); saving calls the define/replace operation and shows its validation
  errors in place; delete asks first; move = changing the folder in the editor. Works on the
  WebAssembly playground and on the dev server alike (session methods).
- **Playground: browsing and running**: a Saved queries tree (folders, descriptions as tooltips, error status);
  "Run..." opens a form built from the parameter schemas (number fields with min/max, a select
  for `enum`, a checkbox for boolean, UUID text with validation, a list for arrays; defaults
  prefilled, required fields marked, descriptions as hints). The form **writes the query text**
  (`g.query("top_customers", #{country: "DE", limit: 50})`) into a tab instead of calling
  invisibly: visible, editable, in the history. Last values per query in `localStorage`. Completion of `g.query("` lists names grouped by folder.
  Later: options from the data (`options: "g.v().has_label('country').values('code').dedup()"`)
  for a select.
- **MCP** (DECIDED): generic tools `list_saved_queries` {folder?, search?} (name, folder,
  description, parameters) and `run_saved_query` {name, params?}, a resource
  `graphersal://saved-queries`, and a hint in the server instructions and the `query` tool to look
  for a saved query before writing a new one. A fixed tool set keeps the tool list small and stable
  however many queries exist. Optional `--mcp-query-tools` also exposes each query as its own tool
  (`inputSchema` from the parameter schemas; our schema nodes are a JSON Schema subset) for small
  curated catalogs. All of it works in `--mcp-read-only` (saved queries are read-only).
- **CLI / Python**: the same DSL calls; `graphersal store` may list the catalog.

## 9. Decisions on the former open questions

1. **Position**: v1 supports the start position only (`g.query(..)`); mid-traversal calls
   (`.query(..)` on incoming traversers, body returning an anonymous traversal spliced in) are a
   follow-up.
2. **Arguments**: named only (`#{..}` map, or no argument when every parameter has a default).
3. **Dependencies**: not tracked. Dropping or changing a query others call is allowed; a missing
   callee fails at call time with a located error. Recursion is bounded at run time (call depth
   16, `QueryDepthExceeded`), not analysed at definition.
4. **Case**: names and folders are case-sensitive, like labels and property keys.
5. **Terminals** are allowed in every query (a value-returning query needs them); read-only
   is enforced by the authorizer, not by forbidding terminals. The return shape is not declared:
   a traversal result can be continued, a value is returned as is.
6. **Limits**: name 1-128 bytes (identifier characters `[A-Za-z_][A-Za-z0-9_]*`, since it is also
   the Rhai function name and an MCP tool name), folder at most 255 bytes and 8 segments,
   description at most 4 KiB, source at most 64 KiB, at most 10 000 definitions, call depth 16.

## 10. Related notes

- **Schema display gap**: a vertex label's own `description` (in its root schema node) is stored
  and returned by `get_schema()`, but the table view shows no row for it (edges show theirs in
  Notes, because an edge without properties gets a label row).
- **Parked, for the dev server only**: read-only store views beside the live graph. In 0.1.0 a

  "View" of a past point replaces the graph the session serves for every client (playground tabs,
  MCP) until "Back to the live graph", although the store itself keeps accepting writes. A
  per-request database target (`db` in `/api/execute` and the MCP `query` tool) with views as
  named read-only databases would keep the live graph writable. Not for the CLI.
