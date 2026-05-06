# Phase 9A Fixture Registry 扩展执行方案

> 日期：2026-05-06
> 背景：Phase 9A 已完成 registry、availability summary、report line；Phase 9C 已补 promotion provenance 与 normalized projection source note 回归。下一步推荐方案进入 fixture registry 扩展，但当前本地仅有 5 个 `.pid` fixture，距离 8-12 个目标仍缺外部真实样本。

## 1. 当前结论

已完成：

- `geometry_fixture_cases()` 建立显式 registry。
- `geometry_fixture_availability_summary()` 输出 `registered`、`target_min_available`、`available`、`missing`。
- `geometry_fixture_availability_report_line()` 已接入 geometry evidence inventory 输出。
- promoted `SheetObjectGeometryHint` note 已包含 `score`、`identity`、`stable_shape` 证据。
- normalized geometry projection 已用回归锁定 `PidGraphicProvenance.note` 保留 promoted hint source note。

当前阻塞：

- registry 目标是 8-12 个真实 PID fixture。
- 本地当前 registry 只有 5 个 fixture，且都已登记。
- 没有新增真实 `.pid` 样本时，不应伪造 registry 扩容，也不应把目标下限降到 5。

## 2. 推荐方案

推荐继续保持“真实 fixture 优先、缺口显式暴露、parser 不混入 registry 扩展”的策略。

执行顺序：

1. 获取 3-7 个额外真实 `.pid` fixture。
2. 先登记为 candidate fixture，并记录来源、模板、语言、Sheet 数量、预期用途。
3. 运行 availability summary，确认 `registered >= 8`。
4. 对每个新增 fixture 运行 geometry evidence inventory。
5. 只对稳定出现的 promotion/text/symbol 证据新增 focused regression。
6. 更新 docs、progress、findings，并提交本轮基线。

## 3. Fixture 选择标准

优先选择：

- 不同模板或项目来源的 `.pid` 文件。
- 包含多个 Sheet stream。
- 包含中文/英文/符号混合标注。
- 有较多 Dynamic Attributes 与 cross-reference endpoint links。
- 可以本地长期保存或脱敏后提交。

暂不选择：

- 来源不明或不能记录来源的私有文件。
- 需要外部服务或额外数据库才能解析的文件。
- 会强迫 parser 大改才能 soft-skip 的样本。
- 与 Sheet geometry / object-coordinate evidence 无关的样本。

## 4. Registry 元数据建议

新增 fixture 时，为每个样本记录：

| 字段 | 说明 |
|---|---|
| `path` | `test-file/...` 下的相对路径 |
| `category` | 模板或来源类别，例如 `dwg`、`publish_a01`、`non_ascii` |
| `source_note` | 样本来源与是否可提交 |
| `expected_available` | 当前环境是否期望存在 |
| `known_gap` | 如果不可用，记录缺失原因 |

当前 `GeometryFixtureCase` 只有 `path` 和 `category`。如果新增样本来源差异变复杂，再扩字段；否则先保持结构小。

## 5. TDD 切片

### 切片 A：新增 fixture candidate registry

红测：

- `geometry_fixture_registry_documents_phase9a_targets` 断言 `fixtures.len() >= 8`。
- 若样本不可提交，则新增测试只断言 `missing` 可见，不强制 available。

绿测：

- 添加真实 fixture case。
- availability summary 的 `registered` 达到目标范围。

### 切片 B：新增 per-fixture evidence guard

红测：

- 对新增 fixture 输出 per-fixture / per-sheet evidence summary。
- 缺 cross-reference / dynamic attributes / Sheet stream 时必须输出可操作 skip reason。

绿测：

- 复用现有 inventory 输出路径。
- 不把 skip 当成失败，除非 fixture 被标记为 hard gate。

### 切片 C：按证据新增 focused regression

红测：

- 如果新增 fixture 出现新的 `identity_over_threshold`、`text_over_threshold` 或 stable shape，先写 focused regression 锁定证据。

绿测：

- 只 promotion 通过 gate 的对象。
- 不把 endpoint relationship 误当 CAD line geometry。

## 6. 验证命令

建议顺序：

```powershell
cargo test --test parse_real_files geometry_fixture_registry_documents_phase9a_targets -- --nocapture
cargo test --test parse_real_files geometry_fixture_availability_summary_tracks_target_gap -- --nocapture
cargo test --test parse_real_files geometry_fixture_availability_report_line_is_human_readable -- --nocapture
cargo test --test parse_real_files available_pid_fixtures_geometry_evidence_inventory_stays_probe_only -- --nocapture
cargo test --test parse_real_files promoted_object_geometry_hints_explain_promotion_gate -- --nocapture
cargo test --test parse_real_files normalized_geometry_projection_preserves_promoted_hint_source_notes -- --nocapture
```

如果新增 fixture 影响 parse pipeline，再运行：

```powershell
cargo test --test parse_real_files -- --nocapture
```

## 7. 不做事项

- 不用 synthetic fixture 假装完成 8-12 个真实 fixture 目标。
- 不降低 `GEOMETRY_FIXTURE_TARGET_MIN_AVAILABLE=8`。
- 不为了让 fixture 通过而扩大 parser 行为面。
- 不把 probe-only text/symbol 渲染为 stable geometry。

## 8. 下一步输入

需要新增真实 `.pid` fixture，或确认当前 5-fixture 基线可以先提交。

如果选择继续实现，请提供额外 fixture 路径；如果选择收敛，请更新 `CHANGELOG.md` 后提交并推送当前改动。
