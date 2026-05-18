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

## Session: 2026-05-06

### 当前状态
- **Phase:** 9 - 下一阶段开发计划制定
- **状态:** 已基于当前解析能力与几何证据基线，产出新的中文开发计划；下一步进入 fixture baseline hardening。

### 已完成
- 复核 `docs/prd-pid-parse-current-state.md`、`ARCHITECTURE.md`、`README.md`、`task_plan.md`、`findings.md`、`progress.md`。
- 复核核心源码契约：
  - `src/model.rs`：`PidDocument` 聚合 CFB、metadata、JSite、cluster、dynamic attributes、Sheet、PSM、object graph、cross-reference、layout。
  - `src/import_view.rs`：`PidImportView.relationships` 已暴露 Sheet endpoint provenance。
  - `src/geometry.rs`：当前 normalized geometry 只 promotion coordinate hint 为 inferred point，text/endpoint 仍是 probe-only unknown。
- 确认当前支持进度：
  - `.pid` 容器/metadata/object graph/crossref/layout/writer 已具备稳定工程骨架。
  - MDF-first publish XML A01 主线成熟度高，DWG 侧仍需 fixture/enrichment 闭环。
  - Sheet 深层几何仍未 source-proven，5 fixture inventory 仍无 Line/Text/Symbol promotion 证据。
- 新增开发计划：`docs/plans/2026-05-06-pid-parse-development-plan-cn.md`。
- 更新 `task_plan.md`：新增 Phase 9，并把当前阶段切到“下一阶段开发计划制定”。
- 更新 `findings.md`：记录 Phase 9 顺序、promotion 铁律与 fixture baseline 优先级。

### 验证
| 检查项 | 结果 |
|---|---|
| planning-with-files `session-catchup.py` | 通过，无输出 |
| 代码/文档读取核对 | 通过 |
| 测试执行 | 未执行；本轮仅制定计划与更新 Markdown |

### 错误与限制
| 问题 | 处理 |
|---|---|
| 首次 `check_messages` 未带 `turn_complete` 导致 MCP validation error | 重试时显式传入 `turn_complete=true` 后成功 |
| terminals 目录探测路径不存在 | 不影响本次计划制定；后续 shell 命令仍在项目根目录正常执行 |
| 读取 `progress.md` offset 220 超出文件长度 | 改用已读取的 189 行完整内容作为进度依据 |

### 下一步
- 执行 Phase 9A：扩展 fixture registry 与 inventory baseline，目标 8-12 个真实 PID fixture。
- 对 top aggregate record shapes `(12,-18)`、`(14,38)`、`(68,5)` 建立稳定审查报告。
- 为 `SheetObjectGeometryHint` 保持 no-promotion guardrail，直到 source-proven gate 达标。

### Phase 9A 实现进展
- 按 TDD 新增 `geometry_fixture_registry_documents_phase9a_targets` 红测：
  - RED：缺少 `geometry_fixture_cases()` 与 `GEOMETRY_FIXTURE_TARGET_MIN_AVAILABLE`，编译失败。
  - GREEN：新增 `GeometryFixtureCase`、显式 fixture registry、目标最小 fixture 数 `8`。
- 将 `available_pid_fixtures_geometry_evidence_inventory_stays_probe_only` 改为复用 `geometry_fixture_cases()`。
- inventory detail 现在输出 `category`，区分 `dwg`、`non_ascii`、`publish_a01`、`publish_dwg`。
- 重新实测发现当前代码已非旧 no-promotion 基线：
  - `fixtures=5`
  - `sheets=3`
  - `record_shape_classes=328`
  - `identity_supported=44`
  - `identity_over_threshold=28`
  - `promotable=5`
  - `object_geometry_hint_count=5`
  - `text_over_threshold=0`
- 已同步更新 `docs/plans/2026-05-06-pid-parse-development-plan-cn.md`、`task_plan.md`、`findings.md`，将 Phase 9C 从 no-promotion 改为 promotion gate hardening。

### Phase 9A 验证
| 检查项 | 结果 |
|---|---|
| `cargo test --test parse_real_files geometry_fixture_registry_documents_phase9a_targets -- --nocapture` | RED 阶段按预期缺少 registry；实现后通过 |
| `cargo test --test parse_real_files available_pid_fixtures_geometry_evidence_inventory_stays_probe_only -- --nocapture` | 通过，输出 fixture category 与当前 promotion baseline |
| `cargo test --test parse_real_files geometry_fixture_registry_documents_phase9a_targets -- --nocapture && cargo test --test parse_real_files available_pid_fixtures_geometry_evidence_inventory_stays_probe_only -- --nocapture` | 通过 |
| `ReadLints` | 无错误 |
| `cargo fmt --all -- --check` | 失败；输出包含多处本轮未改的既有未格式化片段，未运行全量 `cargo fmt` 以避免改动用户已有代码 |

### Phase 9A 错误与限制
| 问题 | 处理 |
|---|---|
| 首次尝试用两个 TESTNAME 过滤参数运行 `cargo test` 失败 | Cargo 只支持一个 TESTNAME；改为两条 focused test 顺序执行后通过 |
| `cargo fmt --all -- --check` 发现 `src/cfb/reader.rs`、`src/parsers/sheet_probe.rs`、`tests/parse_real_files.rs` 多处既有格式漂移 | 只手动整理本轮新增 registry 测试块，未运行全量格式化以避免修改无关代码 |

