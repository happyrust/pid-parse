# Phase 32-C Bundle Commit Readiness Review

Date: 2026-06-18

## Scope Reviewed

Phase 32-C implements `.pid.bundle/` export from planning through CLI publish
opt-in:

- `src/export_bundle.rs`
  - `ExportBundlePlan` / `ExportBundlePublishPlan`
  - bundle manifest / raw stream index / decoded split views / geometry split
    files / writer guidance
  - manifest `inputs.pid` and `inputs.publish_mdf` SHA-256 identities
  - deferred `publish/status.json`
  - explicit `export_bundle_publish_xml(...)`
  - optional `publish/publish_diff.json`
- `src/bin/pid_inspect.rs`
  - `--export-bundle`
  - `--export-bundle-raw-streams`
  - `--export-bundle-publish`
  - `--publish-drawing`
  - `--publish-plant`
  - `--publish-style`
  - `--publish-diff-against`
- `src/import_view.rs`
  - serde-backed `PidImportView` export surface
- `src/lib.rs`
  - public export-bundle module and re-exports
- `tests/inspect_cli.rs`
  - CLI bundle smoke coverage
- `Cargo.toml` / `Cargo.lock`
  - direct `sha2` dependency for input identity hashes

## Review Result

No blocking correctness issue found in the Phase 32-C implementation path.

The implementation keeps the important boundaries intact:

- default bundle export does not dump raw stream bytes;
- publish XML is only generated with explicit publish input and drawing UID;
- publish XML remains MDF/legacy-SQLite backed, not inferred from `.pid` raw
  bytes;
- no parser confidence or byte-layout promotion is introduced;
- `publish_diff.json` is a bundle-local summary artifact and does not mutate
  the existing `publish::diff` public DTOs.

## Residual Risks

- The working tree contains unrelated or earlier-scope files alongside
  Phase 32-C work: Phase30/31 analysis docs and `debug.log`. Do not include
  `debug.log` in a Phase 32-C commit unless it is intentionally reviewed and
  documented.
- `git diff --check` passes, but Git reports existing Windows line-ending
  warnings for `Cargo.toml`, `Cargo.lock`, `src/bin/pid_inspect.rs`,
  `src/import_view.rs`, and `tests/inspect_cli.rs`.
- `publish/publish_diff.json` currently records summary counts only. Row-level
  diff details and CI-gating behavior are explicitly deferred.
- `export_bundle_publish_xml(...)` writes `data.xml` / `meta.xml` before
  reference diff comparison. If reference reading/comparison fails, partial XML
  files can remain in the output directory. This is acceptable for the current
  CLI artifact mode but should be revisited before making diff comparison a
  hard CI gate.

## Verification Reviewed

Latest focused gate:

```powershell
cargo fmt --all -- --check
cargo test --locked --lib export_bundle -- --nocapture
cargo test --locked --test inspect_cli export_bundle -- --nocapture
cargo rustdoc --lib --locked -- -W missing-docs
cargo clippy --locked --all-targets -- -D warnings
git diff --check
```

Results:

- `export_bundle` focused tests: 15 passed
- `inspect_cli export_bundle` focused tests: 4 passed
- rustdoc missing-docs: passed
- all-targets clippy: passed
- diff check: passed, with only existing CRLF warnings
- scoped IDE lints: no errors

## Commit Boundary Recommendation

Recommended Phase 32-C commit set:

- `Cargo.toml`
- `Cargo.lock`
- `src/export_bundle.rs`
- `src/bin/pid_inspect.rs`
- `src/import_view.rs`
- `src/lib.rs`
- `tests/inspect_cli.rs`
- `docs/analysis/2026-06-16-pid-format-atlas-cn.md`
- `docs/pid-export-bundle-contract.md`
- `docs/plans/2026-06-16-phase32-full-pid-analysis-and-file-export-plan-cn.md`
- `goals/phase32-full-pid-analysis-and-file-export/`
- `task_plan.md`
- `findings.md`
- `progress.md`

Keep separate unless intentionally part of another commit:

- `docs/analysis/2026-06-12-phase30-sppid-backend-idb-sweep.md`
- `docs/analysis/2026-06-12-phase31-commit-readiness-review.md`
- `docs/analysis/2026-06-13-phase31-olecrt-storage-entrypoints.md`
- `docs/plans/2026-06-12-phase31-post-ida-development-plan-cn.md`
- `docs/analysis/2026-06-12-phase30-ida-gated-next-actions.md`
- `debug.log`

## Next Recommendation

Stop implementation here and prepare a Phase 32-C review/commit. Further work
such as row-level publish diff output or CI-gated publish parity should be a
new follow-up slice.
