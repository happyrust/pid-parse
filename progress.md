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

## Session: 2026-05-18 Phase 23 next-step plan

### 当前状态
- **Phase:** 23 - Coordinate/Page Context 收敛与 transform guardrail
- **状态:** 中文开发方案已制定；待执行 Slice A-E。

### 已完成
- 复核 Phase 20/21/22 状态：
  - Phase 20 只达到 partial AC：`0x0010` GUID / persisted type-table identity confirmed，typed DTO 仍 blocked。
  - Phase 21 D06 baseline / relationship fallback / Sheet audit 已完成。
  - Phase 22 D06 text-placement regression 已完成，raw text probes 继续 no-promotion。
- 复核当前坐标上下文基础：
  - `NormalizedPidGeometry.page_dimensions_mm` 已存在。
  - `PidCoordinateContext` / `PidPageTransform` 已是 public geometry contract。
  - `coordinate_page_metadata_investigation_report` 能输出 i32/f64/page-dimension evidence，但仍是 investigation-only。
  - 现有测试保持 transform unavailable guardrail。
- 制定下一步推荐方案：避开 0x0010 Read/DoIO blocker，优先执行 Coordinate/Page Context hardening。
- 新增中文方案文件：
  - `docs/plans/2026-05-18-phase23-coordinate-page-context-plan-cn.md`
- 更新 `task_plan.md`：当前阶段切到 Phase 23，并新增 Slice A-E。
- 更新 `findings.md`：记录 Phase 23 方案结论与边界。

### 推荐执行顺序
| Slice | 目标 |
|---|---|
| A | coordinate context baseline ratchet |
| B | cross-fixture coordinate metadata report 增强 |
| C | transform promotion gate 合同 |
| D | 文档与下游契约同步 |
| E | 预提交门禁 |

### 下一步
- 执行 Phase 23 Slice A：锁定 page dimensions 与 transform unavailable 可同时存在，防止 investigation evidence 被误升为 decoded transform。

### Phase 23 Slice A 实现进展
- 补强 `src/geometry.rs` 文档注释：
  - `NormalizedPidGeometry.page_dimensions_mm` 只是 page-size evidence。
  - `PidPageTransform::Available` 需要 source-proven transform metadata；page dimensions、scalar hits 或 normalized f64 coordinate evidence 都不够。
- 新增 `template_page_dimensions_do_not_make_page_transform_available`：
  - 使用 `DWG-0201GP06-01.pid` 锁定 template-derived A2 page dimensions：`Some((594.0, 420.0))`。
  - 确认 normalized entities 非空。
  - 确认所有 entity 的 `coordinate_context.page_transform` 仍为 `Unavailable`。
  - 确认 warning 继续暴露 coordinate units / page transforms unavailable。
- 更新 `task_plan.md`：Phase 23 Slice A 标记 complete，阶段状态为 in_progress。
- 更新 `findings.md`：记录 page dimensions 不等于 page transform 的 guardrail 结论。

### Phase 23 Slice A 验证
| 检查项 | 结果 |
|---|---|
| `cargo test --locked -j 1 --test parse_real_files template_page_dimensions_do_not_make_page_transform_available -- --nocapture` | 通过 |
| `cargo test --locked -j 1 --test parse_real_files coordinate_page_metadata -- --nocapture` | 通过 |
| `cargo test --locked -j 1 --test parse_real_files non_sheet_stream_page_metadata -- --nocapture` | 通过 |
| `cargo fmt --all -- --check` | 通过 |
| `ReadLints` | 无错误 |

### Phase 23 Slice A 下一步
- 执行 Slice B：增强 cross-fixture coordinate metadata report，输出 top evidence group（marker/range/support/i32/f64/page scalar/example offset/hex prefix）。

### Phase 23 Slice B 实现进展
- 新增 `SheetCoordinatePageMetadataTopEvidence` compact summary：
  - `marker_type`
  - `range_len`
  - `support`
  - `candidate_kind`
  - `candidate_i32_pairs`
  - `candidate_f64_pairs`
  - `normalized_f64_pairs`
  - `page_dimension_scalar_matches`
  - `example_offset`
  - `example_hex_prefix`
- `SheetCoordinatePageMetadataInvestigationReport` 新增 `top_evidence`，从已排序 candidates 中取前 8 个强候选。
- `coordinate_page_metadata_investigation_keeps_transform_unavailable_until_record_proven` 现在输出并断言 top evidence：
  - 非空且最多 8 条。
  - 每条有 bounded offset、hex prefix 和数值证据。
- `sheet_geometry_investigation_aggregates_cross_fixture_evidence_without_promotion` 聚合 `coordinate_top_evidence`，并断言 cross-fixture top evidence 非空。
- 当前 cross-fixture 输出：
  - `fixtures_seen=5`
  - `sheets_seen=7`
  - `coordinate_metadata_candidates=97`
  - `coordinate_top_evidence=36`
  - `normalized_f64_pair_count=1397`
  - `page_dimension_scalar_matches=0`
  - 仍保持 coordinate/page metadata no-promotion。
- 更新 `task_plan.md`：Phase 23 Slice B 标记 complete，阶段状态为 Slice A-B complete。
- 更新 `findings.md`：记录 top evidence 结果。

### Phase 23 Slice B 验证
| 检查项 | 结果 |
|---|---|
| `cargo test --locked -j 1 --test parse_real_files coordinate_page_metadata -- --nocapture` | 通过 |
| `cargo test --locked -j 1 --test parse_real_files sheet_geometry_investigation_aggregates_cross_fixture_evidence_without_promotion -- --nocapture` | 通过 |
| `cargo fmt --all -- --check` | 通过 |
| `ReadLints` | 无错误 |

### Phase 23 Slice B 下一步
- 执行 Slice C：定义并测试 `PidPageTransform::Available` promotion gate 合同。

### Phase 23 Slice C 实现进展
- 补强 `PidPageTransform::Available` 文档注释：
  - 需要 source-proven transform metadata。
  - page dimensions、scalar hits、normalized f64 coordinate evidence 都不够。
  - decoder 必须知道 source coordinate space、units、transform direction 和 bounded byte provenance。
- 新增 `default_coordinate_context_keeps_page_transform_unavailable_until_promoted`：
  - 默认 `PidCoordinateContext` 的 `coordinate_space` 为 `Unknown`。
  - 默认 `page_transform` 为 `Unavailable`。
  - JSON 序列化必须显式输出 `state=unavailable` 和 diagnostic。
- 增强 `schema::tests::normalized_geometry_schema_exposes_graphic_contract`：
  - 锁定 normalized geometry schema 继续暴露 `available`、`origin`、`scale`、`page_bounds`、`matrix`。
- 更新 `task_plan.md`：Phase 23 Slice C 标记 complete，阶段状态为 Slice A-C complete。
- 更新 `findings.md`：记录 transform promotion gate 合同。

### Phase 23 Slice C 验证
| 检查项 | 结果 |
|---|---|
| `cargo test --locked -j 1 --lib geometry::tests::default_coordinate_context_keeps_page_transform_unavailable_until_promoted -- --nocapture` | 通过 |
| `cargo test --locked -j 1 --lib geometry::tests::available_page_transform_json_exposes_bounds_and_matrix -- --nocapture` | 通过 |
| `cargo test --locked -j 1 --lib schema::tests::normalized_geometry_schema_exposes_graphic_contract -- --nocapture` | 通过 |
| `cargo test --locked -j 1 --test parse_real_files coordinate_page_metadata -- --nocapture` | 通过 |

### Phase 23 Slice C 下一步
- 执行 Slice D：同步 `docs/prd-pid-parse-current-state.md` / `docs/architecture-guide.md` / `CHANGELOG.md` 等下游契约文档。

### Phase 23 Slice D 实现进展
- 更新 `docs/prd-pid-parse-current-state.md`：
  - Sheet 深层几何缺口中新增 Phase 23 坐标合同边界。
  - 明确 `page_dimensions_mm` 是 template-derived page-size evidence。
  - 明确 `PidPageTransform` 仍应保持 `Unavailable`，直到找到完整 source record / scalar source / decoded semantics。
  - 明确 H7CAD / 下游不应把 source coordinates 直接当作 page-space 或 viewport pixels。
- 更新 `docs/architecture-guide.md`：
  - 新增 “Normalized Geometry 坐标合同” 小节。
  - 说明 `page_dimensions_mm`、`coordinate_space`、`PidPageTransform` 的分工。
  - 写明 `PidPageTransform::Available` 的最低证据要求。
- 更新 `CHANGELOG.md`：
  - 新增 Phase 23 条目，记录 planning 文件、Slice A-D 代码/测试/文档变更。
- 更新 `task_plan.md`：Phase 23 Slice D 标记 complete，阶段状态为 Slice A-D complete。
- 更新 `findings.md`：记录下游文档同步结论。

### Phase 23 Slice D 下一步
- 执行 Slice E：运行全量预提交门禁，并记录结果。

### Phase 23 Slice E 验证与收口
| 检查项 | 结果 |
|---|---|
| `cargo build --locked --workspace --all-targets` | 通过 |
| `cargo test --locked --workspace --all-targets` | 通过 |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | 通过 |
| `cargo fmt --all -- --check` | 通过 |
| `cargo rustdoc --lib --locked -- -W missing-docs` | 通过 |

### Phase 23 当前结论
- Phase 23 A-E 已完成。
- 可以声明完成的内容：
  - Coordinate/Page Context guardrail 完成。
  - compact top evidence report 完成。
  - `PidPageTransform::Available` promotion gate 合同完成。
  - 下游文档已同步当前坐标语义。
- 不能声明的内容：
  - page transform 已 decoded。
  - H7CAD 已 page-space 对齐。
  - Text/Symbol 已 source-proven promotion。
- 更新 `task_plan.md`：Phase 23 标记 complete。
- 更新 `findings.md`：记录 Slice E 全量门禁通过。

### Phase 23 后续建议
- 若继续实现，推荐 Phase 24：基于 `top_evidence` 选择稳定 marker group，尝试 typed CoordinatePageMetadata decoder；若证据仍不足，则优先 fixture 扩容。

## Session: 2026-05-18 Phase 24 next-step plan

### 当前状态
- **Phase:** 24 - CoordinatePageMetadata decoder 候选筛选
- **状态:** 中文执行方案已制定；待执行 Task 24-01。

### 已完成
- 在 Phase 23 已提交并推送后，按推荐方向制定 Phase 24 方案。
- 新增 planning-skill 风格中文执行文件：
  - `docs/plans/2026-05-18-phase24-coordinate-page-metadata-decoder-plan-cn.md`
- 更新 `task_plan.md`：当前阶段切到 Phase 24，并新增 Task 24-01..04。
- 更新 `findings.md`：记录 Phase 24 方案结论和 evidence gate。

### Phase 24 起点事实
| 事实 | 数值 / 状态 |
|---|---|
| `fixtures_seen` | 5 |
| `sheets_seen` | 7 |
| `coordinate_metadata_candidates` | 97 |
| `coordinate_top_evidence` | 36 |
| `normalized_f64_pair_count` | 1397 |
| `page_dimension_scalar_matches` | 0 |
| `PidPageTransform::Available` | 禁止在本阶段直接 promotion |

### 下一步
- 执行 Task 24-01：生成 candidate marker group evidence table，并落盘
  `docs/analysis/2026-05-18-phase24-coordinate-page-metadata-candidates.md`。

## Session: 2026-05-18 Phase 22 micro + Phase 24 Task 24-01 闭环

### 当前状态
- **Phase:** 22 micro complete (commit `bf4f972`) + 24 Task 24-01 complete (commit `8f3739c`) + Task 24-02 review 选 negative evidence 收口
- **状态:** 三个 commit 全部已 push 到 origin/main；Phase 24 Task 24-03 跳过，Task 24-04 文档同步完成

### 已完成
- Phase 22 micro：把 `test-file/D06.pid` 列入 6 个 Phase 14 cross-fixture
  decoder fixture 数组（Slice E/J/K/L/M/N），按 D06 baseline 锁定
  的计数精准 ratchet 阈值：K +6 / L +10 / M +4 / N +2；E / J 阈值
  不变（D06 贡献 0，作为 parse-package / panic-safety guard）。
- Phase 24 Task 24-01：
  - 新增 `examples/probe_phase24_top_evidence.rs`（307 行）：跨
    5 fixture × 7 sheet 调
    `coordinate_page_metadata_investigation_report`，dump 每个
    `top_evidence` 行的完整字段为 markdown 表格。
  - 新增 `docs/analysis/2026-05-18-phase24-coordinate-page-metadata-candidates.md`
    （231 行）：覆盖 Top 5 候选评估、5 类 rejected 理由、与 Phase 23
    cross-fixture aggregate 的互证、Task 24-02 review 的 A/B/C 路径
    建议、closure_claim_limit 边界声明。
- Phase 24 Task 24-02：用户明确选择 **路径 A negative evidence 收口**，
  确认 Task 24-03 typed candidate DTO 不实现，Phase 23 guardrail
  保留不变。
- Phase 24 Task 24-04：同步 4 份文档
  （`CHANGELOG.md` / `findings.md` / `progress.md` / `task_plan.md`）
  反映上述闭环。

### Phase 24 Task 24-01 关键证据
| 指标 | 数值 |
|---|---:|
| total `top_evidence` 行 | 29 |
| distinct `marker_type` | 25 |
| 行 `page_dimension_scalar_matches > 0` | 0 |
| 行 `normalized_f64_pairs > 0` | 25 (86 %) |
| 跨 ≥ 2 fixture 的 marker | 1（`0x0000`，但 kind 不一致） |
| 跨 fixture 且 kind 一致 stable marker | 0 |
| 已知 unknown marker `0xC03F (49215)` 跨 fixture support | 仅 DWG-0201 |

