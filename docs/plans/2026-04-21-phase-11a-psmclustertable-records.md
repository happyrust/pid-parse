# 开发计划：Phase 11a — PSMclustertable per-record 结构化

> 起稿：2026-04-21
> 目标版本：v0.10.0（minor bump）
> 前置：Phase 10j ship（v0.9.0 DocumentSummaryInformation section 2）
> 估计工时：6-8 hr
> 所属 roadmap：`docs/sppid/2026-04-21-sppid-full-parse-roadmap.md` **Phase 2.2**

## 动机

当前 `PSMclustertable` 的解析深度：

- **已解**：header（magic `0x636C7374` = "clst"）+ declared_count + 按 null-terminated 字符串枚举出的 cluster 名列表
- **未解**：每条记录在 name 之外的字段（推测含 cluster_id / index / flags / type_tag / segment_count），当前以 raw offset 形式呈现

`inspect --coverage` 对 PSMclustertable 的 status 是 **PartiallyDecoded**：

```
[PART] PSMclustertable (265B)
       parser=psm_tables / field=psm_cluster_table
       note=header + name list only; per-record fields still raw
```

**为什么 Phase 11a 值得投入**：

1. PSMclustertable 是 SmartPlant cluster 体系的**权威清单**，Phase 11b 的 PSMsegmenttable 和 Phase 11c 的 Sheet* 都要通过 cluster id 反查
2. 是 roadmap Phase 2 里最容易结构化的一个（样本 `DWG-0201GP06-01.pid` 只有 5 条记录，可完整 hex walk）
3. 能把 1 个 PartiallyDecoded 升级为 FullyDecoded — 覆盖面可量化

## 非目标

- **不解** PSMcluster0 / StyleCluster / Dynamic Attributes Metadata 的内部记录（这是 Phase 11b+ 的事）
- **不做** cluster 之间的关系抽象（那是 Phase 12a 规范化图层）
- **不改** PSMclustertable writer 行为（当前是 verbatim 透传，Phase 11a 只管 reader；writer 端的 declarative cluster 增删留给 Phase 11a-writer 或 12b）

## 逆向策略

### 已知锚点

1. **记录总数 = `declared_count`**：header 里明说，可做 record count guard
2. **名称已稳定提取**：cluster 名在 record 中出现的 offset 已知，可作为 record boundary 上下文
3. **PSMsegmenttable `flags: [u8; N]`**：N 应等于某种 aggregate segment count；如果 PSMclustertable 里有 per-record `segment_count`，应满足 `sum(segment_count) == N`
4. **每条 cluster 应能映射到实际 CFB 流**：`doc.clusters[i].path` vs PSMclustertable name 一一对应（已在 CrossReference `ClusterCoverage` 里验证）

### 未知字段推测（待 hex walk 确认）

基于 OLE/CFBF 和 SmartPlant 一般模式：

```
每条 PSMclustertable record 推测结构（WIP，需 hex walk 确认）：

  +0:   u32 cluster_id        （全局唯一 id）
  +4:   u16 type_tag          （区分 PSMcluster / StyleCluster / Sheet / DynamicAttrs 等）
  +6:   u16 flags             （bit field，具体含义待查）
  +8:   u32 declared_segment_count  （用于与 PSMsegmenttable 对账）
  +12:  u16 name_char_count   （UTF-16LE 字符数，含 NUL 终止符）
  +14:  UTF-16LE name         （name_char_count × 2 字节）
  +?:   可能的 padding / trailer
```

以上是 **待验证假设**，第一步就是 hex walk 推翻或确认。

### Hex walk 实施步骤

1. 打开 `docs/sppid/v0.7.x-status.md` 里引用的单真实 fixture `test-file/DWG-0201GP06-01.pid`
2. 用 `pid_inspect --probe-cluster` 拿到 PSMclustertable 流的 offset + 全 265 字节 hex dump
3. 用已知 5 个 cluster 名（`PSMcluster0` / `StyleCluster` / `Dynamic Attributes Metadata` / `Sheet6` / `Unclustered Dynamic Attributes`）定位每条 record 边界
4. 对 record 内非 name 部分做逐字节分类：
   - **差异字节**（5 条记录间 varies）：高概率是 id / index / segment_count
   - **相同字节**（所有记录一致）：高概率是 padding / 固定 tag
   - **单调递增**：高概率是 cluster_id / index
