# Requirements Document

出图记账（pdf-export-ledger）需求文档。结构标题沿用规格模板的英文，正文用中文。

## Introduction

我方出图（Rendered Plot）现在有两道把关。带参考出图（Reference Plot）的 11 页由 `pid-plot/tests/reference_plots_acceptance.rs` 按覆盖率验收：其中 10 页 coverage 在 0.9984–0.9997、多画 0 mm，A2 那页 0.9818，剩余差异已有分析归到渲染器之外。另外 42 张备份图没有参考出图，`pid-plot/tests/export_plant_backup.rs` 只能证明它们写得出页、页上有墨有字。解码器或页面模型少画一批东西时，只要页面没空到无墨无字，现有门禁都发现不了。

出图记账给每一页记一本账：页上的每条图形记录、每个实体、每个页面图元、每一段笔画，要么上页，要么归到一个有名字的丢弃原因并计数。记账不依赖参考出图，所以没有参考出图的 42 张也能覆盖到。53 张备份图和 7 个已入库夹具逐页存成基线，没有归因的丢弃必须为 0。

边界：

- 记账只观测，不改画法：写出的 PDF 字节和所有现有门禁的读数都保持不变。
- 不新增阈值，也不为迎合测量改现有阈值（依据 `docs/plans/2026-10-29-plot-pdf-correctness.md` §4、§5）。没有参考出图的页，账本最多支持「未发现可归属缺陷」，不支持「正确」。
- 不在本 spec 内：修 D04、EF井场等被拒或没有解码器的记录；让 CI 写出 PDF（字体问题）；继续压参考出图残差；扩大参考集。

附录 A 是按 2026-11-04 磁盘上的代码穷举的原因目录，覆盖 `src/geometry.rs`、`src/streams/cluster.rs`、`src/symbol_library.rs`、`pid-plot/src/model.rs`、`pid-plot/src/write.rs`、`pid-plot/src/read.rs` 中导出路径上的每个丢弃分支；「今天」一列标出哪些分支现在没有计数，其中标「新发现」的是需求阶段读代码补出来的。附录 B 是需求阶段确认的取向。

## Glossary

- **Graphic_Record（图形记录）**：Sheet 流里的一条 PSM 记录，满足其一：类型码被 SmartPlant 的图形谓词接受（普查里 `is_graphic` 为真），或属于 `SHEET_RECORD_FAMILIES` 已注册的记录族。约束、关系、样式等非图形记录不在账内。
- **Entity（实体）**：`NormalizedPidGeometry::entities` 里的一个 `PidGraphicEntity`。
- **Paper_Primitive（页面图元）**：`PaperModel::primitives` 里的一个 `PaperPrimitive`。一个实体可以产出零个、一个或多个页面图元（多行文字、复杂线型盖印、符号体展开）。
- **Body_Primitive（符号体图元）**：符号体里的一个 `SymbolPrimitive`。符号体指文件内缓存体 `PidSymbolDefinition` 或符号库体 `SymbolGeometry`。
- **Segment（段）**：写出端把一个笔画图元的点链（弧与圆按 `CHORD_MM` 展平后）切成的一条直线段。
- **Drawing_Domain（图纸域）**：`.pid` 自身的记录与实体。
- **Furniture_Domain（家具域）**：页面所穿 `.igr` 边框模板的记录与实体。
- **On_Page（上页）**：页面图元满足其一即上页：`Fill` 被填充；笔画至少有一段被描出；文字至少写出一个字形串（`BT … ET`）。实体至少有一个页面图元上页，即为上页。
- **Partially_On_Page（部分上页）**：已上页，但有段被丢弃、文字被压缩或尾部被页面裁掉、或墨越出页边。
- **Drop_Reason（丢弃原因）**：附录 A 中的一个原因码。
- **Intentional_Drop（有意丢弃）**：代码依 SmartPlant 的出图口径或文件自己的声明而不画的内容，如隐藏图层、`ConsistencyChecks`、原生跳过位。
- **Suspect_Drop（可疑丢弃）**：可能是覆盖缺口（Coverage Gap）的丢弃，如解码器拒绝、零高文字、零宽笔。
- **Unattributed_Drop（未归因丢弃）**：守恒式算出的差额：输入数减去上页数，再减去各原因计数之和。
- **Reason_Catalog（原因目录）**：附录 A。以代码为准，穷举导出路径上的丢弃分支。
- **Export_Ledger（出图账本）**：一页的账，由页级事实，加图纸域、家具域各自的解析层、排版层、写出层计数组成。
- **Ledger_Builder（记账器）**：在排版与写出过程中产出出图账本的组件。
- **Layout_Engine（排版器）**：`pid_plot::model::lay_out_drawing` 与 `lay_out_with_symbol_roots`。
- **Plot_Writer（写出器）**：`pid_plot::write::write_plot_audited` 与 `write_plot`。
- **Geometry_Projection（几何投影）**：`pid_parse::build_normalized_geometry` 及其注册的 `GeometryEmitter`。
- **Plot_CLI**：`pid-plot/src/bin/pid_plot.rs`。
- **Ledger_Baseline（账本基线）**：入库的账本基线文件。
- **Ledger_Gate（账本门禁）**：把本次账本与账本基线比对、并检查未归因丢弃为 0 的测试。
- **Existing_Gates（现有门禁）**：本 spec 开始前已有的全部测试，包括 `export_plant_backup`、`export_every_fixture`、`reference_plots_acceptance`、`plot_compare`、`write_five_drawings`、`style_witness`、`border_template`、`refdata_archive_corpus`、`geometry_golden_snapshot`、`render_gap_census`。
- **Byte_Parity_Check（字节对照）**：对同一批页，比对实施前后写出的 PDF 字节。
- **Backup_Corpus（备份语料）**：`backup-plant-data/**/*.pid` 的 53 张图，在 `PID_TEST_DATA_DIR`、语料目录或两份 `pid-test-data` 下查找。
- **Fixture_Set（夹具集）**：7 个已入库的 `.pid`：`test-file/D06.pid`、`test-file/DWG-0201GP06-01.pid`、`test-file/DWG-0202GP06-01.pid`、`test-file/export-test/publish-data/A01/A01.pid`、`test-file/export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid`、`test-file/工艺管道及仪表流程-1.pid`、`test-file/集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图.pid`。按这份清单取，不按目录遍历取。
- **Reference_Set（参考集）**：`PID_ACCEPTANCE_CORPUS`（默认 `验收数据`）下带参考出图的 11 页。
- **Repository_Corpus（仓库语料）**：只由已入库文件组成的语料：`test-file/Ref/Template Files` 的 4 个 `.igr`、`test-file/symbols` 的 3 个 `.sym`、`test-file/backup-test/*_p/RefData~*.zip`。不含被 gitignore 的 `test-file/symbols-full`。
- **Corpus_Descriptor（语料描述）**：一次排版所用符号库来源、模板来源与字体来源的描述，只用文件名与来源种类表达，不含本机绝对路径。
- **Ledger_Change（本次改动）**：实现本 spec 的全部代码、测试与文档改动。