### Phase 24 Stop-And-Challenge 触发对照
| 条件 | 状态 |
|---|---|
| Top candidate 没有跨 fixture/sheet support | ✅ 触发 |
| `page_dimension_scalar_matches` 继续为 0 | ✅ 触发 |
| 字段解释需要猜单位、方向或 origin | ✅ 触发 |
| 任何实现会让 `PidPageTransform::Available` 出现 | ⏸ 本 Task 未实现 typed DTO，未触发 |

→ 3 / 4 触发，按 Phase 24 plan Task 24-02 `<done>` 选择 negative
evidence 收口。

### 验证
| 检查项 | 结果 |
|---|---|
| `cargo run --release --example probe_phase24_top_evidence` | 通过；输出 29 行 markdown table |
| `cargo build --locked --workspace --all-targets` | 通过 |
| `cargo test --locked --workspace --all-targets` | 通过（59 binaries · 0 failed） |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | 通过 |
| `cargo fmt --all -- --check` | 通过 |
| `cargo rustdoc --lib --locked -- -W missing-docs` | 通过（current=0, baseline=0） |
| `git push origin main` | 通过（用户明确授权） |

### Phase 24 后续触发条件
- 新增 PID fixture 在 **同一 marker** 上出现 **kind 一致** 的
  `top_evidence`，且至少 1 行 `page_dimension_scalar_matches > 0`
  时，重启 Task 24-03 typed candidate DTO 路径。
- 不在 0x0010 RAD class IDA 证据补足前推进 typed `0x0010` DTO。

## Session: 2026-06-03 Phase 26 PID 文件全格式分析说明计划

### 当前状态
- **Phase:** 26 - PID 文件全格式分析说明
- **状态:** Phase 26 完成；coverage/byte-audit JSON 快照因 fixture 缺失未生成

### 已完成
- 按用户要求结合 `grill-me` 思路制定计划；本地 `planning-with-files`
  skill 文件未找到，沿用仓库既有三件套工作流：
  `task_plan.md` / `progress.md` / `findings.md`。
- 复核当前格式说明相关事实源：
  - `AGENTS.md`
  - `README.md`
  - `docs/prd-pid-parse-current-state.md`
  - `docs/sppid/v0.10.x-status.md`
  - `docs/architecture-guide.md`
  - `docs/format-notes.md`
  - `src/inspect/mod.rs`
  - `src/inspect/coverage.rs`
  - `src/byte_audit/aggregate.rs`
  - `src/parsers/*` 中 PSM / cluster / DA / Sheet endpoint / DocVersion / AppObject / JTaggedTxtStgList 相关实现
- 新增计划文件：
  `docs/plans/2026-06-03-phase26-pid-file-format-analysis-plan-cn.md`。
- 在 `task_plan.md` 登记 Phase 26，并拆分为：
  - Phase 26-A：事实源审计
  - Phase 26-B：格式地图主文档
  - Phase 26-C：验证与快照
  - Phase 26-D：文档交叉链接与收口
- 完成 Phase 26-A 事实源审计口径：
  - known top-level stream/storage 以 `src/inspect/mod.rs` 为当前 registry。
  - coverage 等级以 `src/inspect/coverage.rs` 为准。
  - byte-audit registered parser 以 `src/byte_audit/aggregate.rs` 为准。
  - `docs/format-notes.md` 是早期简版，当前不再足以表达 Phase 14+ Sheet decoder / audit-only / guardrail 状态。
- 完成 Phase 26-B 主文档草案：
  `docs/analysis/2026-06-03-pid-file-format-analysis-cn.md`。
  文档覆盖 CFB 外层、top-level stream/storage、magic/signature、主要 stream 字节布局、DA relationship 证据链、Sheet decoded/audit/probe 分层、下游模型映射、验证方法与未知区。
- 执行 Phase 26-C fixture 可用性检查：
  - `Glob(test-file/**/*.pid)`：0 个文件。
  - `Glob(repo/**/*.pid)`：0 个文件。
  - `git status --short --ignored -- test-file`：无 ignored fixture 输出。
  - 因此未运行 `pid_inspect --coverage --json` / `--byte-audit --json`，也未生成 JSON 快照。
- 已将上述阻塞原因写入主文档 §12。
- 完成 Phase 26-D：
  - README 文档入口新增新版格式说明链接。
  - `docs/format-notes.md` 标记为早期简版，并指向新版格式地图。
  - `task_plan.md` / `progress.md` / `findings.md` 同步 Phase 26 收口状态。

### Grill-Me 决策结果
| 问题 | 推荐答案 |
|---|---|
| 说明文档是规范还是当前实现说明？ | 当前实现说明 + 证据等级，不写成最终规范 |
| 是否覆盖 publish XML / MDF？ | 放入附录，不并入 `.pid` 主格式说明 |
| Sheet 几何是否写成完整 decoded？ | 只对 typed decoder 家族写 decoded，audit/probe 继续保守标注 |
| 是否需要真实 fixture 快照？ | 需要，但作为验证附录，不阻塞主文档草案 |
| 粒度到哪里为止？ | 到 stream/record-family 级，不逐 byte 穷举未知 payload |

### 验证
| 检查项 | 结果 |
|---|---|
| 读取 `grill-me` skill | 通过 |
| 定位 `planning-with-files` skill 文件 | 未找到；沿用仓库既有 planning 三件套 |
| 计划文件创建 | 通过 |
| 主格式说明创建 | 通过 |
| `.pid` fixture 搜索 | 未找到可用样本 |
| `git status --short --ignored -- test-file` | 无输出 |
| README / format-notes 入口更新 | 通过 |
| 代码测试 | 未执行；本轮仅制定计划与更新 Markdown |

### 下一步
- 后续恢复真实 `.pid` fixture 后，再补跑 Phase 26-C JSON 快照。

## Session: 2026-06-03 Phase 27 IDA 证据驱动的 PID 全数据类型提取

### 当前状态
- **Phase:** 27 - IDA 证据驱动的 PID 全数据类型提取
- **状态:** Phase 27-A 已启动；`radsrvitem.dll` type-code mapper 已恢复初版，数据类型矩阵已落地

### 已完成
- 使用 `ida-pro-mcp list_instances` 确认当前可用 IDA 实例：
  - `core.dll` at `127.0.0.1:13337`
  - `radsrvitem.dll` at `127.0.0.1:13338`
- 选择 `radsrvitem.dll` 作为 Phase 27 起点。
- survey `radsrvitem.dll`：
  - 32-bit。
  - base `0x56440000`。
  - 5374 functions，1739 strings。
  - exports `GetServerItemTransceiver` / `GetServerItemVersion`。
  - 关键字符串包括 `igTextBox`、`igLine2d`、`igSmartFrame2d`、`CLSID`、`XCeedRAD.dll`。
- 新增 Phase 27 计划：
  `docs/plans/2026-06-03-phase27-ida-driven-pid-data-type-extraction-plan-cn.md`。
- 通过 IDA 搜索定位首批关键函数：
  - `sub_56448F70`：`u16 type_code -> SmartPlant/IGDS type name` mapper。
  - `sub_564468B0`：`igTextBox` 提取/属性写入候选函数。
- 使用 IDA Python 解析 `sub_56448F70` 的 Hex-Rays 伪代码，导出 30 条 IDA-confirmed type name：
  - 27 条 switch case。
  - `0x0115 igDimension`、`0x0117 igBalloon`、`0x0118 igLeader` 三条 if/else return。
- 确认 `0x00CE = igSymbol2d`，补强当前 parser 对 `decode_igsymbols` 的名称证据。
- 新增数据类型矩阵初版：
  `docs/analysis/2026-06-03-phase27-pid-data-type-matrix-cn.md`。
- 矩阵已将 IDA type table 与当前 `src/parsers/sheet_records.rs` 对齐：
  - 已覆盖：`igLine2d`、`igLineString2d`、`igPoint2d`、`igTextBox`、`igSymbol2d`。
  - 当前 parser 已覆盖但不在该 IGDS mapper 表中：`0x0010`、`0x0030 JStyleOverride`、`0x00FA GraphicGroup`、`0x3FE6 GLine2d`。
- 继续执行 Phase 27-B `igTextBox` 样板：
  - 阅读 `decode_igtextbox_at`，确认 Rust raw Sheet decoder 当前使用 PSM 6-byte header、payload offset 30 的 `text_length`、payload offset 32 的 UTF-16LE text，以及 text 后 3 个 f64。
  - 分析 `sub_56445F40`，确认 dispatch：`0x004D -> sub_564468B0`、`0x00FA -> sub_56446020`、`0x003D -> sub_564464D0`、default -> `sub_564462F0`。
  - 导出 `sub_564468B0`、`sub_56449240`、`sub_56447710`、`sub_56447730` 完整伪代码，确认 `igTextBox` 有 3 种 runtime text layout。
  - 更新数据类型矩阵 §8，加入 `igTextBox` 的 `IDA reader ↔ Rust decoder` 对照表。
  - 结论：type identity / UTF-16LE text / `TEXT` property match；raw offset、trailing f64、oid/parent/index 仍未由该 IDA function 直接确认。
- 继续执行 Phase 27-B 默认路径调查：
  - 分析 `sub_564462F0`，确认 default IGDS path 只做 type-name lookup、RAD object property creation 与 `"RELEATIONS"` 写入。
  - 未观察到 line/point/polyline/symbol/circle/arc/ellipse 的几何字段读取。
  - 搜索 `igPoint2d`、`igLineString2d`、`igSymbol2d`、`igCircle2d`，当前 `radsrvitem.dll` 中均只命中 `sub_56448F70` mapper 与 `.rdata` 字符串本体。
  - 更新数据类型矩阵 §9，记录 `radsrvitem.dll` 对普通几何字段的 negative evidence。
- 继续追 `sub_56445F40` 的 `v10` record pointer 来源：
  - 确认 `v10` 来自 `this+0x3c` runtime record manager 的 vtable `+0xA4` lookup，调用形态为 `(manager, record_id, 0x40, &record_ptr)`。
  - 分析 `sub_5644B640`，确认它有两种模式：`a3 == 0` 遍历 type=1 section payload 的 6-byte stride record-id list 并筛 `*(u16*)record == 0x0089`；`a3 == 1` 直接解析外部 `record_id`。
  - 分析 `sub_564546F0` / `sub_56454A20` / `sub_56454880` / `sub_56454860` / `sub_56455130`，恢复 runtime section 编码：低 7 bit section type，高 bit list-end；普通/扩展长度决定 payload 从 `section+2` 或 `section+4` 开始。
  - 更新数据类型矩阵 §10，记录 `sub_564468B0` 处理的是 runtime record layout，不直接证明 Rust raw `Sheet*` decoder 的 `payload+30 text_length` / `payload+32 text` / trailing f64 offset。
- 继续定位 `this+0x3c` record manager vtable `+0xA4` 具体实现：
  - `sub_56445C90` 对 `PersistManager` QueryInterface，IID `{1FC155A0-6BE3-101B-97A9-08003601CDC9}` 填入 `this+0x3c`；`sub_56467810` 构造函数显示这是 `ImpIJPersistManager::vftable`。
  - `ImpIJPersistManager::vtable+0xA4 = sub_56468DB0`；核心 `sub_56468DF0` 将 `record_id` 拆为 `record_id >> 13` segment/bucket 与 `record_id & 0x1FFF` descriptor index。
  - `sub_56468DF0` 通过 `sub_5648C0F0` 选择 page/segment entry，并调用 entry handler vtable `+0x70` materialize record。
  - `SerialCluster::vtable+0x70 = sub_56493F50`；该函数最终 `out_record_ptr = serial_cluster_base + record_descriptor[0]`。
  - `sub_56495BD0` 在页未加载时按 4KB page 通过 stream `Seek` / `Read` 映射 bytes，并用 `out_record_ptr+2` 的 record length 判断是否跨页。
  - 继续分析 `SerialCluster` offset 公式：`sub_56493BC0` 反向返回 `ptr - serial_cluster_base`；`sub_56494C40` / `sub_56495BD0` stream IO 同样以 `page_or_record_ptr - serial_cluster_base` 作为 Seek offset。
  - 结论：在 `SerialCluster` 层，`stream_offset = runtime_record_ptr - serial_cluster_base = record_descriptor[0]`；但还没把该 `SerialCluster` stream object 精确绑定回 CFB `Sheet*` stream path。
  - 更新数据类型矩阵 §10.4-10.6、`findings.md` 与 `task_plan.md`，将下一步收敛为追 `SerialCluster` stream object 与 CFB `Sheet*` stream path 的绑定关系。
- 继续追 `SerialCluster` stream object 的 CFB 绑定：
  - `ImpIPersistStorage::vtable+0x18 = sub_56469BF0`，对应 COM `IPersistStorage::Load(IStorage*)`。
  - `sub_56469BF0` 从传入的 CFB `IStorage` 打开 `PSMclustertable`、`PSMroots`、`PSMspacemap`、`PSMcluster0`。
  - `sub_56491090` 是 `IStorage::OpenStream` wrapper；`sub_56491150` 是 `IStorage::OpenStorage` wrapper。
  - 保存侧 `sub_56469950` / `sub_5646AE30` 通过 `sub_56490B30` / `sub_56490BF0` 创建同名 PSM streams/storages。
  - 结论：当前 `radsrvitem.dll` 链路绑定到 CFB 根 storage 下的 PSM 持久化 streams，不是直接 `Sheet*` raw stream；下一步应追 PSM runtime record 到 `Sheet*` raw geometry 的投影关系，或切到其它 DLL 找直接 geometry reader。
