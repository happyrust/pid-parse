# 开发计划：Phase 11c — Sheet 深层几何/图元解码

> 起稿：2026-04-21
> 目标版本：v0.12.0（minor bump；可能多 ship 多轮 v0.12.x）
> 前置：Phase 11a + 11b ship（v0.10.0 + v0.11.0，提供 cluster_id / segment_id 外键）
> 估计工时：10-14 hr（需分 3-4 个 sub-Phase）
> 所属 roadmap：`docs/sppid/2026-04-21-sppid-full-parse-roadmap.md` **Phase 2.5**
> **特别说明**：本 Phase 逆向不确定性最高，强烈建议单独 session 设计，本 plan 仅定框架

## 动机

Sheet 流当前解析深度：

- **已解**（Phase 4 + Phase 6）：cluster header（magic `0x6C90F544` 共用）+ 0x89 marker probe + 端点对记录（`endpoint_records`）
- **未解**：type=0x00CE body 里的其他 records — 页面图元、坐标、标签文本、符号引用、标注、线型、颜色

Sheet 是 SmartPlant P&ID 的**视觉层真值**：所有"图纸上画了什么"的信息都在这里。当前 `PidLayoutModel` 把几何布局拼出来靠的是 dynamic attrs + cross-reference 推断，不是 Sheet 内字节 — **语义强度不够**。

`inspect --coverage` 状态：

```
[ID]  Sheet* (storage prefix)
      parser=sheet_probe / field=sheets
      note=storage identified, page geometry/symbols/labels not decoded
```

**Phase 11c 的价值边界**：不求 SmartPlant 原始像素级复刻，**只求能把"这张图上有哪些图元、各在什么坐标、挂哪个符号、写了什么标签"这四个问题的答案从推断升级为字节证据**。

## 非目标

- **不做** SmartPlant 原始线型 / 标注风格 / 字高 / 版式的像素级复刻（roadmap 明确说过）
- **不做** Sheet 写入（writer 层 Sheet 仍走 `sheet_patches` byte-range splice，不引入声明式图元增删）
- **不做** `.sym` 文件本身的几何解码（那是独立的外部资源；只管 Sheet 引用 symbol_path 这一步）
- **不在** Phase 11c 推进规范化语义图层（那是 Phase 12a）
- **不接** 多 sheet 之间的跨引用（先解单 sheet，OPC 跨图是后续）

## 逆向策略

### 已知锚点

| 锚点 | 来源 | 应用 |
|---|---|---|
| cluster header magic `0x6C90F544` | Phase 4 | 所有 Sheet 流共用 header schema |
| type = 0x00CE | Phase 4 实测 | Sheet body 记录类型 |
| `records = 354` | Phase 4 header field | 预期记录总数，用于遍历结束 guard |
| `body_len = 121`（**记录**！而非 bytes？）| 需重新确认 | 如果是 byte 数，354 × 121 = 42834 远小于 stream 29594，需重新解释 |
| 0x89 marker count = 0（Sheet）| Phase 4 实测 | Sheet 不使用 DA record format |
| endpoint_records 每条 24B | Phase 6 实测 | 已稳定的子记录类型 |
| rel_field_x vs PSMsegment.segment_id | Phase 11b 对账 | segment 反向索引 |

### 逆向方法：频次驱动（按 roadmap 建议）

**不**逐字节深挖；先做**统计**。

**Step 1（W1）：records-by-type 统计**

扫 Sheet body 字节，按 2-byte / 4-byte / 8-byte 重复 pattern 分类，产出频次表：

```
Sheet6 body analysis:
  Total bytes: 29594
  Declared records: 354

Top record-start patterns (u16 at aligned offsets):
  0x0001 (tag=?): 112 occurrences
  0x0002 (tag=?): 87 occurrences
  0x0010 (tag=?): 64 occurrences
  0x0020 (tag=?): 45 occurrences
  0x00CE (tag=type marker): 354 occurrences  ← 这是 type 不是 record start
  0x0089 (tag=DA marker): 0 occurrences     ← 已验证
  ...
```

