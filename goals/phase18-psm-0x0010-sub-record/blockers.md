# Blockers: Phase 18 PSM 0x0010 sub-record family audit-only decoder

## Open Questions

### Q1 — DTO 命名最终拍板 [OPEN]

候选命名（parser → model）：

- `SheetSubRecord0x0010Decoded` → `DecodedSubRecord0x0010Record`（**推荐**，
  与现有 `SheetGraphicGroupDecoded` / `DecodedGraphicGroupRecord` 命名风格
  一致；type code 直接出现在名字里，避免假装我们知道真实类）
- `SheetTypeOx0010Decoded` → `DecodedTypeOx0010Record`（同上，但更短）
- `Sheet0010FragmentDecoded` → `Decoded0010FragmentRecord`（贴近 Phase 14
  §6.3 "fragment" 措辞）

执行时若需调整，先在 progress.jsonl 记录决策，再改 DTO 名。Q1 不阻塞 Slice A。

### Q2 — `raw_payload` 的 serde 形态 [OPEN]

候选：

- `Vec<u8>` 默认（serde_json 序列化为 number array）— 与 Phase 15
  `raw_variable_tail` 保持一致；JSON 大但可读
- 自定义 `serialize_with = "hex"` / `"base64"` — JSON 紧凑但需额外
  helper

**推荐**：先用 `Vec<u8>` 默认与 Phase 15 一致，避免引入新依赖。
若 JSON 体积成为问题，下个 phase 再切换。

### Q3 — 是否要在 ratchet test 里 assert sub-kind size 分布 [DEFERRED]

probe 输出显示 size 高度多形态。是否在 ratchet test 中固化每个 fixture
的 top-N size bucket（如 DWG-0201 70B=53 / 76B=24…）？

- ✅ 优势：catch 验证规则漂移
- ❌ 劣势：增加 test 维护成本；本 phase 是 audit-only，size 分布属于
  sub-kind 反向工程范畴，本 phase 不该 over-fit

**决定**：本 phase 只 ratchet 跨 fixture 总数 638；不 freeze size 分布。
若日后 sub-kind decoder 落地，再单独 ratchet。

## Stop And Ask

任一条件成立立即停手，写 `progress.jsonl`，等用户回复：

1. 跨 fixture 总数 < 638 或 > 638（超过 ±1）：说明 validation 规则不对，
   或 probe baseline 失效。
2. Phase 14 / 15 / 16 / 17 任一 baseline 退化（cross-fixture decoded
   counts 变化或 5 道 gate 任一失败原因不是简单错误）。
3. 出现要新增 `PidGraphicKind` variant 的诱因（说明 audit-only 选择
   被挑战）。
4. 出现要解析 reference chain（JStyleOverride.ref → 0x0010） 的诱因。
5. 出现要把 sub-kind discriminator 命名为字段的诱因。
6. 5 道 gate 中任一项连续失败且原因不是本 diff 内的简单错误。
7. `missing_docs` ratchet 上升（current > baseline=0）。
8. cargo audit 在 CI 暴露新 CVE（虽不属本 phase scope，但 CI 失败
   会卡住）。

## Dangerous Or High-Risk Actions

必须先获得用户授权：

- commit / push 任意改动
- 删除任何已存在 public parser API / model DTO / schema field /
  Sheet* test
- 加载新 IDA instance（确认 0x0010 真实类身份）
- 修改 Phase 14 / 15 / 16 / 17 任一 stable DTO 既有字段
- 提交 `dlls/`、`.i64`、私有 fixture
- 把 Phase 18 scope 扩到 sub-kind discriminator 反向、reference
  resolver、plant tag extraction、`PidGraphicKind` 新 variant
- 修改 Phase 14 / 15 / 16 / 17 analysis 文档

## Known Blockers

| ID | 类型 | 状态 | next action | owner |
|---|---|---|---|---|
| B1 | scope | RESOLVED BY DEFAULT | audit-only strategy adopted; no IDA required for this phase | user + agent |
| Q1 | question | OPEN | 执行时按推荐命名落地，progress.jsonl 记录 | agent |
| Q2 | question | OPEN | 执行时按推荐 `Vec<u8>` 落地 | agent |
| Q3 | question | DEFERRED | 不在本 phase ratchet size 分布 | agent |

## 当前状态总表

audit-only 路径 + Phase 15 GraphicGroup 模板复用 = 低风险落地路径。
B1 默认解锁；Q1-Q3 不阻塞 Slice 启动。任何想升级到 sub-kind typed
decoder 的诱因必须先经过 Stop And Ask 用户确认。
