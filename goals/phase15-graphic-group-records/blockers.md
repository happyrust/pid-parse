# Blockers: Phase 15 PSM 0x00FA GraphicGroup Records

## 当前 Known Blockers

### B1 — `0x00FA` stable header / variable tail boundary [PARTIALLY RESOLVED]

| 项 | 内容 |
|---|---|
| **现状** | Phase 15 已解出 stable header，并将 raw tail 保留在 audit-only decoded group collection |
| **风险** | 过早把 tail 命名成 child list / style / color / layer，会把未验证猜想固化进 stable model |
| **解锁动作** | 已完成 size/sub_type bucket audit；child OID list 仍需后续更强规则 |
| **owner** | agent |
| **解锁判据** | stable header 已解锁；tail 仍只允许 raw / audit-only |

### B2 — Public DTO / schema 字段边界 [RESOLVED AUDIT-ONLY]

| 项 | 内容 |
|---|---|
| **现状** | 已新增 `SheetGeometry::decoded_graphic_groups` audit-only collection |
| **风险** | Public schema 一旦发布，字段名和语义会成为下游契约 |
| **解锁动作** | 只暴露 header + raw payload；不暴露 `child_oids`，不 emit normalized geometry |
| **owner** | 用户 + agent |
| **解锁判据** | audit-only/raw payload 输出已落地，full gates 通过 |

## Open Questions

### Q1 — `sub_type_word` 的语义是什么？ [OPEN]

已观测值包括 `0x0001` / `0x0002` / `0x0007` 一类版本或子类型。Phase 15
只记录 raw value；除非 cross-fixture evidence 能证明含义，否则不命名语义。

### Q2 — tail 是否真的是 child OID list？ [OPEN / AUDIT-ONLY]

Phase 14 summary 推测 tail 是 variable OID references list。Phase 15 必须验证：

- candidate OID 是否能在同一 Sheet geometry decoded records 中找到
- candidate OID 是否随 record size 按固定 stride 增长
- trailing zeros / sentinels 是否稳定
- 不同 sub_type 是否有不同 list encoding

当前验证不足以暴露 stable child list；本阶段只暴露 raw reference payload。

### Q3 — group association 应进入 normalized geometry 还是 audit-only？ [RESOLVED]

本阶段选择 audit-only。`geometry.rs` 不从 `0x00FA` records 生成
`PidGraphicEntity`，也不建立 group -> child provenance association。

## Stop And Ask

任一条件成立立即停手，写 `progress.jsonl`，等用户回复：

1. 需要新增或修改 public schema 字段，但字段语义仍依赖猜测。
2. `0x00FA` tail 在不同 fixture / sub_type 间无法归一，decoder 只能覆盖单一 fixture。
3. decoder 在 random/adversarial input 上出现明显 false positive。
4. child OID list 与 decoded geometry OID 对不上，或对上率低于可解释阈值。
5. 需要把 group association 投射到 `geometry.rs` 的 normalized entities。
6. 需要引入新依赖、提交私有 fixture、提交 DLL、push、force push 或改写 history。
7. 5 道 pre-commit gate 中任一项连续失败且原因不是本 diff 内的简单错误。

## 高风险动作（必须先授权）

- 提交或 push 任意改动。
- 修改 `PidGeometryConfidence`、`PidGraphicKind`、coverage tier public contract。
- 把 guessed tail fields 命名为 style/color/layer。
- 把 `0x0010` 纳入同一个 Phase 15 goal。
- 删除或改写 Phase 14 decoder tests。
- 提交 `dlls/`、`.i64`、fixture 私有样本。

## 当前状态总表

| ID | 类型 | 状态 | next action | owner |
|---|---|---|---|---|
| B1 | blocker | PARTIAL | stable header done; tail remains raw/audit-only | agent |
| B2 | decision | RESOLVED | audit-only collection landed | 用户 + agent |
| Q1 | question | OPEN | sub_type semantics not named | agent |
| Q2 | question | OPEN | candidate OID list stays probe/audit-only | agent |
| Q3 | question | RESOLVED | normalized geometry association deferred | 用户 + agent |
