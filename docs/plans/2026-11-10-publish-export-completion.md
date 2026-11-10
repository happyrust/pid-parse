# publish 导出收尾计划（2026-11-10）

> 依据：`docs/analysis/2026-11-10-publish-clean-is-nothing-wrong-not-everything-stated.md`（现状、对照矩阵、完整度普查与归因都在那里，本文不重复数字的出处）。
> 承接：`docs/plans/2026-10-13-publish-progress-and-next-steps.md`（N9–N12）与 `docs/analysis/2026-11-05-seven-reference-publish-diff.md`（七张参考图）。
> 基线 `main` = `803984a`；工作在 worktree `D:\work\plant-code\cad\pid-parse-wt\publish-export`、分支 `publish-export-20261110`，不 push、不动 `main`。
> 状态：§5 的四片已提交；§2 的 Q1–Q6 等批复；§3 其余切片未开工。

## 一句话

publish 现在的状态是「7 张图写出的都对，但只写出参考的一半」。要到「正确导出」，剩下三件事：把门禁没看的地方补上（接口元素、`<Container>`、数据库线），把类的固定接口和库里现成的属性写出来（接口从 57 % 到全、非空值从 50 % 到九成以上），以及给 DWG 与 4 张旧图找到同版的数据。第一件和第二件手头的数据够做，第三件要现场给数据。

## 1. 建议的验收口径

现在的门禁回答「写出的有没有错、没写的有没有登记理由」。建议在它之上加一条**完整度**口径，否则「把一类属性挪进豁免清单」和「把它写出来」在门禁上是同一个绿：

- 对每张有同版库的参考图，`--oracle-exp` 与整包两种模式下：对象 100 %、关系 100 %（已达到）、**接口元素 100 %**、**非空值 ≥ 豁免清单收缩后的全部**。豁免清单里只允许留库与 `.pid` 都没有的东西（发布时的合成值、`_Meta.xml` 那三类），每一项带出处。
- `.pid` 单发：对象与关系维持现状（文件里没有端口、管线、连通），接口元素写全，值只写文件里有的。
- 数字由入库的探针出（S0），钉进测试，只许升。

## 2. 要你拍板的

| # | 问题 | 选项 | 推荐 |
|---|---|---|---|
| Q1 | 库里的属性从哪条线补 | (a) 在 `.pid` + exp 线上补：类的固定接口表 + 按 `SPPIDDataMap.xml` 把 dump 的列发成属性；(b) 把 Oracle `exp` 的行送进数据库线（MDF 那条线的 `sqlite_load` + `xml_writer`，`backup-store-v2` 分支计划的方向）；(c) 两条都做 | **(a)**。`.pid` + exp 线的对象与关系在 7 张图、21 个类上已经 100 % 对上，这是最难的一半；数据库线只在 A01 一张图、8 个类上验证过，比它少 6 个类，写手是逐类手写的。数据映射是原厂自己的文件，对 87.5 % 的参考值给得出「哪一列发成哪个属性」，比照参考手抄更站得住。(b) 的好处是 14 条 DWG 软跳过能真跑、MTF 与 Oracle 同一条路，代价是要一个完整的 Oracle 行解码器（LOB、所有类型），那边分支上有未提交的一半。不选 (a) 的话 S5–S9 换成「Oracle 行进 SQLite」一大片 |
| Q2 | 主检出 `stash@{0}`（18 个文件的 publish 未合入改动）怎么办 | (a) 原样并到本分支，单独成提交，再在其上继续；(b) 只取门禁那几项（接口、`<Container>`、重复 UID、包模式版本检查）；(c) 不要 | **(a)**。在本 worktree 临时套上验过：348 条库测与 14 个 `publish_*` 全过，对照矩阵不变。它让门禁更严，所以**会往基线里加条目**（A01 27 → 28，I井场 别名 1 → 2）并改两处测试计数——这是口径变严带来的登记，不是放宽，但要你点头。它与本轮 S1 改了 `oracle_exp.rs` 的同一处（`ExpTables::read` 五张表改宽松读）、与 S3b 改了 `pid_writer.rs` 的同一处（空的 `IOffpageConnector`），并入时这些行两边写法相同，`oracle_exp.rs` 的端口投影（它改成跳过非端口行，本轮保持报错）和测试辅助函数要手工对一下 |
| Q3 | `.pid` 单发要不要也写类的固定接口（`<IDrawingItem/>` 这类没有属性的元素） | (a) 两种模式都写；(b) 只在有 dump 时写，`.pid` 单发保持「只写文件里说的」 | **(a)**。13 份参考里 12 个类的接口序列只有一种写法，这是类的定义，不是从库里读的值；不写的话下游按接口认对象时，`.pid` 单发的对象什么都不是。空值属性（`InstrTagSuffix=""`）建议一并照参考写 |
| Q4 | DWG-0202GP06-01 的备份包比参考新（管段的序号、位号写法都改过） | (a) 承认它是版本差：`.pid` 单发收进基线，`--oracle-exp` 的管线类差异钉在测试里并写明原因；同时请现场对**现在的库**重新发布一次这张图，或给参考当时的备份；(b) 只管 `.pid` 单发 | **(a)**。这是唯一一个三件都在仓库里、CI 上真跑的 Oracle 图。有了同版参考，「`T_PIPELINE` 有行时管线用它的 UID」（S10）才能验证 |
| Q5 | 数据库线（`Export.mdf --drawing`）的命令行门禁只比个数 | (a) 换成与 `.pid` 线同一个 `diff_pid_export`，A01 的三类合成残差（连接件 UID、`Rel` UID、`GraphicOID`）逐条登记进基线；(b) 维持 | **(a)**。测试层已经逐字节比过，命令行给用户的却是最弱的一档；MTF 包走包门禁时报的 27 条现在既没登记也没人看 |
| Q6 | 还缺的参考数据 | 请现场提供：① 4 张旧集注站图那一版的备份包；② 53 张里没有参考的图挑几张发布（建议 C井场——27 个只在库里的管件是外推的——和任意一张带设备 `Description` 标签的图）；③ Q4 的 DWG 重新发布 | 没有这些，对应的结论只能停在「外推」或「版本差」 |

