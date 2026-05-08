# Phase 11 开发方案：坐标系验证、Text 渲染与几何质量加固

> 日期：2026-05-09
> 前置：Phase 10/10B 已实现 f64 pair/triple 坐标源，3/5 fixture 产生 inferred lines（合计 40 条），H7CAD 已消费 34 条 line。
> 原则：先验证已产出坐标的物理正确性，再扩展新图元类型；quality gate 优先于 quantity expansion。

## 0. 当前基线

```
DWG-0201GP06-01.pid:
  inferred_points=106, inferred_lines=34, probe_only=34
  endpoint_pairs=59, fully_promoted=34, only_a=11, only_b=13, neither=1

DWG-0202GP06-01.pid:
  inferred_points=74, inferred_lines=3, probe_only=42

H7CAD:
  PID_GEOM_POINTS=42, PID_GEOM_LINES=34
  skipped_probe_only=34, skipped_broad_coordinate_hints=64
```

关键缺口：
- f64 坐标值域为 0..1 范围（可能是归一化坐标），与实际 CAD 页面坐标（通常 mm/inch 级）的映射尚未验证。
- Text/Symbol 仍无 promotion（`text_over_threshold=0`）。
- 剩余 24/59 endpoint pairs 一端或两端缺 promoted position。
- probe_only 实体（34+42）仍需分析是否包含可提取信息。

## 1. 目标

1. 验证 f64 坐标的物理意义：确定其与页面坐标的关系（归一化 0-1 / 画面分数 / 其他）。
2. 建立坐标 → 页面映射的初步模型，使 H7CAD 渲染位置与真实图纸一致。
3. 推进 Text placement 到可 promotion 状态，让至少一个 fixture 产生 `PidGraphicKind::Text`。
4. 加固剩余 endpoint pair 覆盖（24/59 → 目标 40+/59）。
5. 保持所有现有 gate 条件不变，新增只做 additive 扩展。

## 2. 非目标

- 不推进 Symbol rendering（依赖 JSite symbol path 交叉验证，独立 phase）。
- 不替换已有 i32 coordinate hint 管线。
- 不实现坐标系自动检测（先做最小人工配置）。
- 不改动 Publish XML 管线。

## 3. 开发切片

### Slice 1：f64 坐标值域分析与页面映射研究

目标：确定 f64 pair/triple 坐标的物理含义。

任务：

1. 对 `DWG-0201GP06-01.pid` 提取所有 promoted f64 positions，统计值域：
   - min_x, max_x, min_y, max_y
   - 是否在 0-1 范围（归一化坐标假设）
   - 值间距分布（等距 / 对数 / 随机）
2. 对比 Sheet metadata 中是否有页面尺寸信息：
   - `TaggedTxtData/Drawing` XML 中的 page/scale 属性
   - `DocVersion2/3` 中的尺寸字段
   - `AppObject` 中的 drawing size
3. 对比已有 i32 coordinate hints 的值域（1000-100000 级）与 f64 值域（0-1 级）的比例关系。
4. 如果找到 page width/height，尝试 `f64 × page_dimension` 映射并检查是否与 i32 hints 对齐。
5. 输出结论到 findings.md：
   - 坐标系类型（归一化 / 其他）
   - 映射公式（如有）
   - 置信度评估

验收：

```powershell
cargo test --locked -j 1 --test parse_real_files f64_coordinate_domain_analysis -- --nocapture
```

### Slice 2：剩余 endpoint pair 覆盖扩展

目标：将 `DWG-0201GP06-01.pid` 的 `fully_promoted` 从 34/59 提高到 40+/59。

任务：

1. 分析 `only_a=11` 的 endpoint_a field_xs：
   - 哪些有 f64 pair/triple 但 support 不足 3？
   - 哪些完全没有任何 f64 marker pattern？
   - 哪些是 relay/junction 类对象（可能不需要坐标）？
2. 分析 `only_b=13` 的 endpoint_b field_xs（同上逻辑）。
3. 分析 `neither=1` 的那一对：两端都是什么类型对象？
4. 如果发现新的 marker pattern（第三种），按 TDD 新增 extraction。
5. 如果部分 field_x 本质上不应有坐标（非图形对象），标记为 `non_graphical_skip` 而非缺口。

验收：

