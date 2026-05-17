# Phase 21 开发方案：D06 解析覆盖收敛与关系/Sheet 审计闭环

> 日期：2026-05-18  
> 工作目录：`D:\work\plant-code\cad\pid-parse`  
> 触发样本：`test-file\D06.pid`  
> 基线来源：`pid_inspect --geometry-summary`、`pid_inspect --coverage`、结构化 JSON 计数  
> 状态：待执行

## 0. 目标摘要

本阶段不追求“新增一个大 decoder”，而是把 D06 作为新的真实 fixture
纳入解析质量闭环，回答三个工程问题：

1. D06 当前解析结果是否稳定、可回归验证？
2. D06 中曾出现 `relationship_probes = 10` 但 `object_graph.relationships = 0`
   是数据真实缺失、解析规则不足，还是关联条件过窄？
3. D06 的 Sheet 剩余 evidence（`probe-only unknown = 8`、`GraphicGroup = 21`、
   `0x0010 = 20`）是否足以支撑下一步 typed decoding，还是只能继续
   audit-only？

阶段完成后，应得到一个可重复的 D06 baseline 测试、一个关系图诊断结论、
一份 Sheet unknown / audit inventory，并且不破坏 Phase 14-20 的既有边界。

## 1. 当前 D06 基线

### 1.1 命令基线

```powershell
cargo run --bin pid_inspect -- "test-file/D06.pid" --geometry-summary
cargo run --bin pid_inspect -- "test-file/D06.pid" --coverage
```

### 1.2 解析结果

`D06.pid` 能被当前 parser 正常解析，命令退出码为 0。

Geometry summary：

| 类别 | 数量 |
|---|---:|
| Total entities | 97 |
| Decoded total | 25 |
| Decoded Line (`GLine2d` / `igLine2d`) | 0 |
| Decoded Polyline (`igLineString2d`) | 6 |
| Decoded Point (`igPoint2d`) | 10 |
| Decoded Text (`igTextBox`) | 4 |
| Decoded SymbolInstance (`igSymbol2d`) | 2 |
| Decoded Annotation (`JStyleOverride`) | 3 |
| Inferred coordinate points | 64 |
| Probe-only unknown | 8 |

Coverage：

| 状态 | 数量 |
|---|---:|
| Fully decoded | 7 |
| Partially decoded | 6 |
| Identified only | 13 |
| Unknown | 0 |

结构化 JSON 计数：

| 结构 | 数量 |
|---|---:|
| streams | 56 |
| JSite | 10 |
| Sheet streams | 1 (`/Sheet6`) |
| PSM roots | 7 |
| PSM cluster entries / decoded records | 5 / 5 |
| PSM segment entries | 4 |
| DocVersion2 / DocVersion3 records | 2 / 2 |
| AppObject entries | 5 |
| Dynamic Attribute records | 47 |
| DA trailers | 25 |
| Relationship probes | 10 |
| Object inventory items | 23 |
| Object graph objects / relationships | 10 / 10 |
| `/Sheet6` GraphicGroup audit records | 21 |
| `/Sheet6` `0x0010` audit records | 20 |

### 1.3 关键观察

- D06 不是解析失败样本；它是“解析成功但线段几何薄弱”的样本。
- Slice B 已确认关系图缺口来自 parser 关联条件过窄：D06 的关系身份存放在
  `P&IDAttributes` 的 `ModelItemType=Relationship` +
  `ModelID=Relationship.<GUID>` 中，但没有 `class_id == 0xF6` trailer。
  当前修复会保留 10 个 unresolved relationships，端点仍为 `None`。
- `decoded line = 0` 不一定是 bug。D06 可能主要用 polyline / point / annotation
  表达局部图形，或者线段语义藏在仍 audit-only 的记录里。
- `0x0010` 在 D06 只有 20 条，不足以单独命名 sub-kind；必须继续遵守
  Phase 20 partial AC：不命名 sub-kind，不实现 typed DTO。

## 2. 需求与成功标准

### REQ-D06-01：D06 fixture baseline 可回归

**成功标准**：

- 新增或扩展真实文件测试，能稳定解析 `test-file/D06.pid`。
- 测试断言 D06 的核心结构计数：Sheet、PSM 表、DocVersion、DA、object inventory、
  object graph、geometry summary。
