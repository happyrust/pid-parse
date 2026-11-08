# Implementation Plan: 出图记账（pdf-export-ledger）

## Overview

按 design.md 实施，语言为 Rust。顺序是：先确认并发改动已落定、在改库代码之前记下字节对照的「实施前」快照；再自底向上建原因目录与账本类型、根 crate 的发射器账、写出端判定、排版挂钩、汇总、基线与门禁；最后接上两道门禁与 CLI，取首次基线，做字节对照与全部门禁的收尾检查。

每一步都要保持画法不变：写出的 PDF 字节、`PaperModel` 已有字段、`tests/golden/geometry/*.json` 都不许动。需求偏差 D1、D2 已于 2026-11-04 确认按推荐处理。

## Tasks

- [ ] 1. 前置检查与实施前快照
  - [ ] 1.1 确认并发改动已落定
    - 运行 `git status --short`，查看 `pid-plot/src/{model,write,compare,fonts,witness}.rs`、`pid-plot/tests/reference_plots_acceptance.rs`、`pid-plot/examples/`、`docs/analysis/` 是否仍有他人未提交的改动
    - 若有：停下，向用户确认这些改动已提交或约定先后，再继续；不 stash、不 reset、不改动它们
    - 记下基点提交（`git rev-parse HEAD`），后续字节对照以它为准
    - _Requirements: 9.1, 9.2_

  - [ ] 1.2 编写字节对照探针
    - 新建 `pid-plot/examples/probe_export_bytes_unchanged.rs`，子命令 `record <label>` 与 `compare <before> <after>`，三批页（备份 53、夹具 7、参考 11）各按其已有门禁的配置排版、准备字体、写出，按设计组件 10
    - 每页记 PDF 的 SHA-256 与字节数、`serde_json::to_vec(&model)` 的 SHA-256；产物写到 `target/export-parity/<label>.json`，文件头记 HEAD、工作树是否有未提交改动、字型键名
    - 只调用实施前后签名不变的 API：`lay_out_drawing`、`font_dirs`、`prepare_fonts`、`write_plot`
    - `pid-plot/Cargo.toml` 的 dev-dependencies 加 `sha2 = "0.11"`，更新 `Cargo.lock`
    - _Requirements: 9.1, 9.2, 9.3, 15.5_

  - [ ] 1.3 记下实施前快照与耗时
    - 在尚未改库代码的工作树上运行两次 `record`：共享语料（默认变量）记为 `before-share`；备份包语料（`PID_PARSE_SELF_CHECK_DIR=D:\work\cad\pid-test-data`、`PID_PLOT_FONTS=D:\work\cad\PID自研软件验证\fonts`）记为 `before-package`
    - 在备份包语料下运行一次 `cargo test --locked -p pid-plot --test export_plant_backup`，把 cargo 报告的耗时写到 `target/export-parity/timing-before.txt`
    - _Requirements: 9.1, 9.2, 9.3, 11.4_

- [ ] 2. 原因目录与账本类型
  - [ ] 2.1 建立原因目录 `ledger::reason`
    - 新建 `pid-plot/src/ledger.rs`（模块根，顶部 `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]`）与 `pid-plot/src/ledger/reason.rs`；`pid-plot/src/lib.rs` 加 `pub mod ledger;`
    - `DropReason` 43 个变体按附录 A 的行序声明，derive `Ord` 与 serde（`snake_case`）；实现 `ALL`、`code()`、`tier()`、`layer()`；每个变体的 rustdoc 写判据与代码位置；`point_no_marker` 与 `point_marker_degenerate` 按 D2 口径（有无 marker）写
    - 定义 `Tier`、`ReasonLayer`、`PartialMark`、`Asymmetry`、`PlacementSource`、`LedgerDomain`、`DomainPair<T>`（按 `match` 取值，不用下标）
    - `pid-plot/Cargo.toml` 与根 `Cargo.toml` 的 dev-dependencies 加 `proptest = "1"`，更新 `Cargo.lock`
    - _Requirements: 2.2, 2.5, 10.4, 14.4_

  - [ ]* 2.2 原因目录单测
    - `ALL` 的 `code()` 序列等于附录 A 码表；`tier()` 等于附录 A 的档位列（可疑 22 个、有意 21 个）；serde 名等于 `code()`
    - _Requirements: 2.2, 2.5_

  - [ ] 2.3 定义账本数据类型
    - `pid-plot/src/ledger/layout.rs`：`LayoutLedger`（`Default`、`Eq`）、`EntityRecord`、`Site`、`SourceName`、`SourceKind`、`PageSources`、`TemplateTrace`、`EntityLayout`、`PlacementLayout`、`BodyItemLayout`、`BodyTally`、`TextNormalization`、`EntityKind`、`Confidence`
    - `pid-plot/src/ledger/assemble.rs`：`ExportLedger`、`PageAccount`、`DomainAccount`、`ParseAccount`、`RecordItem`、`EntityAccount`、`BodyAccount`、`PrimitiveAccount`、`SegmentAccount`、`SuspectSite`、`TemplateFact`、`PageSizeSource`；全部 Serialize 与 Deserialize，`type_code` 以 `0xNNNN` 字符串读写，映射一律 `BTreeMap`，不含浮点数
    - `ledger.rs` 声明 `layout`、`assemble`、`baseline` 三个子模块；`baseline.rs` 先只放模块文档
    - _Requirements: 6.4, 10.2, 12.2, 12.4, 14.4_

