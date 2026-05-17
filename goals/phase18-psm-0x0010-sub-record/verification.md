# Verification: Phase 18 PSM 0x0010 sub-record family audit-only decoder

## Commands

| Command | Purpose | Expected pass condition | Evidence location |
|---|---|---|---|
| `cargo run --release --example probe_psm_0x0010_shape` | Re-confirm cross-fixture probe baseline (non-advancing scan, may double-count overlapping records) | DWG-0201=183, DWG-0202=133, 工艺管道-1=315, A01=7, total=638 | `progress.jsonl` |
| `cargo test --locked --lib parsers::sheet_records` | New parser unit tests (canonical, each rejection, panic safety) all green | exit 0 | `progress.jsonl` |
| `cargo test --locked --lib schema` | schema ratchet covers new DTO + field needles | exit 0 | `progress.jsonl` |
| `cargo test --locked --lib model` | model `From` impl + `JsonSchema` derive | exit 0 | `progress.jsonl` |
| `cargo test --locked -j 1 --test parser_panic_safety` | adversarial matrix covers new entries | exit 0 | `progress.jsonl` |
| `cargo test --test parse_real_files sub_records_0x0010_decoder_emits_audit_records_with_provenance -- --nocapture` | new cross-fixture ratchet test (advancing scan, Phase 15 GraphicGroup template) | total = 582 (DWG-0201=161, DWG-0202=104, 工艺管道-1=306, A01=11); per-record provenance non-empty | `progress.jsonl` |
| `cargo test --test parse_real_files normalized_geometry_probe_baseline_on_real_fixture -- --nocapture` | normalized geometry contract unchanged | total entity count identical to pre-Phase 18 baseline | `progress.jsonl` |
| `cargo test --test parse_real_files graphic_group_decoder_ratchets_fixture_counts_and_header_fields -- --nocapture` | Phase 15 audit-only baseline preserved | 352 cross-fixture preserved | `progress.jsonl` |
| `cargo test --test parse_real_files jstyle_override_decoder_emits_audit_records_with_provenance -- --nocapture` | Phase 16 authoritative baseline preserved | 98 cross-fixture preserved | `progress.jsonl` |
| `cargo test --test parse_real_files primitive_line_decoder_emits_decoded_lines_with_provenance -- --nocapture` | Phase 14 GLine2d not regressed | 3 cross-fixture | `progress.jsonl` |
| `cargo test --test parse_real_files iglines_decoder_emits_decoded_iglines_with_provenance -- --nocapture` | Phase 14 igLine2d not regressed | 284 cross-fixture | `progress.jsonl` |
| `cargo test --test parse_real_files iglinestrings_decoder_emits_decoded_polylines_with_provenance -- --nocapture` | Phase 14 igLineString2d not regressed | 119 cross-fixture | `progress.jsonl` |
| `cargo test --test parse_real_files igpoints_decoder_emits_decoded_points_with_provenance -- --nocapture` | Phase 14 igPoint2d not regressed | 146 cross-fixture | `progress.jsonl` |
| `cargo test --test parse_real_files igtextboxes_decoder_emits_decoded_texts_with_provenance -- --nocapture` | Phase 14 igTextBox not regressed | 142 cross-fixture | `progress.jsonl` |
| `cargo test --test parse_real_files igsymbols_decoder_emits_decoded_symbols_with_provenance -- --nocapture` | Phase 14 igSymbol2d not regressed | 27 cross-fixture | `progress.jsonl` |
| `cargo build --locked --workspace --all-targets` | full workspace compile | exit 0 | `progress.jsonl` |
| `cargo test --locked --workspace --all-targets` | full workspace tests | exit 0 | `progress.jsonl` |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | lint gate | exit 0 | `progress.jsonl` |
| `cargo fmt --all -- --check` | formatting gate | exit 0 | `progress.jsonl` |
| `bash .github/scripts/check-missing-docs.sh` (Windows: `cargo rustdoc --lib --locked -- -W missing-docs` + 与 baseline 对账) | missing_docs ratchet | current = baseline (= 0) | `progress.jsonl` |

## Manual Checks

| Check | How | Pass condition |
|---|---|---|
| Audit-only invariant | grep `decode_sub_records_0x0010` 周边代码 / `SheetSubRecord0x0010Decoded` 字段 | 无 `coord_*` / `referenced_*` / `sub_kind` / `tag_*` 字段名 |
| Provenance completeness | 看 cross-fixture ratchet 测试输出 | 每条带 sheet path / byte range / oid / non-empty raw_payload |
| Normalized geometry unchanged | 看 `normalized_geometry_probe_baseline_on_real_fixture` 输出 | entity 总数与 Phase 17 末态完全一致 |
| CHANGELOG quality | 看 Phase 18 entry diff | 含 638 baseline, audit-only 设计选择, no PidGraphicKind 提升说明 |
| panic-safety completeness | 看 `tests/parser_panic_safety.rs` matrix | 含 `decode_sub_records_0x0010` + `decode_sub_record_0x0010_at` |

## Evidence Rules

- 每完成一个 AC，在 `progress.jsonl` append 一条 JSON line。例：

```json
{"type":"slice_b_complete","timestamp":"...","ac":["AC2","AC7"],"command":"cargo test --locked --lib parsers::sheet_records sub_records_0x0010","summary":"parser DTO + decoder + decoder_at + 8 unit tests all green; panic-safety on empty/truncated/全0/全0xFF inputs verified."}
```

- 跨 fixture ratchet test 输出要 capture（`--nocapture`）后摘要：

```json
{"type":"ratchet","timestamp":"...","ac":["AC1","AC6"],"command":"cargo test --test parse_real_files sub_records_0x0010_decoder_emits_audit_records_with_provenance -- --nocapture","fixture_counts":{"DWG-0201GP06-01.pid":161,"DWG-0202GP06-01.pid":104,"工艺管道及仪表流程-1.pid":306,"export-test/publish-data/A01/A01.pid":11,"total":582}}
```

- 5 道 gate 写为单条 entry：

```json
{"type":"gates","timestamp":"...","ac":["AC10"],"commands":["build","test","clippy","fmt","missing-docs"],"results":{"build":"ok","test":"ok ... 0 failed","clippy":"ok","fmt":"ok","missing_docs":"current=0 baseline=0 ratchet pass"},"summary":"5/5 pre-commit gates green."}
```

## 收口检查

merge 或声明完成前按顺序跑：

```powershell
cargo build --locked --workspace --all-targets
cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo rustdoc --lib --locked -- -W missing-docs
```

任一 gate 失败：停止，记录 blocker，不继续扩大 scope。

## 完成签名

最后 append：

```json
{"type":"goal_complete","timestamp":"...","phase":"18","decoded_type":"PSM 0x0010","class_name":"audit-only (not yet IDA-confirmed)","sub_record_count":582,"per_fixture":{"DWG-0201GP06-01.pid":161,"DWG-0202GP06-01.pid":104,"工艺管道及仪表流程-1.pid":306,"export-test/publish-data/A01/A01.pid":11},"phase14_baselines_preserved":true,"phase15_audit_preserved":true,"phase16_jstyle_preserved":true,"phase17_primitive_arc_removed":true,"normalized_geometry_unchanged":true,"gates":"5/5 green","pidgraphickind_new_variant":false}
```

然后暂停等用户签收，不主动扩到 sub-kind 字段反向 / reference resolver。