- 测试失败时能指出是“解析失败”“计数漂移”还是“几何输出漂移”。

### REQ-D06-02：关系图缺口被诊断

**成功标准**：

- 明确解释为什么 D06 曾有 10 个 relationship probes，但 object graph relationships 为 0。
- 若是 parser 逻辑不足：最小修复关系抽取 / 关联逻辑，并加 D06 回归测试。
- 若是 D06 数据本身不满足当前 relationship 建图条件：写入分析文档，测试锁定
  当前合理行为，避免误判为回归。

### REQ-D06-03：Sheet audit evidence 被分层归档

**成功标准**：

- 对 `/Sheet6` 的 decoded、inferred、probe-only、audit-only evidence 形成一张
  inventory 表。
- `GraphicGroup = 21`、`0x0010 = 20` 有 byte-range 和 OID / payload 层面的样例。
- 文档明确哪些 evidence 可用于下一阶段，哪些仍禁止 promotion。

### REQ-D06-04：保持 Phase 14-20 边界

**成功标准**：

- 不把 `0x0010.leading_word` 命名为 semantic sub_kind。
- 不新增 `SheetSubRecord0x0010Kind` typed DTO。
- 不把 `GraphicGroup` tail payload 解释为 child OIDs，除非另有独立证据。
- 所有已有 decoder ratchet 仍通过。

## 3. 非目标

- 不实现 Phase 21 typed `0x0010` sub-kind DTO。
- 不实现 `JStyleOverride` / `GraphicGroup` / `0x0010` reference resolver。
- 不把 D06 的 inferred coordinate points 直接提升为 decoded geometry。
- 不修改 writer / round-trip 行为。
- 不提交或移动 fixture / DLL / IDA 数据库文件。

## 4. 开发切片

### Slice A：D06 baseline ratchet

**目的**：把 D06 当前解析能力变成自动回归约束。

**预计改动文件**：

- `tests/parse_real_files.rs`
- 可选：`docs/analysis/2026-05-18-d06-parse-baseline.md`

**实现步骤**：

1. 添加 `d06_pid_parses_with_expected_structure_and_geometry_summary`。
2. 使用现有 `parse_test_file("D06.pid")` / `build_normalized_geometry` 测试模式。
3. 断言：
   - `streams.len() == 56`
   - `sheet_streams.len() == 1`
   - `psm_roots.entries.len() == 7`
   - `psm_cluster_table.entries.len() == 5`
   - `psm_segment_table.entries.len() == 4`
   - `doc_version2_decoded.records.len() == 2`
   - `version_history.records.len() == 2`
   - `dynamic_attributes.attribute_records.len() == 47`
   - `dynamic_attributes.record_trailers.len() == 25`
   - `dynamic_attributes.relationship_probes.len() == 10`
   - `object_inventory.items.len() == 23`
   - `object_graph.objects.len() == 10`
   - `object_graph.relationships.len() == 0`
4. 断言 normalized geometry：
   - total = 97
   - decoded total = 25
   - decoded lines = 0
   - decoded polylines = 6
   - decoded points = 10
   - decoded texts = 4
   - decoded symbols = 2
   - decoded annotations = 3
   - inferred points = 64
   - probe-only unknown = 8
5. 断言 `/Sheet6` audit collections：
   - `decoded_graphic_groups.len() == 21`
   - `decoded_sub_records_0x0010.len() == 20`

**验证命令**：

```powershell
cargo test --test parse_real_files d06_pid_parses_with_expected_structure_and_geometry_summary -- --nocapture
```

**完成条件**：

- D06 baseline 测试通过。
- 测试失败信息能定位漂移类别。

### Slice B：关系图缺口诊断

**目的**：解释或修复 `relationship_probes = 10` 但 `object_graph.relationships = 0`。

**执行补记（2026-05-18）**：已选择修复路径。D06 的 10 条
`P&IDAttributes` 关系记录携带 `ModelItemType=Relationship` 和
`ModelID=Relationship.<GUID>`，与 relationship probes 一一对应；它们缺少
`class_id == 0xF6` trailer，因此没有 `record_id` / `field_x`，但 GUID 身份
足够进入 `ObjectGraph.relationships`，端点保持 unresolved。

