# publish 两条线还在等的那些数据（2026-11-15）

> 口径：**先写红测与缺口文档，等数据到位再改规则**。这一页只记录，不改 `src/`。
> 关联：GitHub #71（现场数据）、#62（`Export.mdf`）、#56 / #57 / #53（墨迹门禁的已知缺陷）。
> 上游：`docs/plans/2026-11-10-publish-export-completion.md` §3 的 S10 / S12 / S15 / S16 / S17 与 Q6。
> 红测：`tests/publish_missing_data.rs`（5 条，全部只读入库夹具，今天全绿；规则一改就红）。
> 行号：2026-11-17 这条线与审核整改线合并之后，这一页引的 `文件:行号` 按合并出来的树重对过一遍；S12 的那次筛选整改线已经收进 `attach_dump` 一处，那一段照现在的样子改写了。

## 一句话

五项缺的都是**第二个样本**：S10 要 DWG 的同版发布件，S12 要第二例 stockpile OPC，S15 要一份带正文与 OPC 的 MDF，S16 要一条走回退拼名的管段，S17 要 C井场 的原厂发布件。今天的代码在每一项上都有一个确定的、可断言的行为，这五项各自被一条测试钉住；数据到位之后改规则，那五条先红，再跟着改断言。

## 1. #71 的五项

每一项四件事：**缺什么数据 / 现在代码怎么写 / 数据到位后哪条断言要改成什么 / 关联 issue**。

### S10 · 管线 UID 取 `T_PIPELINE.SP_ID`

- **缺什么数据**：`DWG-0202GP06-01` 用现在的库重新发布一次，或参考发布件当时的备份包。仓库里 `test-file/backup-test/DWG-0202GP06-01_p` 比参考新（管段序号、位号写法都改过），所以「库里有 `T_PIPELINE` 行时 UID 取 `SP_ID`」这条规则没有同版的参考可对。
- **现在代码怎么写**：`pipeline_uid(plant, name)`（`src/publish/pipeline.rs:205`）是 UUID v5 —— 命名空间 `pid-parse/pipeline` 混 plant（`:206-207`），再混管线名（`:208`），输出 32 位大写十六进制。`attach_pipelines`（同文件 `:43`）只从 `T_PIPERUN` 拼名字，`grouped.insert` 时调 `pipeline_uid(plant, &name)`（`:99`）当 UID。dump 里的 `T_PIPELINE` 一行都不读：`ExpTables::read`（`src/publish/oracle_exp.rs:353`）读的 19 张表与一份数据字典里没有它（`:357-376`），入库包里那张表只以一条 `INSERT INTO "T_PIPELINE" (SP_ID, OPERFLUIDCODE, TAGSEQUENCENO, TAGSUFFIX, FLUIDSYSTEM)` 出现，没有任何一行被投影。
- **数据到位后哪条断言要改成什么**：`tests/publish_missing_data.rs::s10_a_pipeline_uid_is_composed_from_the_plant_and_the_name_not_from_the_dump` 今天断言三件事 —— 写出的六个 UID 都等于 `pipeline_uid("P01", <本次运行写出的某个名字>)`（`:139-151`）、与参考的五个 UID 交集为空（`:154-161`）、`ours.len() == 6 && theirs.len() == 5`（`:135-136`）。同版参考到位后改成：写出的每条管线 UID 等于参考里那条管线的 UID（参考的五个是 `2D010BCE…` / `1AC99742…` / `B3D0A6FD…` / `7779C14F…` / `81C620DD…`，见 `test-file/export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01_Data.xml:1370,1382,1394,1406,1418`），并且名字取 `T_PIPELINE` 的 `ITEMTAG` 而不是拼名。今天那条「交集为空」的断言届时会直接失败。
- **关联 issue**：#71 第 1 项；计划 S10（`docs/plans/2026-11-10-publish-export-completion.md` §3）；Q4。

### S12 · 本图 stockpile 里的 OPC