- 复查现有 `core.dll` IDA instance：
  - 搜索 `sheet` / `psm` / `igTextBox` / `igLine2d` / `igPoint2d` / `OpenStream` 等字符串与名字。
  - 命中 `ASHEET` / `DSHEET`、`CMPTSZ` 的 `Co-ordinates refer to sheet token`、以及若干 `psm` 命令菜单文本。
  - 分析 `sub_5655550` / `sub_569C5F0`，确认只是 `DB_Attribute("ASHEET"/"DSHEET")` 初始化。
  - 分析 `CMPTSZ`，确认它是 sheet token 坐标调试/命令输出路径，不是 PID raw `Sheet*` record reader。

### 验证
| 检查项 | 结果 |
|---|---|
| `ida-pro-mcp list_instances` | 通过；2 个实例 reachable |
| `ida-pro-mcp select_instance` | 通过；已选 `radsrvitem.dll` |
| `ida-pro-mcp survey_binary` | 通过；确认 binary metadata |
| `search_text("igTextBox")` | 命中 `sub_564468B0` 与 `sub_56448F70` |
| `search_text("igLine2d")` | 命中 `sub_56448F70` |
| `analyze_function("sub_56448F70")` | 通过；反编译结果有 truncation，但足以确认首批 mapping |
| `py_eval` 解析 Hex-Rays 伪代码 | 通过；导出 30 条 type-name mapping |
| `analyze_function("sub_564468B0")` | 通过；确认 `igTextBox` 文本 payload 与 `"TEXT"` 属性写入路径 |
| `analyze_function("sub_56445F40")` | 通过；确认 per-record dispatch 到 `sub_564468B0` |
| `py_eval` 导出 `igTextBox` helpers | 通过；确认 mode 1/2/3 文本 payload reader |
| `analyze_function("sub_564462F0")` | 通过；确认 default path 不读取几何字段 |
| P0/P1 type name `search_text` | 通过；多数仅命中 mapper，未发现普通几何 reader |
| `analyze_function("sub_5644B640")` | 通过；确认 record-id list 遍历与 `sub_56445F40` 指定 record 解析两种模式 |
| `analyze_function("sub_56455240")` | 通过；确认 type=1 section payload 来源 |
| `analyze_function("sub_564546F0" / section helpers)` | 通过；恢复 runtime section type/length/payload 编码 |
| `analyze_function("sub_56445C90" / "sub_56467810")` | 通过；确认 `this+0x3c` 来自 `ImpIJPersistManager` QueryInterface |
| `analyze_function("sub_56468DB0" / "sub_56468DF0")` | 通过；确认 `record_id` 到 descriptor / page entry / handler materialization 链 |
| `analyze_function("sub_56493F50")` | 通过；确认 `SerialCluster` materializer 输出 cluster base + descriptor offset |
| `analyze_function("sub_56495BD0")` | 通过；确认 page-backed Seek/Read 加载逻辑 |
| `analyze_function("sub_56493BC0" / "sub_56494C40")` | 通过；确认 `ptr - base` 与 stream Seek offset 公式 |
| `analyze_function("sub_56469BF0")` | 通过；确认 `ImpIPersistStorage::Load` 打开 PSM streams |
| `analyze_function("sub_56491090" / "sub_56491150")` | 通过；确认 OpenStream / OpenStorage wrapper |
| `analyze_function("sub_56469950" / "sub_5646AE30")` | 通过；确认保存侧创建同名 PSM streams/storages |
| `ida-pro-mcp list_instances` | 通过；当前只有 `core.dll` 与 `radsrvitem.dll` reachable |
| `py_eval` 搜索 `core.dll` Sheet/PSM/IGDS 字符串与名字 | 通过；未发现 PID raw record reader 命名证据 |
| `analyze_function("sub_5655550" / "sub_569C5F0" / "CMPTSZ")` | 通过；确认 `core.dll` 线索偏数据库属性/命令输出 |

### 下一步
- 继续追 PSM runtime record 到 `Sheet*` raw geometry 的投影关系，确认是否存在 envelope/header offset。
- 如果当前 `radsrvitem.dll` 继续无法提供普通几何字段 reader，需要让 IDA 打开/选择 `J2DSrv.dll`、`style.dll`、`sppid.dll`、`XCeedRAD.dll` 或 `smartplantpid.exe`。

## Session: 2026-06-08 Phase 28 Spec Kit 风格 PID 文件全格式规格包

### 当前状态
- **Phase:** 28 - Spec Kit 风格 PID 文件全格式规格包
- **状态:** 规格包初版已落地；live IDA refresh 等待相关 IDA 模块 / tool descriptor 可用

### 已完成
- 根据用户要求“使用 spek kit 的方式，整理 PID 文件所有格式，并结合 ida-pro-mcp”，将现有 Phase 26/27 事实源组织成 Spec Kit 风格规格包。
- 新增目录：
  `docs/specs/2026-06-08-pid-file-format-spec-kit/`
- 新增 `spec.md`：
  - 定义目标、用户故事、证据等级、功能需求、guardrails、验收标准。
- 新增 `plan.md`：
  - 定义 Phase 28-A..E，覆盖 inventory consolidation、IDA evidence refresh、format entry completion、promotion backlog、verification。
- 新增 `research.md`：
  - 汇总 parser 已稳定能力、Sheet/PSM decoder 事实、Phase 27 IDA 证据、fixture 限制、`0x0010` / JStyle 阻塞。
- 新增 `data-model.md`：
  - 用 evidence-graded inventory 覆盖 container / metadata / registry / PSM / Sheet type-code / IDA-confirmed 未覆盖类型 / derived geometry / writer-publish 边界。
- 新增 `tasks.md`：
  - 拆分后续 IDA availability check、IDA evidence refresh、fixture snapshot、format completion backlog 与 validation tasks。
- 新增 `quickstart.md`：
  - 记录 parser inspection、coverage / byte-audit、focused tests、full gates 与 IDA workflow。
- 更新 `task_plan.md`：
  - 当前阶段切到 Phase 28。
  - 增加 Phase 28 条目与规格包文件清单。
- 更新 `findings.md`：
  - 记录 Phase 28 规格包定位、IDA 结合方式、当前 live IDA 调用限制与 guardrails。
- 已读取 `planning-with-files` skill，并沿用仓库既有 `task_plan.md` / `findings.md` / `progress.md` 三件套。

### IDA / MCP 限制
- 本轮尝试定位 `user-ida-pro-mcp` tool descriptor。
- `C:\Users\dpc\.cursor\projects\d-work-plant-code-cad-pid-parse\mcps\user-ida-pro-mcp\tools` 不存在。
- 该 MCP 目录下仅发现 `SERVER_METADATA.json`，未发现可读取的 tool schema。
- 按 MCP 使用规则，未读取到 tool schema 前不直接调用 `ida-pro-mcp` 工具；因此本规格包使用已落盘 Phase 27 与 `load_progress` 返回的 IDA 证据，没有新增 live IDA 取证。

### 关键结论
- 当前格式整理应以“当前实现说明 + 证据等级 + re-open trigger”为口径，不写成官方最终规范。
- `radsrvitem.dll` 已能支撑 type identity / `igTextBox` runtime reader 样板 / default IGDS negative evidence / SerialCluster runtime pointer 链。
- 继续 `0x0010` / JStyle 深层字段语义，需要打开 `style.dll`、`J2DSrv.dll`、`sppid.dll`、`XCeedRAD.dll` 或其它含 JStyle/RAD host 实现的 IDB。
- 在新 IDA 证据前，禁止把 `0x0010.leading_word` 命名为 `sub_kind`，禁止把 `0x00FA GraphicGroup` raw tail 命名为 child OID list，禁止让 `PidPageTransform::Available` 出现。

### 验证
| 检查项 | 结果 |
|---|---|
| 读取 `planning-with-files` skill | 通过 |
| 读取 Phase 26 / 27 相关事实源 | 通过 |
| 创建 Spec Kit 规格包文件 | 通过 |
| 更新 `task_plan.md` / `findings.md` / `progress.md` | 通过 |
| live `ida-pro-mcp` 调用 | 未执行；tool descriptor 缺失，按规则不调用 |

### 下一步
- 若用户打开 / 提供 `style.dll`、`J2DSrv.dll`、`sppid.dll`、`XCeedRAD.dll` 等相关 IDA instance，并且 tool descriptor 可读，则执行 Phase 28-C/D 的 IDA evidence refresh。
- 如恢复真实 `.pid` fixture，可执行 Phase 28-E coverage / byte-audit snapshot。

### Phase 28-E/F 继续执行进展
- 用户要求“按推荐方案继续下一步”后，先执行不依赖 live IDA 的分支：
  - fixture availability check。
  - representative coverage / byte-audit snapshot。
  - format completion classification。
- `Glob` / `rg` 在当前 Windows 工作区对 `.pid` glob 返回 `os error 3`，改用 PowerShell `Get-ChildItem -Recurse -Filter *.pid -File` 只枚举文件名。
- 当前实际发现 6 个 `.pid` fixture：
  - `test-file/工艺管道及仪表流程-1.pid`
  - `test-file/D06.pid`
  - `test-file/DWG-0201GP06-01.pid`
  - `test-file/DWG-0202GP06-01.pid`
  - `test-file/export-test/publish-data/A01/A01.pid`
  - `test-file/export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid`
- 已用 `test-file/D06.pid` 生成代表性快照：
  - `docs/specs/2026-06-08-pid-file-format-spec-kit/d06-coverage.json`
  - `docs/specs/2026-06-08-pid-file-format-spec-kit/d06-byte-audit.json`
- D06 coverage JSON 摘要：
  - `coverage_entries=26`
  - `FullyDecoded=7`
  - `PartiallyDecoded=6`
  - `IdentifiedOnly=13`
  - `TopLevelStream=13`
  - `TopLevelStorage=13`
- D06 byte-audit JSON 摘要：
  - `traces=23`
  - `per_stream_count=56`
  - `unregistered_paths_count=33`
  - `total_file_bytes=69579`
  - `overall_consumed=6122`
  - `overall_leftover=63457`
  - `overall_coverage_ratio=0.08798632`
- 已更新规格包：
  - `data-model.md` 新增 D06 snapshot section 与 completion classification。
  - `tasks.md` 将 Phase 28-E 的 fixture locate / D06 coverage / D06 byte-audit 标为完成，将 Phase 28-F 的分类任务标为完成。
  - `quickstart.md` 增加 D06 snapshot 文件路径与生成命令。
- 已更新根目录 planning 文件：
  - `task_plan.md` Phase 28 状态改为 spec package + D06 snapshot + completion classification complete。
  - `findings.md` 记录 6 个 fixture、D06 快照摘要、completion classification。

### Phase 28-E/F 验证
| 检查项 | 结果 |
|---|---|
| `Get-ChildItem -Recurse -Filter *.pid -File` | 通过，发现 6 个 fixture |
| `cargo run --bin pid_inspect -- test-file/D06.pid --coverage --json` | 通过，输出 `d06-coverage.json` |
| `cargo run --bin pid_inspect -- test-file/D06.pid --byte-audit --json` | 通过，输出 `d06-byte-audit.json` |
| JSON 摘要提取 | 通过 |

### Phase 28-E batch snapshot 继续执行
- 按用户再次要求“按推荐方案继续下一步”，将 D06 代表性快照扩展到全部 6 个本地 `.pid` fixture。
- 新增生成的 snapshot 文件：
  - `nonascii-process-1-coverage.json`
  - `nonascii-process-1-byte-audit.json`
  - `dwg0201-coverage.json`
  - `dwg0201-byte-audit.json`
  - `dwg0202-coverage.json`
  - `dwg0202-byte-audit.json`
  - `publish-a01-coverage.json`
  - `publish-a01-byte-audit.json`
  - `publish-dwg0202-coverage.json`
  - `publish-dwg0202-byte-audit.json`
- 6-fixture snapshot summary：
  - `d06`: entries=26, Fully=7, Partial=6, Identified=13, ratio=0.08798632
  - `nonascii-process-1`: entries=25, Fully=6, Partial=6, Identified=13, ratio=0.042824525
  - `dwg0201`: entries=37, Fully=7, Partial=6, Identified=24, ratio=0.10965584
  - `dwg0202`: entries=31, Fully=7, Partial=6, Identified=18, ratio=0.09147366
  - `publish-a01`: entries=22, Fully=7, Partial=6, Identified=9, ratio=0.062267642
  - `publish-dwg0202`: entries=31, Fully=7, Partial=6, Identified=18, ratio=0.091408126
- 已更新规格包：
  - `data-model.md` 将 D06 snapshot section 改为 6-fixture snapshot matrix。
  - `tasks.md` 记录全部 12 个 snapshot JSON 文件。
  - `quickstart.md` 记录 6 个 fixture id 与源路径。
- 已更新根目录：
  - `task_plan.md` Phase 28-E 改为全部 6 fixture snapshot complete。
  - `findings.md` 记录 6-fixture snapshot matrix 范围。

### Phase 28-E batch snapshot 验证
| 检查项 | 结果 |
|---|---|
| 5 个剩余 fixture coverage / byte-audit 生成 | 通过 |
| 6-fixture JSON 汇总 | 第一次 Python one-liner 因 PowerShell/Python 引号组合 SyntaxError；改用简单 `join(map(str, ...))` 后通过 |

