# Phase 9C 开发方案：真实 fixture 覆盖与 endpoint line 闭环

> 日期：2026-05-08  
> 前置：Phase 9A 已在 `pid-parse` 中产生 `Inferred Point` 与条件性的
> `Inferred Line`；Phase 9B 已让 H7CAD 消费 source-backed geometry，
> 并在打开 PID 时优先 fit `PID_GEOM_POINTS` / `PID_GEOM_LINES`。

## 1. 当前事实

- H7CAD `cargo check --locked` 已通过。
- H7CAD `cargo test --locked -j 1 pid_import -- --nocapture`
  已通过：`81 passed, 390 filtered out`。
- `DWG-0201GP06-01.pid` 当前端到端统计：
  - `PID_GEOM_POINTS = 5`
  - `PID_GEOM_LINES = 0`
  - `skipped_probe_only_geometry = 68`
  - `skipped_broad_coordinate_hints = 64`
- Synthetic inferred endpoint line 测试已覆盖：
  - `confidence=inferred`
  - `record_kind=endpoint_pair`
  - native entity xdata app 为 `H7CAD_PID_GEOMETRY`
- 当前真实 fixture 还没有覆盖 `PID_GEOM_LINES > 0` 的端到端路径。

## 2. 目标

1. 找到或构造至少一个能稳定产生 `PID_GEOM_LINES > 0` 的真实 `.pid`
   fixture。
2. 在 `pid-parse` 中解释为什么现有 `DWG-0201GP06-01.pid` 只有
   promoted points、没有 endpoint-pair lines。
3. 在 H7CAD 中补真实 endpoint line 端到端测试，确认 layer、summary、
   xdata 和 fit 策略一致。
4. 明确 Text/Symbol 的进入条件，避免没有独立 gate 时提前主渲染。

## 3. 非目标

- 不在 H7CAD 中重新推断 endpoint line。H7CAD 只消费
  `pid_parse::build_normalized_geometry` 的合同输出。
- 不为了让测试过而放宽 `pid-parse` endpoint-pair gate。
- 不把 broad coordinate hints 渲染成主图对象。
- 不把 Text / Symbol 放进主渲染，除非 `pid-parse` 先提供独立 gate。

## 4. 开发切片

### Slice 1：真实 fixture inventory

目标：建立一个小型 fixture 清单，记录每个 `.pid` 的 source-backed
geometry 形态。

2026-05-08 实现状态：

- 已新增 `geometry_fixture_inventory_reports_normalized_geometry_counts`。
- 当前 5 个登记 fixture 均可用，但仍低于目标 8 个。
- 当前 inventory：
  - `DWG-0201GP06-01.pid`：`inferred_points=69`，
    `inferred_lines=0`，`probe_only_unknowns=68`
  - `DWG-0202GP06-01.pid`：`inferred_points=69`，
    `inferred_lines=0`，`probe_only_unknowns=45`
  - `工艺管道及仪表流程-1.pid`：`inferred_points=64`，
    `inferred_lines=0`，`probe_only_unknowns=12`
  - `export-test/publish-data/A01/A01.pid`：
    `inferred_points=132`，`inferred_lines=0`，
    `probe_only_unknowns=19`
  - `export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid`：
    `inferred_points=69`，`inferred_lines=0`，
    `probe_only_unknowns=45`
- 结论：当前登记 fixture 中没有 line-producing fixture。

任务：

1. 扫描可用 `.pid` 样本，至少登记 8 个可访问 fixture；如果本机不足，
   先记录缺口。
2. 对每个 fixture 跑 `pid-parse` 的 `build_normalized_geometry` 统计：
   - `decoded_points`
   - `inferred_points`
   - `decoded_lines`
   - `inferred_lines`
   - `probe_only_unknowns`
   - `skipped_broad_coordinate_hints`
3. 输出一份表格到 `pid-parse` docs，作为后续回归基线。
4. 标记第一个满足 `inferred_lines > 0` 的 fixture 为 Phase 9C 主样本。

验收：

```powershell
cargo test --locked geometry_fixture_inventory -- --nocapture
```

### Slice 2：解释 endpoint line 缺口

目标：定位 `DWG-0201GP06-01.pid` 没有 `PID_GEOM_LINES` 的原因。

