# Phase 10 开发方案：f64 Record Shape 坐标源与 Endpoint Line 闭环

> 日期：2026-05-09
> 前置：Phase 9C 诊断链已发现 `/Sheet6` missing endpoint field_x 附近存在 repeated f64 pair 坐标候选；
> 当前 5 个 fixture 均为 `inferred_lines=0`，endpoint line 缺口的根因是 known object 的 field_x 没有通过现有 `SheetCoordinateHint` promotion gate。
> 原则：probe → diagnostic reason → 独立验证 → promotion gate 扩展，不跳步。

## 0. 核心发现回顾

Phase 9C Slice 2 的逐层诊断链揭示了以下事实链：

```
endpoint_pairs=59
  → fully_promoted_with_byte_ranges=0
  → only_endpoint_a_promoted=5, only_endpoint_b_promoted=6
  → neither_endpoint_promoted=48

missing endpoint field_x (e.g. 630..639):
  → 均为 known object field_x
  → best score=40 (NonEndpointHit + ObjectFieldResolves + StableMarkerNearby)
  → candidate_position=None
  → 扩大搜索半径 96/192/384 均为 candidate_positions=0

但 field_x marker 前 22 字节处存在 repeated f64 pair：
  → 630: (0.244002, 0.000000)
  → 631..639: x 递增 ~0.082..0.184, y 稳定 0.224561
  → 非随机噪声，坐标呈连续递增
```

结论：当前 `nearest_coordinate` 搜索范围和形态不覆盖该 repeated record shape 中的 f64 pair；这些 f64 pair 是连接 endpoint line 的关键缺失坐标源。

## 1. 目标

1. 将 repeated f64 pair 从纯诊断 reason 升级为**保守坐标候选源**，作为 `SheetCoordinateHint` 的补充。
2. 让至少一个真实 fixture 产生 `inferred_lines > 0`。
3. 保持 probe/decode 分层纪律：新坐标源在 multi-fixture 验证前只作为 `Inferred`，不作为 `Decoded`。
4. 不降低现有 promotion threshold 或移除任何 gate 条件。

## 2. 非目标

- 不替换现有 `SheetCoordinateHint` / `nearest_coordinate` 搜索机制。
- 不把 f64 pair 坐标当作最终 CAD 坐标（可能存在单位转换 / 坐标系偏移）。
- 不把 Text/Symbol promotion 混入本 phase。
- 不放宽 endpoint pair 的双端 promoted position 要求。

## 3. 开发切片

### Slice 1：f64 Pair 坐标候选 DTO 与 extraction helper

目标：将 Phase 9C 测试内的 f64 pair 提取逻辑提升为 `sheet_probe` 公共能力。

文件：
- `src/parsers/sheet_probe.rs`
- `src/model.rs`（如需新 DTO）

任务：

1. 新增 `SheetFieldXF64PairCandidate` DTO：
   - `field_x: u32`
   - `marker_offset: usize`（marker `5E 00 22 00 00 00 <field_x-le>` 在 Sheet stream 中的绝对偏移）
   - `f64_pair_offset: usize`（f64 pair 起始偏移，= marker_offset - 22）
   - `x: f64, y: f64`
   - `confidence: PidGeometryConfidence::Inferred`
   - `reason: &str`（"repeated_f64_pair_before_field_marker"）
2. 新增 `extract_f64_pair_candidates_for_field_xs(sheet_data: &[u8], field_xs: &[u32]) -> Vec<SheetFieldXF64PairCandidate>`。
3. 基本过滤条件：
   - f64 值必须为有限值（`is_finite()`）
   - x, y 绝对值必须在合理 CAD 坐标范围（暂定 `0.0..10000.0`）
   - marker offset - 22 不得小于 0
   - 同一 `field_x` 不得重复提取多个候选（取第一个稳定 marker 命中）

验收：

```powershell
cargo test --lib parsers::sheet_probe::tests::f64_pair_candidate_extracts_finite_coordinates_from_marker -- --nocapture
```

