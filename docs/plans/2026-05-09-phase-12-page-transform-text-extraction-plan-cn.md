# Phase 12 开发方案：页面坐标变换与 Text 字段提取

> 日期：2026-05-09
> 前置：Phase 10-11 已实现 49 条 inferred lines，f64 坐标确认为 0-1 归一化页面坐标，
> 模板 A2（594×420mm），H7CAD 端到端消费验证通过。
> 原则：先让已有几何在 H7CAD 中正确定位，再扩展新图元类型。

## 0. 当前基线

```
DWG-0201GP06-01.pid:
  inferred_points=117, inferred_lines=49, probe_only=19
  f64 domain: x=[0.082, 0.475], y=[0.000, 0.275]
  template: A2 (594×420mm)
  endpoint pair: 49/59 (83.1%), 4 null endpoints

H7CAD:
  PID_GEOM_POINTS=53, PID_GEOM_LINES=49
  coordinates in 0-1 normalized space
  visual scale: ~0.5 × ~0.3 units (tiny without scaling)
```

## 1. 目标

1. 从 `.pid` 元数据中自动提取页面尺寸，建立归一化坐标 → 物理坐标映射。
2. 让 H7CAD 渲染的 PID 几何与 topology preview 在同一坐标空间。
3. 从 Sheet record 中识别第一批工程文本标签（tag number、line number 等），建立 Text record 识别基础。
4. 保持所有现有 gate 条件和 test 不变。

## 2. 非目标

- 不建立自动坐标系检测（先做最小人工推断）。
- 不实现完整 Text rendering（先识别 record 中的文本字段，不推到 PidGraphicKind::Text）。
- 不推进 Symbol rendering。
- 不改动 Publish XML 管线。

## 3. 开发切片

### Slice 1：页面尺寸提取

目标：从 DrawingMeta 中提取模板名 → 推断页面尺寸。

任务：

1. 分析 `DrawingMeta.tags["Template"]` 的命名约定：
   - `XIONGANA2.pid` → A2（594×420mm）
   - 检查其他 fixture 的 Template 值
   - 建立 template → page_size 映射表
2. 新增 `PidPageDimension { width_mm: f64, height_mm: f64, source: String }` 到 `PidDocument`。
3. 在 `cfb/reader.rs` 解析阶段填充 `page_dimension`。
4. fallback：如果 template 未识别，默认 A3（420×297mm）。

验收：

```powershell
cargo test --locked -j 1 --test parse_real_files page_dimension_extraction -- --nocapture
```

### Slice 2：NormalizedPidGeometry 坐标变换

目标：将 f64 归一化坐标乘以页面尺寸，输出物理坐标。

任务：

1. 在 `build_normalized_geometry` 中接受 `PidPageDimension`。
2. 对 `PidPoint { x, y }` 应用 `x * width_mm, y * height_mm`。
3. 对 endpoint pair line 的 start/end 同样变换。
4. 保留未变换的原始坐标在 provenance note 中。
5. 如果 `page_dimension` 不可用，保持原始 0-1 坐标。

验收：

```powershell
cargo test --locked -j 1 --test parse_real_files normalized_geometry_uses_page_transform -- --nocapture
```

### Slice 3：H7CAD 坐标空间对齐

目标：让 PID 几何与 topology preview 在同一视觉范围。

前置：Slice 1-2。

任务：

1. 比较变换后的坐标范围与 topology preview 的坐标范围。
2. 如果吻合，保持现有 fit 策略。
3. 如果偏移，在 `PidCoordinateContext` 中记录 offset/scale 用于后续修正。
4. 更新 `pid_import_real_sample_geometry` 测试验证新坐标范围。

验收：

```powershell
cargo test --locked -j 1 --bin H7CAD pid_import -- --nocapture
```

### Slice 4：Sheet Record Text 字段识别

目标：从已知的 f64 record shape 中识别文本字段引用。

任务：

1. 对已 promoted 的 record shape（5E/FA/CE marker 后续字节），分析是否包含：
   - DA trailer 中的 tag number 引用
   - 固定偏移处的 text length + text bytes
   - UTF-16LE 编码的标签文本
2. 对 `/Sheet6` 的 promoted records，dump marker 后 64 字节的结构：
   - 识别重复的 u32 字段模式
   - 标记可能的 text offset / text length 位置
3. 如果发现 text field，新增 `SheetRecordTextField` probe DTO。
4. 不推进到 PidGraphicKind::Text（先保持 probe）。

验收：

```powershell
cargo test --locked -j 1 --test parse_real_files sheet_record_text_field_investigation -- --nocapture
```

### Slice 5：质量回归与文档

任务：

1. 运行全量预提交门禁。
2. 更新 CHANGELOG、task_plan.md、findings.md、progress.md。
3. 提交并推送。

## 4. 决策

| 决策 | 理由 |
|---|---|
| 模板名推断页面尺寸 | 最小实现，不需要额外解析 |
| 坐标变换在 geometry 层而非 renderer 层 | 保持 H7CAD 消费的简洁性 |
| Text 先做 investigation 不做 promotion | 当前 text quality 仍为 0，需要先证明 record 中有真实文本 |
| 默认 fallback A3 | SmartPlant 常用纸张尺寸 |

## 5. 风险

| 风险 | 缓解 |
|---|---|
| 模板名不包含纸张信息 | fallback + 在 DrawingMeta.tags 中搜索其他尺寸线索 |
| f64 坐标不是简单的 归一化×尺寸 映射 | 保留原始坐标，变换后 provenance 记录 transform_source |
| H7CAD 坐标系与物理 mm 不一致 | 保持独立 layer，topology preview 不受影响 |
| Record 中没有可识别的文本字段 | 保持 text_over_threshold=0 基线，不强推 |

## 6. 推荐执行顺序

1. **Slice 1** → 页面尺寸提取
2. **Slice 2** → 坐标变换
3. **Slice 3** → H7CAD 对齐
4. **Slice 4** → Text 字段识别
5. **Slice 5** → 回归与文档
