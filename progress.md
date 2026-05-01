# 进度日志：PID 解析开发方案

## Session: 2026-04-30

### 当前状态
- **Phase:** 4 - 规范化语义图层
- **状态:** Phase 4 已开始，首个 import-view relationship provenance 切片完成

### 已完成
- 使用 planning-with-files 创建 `task_plan.md`、`findings.md`、`progress.md`。
- 阅读并汇总当前解析支持范围、成熟度与缺口。
- 形成中文开发优先级：PSM → Sheet → canonical graph → DWG publish。
- 生成中文开发方案文档：`docs/plans/2026-04-30-pid-parse-development-plan-cn.md`。
- 生成技术路线图 SVG：`docs/diagrams/pid-parse-development-roadmap.svg`。
- 使用 Python XML parser 验证 SVG 语法通过。
- 按 TDD 补齐 `PsmClusterRecordDecoded::unknown_prefix_bytes`：先让测试失败，再实现真实未知前缀字节收集。
- 按 TDD 为 `PsmSegmentEntry` 增加 `candidate_owner_cluster_index/name`：先用测试确认字段缺失，再实现 1:1 positional owner 候选关联。
- 按 TDD 将 segment `owner_candidate=index:name` 接入文本 report，便于 `pid_inspect` 人工审查。
- 按 TDD 更新 coverage note，将 `PSMsegmenttable` 描述改为 segment flags + owner candidate mapping，仍保持 partial decoded。
- 补充 schema 回归测试，确认 `pid_inspect --schema` 输出包含 `PsmSegmentEntry` candidate owner 字段。
- 扩展真实 fixture soft-skip 测试 `psm_segment_record_probes_align_with_flags`，校验 candidate owner 与旧 probe hint 一致。
- 补充 byte-audit aggregate 回归测试，确认 `/PSMclustertable` 同时保留 decoded/probed/leftover 分桶。
- 完成 byte-audit confidence 评估：candidate 字段继续留在 `Probed` 前缀范围，不升级为 `Decoded`。
- 将 `task_plan.md` 中 Phase 2 标记为 complete；下一步可提交当前增量，或进入 Phase 3 Sheet 几何。
- 按 TDD 新增 Sheet geometry DTO schema 合同：先确认 schema 缺 `SheetGeometry`，再新增 DTO 与 `SheetStream.geometry`。
- 按 TDD 将 `sheet_probe` 的 text runs 与 coordinate hints 归一化填充到 `SheetStream.geometry`。
- 按 TDD 将 `SheetEndpointRecord` 同步填充到 `SheetStream.geometry.endpoints`。
- 扩展 synthetic 回归，确认 endpoint 同步不会覆盖已归一化的 text 与 coordinate hint。
- 将 `task_plan.md` 中 Phase 3 当前范围标记为 complete；下一步可提交当前增量，或进入 Phase 4 canonical graph。
- 按 TDD 将 `CrossReferenceGraph.relationship_endpoint_links` 映射到 `PidImportView.relationships`，暴露 sheet path/offset 与 source/target field_x。

