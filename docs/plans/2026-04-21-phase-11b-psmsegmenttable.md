# 开发计划：Phase 11b — PSMsegmenttable 结构化解析

> 起稿：2026-04-21
> 目标版本：v0.11.0（minor bump）
> 前置：Phase 11a (v0.10.0) — PSMclustertable per-record 结构化（提供 cluster_id 外键）
> 估计工时：4-6 hr
> 所属 roadmap：`docs/sppid/2026-04-21-sppid-full-parse-roadmap.md` **Phase 2.3**

## 动机

`PSMsegmenttable` 当前解析深度：

- **已解**：12 字节 header（magic `0x62617473` = "stab"）+ count + `flags: [u8; count]` 字节数组
- **未解**：每个 byte flag 的语义；没有 per-segment record 结构化

`inspect --coverage` 状态：

```
[PART] PSMsegmenttable (12B)
       parser=psm_tables / field=psm_segment_table
       note=flags array decoded, segment semantics unknown
```

**为什么 Phase 11b 值得做**：

1. PSMsegmenttable 是**关系层的底座**。当前 Phase 6 (v0.3.0) 逆向出的 Sheet endpoint_records 通过 `rel_field_x` 索引间接引用对象；如果 PSMsegmenttable 能结构化，就能确认 `rel_field_x == segment_id` 这条关键等式
2. 把"`flags: [u8; N]`"这种 opaque 数组升级为有 kind / owner / target 的 record，**让 layout.segments / ObjectGraph.relationships / Sheet.endpoint_records 三个视图共享同一套 provenance**
3. 是 Phase 12a 规范化语义图层的**输入前提**

## 非目标

- **不做** segment 到 layout / geometry 的实际图元映射（那是 Phase 11c Sheet 深层）
- **不引入** writer 对 PSMsegmenttable 的编辑（声明式 segment 增删是 Phase 12+）
- **不验证** 跨多 sample 的 segment flag 语义稳定性（单 fixture 约束，multi-fixture 是 Phase 11 的硬约束但 fixture 收集在 Phase 11a W1 一起推进）

## 逆向策略

### 已知锚点（来自 Phase 11a 完成后）

1. `PSMclustertable.records_decoded[i].declared_segment_count` 提供每个 cluster 应有的 segment 数
2. `sum(declared_segment_count) == PSMsegmenttable.flags.len() == N`（已在 11a 验证）
3. Sheet endpoint_records 每条有 `rel_field_x: u32`（Phase 6 逆向）

### 未知：flags 内容的业务语义

SmartPlant 样本里 flags 实测 `[0x01, 0x01, 0x01, 0x01]`（4 条全 1）。这告诉我们：

- **Hypothesis A**（最可能）：flag 是 "segment 是否有效 / 是否引用成功" 的 boolean
- **Hypothesis B**：flag 是 segment 类型 enum（1 = Connection, 其他值 = Geometric / Reference），但样本全 1 无法区分
- **Hypothesis C**：flag 是 bit field（bit 0 = valid, bit 1-7 reserved）

**单 fixture 只能部分约束**。Phase 11b 的产出必须明确：
- 如果多 fixture 可得 → 可以断定 flag 语义
- 如果仍是单 fixture → confidence = Decoded（不 claim FullyDecoded），inferred_kind 用 `Option<SegmentKind>`

### 扩展路径：PSMsegmenttable 可能有更多字段

当前 parser 只读了 12B header + N×1B flags，但 **stream size 是 12B**（header 占满，实际没有 flag）或 16-44B？需要 hex walk 确认：

```
PSMsegmenttable stream = header (12B) + flags (N×1B) + ???
```

**如果 stream 末尾有 trailing bytes**，很可能含 per-segment record（cluster_id / role / target），而不是纯 flag 数组。

## 范围

| 文件 | 改动 | 行数（估）|
|---|---|---|
| `Cargo.toml` | version 0.10.0 → 0.11.0 | ±1 |
| `src/parsers/psm_tables.rs` | `parse_psm_segment_table` 深化 | +140 |
| `src/model.rs` | `PsmSegmentRecord` / `SegmentKind` 枚举 / `PsmSegmentTable.records` 字段 | +90 |
| `src/streams/psm_tables.rs` | records 挂到 doc | +15 |
| `src/crossref.rs` | 建立 segment ↔ sheet endpoint ↔ layout 三向对账 | +80 |
| `src/inspect/report.rs` | "--- PSMsegmenttable ---" 段展示 record 结构 | +50 |
| `src/inspect/coverage.rs` | 动态探针 | +20 |
| `tests/parse_real_files.rs` | 真实 fixture 断言 | +80 |
| `tests/unit_parsers.rs` | 单测 | +120 |
| `CHANGELOG.md` | `[0.11.0]` 段 | +80 |
| **本 plan** | | +本文件 |

