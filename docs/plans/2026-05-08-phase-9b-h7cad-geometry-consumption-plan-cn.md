# Phase 9B 开发方案：H7CAD 消费 pid-parse 真实几何

> 日期：2026-05-08  
> 前置：Phase 8C/9A 已在 `pid-parse` 中产生 promoted object hints，并能把两端可定位的 endpoint pair 投影为 `PidGraphicKind::Line + Inferred`。  
> 目标：让 H7CAD 优先显示 `NormalizedPidGeometry` 的 source-backed 点/线几何，同时保留拓扑布局作为 fallback。

## 1. 当前事实

`pid-parse` 当前合同：

- `SheetObjectGeometryHint.position` 会投影为 `PidGraphicKind::Point + PidGeometryConfidence::Inferred`。
- endpoint pair 只有在 `endpoint_a` / `endpoint_b` 都解析到 promoted position 且所有 byte ranges 有效时，才投影为 `PidGraphicKind::Line + Inferred`。
- endpoint pair 推导线段的 `source.record_kind = EndpointPair`，不是 `PrimitiveLine`；调用方必须同时检查 `kind`、`confidence`、`source.record_kind`。
- `Text` / `SymbolInstance` 仍未通过独立 gate，H7CAD 暂不应期待真实文本或符号几何。
- `ProbeOnly` 仍是诊断证据，默认不进入主渲染路径。

H7CAD 当前相关实现：

- `src/io/pid_import.rs` 已有 `add_geometry_entities_from`，能按 `PidGraphicKind` 渲染到 `PID_GEOM_*` layer。
- `PID_GEOM_POINTS` / `PID_GEOM_LINES` / `PID_GEOM_TEXT` / `PID_GEOM_SYMBOLS` layer 已预置。
- `add_geometry_entities_from` 已允许 `Decoded` 和 `Inferred`，跳过 `ProbeOnly`。
- 现有测试中的 synthetic `PidGraphicEntity` 初始化还需要补齐 `coordinate_context`，否则会跟随 `pid-parse` 新合同编译失败。

## 2. 非目标

- 不在 H7CAD 中重新推断几何。所有主渲染几何必须来自 `pid_parse::build_normalized_geometry`。
- 不把 `coordinate_hints` 的裸点渲染为主图对象。无 `field_x` 的 inferred point 继续作为诊断点跳过或放入诊断层。
- 不把 `EndpointPair + Inferred` 当作 decoded primitive line；UI/诊断文案应保留 inferred/source-backed 语义。
- 不推进 Text/Symbol 主渲染，除非 `pid-parse` 后续提供独立 gate。

## 3. 开发切片

### Slice 1：修复 H7CAD 几何测试编译

目标：让 `pid_import.rs` 的几何 synthetic tests 适配 `PidGraphicEntity.coordinate_context`。

文件：

- `d:\work\plant-code\cad\H7CAD\src\io\pid_import.rs`

任务：

1. 在测试 helper `ent()` 中补齐 `coordinate_context`。
2. 优先使用 `PidCoordinateContext::default()`，除非 H7CAD 需要断言 `SourceSheet`。
3. 新增一条 synthetic inferred endpoint line 测试：
   - `kind = Line`
   - `confidence = Inferred`
   - `source.record_kind = EndpointPair`
   - 期望落在 `PID_GEOM_LINES`
4. 保留 `ProbeOnly` 跳过测试。

验收：

```powershell
cargo test --bin H7CAD pid_import::tests::add_geometry_entities_from -- --nocapture
```

### Slice 2：渲染策略收敛

目标：明确 H7CAD 对不同 `PidGeometryConfidence` 的消费策略。

2026-05-08 实现状态：

- H7CAD `add_geometry_entities_from` 已返回渲染统计：
  `rendered_geom_points`、`rendered_geom_lines`、
  `skipped_probe_only_geometry`、`skipped_broad_coordinate_hints`。
- `PidImportSummary` 与 PID 属性面板已暴露 source-backed geometry 统计。
- `Inferred Point` 只有 `source.field_x.is_some()` 时进入
  `PID_GEOM_POINTS`；无 field provenance 的 broad hint 继续跳过。
- `ProbeOnly` 继续跳过主渲染，并计入 skipped 统计。

文件：

- `d:\work\plant-code\cad\H7CAD\src\io\pid_import.rs`

任务：

1. `Decoded`：正常渲染到对应 `PID_GEOM_*` layer。
2. `Inferred Line`：渲染到 `PID_GEOM_LINES`，但 preview metadata / reverse lookup 中保留 `confidence=inferred` 与 `record_kind=endpoint_pair`。
3. `Inferred Point`：
   - `source.field_x.is_some()`：渲染到 `PID_GEOM_POINTS`，作为 object anchor。
   - `source.field_x.is_none()`：继续跳过，避免 broad coordinate hints 污染主图。
4. `ProbeOnly`：继续跳过主渲染。

验收：

```powershell
cargo test --bin H7CAD pid_import -- --nocapture
```

### Slice 3：Open PID 优先级策略

目标：打开 `.pid` 时优先显示 source-backed geometry；当真实几何为空时 fallback 到现有拓扑预览。

2026-05-08 实现状态：

- `Message::FileOpened` 的 PID 分支会读取 `PidImportSummary`：
  - 若 `rendered_geom_points + rendered_geom_lines > 0`，只 fit
    `PID_GEOM_POINTS` / `PID_GEOM_LINES`。
  - 若 source-backed geometry 为空，保持既有
    `PID_OBJECTS_` / `PID_LAYOUT_TEXT` / `PID_RELATIONSHIPS`
    topology preview fallback。
