# H7CAD PID 真实几何 PR 执行清单

> 日期：2026-05-01  
> 目标：把当前混合工作树拆成可 review、可验证、互不污染的 PR。  
> 原则：不把 endpoint / topology 当作 CAD line；`object_geometry_hints` 在 source-proven mapping 前必须保持为空。

## 0. 当前结论

- H7CAD 已能显示 PID 中的 inferred points。
- `/Sheet6` 当前没有可 promotion 的 object-coordinate mapping。
- `GraphicIdentityNearby` 有真实信号，但不与可评分 feature candidate 相交：
  - identity report：`same_object=11`、`wrong_object=414`
  - identity scoring：`identity_supported=0`、`max_score=45`、`over_threshold=0`
- `Text placement` 已完成 Phase A/B investigation：
  - text window report：`text_runs=9`、`candidates=121`、`same_chunk=25`
  - text scoring：`text_quality_passed=0`、`max_score=-50`、`over_threshold=0`
- 当前仍不能生成 `Line + Inferred`。
- 当前仍不能生成 `Text + Inferred`。

## 1. 执行前硬约束

- 不做 `git reset --hard` / `git checkout --` 之类破坏性操作。
- 不自动提交；只有用户明确要求后才 commit / branch / PR。
- 对 `tests/parse_real_files.rs` 和 `src/parsers/sheet_probe.rs` 必须按 hunk 拆，不要一次塞入一个 PR。
- `src/app/update.rs` 只取两个 PR2 功能 hunk：
  - PID open message 增加 inferred geometry points 统计
  - PID tab 用 `fit_layers_matching` 优先 fit 主绘图层

## 2. PR1：Normalized Geometry Contract

### 范围

- `src/geometry.rs`
- `src/lib.rs`
- `src/model.rs` 中基础 `SheetGeometry` / `SheetText` / `SheetEndpoint` / `SheetCoordinateHintDto`
- `src/schema.rs` 中 normalized geometry schema
- `src/bin/pid_inspect.rs`
- `src/inspect/report.rs`
- `tests/inspect_cli.rs`
- `tests/parse_real_files.rs` 中 `normalized_geometry_probe_baseline_on_real_fixture`

### 不包含

- `SheetObjectGeometryHint`
- field-x window investigation
- GraphicIdentityNearby
- H7CAD UI / rendering

### 验证

```powershell
cargo test --test inspect_cli -- --nocapture
cargo test --lib schema -- --nocapture
cargo test --test parse_real_files normalized_geometry_probe_baseline_on_real_fixture -- --nocapture
```

## 3. PR2：H7CAD Inferred Point Rendering

### 范围

- `H7CAD-pid-real-geometry-display/src/io/pid_import.rs`
- `H7CAD-pid-real-geometry-display/src/io/pid_screenshot.rs`
- `H7CAD-pid-real-geometry-display/src/io/svg_export.rs`
- `H7CAD-pid-real-geometry-display/src/app/update.rs` 的最小功能 hunk
- H7CAD 相关计划文件

### 不包含

- endpoint line rendering
- Unknown / ProbeOnly rendering
- `src/app/update.rs` 大范围 rustfmt churn

### 验证

```powershell
cargo test -p H7CAD pid_bundle -- --nocapture
```

## 4. PR3：Sheet6 Evidence Guardrails

### 范围

- `SheetObjectGeometryHint` DTO
- `SheetGeometry.object_geometry_hints`
- schema 中 `SheetObjectGeometryHint` 暴露测试
- `sheet6_object_geometry_hints_baseline_is_empty_until_mapping_is_proven`
- Sheet6 evidence inventory / mapping probe plan

### 不包含

- field-x scoring helper
- identity scanner
- H7CAD 渲染变化

### 验证

```powershell
cargo test --test parse_real_files sheet6_object_geometry_hints_baseline_is_empty_until_mapping_is_proven -- --nocapture
cargo test --lib schema -- --nocapture
```

## 5. PR4：Field-X Window / Feature Investigation

### 范围