- [ ] 3. 根 crate 的发射器账
  - [ ] 3.1 在 `src/geometry.rs` 记发射器账
    - 新增公开结构 `PidEmitterAccount`（文档齐全，derive 同 `PidDroppedGraphicRecords`）与字段 `NormalizedPidGeometry::emitter_accounts`（`#[serde(default, skip_serializing_if = "Vec::is_empty")]`）
    - `GeometryEmitter::emit` 改为返回私有的 `EmitSkips { out_of_range, shape_rejected }`；11 个发射族在 `source_range(..)` 为 `None` 时计 `out_of_range`；`igLineString2d`、`igBoundary2d`、`igBspCurve2d` 的形状跳过计 `shape_rejected`
    - `EMITTERS` 循环按 `SHEET_RECORD_FAMILIES` 的 `record_count` 填 `decoded`，按 `entities.len()` 的前后差填 `emitted`，两者都为 0 的行不记；发射顺序不变
    - 去掉 `family()`、`is_no_op()` 上的 `#[allow(dead_code)]`；补上约 4330 行测试里的字面量
    - 确认 `tests/geometry_golden_snapshot.rs` 不带 UPDATE 通过，`cargo test --locked -p pid-parse --no-default-features --lib`、wasm32 的 `cargo check` 与 `bash .github/scripts/check-missing-docs.sh` 通过且计数不变
    - _Requirements: 3.4, 3.5, 3.6, 9.5, 15.4_

  - [ ]* 3.2 发射器跳过分支单测
    - 每个跳过分支一例：任选一个发射族的越界字节范围；`igLineString2d` 顶点数不等、全重合且无顶点；`igBoundary2d` 少于 3 段；`igBspCurve2d` 采不出 2 点；无操作族的 `audit_only` 行
    - _Requirements: 3.4, 3.5_

  - [ ]* 3.3 性质测试：发射器族级守恒
    - **Property 7: 发射器族级守恒**
    - **Validates: Requirements 3.5, 3.6**
    - 放在 `src/geometry.rs` 测试模块，`ProptestConfig::with_cases(256)`，加标签注释

