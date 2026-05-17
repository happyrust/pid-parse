# Plan: Phase 17 `PrimitiveArc` 兼容路径退役 / 迁移

## 1. 方案总览

Phase 17 是一个 **public contract migration**，不是新的 byte-layout
decoder。Phase 16 已经证明 PSM type `0x0030` 的 authoritative path 是
`JStyleOverride`，但仓库仍保留 Phase 14 的 `PrimitiveArc` 兼容路径并在
normalized geometry 中 emit `Arc`。这个状态对新 consumer 有误导风险。

推荐策略是分两层处理：

1. **先做决策和迁移说明**：列出旧 surface，确认是否允许破坏性删除。
2. **再做最小实现**：优先把旧路径从 normalized geometry 默认输出中移除
   或降级为 legacy/audit-only，同时保持 parser panic-safety 和 fixture
   baseline 可审计。

不要在本 phase 顺手解决 `0x0010` tail、其它 RAD siblings、plant tag
提取。这些已有 Phase 18/19/future work 边界。

## 2. 迁移策略候选

| 策略 | 适用场景 | 行为 | 风险 |
|---|---|---|---|
| A. Soft deprecate | 需要兼容现有 JSON/API consumer | 给旧 parser/DTO/field 加 deprecated docs，pipeline 暂留，normalized geometry 停止默认 Arc emission 或标注 legacy | schema 仍有旧字段，误用风险降低但未消失 |
| B. Rename legacy surface | 可以接受 breaking schema rename | `PrimitiveArc` 改名为 `LegacyJStyleArcHypothesis` / 类似审计名，旧字段迁移 | 改动面大，测试/serde/schema 全部受影响 |
| C. Remove pipeline collection | 下游已迁到 Phase 16 | parser 可保留测试入口，但 `SheetGeometry` 不再填充 `decoded_primitive_arcs`，normalized geometry 只走 `Annotation` | 破坏依赖旧 JSON 字段的 consumer |
| D. Full remove | 无外部兼容要求 | 删除旧 decoder/DTO/tests/schema mapping | 最大破坏性，不推荐作为首选 |

默认推荐 **A 或 C**：

- A 适合不确定外部 consumer 的场景；
- C 适合确认 normalized geometry / publish users 都已迁到 `Annotation` 的场景。

Slice C 必须让用户拍板。没有拍板前，不做破坏性删除。

## 3. Slice 表

| Slice | 目的 | 主要文件 | Done when | 风险 |
|---|---|---|---|---|
| A | Surface inventory：列出所有 `PrimitiveArc` symbols / tests / schema references | `src/**/*.rs`, `tests/**/*.rs`, `CHANGELOG.md`, docs | inventory 记录到 `progress.jsonl`，按 public/internal/test 分组 | 漏掉 serde/schema 字符串 |
| B | Consumer impact audit：确认 normalized geometry 与 JSON schema 影响 | `src/geometry.rs`, `src/schema.rs`, `tests/parse_real_files.rs` | 明确哪些 outputs 会变、哪些 baseline 必须保留 | 外部 consumer 不可见 |
| C | 用户决策：A/B/C/D 迁移策略 + 是否移除旧 Arc emission | 对话 + `blockers.md` | 决策写入 `progress.jsonl` | 未获授权就破坏 API |
| D | Parser/model migration | `src/parsers/sheet_records.rs`, `src/model.rs`, `src/streams/cluster.rs`, `src/cfb/reader.rs` | 旧 surface deprecated/renamed/removed 与 Slice C 一致 | missing_docs / serde 破坏 |
| E | Geometry migration | `src/geometry.rs` | 旧误名路径不再默认 emit misleading `Arc`，或 legacy note 明确 | normalized inventory baseline 需重算 |
| F | Schema + docs + changelog | `src/schema.rs`, `CHANGELOG.md`, relevant docs | schema ratchet 反映新 contract；docs 不再把 0x0030 称作真实 Arc | downstream schema break |
| G | Tests | `tests/parse_real_files.rs`, `tests/parser_panic_safety.rs`, parser unit tests | Phase 16 98 annotation baseline 绿；旧路径测试与策略一致 | 误删 panic-safety |
| H | 收口验证 | `progress.jsonl`, gates | 5 道 gate 绿；AC evidence 完整 | CI 时间长 |