- **缺什么数据**：一张本图 stockpile 里放着 OPC 的图的原厂发布件，连同发布那一刻的备份。现在只有 DWG 一例，要第二例才能定四件事：① 是不是每个这样的 OPC 都发；② UID 取「配对里较小的」还是「不在本图的那一端」；③ 带哪些接口与属性（有 `ToFromText` 时写不写 `IOffpageConnector`）；④ 两端都在 stockpile 里时发几个。
- **现在代码怎么写**：`ExpTables::opc_points(drawing_uid)`（`src/publish/oracle_exp.rs:2276`）遍历 `T_REPRESENTATION`，取本图的行（`:2316-2319`），UID 用 `lesser_id(&model, paired)`（`:2347`，定义在同文件）。它本身**不**按「表示在 `.pid` 里」筛；筛在 `attach_dump`（同文件 `:1545`）里：`opc_points.retain(|point| representations.contains(...))`（`:1562-1569`，`retain` 在 `:1568`）。两条线都调 `attach_dump`：`.pid` 线在 `src/bin/pid_publish_xml.rs:848`，整包线在 `src/publish/backup_package.rs:881`。入库包上这个投影看到 5 个 OPC，`.pid` 画了其中 4 个的表示，第 5 个（`799B9008DB6848F28127C78AC745C5A8`，名 `278`，表示 `3A36F254…`，`INSTOCKPILE = 2`、没有 `GRAPHICOID`）被丢掉。参考把它发成孤立的 `PIDBranchPoint`（`…_Data.xml:1488-1490`）。
- **数据到位后哪条断言要改成什么**：`s12_a_stockpiled_opc_is_read_from_the_dump_and_then_dropped_for_having_no_representation` 今天断言 `points.len() == 5`（`:183`）、`kept.len() == 4`、`dropped == ["799B9008…"]`（`:201-206`）、参考里有它而我方文档里没有它（`:209-234`）。第二例到位后改成：这类 OPC 照发一个对象，UID 按定下来的那一端取，`IObject.Name` 取 `OPCTAG`，类接口按 `catalog::class_interfaces("PIDBranchPoint")` 写，不发表示、不发关系；`dropped` 那条断言要删掉，改成断言它出现在写出的文档里。
- **关联 issue**：#71 第 3 项；计划 S12；Q6 ④。

### S17 · 库里画在图上、`.pid` 没画的非管件对象

- **缺什么数据**：C井场 的原厂发布件（与 53 张那个备份同一时刻），以及 Q井场 的 2 个仪表如果有的话。53 张整包对库普查量出 23 条：C井场 21（仪表 5、注释 7、管嘴 5、机械设备 3、容器 1），Q井场 2（仪表）。见 `docs/analysis/2026-11-12-what-the-plant-database-draws-that-a-publish-leaves-out.md`。
- **现在代码怎么写**：`ExpTables::dump_only_piping_components(drawing_uid, drawn)`（`src/publish/oracle_exp.rs:678`）只把 `T_MODELITEM.ITEMTYPENAME == "PipingComp"` 的行收进 `components`（`:689-694`，过滤在 `:692`），别的类型连集合都不进；`attach_dump_only_components`（同文件 `:1398`）只把这个返回值挂到 `drawing.items` / `drawing.representations` 上（`:1412-1420`）。依据只有 J K井场 的 3 例。
- **数据到位后哪条断言要改成什么**：`s17_only_a_piping_component_the_database_draws_is_added_and_no_other_class_is_looked_at` 今天断言这个投影在本图上一共看到 2 个表示、类型集合恰好是 `{"PipingComp"}`、每个的类都是 `PIDPipingComponent`（`:374-389`），并且 `.pid` 画了这两个表示时一个也不加（`:393-407`）。C井场 的发布件到位后改成：仪表 / 注释 / 管嘴 / 机械设备 / 容器也各有一条「本图有表示、`.pid` 没画就照库发」的规则，写出的对象对上参考；今天那两条「类型集合只有 `PipingComp`」「一个也不加」的断言会失败。
- **关联 issue**：#71 第 2 项；计划 S17；Q6 ②。

### S15 · 数据库线的注释正文与 OPC 接口

- **缺什么数据**：一份带注释正文和 OPC 的 SQL Server（MDF / MTF）备份。仓库里没有这样的夹具：A01 的 12 个对象里这两类都没有。
- **现在代码怎么写**：
  - 注释：`xml_writer::write_item_note`（`src/publish/xml_writer.rs:1509`）取 `obj.fields["NoteText"]`，没有就取 `obj.description`（`:1510-1515`），两者都空时写裸 `<INote/>`（`:1526-1527`）。库里正文在 `T_NOTE.BODY`，而 `mdf_load` 暂存的 24 张表里有 `T_ItemNote`、没有 `T_NOTE`（`src/publish/mdf_load.rs:53` 是 `T_ItemNote`，整张表清单见 `:28-59`）。`.pid` 线读的恰是 `Note.Body`（`src/publish/pid_load.rs:739` 把 `Note.Body` 映射成 `("INote", "NoteText")`）。
  - OPC：`catalog::run_branch_point`（`src/publish/catalog.rs:196`）走 `xml_writer::write_pid_branch_point`（`src/publish/xml_writer.rs:1969`），它写 `IPIDBranchPoint` / `IDuctConnection` / `IConnection` / … 但**从不**写 `IOffpageConnector`；`T_OPC` 也不在暂存的表里。`.pid` 线的 `write_opc_point`（`src/publish/pid_writer.rs:286`）有 `ToFromText` 才写这个元素（`:294-302`）。