### Phase 9A 下一步
- 为 promoted `SheetObjectGeometryHint` 增加 provenance-focused regression。
- 为 registry 增加 fixture availability summary，明确当前 5/目标 8 的缺口。
- 继续扩展 fixture registry 到 8-12 个真实 PID fixture。

### Phase 9C 实现进展
- 按 TDD 新增 `promoted_object_geometry_hints_explain_promotion_gate`：
  - 初始红测假设 `field_x` 必须直接命中 `ObjectGraph.objects.field_x`，失败后确认该假设过强；当前 same-object 证据来自 DA trailer identity resolver。
  - 调整红测为锁定 source Sheet offset、coordinate offset、promotion note。
  - RED：note 只有 `score=95`，缺少 identity/stable shape 说明。
  - GREEN：`populate_object_geometry_hints()` 改用 `object_geometry_hint_note()`，输出 `score=...;identity=graphic_nearby;stable_shape=...`。
- 更新 `docs/plans/2026-05-06-pid-parse-development-plan-cn.md`、`task_plan.md`、`findings.md`，记录 provenance guardrail 已完成。

### Phase 9C 验证
| 检查项 | 结果 |
|---|---|
| `cargo test --test parse_real_files promoted_object_geometry_hints_explain_promotion_gate -- --nocapture` | RED 阶段按预期缺少 promotion note 证据；实现后通过 |
| `cargo test --test parse_real_files geometry_fixture_registry_documents_phase9a_targets -- --nocapture && cargo test --test parse_real_files available_pid_fixtures_geometry_evidence_inventory_stays_probe_only -- --nocapture && cargo test --test parse_real_files promoted_object_geometry_hints_explain_promotion_gate -- --nocapture` | 通过 |

### Phase 9C 下一步
- 给 normalized geometry projection 增加 promoted hint source note 回归，确认 H7CAD/renderer 能读到 promotion gate 摘要。
- 为 registry 增加 fixture availability summary。

### Phase 9A Availability Summary 实现进展
- 按 TDD 新增 `geometry_fixture_availability_summary_tracks_target_gap`：
  - RED：缺少 `geometry_fixture_availability_summary()`，编译失败。
  - GREEN：新增 `GeometryFixtureAvailabilitySummary`，输出 `registered`、`target_min_available`、`available`、`missing`。
- 当前 summary 用 `test-file/<fixture.path>` 判断 fixture 是否可用，并继续保留 `GEOMETRY_FIXTURE_TARGET_MIN_AVAILABLE=8` 的目标缺口。

### Phase 9A Availability Summary 验证
| 检查项 | 结果 |
|---|---|
| `cargo test --test parse_real_files geometry_fixture_availability_summary_tracks_target_gap -- --nocapture` | RED 阶段按预期缺少 helper；实现后通过 |
| registry / availability / inventory / promotion provenance 四个 focused tests 顺序执行 | 通过 |

### Phase 9A Availability Summary 下一步
- 将 availability summary 接入 inventory report 输出，方便人工阅读当前 registered/available/missing/target 状态。
- 继续收集并登记更多真实 PID fixture。

### Phase 9A Availability Report 实现进展
- 按 TDD 新增 `geometry_fixture_availability_report_line_is_human_readable`：
  - RED：缺少 `geometry_fixture_availability_report_line()`，编译失败。
  - GREEN：新增 report line helper，输出 `registered`、`target_min_available`、`available`、`missing`。
- 将 report line 接入 `available_pid_fixtures_geometry_evidence_inventory_stays_probe_only` 输出。
- 当前 inventory 输出头：
  - `geometry fixture availability: registered=5, target_min_available=8, available=5, missing=[]`

### Phase 9A Availability Report 验证
| 检查项 | 结果 |
|---|---|
| `cargo test --test parse_real_files geometry_fixture_availability_report_line_is_human_readable -- --nocapture` | RED 阶段按预期缺少 helper；实现后通过 |
| `cargo test --test parse_real_files available_pid_fixtures_geometry_evidence_inventory_stays_probe_only -- --nocapture` | 通过，输出 availability report line |
| registry / availability / report line / inventory / promotion provenance 五个 focused tests 顺序执行 | 通过 |

### Phase 9A Availability Report 下一步
- 继续扩展 fixture registry 到 8-12 个真实 PID fixture。
- 给 normalized geometry projection 增加 promoted hint source note 回归。

### Phase 9C Normalized Projection Source Note 回归进展
- 新增 `normalized_geometry_projection_preserves_promoted_hint_source_notes`：
  - 覆盖真实 fixture `DWG-0201GP06-01.pid`。
  - 对每个带 position 的 promoted `SheetObjectGeometryHint`，确认 `build_normalized_geometry()` 生成的 inferred point 保留 `stream_path`、`field_x`、position 与 source note。
  - 锁定 source note 包含 `score=`、`identity`、`stable_shape` promotion gate 摘要。
- 运行后测试直接通过，说明生产代码已将 `hint.note` 复制到 `PidGraphicProvenance.note`；本轮无需修改 `src/geometry.rs`。
- 已同步更新 `docs/plans/2026-05-06-pid-parse-development-plan-cn.md`、`task_plan.md`、`findings.md`。