~700 行，核心 350 行。

## 具体实现

### 1. 新类型

```rust
// src/model.rs
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SegmentKind {
    /// 连接实体：对应 relationship 的端点（推测）
    Connection,
    /// 几何实体：对应 layout 图元的独立几何（推测）
    Geometric,
    /// 引用实体：跨 cluster 引用另一个 object（推测）
    Reference,
    /// 当前 Phase 11b 无法确定具体 kind，保留原 flag 字节
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PsmSegmentRecord {
    pub segment_id: u32,         // 0-based index，等于 flags 数组的 position
    pub flag: u8,                // 原始 flag byte
    pub inferred_kind: SegmentKind,
    /// 推测的 owner cluster id；由 Phase 11a 的 declared_segment_count 反推
    pub owner_cluster_id: Option<u32>,
    pub confidence: Confidence,
}

pub struct PsmSegmentTable {
    pub magic_u32_le: u32,
    pub count: u32,
    pub flags: Vec<u8>,  // 保留旧视图向后兼容
    #[serde(default)]
    pub records: Vec<PsmSegmentRecord>,  // 新
    pub trailing_bytes: Option<Vec<u8>>,  // hex walk 如果发现有 trailer，保留
}
```

### 2. Parser 升级

```rust
// src/parsers/psm_tables.rs
pub fn parse_psm_segment_table_decoded(
    bytes: &[u8],
    cluster_table: Option<&PsmClusterTable>,
) -> Option<PsmSegmentTable> {
    let (magic, count) = parse_header(bytes)?;
    let flags_start = 12;  // 固定 header size
    let flags_end = flags_start + count as usize;

    if bytes.len() < flags_end {
        return None;
    }

    let flags: Vec<u8> = bytes[flags_start..flags_end].to_vec();
    let trailing: Option<Vec<u8>> = if bytes.len() > flags_end {
        Some(bytes[flags_end..].to_vec())
    } else {
        None
    };

    let records: Vec<PsmSegmentRecord> = flags.iter().enumerate().map(|(i, &flag)| {
        let segment_id = i as u32;
        let owner_cluster_id = cluster_table
            .and_then(|ct| attribute_segment_to_cluster(ct, segment_id));
        PsmSegmentRecord {
            segment_id,
            flag,
            inferred_kind: infer_segment_kind(flag),
            owner_cluster_id,
            confidence: Confidence::Decoded,  // flag byte 本身 decoded，kind 是 inferred
        }
    }).collect();

    Some(PsmSegmentTable {
        magic_u32_le: magic,
        count,
        flags,
        records,
        trailing_bytes: trailing,
    })
}

fn attribute_segment_to_cluster(
    ct: &PsmClusterTable,
    segment_id: u32,
) -> Option<u32> {
    // 按 cluster records 的 declared_segment_count 累积定位
    let mut acc: u32 = 0;
    for record in &ct.records_decoded {
        if segment_id < acc + record.declared_segment_count {
            return Some(record.cluster_id);
        }
        acc += record.declared_segment_count;
    }
    None
}

fn infer_segment_kind(flag: u8) -> SegmentKind {
    match flag {
        0x01 => SegmentKind::Connection,  // 样本默认值；推测为连接实体
        0x02 => SegmentKind::Geometric,   // 推测
        0x04 => SegmentKind::Reference,   // 推测
        _ => SegmentKind::Unknown,
    }
}
```

**注意**：`infer_segment_kind` 的映射基于单样本推测；如果多 fixture 发现 flag 取值范围不同，这里需要调整。

### 3. 交叉验证（三向对账）

```rust
// src/crossref.rs
pub struct SegmentReconciliation {
    /// PSMsegmenttable 声明的 segment 总数
    pub declared_segments: u32,
    /// PSMclustertable 声明的 segment 总和
    pub cluster_sum: u32,
    /// Sheet endpoint_records 引用的 rel_field_x 最大值 + 1（应 <= declared_segments）
    pub max_rel_field_x: Option<u32>,
    /// layout.segments 数量（派生层产出）
    pub layout_segments: usize,
    /// 所有对账关系是否一致
    pub consistent: bool,
}
```

放在 `CrossReferenceGraph` 里，`--crossref` 输出：

```
Segment reconciliation:
  declared_segments: 4
  cluster_sum:       4
  max_rel_field_x:   3 (< 4 OK)
  layout_segments:   4
  status:            [CONSISTENT]
```

### 4. inspect report 展示

```
--- PSMsegmenttable (12 bytes, count=4) ---
  [0] id=0 flag=0x01 kind=Connection owner_cluster=1 (PSMcluster0)
  [1] id=1 flag=0x01 kind=Connection owner_cluster=2 (StyleCluster)
  [2] id=2 flag=0x01 kind=Connection owner_cluster=3 (Dynamic Attributes Metadata)
  [3] id=3 flag=0x01 kind=Connection owner_cluster=10 (Sheet6)
```