- **数据到位后哪条断言要改成什么**：`s15_a_database_note_has_no_body_and_a_database_branch_point_has_no_offpage_connector` 今天把一份 DTO 直接喂给 `write_data_xml`（`PublishDrawing` 里一个 `Note` 的正文放在 `Body` 字段、一个 `BranchPoint` 带 `ToFromText`），断言文档里是裸 `<INote/>`、没有 `NoteText`、没有 `IOffpageConnector`、没有 `DWG-1`。夹具到位后改成：`<INote NoteText="…"/>` 带正文、`<IOffpageConnector ToFromText="…"/>` 写出来；今天那四条断言全部会失败。
- **关联 issue**：#71 第 5 项；计划 S15。

### S16 · 管线名回退拼名时的流体代号

- **缺什么数据**：一条走管线名回退拼名、流体代号又只填了 `CODELIST_TEXT` 的管段，以及它的发布件。（可选，#71 第 6 项。）
- **现在代码怎么写**：`attach_pipelines`（`src/publish/pipeline.rs:43`）在 `:51` 调 `codelist_short_text(19)`；`ExpTables::codelist_short_text`（`src/publish/oracle_exp.rs:791`）只读 `CODELIST_SHORT_TEXT` 一列（`:792`），完全不看 `CODELIST_TEXT`。19 号表出厂自带的代号只填 `CODELIST_TEXT`，于是 `fluid_slot`（`src/publish/pipeline.rs:146`）把 `None` 交给 `pipeline_name`（`:129`），名字里少一格，并计入 `fluid_code_missing`（`:71-73`）。管段在 `T_PLANTITEM` 里有行时名字取 `ITEMTAG`（`run_pipeline_name`，`:116`），输出不受影响 —— 所以只有回退拼名才看得见。
- **数据到位后哪条断言要改成什么**：`s16_a_fluid_code_that_only_states_codelist_text_is_left_out_of_a_spelled_pipeline_name` 今天断言 19 号表的短文本只有 55 条、代号 493（`OD`）与 20（`AI`）都不在里面（`:313-315`），`pipeline_name(&run, None) == "50 mm-0401-1.6AR12-WE50mm"`（`:331-335`），本包 dump 上 `fluid_code_missing == 5`（`:349-352`）。样本到位后改成：代号只填 `CODELIST_TEXT` 时回退读那一列，名字里带代号（`50 mm-OD-0401-1.6AR12-WE50mm`），`fluid_code_missing` 降到参考所见的那个数；今天那三条断言会失败。
- **关联 issue**：#71 第 6 项（可选）；计划 S16。

## 2. #62 的 `Export.mdf`

**13 条 DWG 镜像门禁原本等 `test-file/backup-test/DWG-0202GP06-01_p/extracted/Export.mdf`，那个文件从未存在过**（包的 `Export.dmp` 是 Oracle `exp` dump，MDF 线读不了；这台机器上也没有 `P01` 的 SQL Server 副本）。

**B5 已经把这 13 条改成对入库的 Oracle 包真跑**：`tests/publish_dwg_mirror.rs`（4 条）、`tests/publish_dwg_generated_parity.rs`（4 条）、`tests/publish_meta_parity.rs` 的 DWG 那两条、`tests/publish_rel_parity.rs` 的 DWG 那三条，全部走 `common::generate_dwg_data_xml` / `generate_dwg_meta_xml`，也就是 CLI 的 `--oracle-exp` 线，拿包自己的 `Export.dmp` 与 `RefData~4~804.zip` 里的 `SPPIDDataMap.xml`。这些文件里已经没有 `extracted/Export.mdf` 的字样。

**所以 `Export.mdf` 只在这条线上还缺**：数据库线（`pid_publish_xml <mdf> --drawing UID`）的 DWG 侧仍然只有 A01 一个夹具（`test-file/backup-test/TEST02_p/extracted/Export.mdf`）。要有 DWG 的 `Export.mdf` 才能把 `sqlite_load` / `xml_writer` 的 DWG 类映射（`item_type_name` 对 `PIDBranchPoint` / `PIDPipingBranchPoint` 至今是 provisional，见 `src/publish/mod.rs:120-125`）在真实数据上确认。S13（DWG 进 `publish-diff-accepted.json`）与 S14 也挂在这上面。

