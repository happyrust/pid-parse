# Verification: Phase 32 Full PID Analysis And File Export

> **[DRAFT — awaiting Plannotator gate]**

## Planning Verification

| Check | Command / Method | Pass condition |
|---|---|---|
| Goal package exists | inspect files | five docs + `progress.jsonl` exist |
| Main plan exists | inspect `docs/plans/2026-06-16-phase32-full-pid-analysis-and-file-export-plan-cn.md` | covers analysis + export + writer + publish + gates |
| Planning files updated | inspect `task_plan.md`, `findings.md`, `progress.md` | Phase 32 entry present |
| Markdown sanity | `git diff --check` | no whitespace errors |
| Plannotator gate | `plannotator annotate <doc> --gate` | approved, or failure recorded |

## Future Implementation Commands

| Command | Purpose | Expected pass |
|---|---|---|
| `cargo test --locked --lib` | fast library regression | all pass |
| `cargo test --locked --test parse_real_files -- --nocapture` | real fixture soft-skip / ratchet | all available fixtures pass |
| `cargo test --locked --test parser_panic_safety -- --nocapture` | adversarial parser safety | no panic |
| `cargo run --bin pid_inspect -- test-file/D06.pid --export-bundle target/tmp/d06.bundle` | bundle smoke | manifest + decoded/audit files exist |
| `cargo run --bin pid_inspect -- test-file/D06.pid --round-trip target/tmp/d06.copy.pid --verify` | writer boundary smoke | zero unintended diffs |
| `cargo build --locked --workspace --all-targets` | gate 1/5 | exit 0 |
| `cargo test --locked --workspace --all-targets` | gate 2/5 | all pass |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | gate 3/5 | exit 0 |
| `cargo fmt --all -- --check` | gate 4/5 | exit 0 |
| `bash .github/scripts/check-missing-docs.sh` | gate 5/5 | current <= baseline |

## Bundle Shape Checks

| Check | Pass condition |
|---|---|
| Manifest source identity | `manifest.json` contains source path, source hash, generation command, parser version |
| Confidence separation | decoded / audit / probe outputs are not merged into one ambiguous JSON |
| Raw opt-in | raw stream `.bin` files are absent unless explicitly requested |
| Path escaping | stream paths with `/`, control chars, and non-ASCII are reversible |
| Byte provenance | decoded/audit/probe entities carry stream path + byte range where available |
| Publish separation | `publish/` appears only when MDF/SQLite publish input is provided |
| Writer boundary | `writer/round_trip_plan.json` marks editable vs read-only surfaces |

## Evidence Rules

Every implementation slice must append a JSONL entry to `progress.jsonl`:

```json
{"type":"slice_complete","timestamp":"...","slice":"A","command":"...","summary":"...","next":"..."}
```

Promotion decisions must append:

```json
{"type":"promotion_decision","timestamp":"...","topic":"...","from":"Probe","to":"Decoded","evidence":["byte_range","fixture_ratchet","IDA_or_controlled_fixture"],"approved":true}
```

If evidence is insufficient:

```json
{"type":"negative_closeout","timestamp":"...","topic":"PSMspacemap raw page","reason":"no direct reader/writer path","parser_change":false}
```

## Completion Signatures

Planning-only:

```json
{"type":"planning_complete","timestamp":"...","phase":"32","goal_package":"goals/phase32-full-pid-analysis-and-file-export","implementation_started":false}
```

Implementation complete:

```json
{"type":"goal_complete","timestamp":"...","phase":"32","export_bundle_cli":true,"format_atlas":true,"bundle_contract":true,"gates":"5/5 green","parser_promotions_without_evidence":0}
```