### 验证
| 检查项 | 结果 |
|---|---|
| 根目录是否已有 planning 三文件 | 无，已新增 |
| `rsvg-convert` | 未安装 / 不在 PATH |
| `magick` | 未安装 / 不在 PATH |
| SVG XML 验证 | 通过 |
| `cargo test parsers::psm_tables::tests::cluster_table_decoded_records_capture_conservative_prefix_candidates -- --nocapture` | 先失败后通过 |
| `cargo test parsers::psm_tables::tests::apply_segment_owner_hints_backfills_matching_lengths -- --nocapture` | 先失败后通过 |
| `cargo test parsers::psm_tables -- --nocapture` | 通过，31 passed |
| `cargo test inspect::report::tests::report_shows_psm_segment_record_probe_sample -- --nocapture` | 通过 |
| `cargo test inspect::coverage::tests::coverage_note_mentions_psm_segment_owner_candidates -- --nocapture` | 先失败后通过 |
| `cargo test schema::tests::schema_exposes_psm_segment_owner_candidates -- --nocapture` | 通过 |
| `cargo test psm_segment_record_probes_align_with_flags -- --nocapture` | 通过 |
| `cargo test byte_audit::aggregate::tests::psm_cluster_table_audit_keeps_decoded_probed_and_leftover_buckets -- --nocapture` | 通过 |
| `cargo test schema::tests::schema_exposes_sheet_geometry_dtos -- --nocapture` | 先失败后通过 |
| `cargo test streams::cluster::tests::geometry_from_sheet_probe_normalizes_text_and_coordinate_hints -- --nocapture` | 先失败后通过 |
| `cargo test cfb::reader::tests::sync_sheet_geometry_endpoints_copies_endpoint_records -- --nocapture` | 先失败后通过 |
| `cargo test cfb::reader::tests::sync_sheet_geometry_endpoints_copies_endpoint_records -- --nocapture && cargo fmt --check && cargo test` | 通过 |
| `cargo test import_view::tests::build_import_view_collects_objects_symbols_and_unresolved -- --nocapture` | 先失败后通过 |
| `cargo fmt --check` | 通过 |
| `cargo test` | 通过 |
| `cargo fmt --check && cargo test` | 通过 |
| `cargo fmt && cargo test` | 通过 |
| ReadLints | 无错误 |

### 备注
- 首次尝试使用 bash heredoc 生成 SVG 失败，原因是 PowerShell 不支持 `python - <<'PY'`；已切换为 PowerShell here-string 管道给 Python。
- SVG 已生成并验证；PNG 导出等待转换工具可用后补。
- `unknown_prefix_bytes` 现在表示已识别候选字段之外的前缀字节，可用于后续 fixture 对比和字段命名收敛。
- 首次尝试同时传两个 `cargo test` 过滤参数失败，原因是 `cargo test` 只支持一个 TESTNAME；已改为两条测试顺序执行。
- 扩展真实 fixture 测试后，`cargo fmt --check` 发现一处 rustfmt 换行差异；已运行 `cargo fmt` 修复。
- `TraceConfidence::Decoded` 语义是稳定 semantic meaning；PSM candidate 字段仍是 byte-layout evidence，因此不做 confidence 升级。

## Session: 2026-05-01

### 当前状态
- **Phase:** 7 - H7CAD PID 真实几何显示与证据门禁
- **状态:** H7CAD 可显示 inferred points；object-coordinate mapping 仍未 source-proven；GraphicIdentityNearby 已完成 Phase A-C 调查并保持 no-promotion。

### 已完成
- 建立 `NormalizedPidGeometry` 与 H7CAD inferred point rendering。
- 建立 `/Sheet6.object_geometry_hints == 0` guardrail。
- 完成 field-x window / repeated-delta / stable chunk-shape / marker / coordinate-quality filters。
- 完成 GraphicIdentityNearby：
  - identity index helper。
  - record_id / ASCII DrawingID / UTF-16LE DrawingID scanner。
  - same-object identity scoring wrapper。
  - `/Sheet6` identity report 与 scoring report。