## Requirements

### Requirement 1: 每个实体一个结局

**User Story:** 作为维护出图的开发者，我希望页上每个实体都有且只有一个结局，以便没有参考出图的页少画了东西也能被发现。

#### Acceptance Criteria

1. THE Ledger_Builder SHALL 为每页每个账域中的每个 Entity 记下恰好一个结局：On_Page，或 Reason_Catalog 中的一个 Drop_Reason。
2. WHEN 一个 Entity 产出的 Paper_Primitive 中至少有一个 On_Page, THE Ledger_Builder SHALL 把该 Entity 记为 On_Page。
3. WHEN Layout_Engine 没有为一个 Entity 产出任何 Paper_Primitive, THE Ledger_Builder SHALL 把该 Entity 记为 Layout_Engine 判定该 Entity 时命中的 Drop_Reason。
4. WHEN 一个 Entity 同时满足多个排版层判据, THE Ledger_Builder SHALL 记 Layout_Engine 判定顺序中第一个命中的 Drop_Reason。
5. WHEN 一个 Entity 产出的 Paper_Primitive 全部被移除或丢弃, THE Ledger_Builder SHALL 把该 Entity 记为这些图元的原因中在附录 A 里排位最前的那个。
6. THE Ledger_Builder SHALL 把每个账域的 Entity 按种类（`PidGraphicKind` 变体）、置信度（`decoded` / `inferred` / `probe_only`）与结局三者交叉计数。
7. THE Ledger_Builder SHALL 另计 On_Page 的 Entity 中属于 Partially_On_Page 的数目。

### Requirement 2: 原因目录

**User Story:** 作为开发者，我希望代码里每一个丢弃分支都对应一个有名字的原因码，以便不再有静默分支。

#### Acceptance Criteria

1. WHEN 附录 A 某一行的判据对一条记录、一个实体、一个图元或一段成立, THE Ledger_Builder SHALL 在该行所属的层记下该行的原因码。
2. THE Ledger_Builder SHALL 把每个 Drop_Reason 归入 Intentional_Drop 或 Suspect_Drop，归档以附录 A 的「档位」列为准。
3. THE Ledger_Builder SHALL 只使用代码里已有的判据与阈值，包括 1e-4 mm、`sheet_edge` 的 1 mm 与 0.9 倍页边、`read::sheet_edge_mm` 的 0.282 mm 与 0.9 倍页边、`SHEET_REACH = 4`。
4. THE Ledger_Change SHALL 为附录 A 的每个原因码提供至少一个单元测试：该测试构造触发对应分支的输入，并断言 Ledger_Builder 记出该原因码。
5. THE Ledger_Builder SHALL 把原因码定义为一个封闭集合，并在每个原因码的 rustdoc 中写明判据与代码位置，与附录 A 逐行对应。

