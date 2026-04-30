# Text Placement 证据路线计划

> 日期：2026-05-01  
> 目标：在不渲染 endpoint line 的前提下，尝试把已解码文本从 `ProbeOnly Unknown` 推进到 `Text + Inferred`。  
> 结论先行：文本比线段风险更低，因为 `PidGraphicKind::Text` 已存在，当前缺的是 text run 与 insertion coordinate 的 source-backed 关联。

## 1. 当前状态

已存在能力：

- `sheet_probe.rs` 能扫描 `SheetTextRun`，包含 offset、encoding、value。
- `streams::cluster` 会把 `SheetTextRun` 转成 `SheetText`。
- `geometry.rs` 已定义 `PidGraphicKind::Text { insertion, value, height, rotation }`。
- 目前 `sheet.extracted_texts` 只能导出为：
  - `PidGraphicKind::Unknown`
  - `PidGeometryConfidence::ProbeOnly`
  - note: `text position is not decoded yet`

不可做的事：

- 不从 endpoint / topology 推断文本坐标。
- 不给所有 text run 随机匹配最近 coordinate hint。
- 不把低质量结构坐标当 insertion point。

## 2. Promotion Gate

只有同时满足以下条件，才能生成 `Text + Inferred`：

1. 文本 run 有稳定 byte range。
2. 同一 Sheet chunk 内存在通过 `is_high_quality_coordinate_candidate` 的坐标对。
3. 坐标在 text run 附近窗口内，且不是 endpoint id / packed field / 256-aligned structural value。
4. 同一种 record-shape 在多个 text run 上重复出现，或存在 text-specific marker。
5. entity provenance 同时记录：
   - stream path
   - text byte range
   - coordinate byte range
   - scoring reason

未满足时仍保持 `ProbeOnly Unknown`。

## 3. Phase A：Text Window Report

状态：已完成最小 investigation helper 与 `/Sheet6` report。

新增 investigation-only report，不改变 DTO：

- 输入：`SheetTextRun` + `SheetCoordinateHint` + Sheet chunks。
- 输出候选窗口：
  - `text_offset`
  - `text_value`
  - `text_encoding`
  - `candidate_coordinate_offset`
  - `candidate_x/y`
  - `same_chunk`
  - `byte_distance`
  - `quality_passed`

测试：

```powershell
cargo test --test parse_real_files sheet6_text_window_report -- --nocapture
```

验收：

- 能输出 text runs 与附近 coordinate candidates 的数量。
- 不改变 `NormalizedPidGeometry` baseline。

当前 `/Sheet6` 结果：

- `text_runs=9`
- `coordinates=64`
- `candidates=121`
- `same_chunk=25`
- `quality_passed=2`
- normalized geometry 仍无 `PidGraphicKind::Text`
- `/Sheet6` text 仍保持 `ProbeOnly Unknown`

关键限制：

- 当前 top text run 多为疑似二进制误识别的 CJK/韩文字符串。
- Phase B 必须先加入 text quality filter，避免把二进制 payload 当作标签文本。

## 4. Phase B：Text Record Shape Scoring

状态：已完成第一版 text-quality filter 与 investigation-only scoring。

为 text window 增加 scoring，但仍不 promotion：

评分信号：

- `SameChunk`: text 与 coordinate 在同一 chunk。
- `NearbyCoordinate`: 坐标距离 text run 在阈值内。
- `HighQualityCoordinate`: 坐标通过现有 quality predicate。
- `RepeatedShape`: 多个 text run 复用相同相对布局。
- `TextSpecificMarker`: 附近有非结构常量 marker。

拒绝信号：

- 文本值像随机二进制误识别，而不是工程标签 / 标注。
- 坐标值像 object id / endpoint id。
- 坐标值过小且成对出现为 id-like。
- 坐标值 256-aligned。
- 同一坐标被大量 text run 共享。

验收：

- report 能排序 top candidates。
- 阈值暂定 `score >= 70`，但只进入 report，不进入 DTO。
- 若 top candidates 看起来是结构值，必须记录 findings，不降低阈值。

当前 `/Sheet6` 结果：

- `text_quality_passed=0`
- `max_score=-50`
- `over_threshold=0`
- normalized geometry 仍无 `PidGraphicKind::Text`

关键结论：

- `" 060101럀"` 这类“数字 + Hangul 尾字”的 text run 不是可靠工程标签。
- 第一版 text-quality filter 要求至少有 ASCII tag 信号，并拒绝 Hangul 等当前误识别特征。
- 当前 `/Sheet6` 没有可 promotion 的 Text placement candidate。

## 5. Phase C：DTO Promotion 试验门

只有 Phase B 真实样本显示稳定、可解释、可重复的 text insertion pattern 后，才允许新增 DTO：

- `SheetTextGeometryHint`
  - `text_offset`
  - `coordinate_offset`
  - `value`
  - `x`
  - `y`
  - `score`
  - `reasons`

然后在 `geometry.rs` 中把它映射为：

- `PidGraphicKind::Text`
- `PidGeometryConfidence::Inferred`
- 默认 height / rotation 必须显式标注为 fallback：
  - `height=1.0`
  - `rotation=0.0`

H7CAD 渲染必须单独 PR，并先用 synthetic fixture 覆盖。

## 6. 推荐 PR 边界

建议不要混入当前 PR1-PR5，另开 PR6：

- PR6：Text Placement Investigation
  - Phase A/B report helpers
  - real fixture report tests
  - no DTO promotion
  - no H7CAD behavior change

若 Phase B 找到强证据，再拆：

- PR7：Text Geometry Contract
- PR8：H7CAD Text Rendering

## 7. 当前下一步

1. 添加 `sheet_text_window_candidates` helper。
2. 添加 synthetic test，构造 text run 与同 chunk coordinate。
3. 添加 `/Sheet6` real report。
4. 只记录数量和 top candidates，不 promotion。

