# Blockers: Phase 16 PSM 0x0030 真实归属与 decoder 重写

## 当前 Known Blockers

> 2026-05-16 closeout update: the blockers below were written at goal start.
> Slice A/B/C/F progress has since resolved the Phase 16 decision points:
> `style.dll` provided the real class (`JStyleOverride`), the user-selected
> full Annotation emission path landed, and the schema conflict is closed in
> `docs/analysis/2026-05-16-jstyleoverride-v3-fields.md` §11.

### B1 — `j2dsrv.dll` / `style.dll` IDA evidence [RESOLVED]

| 项 | 内容 |
|---|---|
| **现状** | 已完成。Evidence chain: `radsrvitem.dll` PSM table → `J2DSrv.dll` consumer → `JUTIL.dll` RAD CLSID registry → `style.dll` implementer |
| **风险** | 已解除；DTO 命名和 schema 不再基于 GArc2d 假设 |
| **解锁动作** | 已完成，见 `progress.jsonl` `ida_slice_b_*` / `schema_conflict_closed` |
| **owner** | agent + user |
| **解锁判据** | `JStyleOverride` class, vtable, V2/V3 IO paths and field table recorded |

### B2 — `47FCC338` 真实类名 / DTO 重命名 [RESOLVED]

| 项 | 内容 |
|---|---|
| **现状** | 已落地为 `SheetJStyleOverrideDecoded` / `DecodedJStyleOverrideRecord` / `decode_jstyle_overrides` |
| **风险** | 已解除；Phase 14 `decode_primitive_arcs` 保留为 additive compatibility path |
| **解锁动作** | 已完成 |
| **owner** | user + agent |
| **解锁判据** | `CHANGELOG.md` and final summary record the rename/additive decoder decision |

### B3 — `PidGraphicKind` 新 variant 是否新增 [RESOLVED]

| 项 | 内容 |
|---|---|
| **现状** | 已选择 full Annotation path：`PidGraphicKind::Annotation { anchor, rotation_angle, secondary_radius, note }` |
| **风险** | Additive enum variant documented in final summary; in-crate exhaustive matches updated |
| **解锁动作** | 已完成 |
| **owner** | user |
| **解锁判据** | 98 `PidGraphicEntity { kind: Annotation, confidence: Decoded }` cross-fixture |

### B4 — 47FCC338 Save / Load 是否动态序列化 [RESOLVED]

| 项 | 内容 |
|---|---|
| **现状** | Fixture path is fixed Version 3 64-byte layout via `style.dll!sub_1000F030` (13 `IOContext::DoIO` calls) |
| **风险** | V2 68-byte path remains future work for non-fixture data, not a Phase 16 blocker |
| **解锁动作** | 已完成 |
| **owner** | agent |
| **解锁判据** | V3 byte count exactly matches fixture payload; field table documented |

## Open Questions

### Q1 — `+24..31` 是 rotation 还是 sweep_extent？ [RESOLVED FOR PHASE 16]

probe 显示该字段取值集中在 `{0, π/2, 3π/2, 2π}`。两种 hypothesis：

- **A**：rotation_angle（仪表符号的方向），主轴沿 +X 时 = 0，沿 +Y 时
  = π/2，沿 -X 时 = π（fixture 里没观察到 π，因为可能少用）
- **B**：sweep_extent（弧度跨度），全圆 = 2π，半圆 = π，1/4 圆 = π/2

Phase 16 exposes this as `rotation_angle` in the SmartPlant annotation view.
The raw audit DTO still preserves the underlying RAD field bytes.

### Q2 — `+32..63` 的 reference 链精确 schema [DEFERRED]

probe 已识别 `+32..33` u16 是被引用 PSM type code（`0x0018` igLine2d /
`0x004D` igTextBox / `0x00CE` igSymbol2d / `0x00FA` GraphicGroup /
`0x0084` igLineString2d 等），但 `+34/+38/+42/+44/+46` 等字段的语义
（sub_kind、index、parent_ref、flags 等）未锁定。

### Q3 — attribute tail 中 plant tag 是否对所有 0x0030 record 都存在？ [DEFERRED]

只在 DWG-0202 hit[1] (btf=384, oid=1) dump 中看到 `"A3-FA060201"`，
其他 dump 未见。可能：

- 只有 btf ≥ 某阈值 的 record 才含 tag
- tag 是可选 attribute，按 flag 决定是否存在
- DWG-0202 hit[1] 是异常 record

需要 probe v4 按 btf 分桶统计 tail 起始字节模式。

### Q4 — `1.0` 常量 marker 的语义 [DEFERRED]

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
| B1 | blocker | RESOLVED | Evidence in `progress.jsonl` + analysis doc | user + agent |
| B2 | blocker | RESOLVED | `SheetJStyleOverrideDecoded` landed | user + agent |
| B3 | decision | RESOLVED | `PidGraphicKind::Annotation` landed | user |
| B4 | unknown | RESOLVED | V3 fixed 64B path documented | agent |
| Q1 | question | RESOLVED FOR PHASE 16 | `rotation_angle` + raw audit fields | agent |
| Q2 | question | DEFERRED | Phase 18/19 reference-chain work | agent |
| Q3 | question | DEFERRED | Tail decoder / instrument tag extraction | agent |
| Q4 | question | DEFERRED | Tail semantic reverse engineering | agent |
