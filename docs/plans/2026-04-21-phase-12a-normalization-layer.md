# 开发计划骨架：Phase 12a — 规范化语义图层

> 起稿：2026-04-21
> 目标版本：v1.0.0（major bump 候选；或 v0.13.0 minor，视 API 破坏面）
> 前置：Phase 11a + 11b + 11c 全部 ship
> 估计工时：20-30 hr（大 Phase，分多轮 ship）
> 所属 roadmap：`docs/sppid/2026-04-21-sppid-full-parse-roadmap.md` **Phase 3**
> **特别说明**：本文件是**骨架 plan**，正式实施前需独立 session 做设计评审 + 公开 API RFC

## 目的与定位

> **Phase 12a 不是"多写一个 parser"，是把当前分散在多个模块中的对象、关系、端点、符号、cluster、sheet 信息统一到一套规范化语义图层。**

这是项目从"逆向完备性" → "消费友好 API" 的关键转折。完成后：

- 下游消费者（H7CAD PID 工作台、CI 脚本、外部 binding）面对一个统一的 `PidNormalizedModel`，不用再组合 `dynamic_attributes` + `sheets` + `cross_reference` + `layout`
- `inspect` / `report` / `import_view` / `layout` 全部重构为 normalized model 的消费者
- 每个实体携带 `Provenance`，让"这条数据是哪来的"永远可追溯

## 非目标

- **不引入** 新的 parser 能力（Phase 11 已承担）
- **不引入** 新的 writer 能力（Phase 10 已覆盖）
- **不修改** 现有 PidDocument 的保留字段（backward compat via 新字段 `normalized: Option<PidNormalizedModel>`）
- **不合并** layout / object_graph / cross_reference 到同一个顶层 field — 它们作为**派生视图**继续存在，但统一从 `normalized` 派生

## 核心类型草案

```rust
// src/normalized/mod.rs（新模块）

pub struct PidNormalizedModel {
    pub objects: BTreeMap<NormalizedId, NormalizedObject>,
    pub relationships: BTreeMap<NormalizedId, NormalizedRelationship>,
    pub endpoints: BTreeMap<NormalizedId, NormalizedEndpoint>,
    pub symbol_refs: Vec<NormalizedSymbolRef>,
    pub cluster_refs: Vec<NormalizedClusterRef>,
    pub sheets: BTreeMap<String, NormalizedSheet>,
}

pub struct NormalizedObject {
    pub id: NormalizedId,
    pub kind: NormalizedObjectKind,  // PipeRun / Instrument / Nozzle / Vessel / Note / ...
    pub drawing_id: Option<String>,
    pub model_id: Option<String>,
    pub guid: Option<String>,
    pub attributes: BTreeMap<String, AttributeValue>,
    pub symbol: Option<NormalizedId>,  // 指向 symbol_refs
    pub sheet: Option<String>,          // 所在 sheet name
    pub geometry: Option<NormalizedGeometry>,  // 来自 Phase 11c SheetObjectRecord
    pub provenance: Provenance,
}

pub struct NormalizedRelationship {
    pub id: NormalizedId,
    pub source: Option<NormalizedId>,
    pub target: Option<NormalizedId>,
    pub kind: Option<String>,  // PipingEnd1Conn / ProcessPointCollection / ...
    pub provenance: Provenance,
}

pub struct NormalizedEndpoint {
    pub id: NormalizedId,
    pub owner_object: NormalizedId,
    pub segment_id: Option<u32>,  // Phase 11b 锚点
    pub rel_field_x: Option<u32>, // Phase 6 锚点
    pub provenance: Provenance,
}

pub struct Provenance {
    pub stream_path: String,
    pub record_id: Option<u32>,
    pub field_x: Option<u32>,
    pub cluster_id: Option<u32>,
    pub segment_id: Option<u32>,
    pub sheet_name: Option<String>,
    pub source_layer: SourceLayer,
}

pub enum SourceLayer {
    /// 直接 byte-level decoded（最高 confidence）
    Raw,
    /// 从 decoded 结构化字段派生（次高）
    Decoded,
    /// 跨流推断（confidence 可变）
    Inferred,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema)]
pub struct NormalizedId(String);  // e.g. "obj:PipeRun:DWG-001-PR-0001"
```

## 设计问题待评审

以下问题在独立 session 设计时逐项决议，**不在本骨架 plan 敲定**：

### Q1：NormalizedId 格式

候选：
- **String-based**：`"obj:PipeRun:DWG-001-PR-0001"` — 可读 / 可序列化 / 易 debug，但长
- **struct-based**：`struct { kind: String, drawing_id: String, model_id: Option<String> }` — 类型安全，但 serde/hash 复杂

### Q2：与现有 PidObject / PidRelationship 的关系

候选：
- **A. 平移**：normalized 字段替代 object_graph；旧 `PidDocument.object_graph` deprecate 但保留 3-6 个月
- **B. 共存**：两套视图并存，normalized 作为 "canonical"，object_graph 作为 "raw binding"
- **C. 重写**：object_graph 内部重构为 normalized 的 thin wrapper