### Phase 28-E/F 下一步
- live IDA refresh 仍等待 tool descriptor 与相关 IDB。
- 若要继续不依赖 IDA 的工作，可把 6-fixture snapshot matrix 转成更细的 per-stream delta / priority backlog。

### Phase 28-G Snapshot Priority Backlog 进展
- 按用户再次要求“按推荐方案继续下一步”，从 6-fixture byte-audit JSON 生成 per-stream / per-family priority backlog。
- 新增文件：
  - `docs/specs/2026-06-08-pid-file-format-spec-kit/snapshot-priority-backlog.md`
- 生成内容包括：
  - fixture summary。
  - highest leftover families。
  - highest leftover individual paths。
  - common unregistered paths。
  - recommended backlog。
  - re-open triggers。
- highest-leftover families 当前排序：
  1. `JSite*`: leftover 326,403 bytes, ratio 0.04438140。
  2. `PSMcluster0`: leftover 193,173 bytes, ratio 0.00417562。
  3. `Sheet*`: leftover 121,176 bytes, ratio 0.06886536。
  4. `Unclustered Dynamic Attributes`: leftover 111,120 bytes, ratio 0.22326840。
  5. `StyleCluster`: leftover 83,468 bytes, ratio 0.00114882。
  6. `PSMspacemap`: leftover 62,802 bytes, ratio 0。
- 生成后发现 markdown 中存在控制字符路径（如 `\x01Ole`），已用脚本转义为 `\\x01` / `\\x05` 可读形式。
- 更新：
  - `plan.md` 规格包文件清单新增 `snapshot-priority-backlog.md`。
  - `tasks.md` 新增 Phase 28-G 并标记 aggregate / rank / unregistered / file generation 完成。
  - `task_plan.md` Phase 28-G 标记完成。
  - `findings.md` 记录 backlog 核心排序。

### Phase 28-G 验证
| 检查项 | 结果 |
|---|---|
| 解析 `per_stream` JSON 结构 | 通过；发现其为 path-indexed object |
| 生成 `snapshot-priority-backlog.md` | 通过 |
| 控制字符转义 | 通过 |

### Phase 28-G 下一步
- 下一步若继续非 IDA 路线，可将 priority backlog 转成 Phase 29 候选 slices。
- 若继续 IDA 路线，仍需打开相关 IDB 与 tool descriptor。

### Phase 28-H Phase 29 Candidate Slices 进展
- 按用户再次要求“按推荐方案继续下一步”，将 `snapshot-priority-backlog.md`
  转为 Phase 29 候选 implementation slices。
- 新增文件：
  - `docs/specs/2026-06-08-pid-file-format-spec-kit/phase29-candidate-slices.md`
- 候选 slices：
  - 29-A Sheet stream delta and unknown record prioritization：推荐为 IDA 仍阻塞时的第一非 IDA slice；只做 unknown record family 排序，不改变 parser 语义。
  - 29-B PSMcluster0 body triage：解释 `/PSMcluster0` 高 leftover，判断 parser-only 还是 IDA-backed。
  - 29-C Dynamic Attributes deep body backlog：只处理能改善 object / relationship 语义的字段。
  - 29-D PSMspacemap `tseg` evidence gate：blocked，需 IDA / controlled fixture evidence。
  - 29-E JSite symbol-instance demand gate：leftover 最大但需下游指名需求。
  - 29-F IDA module enablement：打开 `style.dll` / `J2DSrv.dll` / `sppid.dll` / `XCeedRAD.dll` / `smartplantpid.exe` 并恢复 tool descriptor。
- 更新：
  - `plan.md` 规格包文件清单新增 `phase29-candidate-slices.md`。
  - `tasks.md` 新增 Phase 28-H 并标记 Phase 29-A..F 候选切片完成。
  - `task_plan.md` Phase 28-H 标记完成。
  - `findings.md` 记录推荐分支：IDA 阻塞时执行 29-A；IDA 可用时执行 29-F。

### Phase 28-H 下一步
- 如果继续非 IDA 路线，执行 29-A，生成 Sheet leftover unknown record family priority report。
- 如果用户打开相关 IDB / tool descriptor 可用，执行 29-F，恢复 live IDA evidence refresh。

## Session: 2026-06-08 Phase 29-A Sheet leftover priority report

### 当前状态
- **Phase:** 29 - Sheet leftover unknown record priority
- **状态:** 29-A priority report complete；bounded Sheet leftover window extractor 未开始

### 已完成
- 按用户再次要求“按推荐方案继续下一步”，执行 Phase 29-A 的第一步：从 6-fixture byte-audit snapshot 生成 Sheet leftover priority report。
- 新增分析文档：
  `docs/analysis/2026-06-08-phase29-sheet-leftover-priority.md`
- 报告内容：
  - executive summary。
  - aggregate Sheet paths。
  - top Sheet leftover items。
  - proposed 29-A work items。
  - guardrails。
  - acceptance for next slice。
- 关键结果：
  - `/Sheet6` 是主导 registered Sheet hotspot：6 fixtures，total 129,506，consumed 8,818，leftover 120,688，ratio 0.06808951。
  - `/JSite204/Sheet6` 是 publish A01 嵌套 unregistered hotspot：leftover 6,870。
  - `/Sheet6615` 在 DWG fixture 和 publish DWG fixture 出现：2 fixtures，leftover 488。
  - Top 4 leftover items 都是 `/Sheet6`。
- 已更新：
  - `task_plan.md` 当前阶段切换为 Phase 29。
  - `docs/specs/.../tasks.md` 新增 Phase 29-A 任务。
  - `findings.md` 记录 Phase 29-A 关键结论。

### Phase 29-A 验证
| 检查项 | 结果 |
|---|---|
| Sheet path byte-audit listing | 通过 |
| 生成 `docs/analysis/2026-06-08-phase29-sheet-leftover-priority.md` | 通过 |

### Phase 29-A 下一步
- 构建 bounded Sheet leftover window extractor。
- 将 `/Sheet6` leftover ranges 转为 bounded source byte windows。
- 按 candidate PSM type code / bytes-to-follow / size bucket / marker bytes 分组。
- 映射每组到 typed / audit-only / probe-only / unknown parser status。
- 继续禁止字段命名和 parser semantics promotion。

### Phase 29-A bounded window extractor 进展
- 按用户再次要求“按推荐方案继续下一步”，实现 bounded Sheet leftover window extractor。
- 新增 read-only example：
  - `examples/probe_phase29_sheet_leftover_windows.rs`
- 该 example 使用：
  - `PidParser::parse_package` 读取 raw stream bytes。
  - `byte_audit_report` 获取 Sheet-related leftover ranges。
  - 对每个 range 输出最多 96 字节 bounded source window。
  - 在 range 前 32 字节内尝试识别保守的 PSM-like header，仅作为 local shape 分组。
- 首次运行 `cargo run --example probe_phase29_sheet_leftover_windows` 编译失败：
  - Rust error `E0382 borrow of moved value: path`。
  - 原因：在 leftover range loop 中将 `path: String` move 进 `group.paths.entry(path)`，后续迭代仍需借用。
  - 修复：改为 `group.paths.entry(path.clone())`。
- 重新运行通过，并生成：
  - `docs/analysis/2026-06-08-phase29-sheet-leftover-windows.md`
- 当前 top local byte-shape groups：
  - #1 `top-level Sheet / 0x0001 unknown / btf 1024+ / prefix 01 00 FB FF`，leftover 16,760，4 fixtures。
  - #2 `top-level Sheet / 0x0002 unknown / btf 1024+ / prefix 01 00 01 00`，leftover 10,787，3 fixtures。
  - #3 `top-level Sheet / 0x0001 unknown / btf 0512-1023 / prefix 01 00 01 00`，leftover 10,537，3 fixtures。
  - #4 `top-level Sheet / 0x0005 unknown / btf 1024+ / prefix 01 00 01 00`，leftover 9,687，3 fixtures。
  - #5 nested `JSite204/Sheet6` cluster-like prefix `44 F5 90 6C`，leftover 6,870。
- 更新：
  - `docs/specs/.../tasks.md` Phase 29-A 标记 extractor/report 完成，保留 manual decoder review。
  - `task_plan.md` Phase 29-A 标记 extractor 完成，下一步是 manual decoder review。
  - `findings.md` 记录 top local byte-shape groups 与保守解释。

### Phase 29-A bounded window extractor 验证
| 检查项 | 结果 |
|---|---|
| `cargo fmt --all` | 通过 |
| `cargo run --example probe_phase29_sheet_leftover_windows` | 首次 E0382，修复后通过 |
| `docs/analysis/2026-06-08-phase29-sheet-leftover-windows.md` | 已生成 |
| `cargo fmt --all -- --check` | 通过 |
| `ReadLints` | 无错误 |

### Phase 29-A bounded window extractor 下一步
- 人工 review top groups 与现有 typed / audit-only decoder 的对应关系。
- 将每组分类为 parser-only / NeedsIDA / Blocked。
- 不在本阶段命名字段或 promotion。

### Phase 29-A manual review 进展
- 按用户再次要求“按推荐方案继续下一步”，执行 top groups manual review。
- 读取 `src/byte_audit/aggregate.rs` 后确认：
  - top-level Sheet streams 当前由 `probe_sheet_stream` 注册。
  - trace 只声明 text runs 和 26-byte endpoint records。
  - byte-audit 当前没有调用 Phase 14+ typed / audit-only Sheet decoders。
- 新增 manual review 文档：
  - `docs/analysis/2026-06-08-phase29-sheet-leftover-review.md`
- manual review 结论：
  - Sheet leftover 不等于未知格式；可能是已 decoded/audit-only 但未 byte-audit claim 的记录。
  - Top local shapes #1-#4 应标为 `NeedsShapeReview + NeedsIDA`，不能直接写新 decoder。
  - Nested `/JSite204/Sheet6` 应标为 `NeedsRegistration + OwnershipDecision`。
  - `0x00CE igSymbol2d` group 应标为 `NeedsByteAuditIntegration`，因为 typed decoder 已存在。
- 推荐下一步：
  - Phase 29-B Sheet Byte-Audit Trace Integration。
  - 将现有 typed / audit-only Sheet decoders 的 byte ranges 接入 byte-audit trace。
  - 重跑 6-fixture snapshot 与 bounded window probe，只对剩余 leftover 做新 decoder / IDA 分析。
- 更新：
  - `task_plan.md` 当前阶段描述改为 Phase 29-B recommended。
  - `docs/specs/.../tasks.md` Phase 29-A review 标记完成，新增 Phase 29-B 待办。
  - `findings.md` 记录 manual review 分类。

### Phase 29-A manual review 下一步
- 执行 Phase 29-B：为 existing Sheet typed / audit-only decoders 添加 byte-audit trace consumption。

### Phase 29-A manual review 验证
| 检查项 | 结果 |
|---|---|
| `cargo fmt --all -- --check` | 通过 |
| `ReadLints` | 无错误 |

### Phase 29-B Sheet byte-audit trace integration 进展
- 按用户再次要求“按推荐方案继续下一步”，执行 Phase 29-B。
- 修改 `src/byte_audit/aggregate.rs`：
  - 在 top-level Sheet branch 中新增 `trace_sheet_decoded_records(data, &mut b)`。
  - 额外调用 `parse_header_with_trace(data, &mut b)`，声明 Sheet common
    cluster-family header bytes。
  - 对现有 typed decoders 的 byte ranges 使用 `TraceConfidence::Decoded`。
  - 对 audit-only `decode_graphic_groups` / `decode_sub_records_0x0010` 使用 `TraceConfidence::Probed`。
- 修改 `src/byte_audit/mod.rs`：
  - `ParserTrace::consumed_bytes()` 从直接求和 `consumed_ranges` 改为
    `total_bytes - leftover_bytes()`，避免 mixed-confidence overlap 双计数。
- 新增 focused tests：
  - `consumed_bytes_counts_union_across_confidence_overlap`
  - `sheet_typed_decoders_claim_known_record_ranges`
- 首次重跑 6-fixture snapshot 后发现 `/Sheet6` coverage ratio > 1：
  - 原因是 Decoded 与 Probed ranges 可能重叠，而旧 `consumed_bytes()` 按不同 confidence 分桶双计数。
  - 修正 union 计数后重跑通过。
- 重跑 6-fixture coverage / byte-audit JSON。
- 重跑 bounded Sheet leftover windows，输出：
  - `docs/analysis/2026-06-08-phase29-sheet-leftover-windows-after-trace.md`
- 在发现 top-level Sheet header `44 F5 90 6C` 仍作为 leftover 出现后，
  追加 Sheet common header trace，并再次重跑 byte-audit tests 与 snapshots。
- 新增结果说明：
  - `docs/analysis/2026-06-08-phase29-sheet-byte-audit-trace-integration.md`
- 更新：
  - `data-model.md` snapshot matrix 使用 after-trace byte-audit 数值。
  - `snapshot-priority-backlog.md` 按 after-trace JSON 重生成。
  - `docs/specs/.../tasks.md` Phase 29-B 标记完成。
  - `task_plan.md` 当前阶段更新为 Phase 29-B complete。
  - `findings.md` 记录 Phase 29-B 结论。

### Phase 29-B snapshot result
| Fixture | Overall ratio | `/Sheet6` ratio |
|---|---:|---:|
| `d06` | 0.13231003 | 0.8568656 |
| `nonascii-process-1` | 0.21170664 | 0.8194456 |
| `dwg0201` | 0.22253118 | 0.9259647 |
| `dwg0202` | 0.18653207 | 0.89802366 |
| `publish-a01` | 0.08193194 | 0.855618 |
| `publish-dwg0202` | 0.18639842 | 0.89772105 |

