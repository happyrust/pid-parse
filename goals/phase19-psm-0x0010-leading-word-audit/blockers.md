# Blockers: Phase 19 PSM 0x0010 leading-word audit

## Open Questions

### Q1 — `leading_word` 命名最终拍板 [OPEN]

候选：

- `leading_word: Option<u16>`（**推荐**，描述字节位置 +0..+1 + 类型，
  完全不暗示语义；mirrors Phase 15 `raw_variable_tail` 的命名风格）
- `payload_word_0: Option<u16>`（更直白的偏移注释，但读起来啰嗦）
- `first_word: Option<u16>`（更短，但不如 `leading_word` 直观）

执行时若需调整，先在 progress.jsonl 记录决策，再改字段名。Q1 不阻塞
Slice A。

**禁止命名**：`sub_kind` / `record_kind` / `family_tag` / `payload_kind`
/ `discriminator` / `type_tag` —— 全部暗示语义。

### Q2 — `Option<u16>` 序列化形态 [OPEN]

候选：

- 默认 `serde::Serialize` → `null` (None) / number (Some(u16))
- 自定义 `serialize_with` 把 Some(0x0002) 写成 `"0x0002"` 字符串

**推荐**：先用默认，与 Phase 15/18 的字段序列化一致；JSON 数字精度
对 u16 完全够用，不需要 hex 字符串。

### Q3 — Phase 19 ratchet 数字与 probe 数字差 ±4 处理 [DEFERRED]

probe 报告 578 records / 0x0002=164；Phase 18 ratchet 是 582 records。
差 4 records 大概率因为：

- probe `iter_records` 的 `payload_end > bytes.len()` 是严格 > 而
  Phase 18 decoder 是另一种边界处理
- probe 没复用 Phase 18 `SUB_RECORD_0X0010_MIN/MAX_BYTES_TO_FOLLOW`
  constants

**决定**：Slice D 写 ratchet 前先跑一次 decoder-side 计数，把
probe 估算的 0x0002=164 / 0x0003=21 / 0x0001=18 **校准到 decoder
ground truth**。如校准后数字偏离 ±5%（即 0x0002 ≠ 156..172），停手
写 progress.jsonl `[discovery]` entry 等用户确认。

### Q4 — 是否要 ratchet leading_word 完整直方图 [DEFERRED]

probe 显示 top 12 words；是否在 ratchet test 中固化全部 ≥ 5
records 的 leading_word 值？

- ✅ 优势：catch decoder 漂移
- ❌ 劣势：~30+ ratchet 值；维护成本高；本 phase 是 audit not 反向

**决定**：本 phase 只 ratchet top 3 + None（0x0002 / 0x0003 / 0x0001 /
None）。其余值留给 Phase 20 sub-kind 反向。

## Stop And Ask

任一条件成立立即停手，写 `progress.jsonl`，等用户回复：

1. Phase 18 ratchet 582 数字退化（说明 leading_word 引入意外
   改了 validation 或字段顺序）。
2. Phase 14 / 15 / 16 / 17 任一 baseline 退化。
3. Slice D decoder-side 0x0002 计数偏离 probe 数字 ±5%（即不在
   156..172 范围）。
4. 出现要新增 `PidGraphicKind` variant 的诱因。
5. 出现要把 `leading_word` 重命名为 `sub_kind` / `record_kind` 等
   语义字段的诱因（即使 0x0002 = 28% 覆盖也不行；本 phase 是 audit）。
6. 出现要按 size bucket 给不同 discriminator 偏移的诱因（属于
   Phase 20）。
7. 出现要解析 reference chain（JStyleOverride.ref → 0x0010）的诱因。
8. 5 道 gate 中任一项连续失败且原因不是本 diff 内的简单错误。
9. `missing_docs` ratchet 上升（current > baseline=0）。
10. cargo audit 在 CI 暴露新 CVE。

## Dangerous Or High-Risk Actions

必须先获得用户授权：

- commit / push 任意改动
- 删除任何已存在 public parser API / model DTO / schema field /
  Sheet* test / Phase 14-18 既有字段
- 加载新 IDA instance（确认 0x0010 sub-kind 真实类身份）
- 修改 Phase 14 / 15 / 16 / 17 / 18 任一 stable DTO 既有字段
- 提交 `dlls/`、`.i64`、私有 fixture
- 把 Phase 19 scope 扩到 size 31 bucket 反向 / 多偏移 discriminator
  / sub-kind 命名 / reference resolver / `PidGraphicKind` 新 variant
- 修改 Phase 14 / 15 / 16 / 17 / 18 analysis 文档

## Known Blockers

| ID | 类型 | 状态 | next action | owner |
|---|---|---|---|---|
| B1 | scope | RESOLVED BY DEFAULT | leading_word audit-only strategy adopted; no IDA required | user + agent |
| Q1 | question | OPEN | 执行时按 `leading_word: Option<u16>` 落地 | agent |
| Q2 | question | OPEN | 执行时按默认 serde 落地 | agent |
| Q3 | question | DEFERRED | Slice D 校准 decoder vs probe，偏离 > 5% 才停 | agent |
| Q4 | question | DEFERRED | 不在本 phase ratchet 完整直方图 | agent |

## 当前状态总表

audit-only `leading_word` 字段 + Phase 18 模板扩展 = 极低风险落地路径。
B1 默认解锁；Q1-Q4 不阻塞 Slice 启动。任何想升级到 sub-kind typed
naming 的诱因必须先经过 Stop And Ask 用户确认。

整体工作量预估 ~30% of Phase 18（一个值字段 + 一个 ratchet test +
changelog）；预计 0.5–1 天。