### Requirement 3: 没有变成实体的图形记录

**User Story:** 作为开发者，我希望没有变成实体的图形记录也记在同一本账上，以便 D04 的 5 个被拒 `igSymbol2d` 这类缺口逐页可见。

#### Acceptance Criteria

1. THE Ledger_Builder SHALL 按（流路径，类型码）把 `NormalizedPidGeometry` 的 `dropped_graphic_records`、`refused_graphic_records`、`skipped_graphic_records` 分别记为 `record_no_decoder`、`record_refused`、`record_native_skip`。
2. THE Ledger_Builder SHALL 按流路径记下 `SheetStream::geometry` 为 `None` 的 Sheet 流及其字节数，原因 `sheet_stream_without_geometry`。
3. THE Ledger_Builder SHALL 按存储记下嵌套 `JSite*/PSMcluster0` 的符号体记录数（`record_symbol_body_cache`），并记下没有任何放置引用的缓存定义数及其记录数（`cache_definition_unplaced`）。
4. THE Ledger_Builder SHALL 按记录族记下已解码、但所属族在 `EMITTERS` 中为无操作发射器的记录数（`record_audit_only_family`）。
5. THE Ledger_Builder SHALL 按记录族记下已解码、但被发射器跳过的记录数（`record_emitter_out_of_range`、`record_emitter_shape_rejected`）。
6. FOR ALL 发射几何的记录族与 Sheet 流，该族的已解码记录数 SHALL 等于该族产出的 Entity 数与该族被发射器跳过的记录数之和（族级守恒）。
7. THE Ledger_Builder SHALL 对页面所穿 `.igr` 文档记同样的解析层计数，计入家具域。

### Requirement 4: 符号放置与符号体

**User Story:** 作为开发者，我希望每个符号放置画了什么、从哪个体画、体内哪些图元没画都有账，以便把符号少画和符号库缺口分开。

#### Acceptance Criteria

1. THE Ledger_Builder SHALL 为每个 `SymbolInstance` 记下符号体来源：`cached`、`library` 或 `missing`。
2. WHEN 一个 `SymbolInstance` 既没有符号路径、也没有可解析的缓存体, THE Ledger_Builder SHALL 记 `symbol_unresolvable`，并记下该放置的流路径与 oid。
3. THE Ledger_Builder SHALL 按 Body_Primitive 计数符号体内的丢弃（附录 A.3）。
4. FOR ALL 解析到符号体的放置，该体的 Body_Primitive 总数 SHALL 等于产出了 Paper_Primitive 的 Body_Primitive 数与附录 A.3 各原因计数之和。

### Requirement 5: 写出层

**User Story:** 作为开发者，我希望写出器每一处不写的分支都留下原因码，以便零高文字、零宽笔这类丢弃在页上可数。

#### Acceptance Criteria

1. WHEN Plot_Writer 不写一个 Paper_Primitive, THE Ledger_Builder SHALL 记下附录 A.5 中命中的第一个原因码。
2. WHEN Plot_Writer 丢掉一个笔画图元的某一段, THE Ledger_Builder SHALL 在段层记下该段的原因码。
3. WHEN 一个笔画图元的所有段都被丢掉, THE Ledger_Builder SHALL 把该图元记为未上页，原因取其段原因中在附录 A.5 里排位最前的那个。
4. WHEN 一个图元上页但被压缩、被页面裁掉尾部、墨越出页边超过 0.5 mm 或有段被丢掉, THE Ledger_Builder SHALL 把该图元记为 Partially_On_Page，并注明附录 A.6 中的哪一种。
5. THE Ledger_Builder SHALL 在不读取任何字体的情况下判定每个 Paper_Primitive 是否 On_Page 及其 Drop_Reason。
6. FOR ALL 页面模型，Ledger_Builder 判为 On_Page 的图元集合 SHALL 等于 Plot_Writer 实际写出路径（`S` 或 `f`）或字形串（`BT … ET`）的图元集合。

### Requirement 6: 守恒与未归因

**User Story:** 作为开发者，我希望每一层都有一条守恒式，以便任何没有登记原因的丢弃都会显出差额。

#### Acceptance Criteria

1. FOR ALL 页与账域，Entity 总数 SHALL 等于 On_Page 的 Entity 数、各 Drop_Reason 的 Entity 计数与实体层 Unattributed_Drop 之和。
2. FOR ALL 页与账域，Layout_Engine 产出的 Paper_Primitive 总数 SHALL 等于 `off_sheet` 计数、On_Page 图元数、附录 A.5 各原因的图元计数与图元层 Unattributed_Drop 之和。
3. FOR ALL 笔画图元，段总数 SHALL 等于写出的段数、附录 A.5 各段原因计数与段层 Unattributed_Drop 之和。
4. THE Export_Ledger SHALL 显式给出每层每域的 Unattributed_Drop 计数，计数为 0 时也给出。
5. IF 任何一页任何一层的 Unattributed_Drop 大于 0, THEN THE Ledger_Gate SHALL 失败，并列出页、账域、层与计数。

