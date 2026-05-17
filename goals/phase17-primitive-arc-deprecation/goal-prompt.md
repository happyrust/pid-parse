# Codex Goal Prompt: Phase 17 `PrimitiveArc` 兼容路径迁移

本目录下的 goal package 用于启动 Phase 17。准备执行时，把下面 `/goal`
段落粘到 Codex：

```text
/goal 开始 Phase 17：退役 / 迁移 Phase 14 的误名 `decode_primitive_arcs` / `SheetPrimitiveArcDecoded` / `DecodedPrimitiveArcRecord` 兼容路径，让 PSM type `0x0030` 的 authoritative user-facing path 成为 Phase 16 已落地的 `decode_jstyle_overrides` + `SheetGeometry::decoded_jstyle_overrides` + `PidGraphicKind::Annotation`。

用 `goals/phase17-primitive-arc-deprecation/` 作为 durable source of truth：

- 读 `brief.md`：使命、背景、约束、非目标、Ask Before、Done Means
- 跟 `plan.md`：迁移策略 A/B/C/D、Slice A-H、AC1-AC11、required evidence
- 跑 `verification.md`：surface inventory、parser/schema/panic-safety/cross-fixture tests、5 道 gate
- 遇到 `blockers.md` 的 Stop-And-Ask 条件时立即暂停、写 `progress.jsonl`、等用户

执行顺序：

1. **Slice A**：用 `rg "PrimitiveArc|primitive_arc|decode_primitive_arcs|SheetPrimitiveArcDecoded|DecodedPrimitiveArcRecord" src tests CHANGELOG.md docs goals` 盘点旧 surface，并把 public/internal/test/historical 分类 append 到 `progress.jsonl`。
2. **Slice B**：审计 consumer impact，重点看 `src/geometry.rs` 旧 `Arc` emission、`src/schema.rs` schema ratchet、`tests/parse_real_files.rs` normalized inventory。
3. **Slice C**：停下来让用户选择迁移策略：A soft deprecate、B rename legacy surface、C remove pipeline collection、D full remove；同时确认是否移除旧 `PidGraphicKind::Arc` emission。没有用户确认，不做破坏性删除 / rename。
4. **Slice D**：按用户选择迁移 parser/model/pipeline surface。保留旧 public parser entry 时加 deprecated / legacy 文档并保持 panic-safety；删除时同步移除 imports/tests/schema references。
5. **Slice E**：迁移 `src/geometry.rs`，确保 PSM `0x0030` 不再默认通过旧误名路径产生 misleading decoded `Arc`。Phase 16 `Annotation` emission 必须保持。
6. **Slice F**：更新 `src/schema.rs`、`CHANGELOG.md`、必要 docs，让 public contract 清楚表达：`PrimitiveArc` 是 Phase 14 historical misidentification，`JStyleOverride` 是 authoritative path。
7. **Slice G**：更新 tests。必须保持 `jstyle_override_decoder_emits_audit_records_with_provenance` 98 条 baseline、`Annotation` normalized emission、Phase 14 其它 decoder baseline、Phase 15 GraphicGroup baseline。
8. **Slice H**：跑 5 道 gate：`cargo build --locked --workspace --all-targets`、`cargo test --locked --workspace --all-targets`、`cargo clippy --locked --workspace --all-targets -- -D warnings`、`cargo fmt --all -- --check`、`bash .github/scripts/check-missing-docs.sh`（Windows 上可用 `cargo rustdoc --lib --locked -- -W missing-docs` 等价核查）。每个 AC 的命令 / artifact / 结果 append 到 `goals/phase17-primitive-arc-deprecation/progress.jsonl`。

不要做：

- 不把 PSM `0x0030` 继续描述为真实 `GArc2d` / `PrimitiveArc`，除非明确标注 legacy / deprecated / historical misidentification。
- 不解析 `0x0010` sub-record family。
- 不反向 RAD `47FCC330..47FCC33E` 其它 siblings。
- 不提取 plant instrument tag。
- 不删除 Phase 16 `JStyleOverride` / `Annotation` authoritative path。
- 不提交 DLL / `.i64` / 私有 fixture。
- 不 commit / push，除非用户明确授权。

完成时 append：

```json
{"type":"goal_complete","timestamp":"...","phase":"17","migration_strategy":"...","primitive_arc_surface":"deprecated|renamed|pipeline_removed|removed","jstyle_override_count":98,"annotation_emission_preserved":true,"phase14_other_baselines_preserved":true,"phase15_audit_preserved":true,"gates":"5/5 green"}
```

然后暂停等用户签收，不主动扩到 `0x0010`、RAD siblings 或 tag extraction。
```

## 启动检查清单

- [ ] `brief.md` / `plan.md` / `verification.md` / `blockers.md` 已读
- [ ] `progress.jsonl` 有 initial scaffold 条目
- [ ] 已读 `docs/plans/2026-05-16-phase16-jstyleoverride-final-summary.md`
- [ ] 已确认 Phase 16 authoritative path 是 `decode_jstyle_overrides` + `Annotation`
- [ ] 首个执行动作是 surface inventory，不是直接删除旧 decoder
- [ ] Slice C 前不做破坏性 schema / public API 改动