```powershell
cargo test --locked -j 1 --test parse_real_files endpoint_pair_coverage_analysis -- --nocapture
```

### Slice 3：Text placement gate 重新评估

目标：在 Phase 10 的 f64 坐标成功经验上，重新评估 Text 证据路线。

任务：

1. 重跑 text quality scoring，检查 Phase 7/8 时 `text_over_threshold=0` 的原因是否仍然成立。
2. 对 `/Sheet6` 的 top text candidates dump：
   - text 内容是否仍是二进制误识别？
   - coordinate 是否能与新 f64 坐标源对齐？
3. 如果 text extraction 质量仍不足，调查 Sheet stream 中是否有新的 text record pattern。
4. 如果找到可行 text evidence，按 TDD 新增 `SheetTextGeometryHint`。
5. 设计 Text promotion gate：
   - text value 非二进制/非空
   - insertion point 有 source-backed 坐标
   - text 在至少 2 个 fixture 中出现相似 record shape

验收：

```powershell
cargo test --locked -j 1 --test parse_real_files text_placement_reevaluation -- --nocapture
```

### Slice 4：H7CAD 坐标映射集成

目标：让 H7CAD 渲染的 point/line 位置与真实图纸布局匹配。

前置：Slice 1 的坐标映射模型。

文件：
- `D:\work\plant-code\cad\H7CAD\src\io\pid_import.rs`

任务：

1. 如果 Slice 1 确认 f64 为归一化坐标：
   - 在 `PidCoordinateContext` 中新增 `page_width` / `page_height`
   - `add_geometry_entities_from` 在渲染时乘以页面尺寸
2. 如果 f64 就是物理坐标（无映射需要）：
   - 确认 H7CAD 的坐标系与 f64 一致（Y 轴方向、原点位置）
3. 验证 inferred lines 的渲染位置是否与 topology preview 的布局大致吻合。
4. 如果坐标映射成功，考虑将 topology preview 的 source-backed geometry 优先级从 "fit" 调整为 "主渲染"。

验收：

```powershell
cargo test --locked -j 1 --bin H7CAD pid_import -- --nocapture
```

### Slice 5：质量回归与文档

任务：

1. 运行全量预提交门禁。
2. 更新 AGENTS.md：记录 f64 坐标映射模型。
3. 更新 task_plan.md：Phase 11 状态。
4. 更新 findings.md：坐标分析结论、text gate 评估结论。
5. 更新 progress.md：Session 2026-05-09 续篇。

## 4. 决策

| 决策 | 理由 |
|---|---|
| 先验证坐标再扩展图元 | 34 条 line 的物理位置正确性是 Text/Symbol 的前提 |
| 优先 text 而非 symbol | text 有直接 value + insertion point 对偶关系，比 symbol 更容易证明 |
| 不修改已有 promotion gate | Phase 10 的 gate 已验证有效，只做 additive 扩展 |
| 坐标映射先手动配置后自动检测 | 归一化坐标的 page 尺寸可能不在 Sheet stream 内 |

## 5. 风险

| 风险 | 缓解 |
|---|---|
| f64 不是归一化坐标，是其他单位 | 多 fixture 横向对比；与 i32 coordinate hints 交叉验证 |
| 页面尺寸信息不在解析范围内 | 降级为固定比例渲染，标注 `scale=unknown` |
| Text extraction 质量不足 | 保持 `text_over_threshold=0` 基线，不降低 quality gate |
| H7CAD 渲染位置偏移 | 保留 topology fallback，source-backed geometry 作为独立可切换 layer |

## 6. 推荐执行顺序

1. **Slice 1** → 坐标值域分析
2. **Slice 2** → 剩余 endpoint pair 覆盖
3. **Slice 3** → Text gate 重新评估
4. **Slice 4** → H7CAD 坐标映射
5. **Slice 5** → 质量回归与文档

## 7. 后续 Phase 展望

| Phase | 触发 | 内容 |
|---|---|---|
| Phase 11B | Slice 1 确认归一化坐标 | 自动 page dimension extraction |
| Phase 11C | Slice 3 发现可行 text evidence | Text promotion gate |
| Phase 12 | Phase 11 收敛 | Symbol anchor promotion |
| Phase 13 | Phase 12 收敛 | Canonical graph integration |