### Requirement 7: 账域与页级事实

**User Story:** 作为开发者，我希望模板家具的账和图纸自身的账分开，以便两条排版路径之间的不对称可以量出来。

#### Acceptance Criteria

1. THE Export_Ledger SHALL 把 Drawing_Domain 与 Furniture_Domain 分开计数。
2. THE Ledger_Builder SHALL 把家具域里位于隐藏层或 `ConsistencyChecks` 层、仍照常画出的 `SymbolInstance` 计为 `furniture_symbol_on_suppressed_layer`。
3. THE Ledger_Builder SHALL 把家具域里照常进入排版的 `EndpointPair` 实体计为 `furniture_endpoint_pair_drawn`。
4. WHEN 一个图纸域 `igBoundary2d` 填充因为与家具域填充同键而被丢弃, THE Ledger_Builder SHALL 在图纸域的 `fill_duplicate` 下注明先占该键的是家具域。
5. THE Export_Ledger SHALL 在页级记下页幅来源（声明或测得）、所穿模板的文件名与来源种类（目录或 `RefData~*.zip`）、`missing_templates`，以及 `template_candidate_unreadable` 计数。

### Requirement 8: 与现有计数一致

**User Story:** 作为开发者，我希望账本和代码里已有的计数逐项对得上，以便账本本身的错误能被现有读数抓到。

#### Acceptance Criteria

1. THE Export_Ledger 中图纸域 `endpoint_pair_guess` 的计数 SHALL 等于 `PaperModel::skipped_endpoint_pairs`。
2. THE Export_Ledger 中两个账域 `off_sheet` 计数之和 SHALL 等于 `PaperModel::off_sheet_primitives`。
3. THE Export_Ledger 中图纸域 `symbol_missing` 的计数 SHALL 等于 `PaperModel::missing_boxes` 的长度。
4. THE Export_Ledger 中家具域产出的 Paper_Primitive 总数 SHALL 等于 `BorderTemplate::primitives`。
5. THE Export_Ledger 中 `record_no_decoder`、`record_refused`、`record_native_skip` 的计数 SHALL 逐（流路径，类型码）等于 `NormalizedPidGeometry` 对应三张表的 `count`。
6. THE Export_Ledger 中 `text_condensed` 与 `text_clipped_at_floor` 的计数 SHALL 分别等于 `PlotAudit::runs_condensed` 与 `PlotAudit::runs_clipped_at_the_condensation_floor`。
7. THE Export_Ledger 中 `past_the_edge` 的计数 SHALL 等于 `PaperModel::primitives_past_the_edge(0.5)` 的长度。

### Requirement 9: 只观测，不改画法

**User Story:** 作为维护出图的开发者，我希望记账不改动任何一页的画法，以便已经验收过的页不受影响。

#### Acceptance Criteria

1. THE Plot_Writer SHALL 为 Backup_Corpus、Fixture_Set 与 Reference_Set 的每一页写出与实施前逐字节相同的 PDF。
2. WHEN Ledger_Change 完成, THE Byte_Parity_Check SHALL 报告上述每一页实施前后的 PDF 字节全部相同。
3. THE Layout_Engine SHALL 使 `PaperModel` 已有字段（`primitives`、`off_sheet_primitives`、`skipped_endpoint_pairs`、`missing_symbols`、`missing_templates`、`missing_boxes`、`template`、`symbols_found`、`symbols_referenced`）的值与实施前相同。
4. THE Existing_Gates SHALL 在不改动其钉住常量与断言的前提下全部通过。
5. THE Geometry_Projection SHALL 使 `tests/golden/geometry/*.json` 保持不变。

### Requirement 10: 确定性与 panic-free

**User Story:** 作为开发者，我希望同样的输入总得到同样的账本、坏输入也不会让记账崩溃，以便基线比对可信。

#### Acceptance Criteria

1. FOR ALL 页，用同一输入与同一语料连续产出两次 Export_Ledger，序列化结果 SHALL 逐字节相同。
2. THE Export_Ledger SHALL 按确定的键序（账域、层、流路径、类型码、原因码）排列全部计数。
3. IF 实体或图元含非有限坐标、零长度、零半径、空文本或越界字节范围, THEN THE Ledger_Builder SHALL 记出对应的 Drop_Reason 并继续处理本页其余内容。
4. THE Ledger_Builder SHALL 对不可信数据只使用返回 `Option` 或 `Result` 的访问与 checked 算术，不调用 `unwrap`、`expect`、`panic!`，也不使用可能越界的下标。

### Requirement 11: 性能