### Phase 9C Normalized Projection Source Note 验证
| 检查项 | 结果 |
|---|---|
| `cargo test --test parse_real_files normalized_geometry_projection_preserves_promoted_hint_source_notes -- --nocapture` | 通过 |
| `cargo test --test parse_real_files geometry_fixture_availability_report_line_is_human_readable -- --nocapture; cargo test --test parse_real_files promoted_object_geometry_hints_explain_promotion_gate -- --nocapture; cargo test --test parse_real_files normalized_geometry_projection_preserves_promoted_hint_source_notes -- --nocapture` | 通过 |
| `ReadLints` on edited files | 无错误 |
| `cargo fmt --all -- --check` | 失败；仍包含既有 `src/cfb/reader.rs`、`src/parsers/sheet_probe.rs`、`tests/parse_real_files.rs` 格式漂移，未运行全量 `cargo fmt` 以避免改动无关代码 |

### Phase 9C 下一步
- 继续扩展 fixture registry 到 8-12 个真实 PID fixture。
- 视新增 fixture 结果补充新的 promotion gate focused regression。

### Phase 9A Fixture Expansion 方案补充
- 已新增中文执行方案：`docs/plans/2026-05-06-phase-9a-fixture-expansion-plan-cn.md`。
- 方案明确当前 Phase 4 的真实阻塞：本地 registry 只有 5 个 `.pid` fixture，目标 8-12 需要新增外部真实样本。
- 方案给出 fixture 选择标准、registry 元数据建议、TDD 切片、验证命令与不做事项。
- 下一步需要二选一：
  - 提供额外真实 `.pid` fixture 后继续扩展 registry。
  - 或确认先提交当前 5-fixture 基线，再等待后续样本。

### Phase 9A Fixture 扩容复查
- 使用本地 `test-file` 递归枚举 `.pid` fixture，当前仅发现：
  - `test-file\工艺管道及仪表流程-1.pid`
  - `test-file\DWG-0201GP06-01.pid`
  - `test-file\DWG-0202GP06-01.pid`
  - `test-file\export-test\publish-data\A01\A01.pid`
  - `test-file\export-test\publish-data\DWG-0202GP06-01\DWG-0202GP06-01.pid`
- 这些路径均已在 `geometry_fixture_cases()` registry 中；当前没有额外本地真实 PID 样本可登记。
- Phase 9A 的 8-12 fixture 目标现在被 fixture 供给阻塞，需用户提供更多真实 PID 文件后继续。

## Session: 2026-05-09

### 当前状态
- **Phase:** 10 - f64 Record Shape 坐标源与 Endpoint Line 闭环
- **状态:** 方案制定完成，待执行

### 已完成
- 全面复核项目当前实现进度：75 源码文件、26 集成测试、806+ 测试用例。
- 分析 Phase 9 各子阶段状态：
  - 9A fixture baseline hardening 被 fixture 供给阻塞（5/8 目标）。
  - 9B Sheet record grammar RE 未深入。
  - 9C promotion gate hardening 诊断链揭示关键突破：repeated f64 pair 坐标候选。
  - 9D/9E/9F 待开始。
- 识别核心突破口：Phase 9C 已发现 `/Sheet6` missing endpoint field_x 前 22 字节有 repeated f64 pair 坐标值，且呈连续递增非随机形态。
- 制定 Phase 10 开发方案：6 个 Slice，从 f64 pair extraction → promotion gate fallback → endpoint line 产生 → 多 fixture 横向验证 → H7CAD 消费 → 全量回归。
- 方案文件：`docs/plans/2026-05-09-phase-10-f64-coordinate-source-endpoint-line-plan-cn.md`。
- 更新 `task_plan.md`：新增 Phase 10，切换当前阶段。
- 更新 `findings.md`：记录 f64 pair 坐标候选突破。

### Phase 10 Slice 1-3 实现进展
- Slice 1：`SheetFieldXF64PairCandidate` 与 `repeated_f64_pair_candidate_before_field_x` 已存在；扩展 `SheetFieldXF64PairShape` 携带 `x, y` 值，新增 `into_candidate()` 方法。
- Slice 2：
  - 新增 `f64_pair_candidate: Option<SheetFieldXF64PairCandidate>` 到 `SheetFieldXWindowScore`。
  - `score_field_x_window_features` 现在在 f64 pair 支持 >= 3 时填充 `f64_pair_candidate`。
  - 新增 `SheetF64CoordinateHintDto` DTO 到 `model.rs`。
  - 新增 `f64_position: Option<SheetF64CoordinateHintDto>` 到 `SheetObjectGeometryHint`。
  - 新增 `passes_f64_pair_gate()` 作为 `populate_object_geometry_hints` 的替代 promotion gate。
  - 替代 gate 条件：`ObjectFieldResolves + RepeatedF64PairBeforeField(support >= 3)`。
  - promotion note 包含 `coordinate_source=f64_pair_before_marker` 或 `coordinate_source=nearest_coordinate_hint`。
  - `build_normalized_geometry` 新增统一 `ResolvedObjectPosition`，同时支持 i32 和 f64 坐标源，用于 endpoint pair line 推断。
- Slice 3 结果：
  - `DWG-0201GP06-01.pid`：`promotable` 从 5 → 16（+11 f64 pair gate）。
  - `DWG-0202GP06-01.pid`：`promotable` 从 0 → 2（新）。
  - `inferred_points` 从 69 → 80（+11）。
  - `inferred_lines` 仍为 0：endpoint pair 两端不同时 promoted（`only_b=39` 但 `only_a=5`）。
  - 结论：f64 pair gate 显著扩展了单端 promoted 覆盖，但 endpoint pair line 需要进一步扩展对端 promotion 覆盖。