2026-05-08 实现状态：

- 已新增 `endpoint_pair_geometry_diagnostics_explain_dwg0201_line_gap`。
- `DWG-0201GP06-01.pid` 诊断结果：
  - `endpoint_pairs=59`
  - `fully_promoted_with_byte_ranges=0`
  - `endpoint_range_missing=0`
  - `position_range_missing=0`
  - `only_endpoint_a_promoted=5`
  - `only_endpoint_b_promoted=6`
  - `neither_endpoint_promoted=48`
  - `inferred_lines=0`
- 结论：line 缺口不是 byte range 问题，而是 endpoint pair 两端没有同时
  映射到 promoted object position。

追加诊断：

- 已新增
  `endpoint_field_x_diagnostics_report_promoted_and_missing_distribution`。
- `DWG-0201GP06-01.pid` endpoint field_x 分布：
  - `unique_endpoint_fields=57`
  - `endpoint_refs=118`
  - `promoted_refs=11`
  - `missing_refs=107`
  - `missing_known_object_refs=99`
  - top missing 中 `630..638` 等多数仍是 known object field_x
- 结论：缺口主要不是 endpoint 指向了未知对象，而是这些 known object 的
  field_x 没有通过当前 Sheet object geometry promotion gate。

追加 gate score 诊断：

- 已新增 `endpoint_missing_known_field_xs_report_promotion_gate_scores`。
- top missing known field_x `630..639` 都位于 `/Sheet6`。
- 这些 field_x 的 best score 均为 `40`，reasons 形态为：
  - `NonEndpointHit`
  - `ObjectFieldResolves`
  - `StableMarkerNearby`
- 但 `candidate_position=None`。
- 结论：这些 endpoint object 身份可以解析，但窗口内没有被
  `nearest_coordinate` 识别出的候选坐标；下一步应分析 Sheet6 这些
  field_x 附近的 byte window 和 coordinate hint 搜索半径/形态。

追加 radius / byte-window 诊断：

- 已新增
  `sheet6_missing_endpoint_field_xs_compare_coordinate_search_radii`。
- 对 `/Sheet6` 的 `field_x=630..639` 比较 `96 / 192 / 384`
  字节搜索半径：
  - 每个半径均 `windows=80`
  - 每个半径均 `candidate_positions=0`
  - best score 仍为 `40`
- byte window 呈现重复记录形态：
  `... 5E 00 22 00 00 00 <field_x-le> 06 00 00 00 08 00 00 00 ...`
  且 field_x 前后存在 f64-like 数值片段，但当前
  `nearest_coordinate` 没有将其识别为坐标候选。
- 结论：单纯扩大搜索半径不能补齐 endpoint line；下一步应针对该
  repeated record shape 增加保守解析实验，而不是只调大 radius。

追加 repeated-record f64 解析实验：

- 已新增
  `sheet6_missing_endpoint_field_xs_have_preceding_f64_coordinate_pairs`。
- 对 `/Sheet6` 的 `field_x=630..639`，固定 marker
  `5E 00 22 00 00 00 <field_x-le>` 前 22 字节处均可解析出有限
  `f64, f64` 数值对：
  - `630 -> (0.244002, 0.000000)`
  - `631..639 -> x` 递增约 `0.081832..0.184125`，
    `y` 稳定为 `0.224561`
- 结论：该 repeated record shape 不是随机噪声，具备作为
  `nearest_coordinate` 之外保守坐标候选来源的价值；下一步应先把它
  做成独立 diagnostic reason，再评估是否纳入正式 promotion gate。

2026-05-08 追加实现状态：

- 已将 f64 pair 实验从测试内 helper 提升为 `sheet_probe` 公共诊断能力：
  - `repeated_f64_pair_candidate_before_field_x`
  - `stable_f64_pair_shape_support`
  - `SheetFieldXWindowScoreReason::RepeatedF64PairBeforeField`
- 该 reason 只进入 score explainability，不增加分数，也不改变
  `populate_object_geometry_hints` 的三条件 promotion gate。
- 验证：
  - `cargo test --locked --lib parsers::sheet_probe -j 1`
  - `cargo test --locked --test parse_real_files sheet6_missing_endpoint_field_xs_have_preceding_f64_coordinate_pairs -j 1 -- --nocapture`
