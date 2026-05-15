# Controlled Diff Evidence Report Plan

> Date: 2026-05-09
> Scope: Phase 14 follow-up, evidence intake only.

## Objective

Move controlled `.pid` before/after diff evidence from `pid_inspect` private CLI
structures into a reusable investigation layer.

This phase must produce a `ControlledDiffEvidenceReport` that can be tested,
serialized, and reused by future decoder spikes without promoting geometry.

## Non-goals

- Do not emit new `NormalizedPidGeometry` entities.
- Do not mark any Sheet primitive as decoded.
- Do not stabilize a top-level public geometry API for controlled diffs yet.
- Do not require proprietary fixtures in CI.

## Recommended Module Boundary

Add `src/inspect/controlled_diff.rs`.

The module should own:

- `ControlledDiffMetadata`
- `ControlledDiffEvidenceReport`
- `ControlledDiffCaseReport`
- `ControlledDiffStreamReport`
- a pure report builder that accepts before/after `PidPackage` values and
  validated metadata

`pid_inspect --controlled-diff-dir` should keep filesystem scanning and output
rendering, but delegate evidence construction to the inspect module.

## First Red Test

Use synthetic in-memory or temp-file `PidPackage` inputs, not private `.pid`
fixtures.

The first test should prove:

- `promoted_geometry == false`
- `case` and `operation` come from metadata
- `expected` metadata payload is preserved
- stream diff count is deterministic
- modified Sheet stream count is deterministic
- first modified stream reports `/Sheet6` and first mismatch context

## Promotion Gate

The evidence report may feed a typed decoder spike only after controlled samples
prove all of:

- bounded Sheet byte range;
- operation metadata explains that range;
- shape repeats across at least two independent cases;
- parsed coordinate/style fields can round-trip back to the expected payload.

Until then, results remain investigation evidence.

## Verification

Run focused checks first:

```powershell
cargo test --locked -j 1 --test inspect_cli controlled_diff_dir -- --nocapture
cargo test --locked -j 1 --test parse_real_files controlled_pid_diff_pairs_report_stream_level_evidence_when_available -- --nocapture
cargo clippy --locked -j 1 --bin pid_inspect --test inspect_cli --test parse_real_files -- -D warnings
cargo fmt --all -- --check
```

Use `CARGO_INCREMENTAL=0` and `CARGO_BUILD_JOBS=1` on Windows if linker/pagefile
pressure appears.