### Phase 10 验证
| 检查项 | 结果 |
|---|---|
| `cargo build --locked -j 1` | 通过 |
| `cargo test --locked -j 1 --lib` | 通过，759 passed |
| `cargo test --locked -j 1 --test parse_real_files` | 通过，65 passed |
| `cargo clippy --locked -j 1 -- -D warnings` | 通过 |
| `cargo fmt --all -- --check` | 通过 |
| `object_geometry_hint_count=20, promotable=20` | 对齐 |

### Phase 10 Slice 3B-6 实现进展
- Slice 3B：诊断 endpoint pair 两端不对称原因。
  - `only_b=39` 的 endpoint_b 为 630-640（f64 pair gate 覆盖）。
  - endpoint_a 值（646, 661, 665, 673 等）不含 `5E 00 22 00 00 00` marker pattern，无法触发 f64 pair gate。
  - 这些 endpoint_a field_x 的 best_score=40，缺少 `GraphicIdentityNearby` 和 `StableChunkShape`。
  - 结论：endpoint line 需要进一步扩展对端 marker 模式覆盖，非当前 Phase 10 scope。
- Slice 4：多 fixture 横向验证。
  - DWG-0201GP06-01.pid：inferred_points 69→80（+11 f64 pair）。
  - DWG-0202GP06-01.pid：inferred_points 69→71（+2 f64 pair）。
  - 其他 3 fixture 无 endpoint field_x，无变化。
- Slice 5：取消（当前无 inferred_lines 可供 H7CAD 消费）。
- Slice 6：全量回归。
  - cargo build 通过。
  - cargo test --lib：759 passed。
  - cargo test --test parse_real_files：65 passed。
  - cargo clippy -D warnings 通过。
  - cargo fmt --check 通过。
  - cargo rustdoc --lib -- -W missing-docs 通过。

### Phase 10B: f64 Triple Pattern 实现进展
- 发现 endpoint_a field_xs 使用不同的 marker pattern：`FA 00 XX 00 00 00`（vs 原有 `5E 00 22 00 00 00`）。
- 新 pattern 前有 3 个 f64 值（24 字节），其中第 1 个是稳定基线（≈ 0.2245，与 endpoint_b 的 y 坐标一致），第 2-3 个是坐标候选。
- 新增 `repeated_f64_triple_candidate_before_field_x` 公共 extraction helper。
- 将新 extraction 集成到 `field_x_window_features` 的 f64 pair shape 搜索中（作为 fallback）。
- 结果：
  - `DWG-0201GP06-01.pid`：`inferred_points` 69→106，`inferred_lines` 0→**34**。
  - `DWG-0202GP06-01.pid`：`inferred_points` 69→74，`inferred_lines` 0→**3**。
  - 3 个 fixture 现在产生 line-producing geometry。

### Phase 10B 验证
| 检查项 | 结果 |
|---|---|
| `cargo build --locked -j 1` | 通过 |
| `cargo test --locked -j 1 --lib` | 通过，759 passed |
| `cargo test --locked -j 1 --test parse_real_files` | 通过，66 passed |
| `cargo clippy --locked -j 1 -- -D warnings` | 通过 |
| `cargo fmt --all -- --check` | 通过 |
| `cargo rustdoc --lib --locked -- -W missing-docs` | 通过 |
| line-producing fixtures | DWG-0201GP06-01.pid (34), DWG-0202GP06-01.pid (3), publish DWG (3) |

### Phase 10 Slice 5: H7CAD 端到端 line 消费
- H7CAD `cargo check --locked` 通过。
- `pid_import_real_sample_geometry_consumes_source_backed_layers` 自动接收到 pid-parse 新 geometry 输出，无需修改 H7CAD 代码。
- H7CAD 端到端结果：`points=42, lines=34, skipped_probe=34, skipped_broad=64`。
- 34 条 inferred endpoint lines 成功渲染到 `PID_GEOM_LINES` layer。

### Phase 11 Slice 1: 坐标值域分析
- f64 坐标域：`x ∈ [0.082, 0.475], y ∈ [0.000, 0.275]`，确认为 0-1 归一化页面坐标。
- 模板：`Template = XIONGANA2.pid`（A2 纸 594×420mm）。
- i32 坐标域：`x ∈ [0, 983056], y ∈ [-327679, 983056]`，独立坐标系。
- 新增 `f64_coordinate_domain_analysis_for_page_mapping` 诊断测试。

### Phase 11 Slice 2: 剩余 endpoint pair 覆盖扩展
- 初始 34/59 fully promoted（57.6%）。
- `only_a=11` 中有 4 对的 endpoint_b=0（空端点 / null），不是真实 line gap。
- `only_b=13` 中大多数缺失 endpoint_a 为低编号 field_xs（35, 68, 111, 139, 147, 157, 169, 229, 433, 440, 490），使用不同 record shape。
- 剩余真实 gap 约 21 对，需要另一轮逆向调查来覆盖低编号 field_x 的 record shape。
- 发现第三种 marker pattern `CE 00 XX 00 00 00`：2 f64 + 8 零字节 + marker + field_x。
- 扩展 `repeated_f64_triple_candidate_before_field_x` 支持 `FA 00` 和 `CE 00` 两种 marker。
- CE marker 的 f64 使用第 1-2 个值（非第 2-3 个），因为第 3 个是零填充。
- 结果：`fully_promoted` 34→**49/59**（83.1%），`inferred_lines` 34→**49**，`neither` 1→**0**。
- `probe_only_unknowns` 从 34 降到 19。
- 进一步分析：剩余 10 对 gap 中 4 对 endpoint_b=0（null），6 对涉及 field_x=659/671/35/68 不在 object_field_xs 中，属于 scope 边界。
- 排除 null 后有效覆盖率：49/55 = **89.1%**。