- **缺什么数据**：`DWG-0202GP06-01` 备份里解出来的 `Export.mdf`（或一份 `P01` 的 SQL Server 备份）。
- **现在代码怎么写**：数据库线走 `mdf_load` → `sqlite_load` → `xml_writer`；DWG 的两个分支点类没有夹具确认，`catalog` 里那两行 `provisional: true`（`src/publish/catalog.rs:372`、`:384`）。
- **数据到位后哪条断言要改成什么**：`tests/publish_dwg_mirror.rs` 等 13 条要从「Oracle 线上真跑」再各加一条「数据库线上真跑」，并且 `src/publish/catalog.rs` 那两行的 `provisional` 翻成 `false`；`tests/publish_mdf_load.rs` 的 `A01_MDF_PATH`（`:10`）之外再多一个 DWG 路径。今天没有这条断言，所以没有一条会红 —— 这是唯一一项「数据到位后不会自动变红」的缺口，补的时候要先写红测。
- **关联 issue**：#62；计划 S14 / S13；Q4。

## 3. #56 / #57 / #53 与墨迹缺陷表的行数

`pid-plot/tests/reference_plots_ink.rs` 的 `SHEETS`（`:443-698`）里，每张 `Sheet` 的 `defects` 是一个 `&[Defect]`。**实际一共 22 行**，落在三张图上：

| 图纸 | 行数 | 字段（`Field::*`） | `cause` |
|---|---:|---|---|
| 一级闪蒸分离器A | 6 | `WholeExtra`、`WholeLabelsAlone`、`PenUnpaired`、`SheetEdgesApart`、`WipesSame`、`WipesOther` | 前四个 `Undetermined` / `BARE_BORDER`，后两个 `Undetermined` / `OTHER_ORDER` |
| 低温分离器A | 4 | `WholeExtra`、`WholeLabelsAlone`、`PenUnpaired`、`SheetEdgesApart` | `Undetermined` / `BARE_BORDER` |
| 轻烃分离器I | 12 | `MaskedCoverage`、`MaskedUncovered`、`MaskedExtra`、`MaskedLabelsPaired`、`MaskedLabelsAlone`、`WholeCoverage`、`WholeUncovered`、`WholeExtra`、`WholeLabelsPaired`、`WholeLabelsAlone`、`PenUnpaired`、`WipesSame` | `SiteData`（`LATER_REVISION` 与两条各写各的 `what`） |
| **合计** | **22** | | 0 条 `NotFixed`、12 条 `SiteData`、10 条 `Undetermined` |

（`Cause::NotFixed` 自 2026-11-11 A2 页变成自己的纸张之后一条都没有，`reference_plots_ink.rs:302-306` 的注释说的就是这件事。）

### 三个 issue 与代码对齐

| issue | issue 说的条数 | 代码里的条数 | 差在哪 |
|---|---:|---:|---|
| #57 闪蒸A、低温A 的图框与绘制次序 | 8 | 10（那两张图合计） | issue 的 8 = 那两张图上**图框**那 8 行（`BARE_BORDER`：闪蒸A 4 + 低温A 4）。多出来的 2 行是闪蒸A 的 `WipesSame` / `WipesOther`（`OTHER_ORDER`），issue 把它们归到 #53 去了，所以两张图合计 10 = 8 + 2，**没有重叠**。 |
| #53 闪蒸A 上标签 `P` 的原点标记与参考同次序 | 2 | 2（闪蒸A 的 `WipesSame` + `WipesOther`） | 一致。这两行就是 `OTHER_ORDER`（`reference_plots_ink.rs:422`）说的 `P70` 原点标记，而 `P70` 是参考图上的写法、我方记录里的同一个标签（`a_label_and_its_origin_mark_are_painted_in_the_order_the_sheet_lists_them` 的 `AGAINST_ITS_LIST` 一行）。 |
| #56 A2（轻烃分离器I）与参考图同版 | 12 | 12（轻烃分离器I 的全部行） | 一致，`cause` 全是 `SiteData`。 |

**结论**：22 = 8（#57）+ 2（#53）+ 12（#56），三个 issue 的条数加起来正好是表里的行数，**没有一行被两个 issue 重复计算，也没有一行没被 issue 提到**。#57 的「8」是它自己那一半（图框），把闪蒸A 上那 2 行次序读数算成 #53 的，就对上了。

`every_known_defect_is_one_reading_with_a_cause`（`reference_plots_ink.rs:1123`）会在 `--nocapture` 下打印这三个数，跑一次就能核：
`known defects: 0 not fixed yet, 12 needing data from the site, 10 not settled`。

## 4. 这一页没有做的事

- 没有改 `src/` 下的任何文件，也没有改 `reference_plots_ink.rs` 的缺陷表内容（那会改门禁行为）。
- 没有改三个 issue 的正文。
- 五条红测都只用入库夹具（`test-file/**`），没有一条读 `D:\work\…`，也没有一条软跳过；`tests/soft_skip_spelling.rs` 过。
