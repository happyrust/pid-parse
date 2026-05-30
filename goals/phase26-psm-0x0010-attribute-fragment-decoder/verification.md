# Verification: Phase 26 PSM 0x0010 Attribute Fragment Decoder

## Commands

| Command | Purpose | Expected pass | Evidence |
|---|---|---|---|
| `cargo run --release --example probe_attribute_fragment` | Slice A: Sheet-stream probe | exit 0; per-fixture string count + samples | `progress.jsonl` |
| `cargo test --locked -j 4 --lib parsers::sheet_records::tests::attribute_fragment -- --nocapture` | Slice C: DTO + API unit tests | 8–12 tests, 0 failed | `progress.jsonl` |
| `cargo test --locked -j 4 --lib schema -- --nocapture` | Slice D: schema ratchet | all pass | `progress.jsonl` |
| `cargo test --locked -j 4 --test parse_real_files attribute_fragment_cross_fixture -- --nocapture` | Slice E: cross-fixture ratchet | per-fixture string count >= N | `progress.jsonl` |
| `cargo test --locked -j 4 --test parse_real_files -- --nocapture` (0x0010 raw) | Slice H: 582 baseline intact | `decoded_sub_records_0x0010 == 582` | `progress.jsonl` |
| `cargo test --locked -j 4 --test parser_panic_safety -- --nocapture` | Slice F: panic safety | all pass | `progress.jsonl` |
| `cargo build --locked --workspace --all-targets` | gate 1/5 | exit 0 | `progress.jsonl` |
| `cargo test --locked --workspace --all-targets` | gate 2/5 | all pass | `progress.jsonl` |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | gate 3/5 | exit 0 | `progress.jsonl` |
| `cargo fmt --all -- --check` | gate 4/5 | exit 0 | `progress.jsonl` |
| `bash .github/scripts/check-missing-docs.sh` | gate 5/5 | current <= baseline | `progress.jsonl` |

## Checks

| Check | How | Pass condition |
|---|---|---|
| Typed decoder correctness | unit test on `ODOIL020150 MM` synthetic record | strings == ["ODOIL020150 MM"], marker==0x00010002 |
| CJK decode | unit test with `设计温度` UTF-16LE | text round-trips exactly |
| Illegal UTF-16 rejection | unit test with unpaired surrogate | record not in typed set (falls back to raw) |
| Truncated length prefix | unit test `len` exceeds payload | record skipped, no panic |
| Sheet-stream extraction | Slice E integration | strings extracted from Sheet streams, not whole-file scan |
| **582 raw baseline intact** | Slice H | `decoded_sub_records_0x0010 == 582` unchanged |
| Cross-fixture string ratchet | Slice E | per-fixture string count >= N (N from Slice A) |
| Panic safety | adversarial matrix | 0 panics |
| Phase 14–25 baselines | full workspace test | 0 regressions |
| missing-docs | check-missing-docs.sh | current <= baseline (0) |

## Evidence Rules

每 Slice 完成 append:

```json
{"type":"slice_complete","timestamp":"...","slice":"A","ac":["AC1"],"command":"...","summary":"...","next":"..."}
```

关键决策 append `decision`;5 道 gate 单条 `gates` entry。

## Partial AC Closeout (conservative path)

若 Slice B 无 IDA 确认多 string tail:

| Check | Status | Evidence |
|---|---|---|
| AC1 typed decoder | pass (single-string subset) | DTO + API decode 1st string only |
| AC2 Sheet-stream extraction | pass | from cluster pipeline |
| AC3 UTF-16LE strings | pass (single) | clean decode + reject illegal |
| AC4 cross-fixture ratchet | pass (conservative N) | single-string count locked |
| AC5 panic safety | pass | matrix green |
| AC6 analysis doc | pass | records "multi-string tail pending IDA" |
| AC7 baselines | pass | 582 + all unchanged |
| AC8 gates | pass | 5/5 green |
| AC9 progress | pass | `partial_complete` entry |

Partial entry:

```json
{"type":"partial_complete","timestamp":"...","phase":"26","work_type":"attribute_fragment_decoder_conservative","scope":"single-string records only","deferred":["multi-string tail layout (needs dlls/radsrvitem.dll.i64 analyze_function)","aux 8-byte semantics","promotion to PidGraphicKind"],"raw_baseline_intact":{"decoded_sub_records_0x0010":582},"src_code_changes":true,"test_changes":true}
```

## 收口检查

```powershell
cargo build --locked --workspace --all-targets
cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
bash .github/scripts/check-missing-docs.sh
```

## 完成签名 (full path)

```json
{"type":"goal_complete","timestamp":"...","phase":"26","work_type":"attribute_fragment_decoder","new_typed_collection":"decoded_attribute_fragments","per_fixture_string_count":{"DWG-0201":"<n>","DWG-0202":"<n>","工艺管道-1":"<n>","A01":"<n>","D06":"<n>"},"raw_baseline_intact":{"decoded_sub_records_0x0010":582},"pidgraphickind_emitted":false,"phase14_through_25_baselines_preserved":true,"gates":"5/5 green","src_code_changes":true,"test_changes":true}
```

然后暂停等用户签收。Promotion(emit PidGraphicKind)需单独后续 phase。