### Phase 11 Slice 3: Text placement 重评估
- `text_quality_passed=0, max_score=-50` — 与 Phase 7/8 结论一致。
- Top text candidates 仍是二进制数据误识别（`"060101럀"`, `"휱爿낳큷툪?"`），非工程标注。
- 结论：Text promotion 需要 text extraction 层面的根本改进（新 record shape 中的 text 字段识别），非当前 scope。

### 下一步
- 如有新真实 PID fixture 可供使用，优先扩展 registry（Phase 9A 仍待闭环）。
- 调查 f64 record shape 中是否包含 text field index 或 text record reference。
- H7CAD 坐标映射：f64 归一化坐标 × 页面尺寸 → 物理坐标。

## Session: 2026-05-17

### 当前状态
- **Phase:** 20 - PSM 0x0010 IDA-confirmed RAD class identity + sub-kind discriminator
- **状态:** Phase 18 / 19 已 commit + push；Phase 20 goal package 已落盘，
  详细中文路线图已就位；待用户 `/goal` 授权进入执行。
- **commit baseline:** `6beb6f1` (Phase 19) on origin/main
- **Phase 13-17 进度:** 全部 complete，细节托管在 `goals/phaseNN-…/`
  五件套；详见 `task_plan.md` Phase 13-20 条目。

### 已完成（本 session）
- 通过 MCP 桥接确认本会话工作目录切到 `d:\work\plant-code\cad\pid-parse`。
- 复核 Phase 18 commit `81daa20` + Phase 19 commit `6beb6f1` 的落盘情况，
  确认 working tree 只剩 `.superdesign/` 与 `dlls/` 两个未追踪目录（项目规则禁提交）。
- **Phase 19 RAD sibling probe 证伪**：跑 `examples/probe_rad_siblings_0x0029_0x0035.rs`，
  跨 4 fixture `/Sheet6` 上 PSM type code `0x0029..=0x0035` 只有 `0x0030` 有 hits
  （115 total，其余 12 个全 0）；落盘 evidence
  `docs/analysis/2026-05-17-phase19-rad-sibling-probe-null-result.md`。
- **Phase 19 leading-word probe 落地**：写 `examples/probe_psm_0x0010_sub_kind.rs`
  并跑通；578 records 中 `leading_word == 0x0002` = 164 (28%)、`0x0003` = 21、
  `0x0001` = 18；~30 个 size bucket 在 `+0` 处单 word 100% 覆盖，但 size 31
  (182 records) / 70 / 13 / 16 / 43 在 `+0` 异质。
- **Phase 19 goal package 起草**：
  `goals/phase19-psm-0x0010-leading-word-audit/` 五件套
  （brief / plan / verification / blockers / goal-prompt）+ 初始 progress.jsonl
  4 条 entry，总 ~41 KB。
- **Phase 19 Slice A-G 执行**：
  - Slice A 写 Phase 18 mirror 点 inventory 到 progress.jsonl。
  - Slice B 给 `SheetSubRecord0x0010Decoded` 加 `leading_word: Option<u16>` 字段，
    decoder 用 `raw_payload.get(0..2).map(|s| u16::from_le_bytes([s[0], s[1]]))`
    填充；扩展原有 canonical test + 新增 2 个 unit test
    （`sub_record_0x0010_leading_word_matches_first_two_payload_bytes_le` 验证
    0x0002 / 0x0003 / 0x4E1C 的 LE 提取；
    `sub_record_0x0010_leading_word_is_none_for_sub_two_byte_payload` 验证
    `Option<>` 契约）。14 parser unit test 全绿。
  - Slice C 给 `DecodedSubRecord0x0010Record` mirror `leading_word: Option<u16>`
    带 `#[serde(default)]`（向后兼容 Phase 18 JSON）；同步 From impl；schema
    needle ratchet 加 `leading_word`；8 schema test 全绿。
  - Slice D 写 cross-fixture ratchet test
    `sub_records_0x0010_leading_word_distribution_matches_phase19_probe`；
    decoder-side 数字与 probe 完全一致：0x0002=164 / 0x0003=21 / 0x0001=18 /
    None=0 / total=582；assertion 锁定 top-3 ranking + None=0 + total=582 +
    0x0002 coverage ≥ 25%。
  - Slice E 跑 Phase 18 既有 ratchet 确认仍 582；跑
    `normalized_geometry_probe_baseline_on_real_fixture` 确认 entity 仍 394。
  - Slice F 在 `CHANGELOG.md` 写 Phase 19 入口（~95 行：RAD sibling null-result
    context + leading_word probe evidence + audit-only 设计选择 + 4 个 "no"
    边界 + Future Work pointer）；`AGENTS.md` 0x0010 段落补 Phase 19 信息。
  - Slice G 跑 5 道 gate：build OK / test 851 lib + 90 integration 0 failed /
    clippy OK / fmt OK / missing-docs current=0 baseline=0。
