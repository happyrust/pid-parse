# Blockers: Phase 32 Full PID Analysis And File Export

> **[DRAFT — awaiting Plannotator gate]**

## Open Questions

### Q1 — Plannotator gate availability [OPEN]

`plannotator.exe` exists locally, but this goal package still needs an actual gate run. If the browser panel is unavailable or the command hangs, keep this phase as DRAFT and record `gate_deferred`.

### Q2 — Default raw stream export [OPEN]

Raw stream `.bin` output is useful for audit and downstream tooling, but `.pid` fixtures may contain proprietary data. Default recommendation: manifest + decoded/audit outputs only; raw stream bytes require explicit `--export-bundle-raw-streams`.

### Q3 — Bundle schema versioning [OPEN]

The bundle should carry a schema version independent of crate version. Default recommendation: `bundle_schema_version = 1`, with additive fields only unless a migration doc exists.

### Q4 — Publish XML subtree [OPEN]

Publish XML is MDF-backed, not derived solely from `.pid`. Default recommendation: `publish/` subtree is opt-in and must record its own input MDF hash.

### Q5 — Phase 32 scope boundary [PRESET]

This phase is allowed to create docs, contracts, and export implementation. It is not allowed to promote remaining unknown byte layouts unless Phase 32-D evidence triggers are met.

## Stop And Ask

Stop immediately if any of these occur:

1. User has not explicitly authorized commit / push.
2. A slice wants to mark `Probe` / `TypedAudit` as `Decoded` without byte-range + fixture + IDA/controlled fixture evidence.
3. A bundle implementation wants to default-write all raw stream bytes.
4. A writer implementation wants to update Sheet bytes from decoded geometry/probe JSON.
5. A publish implementation assumes MDF facts came from `.pid` raw streams.
6. A new IDA search is broad rather than tied to direct stream name / `IOContext::DoIO` / persist-manager clue.
7. `cargo fmt`, test, clippy, rustdoc, or missing-docs ratchet fails and the fix is not obvious.

## Dangerous Or High-Risk Actions

- Editing `vendor/oxidized-mdf/`.
- Changing existing `PidDocument` public fields without schema migration.
- Emitting new `PidGraphicKind` variants from audit/probe evidence.
- Changing `PidPageTransform::Unavailable` semantics.
- Making raw stream export default-on.
- Deleting or rewriting existing Phase 14–31 analysis history.

## Known Blockers

| ID | 类型 | 状态 | next action | owner |
|---|---|---|---|---|
| Q1 | gate | OPEN | run Plannotator gate or record deferred | user/agent |
| Q2 | privacy/size | OPEN | confirm raw export default-off | user |
| Q3 | schema | OPEN | approve `bundle_schema_version = 1` | user/agent |
| Q4 | publish | OPEN | keep MDF publish opt-in | user/agent |
| Q5 | scope | PRESET | no parser promotion without evidence | agent |

## Current Status

- Plan and goal package are drafted.
- Implementation should not start until Plannotator gate is approved or the user explicitly waives the gate.
- Current repo has pre-existing Phase 30/31 documentation changes; do not mix commits without user authorization.