**User Story:** 作为开发者，我希望记账搭在已有的排版与写出上，以便 53 张的门禁不会因为记账再跑一遍。

#### Acceptance Criteria

1. THE Ledger_Builder SHALL 在产出 `PaperModel` 的同一次 Layout_Engine 调用内产出解析层与排版层账目。
2. THE Ledger_Builder SHALL 在写出 PDF 的同一次 Plot_Writer 调用内产出写出层账目。
3. WHEN Ledger_Gate 校验 Backup_Corpus, THE Ledger_Gate SHALL 只使用 `export_plant_backup` 本次运行已经产出的 `PaperModel` 与写出结果。
4. WHEN `export_plant_backup` 在同一台机器、同一语料上运行, THE Ledger_Change SHALL 使该测试的耗时比实施前增加不超过 10%（本机实施前约 107 s）。

### Requirement 12: 账本基线

**User Story:** 作为开发者，我希望 53 张备份图和 7 个夹具的账逐页入库，以便任何一页的账变了都能在评审里看到。

#### Acceptance Criteria

1. THE Ledger_Baseline SHALL 由 `docs/baselines/` 下两份文件名只含 ASCII 的 JSON 组成：`export-ledger-backup.json`（Backup_Corpus 53 页）与 `export-ledger-fixtures.json`（Fixture_Set 7 页）。
2. THE Ledger_Baseline SHALL 以页相对其语料根的路径为键，逐页记录页级事实、两个账域各层各原因的计数，以及需求 1.6 的交叉计数。
3. THE Ledger_Baseline SHALL 在文件头记下取基线时的 Corpus_Descriptor。
4. THE Ledger_Baseline SHALL 只含原因码、计数、流路径、类型码、oid、字节范围、模板文件名与页路径，不含图面文字内容与本机绝对路径。
5. WHERE 环境变量 `UPDATE_EXPORT_LEDGER=1`, THE Ledger_Gate SHALL 用本次账本重写对应的 Ledger_Baseline。
6. IF 重写会让某页某个 Suspect_Drop 计数高于原基线, THEN THE Ledger_Gate SHALL 拒绝写入并失败，列出这些计数。
7. THE Ledger_Gate SHALL 在 Repository_Corpus 下取 Fixture_Set 的基线，使 CI 与本机读到同一份账。
8. FOR ALL Export_Ledger，序列化为基线 JSON 再解析 SHALL 得到相等的 Export_Ledger（往返性质）。
9. THE Ledger_Baseline SHALL 在 `docs/baselines/README.md` 中有一节说明，写明两份基线的语料、比对规则与重写方法。

### Requirement 13: 账本门禁

**User Story:** 作为开发者，我希望门禁按基线逐页比对账本，以便可疑丢弃只能变少、有意丢弃不会悄悄变化。

#### Acceptance Criteria

1. WHEN Backup_Corpus 存在, THE Ledger_Gate SHALL 逐页比对本次账本与 `export-ledger-backup.json`。
2. WHEN Backup_Corpus 不存在, THE Ledger_Gate SHALL 软跳过 Backup_Corpus 的比对并打印原因。
3. THE Ledger_Gate SHALL 在 CI 上不借助字体对 Fixture_Set 比对 `export-ledger-fixtures.json`。
4. WHEN 某页某个 Suspect_Drop 计数高于基线, THE Ledger_Gate SHALL 失败，并列出页、账域、原因、增量，以及这些丢弃的来源（流路径、oid 或字节范围、类型码）。
5. WHEN 某页某个 Suspect_Drop 计数低于基线, THE Ledger_Gate SHALL 失败，并提示把基线降到新值（与 `missing_docs` 棘轮同一约定）。
6. WHEN 某页某个 Intentional_Drop 计数、On_Page 计数或需求 1.6 的交叉计数与基线不等, THE Ledger_Gate SHALL 失败并列出差异。
7. IF 本次 Corpus_Descriptor 与基线文件头不同, THEN THE Ledger_Gate SHALL 跳过计数比对、打印两份描述的差异，并照常检查 Unattributed_Drop 为 0。
8. IF 有页排版或写出失败, THEN THE Ledger_Gate SHALL 失败并列出该页与错误。

### Requirement 14: 暴露位置

**User Story:** 作为开发者，我希望从 API 和命令行都能拿到账本，以便排查时不必改测试。

#### Acceptance Criteria

1. THE Layout_Engine SHALL 在 `PaperModel` 上带出该页的解析层与排版层账目。
2. THE Plot_Writer SHALL 在 `write_plot_audited` 的返回值里带出写出层账目，与 `PlotAudit` 并列。
3. WHEN Plot_CLI 写完一页, THE Plot_CLI SHALL 打印一行账本汇总：实体总数、上页数、有意丢弃数、可疑丢弃数、未归因数。
4. THE Ledger_Builder SHALL 为每个公开类型、字段与函数提供 rustdoc 文档。

### Requirement 15: 结论措辞与仓库约定