- [ ] 4. 写出端判定与写出账
  - [ ] 4.1 实现写出端纯判定 `write::verdict`
    - 新建 `pid-plot/src/write/verdict.rs`（顶部同样的 clippy deny），`pid-plot/src/write.rs` 加 `mod verdict;` 与需要的 `pub use`
    - 实现 `fill_fate`、`pen_fate`、`segment_fate`、`text_fate` 与 `primitive_verdict`，判据与短路顺序逐字照搬 `paint_primitive`、`stroke_runs`、`show_text` 的现有表达式；高度为 `None` 时按 `DEFAULT_TEXT_MM` 判
    - 定义 `PrimitiveFate`、`PrimitiveOutcome`、`SegmentTally`、`TextMarks`、`WriteLedger`、`WriteMismatch`
    - _Requirements: 2.3, 5.1, 5.2, 5.3, 5.5, 10.3, 10.4_

  - [ ]* 4.2 A.5 逐码单测
    - 每个 A.5 原因码一例，含阈值两侧（1e-4 mm、0.2822 mm、0.9 倍页边）与家具段对 `segment_on_sheet_edge` 的豁免
    - _Requirements: 2.3, 2.4, 5.1, 5.2_

  - [ ]* 4.3 性质测试：写出判定优先级
    - **Property 3: 写出判定取固定优先级中的第一个命中**
    - **Validates: Requirements 5.1, 5.2, 5.3, 10.3**
    - 放在 `write/verdict.rs` 测试模块；神谕按附录 A.5 的顺序在测试里另写

  - [ ] 4.4 让写出器调用判定并返回写出账
    - `stroke_runs` 的 `prints` 改为 `segment_fate(..).is_none()`；`paint_primitive` 的两处早退改调 `fill_fate`、`pen_fate` 并返回 `Painted`；`show_text` 的三处早退改调 `text_fate` 并返回 `Shown`（压缩与尾部裁切标记与 `PlotAudit` 计数同处置位）；`push_layer` 用 `.enumerate()` 把观测交给 `WriteLedger`
    - 新增 `WrittenPlot { pdf, audit, ledger }`；`write_plot_audited` 返回它：先对全部图元求判定，写页时收观测，写完对账填 `mismatches`；新增无字体入口 `write_ledger`；`write_plot` 取 `.pdf`
    - 适配两个外部调用方，只改解构：`pid-plot/tests/export_plant_backup.rs` 约 1330 行、`pid-plot/src/bin/pid_plot.rs` 约 59 行
    - 运行 `cargo test --locked -p pid-plot`，现有断言一条不改
    - _Requirements: 5.5, 5.6, 9.1, 9.4, 11.2, 14.2_

  - [ ]* 4.5 性质测试：判定与真实写出一致
    - **Property 5: 判定与真实写出一致**
    - **Validates: Requirements 5.5, 5.6**
    - 放在 `write.rs` 测试模块，字体用 `pdfcraft_fonts::INTER_REGULAR`，文字限 ASCII，`with_cases(128)`

  - [ ]* 4.6 A.6 字体标记单测
    - 用 `INTER_REGULAR` 写一条比页面长的文字，断言 `text_condensed` 与 `PlotAudit::runs_condensed` 一致；再构造压到 `MIN_CONDENSE` 仍越界的文字，断言 `text_clipped_at_floor` 与 `runs_clipped_at_the_condensation_floor` 一致
    - _Requirements: 5.4, 8.6_

- [ ] 5. Checkpoint - 写出器改完
  - Ensure all tests pass, ask the user if questions arise.
  - 另用探针记 `mid-share` 与 `mid-package`，与 `before-*` 做 `compare`，确认 71 页的 PDF 与模型哈希都没变

