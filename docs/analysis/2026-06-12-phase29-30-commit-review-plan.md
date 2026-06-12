# Phase 29/30 Commit / Review Plan

> Prepared after final local gates on 2026-06-12. No commit was created.

## Gate Status

Passed:

- `cargo fmt --all -- --check`
- `cargo build --locked --workspace --all-targets`
- `cargo test --locked --workspace --all-targets`
- `cargo clippy --locked --workspace --all-targets -- -D warnings`
- fallback `cargo rustdoc --lib --locked -- -W missing-docs`

Known local environment issue:

- `bash .github/scripts/check-missing-docs.sh` fails on this Windows shell
  with mojibake / `REGDB_E_CLASSNOTREG`. The rustdoc fallback passed and
  is the useful local code signal.

## Recommended Review Shape

The current worktree can be reviewed as one milestone if the reviewer is
comfortable with a large reverse-engineering batch. If smaller review
units are preferred, split along these boundaries.

### Option A: Single Milestone Commit

Use when the goal is to preserve the exact Phase 29/30 narrative and avoid
staging mistakes across tightly related docs/tests/snapshots.

Suggested commit message:

```text
advance Phase 29 byte-audit closeout and Phase 30 IDA evidence

Completes the remaining parser, byte-audit, and documentation work for
Phase 29, then records the reachable radsrvitem.dll IDA evidence and
gated next actions for Phase 30.
```

Review focus:

- `src/byte_audit/aggregate.rs`
- `src/parsers/cluster_header.rs`
- `src/parsers/dynamic_attr_records.rs`
- `src/parsers/jsites_list.rs`
- `tests/parse_real_files.rs`
- `tests/parser_panic_safety.rs`
- Phase 29/30 analysis docs and spec-kit outputs

### Option B: Three Review Units

Use when reviewers want a smaller blast radius.

#### Unit 1: Phase 29 Parser / Byte-Audit Implementation

Contents:

- DA body walker and trace integration.
- Chain-scoped dynamic attribute extraction.
- Nested JSite cluster/registry dispatch.
- `JSitesList` parser and Revision registration.
- Parser panic-safety and real-fixture ratchets.

Key files:

- `src/byte_audit/aggregate.rs`
- `src/byte_audit/mod.rs`
- `src/parsers/cluster_header.rs`
- `src/parsers/dynamic_attr_records.rs`
- `src/parsers/jsites_list.rs`
- `src/parsers/mod.rs`
- `src/streams/dynamic_attrs.rs`
- `tests/parse_real_files.rs`
- `tests/parser_panic_safety.rs`

#### Unit 2: Phase 29 Probe / Spec / Analysis Artifacts

Contents:

- Probe examples used to justify parser decisions.
- Spec-kit snapshots and backlog updates.
- Phase 29 analysis writeups.
- README / format notes / agent guide updates.

Key paths:

- `examples/probe_phase29_*.rs`
- `examples/probe_psm_undecoded_shapes.rs`
- `docs/analysis/2026-06-08-phase29-*.md`
- `docs/specs/2026-06-08-pid-file-format-spec-kit/`
- `README.md`
- `docs/format-notes.md`
- `AGENTS.md`

#### Unit 3: Phase 30 IDA Evidence / Handoff Docs

Contents:

- `radsrvitem.dll` JSite evidence.
- `0x0089` export boundary.
- PSMspacemap handle model.
- Style/JStyle negative evidence.
- IDA-gated next-actions checklist.
- Worktree readiness and gate results.

Key paths:

- `docs/analysis/2026-06-12-phase30-*.md`
- `docs/analysis/2026-06-12-phase29-30-worktree-readiness.md`
- `docs/analysis/2026-06-12-phase29-30-commit-review-plan.md`
- `task_plan.md`
- `progress.md`
- `findings.md`
- `CHANGELOG.md`

## Suggested Next Command Sequence

Only after explicit commit authorization:

```bash
git status --short
git add <selected files>
git commit -m "<message>"
git status --short
```

Do not skip hooks unless explicitly requested.

## Residual Risks

- Large accumulated diff can be hard to review as one unit.
- Some files still trigger Git LF-to-CRLF warnings in status/diff output;
  this appears to be local line-ending policy noise, but it should be kept
  visible during review.
- Remaining semantic questions are IDA-gated, not test-gated:
  `/JSitesList` writer/stale-tail semantics, raw `/PSMspacemap` page
  layout, `0x0089` semantic family, StyleCluster prefix, and `0x0010`
  discriminator.