- 结论：`field_x=630..639` 的 repeated f64 shape 已成为可复用诊断
  evidence；下一步如果要纳入主 gate，需要另设坐标尺度/单位转换与
  多 fixture 验证，不能直接替代当前 `SheetCoordinateHint`。

任务：

1. 在 `pid-parse` 侧增加诊断统计：
   - endpoint pair 总数
   - 两端都能解析到 promoted position 的数量
   - 只有一端可解析的数量
   - 两端都缺失 promoted position 的数量
   - byte provenance 缺失导致不能升格的数量
2. 将统计写入 focused test 输出，不改变 JSON 主合同。
3. 根据统计判断缺口属于：
   - fixture 本身没有可升格 endpoint pair
   - object id / drawing id 关联缺失
   - coordinate hint gate 过严
   - byte range provenance 不完整

验收：

```powershell
cargo test --locked endpoint_pair_geometry_diagnostics -- --nocapture
```

### Slice 3：H7CAD 真实 line 端到端测试

目标：一旦找到 line-producing fixture，固定 H7CAD 的真实消费合同。

任务：

1. 新增 H7CAD 测试：
   `pid_import_real_sample_geometry_renders_endpoint_lines`。
2. 断言：
   - `summary.rendered_geom_lines > 0`
   - `PID_GEOM_LINES` layer 数量等于 summary
   - 至少一条 line 的 xdata 包含
     `confidence=inferred`
   - 至少一条 line 的 xdata 包含
     `record_kind=endpoint_pair`
3. 用 `Scene::fit_layers_matching(&["PID_GEOM_POINTS", "PID_GEOM_LINES"])`
   验证 source geometry fit 路径可用。
4. 保留现有 `DWG-0201GP06-01.pid` points-only 测试，避免误把
   lines 当成所有 fixture 的硬要求。

验收：

```powershell
$env:CARGO_TARGET_DIR="D:\work\plant-code\cad\H7CAD\target\cursor-pid-import-test"
cargo test --locked -j 1 pid_import_real_sample_geometry -- --nocapture
Remove-Item Env:CARGO_TARGET_DIR
```

### Slice 4：Text / Symbol gate 准入设计

目标：为后续 Text / Symbol 主渲染定义进入条件。

任务：

1. 在 `pid-parse` 中分别设计 Text 与 Symbol 的独立 promotion gate。
2. Text 至少需要：
   - insertion point
   - text value 或可回溯 text record
   - height / rotation 的保守默认或 provenance
3. Symbol 至少需要：
   - insertion point
   - symbol path 或稳定 placeholder id
   - scale / rotation 的保守默认或 provenance
4. H7CAD 在 gate 未完成前继续断言：
   - `PID_GEOM_TEXT = 0`
   - `PID_GEOM_SYMBOLS = 0`

验收：

```powershell
cargo test --locked text_symbol_geometry_gate -- --nocapture
```

## 5. PR 拆分建议

| PR | 内容 | 验收 |
|---|---|---|
| PR-9C-A | fixture inventory 与 endpoint-pair diagnostics | `cargo test --locked geometry_fixture_inventory endpoint_pair_geometry_diagnostics -- --nocapture` |
| PR-9C-B | H7CAD line-producing fixture 端到端测试 | `cargo test --locked -j 1 pid_import_real_sample_geometry -- --nocapture` |
| PR-9C-C | Text/Symbol gate 设计与 red tests | focused gate tests |
| PR-9C-D | 文档基线与 AGENTS.md/规则迁移建议 | docs review |

## 6. 风险与回滚

- 如果没有可共享的 line-producing fixture：保留 synthetic line 测试，
  将真实 line 测试标记为 soft-skip，并把 fixture 缺口写入文档。
- 如果 endpoint line 数量不稳定：先在 `pid-parse` 侧稳定 promotion
  inventory，再更新 H7CAD 断言。
- 如果 Text/Symbol gate 与现有 Sheet probe 冲突：保持 H7CAD 禁渲染，
  只扩展 `pid-parse` 诊断输出。

## 7. 推荐下一步

先执行 Slice 1 和 Slice 2。只有当 inventory 证明某个 fixture 能稳定
产生 `inferred_lines > 0` 后，再进入 H7CAD 的真实 line 端到端测试。