TDD 红测先行：先写测试断言 helper 存在且能输出 candidate，编译失败后再实现。

### Slice 2：f64 Pair 候选接入 promotion gate

目标：将 f64 pair 坐标候选作为 `populate_object_geometry_hints` 的补充位置源。

文件：
- `src/geometry.rs`
- `src/parsers/sheet_probe.rs`

任务：

1. 在 `populate_object_geometry_hints` 中增加 fallback 路径：
   - 当 field_x 的 `nearest_coordinate` 搜索返回 `candidate_position=None` 时，
   - 查找 `f64_pair_candidates` 中是否有该 field_x 的候选。
   - 若有且 score 已满足 identity + stable_shape 门槛，则用 f64 pair 坐标作为 position。
2. promotion note 必须包含 `coordinate_source=f64_pair_before_marker`，区别于 `coordinate_source=nearest_coordinate_hint`。
3. 新坐标源的 `SheetObjectGeometryHint.note` 格式：
   `score=N;identity=graphic_nearby;stable_shape=(...);coordinate_source=f64_pair_before_marker`
4. 不修改现有 `nearest_coordinate` 路径的行为。

验收：

```powershell
cargo test --test parse_real_files promoted_object_geometry_hints_explain_promotion_gate -- --nocapture
cargo test --test parse_real_files endpoint_pair_geometry_diagnostics_explain_dwg0201_line_gap -- --nocapture
```

验证 promoted hint count 上升，且 note 包含新坐标源标记。

### Slice 3：Endpoint pair line 产生验证

目标：验证 f64 pair 坐标补齐后，endpoint pair 两端是否能同时 promoted，从而产生 `inferred_lines > 0`。

文件：
- `tests/parse_real_files.rs`

任务：

1. 复用 `endpoint_pair_geometry_diagnostics_explain_dwg0201_line_gap` 诊断测试，期望：
   - `fully_promoted_with_byte_ranges > 0`
   - `inferred_lines > 0`
2. 若仍有部分 endpoint pair 无法闭环，输出 residual gap 统计：
   - 一端有 f64 pair 但另一端没有的数量
   - 两端都没有 f64 pair 候选的数量
3. 更新 `geometry_fixture_inventory_reports_normalized_geometry_counts`：
   - `DWG-0201GP06-01.pid` 的 `inferred_lines` 应从 0 变为 >0
4. 新增 focused regression：`dwg0201_produces_at_least_one_inferred_endpoint_line`

验收：

```powershell
cargo test --locked --test parse_real_files dwg0201_produces_at_least_one_inferred_endpoint_line -- --nocapture
cargo test --locked --test parse_real_files geometry_fixture_inventory -- --nocapture
```

### Slice 4：坐标尺度验证与多 fixture 横向确认

目标：验证 f64 pair 坐标在多个 fixture 上的一致性。

文件：
- `tests/parse_real_files.rs`

任务：

1. 对所有 5 个 fixture 提取 f64 pair candidates，输出：
   - fixture, sheet, field_x_count, f64_pair_candidate_count
   - value range (min_x, max_x, min_y, max_y)
   - 空候选 fixture 的原因说明（无 marker / 无有限 f64 等）
2. 验证同一 fixture 内的 f64 pair 坐标是否在一致尺度范围内。
3. 对比 f64 pair 坐标值域与已有 `SheetCoordinateHint` 值域：
   - 若在同一数量级，记录为 scale-consistent
   - 若差异显著，记录为 scale-divergent，并标注 coordinate_source 的 confidence 限制
4. 结论写入 findings.md。

验收：

```powershell
cargo test --locked --test parse_real_files f64_pair_cross_fixture_scale_consistency -- --nocapture
```

### Slice 5：H7CAD 端到端 line 消费

目标：在 H7CAD 中验证 inferred endpoint line 被真实消费。

前置：Slice 3 确认至少一个 fixture 产生 `inferred_lines > 0`。

文件：
- H7CAD `src/io/pid_import.rs`

任务：