- 更新 PR split inventory：新增 PR5 作为 GraphicIdentityNearby 独立 investigation PR。
- 按 planning-with-files 更新 `task_plan.md`、`findings.md`、`progress.md`。
- 使用 diagram skill 生成路线图：`docs/diagrams/h7cad-pid-real-geometry-roadmap.svg`。
- 导出 PNG：`docs/diagrams/h7cad-pid-real-geometry-roadmap.png`。
- 新增中文 PR 执行清单：`docs/plans/2026-05-01-h7cad-pid-pr-execution-checklist-cn.md`。
- 新增 Text placement 证据路线计划：`docs/plans/2026-05-01-text-placement-evidence-plan-cn.md`。
- 实现 Text placement Phase A investigation helper：`sheet_text_window_candidates`。
- 添加 `/Sheet6` Text placement report 测试，确认仍不 promotion 为 `PidGraphicKind::Text`。
- 实现 Text placement Phase B 第一版 text-quality filter 与 investigation-only scoring。
- 更新 `task_plan.md` / `findings.md`，纳入 Text placement Phase A/B 结论。
- 生成 Text placement 路线图：`docs/diagrams/h7cad-pid-text-placement-roadmap.svg`。
- 导出 PNG：`docs/diagrams/h7cad-pid-text-placement-roadmap.png`。
- 生成 PR1-PR6 拆分路线图：`docs/diagrams/h7cad-pid-pr-split-roadmap.svg`。
- 导出 PNG：`docs/diagrams/h7cad-pid-pr-split-roadmap.png`。
- 按推荐方案执行非破坏性下一步：保留已推送 `main`，进入多 fixture evidence inventory。
- 新增 `available_pid_fixtures_geometry_evidence_inventory_stays_probe_only`，横向扫描当前可用 5 个 PID fixture，并包含非 ASCII 文件名 fixture。
- 确认多 fixture 结果仍不支持 geometry promotion：`identity_supported=0`、`identity_over_threshold=0`、`text_over_threshold=0`。
- 按用户要求继续使用 planning-with-files 与 diagram skill，新增 Phase 8 完整解析推进路线。
- 新增中文方案：`docs/plans/2026-05-02-h7cad-pid-complete-parse-next-stage-plan-cn.md`。
- 新增路线图：`docs/diagrams/h7cad-pid-complete-parse-next-stage.svg` 与 `.png`。
- 更新 `task_plan.md` 当前阶段为 Phase 8，并记录 fixture 扩容、Sheet record grammar、promotion gate、H7CAD Line/Text/Symbol layer 的后续任务。
- 按 Phase 8 第一项继续扩展 inventory：新增 per-fixture / per-sheet 明细输出，区分无 endpoint `field_x` 的 Sheet。

