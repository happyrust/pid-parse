# Verification: Phase 17 `PrimitiveArc` 兼容路径迁移

## 验证命令矩阵

| 命令 | 目的 | 通过判据 | 证据存放点 |
|---|---|---|---|
| `rg "PrimitiveArc|primitive_arc|decode_primitive_arcs|SheetPrimitiveArcDecoded|DecodedPrimitiveArcRecord" src tests CHANGELOG.md docs goals` | 旧 surface inventory | 输出被分类为 public / internal / test / historical docs | `progress.jsonl` |
| `cargo test --locked --lib parsers::sheet_records` | parser 单测 | 与迁移策略一致：保留则 deprecated path 仍 panic-safe；删除则无 dangling test | `progress.jsonl` |
| `cargo test --locked --lib schema` | schema ratchet | schema exposed fields / mappings 与 Phase17 contract 一致 | `progress.jsonl` |
| `cargo test --locked -j 1 --test parser_panic_safety` | adversarial matrix | 所有保留 public parser entry 不 panic | `progress.jsonl` |
| `cargo test --test parse_real_files jstyle_override_decoder_emits_audit_records_with_provenance -- --nocapture` | Phase 16 authoritative baseline | 98 条 JStyleOverride records (20 + 30 + 47 + 1) 保持 | `progress.jsonl` |
| `cargo test --test parse_real_files normalized_geometry_probe_baseline_on_real_fixture -- --nocapture` | normalized geometry contract | `Annotation` bucket 保持；旧 `Arc` bucket 行为与迁移策略一致 | `progress.jsonl` |
| `cargo test --test parse_real_files primitive_arc_decoder_emits_decoded_arcs_with_provenance -- --nocapture` | legacy path guard（仅当保留旧 parser） | 若保留 compatibility path，测试名称 / assertion 明确 legacy；若删除，此测试不存在 | `progress.jsonl` |
| `cargo test --test parse_real_files primitive_line_decoder_emits_decoded_lines_with_provenance -- --nocapture` | Phase 14 GLine2d 不退化 | 3 跨 fixture 维持 | `progress.jsonl` |
| `cargo test --test parse_real_files iglines_decoder_emits_decoded_iglines_with_provenance -- --nocapture` | Phase 14 igLine 不退化 | 284 跨 fixture 维持 | `progress.jsonl` |
| `cargo test --test parse_real_files iglinestrings_decoder_emits_decoded_polylines_with_provenance -- --nocapture` | Phase 14 igLineString 不退化 | 119 跨 fixture 维持 | `progress.jsonl` |
| `cargo test --test parse_real_files igpoints_decoder_emits_decoded_points_with_provenance -- --nocapture` | Phase 14 igPoint 不退化 | 146 跨 fixture 维持 | `progress.jsonl` |
| `cargo test --test parse_real_files igtextboxes_decoder_emits_decoded_texts_with_provenance -- --nocapture` | Phase 14 igTextBox 不退化 | 142 跨 fixture 维持 | `progress.jsonl` |
| `cargo test --test parse_real_files igsymbols_decoder_emits_decoded_symbols_with_provenance -- --nocapture` | Phase 14 igSymbol 不退化 | 27 跨 fixture 维持 | `progress.jsonl` |
| `cargo test --test parse_real_files graphic_group_decoder_ratchets_fixture_counts_and_header_fields -- --nocapture` | Phase 15 GraphicGroup 不退化 | 352 跨 fixture 维持 | `progress.jsonl` |
| `cargo build --locked --workspace --all-targets` | 全 workspace 编译 | exit 0，无 warning | `progress.jsonl` |
| `cargo test --locked --workspace --all-targets` | 全测试 | exit 0 | `progress.jsonl` |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | lint gate | exit 0 | `progress.jsonl` |
| `cargo fmt --all -- --check` | formatting gate | exit 0 | `progress.jsonl` |
| `bash .github/scripts/check-missing-docs.sh` | missing_docs ratchet | baseline 不上升 | `progress.jsonl` |

Windows 本地若 `bash` 不可用，用等价命令：

```powershell
cargo rustdoc --lib --locked -- -W missing-docs
```

并与 `.github/missing-docs-baseline.txt` 对账。

## 手工 / 半手工核查

| 核查项 | 怎么做 | 通过判据 |
|---|---|---|
| 旧文案是否清楚 | 检查 `src/`, `tests/`, `CHANGELOG.md`, relevant docs | 任何 `PrimitiveArc` / `GArc2d` 旧描述都标注 legacy / deprecated / historical misidentification |
| normalized geometry 是否不再误导 | 检查 `src/geometry.rs` diff | 默认 decoded user-facing path 是 `Annotation`；旧 `Arc` emission 不再 silently 表示 PSM 0x0030 |
| schema 是否可解释 | 检查 `src/schema.rs` tests 和 generated schema | JSON consumer 能看出迁移后的 contract；没有 dangling names |
| Phase 16 是否保持权威路径 | 看 parse_real_files 输出 | `decoded_jstyle_overrides` 仍 98；`Annotation` emission 仍 Decoded |
| panic-safety 是否完整 | 看 `tests/parser_panic_safety.rs` | 保留的 public parser entry 都在 matrix；删除的 entry 无残留 import |
| changelog 是否可迁移 | 看 `CHANGELOG.md` Phase17 段 | 包含 migration note、old surface、new surface、breaking/change behavior |

## Evidence JSONL 规则

每完成一个 acceptance item，在 `progress.jsonl` append 一条 JSON line，例如：

```json
{"type":"inventory","timestamp":"2026-05-17T...Z","ac":["AC1"],"command":"rg \"PrimitiveArc|primitive_arc|decode_primitive_arcs|SheetPrimitiveArcDecoded|DecodedPrimitiveArcRecord\" src tests CHANGELOG.md docs goals","summary":"public parser=2; model DTO=1; SheetGeometry field=1; geometry Arc emission=1; schema refs=N; tests=N; docs historical=N"}
```

用户决策证据：

```json
{"type":"decision","timestamp":"...","ac":["AC2"],"strategy":"soft-deprecate|rename|pipeline-remove|full-remove","arc_emission":"remove|legacy-only|keep-with-warning","user_acked":true}
```

迁移实现证据：

```json
{"type":"implementation","timestamp":"...","ac":["AC3","AC5","AC6","AC8"],"files":["src/parsers/sheet_records.rs","src/model.rs","src/streams/cluster.rs","src/geometry.rs","src/schema.rs","tests/parse_real_files.rs","tests/parser_panic_safety.rs","CHANGELOG.md"],"summary":"legacy PrimitiveArc path ...; authoritative JStyleOverride path preserved"}
```

## 收口检查

merge 或声明完成前按顺序跑：

```powershell
cargo build --locked --workspace --all-targets
cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
bash .github/scripts/check-missing-docs.sh
```

任一 gate 失败：停止，记录 blocker，不继续扩大 scope。

## 完成签名

最后 append：

```json
{"type":"goal_complete","timestamp":"...","phase":"17","migration_strategy":"...","primitive_arc_surface":"deprecated|renamed|pipeline_removed|removed","jstyle_override_count":98,"annotation_emission_preserved":true,"phase14_other_baselines_preserved":true,"phase15_audit_preserved":true,"gates":"5/5 green"}
```

然后暂停等用户签收，不主动扩到 `0x0010`、RAD siblings 或 tag extraction。