**预计改动文件**：

- `src/cfb/reader.rs`
- `src/parsers/dynamic_attr_records.rs`
- `src/crossref.rs`
- `tests/parse_real_files.rs`
- `docs/analysis/2026-05-18-d06-relationship-gap.md`

**实现步骤**：

1. 在测试或临时 probe 中列出 D06 的：
   - `relationship_probes[*].guid`
   - `record_trailers[*].class_id`
   - `record_trailers[*].relationship_guid`
   - `AttributeRecord.class_name`
   - object graph 建图时被接受 / 跳过的原因
2. 对 `build_object_graph` 的 relationship 分支做最小定位：
   - 是否要求 `AttributeRecord.class_name == "Relationship"`？
   - 是否要求 record 有 `DrawingID`？
   - 是否因为 D06 relationship record 只有 trailer/probe，没有完整属性而跳过？
3. 根据证据二选一：
   - **修复路径**：如果 D06 提供了足够字段，扩展建图逻辑，让 relationship 从
     attribute / probe / trailer evidence 进入 `ObjectGraph.relationships`。
   - **记录路径**：如果 D06 缺少 source/target 或 drawing id，保持 0 relationships，
     但在分析文档和测试注释中说明这是“数据不足”而非 parser failure。
4. 若走修复路径，新增最小单元测试覆盖 D06 的关系建图条件。

**验证命令**：

```powershell
cargo test --lib cfb::reader::tests -- --nocapture
cargo test --test parse_real_files d06_pid_parses_with_expected_structure_and_geometry_summary -- --nocapture
```

**完成条件**：

- 有明确结论：修复并产生 relationships，或证明当前 0 relationships 合理。
- 不因 D06 特例破坏 DWG-0201 / DWG-0202 / A01 的 object graph 现有行为。

### Slice C：Sheet `/Sheet6` audit inventory

**目的**：把 D06 的 Sheet 剩余 evidence 分层，决定后续 decoder 优先级。

**预计改动文件**：

- `docs/analysis/2026-05-18-d06-sheet6-audit-inventory.md`
- 可选：`examples/probe_d06_sheet6_audit.rs`

**实现步骤**：

1. 生成 `/Sheet6` 的 decoded/audit 表：
   - `igLineString2d` 6
   - `igPoint2d` 10
   - `igTextBox` 4
   - `igSymbol2d` 2
   - `JStyleOverride` 3
   - `GraphicGroup` 21
   - `0x0010` 20
   - probe-only unknown 8
2. 对 `GraphicGroup` 记录抽取样例：
   - byte range
   - oid
   - `group_kind_word`
   - `sub_type_word`
   - `raw_reference_payload.len()`
3. 对 `0x0010` 记录抽取样例：
   - byte range
   - `bytes_to_follow`
   - `leading_word`
   - payload 前 16 字节 hex
4. 对 probe-only unknown 记录抽取 note / source byte range。
5. 输出“下一 decoder 候选排序”：
   - 最高优先：关系图缺口（若数据支持）
   - 中优先：D06-specific unknown pattern inventory
   - 暂缓：`0x0010` typed DTO / `GraphicGroup` child OIDs

**验证命令**：

```powershell
cargo run --bin pid_inspect -- "test-file/D06.pid" --geometry-summary
cargo run --bin pid_inspect -- "test-file/D06.pid" --json
```

**完成条件**：

- 分析文档包含 D06 Sheet audit 表和样例。
- 文档明确哪些 evidence 不足以 promotion。

### Slice D：最小 inspect 可见性改进

**目的**：如果 Slice C 发现现有 CLI 很难查看 audit-only 集合，则做一个小型可见性增强。

**预计改动文件**：

- `src/bin/pid_inspect.rs`
- `tests/parse_real_files.rs` 或新增 CLI snapshot 测试（若项目已有模式）

**候选实现**：

- 在 `--geometry-summary` 后追加 audit-only summary：
  - `GraphicGroup audit records`
  - `0x0010 audit records`
  - `JStyleOverride records`
- 或新增 `--sheet-audit-summary`，只输出 per-sheet decoded/audit count。

