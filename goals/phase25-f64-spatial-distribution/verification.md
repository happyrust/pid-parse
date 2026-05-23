# Verification: Phase 25-A normalized_f64_pair 空间分布挖掘

## Commands

| Command | Purpose | Expected pass condition | Evidence location |
|---|---|---|---|
| `cargo run --release --example probe_phase25_f64_spatial -- --json` | Slice A：跨 fixture probe | exit 0；每 sheet 的 cluster_count / bbox dump | `progress.jsonl` |
| `cargo test --locked -j 4 --lib parsers::sheet_records::tests::spatial_analysis -- --nocapture` | Slice B：DTO + API unit tests | 6-10 tests, 0 failed | `progress.jsonl` |
| `cargo test --locked -j 4 --lib schema -- --nocapture` | Slice C：schema ratchet | 8 tests pass | `progress.jsonl` |
| `cargo test --locked -j 4 --lib streams::cluster::tests -- --nocapture` | Slice D：pipeline 接入 | all pass | `progress.jsonl` |
| `cargo test --locked -j 4 --test parse_real_files spatial_analysis_cross_fixture -- --nocapture` | Slice E：cross-fixture ratchet | cluster_count >= N 通过 | `progress.jsonl` |
| `cargo test --locked -j 4 --test parse_real_files template_page_dimensions_do_not_make_page_transform_available -- --nocapture` | Phase 23 guardrail | pass | `progress.jsonl` |
| `cargo test --locked -j 4 --test parse_real_files probe_only_no_coordinate_page_metadata_promotion -- --nocapture` | Phase 24 guardrail | pass | `progress.jsonl` |
| `cargo build --locked --workspace --all-targets` | 5 道 gate 1/5 | exit 0 | `progress.jsonl` |
| `cargo test --locked --workspace --all-targets` | 5 道 gate 2/5 | all tests pass | `progress.jsonl` |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | 5 道 gate 3/5 | exit 0 | `progress.jsonl` |
| `cargo fmt --all -- --check` | 5 道 gate 4/5 | exit 0 | `progress.jsonl` |
| `cargo rustdoc --lib --locked -- -W missing-docs && bash .github/scripts/check-missing-docs.sh` | 5 道 gate 5/5 | current <= baseline | `progress.jsonl` |

## Checks

| Check | How | Pass condition |
|---|---|---|
| Spatial Analysis API 确定性 | 2 次跑 `coordinate_pair_spatial_analysis` 输入相同 pair set | 两次输出 `cluster_count` / `cluster_id` 完全一致 |
| Cross-fixture cluster baseline | Slice E ratchet test | 每 fixture 至少 N 个 cluster（N 由 Slice A 实际数据决定） |
| Provenance.note backward-compat | grep 现有消费 note 的代码 | append 而非 replace；分隔符 ` \| ` |
| Analysis doc 完整 | manual review | 每 fixture × 每 sheet 的 cluster 报告 + cross-fixture 对比 |
| Phase 23/24 guardrail | guard tests | 0 退化 |
| missing-docs baseline | `.github/scripts/check-missing-docs.sh` | current <= baseline (0) |

## Partial AC Closeout（negative 路径）

若 Slice A 发现所有 fixture 都是 uniform 分布，按 `blockers.md` Q5
接受 partial AC closeout：

| Check | Status | Evidence |
|---|---|---|
| AC1 spatial analysis API | pass | API + DTO 实现完整 |
| AC2 determinism | pass | unit test 通过 |
| AC3 cross-fixture aggregate | pass（uniform baseline） | ratchet 锁定 cluster_count <= 1 per fixture |
| AC4 provenance enhancement | skipped | uniform 情况下无意义 |
| AC5 analysis doc | pass（negative） | 明确记录 negative result + 推理链 |
| AC6 Phase 23/24 guardrails | pass | 全绿 |
| AC7 5 道 gates | pass | 全绿 |
| AC8 progress.jsonl | pass | 含 partial_complete entry |

Partial closeout entry format：

```json
{"type":"partial_complete","timestamp":"...","phase":"25-A","work_type":"spatial_analysis_negative","cross_fixture_finding":"uniform_distribution","cluster_count_per_fixture":{"DWG-0201":1,"DWG-0202":1,"工艺管道-1":1,"A01":1,"D06":1},"deferred":["attach_cluster_id_to_provenance"],"analysis_doc":"docs/analysis/2026-05-23-phase25-f64-spatial-distribution.md","src_code_changes":true,"test_changes":true}
```

## Evidence Rules

- 每个 Slice 完成时 append progress.jsonl entry：

```json
{"type":"slice_complete","timestamp":"...","slice":"A","ac":["AC1"],"command":"cargo run --release --example probe_phase25_f64_spatial","summary":"per-sheet cluster_count distribution: ...","next":"decide Slice B or negative"}
```

- 关键决策 append `decision` entry：

```json
{"type":"decision","timestamp":"...","at":"Slice A→B","choice":"continue/negative","reason":"...","evidence":"..."}
```

- 5 道 gate 单条 entry：

```json
{"type":"gates","timestamp":"...","ac":["AC7"],"commands":["build","test","clippy","fmt","missing-docs"],"results":{"build":"ok","test":"ok N lib + M integration, 0 failed","clippy":"ok","fmt":"ok","missing_docs":"current=0 baseline=0"},"summary":"5/5 pre-commit gates green."}
```

## 收口检查

merge / 完成前按顺序跑：

```powershell
cargo build --locked --workspace --all-targets
cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
bash .github/scripts/check-missing-docs.sh
```

任一 gate 失败：停止，记录 blocker。

## 完成签名

非 negative 路径完成时 append：

```json
{"type":"goal_complete","timestamp":"...","phase":"25-A","work_type":"spatial_analysis","cluster_baseline":{"DWG-0201":<n>,"DWG-0202":<n>,"工艺管道-1":<n>,"A01":<n>,"D06":<n>},"cluster_total":<sum>,"provenance_enhanced_entities":<n>,"analysis_doc":"docs/analysis/2026-05-23-phase25-f64-spatial-distribution.md","phase14_baselines_preserved":true,"phase15_audit_preserved":true,"phase16_jstyle_preserved":true,"phase17_primitive_arc_removed":true,"phase18_audit_preserved":true,"phase19_leading_word_preserved":true,"phase20_partial_preserved":true,"phase21_d06_preserved":true,"phase22_micro_preserved":true,"phase23_guardrails_green":true,"phase24_negative_preserved":true,"gates":"5/5 green","new_promotion_types":false,"src_code_changes":true,"test_changes":true}
```

然后暂停等用户签收。Phase 25-C 实施需要单独 /goal 启动。