5. 验证推测：用 `StyleCluster` 这个 cluster 在代码里已知 kind = `StyleCluster` 反查 type_tag 值

### 交叉验证锚点

| 验证 | 公式 | 来源 |
|---|---|---|
| record count | `psm_cluster_table.records.len() == declared_count` | header |
| cluster name ↔ CFB path | `records[i].name == doc.clusters[i].path` | CrossReference 已用 |
| segment count | `sum(records[i].declared_segment_count) == psm_segment_table.flags.len()` | 新（本 Phase 建立）|
| type_tag ↔ ClusterKind | `records[i].type_tag` 映射到 `ClusterKind::{PsmCluster, StyleCluster, Sheet, DynamicAttributesMetadata, UnclusteredDynamicAttributes}` | `doc.clusters[i].kind` 已有 |

**type_tag 映射表 是本 Phase 最关键的产出**。确定后它成为 Phase 11b 的锚点。

## 范围

| 文件 | 改动 | 行数（估）|
|---|---|---|
| `Cargo.toml` | version 0.9.0 → 0.10.0 | ±1 |
| `src/parsers/psm_tables.rs` | `parse_psm_cluster_table` 深化，返回 `PsmClusterTableDecoded` 含 per-record 字段 | +180 |
| `src/model.rs` | `PsmClusterRecordDecoded` 新类型 + `PsmClusterTable` 新字段 `records_decoded` | +80 |
| `src/streams/psm_tables.rs` | 把 decoded 结果挂到 `PidDocument.psm_cluster_table` | +20 |
| `src/crossref.rs` | 建立 segment count 对账 + 扩展 `ClusterCoverage.records_decoded_count` | +40 |
| `src/inspect/report.rs` | "--- PSMclustertable ---" 段新增 per-record 结构化展示 | +60 |
| `src/inspect/coverage.rs` | 动态探针升级：`records_decoded.len() == declared_count` → FullyDecoded | +20 |
| `tests/parse_real_files.rs` | 真实 fixture 断言 5 条 record 字段 | +80 |
| `tests/unit_parsers.rs` | 合成 fixture 单测 6 条 | +150 |
| `CHANGELOG.md` | `[0.10.0]` 段 | +80 |
| **本 plan** | | +本文件 |

~750 行，核心 400 行。

## 具体实现

### 1. 新模型

```rust
// src/model.rs
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PsmClusterRecordDecoded {
    pub cluster_id: u32,
    pub type_tag: u16,
    pub flags: u16,
    pub declared_segment_count: u32,
    pub name: String,
    /// 未识别字段（record 末尾若有 trailing bytes 保留，for round-trip + audit）
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trailer_bytes: Vec<u8>,
    /// 推断的 ClusterKind（从 type_tag 映射；Unknown 时保留 raw）
    pub inferred_kind: Option<ClusterKind>,
    pub confidence: Confidence,  // Decoded / Heuristic
}

pub struct PsmClusterTable {
    pub magic_u32_le: u32,
    pub declared_count: u32,
    pub entries: Vec<PsmClusterEntry>,  // 保留旧 name-only 视图向后兼容
    #[serde(default)]
    pub records_decoded: Vec<PsmClusterRecordDecoded>,  // 新
    pub raw_bytes: Option<Vec<u8>>,  // 已有
}
```

**注意**：保留 `entries: Vec<PsmClusterEntry>` 作为向后兼容的 name-only 视图，新 `records_decoded` 作为深度解析视图。Phase 11a 之后旧 consumer 代码继续工作。

### 2. Parser 重写

```rust
// src/parsers/psm_tables.rs
pub fn parse_psm_cluster_table(bytes: &[u8]) -> Option<PsmClusterTable> {
    let (magic, declared_count) = parse_header(bytes)?;  // 已有
    let mut records_decoded = Vec::with_capacity(declared_count as usize);
    let mut cursor = 8;  // 跳过 header

    for _ in 0..declared_count {
        let rec = parse_cluster_record(&bytes[cursor..])?;
        cursor += rec.byte_len();
        records_decoded.push(rec);
    }

    // 对账：cursor 应消费到尾部
    if cursor != bytes.len() {
        // 留 leftover 到 trailer_bytes for audit
    }

    Some(PsmClusterTable {
        magic_u32_le: magic,
        declared_count,
        entries: records_decoded.iter().map(|r| /* 转 name-only */).collect(),
        records_decoded,
        raw_bytes: Some(bytes.to_vec()),
    })
}

fn parse_cluster_record(bytes: &[u8]) -> Option<PsmClusterRecordDecoded> {
    // 按推测的字段布局切片读取；失败返回 None
}
```

