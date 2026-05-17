# Plan: Phase 19 PSM 0x0010 leading-word audit (sub-kind discriminator, partial)

## 1. Solution Overview

在 Phase 18 audit-only DTO 上**只加一个 `leading_word: Option<u16>`
字段**，提取自 `payload[0..2]` 小端 u16；其它字段不动；下游消费者
和 ratchet test 沿用 Phase 18 编排。

```
parser-level:  SheetSubRecord0x0010Decoded {
    // Phase 18 (unchanged):
    byte_range, type_code, type_flags, bytes_to_follow, raw_payload,
    // Phase 19 (NEW):
    leading_word: Option<u16>,  // payload[0..2] as u16 LE; None if payload.len() < 2
}

model-level:   DecodedSubRecord0x0010Record { ...phase18 fields..., leading_word }
collection:    SheetGeometry::decoded_sub_records_0x0010 (unchanged shape; new field per element)
geometry.rs:   NO emission change (still no PidGraphicKind)
schema:        decoded_sub_records_0x0010[].leading_word needle ratcheted
ratchet test:  cross-fixture leading_word == 0x0002 count = 164, == 0x0003 = 21, == 0x0001 = 18, == None = 0
```

## 2. Why This Approach（only this approach）

| 候选 | 优点 | 缺点 | 决策 |
|---|---|---|---|
| **A. leading_word audit only**（推荐）| 极低风险；不命名语义；mirrors Phase 18；为 Phase 20+ sub-kind work 留 anchor | 仅暴露 +0..+1，对 size 31/70 bucket 几乎无帮助 | **本 phase 采用** |
| B. leading_dword (u32 @ +0..+3) | 暴露面更大，可能 catch 32-bit discriminator | size 31 bucket top dword 也 ≤ 1%，证据不支持 | 拒绝 |
| C. 按 size bucket 提供不同 discriminator 偏移 | 数据驱动，能 catch size 31 | 强行为每个 bucket 定制 → schema 暴炸；本 phase 是 audit 不是反向 | 拒绝 |
| D. 直接给 0x0002 records 一个 typed sub-record DTO（`SubRecord0x0010Kind02`）| 立即可用 | 命名 sub-kind 违反 Ask-Before；164 records 占比 28% 不算稳定多数 | 拒绝（推迟 Phase 20） |
| E. probe-only（不改 DTO，只产 evidence report） | 零代码改动 | Phase 18 之后下游消费者没有任何 typed access；evidence 只在 probe 里活 | 拒绝（Phase 18 已经 audit，本 phase 应给 typed 字段，不该退回 probe） |

A 的关键原则：**新字段的名字 `leading_word` 描述字节位置而不是语义**，
这跟 Phase 15 GraphicGroup `raw_variable_tail` 一脉相承（描述 "尾部
原始变长字节" 而不是 "child_oid_list"）。

## 3. How It Will Work

数据流：

```
Phase 18 decode_sub_records_0x0010(data: &[u8])
    │ 已有：识别 type_code=0x0010 + bytes_to_follow ∈ [8, 100_000]
    │       + 边界 check + raw_payload 复制
    ▼
Phase 19 NEW（在同一函数内）：
    let leading_word = if payload.len() >= 2 {
        Some(u16::from_le_bytes([payload[0], payload[1]]))
    } else {
        None
    };
    │
    ▼ SheetSubRecord0x0010Decoded { ...phase18 fields..., leading_word }
    │
    ▼ From -> DecodedSubRecord0x0010Record { ..., leading_word }
    │
    ▼ Vec<DecodedSubRecord0x0010Record> → SheetGeometry::decoded_sub_records_0x0010
    │
    ▼ NO change in geometry.rs / cluster.rs（leading_word 是值字段）
    │
    ▼ JSON audit: { "decoded_sub_records_0x0010": [
                      { "byte_start":..., "byte_end":..., "type_code":16,
                        "type_flags":0, "bytes_to_follow":...,
                        "raw_payload":[...u8...], "leading_word":2 }, ... ] }
```

设计要点：

- **panic-safety**：`payload.get(0..2)` + pattern match，禁止直接索引。
- **零向后不兼容**：现有 JSON 消费者只是多看到一个 `leading_word` 字段；
  type / schema 仍兼容 Phase 18 schemars 默认行为。
- **`Option<u16>` 而非 `u16`**：保证 0 / 1 字节 payload 不会强行造一个
  假 word（虽然本 phase probe 没见过这种情况，但 schema 必须诚实）。