**User Story:** 作为项目维护者，我希望账本的结论不越过证据、改动遵守仓库约定，以便它和已有的分析口径一致。

#### Acceptance Criteria

1. WHEN 一页的 Unattributed_Drop 为 0 且没有计数高于基线, THE Ledger_Gate SHALL 把该页报告为「未发现可归属缺陷」。
2. THE Ledger_Gate SHALL 只用「未发现可归属缺陷」或列出具体缺陷两种措辞报告一页。
3. THE Ledger_Change SHALL 通过 `AGENTS.md` 的五道提交前门禁：`cargo build --locked --workspace --all-targets`、`cargo test --locked --workspace --all-targets`、`cargo clippy --locked --workspace --all-targets -- -D warnings`、`cargo fmt --all -- --check`、`bash .github/scripts/check-missing-docs.sh`。
4. WHERE Ledger_Change 改动根 crate, THE Ledger_Change SHALL 同时通过 `cargo test --locked -p pid-parse --no-default-features --lib` 与 `cargo check --lib --locked -p pid-parse --no-default-features --target wasm32-unknown-unknown`。
5. WHERE Ledger_Change 新增探针, THE Ledger_Change SHALL 把探针命名为 `examples/probe_<问题>.rs` 或 `pid-plot/examples/probe_<问题>.rs`。
6. WHERE Ledger_Change 为首次基线读数写分析文档, THE Ledger_Change SHALL 把该文档放在 `docs/analysis/YYYY-MM-DD-*.md`，结论措辞遵守本需求第 1、2 条。

## 附录 A：原因目录

档位：可疑 = Suspect_Drop，有意 = Intentional_Drop，观测 = 只计数的不对称，不是丢弃。「今天」一列：已计数 = 代码已有计数或表；未计数 = 有分支、无计数；静默 = 既不计数也不告警。标「新发现」的是需求阶段补出的分支。

### A.1 解析层：记录没有变成实体

| 原因码 | 档位 | 判据（代码位置） | 今天 |
|---|---|---|---|
| `record_no_decoder` | 可疑 | 图形类类型码没有解码器：`dropped_graphic_records`（`build_normalized_geometry`） | 已计数并告警 |
| `record_refused` | 可疑 | 有解码器的类型码，记录形状被拒：`refused_graphic_records` | 已计数并告警 |
| `record_native_skip` | 有意 | 类型字带 `0x8000` 原生跳过位，是文件自己退役的旧副本：`skipped_graphic_records` | 已计数，不告警 |
| `sheet_stream_without_geometry` | 可疑 | `SheetStream::geometry` 为 `None`：流打不开，或没有任何族解码出记录且探针一无所获；这时该流的三类普查在 `sheet_geometry_from_probe`（`src/streams/cluster.rs`）里一并丢掉 | 静默（新发现） |
| `record_symbol_body_cache` | 有意 | 嵌套 `JSite*/PSMcluster0` 的符号体记录，经引用它的放置画出，本身不是页面内容 | 只有告警文字 |
| `cache_definition_unplaced` | 有意 | 缓存定义没有任何放置引用（例如参数化符号缓存的模板体），其记录不上页 | 未计数（新发现） |
| `record_audit_only_family` | 有意 | 已解码，所属族在 `EMITTERS` 中是无操作发射器：`igSmartFrame2d`、`DependencyObject`、`0x0010` 子记录、属性片段、`igRectangle2d`、`igDimension` | 未计数（新发现） |
| `record_emitter_out_of_range` | 可疑 | 已解码记录的字节范围超出所属流：每个发射器在 `source_range(..)` 为 `None` 时 `continue` | 静默（新发现） |
| `record_emitter_shape_rejected` | 可疑 | 发射器拒绝已解码记录：`igLineString2d` 的 x/y 顶点数不等，或全重合却没有顶点；`igBoundary2d` 环点少于 3；`igBspCurve2d` 采样少于 2 点 | 静默（新发现） |

### A.2 排版层：实体没有产出页面图元

按 Layout_Engine 的判定顺序排列。