### 验证
| 检查项 | 结果 |
|---|---|
| `cargo test --lib parsers::sheet_probe -- --nocapture` | 通过，26 passed |
| `cargo test --test parse_real_files sheet6_field_x_window_identity_report -- --nocapture` | 通过，same_object=11, wrong_object=414 |
| `cargo test --test parse_real_files sheet6_graphic_identity_scoring_keeps_object_hints_empty_until_proven -- --nocapture` | 通过，identity_supported=0, max_score=45, over_threshold=0 |
| `cargo test --test parse_real_files all_sheets_graphic_identity_scoring_report_keeps_object_hints_empty -- --nocapture` | 通过，sheets=1, identity_supported=0, over_threshold=0 |
| `cargo test --lib parsers::sheet_probe::tests::text_window_candidates_link_text_to_nearby_quality_coordinates_without_promotion -- --nocapture` | 通过 |
| `cargo test --lib parsers::sheet_probe::tests::text_window_scoring_rejects_binary_like_text_before_position_scoring -- --nocapture` | 通过 |
| `cargo test --test parse_real_files sheet6_text_window_report_keeps_text_probe_only_until_position_is_proven -- --nocapture` | 通过，text_runs=9, candidates=121, same_chunk=25, coordinate_quality_passed=2, text_quality_passed=0, max_score=-50, over_threshold=0 |
| `cargo test --lib parsers::sheet_probe -- --nocapture` | 通过，28 passed |
| `cargo test --lib -- --nocapture` | 通过，742 passed |
| `cargo test --test parse_real_files -- --nocapture` | 通过，51 passed |
| `cargo test --locked --workspace --all-targets` | 通过 |
| PR5/PR6 focused validation bundle | 通过：identity report、identity scoring、all-Sheet identity scoring、Text placement Sheet6 report 均 green |
| `cargo test -p H7CAD pid_bundle -- --nocapture` | 通过，4 passed |
| `cargo fmt --all -- --check` | 通过 |
| `cargo build --locked --workspace --all-targets` | 通过 |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | 通过；先修复 `sheet_probe.rs` doc/closure/range lint，并将 `cfb/reader.rs` test module 移到文件末尾 |
| `bash .github/scripts/check-missing-docs.sh` | 当前 Windows `C:\Windows\System32\bash.exe` 环境输出乱码并退出 1，未作为有效结果采信 |
| `cargo rustdoc --lib --locked -- -W missing-docs` | 通过；baseline 为 0，无 missing-docs warning |
| `rsvg-convert -w 1920 docs/diagrams/h7cad-pid-real-geometry-roadmap.svg -o docs/diagrams/h7cad-pid-real-geometry-roadmap.png` | 通过，有字体 fallback 警告 |
| `rsvg-convert docs/diagrams/h7cad-pid-text-placement-roadmap.svg -o NUL && rsvg-convert -w 1920 docs/diagrams/h7cad-pid-text-placement-roadmap.svg -o docs/diagrams/h7cad-pid-text-placement-roadmap.png` | 通过，有字体 fallback 警告 |
| `rsvg-convert docs/diagrams/h7cad-pid-pr-split-roadmap.svg -o NUL && rsvg-convert -w 1920 docs/diagrams/h7cad-pid-pr-split-roadmap.svg -o docs/diagrams/h7cad-pid-pr-split-roadmap.png` | 通过，有字体 fallback 警告 |
| `cargo test --test parse_real_files available_pid_fixtures_geometry_evidence_inventory_stays_probe_only -- --nocapture` | 通过，fixtures=4, sheets=3, windows=6337, identities=437, same_object=17, wrong_object=420, identity_supported=0, max_identity_score=45, identity_over_threshold=0, text_candidates=537, text_over_threshold=0 |
| `cargo test --test parse_real_files all_sheets_graphic_identity_scoring_report_keeps_object_hints_empty -- --nocapture` | 通过，sheets=1, identity_supported=0, over_threshold=0 |
| `cargo test --test parse_real_files sheet6_text_window_report_keeps_text_probe_only_until_position_is_proven -- --nocapture` | 通过，max_score=-50, over_threshold=0 |
| ReadLints `tests/parse_real_files.rs` | 无错误 |
| `rsvg-convert docs/diagrams/h7cad-pid-complete-parse-next-stage.svg -o NUL` | 通过，有字体 fallback 警告 |
| `rsvg-convert -w 1920 docs/diagrams/h7cad-pid-complete-parse-next-stage.svg -o docs/diagrams/h7cad-pid-complete-parse-next-stage.png` | 通过，有字体 fallback 警告 |
| `cargo test --test parse_real_files available_pid_fixtures_geometry_evidence_inventory_stays_probe_only -- --nocapture` | 通过，fixtures=5, sheets=3, windows=6337, identities=437, same_object=17, wrong_object=420, identity_supported=0, max_identity_score=45, identity_over_threshold=0, text_candidates=578, text_over_threshold=0；输出 per-fixture / per-sheet 明细 |
| ReadLints `tests/parse_real_files.rs` | 无错误 |

### 下一步
- 增加 top identity/text candidate record dump helper。
- 建立第一版 Sheet record shape classifier。
- 如仍需要 review 形态，需先确认是否从 `b0481c5` 重建 PR1-PR6 分支；否则继续保留 `main` 合并提交。
- 继续禁止从 endpoint/topology 推导 line。

## Session: 2026-05-02

### 当前状态
- **Phase:** 8 - 完整解析推进路线
- **状态:** top candidate record dump helper 与第一版 Sheet record shape classifier 已完成；仍保持 no-promotion gate。

### 已完成
- 重读 `task_plan.md` / `findings.md` / `progress.md`，确认当前计划文件仍是项目根目录的主工作记忆。
- 使用 diagram skill 的 flat-icon 风格要求，准备刷新 Phase 8 路线图。
- 更新 `docs/plans/2026-05-02-h7cad-pid-complete-parse-next-stage-plan-cn.md`：
  - 将 fixture 覆盖从“4 个 ASCII 路径 fixture”更新为“5 个 PID fixture，含非 ASCII 文件名与 publish fixture”。
  - 将近期任务清单前两项标为完成。
- 更新 `findings.md`，补充 H7CAD 主工作树与 `H7CAD-pid-real-geometry-display` 工作树的差异，避免后续误以为主 `H7CAD/` 已接入 `build_normalized_geometry`。
- 按 TDD 增加 top identity/text candidate record dump helper：
  - RED：`top_candidate_record_dumps_rank_scores_and_keep_hex_windows` 先因缺少 helper 编译失败。
  - GREEN：新增 `SheetCandidateRecordWindow`、`SheetFieldXCandidateRecordDump`、`SheetTextCandidateRecordDump` 与两个 top dump helper。
  - 集成：`sheet6_top_candidate_record_dump_stays_investigation_only` 改为复用 helper，继续保持 `object_geometry_hints=0`。