- **不动 Phase 18 ratchet**：582 数字保持不变；新 ratchet test 是
  additive。

## 4. Slices

| Slice | Purpose | Main files | Done when | Risks |
|---|---|---|---|---|
| A | Surface inventory：定位 Phase 18 5 个 mirror 点（parser DTO / model DTO / schema / ratchet test / panic-safety matrix）并确认本 phase 只需改前 3 个 | `src/parsers/sheet_records.rs`, `src/model.rs`, `src/schema.rs`, `tests/parse_real_files.rs` | inventory 写入 `progress.jsonl`，确认 cluster.rs / cfb/reader.rs / geometry.rs **不需** 改 | mirror 点漂移（自 Phase 18 起代码有改动） |
| B | Parser DTO 加 `leading_word` + decoder 填充 + 单测 | `src/parsers/sheet_records.rs` | 新 unit test `leading_word_extracted_from_payload` 红→绿；Phase 18 既有 12 个 unit test 全绿 | 改动顺手碰 Phase 18 既有字段顺序 |
| C | Model DTO `leading_word` + `From` impl 同步 + schema needle | `src/model.rs`, `src/schema.rs` | 新 schema test `default_schema_exposes_leading_word_needle` 绿；既有 8 个 schema test 全绿 | schemars 自动生成漏字段 |
| D | Cross-fixture ratchet test | `tests/parse_real_files.rs` | 新 test `sub_records_0x0010_leading_word_distribution_matches_phase19_probe` 绿（0x0002=164, 0x0003=21, 0x0001=18, None=0） | probe 数字与 decoder 数字不一致 |
| E | Phase 18 ratchet regression check | `tests/parse_real_files.rs` | 既有 `sub_records_0x0010_decoder_emits_audit_records_with_provenance` 仍输出 582 | 无 |
| F | CHANGELOG Phase 19 入口 + 必要 AGENTS.md 注释 | `CHANGELOG.md`, `AGENTS.md` | changelog 写明 178 records（0x0002 + 0x0003 + 0x0001 + 0x0004）有可识别 leading_word + audit-only 选择 + Phase 18 关系 | downstream consumer 期望 typed sub_kind |
| G | 5 道 pre-commit gate + progress.jsonl `goal_complete` | progress.jsonl | gates 全绿 + AC1-AC11 evidence | clippy 新警告 / missing-docs 上升 |

## 5. Sequencing

A → B → C → D → E → F → G 严格顺序。B 是核心改动；E 在 D 之后跑，
确认 leading_word 引入没有副作用退化 Phase 18 ratchet。

## 6. Phase Boundaries

最低可交付：
- AC1-AC11 全部达成
- 新 ratchet test 绿（0x0002=164 / 0x0003=21 / 0x0001=18 / None=0）
- Phase 18 ratchet 仍输出 582
- 5 道 gate 绿
- Phase 14/15/16/17/18 baseline 全部保持

不在本 phase 做：
- size 31/70/13/16/43 bucket 的 discriminator 反向（推迟 Phase 20+）
- 任何 sub-kind 命名（`sub_kind` / `record_kind` 等）
- JStyleOverride / GraphicGroup → 0x0010 reference resolver
- PidGraphicKind 新 variant
- IDA 加载

## 7. Steering Notes

- 整体节奏比 Phase 18 更轻：~30% 工作量（一个值字段 + 一个 ratchet
  test）。预期半天到一天。
- 不要顺手解决 size 31 bucket 的 deferred 反向；本 phase 专注 +0..+1。
- 若 probe 数字（164/21/18）在 decoder 上对不上，立即停手 → 大概率
  Phase 18 advancing-scan 与 probe 的非 advancing-scan 在重叠区有
  ±4 records 差异。先用 decoder-side rerun 校准 ratchet 数字到
  decoder ground truth，**再决定**是否更新 brief.md 中的 probe 数字。
- 若新 ratchet test 显示某些 leading_word 值出现在 size 31 bucket 内
  也呈现稳定分布（如 size=31 + leading_word=0xXYZW = 80% records），
  写一条 progress.jsonl `[discovery]` entry，但**不**自动扩 scope；
  留给 Phase 20。

## 8. Acceptance Criteria

- [ ] **AC1**：`SheetSubRecord0x0010Decoded` 新增 `leading_word:
      Option<u16>` 字段；`decode_sub_records_0x0010` /
      `decode_sub_record_0x0010_at` 在 payload 长度 ≥ 2 时填充
      `Some(u16::from_le_bytes([payload[0], payload[1]]))`，否则 `None`。
      Phase 18 字段全部保持。