### Phase 29-B 验证
| 检查项 | 结果 |
|---|---|
| `cargo test --lib byte_audit -- --nocapture` | 通过，42 passed |
| 6-fixture coverage / byte-audit JSON 生成 | 通过 |
| `cargo run --example probe_phase29_sheet_leftover_windows` | 通过 |

### Phase 29-B 下一步
- 基于 `phase29-sheet-leftover-windows-after-trace.md` review 剩余 groups。
- 优先判断 `0x00CE` variants 是 decoder validation 太窄、非 symbol record，还是仍需 IDA。
- Nested `/JSite204/Sheet6` 继续按 ownership / registration 处理，不与 top-level `/Sheet6` 混同。

### Phase 29-C after-trace remaining groups review 进展
- 按用户再次要求“按推荐方案继续下一步”，执行 Phase 29-C review。
- 新增文档：
  - `docs/analysis/2026-06-08-phase29-sheet-after-trace-review.md`
- review 结论：
  - after-trace 剩余 groups 已不再是 broad pre-trace local shapes。
  - nested `/JSite204/Sheet6` 归类为 `NeedsRegistration + OwnershipDecision`。
  - `0x00CE CE 00 71/79` 归类为 `NeedsSymbolRejectProbe`。
  - `0x00CE CE 80 71/79` 归类为 `NeedsIDAOrVariantEvidence`，因为 type flags 非零。
  - `0x0084 igLineString2d` 归类为 `NeedsLineStringRejectProbe`。
- 推荐下一步：
  - Phase 29-C1 Symbol Reject Probe。
  - 输出 `docs/analysis/2026-06-08-phase29-igsymbol-reject-probe.md`。
- 更新：
  - `docs/specs/.../tasks.md` 新增 Phase 29-C。
  - `task_plan.md` 当前阶段描述更新为 Phase 29-C complete / 29-C1 recommended。
  - `findings.md` 记录 Phase 29-C 分类。

### Phase 29-C 错误与处理
| 问题 | 处理 |
|---|---|
| 误触 ApplyPatch 创建了无关空文件 `D:\work\plant-code\cad\parse\dummy` | 立即用 Delete 工具删除；未留在工作区 |

### Phase 29-C 下一步
- 执行 Phase 29-C1：生成 `0x00CE` rejection probe，按 validation failure reason 分类 symbol-like leftovers。

### Phase 29-C1 igSymbol2d reject probe 进展
- 按用户再次要求“按推荐方案继续下一步”，执行 Phase 29-C1。
- 新增 read-only example：
  - `examples/probe_phase29_igsymbol_rejects.rs`
- 该 probe：
  - 读取 6-fixture after-trace byte-audit leftovers。
  - 只扫描 top-level `/Sheet*` leftover 中的 `0x00CE` candidates。
  - 按当前 `decode_igsymbols` 验证规则分类拒绝原因。
- 生成报告：
  - `docs/analysis/2026-06-08-phase29-igsymbol-reject-probe.md`
- 结果：
  - 所有 rejected `0x00CE` candidates 均为 `out_of_domain_double`。
  - 没有发现 `bytes_to_follow_out_of_range`、`payload_truncated`、
    `missing_double_payload`、`non_finite_double` 或
    `accepted_but_still_leftover`。
  - type flags 包含 `0` 与 `2`，非零 flags 仍需 IDA / controlled evidence。
- 结论：
  - 当前不放宽 `decode_igsymbols` validation。
  - 剩余 symbol-like leftovers 应保持 `NeedsIDAOrVariantEvidence` 或
    `NeedsControlledDiff`，不能直接 promotion。
- 更新：
  - `docs/specs/.../tasks.md` Phase 29-C1 标记完成。
  - `task_plan.md` 当前阶段更新为 Phase 29-C1 complete / 29-C2 recommended。
  - `findings.md` 记录 reject probe 结论。

### Phase 29-C1 下一步
- 若继续非 IDA 路线，执行 Phase 29-C2：nested `/JSite204/Sheet*` ownership /
  byte-audit registration review。
- 若 IDA 可用，针对 `0x00CE` variants 追 reader / flags / transform layout。

### Phase 29-C2 nested Sheet ownership / registration review 进展
- 按用户再次要求“按推荐方案继续下一步”，执行 Phase 29-C2。
- 检查 `publish-a01-byte-audit.json` 中 `/JSite204` 相关 streams，发现其不是孤立 Sheet：
  - Summary / DocumentSummary
  - AppObject
  - DocVersion2 / DocVersion3
  - Dynamic Attributes Metadata
  - JSitesList
  - PSMcluster0 / PSMclustertable / PSMroots / PSMsegmenttable
  - PSMspacemap pages
  - Sheet12 / Sheet22 / Sheet32 / Sheet41 / Sheet51 / Sheet6
  - StyleCluster
  - Unclustered Dynamic Attributes
- 统计 nested unregistered JSite child groups：
  - `JSite793`: 28 child paths
  - `JSite204`: 23 child paths
  - 多个 JSite（145/151/6963/7559/329/396/121/39）各 13 child paths
- 新增 review 文档：
  - `docs/analysis/2026-06-08-phase29-nested-sheet-ownership-review.md`
- 结论：
  - nested `JSite*/Sheet*` 不应接入 top-level Sheet byte-audit registration。
  - 它们更像 symbol-local / embedded-fragment PSM package。
  - 需要先做 ownership inventory，再决定是否 header-only trace / recursive trace / demand-gated ignore。
- 更新：
  - `docs/specs/.../tasks.md` Phase 29-C2 标记完成，新增 Phase 29-D。
  - `task_plan.md` 当前阶段更新为 Phase 29-D recommended。
  - `findings.md` 记录 Phase 29-C2 结论。

### Phase 29-C2 下一步
- 执行 Phase 29-D：生成 6-fixture nested `JSite*` package inventory。

### Phase 29-D nested JSite package inventory 进展
- 按用户再次要求“按推荐方案继续下一步”，执行 Phase 29-D。
- 新增 read-only example：
  - `examples/probe_phase29_nested_jsite_inventory.rs`
- 该 probe 跨 6 fixtures 聚合：
  - parent JSite id / path。
  - child stream names / sizes。
  - mirrored top-level families。
  - nested Sheet children。
  - JProperties symbol path / local symbol path / GUID count。
  - classification：`NeedsOwnership` / `CanTraceHeaderOnly` /
    `IgnoreUntilConsumerNeeds`。
- 生成报告：
  - `docs/analysis/2026-06-08-phase29-nested-jsite-package-inventory.md`
- 初始输出包含 CFB 控制字符路径（`\x01Ole` / `\x05SummaryInformation`），已转义为 `\\x01` / `\\x05`。
- 关键结果：
  - `publish-a01 /JSite204` 是唯一当前带 nested `Sheet*` children 的
    `NeedsOwnership` package：25 child streams，19,760 bytes。
  - `JSite793`、`JSite329`、`JSite7559`、`JSite145` 等为
    `CanTraceHeaderOnly`：它们有 PSM / Style / registry child streams，但没有
    nested Sheet children。
  - 大量 JProperties-only 或 OLE-only symbol instances 为
    `IgnoreUntilConsumerNeeds`。
- 更新：
  - `docs/specs/.../tasks.md` Phase 29-D 标记完成。
  - `task_plan.md` 当前阶段更新为 Phase 29-D complete。
  - `findings.md` 记录 Phase 29-D 结论。

### Phase 29-D 下一步
- 决定是否对 `CanTraceHeaderOnly` nested JSite cluster-family child streams 做
  header-only byte-audit trace。
- `NeedsOwnership` 的 `/JSite204` 应继续保持不与 top-level Sheet geometry 混同。

### Phase 29-D follow-up nested JSite header-only trace 进展
- 按推荐继续后，为 nested JSite cluster-family child streams 添加 header-only
  byte-audit trace。
- 修改 `src/byte_audit/aggregate.rs`：
  - 新增 `nested_jsite_cluster_header_name(path)`。
  - 对一层 nested JSite child 中的 `PSMcluster0`、`StyleCluster`、
    `Dynamic Attributes Metadata`、`Sheet*` 调用 `parse_header_with_trace`。
  - branch 放在 `JProperties` 之前，但 helper 不匹配 `JProperties`，确保 nested
    JProperties 仍由 `parse_jproperties` 处理。
- 新增 tests：
  - `nested_jsite_cluster_header_gets_header_only_trace`
  - `nested_jsite_jproperties_still_uses_jproperties_parser`
- 运行 `cargo test --lib byte_audit -- --nocapture`：
  - 44 passed。
- 重跑 6-fixture coverage / byte-audit JSON。
- 重跑 nested JSite inventory，并再次转义 CFB 控制字符。
- 更新：
  - `data-model.md` final snapshot matrix。
  - `snapshot-priority-backlog.md`。
  - `task_plan.md` 当前阶段改为 Phase 29-D follow-up complete / Phase 29-E recommended。
  - `tasks.md` Phase 29-D 标记 header-only trace 完成，新增 Phase 29-E。
  - `findings.md` 记录 unregistered path 数下降。

### Phase 29-D follow-up snapshot result
| Fixture | Overall ratio | Unregistered paths |
|---|---:|---:|
| `d06` | 0.13368976 | 27 |
| `nonascii-process-1` | 0.2121614 | 26 |
| `dwg0201` | 0.22296105 | 27 |
| `dwg0202` | 0.18676458 | 18 |
| `publish-a01` | 0.08471627 | 40 |
| `publish-dwg0202` | 0.18663079 | 18 |

### Phase 29-D follow-up 下一步
- 执行 Phase 29-E：验证与收口。

### Phase 29-E 验证与收口
- 按推荐继续后，执行 Phase 29-E。
- 验证结果：
  - `cargo test --lib byte_audit -- --nocapture`：通过，44 passed。
  - `cargo fmt --all -- --check`：通过。
  - `ReadLints`：无错误。
- 更新：
  - `docs/specs/.../tasks.md` Phase 29-E 标记完成。
  - `task_plan.md` Phase 29-A..E 标记 complete。
  - `findings.md` 记录最终验证结果。
- 当前 Phase 29-A..E 可收口：
  - Sheet byte-audit 从 probe-only accounting 升级为 typed/audit/header-aware accounting。
  - nested JSite cluster-family child streams 有 header-only accounting。
  - 剩余 deep semantics 不继续猜测，等待 IDA / controlled fixture / 下游需求。

### Phase 29 后续触发条件
- IDA tool descriptor 与相关 IDB 可用：继续普通 geometry reader / JStyle / 0x00CE variant 分析。
- controlled fixture diff 可用：重启 `PSMspacemap` 或 `0x00CE` variant 验证。
- 下游 H7CAD / publish 指名 nested JSite symbol-instance 字段：启动 JSite semantic parser。

### Phase 29 validation follow-up
- 按用户继续要求，尝试运行全量 pre-commit gates：
  - `cargo build --locked --workspace --all-targets`：通过。
  - `cargo test --locked --workspace --all-targets`：通过到 examples test stage，未见失败。
  - `cargo clippy --locked --workspace --all-targets -- -D warnings`：首次失败在既有
    `examples/probe_psm_type_code_histogram.rs` 的 `clippy::type_complexity`。
- 修复 clippy 失败：
  - 为 `examples/probe_psm_type_code_histogram.rs` 增加
    `TypeCodeHistogram` / `FixtureHistogramResult` 类型别名。
- 第二次 clippy 发现本轮新增
  `examples/probe_phase29_sheet_leftover_windows.rs` 的 doc lazy continuation
  与 explicit auto-deref，已修复。
- 第三次全量 clippy 在执行过程中被用户中断；因此未得到完整 all-targets
  clippy 结果。
- 改跑覆盖本轮改动的聚焦验证：
  - `cargo fmt --all -- --check`：通过。
  - `cargo clippy --locked --lib --examples -- -D warnings`：通过。

### Phase 29 validation follow-up 下一步
- 若需要提交前最终确认，仍建议在无人中断时重跑完整：
  `cargo build --locked --workspace --all-targets`、
  `cargo test --locked --workspace --all-targets`、
  `cargo clippy --locked --workspace --all-targets -- -D warnings`、
  `cargo fmt --all -- --check`、
  `cargo rustdoc --lib --locked -- -W missing-docs`。

### Phase 29 full validation follow-up
- 按用户继续要求，重跑完整提交级门禁。
- 本次完整命令链：
  - `cargo build --locked --workspace --all-targets`
  - `cargo test --locked --workspace --all-targets`
  - `cargo clippy --locked --workspace --all-targets -- -D warnings`
  - `cargo fmt --all -- --check`
  - `cargo rustdoc --lib --locked -- -W missing-docs`
- 结果：全部通过，exit code 0。
- 备注：此前中断的 full all-targets clippy 已由本次完整门禁补齐；当前 Phase 28/29 文档、examples 与 byte-audit 改动具备提交级验证结果。

## 2026-06-10 会话：Phase 28-C 核查 + 文档漂移修正 + Phase 29-F PSMcluster0 三角化

### Phase 28-C IDA 可达性核查
- `ida-pro-mcp list_instances`：仅 `core.dll`（AVEVA E3D，无关）与
  `radsrvitem.dll` 可达。
- 结论：目标 IDB（`style.dll` / `J2DSrv.dll` / `sppid.dll` /
  `XCeedRAD.dll` / `smartplantpid.exe`）均未打开；候选 Slice 29-F
  （IDA module enablement）维持 blocked，按推荐路线转入 PSMcluster0
  body 三角化。
