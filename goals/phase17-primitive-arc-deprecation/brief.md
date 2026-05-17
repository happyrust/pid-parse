# Phase 17: 退役 Phase 14 `PrimitiveArc` 兼容路径

## 目标产出

把 Phase 16 已证伪的 `decode_primitive_arcs` /
`SheetPrimitiveArcDecoded` / `DecodedPrimitiveArcRecord` 兼容路径，从
“仍对外承诺 `GArc2d` / `Arc` 语义”的状态迁移到明确的 deprecated / renamed
状态，避免下游继续把 PSM type `0x0030` 误当作 arc geometry。

Phase 17 的核心不是继续反向 `JStyleOverride` 字段，而是完成 **API 与 schema
迁移设计 + 最小破坏性实现**：

1. 盘点所有旧 `PrimitiveArc` public surface：parser API、model DTO、
   `SheetGeometry` 字段、schema ratchet、normalized geometry emission、tests、
   docs / changelog。
2. 选定迁移策略：先 deprecate 并转为 audit-only，还是直接 rename / remove
   旧 `Arc` emission。
3. 让 `decoded_jstyle_overrides` + `PidGraphicKind::Annotation` 成为 PSM
   `0x0030` 的唯一 decoded user-facing path。
4. 保留可追溯证据：旧 Phase 14 48 条 “arc” baseline 只能作为 legacy
   compatibility / regression fixture 解释，不能再被文档描述为真实 `GArc2d`。
5. 更新 tests 和 schema 文档，证明 Phase 16 的 98 条 annotation 输出保持，
   且旧误名路径不会继续污染 normalized geometry。

完成后，下游消费者看到的契约应该清楚表达：

- PSM `0x0030` 的真实身份是 `JStyleOverride`。
- `PrimitiveArc` / `GArc2d` 名称是 Phase 14 历史误判，不应作为新代码入口。
- normalized geometry 不再因为旧兼容路径重复 / 错误 emit `Arc` 实体。

## 背景

Phase 16 已完成 `JStyleOverride` 反向工程和严格 additive 落地：

- `decode_jstyle_overrides` 跨 fixture 输出 98 条记录。
- `PidGraphicKind::Annotation` 已从 `decoded_jstyle_overrides` emission。
- Phase 14 `decode_primitive_arcs` 仍保留，输出 48 条误名 “GArc2d”
  记录，并继续在 `geometry.rs` 中 emit `PidGraphicKind::Arc`。
- Phase 16 final summary 明确把 “Deprecate / rename Phase 14
  `decode_primitive_arcs` family” 列为 Phase 17。

这意味着仓库现在同时有两个 PSM `0x0030` 解读：

| 路径 | 状态 |
|---|---|
| `decode_jstyle_overrides` → `decoded_jstyle_overrides` → `Annotation` | Phase 16 authoritative path |
| `decode_primitive_arcs` → `decoded_primitive_arcs` → `Arc` | Phase 14 compatibility path, 已知误名 |

Phase 17 的任务是消除这个双重语义带来的下游误导，同时控制 public schema
破坏范围。

## 上下文（必读）

| 文档 / 文件 | 作用 |
|---|---|
| `docs/plans/2026-05-16-phase16-jstyleoverride-final-summary.md` | Phase 16 完成态与 Future Work 入口 |
| `docs/analysis/2026-05-16-jstyleoverride-v3-fields.md` | 0x0030 真实字段表与 schema conflict closeout |
| `docs/analysis/2026-05-15-garc2d-packed-int-tail.md` | Phase 14 误判被证伪的触发证据 |
| `CHANGELOG.md` | Phase 14 / Phase 16 public surface 历史说明 |
| `src/parsers/sheet_records.rs` | `decode_primitive_arcs` 与 `decode_jstyle_overrides` 并存处 |
| `src/model.rs` | `decoded_primitive_arcs` / `decoded_jstyle_overrides` DTO surface |
| `src/streams/cluster.rs` | Sheet stream decoder collection 接入 |
| `src/geometry.rs` | 旧 `Arc` emission 与新 `Annotation` emission 并存处 |
| `src/schema.rs` | public schema ratchet 和 `SheetRecordKind` mapping |
| `tests/parse_real_files.rs` | cross-fixture baselines 与 normalized geometry inventory |
| `tests/parser_panic_safety.rs` | public parser entry panic-safety matrix |

## 关键约束

- **不再把 PSM `0x0030` 描述为真实 `GArc2d` / `PrimitiveArc`**，除非文案明确标注为 legacy / deprecated / historical misidentification。
- **不扩大到 Phase 18 / 19**：不解析 `0x0010` sub-record，不反向其它
  `47FCC330..47FCC33E` siblings，不提取 plant instrument tag。
- 若选择破坏性删除 / rename public fields，必须先记录 migration note，并确保 schema / tests 同步更新。
- 不删除 Phase 16 authoritative path；`decoded_jstyle_overrides` 98 条 baseline
  和 `Annotation` emission 必须保持。
- 不把旧 48 条 `PrimitiveArc` 当作 normalized decoded `Arc` 输出，除非用户明确选择保留 legacy emission。
- 保留 panic-safety：任何 public parser entry 留存或新增 alias 都必须在
  `tests/parser_panic_safety.rs` 覆盖。
- 5 道 pre-commit gate 必须保持：build / test / clippy -D warnings /
  fmt --check / missing_docs ratchet。

## 非目标

- 不继续反向 `JStyleOverride` tail schema。
- 不新增 `PidGraphicKind` variant（Phase 16 已新增 `Annotation`）。
- 不做 SmartPlant Sheet geometry 写回。
- 不引入新 fixture。
- 不提交 `dlls/`、`.i64`、私有样本。
- 不 commit / push，除非用户明确授权。

## Ask Before（要先问）

- 是否采用破坏性迁移：删除 / rename `decoded_primitive_arcs` public field、
  `DecodedPrimitiveArcRecord` DTO、`SheetRecordKind::PrimitiveArc` mapping。
- 是否保留 deprecated alias（例如仅加 `#[deprecated]`，继续返回 48 条）
  还是把旧 parser 从 pipeline 中断开。
- 是否允许 normalized geometry 中移除旧 `PidGraphicKind::Arc` emission。
- 是否需要为外部 JSON consumer 提供一版 transitional schema note。
- 任何 commit / push / 删除已存在测试前。

## Done Means（完成判据）

同时满足：

1. 所有旧 `PrimitiveArc` / `GArc2d` surface 已盘点并记录迁移决策。
2. 代码实现符合用户选择的迁移策略：deprecated alias / rename / remove 中的一种。
3. normalized geometry 不再默认从旧误名路径产生 misleading `Arc`，或保留路径时明确降级为 legacy/audit-only。
4. `decoded_jstyle_overrides` 98 条 baseline 和 `Annotation` emission 保持。
5. Schema / docs / changelog 明确标注 Phase 14 旧路径为 deprecated / legacy。
6. Parser unit tests、panic-safety、cross-fixture integration、schema ratchet 全部更新。
7. 5 道 gate 通过，`missing_docs` baseline 不上升。
8. `progress.jsonl` 对每个 AC 都有命令 / artifact / 输出摘要。

停止条件全部写入 `blockers.md`。