## 3. 切片

按影响从大到小。「依赖」列空着的现在就能做。大小：小 = 半天内，中 = 一两天，大 = 多天。

| 片 | 改什么 | 怎么验证 | 依赖 | 大小 |
|---|---|---|---|---|
| S0 完整度探针入库 | `examples/probe_publish_reference_completeness.rs`：一对文件出「对象 / 接口元素 / 非空值 / 空值属性 / 关系」的写出比例与缺得最多的 `(类, 接口, 属性)`；`tests/publish_from_pid.rs` 的 DWG 三条加上比例下限 | 与 `target/publish-audit/census.py` 的 22 行数字相同 | | 小 |
| S1 ✔ exp 宽松读 | §5 | | | |
| S2 ✔ 设备的 `Description` 标签 | §5 | | | |
| S3 ✔ 注释正文的换行 | §5 | | | |
| S3b ✔ 无 `ToFromText` 的 OPC 不写 `IOffpageConnector` | §5 | | | |
| S4 门禁看接口元素、`<Container>`、重复 UID | Q2 (a) 时直接来自 stash；否则重写：`PublishBlocks` 记每个对象带的接口与根属性，写出侧多三种发现项 | 三条删改回归（多写一个接口 / 改 `Plant` / 重复一个块）各退出 1；13 张图矩阵不变 | Q2 | 小（并入）/ 中（重写） |
| S5 类的固定接口表 | `catalog.rs` 每类一行有序接口表（13 份参考量出，可选接口单列条件：有值才写）；`pid_writer` 按表写；门禁加参考侧接口比对，表外的接口记发现项 | 7 张图 exp 模式接口元素 56–58 % → 100 %；DWG 与 4 张旧图 `.pid` 单发同样到 100 %；写出的接口无一不在参考里 | S4、Q3 | 中 |
| S6 数据映射读全 | `src/backup/datamap.rs` 现在只读 `MapEnumToEnum`；补 `MapClassToClass`、`MapClassMapProperties`、`MapPropertyToProperty`、`MapPropertyMapEnumList`、`MapEnumMapEnum`（层级）与 `ProcessPropertyCriteria` | 单测 + 对三个包里那份映射钉计数（1 818 个属性定义、12 087 条关系）；`SPMP_Vessel_Description → LongMaterialDescription` 等 6 条抽查 | Q1 (a) | 中 |
| S7 库属性第一批：原样的值 | 从 dump 取、不经枚举与单位：`IPIDTypical.IsTypical`、`IConnector.RepresentationsAreAllZeroLength`、`IInlineComponent.IsFlowDirectional`、`IPipingSpecifiedItem.PipingMaterialsClass`、`IInsulatedItem.InsulSpec`、`INamedPipingConnector.PipingConnectorSeqNo`、`IPipeCrossSectionItem.NominalDiameter`（补标签没给的）、`IObject.Name`（`ITEMTAG`）、`IObject.Description`（设备取 `NAME`）。每个属性一个提交，同时把它从 `EXCLUDED_ATTRIBUTE_CLASSES` 里拿掉——拿掉之后 7 张图逐值对参考 | 每片：该属性在 7 张图 exp 模式「缺 / 参考」归零、值错 0；`.pid` 单发不变。7 张图合计缺 19 437 个非空值，这一批占 7 667 个 | S6 | 大（9 个小片） |
| S8 库属性第二批：枚举 | `IPBSItem.ConstructionStatus(2)`、`IPipingConnector.PipingConnectorType`、`IConnector.FlowDirection`、`IPipingComponent.*`、`IInstrument.*`、`IInstrumentOcc.InstrumentLocation`、`IEquipment.EqType0–5` 与 `EqTypeDescription`、`IEquipmentComponent.*`、`IDocument.DocCategory / DocType`。要先定层级枚举的规则（`ConstructionStatus` 写 `@NewConstruction`、`ConstructionStatus2` 写 `@{GUID}`；`EqType0…5` 是一条链） | 规则先用探针对 7 张图量：每个 `(类, 属性)` 预测值与参考逐值相同才进代码。这一批占 11 610 个；再剩 160 个零散的（仪表位号 102、`LongMaterialDescription3` 34、`EquipPrefix` 17、`ReliefPressure` 6、`EquipmentTrimSpec` 1）随 S7 / S9 走 | S6；规则若量不出要 `EFAdapter.dll` 的反汇编 | 大 |
| S9 带单位的值 | `TotalInsulThick`（`50 mm`）、`InsulThickSrc`、`SlopedPipingAngle`（`2.9999910000486E-03 rad`）、`HeightRelativeToGrade`（`3 m`）、`ProcessVessel_VesselVolumetricCapacity`（`27 m^3`）、`CatalogPartNumber` | 样本只有 DWG 一张图加 A01；规则写成「SI 列 + 单位名」，对这两张逐值相同 | S6；样本少，建议等 Q6 ② 的图 | 中 |
| S10 管线用 `T_PIPELINE` 的 UID | 库里有 `T_PIPELINE` 行时，管线的 UID 取 `SP_ID`、名字取它的 `ITEMTAG`，不再合成；管段归哪条管线的规则待定（`SP_PARTOFID` 在 DWG 包里全空） | 需要同版参考 | Q4 的数据 | 中 |
| S11 数据库线门禁 | Q5 (a)：`report_diff` 改走 `diff_pid_export`；基线加 `A01 · Export.mdf` 与 `A01 · TEST02_p 目录` 两项 | A01 两种入口打印的每一行都在基线里；删改回归仍退出 1 | Q5 | 中 |
| S12 DWG 少的那个 OPC | `799B9008…`：参考里是个没有表示、没有关系的孤立 `PIDBranchPoint`；dump 里它的配对已画到本图，那条表示不在参考版的 `.pid` 里。要定的是「没画到图上的 OPC 是否照发一个孤立对象」 | 同版参考里再出现一例才写规则；否则记为版本差 | Q4 的数据 | 小 |
| S13 DWG 进基线 | `publish-diff-accepted.json` 加 `DWG-0202GP06-01 · .pid`（S5–S9 之后剩下的才登记）与版本差说明；`tests/publish_diff_accepted_baseline.rs` 加一条 | 与 `tests/publish_from_pid.rs` 的三条钉值一致 | S5–S9、Q4 | 小 |
| S14 14 条 DWG 软跳过 | Q1 (a)：删掉这 14 条等 `Export.mdf` 的用例，换成对 Oracle 包真跑的；Q1 (b)：让它们读 Oracle 行 | 没有一条 `publish_*` 用例在仓库夹具齐全时软跳过 | Q1 | 小 / 大 |

