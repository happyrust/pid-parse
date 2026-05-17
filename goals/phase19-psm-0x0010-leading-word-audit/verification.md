# Verification: Phase 19 PSM 0x0010 leading-word audit

## Commands

| Command | Purpose | Expected pass condition | Evidence location |
|---|---|---|---|
| `cargo run --release --example probe_psm_0x0010_sub_kind` | Re-confirm Phase 19 probe baseline (advancing scan, `payload[0..2]` u16 LE) | total=578, top words 0x0002=164 (28%), 0x0003=21 (3.6%), 0x0001=18 (3.1%) | `progress.jsonl` |
| `cargo test --locked --lib parsers::sheet_records::tests::sub_record_0x0010` | Phase 18 12 + Phase 19 ≥ 2 new unit tests, all green | exit 0 | `progress.jsonl` |
| `cargo test --locked --lib schema` | schema ratchet covers new `leading_word` needle | exit 0 | `progress.jsonl` |
| `cargo test --locked --lib model` | model `From` impl + `JsonSchema` derive | exit 0 | `progress.jsonl` |
| `cargo test --locked -j 1 --test parser_panic_safety` | adversarial matrix unchanged but still green | exit 0 | `progress.jsonl` |
| `cargo test --test parse_real_files sub_records_0x0010_leading_word_distribution_matches_phase19_probe -- --nocapture` | new Phase 19 cross-fixture ratchet (advancing scan, decoder-side) | 0x0002=164, 0x0003=21, 0x0001=18, None=0 (±4 reconciliation note allowed if decoder differs from probe due to overlap-scan diff) | `progress.jsonl` |
| `cargo test --test parse_real_files sub_records_0x0010_decoder_emits_audit_records_with_provenance -- --nocapture` | Phase 18 ratchet still green (additive, not regressive) | total = 582 (DWG-0201=161, DWG-0202=104, 工艺管道-1=306, A01=11) | `progress.jsonl` |
| `cargo test --test parse_real_files normalized_geometry_probe_baseline_on_real_fixture -- --nocapture` | normalized geometry contract unchanged | total entity count identical to Phase 18 baseline | `progress.jsonl` |
| `cargo test --test parse_real_files graphic_group_decoder_ratchets_fixture_counts_and_header_fields -- --nocapture` | Phase 15 audit baseline preserved | 352 cross-fixture | `progress.jsonl` |
| `cargo test --test parse_real_files jstyle_override_decoder_emits_audit_records_with_provenance -- --nocapture` | Phase 16 authoritative baseline preserved | 98 cross-fixture | `progress.jsonl` |
| `cargo test --test parse_real_files primitive_line_decoder_emits_decoded_lines_with_provenance -- --nocapture` | Phase 14 GLine2d preserved | 3 cross-fixture | `progress.jsonl` |
| `cargo test --test parse_real_files iglines_decoder_emits_decoded_iglines_with_provenance -- --nocapture` | Phase 14 igLine2d preserved | 284 cross-fixture | `progress.jsonl` |
| `cargo test --test parse_real_files iglinestrings_decoder_emits_decoded_polylines_with_provenance -- --nocapture` | Phase 14 igLineString2d preserved | 119 cross-fixture | `progress.jsonl` |
| `cargo test --test parse_real_files igpoints_decoder_emits_decoded_points_with_provenance -- --nocapture` | Phase 14 igPoint2d preserved | 146 cross-fixture | `progress.jsonl` |
| `cargo test --test parse_real_files igtextboxes_decoder_emits_decoded_texts_with_provenance -- --nocapture` | Phase 14 igTextBox preserved | 142 cross-fixture | `progress.jsonl` |
| `cargo test --test parse_real_files igsymbols_decoder_emits_decoded_symbols_with_provenance` | Phase 14 igSymbol2d preserved | 27 cross-fixture | `progress.jsonl` |
| `cargo build --locked --workspace --all-targets` | full workspace compile | exit 0 | `progress.jsonl` |
| `cargo test --locked --workspace --all-targets` | full workspace tests | exit 0 | `progress.jsonl` |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | lint gate | exit 0 | `progress.jsonl` |
| `cargo fmt --all -- --check` | formatting gate | exit 0 | `progress.jsonl` |
| `bash .github/scripts/check-missing-docs.sh` (Windows: `cargo rustdoc --lib --locked -- -W missing-docs` + 与 baseline 对账) | missing_docs ratchet | current = baseline (= 0) | `progress.jsonl` |

## Manual Checks

| Check | How | Pass condition |
|---|---|---|
| Audit-only invariant | grep `SheetSubRecord0x0010Decoded` / `DecodedSubRecord0x0010Record` 字段 | 字段名只含 `leading_word`，**无** `sub_kind` / `record_kind` / `family_tag` / `payload_kind` |
| 字段位置语义透明 | 看新字段 doc comment | 必须说明 "= `payload[0..2]` as little-endian u16; `None` if `payload.len() < 2`"；**禁止**说 "sub-kind discriminator"（语义命名） |
| Phase 18 ratchet 数字 | 看 Phase 18 ratchet test 输出 | 仍是 582 (DWG-0201=161, DWG-0202=104, 工艺管道-1=306, A01=11) |
| Normalized geometry unchanged | 看 `normalized_geometry_probe_baseline_on_real_fixture` 输出 | entity 总数与 Phase 18 末态完全一致 |
| CHANGELOG quality | 看 Phase 19 entry diff | 含 leading_word=0x0002 count 164 + audit-only 设计选择 + 与 Phase 18 additive 关系 + 不命名 sub-kind 的解释 |
| Probe vs decoder reconciliation | 对比 probe 数字 (578 records / 0x0002=164) 与 decoder ratchet 数字 | decoder 数字以 ground truth 为准；若与 probe 差 ±4 records 写入 progress.jsonl 解释（advancing-scan vs probe overlap-scan 差异） |

## Evidence Rules

- 每完成一个 AC，在 `progress.jsonl` append 一条 JSON line：

```json
{"type":"slice_b_complete","timestamp":"...","ac":["AC1","AC2"],"command":"cargo test --locked --lib parsers::sheet_records::tests::sub_record_0x0010","summary":"leading_word field added to parser DTO; 2 new unit tests + 12 Phase 18 unit tests all green; canonical decode fills leading_word with Some(0x0002); short payload returns None."}
```

- 跨 fixture ratchet test 输出要 capture（`--nocapture`）后摘要：

```json
{"type":"ratchet","timestamp":"...","ac":["AC5"],"command":"cargo test --test parse_real_files sub_records_0x0010_leading_word_distribution_matches_phase19_probe -- --nocapture","leading_word_counts":{"0x0002":164,"0x0003":21,"0x0001":18,"None":0},"reconciliation":"decoder count == probe count for top 3 words; no overlap-scan discrepancy"}
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
{"type":"goal_complete","timestamp":"...","phase":"19","decoded_type":"PSM 0x0010 leading_word","class_name":"audit-only (still not IDA-confirmed)","sub_record_count":582,"new_typed_field":"leading_word: Option<u16>","leading_word_distribution":{"0x0002":164,"0x0003":21,"0x0001":18,"None":0},"phase14_baselines_preserved":true,"phase15_audit_preserved":true,"phase16_jstyle_preserved":true,"phase17_primitive_arc_removed":true,"phase18_audit_preserved":true,"normalized_geometry_unchanged":true,"gates":"5/5 green","pidgraphickind_new_variant":false,"sub_kind_field_naming":false}
```

然后暂停等用户签收，不主动扩到 sub-kind 字段反向 / reference resolver / Phase 20。