- 按 TDD 建立第一版 Sheet record shape classifier：
  - RED：`record_shape_classifier_groups_distinct_non_endpoint_field_shapes` 先因缺少 classifier 编译失败。
  - GREEN：新增 `SheetFieldXRecordShapeClass` 与 `classify_field_x_record_shapes`，按 chunk-relative field/coordinate deltas 聚合 distinct non-endpoint `field_x` support。
  - 集成：`sheet6_field_x_window_features_report_chunk_shapes` 输出 top record shape classes；当前 `/Sheet6` top shapes 为 `(14,38)`、`(46,70)`，support 均为 2，仍保持 `promotable=0`。
- 扩展多 fixture inventory：输出 `record_shape_classes`、per-sheet `top_record_shape` 与 aggregate `top_record_shapes`；当前 5 fixture 合计 `record_shape_classes=328`，top aggregate shapes 最高累计 support 为 4，仍无 promotion threshold 命中。

### 验证
| 检查项 | 结果 |
|---|---|
| `python C:/Users/Administrator/.codex/skills/planning-with-files/scripts/session-catchup.py D:/work/plant-code/cad/pid-parse` | 通过，无输出 |
| `cargo test -p H7CAD normalized_geometry_real_fixture_renders_expected_points -- --nocapture` | 通过，`normalized=132`、`rendered=64`、`inferred_points=64`、`probe_unknowns=68`、`point_layer=64` |
| `cargo test --test parse_real_files available_pid_fixtures_geometry_evidence_inventory_stays_probe_only -- --nocapture` | 通过，`fixtures=5`、`sheets=3`、`identity_supported=0`、`identity_over_threshold=0`、`text_over_threshold=0` |
| `rsvg-convert docs/diagrams/h7cad-pid-complete-parse-next-stage.svg -o NUL && rsvg-convert -w 1920 ... -o ...png` | 通过，PNG 已刷新；仍有字体 fallback 警告 |
| `cargo test --lib parsers::sheet_probe::tests::top_candidate_record_dumps_rank_scores_and_keep_hex_windows -- --nocapture` | RED 阶段按预期缺少 helper；实现后通过 |
| `cargo test --test parse_real_files sheet6_top_candidate_record_dump_stays_investigation_only -- --nocapture` | 通过，输出结构化 top identity/text dumps，仍无 geometry hint promotion |
| `cargo test --lib parsers::sheet_probe -- --nocapture` | 通过，29 passed |
| `cargo fmt --all && cargo fmt --all -- --check` | 通过 |
| `cargo test record_shape_classifier_groups_distinct_non_endpoint_field_shapes` | RED 阶段按预期缺少 classifier；实现后通过 |
| `cargo test sheet6_field_x_window_features_report_chunk_shapes -- --nocapture` | 通过，输出 top record shape classes，`max_score=45`、`promotable=0` |
| `cargo test --test parse_real_files available_pid_fixtures_geometry_evidence_inventory_stays_probe_only -- --nocapture` | 通过，`record_shape_classes=328`、top aggregate shapes support 最高为 4，`identity_over_threshold=0`、`text_over_threshold=0` |

### 错误与限制
| 问题 | 处理 |
|---|---|
| 首次 session-catchup 使用 `%USERPROFILE%` 未被当前 shell 展开，Python 误拼到工作目录下 | 改用绝对路径 `C:/Users/Administrator/.../session-catchup.py` 后通过 |
| `rsvg-convert` 找不到指定中文字体组合时输出 Pango fallback warning | SVG/PNG 导出成功；当前作为视觉字体差异记录，不影响计划文件有效性 |

### 下一步
- 在 source-proven gate 达标后，再填充 `SheetObjectGeometryHint` 并升级 H7CAD Line/Text/Symbol layer。