S5 做完，接口元素这一栏就满了；S7 + S8 做完，7 张图的非空值从 19 734 / 39 171（50.4 %）到 39 011 / 39 171（99.6 %），豁免清单只剩合成值。

## 4. 风险

- **规则量不出来的属性。** 层级枚举和单位写法现在只有参考可对，原厂的实现在 `EFAdapter.dll`（2026-11-05 已有 IDA 库在 `D:\ida_scratch\publish-native\efadapter-fluid\`）。S8 / S9 按「先量、全对才写」做，量不出的留在豁免清单里并写明。
- **只有一个库有全量参考。** 7 张同版参考都来自同一个工厂库。数据映射是三个包共用的同一份，所以按映射写的属性换库也该成立，但只有 DWG（版本差）和 A01（MTF）能旁证。
- **`.pid` 单发的输出会变**（S5）。交付目录 `publish-v3` 要重发成 `publish-v4` 并出逐字节对比，照 2026-11-05 的做法。
- **与别处的改动撞车。** 主检出未提交的 `rusqlite` 升级动了 `sqlite_load.rs` 与 `publish_a01_raw_residual.rs`（本计划 S11 会碰前者）；stash 动了 13 个 publish 源文件（Q2）；`CHANGELOG.md` 与 `AGENTS.md` 顶部是 pid-plot 那条线在写。
- **A01 的三类合成残差**不会消失：库里搜不到那些值。

## 5. 执行记录

### 第一阶段（2026-11-10）

四片都是「先有红、再改、再跑门」：红是 `803984a` 的二进制在 `target/publish-audit/runs/base` 里留下的输出；门是 `cargo fmt --all -- --check`、`cargo clippy --locked -p pid-parse --all-targets -- -D warnings`、`cargo test --locked -p pid-parse --lib`、全部 14 个 `tests/publish_*.rs` 与 `parser_panic_safety`。每片之后把整张对照矩阵重跑一遍，与上一片逐文件比字节。

| 片 | 提交 | 消掉的 | 其余输出 |
|---|---|---|---|
| S1 | `80a5f73` fix(publish): read an exp dump whose tables hold more than integers | DWG `--oracle-exp` 与 `DWG-0202GP06-01_p` 目录从「读 dump 失败、0 文件」到能发布（单图 108 / 99 / 133，整包 7 张、0 失败） | 228 / 228 个文件逐字节不变 |
| S2 | `a796323` fix(publish): publish an equipment label's Description where the data map sends it | DWG 两种模式各少一条值错（`IObject.Description "新建"` 对 `"污水池"`）和一条 `missing value`（`LongMaterialDescription`） | 只有 DWG 的 3 个 `_Data.xml` 变，其余 250 个文件不变 |
| S3 | `a0f075f` fix(publish): give a note's text the line breaks the plant database stores | DWG 两种模式的最后一条值错（`INote.NoteText` 的 `\r` 对 `\r\n`）；`.pid` 单发从此 13 张图值错全为 0（A01 的 3 处 `GraphicOID` 是登记项） | 只有 DWG 的 3 个 `_Data.xml` 变，其余 250 个文件不变 |
| S3b | `dc78a7c` fix(publish): leave IOffpageConnector out when the connector has no ToFromText | 我方写了而参考对象没有的接口元素：轻烃分离器I 14 个、DWG 4 个空的 `<IOffpageConnector/>` 归零（13 份参考的 142 个 OPC 里 123 个带 `ToFromText`、19 个没有这个元素、0 个写空的）。门禁不比接口元素，这一条靠普查看出来 | 53 张整包里 20 个（轻烃分离器I 14、低压气压缩机入口改造 6）、DWG 包里 5 个同样的空元素消失；矩阵 253 个文件里 6 个 `_Data.xml` 变，其余 247 个不变 |

S3 与 S3b 提交时各跑了全量：`cargo test --locked -p pid-parse --all-targets` 1813 过 / 0 败 / 3 ignored；`--no-default-features --lib` 937 过；wasm32 `cargo check` 过；`missing documentation` 0 = 基线 0（`cargo rustdoc` 自身因 11 条既有的 private intra-doc link 报错退出，都不在本轮改过的文件里，棘轮脚本本来就不看它的退出码）。

新增的门：`tests/publish_from_pid.rs` 的 `the_dwg_fixture_alone_states_nothing_the_reference_lacks`、`the_dwg_fixture_reads_its_own_packages_exp_dump`、`the_dwg_package_publishes_every_drawing_it_lists`，全部读仓库内夹具，任何检出都真跑；`oracle_exp` 3 条、`pid_load` 2 条单测，`pid_writer` 一条单测加了断言。

停在这里的原因：剩下影响大的切片（S4 之后）都要先有 Q1–Q3 的答复；S0 不依赖批复，留给下一阶段开头。