- 打开消息会显示当前使用 source geometry 还是 topology preview。

文件：

- `d:\work\plant-code\cad\H7CAD\src\io\pid_import.rs`
- 如 Fit 逻辑在其他模块，补充对应 scene/view 测试文件。

任务：

1. 计算 `NormalizedPidGeometry` 渲染后的实体数量：
   - `rendered_geom_points`
   - `rendered_geom_lines`
   - `skipped_probe_only`
   - `skipped_broad_coordinate_hints`
2. 当 `rendered_geom_points + rendered_geom_lines > 0`：
   - 允许 `PID_GEOM_*` layer 参与主视图 fit。
   - 拓扑 layout 仍可保留在辅助 layer，但不应决定主图范围。
3. 当真实几何为空：
   - 保持现有 fallback layout 行为。
4. 在 `PidImportSummary` 或 notice 中加入可读统计，方便用户知道当前是 source-backed geometry 还是 fallback。

验收：

```powershell
cargo test --bin H7CAD open_pid_real_sample -- --nocapture
cargo test --bin H7CAD fit -- --nocapture
```

### Slice 4：端到端样本验证

目标：用 `DWG-0201GP06-01.pid` 验证 promoted hints 与 inferred lines 被 H7CAD 消费。

2026-05-08 实现状态：

- H7CAD 已新增 `pid_import_real_sample_geometry_consumes_source_backed_layers`
  测试，直接打开 `DWG-0201GP06-01.pid`。
- 当前 fixture 结果：`PID_GEOM_POINTS=5`、`PID_GEOM_LINES=0`、
  `skipped_probe_only_geometry=68`、
  `skipped_broad_coordinate_hints=64`。
- 因该 fixture 当前没有可消费的 promoted endpoint pair，测试固定
  source-backed points 与 Text/Symbol 禁渲染合同；endpoint-pair line
  xdata 回溯由 synthetic inferred line 测试覆盖，真实 fixture 中若后续
  出现 lines 会同步断言 `confidence=inferred` 与
  `record_kind=endpoint_pair`。

前置：

- 样本位于 `d:\work\plant-code\cad\pid-parse\test-file\DWG-0201GP06-01.pid`，或同步到 H7CAD 测试可访问路径。

任务：

1. 打开样本并统计 native preview layer：
   - `PID_GEOM_POINTS > 0`
   - `PID_GEOM_LINES > 0`（取决于 Phase 9A endpoint 两端 mapping）
2. 确认 `PID_GEOM_LINES` 中 inferred lines 的 preview metadata 能回溯到 endpoint pair。
3. 确认无 `PID_GEOM_TEXT` / `PID_GEOM_SYMBOLS` 误渲染。

验收：

```powershell
cargo test --bin H7CAD pid_import_real_sample_geometry -- --nocapture
```

## 4. 当前 H7CAD 风险

H7CAD 当前分支还有与本计划无关的编译问题，需要先收敛或隔离：

- `WireModel` / `TruckEntity` 新增 `fill_tris` 后存在漏补或重复补。
- `StatusBar::view` 调用参数与签名不一致。
- `Message` 缺少 scale popup 相关 variants。
- `HatchModel::boundary` 已变成 `Arc<Vec<[f32; 2]>>`，部分测试/导出代码仍传 `Vec<_>`。
- `PidGraphicEntity` 初始化缺少 `coordinate_context`。

建议先让 H7CAD 恢复到可运行 `cargo test --bin H7CAD pid_import` 的状态，再推进 Slice 2/3。不要把 H7CAD UI merge 修复和 pid geometry 消费混在一个 PR 里。

## 5. PR 拆分

| PR | 内容 | 验收 |
|---|---|---|
| PR-9B-A | H7CAD `pid_import` synthetic tests 适配 `coordinate_context`，新增 inferred endpoint line 渲染测试 | `cargo test --bin H7CAD pid_import::tests::add_geometry_entities_from -- --nocapture` |
| PR-9B-B | 渲染统计与 `PidImportSummary` / notice 扩展 | `cargo test --bin H7CAD pid_import -- --nocapture` |
| PR-9B-C | `open_pid` source-backed geometry 优先与 fit 策略 | real sample open/fitting tests |
| PR-9B-D | 真实样本端到端验证与文档更新 | H7CAD targeted + pid-parse geometry tests |

## 6. 回滚策略

- 若 H7CAD 渲染出现空视图或错误范围，保留 `build_normalized_geometry` 输出不变，仅关闭 H7CAD source-backed geometry 优先策略。
- 若 inferred line 数量与 fixture inventory 不一致，回退 H7CAD 消费，不回退 `pid-parse` Phase 9A；先在 `pid-parse` 侧补 fixture 断言。
- 若 `PID_GEOM_POINTS` 过多影响视图，继续只渲染 `field_x` backed points，broad coordinate hints 不进入主图。

## 7. 下一步执行顺序

1. 修复 H7CAD 当前编译阻塞中与 `pid_import` 直接相关的 `coordinate_context`。
2. 新增 inferred line synthetic render test。
3. 跑 `cargo test --bin H7CAD pid_import -- --nocapture`。
4. 若 H7CAD 仍被 UI/scene merge 错误阻塞，先单独建 PR 修复 H7CAD 编译基线。
5. 编译基线恢复后，再实现 open/fallback/fit 策略。