- `tasks.md` Phase 28-C 四项勾选完成，附核查记录。

### 文档漂移修正
- `AGENTS.md`：0x0030 GArc2d 旧叙述全部更正为 JStyleOverride
  （测试门禁表 2 行、家族表 1 行、实体计数段、key insight、caveat、
  `SheetGeometry` 字段列表）；删除已不存在的
  `primitive_arc_decoder_*` / `dwg0201_*_arcs_*` 测试行，替换为
  `jstyle_override_decoder_emits_audit_records_with_provenance`。
- `tasks.md`：Phase 28-F 两项与 Validation Tasks 四项选框回写
  （对应 Phase 29 收口时已通过的门禁记录）。

### Phase 29-F PSMcluster0 body 三角化（candidate Slice 29-B）
- 新增 `examples/probe_phase29_psmcluster0_body_triage.rs`：
  header/leftover 汇总 + strict PSM walk + min-chain-3 resync 链扫描 +
  UTF-16 run 扫描 + stride 自相关 + 三段 hex window。
- 关键结果：
  - 6/6 fixture body 为单条连续 PSM envelope record 链
    （offset 145 → 流尾，覆盖 99.81%–99.99%，仅 10 字节 prologue 未解释）。
  - `chain_records == record_count - 2` 全 fixture 成立。
  - type 分布稳定：`0x0089` 主导 + `0x0003`/`0x0081`/`0x00EC` 复现 +
    链首 `0x0002`。
  - payload 是属性/格式 catalog 纹理，非几何。
- 产出 `docs/analysis/2026-06-08-phase29-psmcluster0-leftover-triage.md`：
  - parser backlog：audit-only `decode_psm_cluster0_records` walker
    （Phase 18 GraphicGroup/0x0010 模板），测试靶点含 cross-fixture
    `record_count - 2` ratchet 与 `/PSMcluster0` consumed ≥ 0.99。
  - IDA target request：stream_type 0x0075 reader、type namespace 判定、
    `0x0089` 等命名、prologue 与 off-by-2 解释。
- 同步 `data-model.md` PSMcluster0 条目、`snapshot-priority-backlog.md`
  priority note、`tasks.md` 新增 Phase 29-F 小节、`findings.md`。

### 下一步
- 实现 audit-only `decode_psm_cluster0_records` walker（backlog 已含
  验收条件），或等待 IDA 模块解锁后做语义命名。

### Phase 29-F walker 实现（用户"按推荐方案继续"后同日执行）
- 实现 `src/parsers/cluster_header.rs`：
  - `decode_psm_cluster0_record_at`：6 字节 envelope 校验
    （type_code != 0、btf ≤ 100_000、payload 在界内），raw payload。
  - `decode_psm_cluster0_records`：严格链 walk，零 resync。
  - `decode_psm_cluster0_body_records`：header + string table + 10 字节
    prologue + full-coverage gate（链必须恰好到流尾且 ≥1 record，
    否则返回空，不做 partial claim）。
  - `parse_psm_cluster0_with_trace`：prologue `Probed`，envelope
    `Decoded`（record_count - 2 invariant 佐证），payload `Probed`。
- 测试：
  - `parsers::cluster_header` 16 passed（8 个新测试）。
  - `parser_panic_safety` 2 passed（新增 3 个入口）。
  - `psmcluster0_body_chain_matches_record_count_invariant`：
    6/6 fixture，records=60/442/194/229/43/231，consumed ratio 全部
    1.0000。
- 快照：12 个 coverage/byte-audit JSON 重新生成；
  `/PSMcluster0` leftover 全部归 0；全文件 ratio：
  d06 0.2209、nonascii 0.5852、dwg0201 0.3500、dwg0202 0.3674、
  publish-a01 0.1680、publish-dwg0202 0.3678。
- 文档同步：triage 报告加 Implementation Status、
  `data-model.md` 快照矩阵与 PSMcluster0 条目、
  `snapshot-priority-backlog.md`（PSMcluster0 移入 fully-accounted 组、
  推荐 backlog 改 StyleCluster 为下一候选）、`tasks.md` 29-F 勾选。
- 门禁：5/5 通过
  （build / test exit 0 / clippy -D warnings / fmt --check /
  missing-docs ratchet current=0 baseline=0，bash 不可用故以
  PowerShell 等价执行）。

### Phase 29-F 后续触发条件
- StyleCluster body 三角化：验证是否复用 PSMcluster0 链布局
  （下一个非 IDA 候选 slice）。
- IDA 模块解锁：stream_type 0x0075 reader、record type 命名、
  prologue 与 off-by-2 解释。

### Phase 29-G StyleCluster body 三角化 + walker（继续推荐路线）
- 探针泛化为接受 stream 名参数，对 `/StyleCluster` 跑出形状：
  - 单条 end-anchored record 链（6/6 fixture 恰好到流尾，零 resync）。
  - 变长 GUID-table 形 prefix 未解析（1,529–2,360 字节）。
  - `record_count == chain_records` 仅 2/6 成立 → 整条 record 走
    `Probed`，不给 envelope `Decoded`。
- 实现：
  - 共享核心改名：`ClusterBodyRecordDecoded` /
    `decode_cluster_body_record_at` / `decode_cluster_body_records`
    （PSMcluster0 与 StyleCluster 复用；
    `decode_psm_cluster0_body_records` 保留原名）。
  - 新增 `decode_style_cluster_body_records`：earliest end-anchored
    chain locator（≥3 records、恰好到流尾，否则零 claim）。
  - 新增 `parse_style_cluster_with_trace`；aggregate.rs `/StyleCluster`
    分支从 header-only 切换到该 walker。
- 测试：cluster_header 22 passed；aggregate 既有测试改 parser_name 断言；
  panic-safety 新增 `decode_style_cluster_body_records`；
  `stylecluster_body_chain_is_end_anchored_across_fixtures` 6/6 通过，
  链起点 2376/1545/1899/2267/2042/2267 与探针一致，
  consumed ratio 0.631–0.898。
- 快照重生成：`/StyleCluster` leftover 83,468 → 12,300（仅剩 prefix）；
  全文件 ratio：d06 0.2786、nonascii 0.6259、dwg0201 0.4181、
  dwg0202 0.4636、publish-a01 0.2259、publish-dwg0202 0.4639。
- 文档：新增
  `docs/analysis/2026-06-08-phase29-stylecluster-leftover-triage.md`；
  data-model / snapshot-priority-backlog / tasks.md（新增 Phase 29-G）
  同步；PSMcluster0 triage 文档同步共享 API 名。
- 门禁：5/5 通过（build / test exit 0 / clippy -D warnings /
  fmt --check / missing-docs 0=0）。

### Phase 29-G 后续触发条件
- StyleCluster GUID-table 形 prefix 特征化（下一个有界非 IDA 候选）。
- IDA 解锁：stream_type 0x005A reader、prefix 布局、record_count 语义、
  `0x002C`–`0x002E` 命名。

### Phase 29-H StyleCluster prefix 特征化（继续推荐路线）
- 新增 `examples/probe_phase29_stylecluster_prefix.rs`：
  opener 解析 + stride 对比 + 跨 fixture 公共前缀计算 + UTF-16 run。
- 结果：
  - 12 字节 opener 恒定（10 零 + u16 13）。
  - `[16..548)` 532 字节跨 6 fixture 逐字节相同（writer 模板）。
  - GUID 区非均匀 stride（16/20/24/28/32 均不能装下 13 个干净条目）。
  - 548 之后为 fixture 专属 42 字节 slot 样式区。
- 决策：documentation-only closeout，不上 parser；prefix 保持
  leftover；IDA target request 扩展（0x005A writer/reader、count 13、
  GUID 条目、42 字节 slot）。
- 产出
  `docs/analysis/2026-06-08-phase29-stylecluster-prefix-characterization.md`；
  tasks.md / data-model.md / snapshot-priority-backlog.md 同步。
- 验证：`cargo fmt --check` + `clippy --lib --examples -D warnings`
  通过（本轮仅新增探针与文档，无 parser 改动）。

### Phase 29-H 后续触发条件
- cluster-family 全部收口；剩余非 IDA 候选转向
  `Unclustered Dynamic Attributes` deep body（candidate Slice 29-C，
  111K leftover）或等待 IDA / 下游需求。

## Session: 2026-06-11 Phase 29-I Unclustered DA body 三角化

### 当前状态
- **Phase:** 29-I - Unclustered DA body 三角化（candidate Slice 29-C）
- **状态:** 三角化 + backlog 文档完成；audit-only walker 实现为下一步

### 已完成
- 按推荐路线先核查 IDA 可达性：`list_instances` 仍仅 `core.dll`（无关）
  与 `radsrvitem.dll`，目标 IDB（style/J2DSrv/sppid/XCeedRAD/
  smartplantpid）未打开 → Slice 29-F 维持 blocked，执行非 IDA 候选
  Slice 29-C。
- 复核 DA 现状：`src/parsers/dynamic_attr_records.rs` 的
  `scan_da_landmarks_with_trace` 只 claim 三类 landmark（类名 14B /
  trailer 31B / `DrawingID\0`+32hex），record body 全部 leftover。
- 新增 read-only 探针 `examples/probe_phase29_da_body_triage.rs`：
  全流 strict envelope 链测试（earliest end-anchored locator）、
  trailer/class-name 对齐计数、`0x89 0x00` marker 统计、
  跨 fixture 属性名 census + class_id/class_name 直方图。
- 关键结果（6/6 fixture）：
  - body = 8 字节 prologue + 单条 end-anchored `0x0089` envelope 链
    （覆盖 0.9978–0.9998，零 resync，零 tail gap）。
  - prologue = cluster-family magic `44 F5 90 6C` + u32 counter；
    counter == 字面 marker record 数 6/6，== 链 record 数 5/6
    （nonascii 有一条高位 flag head）。
  - 417/417 trailer offset == 链 record head："31 字节 trailer" 实为
    下一 record 的 envelope head + 固定 head-tail。
  - 链 records：d06=47 / nonascii=69 / dwg0201=231 / dwg0202=169 /
    publish-a01=22 / publish-dwg0202=169。
- 产出 backlog 文档：
  `docs/analysis/2026-06-08-phase29-dynamic-attributes-body-backlog.md`
  （walker backlog + 测试靶点 + 对象/关系收益命名 + IDA target
  request + guardrails）。
- 同步 `tasks.md`（新增 Phase 29-I 小节）、`snapshot-priority-backlog.md`
  （DA family note + recommended backlog #4）、`data-model.md`
  （DA 条目改写）、`task_plan.md`（Phase 29-I + 当前阶段）、
  `findings.md`。

### 验证
| 检查项 | 结果 |
|---|---|
| `ida-pro-mcp list_instances` | 通过；仅 2 个无关/已有 instance，29-F 维持 blocked |
| `cargo run --release --example probe_phase29_da_body_triage` | 通过；6/6 fixture 输出完整 |
| `cargo fmt --all -- --check` | 通过 |
| `cargo clippy --locked --lib --examples -- -D warnings` | 通过 |
| ReadLints | 无错误 |

### 下一步
- Phase 29-I follow-up：实现 audit-only DA body-chain walker
  （`decode_unclustered_da_body_records` +
  `parse_unclustered_da_with_trace`，全 `Probed` claims），
  单测 + panic-safety + ratchet
  `da_body_chain_is_end_anchored_across_fixtures`，重生成 12 个
  snapshot JSON，预期 `/Unclustered Dynamic Attributes` leftover
  111,120 → 0。

### Phase 29-I follow-up：DA walker 实现（同日，用户"按推荐方案继续"后）
- `src/parsers/cluster_header.rs`：
  - `UNCLUSTERED_DA_PROLOGUE_LEN = 8`（magic + u32 counter）。
  - `decode_unclustered_da_body_records`：magic gate + 固定链起点 8 +
    end-anchored full-coverage gate（否则零 claim）；counter 不 gate。
  - `parse_unclustered_da_with_trace`：prologue + 全部 record 走
    `Probed`；gate 失败 builder 不动。
  - 7 个新单测（同步链 / counter mismatch / wrong magic /
    non-end-anchored / truncated / trace 全覆盖 / trace 零 claim）。
- `src/byte_audit/aggregate.rs`：DA branch 合并 walker +
  `scan_da_landmarks_with_trace`，parser name `parse_unclustered_da`；
  landmark 87 字节测试改断言；新增 fixture-shaped 链全覆盖测试。
- `tests/parser_panic_safety.rs`：新增
  `decode_unclustered_da_body_records` 入口。
- `tests/parse_real_files.rs`：新增
  `da_body_chain_is_end_anchored_across_fixtures`（6/6 fixture，
  records 47/69/231/169/22/169，type 全 0x0089，chain start 8，
  end-anchored，leftover=0，parser name 锁定）。
- 快照重生成（12 JSON）：`/Unclustered Dynamic Attributes` leftover
  全归 0；全文件 ratio：d06 0.3793 / nonascii 0.6700 /
  dwg0201 0.5912 / dwg0202 0.5923 / publish-a01 0.2733 /
  publish-dwg0202 0.5925。
- 文档同步：backlog 文档加 Implementation Status、data-model.md
  （DA 条目 + snapshot matrix + interpretation）、
  snapshot-priority-backlog.md（fixture summary + family 表 +
  individual paths + recommended backlog #4 → done）、tasks.md
  Phase 29-I 勾选、task_plan.md、findings.md、CHANGELOG.md。