## 实施步骤

### W1 — Hex walk + trailing bytes 分析（~60min）

1. `pid_inspect --probe-cluster` 拿 PSMsegmenttable 原始 12 字节（header 占满？还是末尾有额外数据？）
2. 核对 stream size vs `12 + count` 公式
3. 如果样本中 stream size > 12 + count，hex walk trailing bytes 内容，猜测字段
4. 产出 "如有 trailing，需扩展 SegmentRecord 字段" 决策

### W2 — 新类型 + parse_psm_segment_table_decoded（~75min）

- `src/model.rs` + `src/parsers/psm_tables.rs`
- 合成 fixture 单测 5 条

### W3 — owner_cluster 反推 + infer_segment_kind（~45min）

- `attribute_segment_to_cluster` 实现 + 单测
- `infer_segment_kind` 映射表（基于样本 flag=0x01 → Connection）
- 单测：owner 分配 / 边界 / 未知 flag

### W4 — CrossReference 三向对账（~60min）

- `SegmentReconciliation` 在 `CrossReferenceGraph` 里
- `--crossref` 输出段扩展
- 单测：CONSISTENT / INCONSISTENT 两路径

### W5 — 真实 fixture 回归（~45min）

- `tests/parse_real_files.rs::psm_segment_records_match_cluster_declarations` 新测
- `tests/parse_real_files.rs::sheet_endpoint_rel_field_x_within_segment_bound` 新测

### W6 — docs + CHANGELOG（~30min）

- `CHANGELOG.md` `[0.11.0]` 段
- `docs/sppid/v0.7.x-status.md` 表 PSMsegmenttable 状态改 FullyDecoded（若 flag 解通）

### W7 — ship v0.11.0

- `cargo fmt/clippy/test` 三零
- commit + tag

## 预计工时

- W1 60min
- W2 75min
- W3 45min
- W4 60min
- W5 45min
- W6 30min
- W7 10min
- **合计 5.4 hr**（区间 4-6hr）

## 验证清单

- [ ] PSMsegmenttable.records.len() == flags.len() == declared_count
- [ ] owner_cluster_id 分配正确（按 Phase 11a declared_segment_count 累积）
- [ ] CrossReference SegmentReconciliation 状态 CONSISTENT
- [ ] Sheet endpoint_records 的 rel_field_x 全部 < declared_segments
- [ ] infer_segment_kind 对未知 flag 返回 Unknown
- [ ] coverage 升级到 FullyDecoded（若 inferred_kind 全命中）或保持 PartiallyDecoded（若单 fixture 不足 confidence）
- [ ] test count: 370+ → 385+
- [ ] clippy / fmt 双零

## 风险缓解

| 风险 | 缓解 |
|---|---|
| flag 语义推测错（样本全 0x01 无法区分）| `SegmentKind::Unknown` 兜底；多 fixture 收集是 11b 的 exit criteria |
| Phase 11a 未 ship 前启动 11b | plan 里明写 W1 前置条件是 Phase 11a ship；W1 会 assert `doc.psm_cluster_table.records_decoded` 可用 |
| trailing_bytes 语义未解 | 保留到 `PsmSegmentTable.trailing_bytes`；inspect 显示 hex preview；confidence 降级 |
| owner_cluster 累积算法错 | 单测 declared_segment_count = 0 的 cluster 不吸收 segment；单测 segment_id 超界返回 None |
| Sheet endpoint_records `rel_field_x` 不在 segment 范围内 | CrossReference 报 INCONSISTENT；分析后决定是扩展 segment 模型还是调整 endpoint 解释 |

## SemVer 判定

- 新增 `PsmSegmentRecord` / `SegmentKind`：minor
- `PsmSegmentTable.records` 新字段（backward compat 保留 flags）：minor
- `CrossReferenceGraph` 新增 `SegmentReconciliation`：minor

综合：**minor bump 0.10 → 0.11.0**。

## Next 候选（11b 完成后）

- **Phase 11c**：Sheet 深层几何解码（roadmap 2.5）— 依赖 Phase 11a + 11b 的 cluster_id / segment_id 外键
- **Phase 11-fixtures**：多真实 fixture 收集（为 11c 大 Phase 准备；若样本获取困难可独立成 meta-phase）

## 交叉引用

- 上游总 roadmap：`docs/plans/2026-04-21-next-steps-roadmap-v0.7.1-onward.md` 阶段 C
- SPPID 战略：`docs/sppid/2026-04-21-sppid-full-parse-roadmap.md` **Phase 2.3**
- 前置 Phase：Phase 11a（PSMclustertable records）+ Phase 6（Sheet endpoint records）
- 后续 Phase：11c / 12a