| 原因码 | 档位 | 判据（代码位置） | 今天 |
|---|---|---|---|
| `layer_hidden` | 有意 | `suppressed_from_plot`：`source_layer.displayed == Some(false)`；主循环与 `push_entity_with` 两处调用 | 未计数 |
| `layer_consistency_checks` | 有意 | `suppressed_from_plot`：图层名为 `ConsistencyChecks` | 未计数 |
| `endpoint_pair_guess` | 有意 | 图纸域主循环：`record_kind == SheetRecordKind::EndpointPair`，推断线与端点探针都算 | 已计数（`skipped_endpoint_pairs`） |
| `kind_misrouted` | 可疑 | 按路由不可达的分支：`push_entity_with` 收到 `SymbolInstance`，或 `push_symbol_instance` 收到非 `SymbolInstance`；应为 0 | 未计数（新发现） |
| `line_on_sheet_edge` | 有意 | 图纸域 `Line`，页幅已声明，`sheet_edge`：一个方向的差 < 1 mm，另一个方向的差 > 0.9 倍页宽或页高 | 未计数 |
| `fill_duplicate` | 有意 | `igBoundary2d` 环（顶点按微米量化，加颜色）已在 `seen_sheet_fills` 中；这个集合由图纸域与家具域共用 | 未计数 |
| `point_no_marker` | 有意 | `Point`：解析不到线型、线型没有 marker，或 `marker.draws()` 为假 | 未计数 |
| `point_marker_degenerate` | 可疑 | `Point`：marker 的笔画全部 `is_degenerate` | 未计数 |
| `kind_annotation` | 可疑 | `PidGraphicKind::Annotation`；当前没有发射器产出，应为 0 | 未计数 |
| `kind_unknown` | 有意 | `PidGraphicKind::Unknown`：探针证据、`JStyleOverride` 等 | 未计数 |
| `symbol_missing` | 可疑 | 放置有符号路径，缓存体与符号库体都没有；写出端不画方框 | 已计数（`missing_symbols` / `missing_boxes`） |
| `symbol_unresolvable` | 可疑 | 放置既没有符号路径，也没有可解析的缓存体；今天任何地方都不记 | 静默（新发现） |
| `symbol_body_empty` | 可疑 | 解析到了符号体，但体内图元全部按 A.3 的原因丢掉，一个页面图元也没产出 | 未计数（新发现） |

### A.3 排版层：符号体图元

按 Body_Primitive 计，不是实体结局。

| 原因码 | 档位 | 判据（代码位置） | 今天 |
|---|---|---|---|
| `body_layer_hidden` | 有意 | 缓存体：`PidSymbolDefinition::visible_styled` 滤掉的图元（体内图层 `displayed == Some(false)`）；库体：`SymbolGeometry::displayed_primitives` 滤掉的图元（非法向视图的 sheet，或不显示的图层） | 未计数（新发现） |
| `body_null_placeholder` | 有意 | 体内文字整段是 `NULL` 占位：`symbol_text_is_null_placeholder` | 未计数 |
| `body_line_on_sheet_edge` | 有意 | 体内图元放置后是贴页边的 `Line`：`push_symbol_primitive` 的 `sheet_edge`；家具域的放置同样适用 | 未计数（新发现） |
| `body_boundary_circle_degenerate` | 可疑 | `Boundary` 的圆成员放置后半径 ≤ 1e-4 mm，不出填充 | 未计数（新发现） |
| `body_boundary_member_unsupported` | 可疑 | `Boundary` 成员既不是 `Circle` 也不是 `Line`；读取端声明不会出现，应为 0 | 未计数（新发现） |

### A.4 排版层：页面图元与文字规整

| 原因码 | 档位 | 判据（代码位置） | 今天 |
|---|---|---|---|
| `off_sheet` | 可疑 | 页幅已声明时，排版末尾 `retain` 用 `beyond_the_sheet` 移除的图元：非文字形状含非有限点、跨度超过 `SHEET_REACH` 条页对角线，或整条在页外；文字只在原点非有限时移除 | 已计数（`off_sheet_primitives`） |
| `text_control_char` | 有意 | `label_line_ranges` 去掉的控制字符，按字符计；制表符改成空格，不算 | 未计数（新发现） |
| `text_trailing_empty_line` | 有意 | `label_line_ranges` 丢掉的末尾空行，按行计 | 未计数（新发现） |

### A.5 写出层：图元与段

表中先后即需求 1.5 与 5.3 的固定优先级。

| 原因码 | 档位 | 判据（代码位置） | 今天 |
|---|---|---|---|
| `probe_only` | 有意 | `push_layer` 只画 `decoded` 与 `inferred` 两层（`is_plotted`） | 未计数 |
| `fill_degenerate` | 可疑 | `paint_primitive`：`Fill` 环点少于 3 或含非有限坐标 | 未计数 |
| `pen_width_invalid` | 可疑 | `paint_primitive`：笔宽非有限或 ≤ 0；没有线型时用的默认笔不在此列 | 未计数 |
| `chain_empty` | 可疑 | `shape_chain` 不足 2 点：折线少于 2 点，或弧、圆的 `arc_points` 为空（半径 ≤ 0、非有限、扫角绝对值 < 1e-8）；笔状态仍写进 PDF | 未计数（新发现） |
| `segment_non_finite` | 可疑 | `stroke_runs`：段端点非有限；按段计 | 未计数 |
| `segment_too_short` | 有意 | `stroke_runs`：段长 < 1e-4 mm；按段计 | 未计数 |
| `segment_on_sheet_edge` | 有意 | `stroke_runs`：非家具段被 `read::sheet_edge_mm` 判为贴页边（一个方向的差 < 0.282 mm，另一个方向的差 > 0.9 倍页边；对测得的页幅同样生效）；按段计 | 未计数 |
| `text_empty` | 有意 | `show_text`：文本为空串 | 未计数 |
| `text_placement_non_finite` | 可疑 | `show_text`：旋转角或原点非有限 | 未计数 |
| `text_height_invalid` | 可疑 | `show_text`：高度非有限或 ≤ 0，包括排版层在记录与样式都没给高度时填的 `Some(0.0)`；`None` 回落到 `DEFAULT_TEXT_MM`，不算 | 未计数 |
| `text_whitespace_only` | 有意 | `show_text`：去掉首尾空白后为空 | 未计数 |