### Phase 29-I follow-up 验证
| 检查项 | 结果 |
|---|---|
| `cargo test --lib parsers::cluster_header` | 29 passed（新增 7） |
| `cargo test --lib byte_audit` | 45 passed |
| `cargo test --test parse_real_files da_body_chain_is_end_anchored_across_fixtures -- --nocapture` | 6/6 fixture 通过，leftover=0 |
| `cargo test --test parser_panic_safety` | 2 passed |
| `cargo build --locked --workspace --all-targets` | 通过 |
| `cargo test --locked --workspace --all-targets` | 通过，0 failed |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | 通过 |
| `cargo fmt --all -- --check` | 通过 |
| `cargo rustdoc --lib --locked -- -W missing-docs` | 0 warnings，baseline=0 |

### Phase 29-I 后续触发条件
- IDA 模块解锁：`0x0089` record reader（PSMcluster0/DA 共享）、DA
  prologue counter 语义、nonascii 高位 flag head、head 字段
  （record_id/field_x/class_id）命名。
- 下游需要更干净的 DA 属性提取时：per-record attribute scoping
  follow-up（用链边界圈定 `parse_attribute_records`，消除 `.sym`
  路径 / hex 片段属性名 artifacts）。

### Phase 29-J：nested JSite cluster body 分派（同日，"按推荐方案继续"）
- IDA 复查：仍仅 `core.dll` / `radsrvitem.dll`，29-F 维持 blocked。
- 新增探针 `examples/probe_phase29_nested_cluster_bodies.rs`：对全部
  一层 nested `JSite*/PSMcluster0|StyleCluster|Unclustered Dynamic
  Attributes` 流直接运行现有 walker（gate 自验证）。
- 探针结果：23/23 end-anchored —— nested PSMcluster0 11/11
  `record_count - 2` + 链起点 145；nested StyleCluster 11/11；
  nested DA 1/1（counter=4=records）；总链覆盖 0.9028。
- 实现：aggregate nested branch 按 child 分派完整 walker；
  `nested_jsite_cluster_header_name` 增加
  `Unclustered Dynamic Attributes`；Sheet* / DA Metadata 维持
  header-only。
- 测试：`nested_jsite_cluster_bodies_dispatch_to_full_walkers`
  （aggregate 单测）+
  `nested_jsite_cluster_bodies_are_end_anchored_across_fixtures`
  （ratchet，23 streams：nested PSMcluster0/DA leftover=0，
  StyleCluster leftover=prefix）。
- 快照重生成（12 JSON）：`JSite*` family leftover 325,843 → 74,559；
  全文件 ratio：d06 0.7699 / nonascii 0.8201 / dwg0201 0.8804 /
  dwg0202 0.8474 / publish-a01 0.6297（unreg 40→39，traces 35→36）/
  publish-dwg0202 0.8474。
- 文档：tasks.md 新增 Phase 29-J、snapshot-priority-backlog（fixture
  summary / JSite family 行 / individual paths 表重生成 /
  recommended backlog #1）、data-model（matrix + interpretation）、
  task_plan / findings / CHANGELOG。

### Phase 29-J 验证
| 检查项 | 结果 |
|---|---|
| `cargo run --release --example probe_phase29_nested_cluster_bodies` | 23/23 end-anchored |
| `cargo test --lib byte_audit` | 46 passed |
| `cargo test --test parse_real_files nested_jsite_cluster_bodies_are_end_anchored_across_fixtures -- --nocapture` | 通过，23 streams |
| `cargo build --locked --workspace --all-targets` | 通过 |
| `cargo test --locked --workspace --all-targets` | 通过，0 failed |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | 通过 |
| `cargo fmt --all -- --check` | 通过 |
| `cargo rustdoc --lib --locked -- -W missing-docs` | 0 warnings |

### Phase 29-J 后续触发条件
- nested PSMspacemap 页与 top-level `/PSMspacemap` 同属 `tseg`
  evidence gate（IDA / controlled fixture）。
- nested `/JSite204/Sheet6` 维持 ownership-gated，不与 top-level
  Sheet 几何混同。
- per-record DA attribute scoping follow-up 仍是唯一未排期的
  非 IDA 候选。

### Phase 29-K：per-record DA attribute scoping（同日，"按推荐方案继续"）
- 执行 Slice C named benefit #1（最后一个未排期非 IDA 候选）。
- 重构 `src/parsers/dynamic_attr_records.rs`：
  - section-body 解析从 `try_parse_record` 抽为 `parse_section_body`
    （提取逻辑不变）。
  - 新增 `parse_attribute_records_chain_scoped`：
    `decode_unclustered_da_body_records` gate 通过 → 按链 record
    精确边界逐 record 解析；否则 byte-for-byte 回退 legacy 扫描。
- `streams/dynamic_attrs.rs` 切换 chain-scoped；`streams/cluster.rs`
  保持 legacy（防 magic+chain gate 在非 DA cluster 流误触发）。
- 测试：
  - 4 新单测：flagged-head 找回 / payload 伪 marker 免疫 /
    无 gate 回退等价 / 无类名 record 跳过。
  - panic-safety 新增 `parse_attribute_records_chain_scoped` 入口。
  - ratchet `da_chain_scoped_attribute_extraction_matches_or_beats_legacy_scan`：
    6/6，scoped ≥ legacy，nonascii 68 → 69（找回 `Symbol` 记录），
    其余 fixture 不变；并断言文档管线（`da.attribute_records`）
    使用 chain-scoped 计数。
- 全量 `parse_real_files` 100 passed（切换前先行验证零回归），
  快照不受影响（coverage JSON 哈希不变）。

### Phase 29-K 验证
| 检查项 | 结果 |
|---|---|
| `cargo test --lib parsers::dynamic_attr_records` | 25 passed（新增 4） |
| `cargo test --test parse_real_files` | 100 passed（切换后含新 ratchet 101） |
| `cargo test --test parser_panic_safety` | 2 passed |
| nonascii coverage JSON 哈希对比 | 不变 |
| `cargo build/test/clippy/fmt/rustdoc` 五项门禁 | 全过（missing-docs 0=0） |

### Phase 29-K 后续触发条件
- head-field surfacing（benefit #2）：非 signature record 的
  record_id/field_x/class_id 提升需要 IDA 确认 head-tail 布局。
- Phase 29 全系收口；下一步动作全部 gated（IDA / controlled
  fixture / 下游需求），或由用户决定提交当前工作树。

### Phase 29-L：nested JSite registry 分派（2026-06-11 夜 ~ 06-12 晨）
- IDA 复查仍 blocked；JSitesList 无现成 parser（确认后维持
  demand-gated）。
- 新增探针 `examples/probe_phase29_nested_registry_streams.rs`：
  对 nested `JSite*/{PSMclustertable,PSMroots,PSMsegmenttable,
  DocVersion2,DocVersion3,AppObject,\x05Summary*}` 运行既有
  top-level trace parser。
- 探针结果：68 流 / 98.4% consumed；DocVersion2/3、PSMclustertable、
  PSMsegmenttable 全量；PSMroots 每流 4 字节尾（与 top-level 一致）；
  AppObject 4 字节 stub 干净 gate-out；summary pair 部分解析。
- 实现：`nested_jsite_registry_parser` helper + nested branch 8 类
  registry child 分派；dispatch 单测 + ratchet
  `nested_jsite_registry_streams_reuse_top_level_parsers`。
- 快照重生成（12 JSON）：JSite family leftover 74,559 → 66,778；
  unregistered paths → 12–19/fixture；全文件 ratio 0.664–0.888。
- 门禁：夜间组合命令被中断后，晨间逐项重跑——build/test/clippy/fmt
  严格退出码 0，missing-docs 0（=baseline 0）。
- 文档：tasks.md Phase 29-L、snapshot-priority-backlog（header /
  fixture summary / JSite family / individual paths / common
  unregistered / recommended backlog）、data-model（matrix +
  interpretation）、task_plan、findings、CHANGELOG。

### Phase 29-L 验证
| 检查项 | 结果 |
|---|---|
| `cargo run --release --example probe_phase29_nested_registry_streams` | 68 流，98.4% consumed |
| `cargo test --lib byte_audit` | 47 passed |
| `cargo test --test parse_real_files nested_jsite -- --nocapture` | 2 passed（registry + cluster ratchet） |
| `cargo build/test/clippy/fmt`（严格退出码） | 全部 0 |
| `cargo rustdoc --lib -- -W missing-docs` | 0 warnings |

### Phase 29 收口总览（A..L）
- byte-accounting：top-level + nested cluster/DA/registry 全部
  walker/parser 化；全文件 coverage ratio 0.664–0.888。
- 提取质量：DA chain-scoped extraction（flagged-head 找回）。
- 剩余全部 gated：IDA（0x0089/0x0010/0x00CE 命名、StyleCluster
  prefix、PSMspacemap tseg、head-field surfacing）、ownership
  （nested Sheet6）、demand（JSitesList / OLE / Revision）。
- 待用户决策：提交当前工作树，或打开目标 IDB 解锁 IDA 路线。

### Phase 29-M：JSitesList / Revision 清尾（2026-06-12）
- IDA 复查仍 blocked → 执行最后的非 IDA 清尾候选。
- 新增探针 `examples/probe_phase29_unregistered_tails.rs`：
  `/JSitesList`（×6）与 `/TaggedTxtData/Revision`（×5）hex /
  UTF-16 / UTF-8 / tagged-stg-list 接受度画像。
- 关键发现：
  - `/JSitesList` = `"OLEM"` + u32 count + u32 slot 表；逻辑
    entries 与 `JSite<id>` storage 6/6 全量对应（共 70 条）。
  - dwg0202 族 16 slot vs count=13：3 个 stale 尾 slot —— 首版
    exact-size gate 被 ratchet 证伪后修正（`len >= 8+4*count` +
    4 对齐；stale 尾不 claim）。
  - `/TaggedTxtData/Revision` 0 字节占位。
- 实现：`src/parsers/jsites_list.rs` + parsers/mod 注册 +
  aggregate 顶层/nested/Revision 三处注册 + panic-safety + 8 单测 +
  ratchet `jsites_list_parses_with_exact_size_and_matches_jsite_storages`。
- 快照重生成：unregistered 51 → 38 distinct（9–14/fixture）；
  全文件 ratio 0.665–0.888。
- 产出：`docs/analysis/2026-06-08-phase29-jsiteslist-revision-tails.md`。

### Phase 29-M 验证
| 检查项 | 结果 |
|---|---|
| `cargo test --lib parsers::jsites_list` | 8 passed |
| `cargo test --lib byte_audit` | 47 passed |
| `cargo test --test parse_real_files` | 103 passed（首次 ratchet 失败暴露 stale tail 后修正） |
| `cargo test --test parser_panic_safety` | 2 passed |
| 五项门禁（严格退出码） | 全部 0（clippy 首跑 101：`manual_is_multiple_of`，修复后 0） |

### Phase 29-M 后续触发条件
- IDA：`"OLEM"` writer、count-vs-slot stale tail 解释、slot 值 ↔
  JSite id 的 writer 侧确认（确认后可命名 `jsite_ids` 并暴露到
  model 层）。
- Phase 29 A..M 全系收口；剩余 multi-fixture unregistered 全部
  IDA / demand gated。待用户决策：提交工作树或打开目标 IDB。

### Phase 30-A：radsrvitem.dll JSite IDA 证据刷新（2026-06-12）
- 收到“按推荐方案继续下一步”后，沿 Phase 29-M 后续触发条件做
  只读 IDA 复查；当前可用 IDA instance 仍为 `core.dll` + 可达
  `radsrvitem.dll`，未见 `style.dll` / `J2DSrv.dll` /
  `sppid.dll` / `XCeedRAD.dll` / `smartplantpid.exe`。
- 在 `radsrvitem.dll` 内搜索：`JSitesList` 0 hits，`OLEM` 0 hits，
  `JSite` 有命中。
- 分析 `sub_56448A10`：调用 `sub_564472F0()` 取 id 指针，并格式化
  `L"%s%d", L"JSite", *id`。
- 分析 `sub_56448A70`：调用 `sub_56448970()` 取 id，并格式化
  `L"%s%d", L"JSite", id`。
- 分析 `sub_5646FF60`：接收整数 `a1`，构造 `JSite<a1>`，并调用
  storage vtable open path；调用者包括 `sub_5645FF00` /
  `sub_56460330` / `sub_56460760` / `sub_56460960`。
- 调用链复核：
  - `sub_5645FF00`：`sub_564472F0(this)` → `v37=*id` →
    `sub_5646FF60(v37, &pStg, ...)` → 在 JSite storage 内打开子流。
  - `sub_56460330`：`sub_564472F0(this)` → `v3=*id` →
    `sub_5646FF60(v3, &pStg, ...)` → `ReadClassStg(pStg, &pclsid)`，
    随后加载 `raslink.dll`，说明该路径围绕 JSite package /
    link storage 处理。
- 结论：Phase 29-M 的 `/JSitesList.entries` ↔ `JSite<id>` storage
  id 关联获得 IDA 旁证加强；但当前 DLL 不包含 `"OLEM"` /
  `JSitesList` writer/reader literal，仍不能把 DTO 字段升级命名为
  `jsite_ids`，也不能解释 stale tail 语义。
- 产出：`docs/analysis/2026-06-12-phase30-radsrvitem-jsite-ida-refresh.md`。

### Phase 30-A 验证
| 检查项 | 结果 |
|---|---|
| IDA search `JSitesList` | 0 hits |
| IDA search `OLEM` | 0 hits |
| IDA search `JSite` | 有命中；关键函数已分析 |
| `sub_56448A10` / `sub_56448A70` / `sub_5646FF60` | 均确认 `JSite<id>` naming/open 路径 |
| 代码门禁 | 未运行；本轮仅新增/同步文档，无生产代码改动 |