### 3. type_tag ↔ ClusterKind 映射

```rust
fn infer_cluster_kind(type_tag: u16) -> Option<ClusterKind> {
    match type_tag {
        // 以下数值基于 hex walk 推测；实现时必须 hex walk 确认
        0x0001 => Some(ClusterKind::PsmCluster),
        0x0002 => Some(ClusterKind::StyleCluster),
        0x0003 => Some(ClusterKind::DynamicAttributesMetadata),
        0x0004 => Some(ClusterKind::UnclusteredDynamicAttributes),
        0x0010 => Some(ClusterKind::Sheet),
        _ => None,
    }
}
```

**关键**：如果 hex walk 发现 type_tag 在样本中 invariant（比如所有 5 条都是同一值），说明它不是 kind tag 而是别的字段；此时降级为 `flags` 的一部分，重新寻找 kind 锚点。

### 4. 交叉验证

```rust
// src/crossref.rs 扩展
impl ClusterCoverage {
    pub fn segment_count_reconciliation(&self, doc: &PidDocument) -> Reconciliation {
        let declared: u32 = doc.psm_cluster_table
            .records_decoded.iter()
            .map(|r| r.declared_segment_count)
            .sum();
        let actual: u32 = doc.psm_segment_table
            .flags.len() as u32;
        Reconciliation { declared, actual, matched: declared == actual }
    }
}
```

`Reconciliation` 进 `--crossref` 输出，mismatch 时打印警告但不 fail。

### 5. coverage 动态探针升级

```rust
// src/inspect/coverage.rs
match name {
    "PSMclustertable" => {
        doc.psm_cluster_table.as_ref().and_then(|t| {
            if t.records_decoded.len() == t.declared_count as usize {
                Some(ParseCoverageStatus::FullyDecoded)
            } else if !t.records_decoded.is_empty() {
                Some(ParseCoverageStatus::PartiallyDecoded)
            } else {
                Some(ParseCoverageStatus::IdentifiedOnly)
            }
        })
    }
    // ...
}
```

### 6. inspect report 展示

```
--- PSMclustertable (265 bytes, declared count=5) ---
  [@+001B] PSMcluster0
    id=0x00000001  kind=PsmCluster  flags=0x0001  segments=1
  [@+0042] StyleCluster
    id=0x00000002  kind=StyleCluster  flags=0x0001  segments=1
  [@+006B] Dynamic Attributes Metadata
    id=0x00000003  kind=DynamicAttributesMetadata  flags=0x0001  segments=1
  [@+00AE] Sheet6
    id=0x00000010  kind=Sheet  flags=0x0001  segments=1
  [@+00CB] Unclustered Dynamic Attributes
    id=0x00000004  kind=UnclusteredDynamicAttributes  flags=0x0001  segments=0
  Segment reconciliation: declared=4 actual=4 [OK]
```

## 实施步骤

### W1 — Hex walk + type_tag 映射表确认（~2hr）

**这一步是整个 Phase 的基础；必须用真实 fixture 做**。

1. `pid_inspect test-file/DWG-0201GP06-01.pid --probe-cluster` 拿 PSMclustertable 265 字节
2. 手工 hex walk 每条 record（5 条），标注推测字段
3. 写一个**临时探针** `examples/probe_psm_cluster_records.rs`（风格对齐既有 `examples/probe_*`）打印字段猜测
4. 用 PSMsegmenttable `flags.len() = 4` 反向对账 declared_segment_count 字段位置
5. 产出 "field offset map" 备忘（不 commit，仅用于 W2 实施参考）

### W2 — `PsmClusterRecordDecoded` 类型 + parse_cluster_record 实现（~90min）

- `src/model.rs` 加类型
- `src/parsers/psm_tables.rs` 深化
- 合成 fixture 单测 6 条（正常 / boundary / type_tag unknown / trailer 有余字节 / byte count 不对账）

### W3 — type_tag ↔ ClusterKind 映射 + crossref 升级（~60min）