### A.6 部分上页

已上页、但不完整；不是丢弃。

| 标记 | 判据（代码位置） | 今天 |
|---|---|---|
| `text_condensed` | `show_text` 把文字沿自身方向压缩后整段留在页内 | 已计数（`PlotAudit::runs_condensed`） |
| `text_clipped_at_floor` | 压到 `MIN_CONDENSE` 仍越过页边，尾部被页面裁掉 | 已计数（`PlotAudit::runs_clipped_at_the_condensation_floor`） |
| `past_the_edge` | 墨越出页边超过 0.5 mm，被页面裁切（`PaperModel::primitives_past_the_edge`） | 门禁读数，不是字段 |
| `segments_dropped` | 笔画上页，但有段按 A.5 的段原因丢掉 | 未计数 |

### A.7 不对称观测

家具域照画、图纸域会丢的情况；只计数，不改画法。

| 标记 | 判据（代码位置） | 今天 |
|---|---|---|
| `furniture_symbol_on_suppressed_layer` | 家具循环的 `SymbolInstance` 不经 `suppressed_from_plot`，位于隐藏层或 `ConsistencyChecks` 层也照画 | 未计数 |
| `furniture_endpoint_pair_drawn` | 家具循环不查 `EndpointPair`，这类实体照常进入排版 | 未计数（新发现） |

### A.8 页级事实

| 原因码或标记 | 档位 | 判据（代码位置） | 今天 |
|---|---|---|---|
| `template_candidate_unreadable` | 可疑 | `load_border_template` 中模板候选读不出（`read_archive_file`）或解析失败（`parse_file` / `parse_reader`）后 `continue` | 静默（新发现） |
| `template_missing` | 可疑 | 页面点名的模板没有文件可答，页面不穿家具 | 已计数（`missing_templates`） |
| `page_size_source` | — | 页幅来自声明（`page_mm`）还是测得（`resolved_page_mm`） | 门禁读数 |

### A.9 不入账的内容

下列内容改变画法或使整页失败，但不丢弃记录，列出以示穷举：

- 画法替换：`dash_array` 为 `None` 时画实线；`pen_width_mm` 把声明笔宽吸附到四支笔；`plot_ink` 把非白色画成黑色；`body_stroke` 对 `As Drawn` 与 `BandAid` 改用体自身笔宽；制表符改成空格；`sheet_text_spans` 与 `line_spans` 在含非 BMP 字符或去掉过控制字符时放弃分段；非等比缩放的圆与弧改成折线；`canonical_rect_chain` 只改矩形环的起点；`stamp_complex_dash` 少盖或不盖复杂线型记号（管线本身照画）。
- 整页失败：`PaperModelError`，以及 `PlotWriteError` 的 `NoPage`、`Font`、`MissingGlyph`、`Save`。现有门禁对这些情况直接报错，账本门禁同样报错（需求 13.8），不作为丢弃入账。
- 比对路径：`write::stroked_segments` 供出图对比使用，不在导出路径上，不入账。

## 附录 B：已确认的取向

需求正文按下列取向写成，用户于 2026-11-04 确认全部采用：

1. 账本粒度：实体结局在写出之后才定（端到端），另记写出端的图元级与段级计数（需求 1、5、6）。
2. 解析层记录：无解码器、被拒、原生跳过三类，连同已解码未发射、符号体缓存、无几何的 Sheet 流，都记进同一本账（需求 3）。
3. 两档：分档。可疑档钉住现值、只许下降：升即失败，降也失败并提示同步降基线；有意档精确钉住（需求 2.2、12.6、13.4–13.6）。
4. 家具账：与图纸账分开记，并单列两处不对称（需求 7）。
5. 基线：`docs/baselines/` 下两份 JSON，`UPDATE_EXPORT_LEDGER=1` 重写；夹具基线在仓库语料下取，CI 可比；53 张基线在本机的备份包语料下取（`PID_PARSE_SELF_CHECK_DIR` 指向含 `RefData~*.zip` 的包目录，字体由 `PID_PLOT_FONTS` 给出），CI 上软跳过（需求 12、13）。
6. 暴露：`PaperModel` 字段、`write_plot_audited` 的返回值、`pid_plot` 打印一行汇总，三处都要（需求 14）。