### Phase 30-A 后续触发条件
- 打开 `style.dll` / `J2DSrv.dll` / `sppid.dll` / `XCeedRAD.dll` /
  `smartplantpid.exe` 任一相关 IDB 后，继续查 `"OLEM"`、`JSitesList`、
  stale tail、`PSMspacemap` `tseg` 页布局、StyleCluster prefix、
  `0x0089` record family semantic、`0x0010` discriminator。

### Phase 30-B/C：0x0089 export 边界 + PSMspacemap handle 证据（同日继续）
- 继续只读 IDA 复查 `sub_5644B640`：
  - `a3 == 0`：从 `sub_56455240` 得到 record-id list，逐项通过
    manager vtable `+0xA4` 取 record pointer，过滤 `*record == 137`，
    可把匹配 pointer 写入输出数组。
  - `a3 == 1`：调用 `sub_56445F40` 对单 record 做导出。
- 分析 `sub_56445F40`：按 `*record` 分派；`0x00FA` →
  `igSimpleDependencyObject`，`0x004D` → `igTextBox`，`0x003D` →
  `igSmartFrame2d`；`0x0089` 不在特例，走 default `sub_564462F0`。
- 分析 default `sub_564462F0` / `sub_56448F70` / `sub_564459D0`：
  - `sub_56448F70` 是 type-code → RAD type-name 表，包含
    `igLine2d` / `igTextBox` / `igPoint2d` / `igLineString2d` /
    `igSymbol2d` 等，**不包含 `0x0089`**。
  - 因此 `0x0089` 在此路径中只写
    `RAD_OBJECT_TYPE = "137"`。
  - 相关 record id 通过字段名 `"RELEATIONS"`（原 DLL 拼写）写入。
  - 该路径不读取 DA/PSMcluster0 head fields，也不解 ASCII class-name
    payload。
- 转向 `PSMspacemap`：
  - 搜索 `tseg`：0 hits。
  - 搜索 `PSMspacemap` / `spacemap`：命中 `sub_56469950`、
    `sub_56469BF0`、`sub_5646AE30`、`sub_5646B3A0` 与
    `ClusterTable::GetSpaceMapSegment()` 日志函数 `sub_5648C370`。
- 分析 `sub_5648C370`：cluster table entry 内有 segment-id array；
  复用 segment 时跳过 flags `0x08`，用 `sub_56479EE0` 判断 segment
  是否仍可用；无可用 segment 时 `sub_56479210` 分配新 segment 并
  append 到 array。
- segment in-memory layout 证据：
  - `segment + 12` = `m_iNext` / next available index。
  - `segment + 22` = flags。
  - `segment + 10` / `+16` / `+20` 参与 reusable/free-list。
  - `sub_56479EE0`：`m_iNext < 0x2000` 或 free-list 非空则可用；
    否则 `flags |= 0x08` 并视为 exhausted。
- handle 编码证据：
  - `sub_56479040(segment_id, entry_index)` 返回
    `(segment_id << 13) | entry_index`，且 `entry_index >= 0x2000`
    时返回 0。
  - `sub_56479C20(handle, out)` 反向使用 `handle >> 13` 和
    `handle & 0x1FFF`。
  - 结论：entry index range = `0..0x1FFF`，segment capacity =
    `0x2000` entries。
- 边界：当前证据证明 handle/segment 选择模型，但尚未直接证明
  `/PSMspacemap` raw page byte layout；parser/byte-audit 仍不能把页
  body 升级为 decoded。
- 产出：
  `docs/analysis/2026-06-12-phase30-radsrvitem-record-spacemap-ida.md`。

### Phase 30-B/C 验证
| 检查项 | 结果 |
|---|---|
| `sub_5644B640` | `0x0089` filter confirmed |
| `sub_56445F40` / `sub_564462F0` / `sub_56448F70` | `0x0089` default export = `RAD_OBJECT_TYPE "137"` |
| `tseg` search | 0 hits |
| `PSMspacemap` / `spacemap` search | storage load/save + `GetSpaceMapSegment` hits |
| `sub_56479040` / `sub_56479C20` | handle encode/decode = `(segment << 13) | entry` |
| 代码门禁 | 未运行；本轮仅新增/同步文档，无生产代码改动 |

### Phase 30 后续触发条件
- 需要更多 IDB（优先 writer/reader 侧模块）来确认：
  - `/JSitesList` `"OLEM"` writer/reader 与 stale tail 语义。
  - `/PSMspacemap` raw stream/page byte layout。
  - `0x0089` DA/PSMcluster0 record family semantic 与 head fields。
  - `StyleCluster` prefix、`0x0010` discriminator、JStyle persistence。

### Phase 30-D：Style / JStyle low-cost negative pass（同日继续）
- 响应继续请求后先确认 IDA instance：仍仅 `core.dll` / `radsrvitem.dll`
  reachable；无 `style.dll` / `J2DSrv.dll` / `sppid.dll` /
  `XCeedRAD.dll` / `smartplantpid.exe`。
- 在 active `radsrvitem.dll` 中搜索：
  - `StyleCluster`：0 hits。
  - `JStyleOverride`：0 hits。
  - `JStyle`：命中 `JStyleBase`、`JStyleBase::IJPersistImp`、
    `IJManageStyle2Imp`、`IJStyleCopyImp`、`IJStyleUserImp` 等 RTTI /
    vtable / thunk。
- 分析 `sub_5655D200`：构造 `JStyleBase` 并安装上述 vtable，只初始化
  base state，不暴露 record layout。
- 分析 `sub_5655DB60` / `sub_5655DBE0`：
  - `sub_5655DB60` 将 `"JStyleBase::IJPersistImp"` 传给
    base object vtable `+52`。
  - `sub_5655DBE0` 将 `"JStyleBase::IJPersistImp"` 传给
    base object vtable `+48`。
  - 二者均为 interface thunk / name-forwarding helper，不是直接
    Load/Save persistence body。
- 结论：当前 `radsrvitem.dll` 只能确认存在 generic JStyle base/interface
  infrastructure；不能解 `0x0030` persistence fields 或
  StyleCluster prefix layout。Phase 16/17 的 `0x0030 = JStyleOverride`
  结论不变，深层字段仍 gated on `style.dll` / `J2DSrv.dll` 等模块。
- 产出：
  `docs/analysis/2026-06-12-phase30-radsrvitem-style-jstyle-negative.md`。

### Phase 30-D 验证
| 检查项 | 结果 |
|---|---|
| IDA instances | `core.dll` + `radsrvitem.dll` only |
| `StyleCluster` search | 0 hits |
| `JStyleOverride` search | 0 hits |
| `JStyle` search | `JStyleBase` / interface RTTI-vtable hits |
| `JStyleBase::IJPersistImp` analysis | thunk/name-forwarding only |
| 代码门禁 | 未运行；本轮仅新增/同步文档，无生产代码改动 |

### Phase 30 当前收口判断
- 当前可达 `radsrvitem.dll` 的低成本 IDA routes 已收口：
  JSite storage naming、`0x0089` RAD export boundary、PSMspacemap
  handle model、Style/JStyle negative pass。
- 剩余问题需要新 IDB 或 controlled fixture；继续在当前 IDB 盲搜的
  边际收益很低。

### Phase 30-F：IDA-gated next actions handoff（同日继续）
- 在无新 IDB 的前提下，新增交接清单
  `docs/analysis/2026-06-12-phase30-ida-gated-next-actions.md`。
- 内容覆盖：
  - 当前可达 IDB 与不可达目标模块。
  - JSite / JSitesList：`JSite<id>` naming/open 已证实；`OLEM` /
    `JSitesList` writer 仍 gated；DTO 不改名。
  - `0x0089`：runtime filter + export default `"137"` 已证实；
    DA/PSMcluster0 head semantics 仍 gated。
  - PSMspacemap：handle `(segment << 13) | entry` 已证实；raw page
    byte layout 仍 gated。
  - Style/JStyle：当前 `radsrvitem.dll` 为负证据，深层字段仍需
    writer/reader module。
  - 下一次打开新 IDB 后的精确搜索 checklist。
- 当前推荐：停止在现有 `radsrvitem.dll` 中继续 broad search；下一步
  需要打开 gated IDB，或提交/评审当前工作树。

### Phase 30-F 验证
| 检查项 | 结果 |
|---|---|
| handoff 文档 | 已新增 |
| task_plan 挂接 | 已同步 |
| 代码门禁 | 未运行；本轮仅新增/同步文档，无生产代码改动 |

### Phase 30-G：worktree readiness check（同日继续）
- 在无新 IDB 且已完成 handoff 后，切到提交/评审前只读核对。
- `git status --short`：当前为 Phase 29/30 累积工作树，包含 16 个
  tracked modified files，另有 Phase 26/27/29/30 docs、spec-kit、
  probe examples、`src/parsers/jsites_list.rs` 等 untracked files。
- `git diff --stat`（tracked only）：16 files changed，
  4552 insertions(+)，109 deletions(-)。
- 观察到 LF→CRLF warning 的 Rust 文件：
  `examples/probe_psm_type_code_histogram.rs`、
  `src/byte_audit/aggregate.rs`、`src/byte_audit/mod.rs`、
  `src/parsers/cluster_header.rs`、
  `src/parsers/dynamic_attr_records.rs`、`src/parsers/mod.rs`、
  `tests/parse_real_files.rs`、`tests/parser_panic_safety.rs`。
- `ReadLints` scoped to关键 Rust parser/test files：无 linter errors。
- 新增 readiness 记录：
  `docs/analysis/2026-06-12-phase29-30-worktree-readiness.md`。

### Phase 30-G 验证
| 检查项 | 结果 |
|---|---|
| `git status --short` | dirty worktree；范围已记录 |
| `git diff --stat` | 16 tracked files，4552 insertions / 109 deletions |
| `ReadLints` scoped to Rust parser/test files | 无错误 |
| readiness 文档 | 已新增 |

### Phase 30-G 后续触发条件
- 若用户授权提交：先跑五项 pre-commit gate，再按实际 diff 拟 commit。
- 若用户要求拆分：按 Phase 29 parser/byte-audit、spec-kit/docs、Phase 30
  IDA docs 拆成 reviewable units。
- 若用户打开新 IDB：回到 Phase 30 IDA checklist。

### Phase 30-H：final pre-commit gate run（同日继续）
- 用户继续后，在无新 IDB 前提下执行提交/评审前门禁验证；未提交。
- 结果：
  - `cargo fmt --all -- --check`：通过。
  - `cargo build --locked --workspace --all-targets`：通过。
  - `cargo test --locked --workspace --all-targets`：通过。
  - `cargo clippy --locked --workspace --all-targets -- -D warnings`：
    通过。
  - `bash .github/scripts/check-missing-docs.sh`：本地 Windows bash
    环境失败，输出乱码 / `REGDB_E_CLASSNOTREG`；与既有记录一致，
    不采信为代码失败。
  - fallback `cargo rustdoc --lib --locked -- -W missing-docs`：通过。
- 更新 readiness 文档：
  `docs/analysis/2026-06-12-phase29-30-worktree-readiness.md`。

### Phase 30-H 后续触发条件
- 当前工作树已通过可执行本地门禁；下一步需用户授权提交/拆分，或打开
  新 IDB 继续 IDA-gated 问题。

### Phase 30-I：commit/review plan（同日继续）
- 用户继续后，因没有新 IDB，且 commit 需要明确授权，未创建提交。
- 新增提交/评审方案：
  `docs/analysis/2026-06-12-phase29-30-commit-review-plan.md`。
- 方案给出两种 review shape：
  - single milestone commit：保留 Phase 29/30 叙事完整性。
  - three review units：
    1. Phase 29 parser / byte-audit implementation。
    2. Phase 29 probe / spec / analysis artifacts。
    3. Phase 30 IDA evidence / handoff docs。
- 记录建议 commit message、关键 review 文件、后续命令序列和 residual
  risks（大 diff、LF→CRLF warning、剩余 IDA-gated semantic questions）。

### Phase 30-I 后续触发条件
- 等待用户明确选择：单提交、拆分、评审，或打开新 IDB。

### Phase 30-J：focused self-review（同日继续）
- 用户继续后，在无提交授权且无新 IDB 的前提下执行提交前自审；未提交。
- 检查：
  - `git diff --check`：无 whitespace errors；仅 LF→CRLF warning。
  - `ReadLints` scoped to关键 Rust parser/test files：无诊断。
  - 结合已通过的 fmt/build/test/clippy/rustdoc fallback。
- 自审重点：
  - `decode_cluster_body_record_at` 的 checked arithmetic、长度 cap、
    payload fit gate。
  - `decode_cluster_body_records` 的 cursor advance 不会零长度循环。
  - `decode_unclustered_da_body_records` 的 full-coverage gate。
  - `parse_attribute_records_chain_scoped` 的 exact record bounds 与
    legacy fallback。
  - `parse_jsites_list_with_trace` 的 count/align gate、checked math、
    stale tail 不 claim。
  - nested JSite dispatch 对未知 children 维持 unregistered。
- 结论：focused self-review 未发现 blocking code issue。
- 新增：
  `docs/analysis/2026-06-12-phase29-30-self-review.md`。

### Phase 30-J 后续触发条件
- 当前可继续的非 IDB 工作已经收口；等待用户选择单提交、拆分、评审，
  或打开新 IDB。
