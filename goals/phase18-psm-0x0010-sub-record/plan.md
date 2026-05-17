# Plan: Phase 18 PSM 0x0010 sub-record family — audit-only decoder

## 1. Solution Overview

按 Phase 15 GraphicGroup audit-only decoder 模板（每个 fixture 都
是变长 payload + **6-byte PSM header**，区别于 Phase 14 IGDS typed
primitives 的 18-byte header），把 PSM `0x0010` 落地为
**conservative typed audit collection**：

```
parser-level:  SheetSubRecord0x0010Decoded {
    byte_range,          // covers header (6B) + payload
    type_code (=0x0010), // u16, low 14 bits
    type_flags,          // u16, top 2 bits
    bytes_to_follow,     // u32
    raw_payload: Vec<u8>,// length == bytes_to_follow
}
model-level:   DecodedSubRecord0x0010Record { byte_start, byte_end,
    type_code, type_flags, bytes_to_follow, raw_payload }
collection:    SheetGeometry::decoded_sub_records_0x0010
geometry.rs:   NO PidGraphicKind emission (sub-record, not standalone geometry)
```

**No `oid` field** — 0x0010 doesn't follow the IGDS-style 18-byte
header convention. Phase 14 typed primitives' header layout
(`type_word + bytes_to_follow + oid + 8B padding`) does **not**
apply here; we mirror Phase 15 GraphicGroup's 6-byte header
(`type_word + bytes_to_follow`) exactly.

设计哲学：在 IDA 反编译还没确认 0x0010 真实类身份和 sub-kind
discriminator 之前，**绝不命名 payload 字段**。只暴露 RAW bytes +
完整 provenance + ratchet-tested cross-fixture count。下游消费者可
按 size bucket 自行解读；后续 phase（如 Phase 19）按 sub-kind 升级
为 typed schema。

## 2. Why This Approach

| 候选 | 优点 | 缺点 | 决策 |
|---|---|---|---|
| **A. Audit-only**（推荐）| 低风险；零字段命名错误；mirrors Phase 15；为 future sub-kind decoder 留稳定 baseline | payload 字段不可直接消费；reference resolution 推迟到 Phase 19 | **本 phase 采用** |
| B. Sub-kind typed | 字段语义直接可用 | 638 hit 含 ≥ 12 种 size bucket，无 IDA 证据强行命名 = 重蹈 Phase 14 GArc2d 错误 | 拒绝 |
| C. Hybrid（audit + 31B/70B/76B 顶层 sub-kind 命名）| 部分字段可用 | 选 sub-kind 的标准缺失；引入 conditional schema 复杂度 | 推迟 |
| D. Reference-only（不解 0x0010 本身，仅在 JStyleOverride 暴露 referenced_oid）| 最小范围 | 638 个 record 完全没在 SheetGeometry 体现，audit gap 大 | 拒绝 |

Phase 14 GArc2d 错误命名的教训：在没 IDA 证据前强行命名字段
（axis_a / axis_ratio / sweep_direction / sweep_angle）→
**98 条真实 record 误归类 / 50 条丢失 / 字段语义全错**。Phase 15
GraphicGroup 在没 IDA 字段表的情况下仅暴露 audit raw payload →
352 条全部 ratchet 通过，零字段错误。Phase 18 必须沿用 Phase 15 模板。

## 3. How It Will Work

数据流（参考 Phase 15）：

```
raw Sheet* stream bytes
    │
    ▼  decode_sub_records_0x0010(data: &[u8])
       — 字节循环 + 6-byte PSM header（type_word(2) + bytes_to_follow(4))
       — type_code = type_word & 0x3FFF == 0x0010
       — bytes_to_follow ∈ [1, 100_000]
       — payload_end = offset + 6 + bytes_to_follow
       — payload_end ≤ data.len()
       — raw_payload = data[header_end..payload_end].to_vec()
       — byte_range = offset..payload_end
    │
    ▼ Vec<SheetSubRecord0x0010Decoded> (parser DTO)
    │
    ▼ map(DecodedSubRecord0x0010Record::from)
    │
    ▼ Vec<DecodedSubRecord0x0010Record> → SheetGeometry::decoded_sub_records_0x0010
    │
    ▼ NO emission into NormalizedPidGeometry.entities
    │
    ▼ audit consumer reads JSON: { "decoded_sub_records_0x0010":
                                    [{"byte_start":...,"byte_end":...,
                                      "type_code":16,"type_flags":0,
                                      "bytes_to_follow":...,
                                      "raw_payload":[..u8..]}, ...] }
```

