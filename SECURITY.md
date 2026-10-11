# Security policy

> Draft for review: the reporting channel is an open decision.

## Supported versions

| Version | Supported |
|---|---|
| 0.1.x | yes (the latest 0.1 release) |

Before 1.0 only the latest minor version receives fixes.

## Reporting a vulnerability

Please report vulnerabilities **privately** through GitHub's private vulnerability reporting: open the repository's
**Security** tab and choose **Report a vulnerability**. Do not open a public issue for a suspected vulnerability.

Include the affected version, a minimal script or program that reproduces the problem, and the impact you expect.
You can expect an acknowledgement and an assessment; fixes are released as patch versions.

## Scope notes

Scripts passed to the Rhai engine can be untrusted. The library bounds them with `ScriptLimits` and
`traversal.max_traversers` (see the book page *Resource Limits*); a way to exhaust CPU or memory despite the documented
limits, or a panic reachable from a script, is in scope. The command line tool `graphersal` is a trusted local tool and
runs without script limits unless asked.