## 4. Acceptance Criteria

- [ ] **AC1**：完成 `PrimitiveArc` surface inventory，至少覆盖
      parser API、model DTO、`SheetGeometry` field、cluster integration、
      geometry emission、schema mapping、tests、docs/changelog。
- [ ] **AC2**：用户拍板迁移策略（A/B/C/D）和旧 `Arc` emission 处理方式。
- [ ] **AC3**：实现与 AC2 一致；所有旧 `GArc2d` / `PrimitiveArc` 文案若保留，必须标注 deprecated / legacy / historical misidentification。
- [ ] **AC4**：`decoded_jstyle_overrides` parser-level 98 条 baseline 保持。
- [ ] **AC5**：normalized geometry 的 `Annotation` emission baseline 保持；若移除旧 `Arc` emission，相关 inventory expected sum 更新并解释。
- [ ] **AC6**：schema ratchet 更新：新增 / 删除 / deprecated surface 的可见 JSON contract 与文档一致。
- [ ] **AC7**：panic-safety matrix 覆盖所有保留 public parser entry；删除 entry 时同步移除测试引用。
- [ ] **AC8**：`CHANGELOG.md` 写明 Phase 17 migration，包含 consumer migration note。
- [ ] **AC9**：Phase 14 其它 decoder（GLine2d / igLine2d / igLineString2d /
      igPoint2d / igTextBox / igSymbol2d）和 Phase 15 GraphicGroup baseline 不退化。
- [ ] **AC10**：5 道 gate 通过：build / test / clippy -D warnings / fmt /
      missing_docs ratchet。
- [ ] **AC11**：`progress.jsonl` 对 AC1-AC10 都有具体命令 / artifact /
      输出摘要。

## 5. Required Evidence

| Requirement | Evidence to inspect | Where recorded |
|---|---|---|
| AC1 | `rg "PrimitiveArc|primitive_arc|decode_primitive_arcs|SheetPrimitiveArcDecoded|DecodedPrimitiveArcRecord"` 输出摘要 | `progress.jsonl` |
| AC2 | 用户选择的迁移策略与旧 Arc emission 决策 | `progress.jsonl` |
| AC3 | git diff：docs/comments/attributes/API rename 或 deprecation | `progress.jsonl` |
| AC4 | `cargo test --test parse_real_files jstyle_override_decoder_emits_audit_records_with_provenance -- --nocapture` | `progress.jsonl` |
| AC5 | `cargo test --test parse_real_files normalized_geometry_probe_baseline_on_real_fixture -- --nocapture` | `progress.jsonl` |
| AC6 | `cargo test --locked --lib schema` 或 full workspace schema tests | `progress.jsonl` |
| AC7 | `cargo test --locked -j 1 --test parser_panic_safety` | `progress.jsonl` |
| AC8 | `CHANGELOG.md` diff | `progress.jsonl` |
| AC9 | targeted Phase 14 / 15 cross-fixture tests | `progress.jsonl` |
| AC10 | 5 道 pre-commit gate 输出 | `progress.jsonl` |

## 6. Phase Boundaries

最低可交付版本：

1. AC1 + AC2 完成。
2. 旧 `PrimitiveArc` normalized `Arc` emission 不再误导新 consumer。
3. Phase 16 `JStyleOverride` / `Annotation` baseline 保持。
4. Schema / changelog 写清 migration。

可选条件性交付：

- 直接 rename / remove `decoded_primitive_arcs` public schema 字段。
- 保留 deprecated compatibility parser 但从 pipeline 断开。
- 给旧 48 条 baseline 改成 explicit legacy test，而不是 decoded geometry test。

若发现外部 consumer 仍依赖旧 JSON 字段，则 Phase 17 应选择 Soft deprecate
并暂停删除；后续单独安排 breaking-change phase。

## 7. Completion Audit

声明完成前逐项对照 AC1-AC11。任何继续出现的 `PrimitiveArc` / `GArc2d`
文案都必须回答两个问题：

1. 它是否是 legacy compatibility / historical note？
2. 它是否会让新 consumer 误以为 PSM `0x0030` 是真实 arc geometry？

如果第二个答案是“会”，Phase 17 尚未完成。