**Step 2（W2）：按频次排序，高频 record type 优先解码**

- 预期 112 次的 0x0001（假设）→ 可能是 "object placement"（每个图元一条）
- 预期 87 次的 0x0002 → 可能是 "label"
- 64 次的 0x0010 → 可能是 "connection / line"
- 低频 < 10% 的 record type 留 `audit`，不强解

**Step 3（W3-W6）：逐 record type 做 hex walk + 字段命名**

每类 record 解码按优先级：
1. 起始 tag / size
2. 坐标字段（浮点或固定点）
3. 文本字段（UTF-16LE runs）
4. 交叉引用字段（cluster_id / segment_id）

## 范围

| 文件 | 改动 | 行数（估）|
|---|---|---|
| `Cargo.toml` | version 0.11.0 → 0.12.0 | ±1 |
| `src/parsers/sheet_probe.rs` | 改名 `sheet_records.rs`；新增 record type 解析骨架 | +300 |
| `src/parsers/sheet_record_types/` **新子模块** | 每 record type 一文件（object / label / line / reference）| +500 |
| `src/model.rs` | `SheetRecord` / `SheetObjectRecord` / `SheetLabelRecord` / `SheetLineRecord` / `SheetReferenceRecord` | +200 |
| `src/streams/sheet.rs` 或扩展 `sheet_probe.rs` | 挂到 doc | +40 |
| `src/crossref.rs` | Sheet record ↔ cluster / segment / layout 的三向对账扩展 | +80 |
| `src/layout.rs` | 消费 SheetRecord 产生 `PidLayoutItem`（替代/补充现有 inference）| +150 |
| `src/inspect/report.rs` | Sheet 段展示 record-type 频次 + 解码覆盖率 | +100 |
| `src/inspect/coverage.rs` | Sheet* 升级到 PartiallyDecoded | +30 |
| `tests/parse_real_files.rs` | 真实 fixture 断言 | +150 |
| `tests/unit_parsers.rs` | 合成 fixture | +200 |
| `examples/probe_sheet_record_types.rs` | 频次统计探针 | +150 |
| `CHANGELOG.md` | `[0.12.0]` 段 | +150 |
| **本 plan** | | +本文件 |

~2150 行，核心 1000 行。

## 具体设计

### 1. record-type 顶层模型

```rust
// src/model.rs
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind")]
pub enum SheetRecord {
    Object(SheetObjectRecord),
    Label(SheetLabelRecord),
    Line(SheetLineRecord),
    Reference(SheetReferenceRecord),
    EndpointPair(SheetEndpointRecord),  // 已有，合并进统一 enum
    Unknown {
        tag: u16,
        size: u16,
        raw_bytes: Vec<u8>,  // 保留，for audit / round-trip
    },
}

pub struct SheetObjectRecord {
    pub record_id: u32,
    pub position: (f32, f32),  // x, y 坐标
    pub size: Option<(f32, f32)>,
    pub rotation: Option<f32>,
    pub symbol_ref: Option<u32>,  // 指向 JSite index 或 cluster_id
    pub label_refs: Vec<u32>,     // 指向同 sheet 的 Label records
    pub extra: BTreeMap<String, String>,  // 未命名字段
    pub confidence: Confidence,
}

pub struct SheetLabelRecord {
    pub record_id: u32,
    pub text: String,
    pub position: (f32, f32),
    pub font_hint: Option<String>,  // 字体名或 font_id
    pub owner_record: Option<u32>,  // 关联的 object record id
    pub confidence: Confidence,
}

pub struct SheetLineRecord {
    pub record_id: u32,
    pub start: (f32, f32),
    pub end: (f32, f32),
    pub style: Option<String>,
    pub connects: Option<(u32, u32)>,  // (from_record_id, to_record_id)
    pub confidence: Confidence,
}

pub struct SheetReferenceRecord {
    pub record_id: u32,
    pub target_cluster_id: Option<u32>,
    pub target_drawing_id: Option<String>,
    pub confidence: Confidence,
}

pub struct SheetStream {
    // 已有字段保留
    // ...
    #[serde(default)]
    pub records: Vec<SheetRecord>,  // 新 — 聚合视图
    #[serde(default)]
    pub record_type_counts: BTreeMap<u16, u32>,  // Phase 11c-probe 频次表
    #[serde(default)]
    pub decoded_bytes: u64,  // 已消费字节数，用于 coverage
}
```