- [ ] 6. 排版记账
  - [ ] 6.1 给 `PaperModel` 加 `ledger` 字段
    - `pid-plot/src/model.rs`：`PaperModel` 加 `#[serde(skip)] pub ledger: LayoutLedger` 并写 rustdoc；生产构造（约 1124 行）先填 `Default::default()`
    - 更新 20 处测试字面量：`write.rs` 14 处、`model.rs` 4 处、`witness.rs` 1 处、`fonts.rs` 1 处
    - _Requirements: 9.3, 14.1_

  - [ ] 6.2 实现记账器 `LedgerRecorder`
    - `pid-plot/src/ledger/layout.rs`：`sources`、`parse`、`template_search`、`begin`、`finish`、`observe`、`note_retained`、`into_ledger`，按设计组件 2
    - `parse` 从三张记录表、`sheet_streams`、`jsites`、`symbol_definitions`、`emitter_accounts` 出 A.1 全部计数项，算族级守恒差额，并记该域实体总数
    - `finish` 用 `primitives.len()` 认领新增图元；记账器另存家具域插入过的填充键，供图纸域的 `fill_duplicate` 注明先占者
    - 计数一律 `saturating_add`，下标转换用 `u32::try_from`，失败计入未归因
    - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7, 7.1, 7.4, 7.5, 10.3, 11.1_

  - [ ] 6.3 在排版器里接上记账
    - `model.rs` 新增 `plot_suppression`（`suppressed_from_plot` 改为委托它）、`drawing_precheck`、`furniture_observations`
    - `push_entity_with`、`push_entity`、`push_furniture_entity` 返回 `EntityLayout`；Point 臂按 D2 口径分 `point_no_marker` 与 `point_marker_degenerate`；`push_symbol_instance` 返回 `PlacementLayout`（含 `body_layer_hidden`、`symbol_missing`、`symbol_unresolvable`、`symbol_body_empty`、`kind_misrouted`）；`push_symbol_primitive` 返回 `BodyItemLayout`
    - 新增 `label_line_ranges_counted` 与 `load_border_template_traced`，原函数委托过去；`retain`（约 1115 行）改为带下标并调 `note_retained`
    - `lay_out_with_symbol_roots` 里接上记账器：符号来源、两次 `parse`、模板搜索、图纸与家具两个实体循环、`retain`、`into_ledger`
    - 只加返回值与记账调用，判断、push 与循环的顺序都不动
    - _Requirements: 1.3, 1.4, 2.3, 3.7, 4.1, 4.2, 4.3, 7.1, 7.2, 7.3, 7.4, 7.5, 9.1, 9.3, 11.1_

  - [ ]* 6.4 A.1 单测
    - 在 `ledger/layout.rs` 用构造的 `SheetStream`、`JSite`、`NormalizedPidGeometry` 覆盖 A.1 的 9 个原因码，以及族级守恒差额非零时记为未归因
    - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6_

  - [ ]* 6.5 性质测试：解析层计数项等于几何表
    - **Property 8: 解析层计数项等于几何表**
    - **Validates: Requirements 3.1, 8.5**
    - 放在 `ledger/layout.rs` 测试模块

  - [ ]* 6.6 排版层逐码单测与原因码覆盖表
    - 在 `model.rs` 测试模块实现 `every_reason_code_has_a_case`：对 `DropReason` 穷尽 `match` 的 `case_for(reason)` 构造触发输入并断言记出该码；`mark_case_for` 覆盖 4 个 `PartialMark` 与 2 个 `Asymmetry`
    - 覆盖 A.2、A.3、A.4、A.7、A.8 各码（沿用 `entity()`、`placement()`、`plain_item()`）；多判据同时成立时取第一个；`template_candidate_unreadable` 用临时目录里的坏 `.igr`
    - _Requirements: 1.4, 2.1, 2.4, 4.1, 4.2, 4.3, 7.2, 7.3, 7.4, 7.5_

  - [ ]* 6.7 性质测试：符号体图元守恒
    - **Property 6: 符号体图元守恒**
    - **Validates: Requirements 4.3, 4.4**
    - 放在 `model.rs` 测试模块

- [ ] 7. Checkpoint - 排版挂钩改完
  - Ensure all tests pass, ask the user if questions arise.
  - 再用探针记一份并与 `before-*` 做 `compare`，确认字节与模型哈希仍然不变

- [ ] 8. 汇总
  - [ ] 8.1 实现 `ExportLedger::assemble`
    - 按设计组件 5 合成实体结局（I2 的部分上页）、体账、段账、交叉计数与可疑来源；`past_the_edge` 由 `primitives_past_the_edge(0.5)` 按指针对回下标
    - 计算五层守恒差额并写入各层 `unattributed`（为 0 也写）；实现 `conservation_breaches`、`cross_check`（8.1–8.7）与 `summary`
    - 无字体账本不出两个字体标记（I1）；段的总数与已写数照写
    - _Requirements: 1.1, 1.2, 1.3, 1.5, 1.6, 1.7, 4.4, 5.4, 6.1, 6.2, 6.3, 6.4, 8.1, 8.2, 8.3, 8.4, 8.5, 8.6, 8.7_

  - [ ]* 8.2 性质测试：实体结局合成
    - **Property 1: 实体结局唯一且端到端合成**
    - **Validates: Requirements 1.1, 1.2, 1.3, 1.5, 6.1**

  - [ ]* 8.3 性质测试：交叉计数与部分上页
    - **Property 2: 交叉计数与部分上页计数自洽**
    - **Validates: Requirements 1.6, 1.7**

  - [ ]* 8.4 性质测试：写出层守恒
    - **Property 4: 写出层守恒**
    - **Validates: Requirements 6.2, 6.3, 5.4**

  - [ ]* 8.5 汇总单测
    - `past_the_edge` 与 `segments_dropped` 各一例；序列化后两域各层的 `unattributed` 键齐全；`summary()` 的格式；`cross_check` 每条不符时都报出
    - _Requirements: 5.4, 6.4, 8.1, 8.7, 14.3_

