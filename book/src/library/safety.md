# Running Queries Safely

By default the library trusts its caller: the Rust API and the plain `script::eval*` helpers allow
everything and run without limits beyond the safe script defaults. A host that runs queries it did
not write (a web service, a plugin system, an AI agent) restricts them on three levels:

- **Permissions** decide *what* a query may do: read or write data, change the schema, read files,
  set administrative options. A ready-made `AccessPolicy` (for example `read_only()`) or your own
  `Authorizer` checks the plan once, before it runs.
- **Limits** decide *how much* it may use: script operations, traversers, materialized bytes,
  string sizes, nesting depth, a memory budget.
- **Time** bounds how long it may run: `evaluationTimeout`, loop limits, and a cancel token that
  stops a running query from another thread.

A host sets the defaults and can lock them, so a query cannot raise its own limits with
`g.with(...)`. Every refused or stopped query is rolled back.

| Page | What it covers |
|---|---|
| [Permissions](../users_guide/permissions.md) | requests, `AccessPolicy`, writing an authorizer, file roots |
| [Resource Limits](../users_guide/resource_limits.md) | script and traversal limits, the memory budget, the defaults of each front end |
| [Query Limits](../users_guide/query_limits.md) | timeouts and loop limits |
| [Execution Options Reference](../users_guide/execution_options_reference.md) | every `g.with()` key and how a host locks it |