- **clippy fix**：probe `BTreeMap<usize, (BTreeMap<u8, usize>, BTreeMap<u16, usize>)>`
  触发 `clippy::type_complexity`，抽出 `ByteHist / WordHist / SizeBucket` 类型别名。
- **fmt fix**：`cargo fmt --all` 顺手清理 `probe_rad_siblings_0x0029_0x0035.rs`
  从上次会话遗留的 if-else 单行排版漂移。
- **Phase 19 commit + push**：commit `6beb6f1`
  "feat(parsers,model,schema,tests,docs,examples): Phase 19 — PSM 0x0010
  leading_word audit field"，15 files / 1345 insertions；`git push origin main`
  从 81daa20..6beb6f1 成功。
- **IDA 可达性确认**：调用 `list_instances`，确认 12 个 IDA instance 全
  reachable（含 `radsrvitem.dll` port 13346、`style.dll` 13348、
  `J2DSrv.dll` 13347 等 Phase 16 反向用到的全部）。
- **radsrvitem.dll 起手 survey**：32-bit / base 0x56440000 / 5374 functions
  (4867 unnamed, ~90%) / 1739 strings / exports `GetServerItemTransceiver` /
  `GetServerItemVersion`；预期 Phase 20 PSM dispatch table 反向需要从 unnamed
  function 入手。
- **Phase 20 goal package 起草**：
  `goals/phase20-psm-0x0010-ida-class-identity/` 五件套 + 初始 progress.jsonl
  4 条 entry，总 ~32 KB；明确 scope = 纯 reverse engineering + 文档，
  不改 src/、不改 test；AC1-AC7 覆盖 RAD class identity / sub-kind discriminator
  offset / sub-kind 枚举 / cross-fixture validation / authoritative analysis doc
  / 5 道 gate / progress.jsonl evidence trail。
- **Phase 20 详细中文路线图落地**：
  `docs/plans/2026-05-17-phase20-ida-rad-class-roadmap-cn.md`（11 节、
  Slice A-G 详细分解、备选方案 20-B/C/D、多 session checkpoint 策略、
  风险登记表、验证命令汇总、与既有 plan/goal 文件的关系矩阵、Phase 21+ 预告）。
- 更新 `task_plan.md`：当前阶段切到 Phase 20；新增 Phase 13-20 条目
  （Phase 13-17 引用 `goals/` package + final summary；Phase 18-19 标 complete
  + commit hash；Phase 20 标 awaiting `/goal`）；决策表新增 6 行
  （Phase 13-20 关键 trade-off 解释）。
- 更新 `findings.md`：新增 5 大节 — Phase 14-17 关键结论、Phase 18 audit-only
  landing、Phase 19 leading_word 完整证据、Phase 20 IDA-RAD-class roadmap、
  关键文件补丁清单。
- 通过 best-mcp-sqlite-1 `save_progress` 多次记录跨 session 进度断点。

### 验证
| 检查项 | 结果 |
|---|---|
| `cargo run --release --example probe_rad_siblings_0x0029_0x0035` | 通过；0x29..0x2F + 0x31..0x35 全 0，仅 0x0030=115 |
| `cargo run --release --example probe_psm_0x0010_sub_kind` | 通过；total=578，top word 0x0002=164 (28%) |
| `cargo test --locked -j 4 --lib parsers::sheet_records::tests::sub_record_0x0010 -- --nocapture` | 14/14 通过（12 Phase 18 + 2 Phase 19） |
| `cargo test --locked -j 4 --lib schema` | 8/8 通过 |
| `cargo test --locked -j 4 --test parse_real_files sub_records_0x0010_leading_word_distribution_matches_phase19_probe -- --nocapture` | 通过；0x0002=164 / 0x0003=21 / 0x0001=18 / None=0 / total=582 |
| `cargo test --locked -j 4 --test parse_real_files sub_records_0x0010_decoder_emits_audit_records_with_provenance -- --nocapture` | 通过；total 582，per-fixture 161/104/306/11 |
| `cargo test --locked -j 4 --test parse_real_files normalized_geometry_probe_baseline_on_real_fixture -- --nocapture` | 通过；entities=394 |
| `cargo build --locked --workspace --all-targets` | 通过 |
| `cargo test --locked -j 4 --workspace --all-targets` | 通过；851 lib + 90 integration + 其他小 target，0 failed |
| `cargo clippy --locked -j 4 --workspace --all-targets -- -D warnings` | 通过（修 `BTreeMap` type complexity 后） |
| `cargo fmt --all -- --check` | 通过（`cargo fmt --all` 后） |
| `cargo rustdoc --lib --locked -- -W missing-docs` | 通过；current=0 baseline=0 |
| `git commit + git push origin main` | 通过；81daa20..6beb6f1 |
| `git status` | 干净，只剩 `.superdesign/` + `dlls/` 两个未追踪目录（项目规则禁提交） |
| MCP `user-ida-pro-mcp.list_instances` | 12 个 instance 全 reachable |
| MCP `user-ida-pro-mcp.select_instance(13346) + survey_binary` | radsrvitem.dll 元数据齐全 |
| ReadLints | 编辑过的 6 个源文件 + 2 个 probe + 2 个 markdown 全无 lint 错误 |