### 2. record type 解码骨架

```rust
// src/parsers/sheet_record_types/mod.rs
pub fn parse_sheet_body(bytes: &[u8]) -> SheetBodyParsed {
    let mut cursor = 0;
    let mut records = Vec::new();
    let mut type_counts: BTreeMap<u16, u32> = BTreeMap::new();

    while cursor < bytes.len() {
        let tag = read_u16_le(&bytes[cursor..])?;
        *type_counts.entry(tag).or_insert(0) += 1;

        match tag {
            0x0001 => {
                let (rec, consumed) = object::parse(&bytes[cursor..])?;
                records.push(SheetRecord::Object(rec));
                cursor += consumed;
            }
            0x0002 => { /* label */ }
            0x0010 => { /* line */ }
            // ...
            _ => {
                // Unknown record：保留 raw bytes，推进一条 size
                let size = peek_size(&bytes[cursor..]);
                records.push(SheetRecord::Unknown { tag, size, raw_bytes: bytes[cursor..cursor+size].to_vec() });
                cursor += size as usize;
            }
        }
    }

    SheetBodyParsed { records, type_counts, decoded_bytes: cursor as u64 }
}
```

### 3. coverage 升级逻辑

Sheet* 当前 IdentifiedOnly（storage prefix，无法细化）。Phase 11c 后：

- 若 `sheet.decoded_bytes / sheet.total_bytes >= 0.5` 且 `records.len() >= 0.7 * declared_records` → **PartiallyDecoded**
- 若 >= 0.8 且所有 record type 都有 decoder（Unknown ratio < 5%） → **FullyDecoded**
- 否则 IdentifiedOnly

### 4. layout 层融合

Phase 11c 产出 Sheet records 之后，`layout.rs` 的 `derive_layout` 流程改为：

```
1. 优先使用 SheetObjectRecord / SheetLineRecord 作为 layout 的字节证据
2. 不可用时回退到 Phase 8c 的 dynamic-attrs + cross-reference 推断
3. Provenance 字段标明来源层级（Decoded from Sheet / Inferred from DA）
```

## 实施步骤（4 个 sub-Phase）

### Phase 11c-1：record-type 频次探针（2-3 hr）

**W1-1** `examples/probe_sheet_record_types.rs` — 统计每种 tag / size 组合的频次，输出 markdown 表  
**W1-2** 人工分析频次表，优先级排序（最高频 3 种先解）  
**W1-3** ship `v0.11.1`（patch）：只加探针 + 新增 `SheetStream.record_type_counts` 字段，不改 record 语义

### Phase 11c-2：object / label 两种高频 record（3-4 hr）

**W2-1** `sheet_record_types/object.rs` — hex walk object record；先解坐标 + id，symbol_ref 留 best-effort  
**W2-2** `sheet_record_types/label.rs` — hex walk label record；解 UTF-16LE text + 位置  
**W2-3** 合成 + 真实 fixture 双线测试  
**W2-4** ship `v0.12.0`：Sheet coverage IdentifiedOnly → PartiallyDecoded

### Phase 11c-3：line + reference（2-3 hr）

**W3-1** `sheet_record_types/line.rs` — 解 start/end 坐标 + connects 外键  
**W3-2** `sheet_record_types/reference.rs` — 跨 cluster reference  
**W3-3** CrossReference 扩展：Sheet record ↔ cluster ↔ segment 三向对账  
**W3-4** ship `v0.12.1`