- `field_x_windows`
- endpoint-record signature marking
- `score_field_x_windows`
- repeated-delta scoring
- `field_x_window_features`
- stable chunk-shape / marker support
- coordinate-quality filters
- PR4 对应真实样本 report tests

### 不包含

- GraphicIdentityNearby identity index / scanner / scoring
- `SheetObjectGeometryHint` population
- H7CAD behavior change

### 验证

```powershell
cargo test --lib parsers::sheet_probe -- --nocapture
cargo test --test parse_real_files sheet6_field_x_window_probe_finds_sample_endpoint_ids -- --nocapture
cargo test --test parse_real_files sheet6_field_x_window_scoring_reports_non_endpoint_candidates -- --nocapture
cargo test --test parse_real_files sheet6_field_x_window_features_report_chunk_shapes -- --nocapture
```

预期关键值：

- `max_score=45`
- `promotable=0`
- `top_feature_scores=[]`

## 6. PR5：GraphicIdentityNearby Investigation

### 范围

- `SheetObjectIdentity`
- `SheetIdentityIndex`
- `sheet_identity_index_from_trailers`
- `SheetFieldXWindowIdentity*`
- `field_x_window_identities`
- `GraphicIdentityNearby` scoring reason
- `score_field_x_window_features_with_identities`
- `/Sheet6` identity report / identity scoring report

### 不包含

- `SheetObjectGeometryHint` population
- endpoint line rendering
- H7CAD behavior change

### 验证

```powershell
cargo test --lib parsers::sheet_probe -- --nocapture
cargo test --test parse_real_files sheet6_field_x_window_identity_report -- --nocapture
cargo test --test parse_real_files sheet6_graphic_identity_scoring_keeps_object_hints_empty_until_proven -- --nocapture
cargo test --test parse_real_files all_sheets_graphic_identity_scoring_report_keeps_object_hints_empty -- --nocapture
```

预期关键值：

- identity report：`same_object=11`、`wrong_object=414`
- identity scoring：`identity_supported=0`、`max_score=45`、`over_threshold=0`
- all-Sheet identity scoring：`sheets=1`、`identity_supported=0`、`over_threshold=0`
- `object_geometry_hints=0`

## 7. PR6：Text Placement Investigation

### 范围

- `SheetTextWindowCandidate`
- `SheetTextWindowScore`
- `SheetTextWindowScoreReason`
- `sheet_text_window_candidates`
- `is_high_quality_text_candidate`
- `score_sheet_text_window_candidates`
- `/Sheet6` text window report / text scoring report
- Text placement 证据路线计划与图

### 不包含

- `PidGraphicKind::Text` promotion
- H7CAD text rendering
- endpoint line rendering
- `SheetObjectGeometryHint` population

### 验证

```powershell
cargo test --lib parsers::sheet_probe -- --nocapture
cargo test --test parse_real_files sheet6_text_window_report_keeps_text_probe_only_until_position_is_proven -- --nocapture
```

预期关键值：

- text report：`text_runs=9`、`candidates=121`、`same_chunk=25`、`coordinate_quality_passed=2`
- text scoring：`text_quality_passed=0`、`max_score=-50`、`over_threshold=0`
- normalized geometry 仍无 `PidGraphicKind::Text`
- `/Sheet6` text 保持 `ProbeOnly Unknown`

## 8. 推荐拆分顺序

1. 先拆 PR1，让 `pid-parse` contract 稳定。
2. 再拆 PR2，让 H7CAD 消费稳定 contract。
3. 再拆 PR3，锁住 no-promotion guardrail。
4. 再拆 PR4，提交 field-x investigation 工具链。
5. 再拆 PR5，提交 GraphicIdentityNearby 证据路线。
6. 最后拆 PR6，提交 Text placement investigation。

## 9. 完成判定

- 每个 PR 都能独立解释“为什么不渲染 line”。
- 每个 PR 都有 focused validation。
- 所有 investigation PR 都只增加 evidence/report，不改变 H7CAD 用户可见渲染行为。
- 用户明确要求后再进入 commit / branch / PR 创建阶段。