1. 更新 `pid_import_real_sample_geometry_consumes_source_backed_layers`：
   - 若 `DWG-0201GP06-01.pid` 现在有 lines，断言 `PID_GEOM_LINES > 0`
2. 确认 line 的 xdata 包含 `confidence=inferred` 与 `record_kind=endpoint_pair`。
3. 确认 line 的 provenance note 包含 `coordinate_source=f64_pair_before_marker`。

验收：

```powershell
cargo test --locked -j 1 --bin H7CAD pid_import_real_sample_geometry -- --nocapture
```

### Slice 6：回归加固与文档更新

目标：确保新坐标源不破坏现有合同。

任务：

1. 运行全量预提交门禁：
   ```powershell
   cargo build --locked --workspace --all-targets
   cargo test  --locked --workspace --all-targets
   cargo clippy --locked --workspace --all-targets -- -D warnings
   cargo fmt --all -- --check
   cargo rustdoc --lib --locked -- -W missing-docs
   ```
2. 更新 `AGENTS.md`：
   - 记录 f64 pair coordinate source 的存在与限制
   - 更新测试数量
3. 更新 `task_plan.md`：Phase 10 状态
4. 更新 `findings.md`：f64 pair 坐标源的结论与尺度验证结果
5. 更新 `progress.md`：Session 2026-05-09

## 4. 决策

| 决策 | 理由 |
|---|---|
| f64 pair 作为 fallback 而非替代 | 现有 `nearest_coordinate` 路径已证明对部分 field_x 有效；f64 pair 补充覆盖盲区 |
| coordinate_source 标注 | provenance 透明化，下游可区分两种坐标来源的置信度 |
| 不做单位转换 | 当前 f64 值域尚未与已知 CAD 坐标系对齐，先保持原值输出 |
| 先 pid-parse 后 H7CAD | 坐标源验证必须在解析端完成，H7CAD 只消费合同输出 |
| 保持 Inferred confidence | multi-fixture scale consistency 验证通过前不升级为 Decoded |

## 5. 风险

| 风险 | 缓解 |
|---|---|
| f64 pair 在部分 fixture 中可能不是坐标 | 有限值 + 合理范围过滤；multi-fixture 横向验证 |
| 单位转换缺失导致 H7CAD 渲染位置偏移 | H7CAD 端保留 provenance 诊断，source geometry layer 独立于 topology |
| 补齐后 promoted hint count 大幅上升 | promotion count regression 会自动捕获；threshold 不降低 |
| f64 pair 的 marker 在其他 fixture 中形态不同 | Slice 4 横向验证覆盖；不同形态的 fixture 暂不接入 |

## 6. 禁止项

- 禁止移除或降低现有 promotion gate 的任何条件。
- 禁止把 f64 pair 坐标直接标为 `Decoded`。
- 禁止跳过 Slice 4 横向验证直接进入 H7CAD 消费。
- 禁止把 endpoint relationship 语义当作 CAD 坐标证据。
- 禁止在本 Phase 中推进 Text/Symbol promotion。

## 7. 推荐执行顺序

1. **Slice 1** → f64 pair extraction helper（TDD 红绿）
2. **Slice 2** → 接入 promotion gate fallback（TDD 红绿）
3. **Slice 3** → endpoint line 产生验证
4. **Slice 4** → 多 fixture 横向尺度验证
5. **Slice 5** → H7CAD 端到端消费（仅当 Slice 3/4 通过后）
6. **Slice 6** → 全量回归与文档

## 8. 后续 Phase 展望

| 后续 Phase | 触发条件 | 内容 |
|---|---|---|
| Phase 10B | f64 pair scale-consistent 跨 3+ fixture | f64 pair 坐标 confidence 从 Inferred 升级评估 |
| Phase 10C | Text quality gate 有新候选 | Text extraction 改进与 Text promotion gate |
| Phase 10D | Symbol identity 跨 2+ fixture 验证 | Symbol anchor promotion gate |
| Phase 11 | Phase 10 收敛 | Canonical graph integration |
| Phase 12 | DWG fixture 可用 | DWG publish XML gate closure |
