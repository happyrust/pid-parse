# Sheet Object Geometry Mapping Probe Plan

> **Date:** 2026-05-01  
> **Goal:** 填充 `SheetGeometry.object_geometry_hints`，证明 object `field_x` 与 Sheet 源坐标之间的关系，为后续 `Line + Inferred` 提供足够 provenance。  
> **Scope:** `pid-parse` parser/probe/model/tests。H7CAD 只在下一阶段消费结果；本计划不渲染线段。

## 1. Starting Point

已完成：

- `SheetGeometry.object_geometry_hints: Vec<SheetObjectGeometryHint>` 已加入 public DTO，默认空。
- `SheetObjectGeometryHint` 当前字段：
  - `offset`
  - `field_x`
  - `position: Option<SheetCoordinateHintDto>`
  - `graphic_oid: Option<u32>`
  - `note: Option<String>`
- `DWG-0201GP06-01.pid` 的 `/Sheet6` baseline 仍保持：
  - 132 normalized entities
  - 64 inferred points
  - 68 probe-only unknowns
- `endpoint_a` / `endpoint_b` 已确认是 object DA record `field_x`，不是坐标索引。
- 当前 `coordinate_hints` 只是 broad heuristic：前 64 个 plausible aligned `i32, i32` pair，不是解码坐标表。
- `field_x_windows` experimental helper 已能在 Sheet bytes 中定位 object `field_x` hit windows，并收集窗口内 coordinate candidates。
- 真实样本测试已锁定当前 `/Sheet6.object_geometry_hints` 仍为 `0`，同时证明 `field_x_windows` 能找到样本 endpoint ids。

关键约束：

- 不用 topology layout 坐标填充 `SheetObjectGeometryHint.position`。
- 不用 byte-nearest coordinate hint 配 endpoint。
- 没有 byte range provenance 的 mapping 不进入 DTO。

## 2. Target Contract

当 probe 能证明某个 Sheet source record 把 object `field_x` 和 source coordinate 放在同一个稳定结构中时，填充：

```rust
SheetObjectGeometryHint {
    offset,
    field_x,
    position: Some(SheetCoordinateHintDto { offset: position_offset, x, y }),
    graphic_oid,
    note: Some("...why still inferred/probe...")
}
```

首版允许 `graphic_oid = None`，但不允许 `position = None` 的记录驱动线段生成。`position = None` 可以暂存为调查证据，但 `build_normalized_geometry` 不应把它升级成 renderable geometry。

## 3. Phase A: Byte Search Inventory

**Goal:** 找到 `/Sheet6` 中 object `field_x` 值出现的位置，并判断其附近是否存在稳定坐标结构。

**Input ids:**

- 来自 `/Sheet6` endpoint links 的 unique endpoint ids。
- 优先样本：`229`, `326`, `740`, `139`, `433`, `646`, `452`, `440`。

**Implementation sketch:**

1. 新增 probe helper，只在测试或 experimental CLI 中使用：
   - input: `sheet_path`, `sheet_bytes`, `field_xs`
   - output: per-field hit windows
2. 搜索 `field_x.to_le_bytes()` 的所有 byte positions。
3. 对每个 hit 采集固定窗口，例如 `hit-32..hit+96`。
4. 在窗口内提取：
   - nearby plausible coordinate pairs
   - nearby repeated `field_x`
   - potential record discriminator / length / marker bytes
   - potential GraphicOID-like u32
5. 输出只作为 investigation report，不改变 `PidDocument`。

**Acceptance:**

- 能打印或断言每个 sampled `field_x` 的 hit count。
- 文档记录哪些 hit 位于 coordinate hint 区域、endpoint record 区域、其它区域。

### Current sampled windows

`sheet6_field_x_window_probe_finds_sample_endpoint_ids -- --nocapture` currently reports:

| `field_x` | Hit count | Endpoint-signature hits | Sample offsets / windows |
|---:|---:|---:|---|
| `229` | 3 | 1 | `1683 (none, coords=4)`, `1865 (none, coords=3)`, `17330 (endpoint@17314, coords=7)` |
| `326` | 19 | 8 | includes non-endpoint hits `877`, `1064`, `1251`, `1438`, `1673`, `1733`, `2230`, `10334`, `11048`, `11524`, `12178`; endpoint-like hits include `15422`, `17336`, `17560`, `20472`, `23720`, `24280`, `24392`, `25164` |
| `740` | 5 | 3 | non-endpoint hits `11496`, `11676`; endpoint-like hits `13656`, `17442`, `18008` |
| `139` | 4 | 2 | non-endpoint hits `921`, `1074`; endpoint-like hits `17448`, `17554` |

Interpretation:

- The same object `field_x` can appear in multiple regions.
- Some hits are near endpoint-record offsets (`17314..24444`), which proves semantic linkage but not object coordinates.
- Some hits are outside endpoint-record offsets and may be better candidates for object geometry mapping, but still need record-shape validation.
- Nearby coordinate candidates in a window are not enough by themselves; the next step must score whether the coordinate and `field_x` belong to the same source record.
- `field_x_windows` now marks `endpoint_record_start` when a hit is part of an endpoint-record signature. These hits should be excluded from object-position promotion.

### Record-shape scoring v0

Use scoring only to rank investigation candidates. Do not populate `SheetObjectGeometryHint` from score alone.

Reject / down-rank:

| Pattern | Score | Reason |
|---|---:|---|
| Window matches endpoint-record signature (`rel_field_x`, discriminator `0x00000006`, type `0x0002`, delimiter `0x0001`) | `-100` | This is a relationship endpoint reference, not an object coordinate record |
| `field_x` appears only as `endpoint_a` / `endpoint_b` inside endpoint record | `-80` | Confirms semantic endpoint, not geometry position |
| Nearby coordinates are only broad `coordinate_hints` with no repeated record shape | `-20` | Coordinate proximity is not provenance |
| Candidate position comes from topology layout | reject | Not Sheet source geometry |

Positive evidence:

| Pattern | Score | Reason |
|---|---:|---|
| Same fixed delta from `field_x` to `(x, y)` across 2+ object ids | `+40` | Suggests repeatable record layout |
| Candidate `(x, y)` has its own byte range inside the same bounded record | `+20` | Position provenance can be named |
| `field_x` resolves to object graph and appears in `/Sheet6` endpoint links | `+10` | Semantic object identity is known |
| Window contains a stable discriminator / record marker repeated across candidates | `+20` | Helps distinguish record shape from random byte hits |
| Candidate excludes endpoint-record signature | `+10` | Avoids confusing relationship linkage with geometry |

Promotion threshold for future implementation:

```text
score >= 70
and same shape appears for >= 2 distinct object field_x values
and both field_x and position byte ranges are retained in provenance
```

Anything below threshold remains investigation output only.

## 4. Phase B: Mapping Candidate DTO Population

**Goal:** 只有在 pattern 稳定时才填充 `object_geometry_hints`。

**Promotion rule v1:**

一个 candidate 必须同时满足：

- `field_x` 命中位置可复现。
- 同一 bounded record/window 内存在 plausible `(x, y)`。
- position byte range 与 `field_x` hit 的距离在可解释范围内。
- 至少两个不同 object `field_x` 共享相同 record shape。
- 该 object `field_x` 出现在 object graph 或 crossref endpoint links 中。

**Non-promotion:**

- 只在 endpoint record 区域出现的 field_x 不填充 position。
- 只在 coordinate hint 区域出现坐标但没有 field_x 不填充 object mapping。
- 只通过 topology layout 得到位置不填充。

**Files likely touched:**

- `src/parsers/sheet_probe.rs`
- `src/streams/cluster.rs`
- `src/model.rs` only if DTO needs small refinement
- `tests/parse_real_files.rs`

## 5. Phase C: Tests

### Synthetic test

Build a small Sheet byte fixture with:

- one record containing `field_x`
- nearby `x, y`
- unrelated decoy coordinate pair
- unrelated decoy field_x

Assert:

- exactly one `SheetObjectGeometryHint`
- `field_x` matches
- `position.offset` points at the real coordinate pair
- decoys are ignored

### Real fixture soft-skip test

For `DWG-0201GP06-01.pid`:

- parse package
- inspect `/Sheet6.geometry.object_geometry_hints`
- assert count only after stable rule is implemented
- until then, keep an explicit test that current count is `0`, so accidental heuristic promotion is caught

### Regression test

Keep `normalized_geometry_probe_baseline_on_real_fixture` unchanged until `Line + Inferred` is intentionally introduced.

## 6. Phase D: Line Upgrade Gate

Only after mapping DTOs are populated should endpoint records be considered for line geometry.

`Line + Inferred` requires:

- endpoint record has `rel_field_x`, `endpoint_a`, `endpoint_b`
- `endpoint_a` resolves to `SheetObjectGeometryHint.position`
- `endpoint_b` resolves to `SheetObjectGeometryHint.position`
- both positions carry source offsets
- line provenance references:
  - endpoint record byte range
  - source endpoint mapping offset
  - target endpoint mapping offset

If either side is missing:

- keep endpoint as `Unknown + ProbeOnly`
- add warning / skipped reason

## 7. Phase E: H7CAD Consumption

H7CAD should not change until `pid-parse` emits real line entities.

When `PidGraphicKind::Line + Inferred` appears:

- render on `PID_GEOMETRY_LINES`
- keep `PID_GEOMETRY_POINTS` visible as diagnostic anchors
- extend stats:
  - `inferred_line_count`
  - `rendered_line_count`
  - `skipped_endpoint_count`

## 8. Validation Commands

```powershell
cd D:\work\plant-code\cad\pid-parse
cargo test --lib sheet_object_geometry -- --nocapture
cargo test --test parse_real_files normalized_geometry_probe_baseline_on_real_fixture -- --nocapture
cargo test --test parse_real_files sheet6_object_geometry_hints_baseline -- --nocapture

cd D:\work\plant-code\cad\H7CAD-pid-real-geometry-display
cargo test -p H7CAD pid_bundle -- --nocapture
```

## 9. Immediate Checklist

- [x] Add `sheet6_object_geometry_hints_baseline` test that asserts current real fixture count is `0`.
- [x] Add an experimental helper to search Sheet bytes for object `field_x` windows.
- [x] Add real fixture smoke coverage for sampled endpoint ids `229`, `326`, `740`, `139`.
- [x] Record sampled hit windows for `229`, `326`, `740`, `139` in an investigation report.
- [x] Draft record-shape scoring v0.
- [ ] Decide whether any stable record shape exists by applying scoring to sampled windows.
- [ ] If stable, add synthetic test for one mapping record.
- [ ] Populate `object_geometry_hints` only for proven record shapes.
- [ ] Keep endpoint-to-line upgrade blocked until two endpoint ids resolve to source-backed positions.

