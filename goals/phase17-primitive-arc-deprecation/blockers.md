# Blockers: Phase 17 `PrimitiveArc` 兼容路径迁移

## 当前 Known Blockers

### B1 — 迁移策略未拍板 [RESOLVED]

| 项 | 内容 |
|---|---|
| **现状** | 用户已选择 D: full remove，并选择移除旧 PSM `0x0030` `Arc` emission |
| **风险** | 已接受 breaking migration；`CHANGELOG.md` 需记录 consumer migration |
| **解锁动作** | 已完成，见 `progress.jsonl` `decision` |
| **owner** | user |
| **解锁判据** | `progress.jsonl` 已写入 AC2 decision 记录 |

### B2 — 外部 consumer 依赖未知 [RESOLVED BY USER DECISION]

| 项 | 内容 |
|---|---|
| **现状** | 用户选择 full remove，显式接受 breaking JSON/API 行为 |
| **风险** | 下游需迁移到 `decoded_jstyle_overrides` / `Annotation` |
| **解锁动作** | 在 `CHANGELOG.md` 写入 migration note |
| **owner** | user + agent |
| **解锁判据** | migration note 已写入 `CHANGELOG.md` |

### B3 — normalized geometry baseline 变更需要显式接受 [RESOLVED]

| 项 | 内容 |
|---|---|
| **现状** | 用户已选择移除旧 `Arc` emission；PSM `0x0030` 只通过 `Annotation` 输出 |
| **风险** | normalized geometry expected counts 需更新并验证 |
| **解锁动作** | 更新 tests 并跑 targeted verification |
| **owner** | user |
| **解锁判据** | normalized geometry baseline tests 通过 |

## Open Questions

### Q1 — deprecated alias 需要保留多久？ [RESOLVED]

最终决定：直接 remove，不留 `#[deprecated]` alias。Public surface
（parser API / model DTO / schema field / record kind variant）一次性删除。

### Q2 — `SheetRecordKind::PrimitiveArc` 是否保留？ [RESOLVED]

最终决定：删除 `SheetRecordKind::PrimitiveArc` variant 与
`primitive_arc` 默认 schema entry。schema 现在只暴露
`SheetRecordKind::JStyleOverride` + `jstyle_override` 作为 PSM
`0x0030` 唯一 typed mapping。

### Q3 — `PidGraphicKind::Arc` 是否仍允许代表未来真实 arc？ [RESOLVED]

最终决定：保留 `PidGraphicKind::Arc` 通用 enum variant，但
`PidGraphicKind::Arc::decoded_sheet_record_kind` 现在返回 `None`
（Phase 17 之前会返回 `Some(SheetRecordKind::PrimitiveArc)`）。
未来真实 arc decoder 落地时可重新接通 Sheet record kind。

## Stop And Ask

任一条件成立立即停手，写 `progress.jsonl`，等用户回复：

1. 需要删除 / rename public JSON schema 字段（例如 `decoded_primitive_arcs`）。
2. 需要删除 `DecodedPrimitiveArcRecord` 或 `SheetPrimitiveArcDecoded` public type。
3. 需要移除 `PidGraphicKind::Arc` variant 本身，而不是只移除旧 PSM
   `0x0030` emission。
4. 发现外部 docs / tests 仍要求 `primitive_arc_decoder...` 作为 Decoded
   Arc baseline，但用户未确认 migration。
5. 移除旧 Arc emission 后 normalized geometry expected sum 与 Phase 16
   `Annotation` bucket 无法平衡。
6. Phase 16 `decoded_jstyle_overrides` 98 条 baseline 退化。
7. Phase 14 其它 decoder 或 Phase 15 GraphicGroup baseline 退化。
8. 5 道 pre-commit gate 中任一项连续失败且原因不是本 diff 内的简单错误。

## 高风险动作（必须先授权）

- commit / push 任意改动。
- 删除 public parser API / model DTO / schema field。
- 修改 `PidGraphicKind` / `SheetRecordKind` public enum 的既有 variant。
- 删除或重写历史 Phase 14 / Phase 16 analysis 文档。
- 将 `0x0010` sub-record、RAD sibling type code、plant tag extraction 纳入本 phase。
- 提交 `dlls/`、`.i64`、私有 fixture。

## 当前状态总表

| ID | 类型 | 状态 | next action | owner |
|---|---|---|---|---|
| B1 | decision | RESOLVED | full remove | user |
| B2 | compatibility | RESOLVED BY USER DECISION | breaking migration note written in CHANGELOG.md | user + agent |
| B3 | baseline | RESOLVED | old Arc emission removed, baselines verified | user |
| Q1 | question | RESOLVED | no alias kept; surface fully removed | user + agent |
| Q2 | question | RESOLVED | `SheetRecordKind::PrimitiveArc` removed | user + agent |
| Q3 | question | RESOLVED | `PidGraphicKind::Arc` variant kept; mapping returns None | user + agent |