- [ ] 9. 基线与门禁
  - [ ] 9.1 实现 `ledger::baseline`
    - `LedgerBaseline`、`CorpusDescriptor`（D1：字体部分记字型键名，比较时分「符号或模板」与「字体」两部分）、`GateInput`、`run_gate`、`GateReport`，按设计组件 7
    - 比对单位为计数项；可疑项升高列来源、降低进「基线待降」、其余变化进「账目变动」；不比段总数与已写数；逐页结论只用两种措辞
    - 描述不同时输出以 `export-ledger:` 开头的醒目段落，含逐项差异与应设的变量；未归因与核对照查
    - `UPDATE_EXPORT_LEDGER` 等于 `1` 时按设计的条件重写或拒绝；写盘用 `serde_json::to_string_pretty` 加末尾换行与 `std::fs::write`
    - _Requirements: 6.5, 12.1, 12.3, 12.5, 12.6, 12.8, 13.4, 13.5, 13.6, 13.7, 13.8, 15.1, 15.2_

  - [ ]* 9.2 性质测试：基线往返与确定性
    - **Property 9: 基线往返与确定性**
    - **Validates: Requirements 12.8, 10.1, 10.2**

  - [ ]* 9.3 性质测试：门禁比对的分类与措辞
    - **Property 10: 门禁比对的分类与措辞**
    - **Validates: Requirements 13.4, 13.5, 13.6, 6.5, 15.1, 15.2**

  - [ ]* 9.4 性质测试：重写棘轮
    - **Property 11: 重写只许可疑项不升**
    - **Validates: Requirements 12.6, 12.5**
    - 用临时目录读写基线文件

  - [ ]* 9.5 门禁单测
    - 描述的符号或模板部分不同：跳过计数比对、打印差异、未归因大于 0 时仍失败；只有字体部分不同：只跳过两个字体标记；失败页；基线缺失、损坏或 `schema` 不同；`UPDATE_EXPORT_LEDGER` 取值不是 `1`
    - _Requirements: 13.7, 13.8, 12.5_

- [ ] 10. 接上门禁与 CLI
  - [ ] 10.1 新建夹具门禁
    - 新建 `pid-plot/tests/export_ledger_fixtures.rs`：按术语表 Fixture_Set 的 7 个路径取页；仓库语料为 `corpus = test-file`、`roots = [test-file/symbols]`、`test-file/backup-test/*_p` 下经 `refdata_archives` 列出并按路径排序的压缩包
    - 每页 `lay_out_with_symbol_roots` → `write_ledger` → `assemble` → `cross_check`（重新解析得几何做 8.5）；同页再排一遍，两次序列化逐字节相同；扫描序列化结果里的仓库根、`CARGO_MANIFEST_DIR`、主目录与盘符路径
    - 调 `run_gate`，基线为 `docs/baselines/export-ledger-fixtures.json`，`UPDATE_EXPORT_LEDGER` 从环境读
    - _Requirements: 8.1, 8.2, 8.3, 8.4, 8.5, 8.7, 10.1, 12.4, 12.7, 13.3, 13.8_

  - [ ] 10.2 在 `export_plant_backup` 里接上备份门禁
    - 写出循环里对每页 `assemble`、`cross_check(model, Some(&written.audit), None)`，要求 `mismatches` 为空，并用 `write_ledger(model)` 求无字体账、与写出账（去掉字体标记）比对
    - 页键为相对 `backup-plant-data` 的路径，以 `/` 分隔；循环后调 `run_gate`，基线为 `docs/baselines/export-ledger-backup.json`，`font_faces` 取本次 `prepare_fonts` 的键名，`hint` 写明取基线的变量
    - 原有断言与钉住的常量一条不改；备份不在时照旧软跳过
    - _Requirements: 5.5, 5.6, 8.1, 8.2, 8.3, 8.4, 8.6, 8.7, 9.4, 11.3, 13.1, 13.2_

  - [ ] 10.3 CLI 打印账本汇总行
    - `pid-plot/src/bin/pid_plot.rs` 写完 PDF 后用 `ExportLedger::assemble` 与 `summary()` 打印 `ledger entities … on_page … intentional … suspect … unattributed …`
    - _Requirements: 14.3_