### 错误与限制
| 问题 | 处理 |
|---|---|
| 初次 `cargo run --release --example probe_psm_0x0010_sub_kind` 报 "no example target named ..." | cargo 需要先 `cargo build --release --example probe_psm_0x0010_sub_kind` 触发 example 注册，之后 `cargo run` 就能 resolve |
| `cargo clippy --workspace --all-targets -- -D warnings` 在新 probe 上报 `clippy::type_complexity` | 抽出 `type ByteHist = BTreeMap<u8, usize>; type WordHist = BTreeMap<u16, usize>; type SizeBucket = (ByteHist, WordHist);` 三个 type alias 后通过 |
| `cargo fmt --all -- --check` 显示 probe_rad_siblings_0x0029_0x0035.rs（上会话遗留）+ 本会话新 probe 都有排版漂移 | 跑一次 `cargo fmt --all` 一并清理，新 commit 一并带走 |
| `git commit -m "$(cat <<EOF ...)"` PowerShell 不支持 heredoc 语法 | 改写 commit message 到 `.git/COMMIT_EDITMSG.phase19` 临时文件，用 `git commit -F <file>` 提交，提交后删除临时文件 |
| Write 与 Shell 并行调用造成 race condition（git commit 在 Write 写完前跑） | 改成顺序调用：先 Write 完成，再 Shell 调用 git commit；Phase 18→19 commit 流程从此 always 串行 |
| `python "$env:USERPROFILE\.cursor\skills-cursor\planning-with-files\scripts\session-catchup.py"` 路径不存在 | planning-with-files 的 session-catchup 脚本在 `~/.codex/skills/planning-with-files/scripts/` 而不是 `.cursor/skills-cursor/`；本会话直接读 `findings.md` / `progress.md` 替代 catchup |
| `Grep` 在 `dlls/` 目录上报 "os error 32 file in use" | IDA 持有 `.id0` / `.id1` / `.nam` 文件锁，正常现象；后续 grep 加 `-g '!dlls/'` 或限定 path 即可 |

### 决策
| 决策 | 理由 |
|---|---|
| Phase 19 选 leading-word 而非 sibling sweep | RAD sibling probe 5 分钟内证伪原假设（只有 0x0030 有 hits）；leading-word probe 数据强信号（0x0002=28% / 28 个 single-word size bucket），可直接产出 audit-only 字段 |
| Phase 19 audit-only 严格遵守 | Phase 14 GArc2d 错误命名教训重申；字段名 `leading_word` 仅描述字节位置，不描述语义 |
| Phase 19 ratchet 接受 decoder-side ground truth | probe 报 578 / decoder ratchet 报 582，差 4 是 probe-side iter_records 边界处理；decoder 是 source of truth，per-word 数字（164/21/18）完全一致 |
| 用 `#[serde(default)]` 标 `Option<u16>` | 向后兼容 Phase 18 JSON：旧 JSON 没有 `leading_word` 字段，反序列化时默认 None |
| Phase 20 拒绝单 session 执行 | 5374 个 function（4867 unnamed）反向工作量与 Phase 16 量级相当；单 session 必然 lost context，必须按 Slice A-G 拆开 |
| Phase 20 scope 纯 reverse engineering + 文档 | typed sub-kind DTO 实现是 Phase 21 工作；本 phase 严格不改 src/、不改 test，避免 IDA 反向过程产生 half-baked decoder |
| Phase 20 详细路线图独立成 docs/plans/ 文件 | `goals/phase20-…/plan.md` 是紧凑版供 `/goal` 启动；详细路线图（Slice 详解 + 备选方案 + 风险登记 + checkpoint 策略）应在 docs/plans/ 长篇分析 |
| 历史 Phase 13+ 详细计划迁移到 `goals/phaseNN-…/` | 单个 task_plan.md 文件超 200 行失焦；goal package 五件套对 Codex `/goal` 与 Plannotator 更友好；task_plan.md 只保留入口与 status |

### 下一步
- 等用户对 Phase 20 `/goal` 授权或选 20-B / 20-C / 20-D 备选角度。
- 如启 Phase 20：从 Slice A radsrvitem.dll dispatch table 侦察开始，
  每个 Slice append progress.jsonl entry，每 2 Slice 跨 session recap。
- 是否要把 Phase 20 goal package + 详细路线图 + 本 session 三个 planning
  文件作为一次 docs/planning commit 推送（独立于 Phase 19 commit 6beb6f1）。

## Session: 2026-05-18

### 当前状态
- **Phase:** 21 - D06 解析覆盖收敛与关系/Sheet 审计闭环
- **状态:** Phase 21 Slice A-C 已完成，Slice D 按 Slice C 结论跳过，Slice E（gates + docs）已完成。

### 已完成
- 按 Phase 21 计划（`docs/plans/2026-05-18-phase21-d06-parse-coverage-plan-cn.md`）执行 Slice A-E。
- **Slice A** D06 baseline ratchet：新增 `d06_pid_parses_with_expected_structure_and_geometry_summary` 测试，覆盖 D06 全部结构计数与 normalized geometry。
- **Slice B** relationship gap 修复：`build_object_graph` 新增 attribute-fallback 路径，当 `class_id == 0xF6` trailer 产生 0 条 relationship 时，从 `P&IDAttributes` 的 `ModelItemType=Relationship` + `ModelID=Relationship.<GUID>` 提取已被 probe 确认的 GUID，保留为 unresolved `PidRelationship`。D06 现在有 10 objects + 10 unresolved relationships。
- **Slice C** Sheet audit inventory：文档化 `/Sheet6` 的 decoded (25)、audit-only (41)、probe-only (8) evidence 分层，含 GraphicGroup 与 0x0010 样例。
- **Slice D** 跳过：现有 CLI (`--geometry-summary`, `--json`) 足够，不新增 flag。
- **Slice E** 验证与文档：5 道 pre-commit gate 全绿（build / test 1000+ passed 0 failed / clippy / fmt / missing-docs=0）；更新 CHANGELOG、findings、progress。

