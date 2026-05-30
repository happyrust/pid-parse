# Blockers: Phase 26 PSM 0x0010 Attribute Fragment Decoder

## Open Questions

### Q1 — 多字符串记录的 tail 推进规则 [OPEN, gates Slice B]

突破已证单字符串记录(如 `ODOIL020150 MM` btf=42=4+8+2+28)。但更长记录
(btf=69/72/93)在第 1 个 string 后还有字节。两种可能:

- 连续 `[u16 len][UTF-16LE]` 数组(多属性)
- 第 1 个 string 后是别的字段(数值/引用),非第 2 个 string

**决定**:`dlls/radsrvitem.dll.i64` 在 IDA 打开后,`analyze_function` 追
0x0010 的 Read 路径确认 tail 推进规则。**无 IDA 时走保守路径**:只解第 1 个
string(`strings.len()==1`),多 string tail 留 raw audit,标 partial_complete。

### Q2 — aux 8 字节(payload[4..12])语义 [OPEN]

探针见 aux 形如 `03 00 01 00 44 00 00 00` / `0b 00 01 00 49 00 00 00`,
疑似 `[seq u16][0x0001 marker][len/offset u32]`。**决定**:本 phase **不**给
aux 字段命名,DTO 用 `aux: [u8; 8]` 原样保留,语义待 Slice B IDA 确认。

### Q3 — marker(payload[0..4])命名 [OPEN]

`0x00010002` 在多记录重复 → 是 type marker 不是 oid。**决定**:DTO 字段名
`marker: u32`(中性),doc 注明"非唯一 oid"。不叫 `oid`(会误导)。

### Q4 — typed decoder 与 raw decoder 关系 [PRESET]

**决定**:并存。`decode_sub_records_0x0010`(582)一行不改;
`decode_attribute_fragments` 新增。`SheetGeometry` 同时持有两个集合。

### Q5 — 字符串接受/拒绝阈值 [OPEN]

UTF-16LE 解码遇 unpaired surrogate / 控制字符如何处理?**决定**:
用 `char::decode_utf16`;任一 code unit 非法 → 该 string 非法 → **整条记录**
回退(不进 typed 集合)。只接受能干净解出 >=1 个 `char_count>=1` 字符串的记录。

## Stop And Ask

任一成立立即停手,写 `progress.jsonl`,等用户:

1. Phase 14–25 任一 baseline 退化(尤其 `decoded_sub_records_0x0010 != 582`)
2. 出现要 emit `PidGraphicKind`(Text/Annotation/Attribute)的诱因 —— 属于
   后续 promotion phase,本 phase 不做
3. 出现要替换/删除现有 raw `decode_sub_records_0x0010` 的诱因
4. 出现要改 Phase 14–25 stable DTO 既有字段的诱因
5. Slice B 需要 IDA 但用户未提供 `.i64` → 走保守 partial,不强行猜 tail
6. UTF-16 decode 在 fixture 上 panic(说明边界处理有漏)
7. `missing_docs` ratchet 上升
8. cross-fixture string 提取出现大量明显假阳(非属性文本被解成 string)
   → 收紧校验,不放宽

## Dangerous Or High-Risk Actions(需用户授权)

- 修改 `vendor/oxidized-mdf/` 内容
- 修改任何 Phase 14–25 stable DTO 既有字段
- 删除 / 替换现有 `decode_sub_records_0x0010` 或其 582 baseline
- emit 任何 `PidGraphicKind`(promotion)
- 改 `PidGraphicProvenance` / `PidPageTransform` 既有语义
- commit / push

## Known Blockers

| ID | 类型 | 状态 | next action | owner |
|---|---|---|---|---|
| Q1 | tail layout | OPEN | IDA `.i64` analyze_function;无则保守单 string | agent/user |
| Q2 | aux semantics | OPEN | 保留 `aux:[u8;8]`,待 IDA | agent |
| Q3 | naming | OPEN | `marker:u32`(非 oid) | agent |
| Q4 | coexistence | PRESET | 并存,raw 不动 | agent |
| Q5 | string validation | OPEN | decode_utf16 严格,非法整条回退 | agent |

## 当前状态总表

- 证据已就绪:`docs/analysis/2026-05-31-psm-0x0010-ida-recheck-plan.md`
  结构 + 跨 5 fixture 验证
- 突破在分析层,本 phase 落地为 additive typed-audit decoder
- 主风险:多 string tail 未字节级确认 → 已预设保守 partial 路径
- `582` raw baseline 受 Slice H 显式保护
- 工作量:1.5–2 session(保守路径约 1.5)