- [ ] 11. Checkpoint - 门禁接好
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 12. 首次基线与文档
  - [ ] 12.1 取夹具基线
    - 运行 `UPDATE_EXPORT_LEDGER=1 cargo test --locked -p pid-plot --test export_ledger_fixtures`，生成 `docs/baselines/export-ledger-fixtures.json`；不带变量再跑一遍，确认通过
    - 逐条看可疑项的来源；若出现未归因或 I3 所说的空成员 `Boundary`，停下向用户报告，不写基线
    - _Requirements: 12.1, 12.2, 12.3, 12.7, 13.3_

  - [ ] 12.2 取备份基线
    - 在备份包语料下（`PID_PARSE_SELF_CHECK_DIR=D:\work\cad\pid-test-data`、`PID_PLOT_FONTS=D:\work\cad\PID自研软件验证\fonts`、`UPDATE_EXPORT_LEDGER=1`）运行 `cargo test --locked -p pid-plot --test export_plant_backup`，生成 `docs/baselines/export-ledger-backup.json`；不带 `UPDATE_EXPORT_LEDGER` 再跑一遍确认通过，再用默认变量跑一遍确认跳过提示醒目
    - 可疑项与本机 `target/audit/backup-gaps.txt` 的普查（无解码器 1 条、被拒 8 条）对照；若写首次读数的分析文档，放在 `docs/analysis/YYYY-MM-DD-*.md`，结论只用「未发现可归属缺陷」或列出具体缺陷
    - _Requirements: 12.1, 12.2, 12.3, 13.1, 15.6_

  - [ ] 12.3 写基线说明
    - 在 `docs/baselines/README.md` 新增一节：两份基线的语料、比对规则（可疑只许降、有意精确、段总数不比、字体部分的处理）、两道门禁的完整调用命令、重写方法与拒绝条件
    - 按仓库惯例，在 `AGENTS.md` 的 Test gates 表加 `export_ledger_fixtures` 一行，并在 `export_plant_backup` 一行注明账本门禁
    - _Requirements: 12.9_

- [ ] 13. Final checkpoint - 收尾检查
  - Ensure all tests pass, ask the user if questions arise.
  - 跑五道提交前门禁与根 crate 的两道门禁（需求 15.3、15.4）
  - 用探针记 `after-share`、`after-package`，与 `before-*` 做 `compare`，53 + 7 + 11 页的 PDF 与模型哈希全部相同（需求 9.1–9.3）
  - 在备份包语料下再跑一次 `export_plant_backup`，与 `timing-before.txt` 比较，增幅不超过 10%（需求 11.4）

## Notes

- 标 `*` 的子任务是测试任务，可以为求快跳过；核心实现任务都不带 `*`。
- 1.3 的快照必须在任何库代码改动之前记下。若基点提交变了，在新基点的独立 worktree 上重记，并给该 worktree 单独的 `CARGO_TARGET_DIR`（见 `AGENTS.md`）。
- 第 5、7 两个检查点各做一次字节对照，把写出器改造与排版挂钩各自引起的字节变化（如果有）分开定位。
- 每个性质测试对应 design.md 的一条性质，至少 100 例，测试上方注释 `// Feature: pdf-export-ledger, Property N: <性质标题>`。
- 每个任务都列了对应的需求条目，便于追溯；检查点用于分段验证。
- 改动根 crate（任务 3.1）后要额外通过 `--no-default-features` 与 wasm32 两道门禁。

## Task Dependency Graph

```json
{
  "waves": [
    { "id": 0, "tasks": ["1.1"] },
    { "id": 1, "tasks": ["1.2"] },
    { "id": 2, "tasks": ["1.3"] },
    { "id": 3, "tasks": ["2.1", "3.1"] },
    { "id": 4, "tasks": ["2.2", "2.3", "3.2", "4.1"] },
    { "id": 5, "tasks": ["3.3", "4.2", "4.4", "6.2"] },
    { "id": 6, "tasks": ["4.3", "4.5", "6.4"] },
    { "id": 7, "tasks": ["4.6", "6.5"] },
    { "id": 8, "tasks": ["6.1"] },
    { "id": 9, "tasks": ["6.3"] },
    { "id": 10, "tasks": ["8.1"] },
    { "id": 11, "tasks": ["6.6", "8.2", "9.1"] },
    { "id": 12, "tasks": ["6.7", "8.3", "9.2", "10.1", "10.2", "10.3"] },
    { "id": 13, "tasks": ["8.4", "9.3"] },
    { "id": 14, "tasks": ["8.5", "9.4"] },
    { "id": 15, "tasks": ["9.5"] },
    { "id": 16, "tasks": ["12.1", "12.2"] },
    { "id": 17, "tasks": ["12.3"] }
  ]
}
```