- [ ] **AC2**：parser unit tests 至少覆盖 4 个 case：
      - canonical decode (`leading_word == Some(expected)`)
      - 短 payload (`leading_word == None`)
      - 既有 12 个 Phase 18 unit test 全绿
      - panic-safe on empty / truncated 输入
- [ ] **AC3**：`DecodedSubRecord0x0010Record` mirror 新字段 +
      `From<SheetSubRecord0x0010Decoded>` impl 同步；`JsonSchema` derive
      自动覆盖；既有 model unit tests 全绿。
- [ ] **AC4**：`src/schema.rs` 默认 schema 含 `leading_word` needle
      （ratchet test 验证）。
- [ ] **AC5**：cross-fixture ratchet test
      `sub_records_0x0010_leading_word_distribution_matches_phase19_probe`
      跨 4 fixture 输出：
      - `leading_word == Some(0x0002)` count = **164**
      - `leading_word == Some(0x0003)` count = **21**
      - `leading_word == Some(0x0001)` count = **18**
      - `leading_word == None` count = **0**
      （数字若与 probe 差 ±4 因为 advancing-scan vs probe 重叠 scan
      不一致，以 decoder 数字为准并在 progress.jsonl 记录差异）
- [ ] **AC6**：Phase 18 cross-fixture ratchet test
      `sub_records_0x0010_decoder_emits_audit_records_with_provenance`
      仍输出 **582**（DWG-0201=161, DWG-0202=104, 工艺管道-1=306,
      A01=11）。
- [ ] **AC7**：`tests/parser_panic_safety.rs` 既有 entry 全绿
      （本 phase 无新 public parser entry，**不**新增 matrix 行）。
- [ ] **AC8**：normalized geometry 不变（`NormalizedPidGeometry.entities`
      数量与 Phase 18 末态完全一致）。
- [ ] **AC9**：Phase 14/15/16/17/18 baseline 全部保持（5 道 gate 内
      cross-fixture decoded counts 与 Phase 18 完成态一致）。
- [ ] **AC10**：5 道 pre-commit gate 全绿：build / test --workspace
      --all-targets / clippy -D warnings / fmt --check / missing-docs
      ratchet（baseline=0 不上升）。
- [ ] **AC11**：`progress.jsonl` 对 AC1-AC10 都有命令 / artifact /
      输出摘要 + `goal_complete` 总结。

## 9. Required Evidence

| Requirement | Evidence to inspect | Where recorded |
|---|---|---|
| AC1 | `src/parsers/sheet_records.rs` diff: 新字段 + decoder 填充 | `progress.jsonl` |
| AC2 | `cargo test --locked --lib parsers::sheet_records::tests::sub_record_0x0010` 输出 | `progress.jsonl` |
| AC3 | `src/model.rs` diff: 新字段 + `From` impl | `progress.jsonl` |
| AC4 | `cargo test --locked --lib schema` 全绿 | `progress.jsonl` |
| AC5 | `cargo test --test parse_real_files sub_records_0x0010_leading_word_distribution_matches_phase19_probe -- --nocapture` 输出 | `progress.jsonl` |
| AC6 | `cargo test --test parse_real_files sub_records_0x0010_decoder_emits_audit_records_with_provenance -- --nocapture` 输出 = 582 | `progress.jsonl` |
| AC7 | `cargo test --locked -j 1 --test parser_panic_safety` 全绿 | `progress.jsonl` |
| AC8 | `cargo test --test parse_real_files normalized_geometry_probe_baseline_on_real_fixture -- --nocapture` entity 总数不变 | `progress.jsonl` |
| AC9 | targeted Phase 14/15/16/17/18 cross-fixture tests 全绿 | `progress.jsonl` |
| AC10 | 5 道 pre-commit gate 输出 | `progress.jsonl` |
| AC11 | `progress.jsonl` 本身 | `progress.jsonl` |

## 10. Completion Audit

声明完成前逐项对照 AC1-AC11。任何 sub-kind 命名（如 `sub_kind`、
`record_kind`、`family_tag`、`payload_kind`）出现在
`SheetSubRecord0x0010Decoded` 或 `DecodedSubRecord0x0010Record`
字段名中都必须**回退**——本 phase 严格 audit-only `leading_word`，
命名 sub-kind 字段属于 Phase 20 / future 工作。