设计要点：

- **panic-safety**：所有 `usize` 算术用 `checked_add`；所有切片用
  `data.get(..)` 而非直接索引；空 input 立即返回空 Vec。
- **validation 极保守**：仅 `type_code == 0x0010` + `bytes_to_follow
  ∈ [1, 100_000]` + 边界检查。不在 payload 上做任何额外 validation
  （不知 sub-kind discriminator 在哪个字节）。
- **`raw_payload` 设计**：复制 payload bytes 到独立 `Vec<u8>`（不
  借用），避免下游 lifetime 问题；JSON 序列化用 `serde` 默认
  byte array 表示（`serde_json` 序列化 `Vec<u8>` 为 number array），
  或可后续按需切换 base64。本 phase 默认 `Vec<u8>` → number array，
  与 Phase 15 GraphicGroup `raw_variable_tail` 保持一致。

## 4. Slices

| Slice | Purpose | Main files | Done when | Risks |
|---|---|---|---|---|
| A | Surface inventory: 检查现有 `decode_graphic_groups` / `DecodedGraphicGroupRecord` / `SheetGeometry::decoded_graphic_groups` 实现路径，作为本 phase 模板 | `src/parsers/sheet_records.rs`, `src/model.rs`, `src/streams/cluster.rs`, `tests/parse_real_files.rs` | inventory 写入 progress.jsonl，确认 5 个 mirror 点 | 模板可能已演化 |
| B | Parser DTO + 公共常量 + decoder + decoder_at + 单测 | `src/parsers/sheet_records.rs` | parser unit tests 全绿（canonical + each rejection + panic-safety） | size validation 太严格漏掉真实 record |
| C | Model DTO + From impl + SheetGeometry 字段 + schema ratchet | `src/model.rs`, `src/schema.rs` | model layer 编译，schema test 加 needles 通过 | schema needles 漏覆盖新 DTO |
| D | Cluster pipeline 接入 + cfb/reader.rs fixture 同步 | `src/streams/cluster.rs`, `src/cfb/reader.rs` | build --workspace --all-targets 绿 | empty-sheet branch 漏 update |
| E | Cross-fixture ratchet test | `tests/parse_real_files.rs` | 跨 fixture 总计 = 638 (DWG-0201=183, DWG-0202=133, 工艺管道-1=315, A01=7) | 跨平台 fixture 缺失导致 soft-skip 行为不一致 |
| F | Panic-safety matrix 加 entry | `tests/parser_panic_safety.rs` | matrix 含 `decode_sub_records_0x0010` + `decode_sub_record_0x0010_at` | 漏掉 boundary case |
| G | CHANGELOG Phase 18 入口 + 必要 AGENTS.md 注释 | `CHANGELOG.md`, optional `AGENTS.md` | changelog 写明 638 baseline + audit-only 选择 | downstream consumer 期望 typed schema |
| H | 5 道 pre-commit gate + progress.jsonl `goal_complete` | progress.jsonl | gates 全绿 + AC1-AC11 evidence | clippy 新警告 / missing-docs 上升 |

## 5. Sequencing

A → B → C → D → E → F → G → H 顺序执行。B 是核心实现（parser 决定
后续所有 layer 的字段形态）；E 之前不更新 changelog，避免数字未敲定
就先写。F-G 可并行（panic-safety 和 docs 互不依赖）。

## 6. Phase Boundaries

最低可交付：
- AC1-AC11 全部达成
- 638 ratchet test 绿
- 5 道 gate 绿
- Phase 14/15/16/17 baseline 保持

不在本 phase 做：
- sub-kind discriminator 反向（推迟 Phase 19 / future）
- JStyleOverride/GraphicGroup → 0x0010 reference resolver
- PidGraphicKind 新 variant
- IDA 反编译（用户授权后才考虑加载 IDA instance）

## 7. Steering Notes

- 整体节奏参考 Phase 15 GraphicGroup（一周时间，audit-only 落地）。
- 不要顺手解决 Phase 14 §6.1 / Phase 16 deferred field 之类的话题，
  本 phase 专注 0x0010 audit collection。
- 若 probe size bucket 在新 fixture 上出现 < 1 或 > 100_000 的 payload，
  立即写 blocker 暂停（说明 validation 阈值需重新校准）。

## 8. Acceptance Criteria