### 验证
| 检查项 | 结果 |
|---|---|
| `cargo test --test parse_real_files d06_pid_parses -- --nocapture` | 1 passed |
| `cargo test --test parse_real_files relationship -- --nocapture` | 9 passed |
| `cargo build --locked --workspace --all-targets` | 通过 |
| `cargo test --locked --workspace --all-targets` | 1000+ passed, 0 failed |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | 通过 |
| `cargo fmt --all -- --check` | 通过 |
| `cargo rustdoc --lib --locked -- -W missing-docs` | 通过，current=0 baseline=0 |

### 决策
| 决策 | 理由 |
|---|---|
| attribute-fallback 只在 trailer 产生 0 relationships 时触发 | 避免在 DWG fixture 中 double-count trailer-backed relationships |
| D06 relationships 全部 unresolved | 无 Sheet-level `field_x` link，endpoint resolution 延后 |
| 跳过 Slice D CLI 改动 | 现有 `--geometry-summary` / `--json` 足够 D06 分析 |
| `0x0010` / GraphicGroup 继续 audit-only | Phase 20 partial AC 边界不破 |

### 下一步
- 如需把 Phase 21 改动提交，可 commit 并 push。
- 后续可对 D06 做 text-placement regression fixture。
- `0x0010` typed DTO 需等待 Read/DoIO 或 IDA 新证据。

## Session: 2026-05-18 Phase 22 D06 text-placement regression

### 当前状态
- **Phase:** 22 - D06 text-placement regression fixture
- **状态:** 小切片完成，新增 D06 专用回归测试；未改解析器语义。

### 已完成
- 新增 `d06_text_placement_regression_keeps_text_probes_unpromoted`。
- 锁定 D06 `/Sheet6` text-placement 证据：8 个 raw text probes、4 个 decoded `igTextBox`。
- 验证 `text_placement_investigation_report` 只保留 bounded evidence，不把 text probes 提升为 inferred `Text` geometry。

### 验证
| 检查项 | 结果 |
|---|---|
| `cargo test --test parse_real_files d06_text_placement_regression_keeps_text_probes_unpromoted -- --nocapture` | 1 passed |
| `cargo test --test parse_real_files d06_pid_parses_with_expected_structure_and_geometry_summary -- --nocapture` | 1 passed |
| `cargo test --test parse_real_files` | 92 passed, 0 failed |
| `cargo fmt --all -- --check` | 通过 |
| IDE lint (`tests/parse_real_files.rs`) | 无错误 |

### 下一步
- 如需提交，提交范围应只包含 `tests/parse_real_files.rs` 与 `progress.md`。

## Session: 2026-05-18 Phase 20/21/22 status sync

### 当前状态
- **Phase:** 22 - D06 text-placement regression fixture
- **状态:** Phase 20 已按 partial AC 收口；Phase 21/22 已完成；本轮同步顶层计划文件。

### 已完成
- 恢复 best-mcp-sqlite-9 存档，发现旧存档只记录到 Phase 21/22 完成，但未包含后续 Phase 20 partial closeout follow-up docs。
- 核对 git 状态：`main...origin/main` 对齐；开始本轮前工作树干净。
- 确认 Phase 20 follow-up docs 已有提交：
  - `b50ca19 docs(phase20): record 0x0010 metadata recon negative`
  - `68d505f docs(phase20): trace 0x0010 Read DoIO candidates`
  - `d586834 docs(phase20): record 0x0010 external GUID lookup`
- 确认 Phase 20 当前结论：
  - `0x0010` persisted type-table identity confirmed：GUID `1D1928C0-0000-0000-C000-000000000046`，parent alias `0x0115`。
  - metadata / RTTI / registry / external GUID lookup 未恢复 human type name。
  - readonly Read/DoIO tracing 恢复了 `JStyleBase` control path，但未绑定 `1D1928C0...`。
  - typed `0x0010` DTO 仍被 Read/DoIO 与 sub-kind discriminator 证据阻塞，继续 audit-only。
- 同步 `task_plan.md`：
  - 更新“当前阶段”为 Phase 22 complete + Phase 20 partial AC。
  - 将 Phase 20 从 awaiting `/goal` 改为 partial closeout。
  - 补齐 Phase 21 D06 coverage / relationship / Sheet audit 完成状态。
  - 补齐 Phase 22 D06 text-placement regression 完成状态。

### 验证
| 检查项 | 结果 |
|---|---|
| `git status --short --branch` | 当前仅 `task_plan.md` / `progress.md` 有未提交文档变更 |
| `ReadLints task_plan.md` | 无错误 |

### 下一步
- 如需落盘本轮文档同步，可提交 `task_plan.md` 与 `progress.md`。
- 不要从 Phase 20 partial AC 直接实现 typed `0x0010` DTO；除非后续拿到更强 Read/DoIO 或 sub-kind discriminator 证据。