### Phase 11c-4：layout 层融合（2-3 hr）

**W4-1** `layout.rs` 重构，优先消费 SheetRecord  
**W4-2** Provenance 字段：`PidLayoutItem.provenance_layer`  
**W4-3** 真实 fixture：layout vs Sheet records 对齐验证  
**W4-4** ship `v0.12.2`

## 预计工时

- Phase 11c-1: 2-3 hr
- Phase 11c-2: 3-4 hr
- Phase 11c-3: 2-3 hr
- Phase 11c-4: 2-3 hr
- docs + CHANGELOG 分散到各 sub-phase: 1 hr
- **合计 10-14 hr**（分 3-4 轮 ship）

## 验证清单

### Phase 11c-1

- [ ] `pid_inspect --probe-sheet-chunks` 输出 record_type_counts
- [ ] 频次表至少识别 5 种 record type

### Phase 11c-2

- [ ] object records 的坐标字段稳定（真实 fixture 5+ 个对象）
- [ ] label text 解为 UTF-16LE 字符串
- [ ] Sheet coverage 升级到 PartiallyDecoded
- [ ] decoded_bytes / total_bytes >= 0.4

### Phase 11c-3

- [ ] line records 的 start/end 坐标正确
- [ ] reference records 指向正确 cluster
- [ ] CrossReference 三向对账 CONSISTENT

### Phase 11c-4

- [ ] layout.items 的 anchor 字段优先来自 SheetRecord（而非 inference）
- [ ] 退化 fixture（缺 SheetRecord 时）仍能 inference fallback

## 风险缓解

| 风险 | 缓解 |
|---|---|
| SmartPlant Sheet 格式封闭 / 版本差异大 | 频次驱动先盘点；高频 record 优先；低频留 audit；多 fixture 是硬约束 |
| record size 字段位置不稳定 | Unknown record 用 `peek_size` 保守读；size 错会导致整段 misaligned，可在 W1-1 验证 |
| 坐标字段是 f32 / f64 / fixed-point 未知 | 尝试 f32 LE / f64 LE / i32 scaled，选出 range reasonable 的；单测覆盖 |
| 一个 sheet 解错影响全文件 | parse_sheet_body 遇到 Unknown 不 fail，只降 confidence |
| Phase 11c-2/3/4 之间 Sheet 行为变化 | 每 sub-phase 独立 ship + tag；可回退到上一 sub-phase |
| 单 fixture 过拟合（本项目硬约束）| 本 Phase 是"多 fixture 硬约束"的主战场；若只有 1 fixture 则明确 confidence = Heuristic；FullyDecoded 延后到多 fixture |

## SemVer 判定

- Phase 11c-1：新增 probe 字段，additive → patch bump 0.11.x
- Phase 11c-2：SheetRecord 枚举新 variants + 解码能力 → minor bump 0.11 → 0.12.0
- Phase 11c-3/4：后续 variants 扩展 → patch bump（除非 API 面变）

## Next 候选（11c 完成后）

- **Phase 12a**：规范化语义图层（roadmap Phase 3）— Sheet / cluster / segment / JSite 统一 provenance
- **Phase 12b**：consumed/leftover 字节验证框架（roadmap Phase 4）— Sheet decoded_bytes 正式入 byte-audit

## 交叉引用

- 上游总 roadmap：`docs/plans/2026-04-21-next-steps-roadmap-v0.7.1-onward.md` 阶段 C
- SPPID 战略：`docs/sppid/2026-04-21-sppid-full-parse-roadmap.md` **Phase 2.5**
- 前置 Phase：Phase 4（Sheet probe）+ Phase 6（Sheet endpoint records）+ Phase 11a + 11b
- 后续 Phase：12a / 12b
- 风险：**单独 session 设计**建议
