# PID 几何 Promotion 与渲染升级开发方案

> 日期：2026-05-02
> 基线：Promotion gate 首次突破，DWG-0201GP06-01 Sheet6 出现 5 个 promotable candidates
> 目标：从 promotable candidates 到 H7CAD 可视 Line/Text/Symbol 层

## 0. Gate 突破现状

```text
promotable           = 5
identity_supported   = 44
identity_over_thresh = 28
max_identity_score   = 105
record_shape_classes = 328
text_over_threshold  = 0
```

突破来自两个关键修正：

1. **Identity 匹配放宽**：`identity_supports_score` 从要求 offset 精确匹配改为 field_x 级别匹配。根因：同一 field_x 在 Sheet 流中出现多次，identity 在 endpoint 窗口找到但高分在别处。
2. **StableChunkShape 阈值降低**：support 从 >=3 降为 >=2。cross-fixture aggregate 已验证 shapes（support=4），三叉 gate 保证安全。

## 1. 开发阶段

### Phase 8C：SheetObjectGeometryHint 填充

**目标**：把 5 个 promotable candidates 的坐标写入 `SheetObjectGeometryHint`。

**任务**：

1. 新增 `populate_object_geometry_hints` 函数：
   - 输入：`scores`、`features`、`identities`、threshold
   - 调用 `summarize_object_geometry_promotion_gate` 筛选 promotable
   - 为每个 promotable candidate 生成 `SheetObjectGeometryHint`
   - 包含 provenance：field_x、offset、score、identity kind、shape class

2. 在 `probe_sheet_stream` 或上层调用链中集成：
   - 当 promotion gate 满足时填充 `SheetGeometry.object_geometry_hints`
   - 保持现有 `assert_eq!(hint_count, 0)` 的 fixture 仍通过
   - 仅对 DWG-0201GP06-01 Sheet6 允许 `hint_count > 0`

3. 新增 regression test：
   - `sheet6_promotable_candidates_populate_geometry_hints`
   - 断言 hint 数量、field_x 集合、坐标范围合理
   - 断言每个 hint 都有完整 provenance

**验收**：

```powershell
cargo test --test parse_real_files geometry_hint -- --nocapture
cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
```

### Phase 9A：NormalizedPidGeometry 投影

**目标**：将 `SheetObjectGeometryHint` 转为 `PidGraphicEntity` 实体。

**任务**：

1. 在 `geometry.rs` 中扩展 `build_normalized_geometry`：
   - 遍历 `sheet_streams[].geometry.object_geometry_hints`
   - 为有坐标的 hint 生成 `PidGraphicKind::InferredPoint`
   - 保留 provenance：source stream path、offset、confidence

2. 对有坐标配对的 hints 尝试 `PidGraphicKind::Line`：
   - 同一 record shape class 内的 hints 间连线
   - confidence = `Inferred`，不是 `Decoded`
   - 来源标注为 `sheet_geometry_hint`

3. Schema 更新：
   - `PidGraphicProvenance` 增加 `SheetGeometryHint` 变体
   - schema snapshot 测试更新

**验收**：

```powershell
cargo test --lib geometry schema -- --nocapture
cargo test --test parse_real_files normalized_geometry -- --nocapture
```

### Phase 9B：H7CAD 渲染路径升级

**目标**：H7CAD 优先显示 promoted geometry。

**任务**：

1. `pid_import.rs` 新增 `pid_geometry_to_native_doc`：
   - `PID_GEOM_POINTS` 层：promoted inferred points
   - `PID_GEOM_LINES` 层：promoted inferred lines
   - `PID_GEOM_TEXT` 层：（Phase 9C 填充）
   - `PID_GEOM_SYMBOLS` 层：（Phase 9C 填充）

2. `open_pid` 策略切换：
   - geometry 非空 → 真实几何优先
   - geometry 空 → fallback 到拓扑预览

3. Fit 逻辑：
   - 优先 fit `PID_GEOM_*` 层
   - 诊断层不影响主图范围

**验收**：

```powershell
cargo test -p H7CAD --bin H7CAD pid_import -- --nocapture
cargo check --locked --workspace --all-targets
```

### Phase 9C：Text 与 Symbol 升级

**目标**：Text 和 Symbol 进入显示链路。

**前置条件**：`text_over_threshold > 0` 或独立的 text quality gate 通过。

**任务**：

1. Text：
   - 降低 text candidate 的二进制误识别率
   - CJK/Hangul 片段质量过滤
   - 通过后生成 `PidGraphicKind::Text`

2. Symbol：
   - JSite symbol_path 与 object identity 交叉验证
   - 生成 `PidGraphicKind::SymbolInstance`（placeholder）
   - H7CAD 用 named block 渲染

## 2. 风险与禁止项

| 风险 | 影响 | 缓解 |
|---|---|---|
| Promotable candidates 含误判 | 画出错误图形 | 三叉 gate + provenance 可追溯 |
| StableChunkShape support=2 不够稳定 | 形状匹配偶然 | cross-fixture aggregate 验证 |
| Identity 放宽导致错误 boost | 无关对象获得高分 | resolves_to_same_object 仍必须为 true |
| Text 二进制误识别 | 渲染乱码 | text_over_threshold gate 独立保护 |

**禁止**：

- 禁止降低 promotion threshold（score >= 70）
- 禁止跳过 identity/shape 双重验证
- 禁止把 relationship endpoint 渲染为 CAD line
- 禁止把 Inferred 实体标记为 Decoded

## 3. PR 拆分建议

| PR | 内容 | 依赖 |
|---|---|---|
| PR-A | `populate_object_geometry_hints` + regression tests | 当前 main |
| PR-B | `NormalizedPidGeometry` 投影扩展 | PR-A |
| PR-C | H7CAD `pid_geometry_to_native_doc` + 层切换 | PR-B |
| PR-D | Text quality gate + Symbol placeholder | PR-B |