- [ ] **AC1**：`decode_sub_records_0x0010` 跨 fixture 输出 **582 条**
      （DWG-0201=161, DWG-0202=104, 工艺管道-1=306, A01=11）。
      probe（non-advancing scan，含 overlap）输出 638；本 decoder
      采用 advancing scan（Phase 15 GraphicGroup 模板），所以
      数量低于 probe，但更具有 "non-overlapping record" 语义。
- [ ] **AC2**：`SheetSubRecord0x0010Decoded` parser DTO 暴露
      `byte_range / type_code / type_flags / bytes_to_follow /
      raw_payload`，**无 `oid` 字段**（6-byte header 形态），不命名
      sub-kind 字段。
- [ ] **AC3**：`DecodedSubRecord0x0010Record` stable model DTO mirror
      parser DTO（`byte_start / byte_end / type_code / type_flags /
      bytes_to_follow / raw_payload`），含 `From` impl 和 `JsonSchema`
      derive。
- [ ] **AC4**：`SheetGeometry::decoded_sub_records_0x0010: Vec<
      DecodedSubRecord0x0010Record>` 字段加入 sheet pipeline。
- [ ] **AC5**：`src/schema.rs` 默认 schema 含
      `DecodedSubRecord0x0010Record` + `decoded_sub_records_0x0010`
      needles（ratchet test 验证）。
- [ ] **AC6**：cross-fixture ratchet test
      `sub_records_0x0010_decoder_emits_audit_records_with_provenance`
      跨 4 fixture 总计 **582 条**；每条带 sheet path + byte range +
      type_code==0x0010 + non-empty raw_payload + raw_payload.len()
      == bytes_to_follow。
- [ ] **AC7**：`tests/parser_panic_safety.rs` adversarial matrix 含
      `decode_sub_records_0x0010` + `decode_sub_record_0x0010_at`，
      empty / truncated / 全 0 / 全 0xFF 输入不 panic。
- [ ] **AC8**：normalized geometry 不变（`NormalizedPidGeometry.entities`
      数量不因 Phase 18 而增长；新 collection 仅作为 audit JSON 字段）。
- [ ] **AC9**：Phase 14/15/16/17 baseline 不退化（cross-fixture decoded
      counts + 5 道 gate）。
- [ ] **AC10**：5 道 pre-commit gate 全绿：build / test / clippy -D
      warnings / fmt --check / missing-docs ratchet（baseline=0 不上升）。
- [ ] **AC11**：`progress.jsonl` 对 AC1-AC10 都有命令 / artifact /
      输出摘要 + `goal_complete` 总结。

## 9. Required Evidence

| Requirement | Evidence to inspect | Where recorded |
|---|---|---|
| AC1 | `cargo test --test parse_real_files sub_records_0x0010_decoder_emits_audit_records_with_provenance -- --nocapture` 输出 = 161+104+306+11=582 | `progress.jsonl` |
| AC2 | `src/parsers/sheet_records.rs` diff: 新增 `SheetSubRecord0x0010Decoded` + `decode_sub_records_0x0010` + `decode_sub_record_0x0010_at` | `progress.jsonl` |
| AC3 | `src/model.rs` diff: 新增 `DecodedSubRecord0x0010Record` + `From` impl | `progress.jsonl` |
| AC4 | `src/streams/cluster.rs` diff: 新 collection populated | `progress.jsonl` |
| AC5 | `cargo test --lib schema` 全绿 | `progress.jsonl` |
| AC6 | `cargo test --test parse_real_files sub_records_0x0010_decoder_emits_audit_records_with_provenance -- --nocapture` 跨 fixture 总数 = 582 | `progress.jsonl` |
| AC7 | `cargo test --locked -j 1 --test parser_panic_safety` 绿 | `progress.jsonl` |
| AC8 | `cargo test --test parse_real_files normalized_geometry_probe_baseline_on_real_fixture -- --nocapture` 输出 entity 总数不变 | `progress.jsonl` |
| AC9 | targeted Phase 14/15/16/17 cross-fixture tests 全绿 | `progress.jsonl` |
| AC10 | 5 道 pre-commit gate 输出 | `progress.jsonl` |
| AC11 | `progress.jsonl` 本身 | `progress.jsonl` |

## 10. Completion Audit

声明完成前逐项对照 AC1-AC11。任何 sub-kind 命名（如 `coord_a_x`、
`referenced_type`、`tag_block`）出现在 `SheetSubRecord0x0010Decoded`
或 `DecodedSubRecord0x0010Record` 字段名中都必须**回退**——
本 phase 严格 audit-only。命名 sub-kind 字段属于 Phase 19 /
future 工作。