推荐 B（最小 breakage） vs A（最干净）的权衡需 session 评审。

### Q3：layout 是否消费 normalized

候选：
- **合并**：`layout.rs` 内部改为从 normalized 生成；旧 `PidLayoutModel` 保持公共 API 不变
- **独立**：normalized 和 layout 各自从 raw 派生；用 provenance 交叉追溯

### Q4：PidDocument 顶层字段演变

候选：
- 新字段 `normalized: Option<PidNormalizedModel>`，默认 derive；老 consumer 无感
- 把 object_graph / cross_reference / layout 全部移到 normalized 内部，顶层只留 `normalized` — 这是 breaking change

### Q5：serde 序列化体积

normalized + 旧 object_graph 可能让 `--json` 输出翻倍。是否要加 `--json-normalized-only` flag？

### Q6：SemVer 判定

- 纯加字段 + deprecation 路径 → minor（0.12 → 0.13）
- 重构 object_graph 内部 → minor 或 major
- 删除 object_graph → major（1.0.0 候选）

## 实施阶段划分

### Phase 12a-1：设计 RFC（~2-3 hr）

- `docs/rfcs/2026-XX-XX-normalization-rfc.md`（新文档类别）
- 覆盖上述 Q1-Q6
- 征求（自己给自己）review + iterate
- 确定 final SemVer 判定

### Phase 12a-2：类型定义 + Provenance infrastructure（~4-6 hr）

- `src/normalized/` 新模块
- `PidNormalizedModel` + 子类型
- `Provenance` + `SourceLayer`
- 单测：type round-trip / provenance 传递

### Phase 12a-3：normalization builder（~6-8 hr）

- `build_normalized(&PidDocument) -> PidNormalizedModel`
- 消费 dynamic_attributes / sheets（11c records）/ cluster_refs / jsites
- 每条记录分配 NormalizedId + 填 Provenance

### Phase 12a-4：下游重构（~8-12 hr）

- `inspect` / `report` / `import_view` 改为消费 normalized
- `layout` 融合决策（Q3）
- 迁移测试：旧 consumer 行为不变 or 明确 deprecation

### Phase 12a-5：ship + docs（~1-2 hr）

- `CHANGELOG.md` 大段写作
- `ARCHITECTURE.md` 深度更新
- `docs/sppid/v1.x-status.md`（如 major bump）

## 预计工时

- 12a-1: 2-3 hr
- 12a-2: 4-6 hr
- 12a-3: 6-8 hr
- 12a-4: 8-12 hr
- 12a-5: 1-2 hr
- **合计 21-31 hr**（区间 20-30hr + buffer）

## 验证清单（待 12a-1 RFC 后补齐）

- [ ] RFC Q1-Q6 均有决议文档
- [ ] normalized 视图能完整 derive 所有现有 object_graph 信息
- [ ] Provenance 追溯到字节级（stream + offset）
- [ ] 下游 consumer（inspect / report）行为等价旧实现
- [ ] test count 增幅合理（估 > 420）
- [ ] clippy / fmt 双零
- [ ] 如果 major bump：所有 breaking change 有 migration guide

## 风险

| 风险 | 缓解 |
|---|---|
| RFC 争议拖长 | 12a-1 设一个 deadline（< 4hr），逾期降级为"决议不阻塞下一阶段"的简化决策 |
| PidDocument 膨胀失控 | 新字段 `normalized` 而非 inline 平铺；老字段按 deprecation 路径移除 |
| 性能退化（重复派生）| 提供 memo cache；normalized 只在 consumer 调用 build_normalized 时懒构造 |
| 向后兼容 break 面积失控 | 严格 deprecation 策略：保留 3-6 个 minor 版本；CHANGELOG 明示 |
| Phase 11c 未按期完成 → 12a 起步 | 12a 可部分启动（type 设计 + Provenance 模型），但 builder 必须等 11c |

## SemVer 判定（预期）

综合 Q1-Q6 的候选方案，预期 **minor bump 0.12 → 0.13.0**（保守路径）；如果决议 `A + deprecation` 同时 `delete` 了旧字段，则 **major bump 0.x → 1.0.0**。

Phase 12a 本身可以多轮 ship（12a-2 先 minor 加类型，12a-4 再 minor 做下游重构）。

## Next 候选

- **Phase 12b**：consumed/leftover 字节验证框架（roadmap Phase 4）— 和 normalization provenance 天然配对
- **Phase 13**：roadmap Phase 5 验收

## 交叉引用

- 上游总 roadmap：`docs/plans/2026-04-21-next-steps-roadmap-v0.7.1-onward.md` 阶段 D
- SPPID 战略：`docs/sppid/2026-04-21-sppid-full-parse-roadmap.md` **Phase 3**
- 前置 Phase：11a / 11b / 11c（parser 结构化完成）
- 平行 Phase：12b（byte-audit）
- 必要 RFC：`docs/rfcs/2026-XX-XX-normalization-rfc.md`（本 Phase 12a-1 产出）
