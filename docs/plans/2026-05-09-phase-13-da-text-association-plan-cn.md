# Phase 13 开发方案：DA Trailer Text 关联与 Tag Label Rendering

> 日期：2026-05-09
> 前置：Phase 12 证明 Sheet geometric record body 不含文本字段；DA record trailers 中存储了
> object 的 tag number / line number 等属性值；f64 坐标已投射到 67 个 promoted objects。
> 原则：利用已有 DA record → object field_x → promoted position 链路，为 promoted objects 关联文本标签。

## 0. 当前基线

- 67 promoted objects 有 f64 坐标。
- DA record trailers 包含 object 属性（tag number、name、ItemTag 等）。
- `DynamicAttributeRecord` 包含 `trailer_offset`、`record_id`、`field_x`。
- Sheet record body 无文本，需从 DA 侧关联。

## 1. 开发切片

### Slice 1：DA Record → Promoted Object Tag Number 关联
- 对每个 promoted `SheetObjectGeometryHint`，通过 `field_x` 查找 DA record trailer。
- 从 trailer 提取 `ItemTag` / `Name` / 最相关的文本属性值。
- 输出 investigation dump：`field_x → tag_text → position`。

### Slice 2：PidGraphicKind::Text 最小实现
- 新增 `SheetTextGeometryHint` 到 model。
- 在 `build_normalized_geometry` 中为每个有 tag text 的 promoted object 生成 Text entity。
- Text insertion point = promoted object 的 position（带小 offset 避免重叠）。

### Slice 3：Text Quality Gate
- 非空、非二进制、ASCII 可打印。
- 至少在 1 个 fixture 中关联成功。
- H7CAD `PID_GEOM_TEXT` layer 消费。

### Slice 4：H7CAD Text Rendering
- `add_geometry_entities_from` 处理 `PidGraphicKind::Text`。
- 渲染到 `PID_GEOM_TEXT` layer。

### Slice 5：回归与文档

## 2. 推荐执行顺序

1. Slice 1 → 2 → 3 → 4 → 5
