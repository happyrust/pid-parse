# Plan: Phase 26 PSM 0x0010 Attribute Fragment Decoder

## 1. Solution Overview

```
[Sheet stream bytes]
       ↓ scan type&0x3FFF==0x10 records (reuse existing extraction)
[raw 0x0010 record] ── (existing) ──> decode_sub_records_0x0010 → 582 audit (UNCHANGED)
       │
       └ (NEW) ──> decode_attribute_fragments
                       ↓ marker(4) + aux(8) + [u16 len + UTF-16LE]*
                   [SheetAttributeFragmentDecoded]
                       ↓
                   model::DecodedAttributeFragment (audit-only)
                       ↓
                   SheetGeometry::decoded_attribute_fragments
                       ↓
                   cross-fixture ratchet + analysis doc
```

**纯 additive typed-audit 层**:与现有 raw `decode_sub_records_0x0010`
**并存**,不替换、不改 `582` baseline,不 emit `PidGraphicKind`。

## 2. Why This Approach (additive typed-audit, not promotion)

| 候选 | 优点 | 缺点 | 决策 |
|---|---|---|---|
| **A. additive typed-audit decoder(推荐)** | 落地突破 / 保留 raw baseline / 不碰 PidGraphicKind / source-proven 安全 | 多一个并行集合 | **本 phase 采用** |
| B. 直接 promotion 为 PidGraphicKind::Text/Attribute | 下游可直接消费 | 违反 source-proven gate(tail 未字节级确认)+ 改 entity 层风险大 | 拒绝(留后续 phase) |
| C. 替换现有 raw decoder | 单一集合 | 破坏 582 baseline + 丢失 raw 审计 | 拒绝 |
| D. 继续只做文档不写代码 | 零风险 | 突破不落地,无回归保护 | 拒绝 |

## 3. How It Will Work

### 3.1 新 DTO(src/parsers/sheet_records.rs,紧邻 SheetSubRecord0x0010Decoded)

```rust
pub struct DecodedAttributeString {
    /// Offset of the u16 length word inside the payload.
    pub len_offset: usize,
    /// Character count from the u16 length prefix.
    pub char_count: u16,
    /// Decoded UTF-16LE text (lossy-free; rejected if not clean UTF-16LE).
    pub text: String,
}

pub struct SheetAttributeFragmentDecoded {
    pub byte_range: std::ops::Range<usize>,
    pub type_code: u16,        // always 0x0010
    pub marker: u32,           // payload[0..4] (e.g. 0x00010002) — type marker, NOT a unique oid
    pub aux: [u8; 8],          // payload[4..12] — semantics pending IDA (Slice B)
    pub strings: Vec<DecodedAttributeString>,
}
```

### 3.2 新 API

```rust
pub fn decode_attribute_fragments(data: &[u8]) -> Vec<SheetAttributeFragmentDecoded>;
pub fn decode_attribute_fragment_at(data: &[u8], offset: usize)
    -> Option<SheetAttributeFragmentDecoded>;
```

### 3.3 解码逻辑(保守)

1. header:`type&0x3FFF==0x10` + `bytes_to_follow ∈ [SUB_RECORD_0X0010_MIN, MAX]`
   (复用现有常量)
2. `marker = payload[0..4] as u32 LE`;`aux = payload[4..12]`
3. 从 `payload[12]` 起循环读 `[u16 len][len*2 bytes UTF-16LE]`:
   - `len == 0` 或 `12 + 2 + len*2 > payload.len()` → 停止该记录的 string 循环
   - UTF-16LE 解码:用 `char::decode_utf16`;遇 unpaired surrogate → 该 string
     判为非法,**整条记录**回退(不进 typed 集合,留给 raw decoder)
   - 接受条件:至少 1 个 `char_count >= 1` 的干净字符串
4. **Slice B gate**:多字符串 tail(读完第 1 个 string 后是否还有 `[len][utf16]`)
   的推进规则需 IDA `.i64` 确认。未确认前**保守**:只读第 1 个 string,
   `strings.len() == 1`;其余 tail 字节不解析(留 audit)。

