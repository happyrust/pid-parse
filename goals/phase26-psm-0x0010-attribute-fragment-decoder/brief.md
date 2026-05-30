# Phase 26: PSM 0x0010 Attribute Fragment Decoder

## 目标产出

把 PSM 0x0010 从 Phase 18 的 **audit-only raw sub-record** 升级为
**typed attribute-fragment decoder**,提取 SmartPlant 承载在 0x0010 里的
工程属性文本(plant tags / line numbers / nominal sizes / drawing refs /
annotation text)。

依据:`docs/analysis/2026-05-31-psm-0x0010-ida-recheck-plan.md` 的
BREAKTHROUGH 节已证明 0x0010 payload 结构为:

```text
payload[0..4]   marker / type   (frequently 0x00010002)
payload[4..12]  8-byte header / aux
payload[12..14] u16 string length (char count)
payload[14..]   UTF-16LE string  (longer records carry more fields/strings)
```

并跨 5 fixture 验证可提取 `A3-FA060201`(位号) / `ODOIL020150 MM`(管线号)
/ `DN80`(口径) / `DWG-0202GP06-02`(图纸号) / `设计温度`(标签) 等。

具体产出:

1. 新 typed decoder `decode_attribute_fragments` + `SheetAttributeFragmentDecoded` DTO
2. 解析 `marker(4) + aux(8) + [u16 len + UTF-16LE]*`,提取 length-prefixed 字符串
3. **从 Sheet 流提取(不是全文件扫描)** —— 全文件扫描含 CFB 噪声,只用于探针
4. **audit-only first**:不 emit `PidGraphicKind`(遵循 source-proven gate),
   promotion 到 Text/Annotation/Attribute 留待后续 phase
5. cross-fixture ratchet(可提取字符串数)+ analysis doc 更新
6. Phase 14–25 baseline 全部不退化

## 背景

Phase 18 把 0x0010 作为 audit-only raw payload 落地(`582` cross-fixture,
`decoded_sub_records_0x0010`)。Phase 19 加了 `leading_word` 审计字段但无法
解释其异质性。Phase 20 经 IDA 确认 GUID `1D1928C0` + PersistTypeTable 身份,
但未恢复 human class name 或 payload 语义。

2026-05-31 本会话(见 recheck-plan 文档)取得突破:

- 模式 A 读原始 PE 独立复现 PersistTypeTable(GUID/tail/20B stride)
- radsrvitem RTTI(188 类)证明 0x0010 是 persist 框架类(`tagAnnotPersistData`
  系),非几何类
- **fixture 数据 UTF-16LE 解码证明 0x0010 携带工程属性文本**
- 结构 `marker(4)+aux(8)+[u16 len+UTF-16LE]` 经 `ODOIL020150 MM`(14 字符,
  btf=42=4+8+2+28)精确验证
- 一举解释 Phase 19 全部困惑:`leading_word=0x0002` 是 marker 低半(28% 真因);
  size 异质 = 不同 tag 长度

本 phase 把这个**分析层突破**落地为**代码层 typed decoder**。

## 上下文(必读)

| 文档 / 文件 | 作用 |
|---|---|
| `docs/analysis/2026-05-31-psm-0x0010-ida-recheck-plan.md` | 本 phase 的全部证据基础(结构 + 跨 fixture 验证 + H2 修正) |
| `src/parsers/sheet_records.rs` `decode_sub_records_0x0010` (~3796) | 现有 audit-only decoder,本 phase 在其旁新增 typed decoder |
| `goals/phase18-.../progress.jsonl` | 582 baseline + per-fixture 计数 |
| `goals/phase20-.../progress.jsonl` | GUID/PersistTypeTable 身份 + 本会话 6 条新 evidence |
| `dlls/radsrvitem.dll.i64` | 字节级 tail 最终确认(Slice B,需 IDA 打开) |
| `tests/parse_real_files.rs` | cross-fixture ratchet 接入点 |
| `tests/parser_panic_safety.rs` | 新 decoder 的 adversarial matrix 接入点 |

## 关键约束

- **从 Sheet 流提取**:复用现有 Sheet 流提取路径(`streams/cluster.rs`),
  **不**用全文件字节扫描(那是探针手段,含 CFB 噪声)
- **audit-only first**:本 phase 只产 typed audit 集合,**不** emit
  `PidGraphicKind`,**不**改 entity confidence/kind
- **保守解码**:只接受能干净解出 length-prefixed UTF-16LE 的记录;
  解不出的保持现有 raw audit 路径
- `582` baseline(`decoded_sub_records_0x0010`)**不动**:新 typed decoder
  与现有 raw decoder **并存**(新增,非替换),各自 ratchet
- **字节级 tail 需 IDA 确认**:多字符串记录的完整 tail(aux 字段语义 +
  多 string 布局)需 `dlls/radsrvitem.dll.i64` 的 `analyze_function` 确认
  → 见 blockers Q1,未确认前 DTO 字段名保守
- 5 道 pre-commit gate 保持绿;missing-docs 不上升

## 非目标

- **不** emit `PidGraphicKind`(promotion 是后续 phase)
- **不**改 Phase 14–25 任何 baseline / DTO / collection
- **不**替换或删除现有 `decode_sub_records_0x0010` / `582` baseline
- **不**跨 fixture 坐标对齐 / page transform
- **不**接入新 fixture(现有 5 fixture 闭环)
- **不** commit/push(完成时再问)

## Ask Before(要先问)

- 多字符串记录的 tail 完整结构(aux 8 字节语义 + 多 string 布局)在 IDA
  `.i64` 确认前,是否先只 decode **单字符串**记录(保守)、多 string 留 raw audit
- DTO 字段命名(`marker` vs `type_marker`、`aux` 是否拆分)
- typed decoder 与现有 raw decoder 的关系(并存确认)
- 未来 promotion 方向(attribute → `PidGraphicKind::Text` / 新 `Attribute` variant)
- commit / push

## Done Means(完成判据)

1. **AC1**:`decode_attribute_fragments` + `decode_attribute_fragment_at`
   + `SheetAttributeFragmentDecoded` DTO + 公共常量,解析
   `marker(4)+aux(8)+[u16 len+UTF-16LE]`
2. **AC2**:从 **Sheet 流**提取(复用 cluster pipeline),非全文件扫描
3. **AC3**:length-prefixed UTF-16LE 字符串正确提取(含 CJK),非法/截断 → 跳过
4. **AC4**:cross-fixture ratchet —— 每 fixture 至少 N 个可提取字符串
   (N 由 Slice A 实际数据定)
5. **AC5**:panic-safe(`tests/parser_panic_safety.rs` adversarial matrix)
6. **AC6**:analysis doc 更新(结构 + 提取统计 + 与 raw decoder 关系)
7. **AC7**:Phase 14–25 baseline 全绿(尤其 `decoded_sub_records_0x0010=582` 不变)
8. **AC8**:5 道 pre-commit gate 全绿
9. **AC9**:progress.jsonl 完整 evidence trail(每 Slice append)

## Closure 子集(partial 路径)

若 Slice B 在无 `.i64` 下无法确认多字符串 tail 结构:

- 只 decode **单字符串记录**(保守子集),多 string 记录保持现有 raw audit
- AC1–AC9 仍满足(N 取保守值)
- analysis doc 明确记录"多 string tail 待 IDA 确认"
- progress.jsonl append `partial_complete`(不 append `goal_complete`)