- `infer_cluster_kind` 函数
- `ClusterCoverage.segment_count_reconciliation` 扩展
- 单测：inferred_kind 正确 / type_tag unknown 返回 None

### W4 — coverage 动态探针 + inspect report（~45min）

- `inspect::coverage::stream_is_populated` PSMclustertable 分支升级
- `inspect::report::generate_report` 新 per-record 段
- `report_includes_psm_cluster_table_section_shows_decoded_records` 新测

### W5 — 真实 fixture 回归（~45min）

- `tests/parse_real_files.rs` 新断言：5 条 record 字段精确匹配
- 验证 segment_count_reconciliation 对账通过

### W6 — docs + CHANGELOG（~30min）

- `CHANGELOG.md` `[0.10.0]` 段
- `ARCHITECTURE.md` Phase 10k → 11a 进度表更新
- `docs/sppid/v0.7.x-status.md` 表 PSMclustertable 状态改 FullyDecoded

### W7 — ship v0.10.0

- `cargo fmt/clippy/test` 三零
- `git commit -m "feat(parser): v0.10.0 — PSMclustertable per-record decoding (Phase 11a)"`
- `git tag v0.10.0`

## 预计工时

- W1 (hex walk) 120min ← 最大风险集中在这
- W2 90min
- W3 60min
- W4 45min
- W5 45min
- W6 30min
- W7 10min
- **合计 6.7 hr**（预留 1.3hr buffer = 6-8 hr 区间）

## 验证清单

- [ ] PSMclustertable.records_decoded.len() == 5（单 fixture）
- [ ] 每条 record 的 name 与已有 entries[i].name 完全一致
- [ ] 每条 record 的 inferred_kind 与 doc.clusters[匹配 path].kind 一致
- [ ] segment count 对账 declared == actual
- [ ] coverage 动态探针：PSMclustertable 升级到 FullyDecoded
- [ ] inspect report 打印 per-record 字段
- [ ] type_tag 映射表至少覆盖 5 个已知 kind
- [ ] test count: 355+ → 370+
- [ ] clippy / fmt 双零

## 风险缓解

| 风险 | 缓解 |
|---|---|
| Hex walk 推测全错 | W1 就是验证阶段；W1 失败直接降级为 `[PART] PSMclustertable - heuristic only`，不强推 FullyDecoded |
| 单样本过拟合 | **硬约束**：W1 后必须在 plan 里明写"需 ≥ 2 fixture 才声明 FullyDecoded"；目前只有 1 fixture，本 Phase 先定义 confidence=Decoded，FullyDecoded 留给 Phase 11a-1 或后续 |
| type_tag 映射表错 | inferred_kind 用 Option；映射失败保留 raw；单测覆盖 unknown type_tag 的降级路径 |
| declared_segment_count 和 PSMsegmenttable flags.len() 不对账 | 不 fail parser；留 warning 到 `--crossref` 输出；用户能看到 mismatch 再查 |
| trailer_bytes 有非零内容 | 保留到 audit；confidence 降级 Decoded → Heuristic |

## SemVer 判定

- 新增 `PsmClusterRecordDecoded` 类型：minor
- `PsmClusterTable` 新字段 `records_decoded`（`#[serde(default)]`）：minor
- 旧 `entries` 字段保留：backward compat
- coverage 探针行为升级（之前 PART 现在可能 FULL）：行为改善，不 break consumer

综合：**minor bump 0.9 → 0.10.0**。

## Next 候选（11a 完成后）

- **Phase 11b**：PSMsegmenttable 结构化 — **强依赖** Phase 11a 的 cluster_id 映射作为外键
- **Phase 11a-writer**：如果有场景需要，可做 `cluster_replacements` 声明式 API（低优先；多数 consumer 走 passthrough 即可）

## 交叉引用

- 上游总 roadmap：`docs/plans/2026-04-21-next-steps-roadmap-v0.7.1-onward.md` 阶段 C
- SPPID 战略：`docs/sppid/2026-04-21-sppid-full-parse-roadmap.md` **Phase 2.2**
- 前置 Phase：`v0.2.3` PSMroots/clustertable/segmenttable 基础（CHANGELOG [0.2.3]）
- 后续 Phase：11b（PSMsegmenttable）/ 11c（Sheet）