### 3.4 与 raw decoder 关系

- `decode_sub_records_0x0010`(582)**保持不变**
- `decode_attribute_fragments` 是**新增并行集合**,只收能干净解出 string 的子集
- `SheetGeometry` 同时持有 `decoded_sub_records_0x0010`(raw, 582)和
  `decoded_attribute_fragments`(typed, 新 ratchet)

## 4. Slices

| Slice | Purpose | Files | Done when | Risks |
|---|---|---|---|---|
| A | Sheet-stream(非全文件)probe:提取 0x0010,按结构解 string,统计跨 fixture 可提取数 + 单/多 string 比例 | `examples/probe_attribute_fragment.rs`(新) | dump 每 fixture string 数 + 样本 + 单/多 string 直方图 | 单/多 string 比例决定 Slice B gate |
| B | 多字符串 tail 字节级确认 | `dlls/radsrvitem.dll.i64`(IDA) + doc | aux 语义 + 多 string 推进规则确认,或判定保守子集 | 需用户开 IDA;否则走保守 partial |
| C | 新 DTO + decode API + 8–12 unit tests | `src/parsers/sheet_records.rs` | DTO + API + tests(canonical/单string/多string/CJK/截断/非法 UTF16/panic) | UTF-16 decode 边界 |
| D | model + schema 接入 | `src/model.rs` + `src/schema.rs` | `DecodedAttributeFragment` + `SheetGeometry::decoded_attribute_fragments` + schema ratchet | schema backward-compat(`#[serde(default)]`) |
| E | pipeline + cross-fixture ratchet | `src/streams/cluster.rs` + `src/cfb/reader.rs` + `src/geometry.rs` + `tests/parse_real_files.rs` | 每 fixture string 数 ratchet 锁定;`582` 不变 | fixture init 同步 |
| F | panic-safety matrix | `tests/parser_panic_safety.rs` | 新 decoder 入 adversarial matrix | —— |
| G | analysis doc 更新 | `docs/analysis/2026-05-31-...md` 或新 doc | 结构 + 提取统计 + 与 raw 关系 + samples | doc 长度 |
| H | 5 道 gate + Phase 14–25 回归 | tests + `.github/scripts/` | 全绿;`582` + 所有 baseline 不变 | missing-docs 不升 |

## 5. Estimated Workload

- Slice A: 0.5 session(probe + 统计)
- Slice B: 0.5–1 session(IDA 确认;无 IDA 走保守 partial,0)
- Slice C–F: 1 session(DTO + API + pipeline + ratchet + panic)
- Slice G–H: 0.5 session(doc + gates)

**总计**:1.5–2 session(走保守 partial 路径约 1.5)。

## 6. Decision Points

| Slice | Decision | 触发 |
|---|---|---|
| A → B | 单 string 占比 | 若 ≥ 多数记录是单 string,Slice B 可走保守子集先落地 |
| B | full / conservative | IDA 确认多 string tail → full;无 IDA → conservative(只单 string) |
| E → G | full goal / partial | conservative 子集落地 → partial_complete;full tail → goal_complete |

## 7. Risk Register

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| 多 string tail 结构未确认就 finalize 字段 | Medium | High | Slice B gate;无 IDA 走保守单 string 子集 |
| 误把非属性 0x0010 解成 string(假阳) | Medium | Medium | 严格 UTF-16LE 干净校验;非法整条回退 raw |
| 破坏 `582` raw baseline | Low | High | 新 decoder 并存,raw decoder 一行不改;Slice H 显式断言 582 |
| schema 新字段破坏 backward-compat | Low | Medium | `#[serde(default)]` |
| CJK / surrogate 边界 panic | Low | High | `char::decode_utf16` + 全量 panic-safety matrix |
| missing-docs 上升 | Low | Medium | 所有 pub item 写 `///` |
