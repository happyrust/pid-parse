# 进度日志：PID 解析开发方案

## Session: 2026-04-30

### 当前状态
- **Phase:** 2 - PSM 结构化补齐
- **状态:** Phase 2 当前执行切片完成，待提交/推送或进入 Phase 3

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