**取舍规则**：

- 如果只为 D06 临时分析服务，不改 CLI，保留文档即可。
- 如果后续 Phase 21/22 会频繁比较 Sheet audit count，新增 CLI summary。
- CLI 输出不得承诺 `0x0010` semantic sub-kind。

**验证命令**：

```powershell
cargo run --bin pid_inspect -- "test-file/D06.pid" --geometry-summary
cargo test --test parse_real_files d06_pid_parses_with_expected_structure_and_geometry_summary -- --nocapture
```

**完成条件**：

- 人类能一眼看到 D06 `/Sheet6` decoded/audit 分布。
- CLI 文案仍区分 decoded / inferred / probe-only / audit-only。

### Slice E：收口验证与文档更新

**目的**：确保 D06 进入长期回归集，不把一次性发现留在聊天上下文里。

**预计改动文件**：

- `CHANGELOG.md`
- `findings.md`
- `progress.md`
- `AGENTS.md`（仅当 D06 结论需要长期 agent 记忆）
- `docs/analysis/2026-05-18-d06-parse-baseline.md`

**验证命令**：

```powershell
cargo test --test parse_real_files d06 -- --nocapture
cargo test --test parse_real_files decoder -- --nocapture
cargo fmt --all -- --check
```

提交前完整 gate：

```powershell
cargo build --locked --workspace --all-targets
cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
bash .github/scripts/check-missing-docs.sh
```

**完成条件**：

- D06 targeted test 通过。
- 既有 decoder ratchet 通过。
- 文档记录 D06 的关系图结论与 Sheet audit 分层。

## 5. 推荐执行顺序

1. Slice A：先把 D06 现状锁成回归测试。
2. Slice B：诊断 relationship gap，决定是修复还是记录为数据不足。
3. Slice C：写 Sheet audit inventory，形成后续 decoder 输入。
4. Slice D：只有在可见性不足时才改 CLI。
5. Slice E：跑 targeted gates，更新文档。

## 6. Stop-And-Challenge 条件

执行过程中遇到以下任一情况必须暂停，不可静默扩 scope：

- 需要把 `0x0010.leading_word` 命名成 semantic sub-kind。
- 需要新增 `SheetSubRecord0x0010Kind` 或改动 Phase 18/19 DTO 字段。
- 需要把 `GraphicGroup.raw_reference_payload` 解释为 child OIDs。
- D06 relationship 修复会改变现有 fixture 的 relationship count。
- 需要修改 writer / round-trip / publish XML pipeline。
- D06 baseline 和现有 Phase 14-20 ratchet 冲突，无法同时满足。

## 7. 风险与缓解

| 风险 | 影响 | 缓解 |
|---|---|---|
| D06 计数锁得过死，未来合法 decoder 增强导致测试频繁改 | 中 | 测试错误信息说明“这是 intentional ratchet”；增强 decoder 时同步更新计划和分析文档 |
| relationship gap 是数据不足，不可修复 | 低 | 已排除：D06 提供 `ModelID=Relationship.<GUID>`，可保留 unresolved relationships |
| Sheet audit evidence 诱导过早 typed DTO | 高 | 继续遵守 Phase 20 partial AC，只保留 audit/inventory |
| CLI summary 改动扩大 scope | 中 | Slice D 设为可选，仅在重复分析确实痛时实施 |

## 8. 验收清单

- [ ] `D06.pid` 有 targeted parse baseline test。
- [ ] D06 geometry summary 计数被测试锁定或文档化。
- [ ] D06 relationship gap 有结论：修复 / expected zero 二选一。
- [ ] `/Sheet6` audit inventory 文档完成。
- [ ] `0x0010` 与 `GraphicGroup` 仍保持 audit-only。
- [ ] Targeted tests 通过。
- [ ] 既有 decoder tests 通过。

## 9. 下一步执行入口

建议下一次执行从 Slice A 开始：

```powershell
cargo test --test parse_real_files d06_pid_parses_with_expected_structure_and_geometry_summary -- --nocapture
```

若测试尚未存在，先在 `tests/parse_real_files.rs` 中按本计划新增 D06 baseline
测试，再进入 Slice B 的 relationship gap 诊断。
