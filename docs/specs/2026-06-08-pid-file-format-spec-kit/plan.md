# Implementation Plan: PID File Format Spec Kit

## Technical Context

The repository already has a working parser, report CLI, byte-audit pipeline,
and IDA-driven reverse-engineering notes. This plan organizes those assets into
a Spec Kit style package and defines the next evidence-gathering steps.

Primary fact sources:

- `README.md`
- `AGENTS.md`
- `docs/prd-pid-parse-current-state.md`
- `docs/architecture-guide.md`
- `docs/analysis/2026-06-03-pid-file-format-analysis-cn.md`
- `docs/analysis/2026-06-03-phase27-pid-data-type-matrix-cn.md`
- `task_plan.md`
- `findings.md`
- `progress.md`

IDA evidence source:

- `radsrvitem.dll` through `ida-pro-mcp`, as captured by Phase 27.
- Latest progress indicates that `radsrvitem.dll` paths have largely
  converged for `0x0010`; deeper proof needs `style.dll`, `J2DSrv.dll`,
  `sppid.dll`, `XCeedRAD.dll`, or another JStyle/RAD host IDB.

## Spec Package Layout

| File | Purpose |
|---|---|
| `spec.md` | Requirements, scope, evidence levels, acceptance criteria. |
| `research.md` | Consolidated findings from parser docs and IDA evidence. |
| `data-model.md` | Evidence-graded inventory of known `.pid` format families. |
| `tasks.md` | Checkable backlog to complete the full format specification. |
| `quickstart.md` | Commands and IDA workflow for reproducing evidence. |
| `snapshot-priority-backlog.md` | Per-stream leftover / unregistered hotspot backlog generated from local fixture snapshots. |
| `phase29-candidate-slices.md` | Candidate implementation slices derived from the snapshot priority backlog. |

## Phase 28-A: Inventory Consolidation

Status: complete in this package.

Steps:

1. Reuse Phase 26 format map as the stream/storage baseline.
2. Reuse Phase 27 matrix as the Sheet/PSM type-code baseline.
3. Add latest saved progress about the `radsrvitem.dll` / JStyle blocker.
4. Express all entries using one evidence vocabulary.

## Phase 28-B: IDA Evidence Refresh

Status: pending.

Goal: refresh the matrix with live IDA evidence before any parser promotion.

Tasks:

1. Confirm currently reachable IDA instances.
2. If only `radsrvitem.dll` and unrelated `core.dll` are available, keep
   ordinary geometry reader status as negative / blocked.
3. Ask the user to open one or more of:
   - `style.dll`
   - `J2DSrv.dll`
   - `sppid.dll`
   - `XCeedRAD.dll`
   - `smartplantpid.exe`
4. For each new IDB, search for:
   - `1D1928C0-0000-0000-C000-000000000046`
   - `JStyleBase`
   - `IJPersist`
   - `DoIO`
   - `Read`
   - `igLine2d`, `igPoint2d`, `igLineString2d`, `igSymbol2d`
   - `GLine2d`
   - `GraphicGroup`

## Phase 28-C: Format Entry Completion

Status: pending.

For each format entry in `data-model.md`, classify it as:

- `Complete`: enough for current docs and tests.
- `NeedsFixture`: parser exists but lacks current real fixture snapshot.
- `NeedsIDA`: type identity or field semantics need IDA evidence.
- `NeedsParser`: IDA / fixture evidence exists but parser work has not been
  implemented.
- `Blocked`: required binary / fixture is not available.

## Phase 28-D: Promotion Backlog

Status: pending.

Produce a follow-up implementation backlog for Phase 29+:

1. P0: mismatch fixes for already decoded types.
2. P1: missing fields with IDA + fixture agreement.
3. P2: new high-value geometry decoders.
4. P3: audit-only families that now have confirmed semantic discriminators.
5. P4: documentation-only negative closeouts.

## Phase 28-E: Verification

Status: pending until parser or matrix changes are made after this package.

Minimum commands for doc-only updates:

```bash
cargo fmt --all -- --check
cargo test --test parse_real_files -- --nocapture
```

Full pre-commit gate when parser code changes:

```bash
cargo build --locked --workspace --all-targets
cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo rustdoc --lib --locked -- -W missing-docs
```

## Risks

| Risk | Impact | Mitigation |
|---|---|---|
| Missing private `.pid` fixtures | Cannot generate fresh coverage / byte-audit snapshots | Keep soft-skip boundaries explicit; require user-provided fixtures for snapshot work. |
| Missing IDA modules | Cannot prove `0x0010`, JStyle, or ordinary geometry readers | Record blocked status and exact module requests. |
| Over-promoting probe evidence | Downstream consumers may treat guesses as CAD truth | Enforce evidence levels and guardrails in `spec.md`. |
| Duplicate documentation drift | Phase 26/27 docs and this package may diverge | Keep this package as an index and decision surface; link detailed evidence back to Phase docs. |
