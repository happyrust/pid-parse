# Blockers: Phase 16 PSM 0x0030 真实归属与 decoder 重写

## 当前 Known Blockers

### B1 — `j2dsrv.dll` IDA instance 尚未加载 [OPEN]

| 项 | 内容 |
|---|---|
| **现状** | 当前 10 个 IDA instance 不含 j2dsrv.dll。`radsrvitem.dll` 只有 GUID lookup，没有 47FCC338 的 Save/Load 实现 |
| **风险** | 没有 IDA 反编译就动 DTO 字段名 = 把猜测固化进 stable contract，重蹈 Phase 14 §6.1 覆辙 |
| **解锁动作** | 需要用户在 IDA 里 File → Open → `dlls/j2dsrv.dll`，然后 IDA MCP plugin 会自动注册到新端口（≥ 13347）。完成后 `list_instances` 应能看到 |
| **owner** | 用户 |
| **解锁判据** | `list_instances` 返回的列表里出现 `j2dsrv.dll` |

### B2 — `47FCC338` 真实类名 / DTO 重命名 [OPEN]

| 项 | 内容 |
|---|---|
| **现状** | candidate 命名列在 `plan.md` §2，但最终命名取决于 IDA 拿到的 RTTI string / 类名 |
| **风险** | 错误命名会让下游消费者把它当 "Arc" / "Annotation" / "Instrument" 错用 |
| **解锁动作** | B1 解决后从 IDA `.rdata` 段拿 RTTI `.?AV<类名>@@` 字符串 + 与用户对齐 |
| **owner** | 用户 + agent |
| **解锁判据** | DTO 新名经用户在对话中确认 |

### B3 — `PidGraphicKind` 新 variant 是否新增 [OPEN]

| 项 | 内容 |
|---|---|
| **现状** | 当前 `PidGraphicKind` 含 Line / Arc / Polyline / Point / Text / SymbolInstance 等。如果 J2D 复合 record 是新类（如 Annotation / Instrument），需要新 variant |
| **风险** | 新 variant 改 stable schema，影响所有下游 consumer |
| **解锁动作** | B2 解决后看真实类名决定；若沿用 audit-only 路径则不必新增 |
| **owner** | 用户 |
| **解锁判据** | "新 variant 名 + emission 路径" 或 "继续 audit-only" 二选一确认 |

### B4 — 47FCC338 Save / Load 是否动态序列化 [UNKNOWN]

| 项 | 内容 |
|---|---|
| **现状** | 不知 47FCC338 的 Save 是固定 offset 还是按字段动态 (length + tag + payload) |
| **风险** | 如果是动态序列化，固定 offset decoder 无法覆盖所有变种 |
| **解锁动作** | B1 解决后看 Save 反编译 |
| **owner** | agent |
| **解锁判据** | 反编译完成后归类 (a) fixed-layout (b) tagged-stream (c) hybrid |

## Open Questions

### Q1 — `+24..31` 是 rotation 还是 sweep_extent？ [OPEN]

probe 显示该字段取值集中在 `{0, π/2, 3π/2, 2π}`。两种 hypothesis：

- **A**：rotation_angle（仪表符号的方向），主轴沿 +X 时 = 0，沿 +Y 时
  = π/2，沿 -X 时 = π（fixture 里没观察到 π，因为可能少用）
- **B**：sweep_extent（弧度跨度），全圆 = 2π，半圆 = π，1/4 圆 = π/2

只能等 IDA 看 Save / Load 上下文才能锁定。

### Q2 — `+32..63` 的 reference 链精确 schema [OPEN]

probe 已识别 `+32..33` u16 是被引用 PSM type code（`0x0018` igLine2d /
`0x004D` igTextBox / `0x00CE` igSymbol2d / `0x00FA` GraphicGroup /
`0x0084` igLineString2d 等），但 `+34/+38/+42/+44/+46` 等字段的语义
（sub_kind、index、parent_ref、flags 等）未锁定。

### Q3 — attribute tail 中 plant tag 是否对所有 0x0030 record 都存在？ [OPEN]

只在 DWG-0202 hit[1] (btf=384, oid=1) dump 中看到 `"A3-FA060201"`，
其他 dump 未见。可能：

- 只有 btf ≥ 某阈值 的 record 才含 tag
- tag 是可选 attribute，按 flag 决定是否存在
- DWG-0202 hit[1] 是异常 record

需要 probe v4 按 btf 分桶统计 tail 起始字节模式。

### Q4 — `1.0` 常量 marker 的语义 [OPEN]

DWG-0201 dump[0..2] tail+064 都是 `00 00 00 00 00 00 F0 3F` = 1.0。

候选解读：unit-vector y / 标准化变换矩阵对角线 / scale factor / linkage
block separator。等 IDA 看上下文。

## Stop And Ask

任一条件成立立即停手，写 `progress.jsonl`，等用户回复：

1. `j2dsrv.dll` 加载到 IDA 后 hexrays 反编译失败 / 类没有 vtable / RTTI
   缺失（即 IDA 没能给出真实类名）。
2. IDA 反编译显示 47FCC338 的字段表与 probe 看到的字节布局**严重冲突**
   （比如 IDA 说 +0..7 是 length 而 probe 说是 center.x）。
3. 真实类名涉及商业秘密 / NDA 字符串（如 Intergraph 内部代号），需要用户
   决定如何在公共代码里表达。
4. 重命名需要 break public schema（`PidGraphicKind` 改动 / DTO field
   rename 影响外部 consumer）。
5. 重写后 cross-fixture decoded count < 90 或 > 98（明显 false negative
   或 false positive）。
6. Phase 14 其他 decoder（igLine2d / igTextBox / igSymbol2d 等）的
   baseline 计数因本 PR 退化。
7. 5 道 pre-commit gate 中任一项连续失败且原因不是本 diff 内的简单错误。
8. 需要把 `0x0010` sub-record 一起解开。

## 高风险动作（必须先授权）

- 提交或 push 任意改动。
- 修改 `PidGeometryConfidence` / `PidGraphicKind` / coverage tier public
  contract。
- 把 `0x0030` record 在 stable schema 里命名为 "Arc" / "Circle" / 任何
  IDA 没确认的几何概念。
- 把 J2DSrv 其他 12 个 type code 一起纳入本 phase。
- 删除或改写 Phase 14 / Phase 15 decoder tests。
- 删除 `examples/probe_garc2d_packed_bytes.rs`（即便它名字过时；保留作
  Phase 16 触发证据）。
- 提交 `dlls/`、`.i64`、fixture 私有样本。

## 当前状态总表

| ID | 类型 | 状态 | next action | owner |
|---|---|---|---|---|
| B1 | blocker | OPEN | 用户在 IDA 加载 j2dsrv.dll | 用户 |
| B2 | blocker | OPEN | 等 B1 + 用户确认 | 用户 + agent |
| B3 | decision | OPEN | 等 B2 后决定 | 用户 |
| B4 | unknown | OPEN | 等 B1 后调查 | agent |
| Q1 | question | OPEN | 等 IDA Save 反编译 | agent |
| Q2 | question | OPEN | 等 IDA + probe v4 | agent |
| Q3 | question | OPEN | probe v4 按 btf 分桶 | agent |
| Q4 | question | OPEN | 等 IDA 反编译 | agent |
