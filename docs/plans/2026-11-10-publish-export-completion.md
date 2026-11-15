# publish 导出收尾计划（2026-11-10）

> 依据：`docs/analysis/2026-11-10-publish-clean-is-nothing-wrong-not-everything-stated.md`（现状、对照矩阵、完整度普查与归因都在那里，本文不重复数字的出处）。
> 承接：`docs/plans/2026-10-13-publish-progress-and-next-steps.md`（N9–N12）与 `docs/analysis/2026-11-05-seven-reference-publish-diff.md`（七张参考图）。
> 基线 `main` = `803984a`；工作在 worktree `D:\work\plant-code\cad\pid-parse-wt\publish-export`、分支 `publish-export-20261110`，不 push、不动 `main`。2026-11-12 把 `main`（`7f71586`）并进了分支（`7de1ef1`）。
> 状态（2026-11-15）：本计划的分支已并入 `main`，之后各片直接并进 `main`。Q1、Q3、Q5 按 (a) 做；S0–S6、S9、S11 已在 `main`，S7 做了 9 个属性里的 7 个，S8 做了不分层级的枚举；交付重发为 `publish-v6`，7 张同版图非空值 35 519 / 39 171，见 §5 第六阶段。之后 S8 的层级按参考图读出的表（`codelist_values::LEAF_LEVELS`）写出，就是 §5 第六阶段量出的那 2 392 个，非空值到 37 911 / 39 171（未重发交付）；表里没有的叶子仍不写。S10、S12、S17 与 Q4、Q6 等现场数据（GitHub #71），Q7 维持 (a)。主检出的 stash（Q2）已逐块比对并存成分支 `publish-stash-20261108`。此前：§5 的四片、S0、独立审查之后的收紧与 S6；第四阶段（2026-11-12）并入 `main`、重发 `publish-v4`、对库普查；第五阶段（同日）复核 GitHub #70 并给入库的 DWG 包补库一级的用例。

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
| Q1 | 库里的属性从哪条线补 | (a) 在 `.pid` + exp 线上补：类的固定接口表 + 按 `SPPIDDataMap.xml` 把 dump 的列发成属性；(b) 把 Oracle `exp` 的行送进数据库线（MDF 那条线的 `sqlite_load` + `xml_writer`，`backup-store-v2` 分支计划的方向）；(c) 两条都做 | **(a)**。`.pid` + exp 线的对象与关系在 7 张图、21 个类上已经 100 % 对上，这是最难的一半；数据库线只在 A01 一张图、8 个类上验证过，比它少 6 个类，写手是逐类手写的。数据映射是原厂自己的文件，对 87.5 % 的参考值给得出「哪一列发成哪个属性」（名字级的匹配，不管值怎么写；只看 7 张图还没写出的 19 437 个值是 76.9 %，叫不出名字的 4 488 个是层级枚举里映射没有点名的几级，见分析 §3.4），比照参考手抄更站得住。(b) 的好处是 14 条 DWG 软跳过能真跑、MTF 与 Oracle 同一条路，代价是要一个完整的 Oracle 行解码器（LOB、所有类型），那边分支上有未提交的一半。不选 (a) 的话 S5–S9 换成「Oracle 行进 SQLite」一大片 |
| Q2 | 主检出 `stash@{0}`（18 个文件的 publish 未合入改动）怎么办 | (a) 原样并到本分支，单独成提交，再在其上继续；(b) 只取门禁那几项（接口、`<Container>`、重复 UID、包模式版本检查）；(c) 不要 | **(a)**。在本 worktree 临时套上验过：348 条库测与 14 个 `publish_*` 全过，对照矩阵不变。它让门禁更严，所以**会往基线里加条目**（A01 27 → 28，I井场 别名 1 → 2）并改两处测试计数——这是口径变严带来的登记，不是放宽，但要你点头。它与本轮 S1 改了 `oracle_exp.rs` 的同一处（`ExpTables::read` 五张表改宽松读）、与 S3b 改了 `pid_writer.rs` 的同一处（空的 `IOffpageConnector`），并入时这些行两边写法相同，`oracle_exp.rs` 的端口投影（它改成跳过非端口行，本轮保持报错）和测试辅助函数要手工对一下。2026-11-12 补：并入 `main` 之后它不能再直接套上——`git apply --check` 对 18 个文件里的 13 个通过，`backup_package.rs`（`main` 加了断链告警）、`oracle_exp.rs`、`pid_writer.rs`、`pid_load.rs`、`tests/parser_panic_safety.rs` 这 5 个不过，(a) 现在是一次三方合并 |
| Q3 | `.pid` 单发要不要也写类的固定接口（`<IDrawingItem/>` 这类没有属性的元素） | (a) 两种模式都写；(b) 只在有 dump 时写，`.pid` 单发保持「只写文件里说的」 | **(a)**。13 份参考里 12 个类的接口序列只有一种写法，这是类的定义，不是从库里读的值；不写的话下游按接口认对象时，`.pid` 单发的对象什么都不是。空值属性（`InstrTagSuffix=""`）建议一并照参考写 |
| Q4 | DWG-0202GP06-01 的备份包比参考新（管段的序号、位号写法都改过） | (a) 承认它是版本差：`.pid` 单发收进基线，`--oracle-exp` 的管线类差异钉在测试里并写明原因；同时请现场对**现在的库**重新发布一次这张图，或给参考当时的备份；(b) 只管 `.pid` 单发 | **(a)**。这是唯一一个三件都在仓库里、CI 上真跑的 Oracle 图。有了同版参考，「`T_PIPELINE` 有行时管线用它的 UID」（S10）才能验证 |
| Q5 | 数据库线（`Export.mdf --drawing`）的命令行门禁只比个数 | (a) 换成与 `.pid` 线同一个 `diff_pid_export`，A01 的三类合成残差（连接件 UID、`Rel` UID、`GraphicOID`）逐条登记进基线；(b) 维持 | **(a)**。测试层已经逐字节比过，命令行给用户的却是最弱的一档；MTF 包走包门禁时报的 27 条现在既没登记也没人看 |
| Q6 | 还缺的参考数据 | 请现场提供：① 4 张旧集注站图那一版的备份包；② 53 张里没有参考的图挑几张发布（建议 C井场——27 个只在库里的管件是外推的，另有 21 个库里画着而 `.pid` 没画的仪表、注释、管嘴、设备现在不发，见 S17——和任意一张带设备 `Description` 标签的图）；③ Q4 的 DWG 重新发布；④ 一张本图 stockpile 里放着 OPC 的图的发布件，连同发布那一刻的备份（`DWG-0202GP06-01_p` 这个库里 `DWG-0201GP06-01` 上就有 2 个，S12） | 没有这些，对应的结论只能停在「外推」或「版本差」 |
| Q7 | 阶段跟的列里有一格读不懂时，拒收到哪一级（2026-11-12 加，GitHub #70 的口径与现状不同） | 现状（`51521ef`）：取这一列的阶段整个报错，整包里凡是要挂库内容的图全部失败，`_Index.json` 逐图点名表、列、行，退出 1；单图 `--oracle-exp` 退出 1、不写文件。拿入库包试过：`T_PIPERUN` 一格坏，7 张图失败 6 张，其中 5 张与那条管段无关（§5 第五阶段）。(a) 维持；(b) 只让用到那一行的图失败，其余照发；(c) 工单的口径：那一格（或那一行）不发，其余照发，stderr 告警、`_Index.json` 的 `problems` 记一条，退出码不变——与 `main` 上断链的处理是同一个思路 | **(a)，直到有一个真实的包触发它**。两份真实 dump 的 66 个被跟列里没有一格没读懂，所以现在改，写的是没有样本的规则：哪些格子丢掉整行、哪些只丢这个属性，要看那一格在原厂数据里是什么；现在的报错点名表、列、行，第一个真实的例子来了能照着查。(c) 是工单的原话，要的话约一到两天：每个投影逐行记「这一行不能用、为什么」，包与命令行多一条告警通道，66 列那条用例的断言从「报错」改成「发出、并在问题清单里点名」——这是改断言，所以要你点头 |

## 3. 切片

按影响从大到小。「依赖」列空着的现在就能做。大小：小 = 半天内，中 = 一两天，大 = 多天。

| 片 | 改什么 | 怎么验证 | 依赖 | 大小 |
|---|---|---|---|---|
| S0 ✔ 完整度探针入库、比例钉进测试 | §5 第二阶段 | | | |
| S1 ✔ exp 宽松读 | §5 | | | |
| S2 ✔ 设备的 `Description` 标签 | §5 | | | |
| S3 ✔ 注释正文的换行 | §5 | | | |
| S3b ✔ 无 `ToFromText` 的 OPC 不写 `IOffpageConnector` | §5 | | | |
| S4 门禁看接口元素、`<Container>`、重复 UID | Q2 (a) 时直接来自 stash；否则重写：`PublishBlocks` 记每个对象带的接口与根属性，写出侧多三种发现项 | 三条删改回归（多写一个接口 / 改 `Plant` / 重复一个块）各退出 1；13 张图矩阵不变 | Q2 | 小（并入）/ 中（重写） |
| S5 类的固定接口表 | `catalog.rs` 每类一行有序接口表（13 份参考量出，可选接口单列条件：有值才写）；`pid_writer` 按表写；门禁加参考侧接口比对，表外的接口记发现项 | 7 张图 exp 模式接口元素 56–58 % → 100 %；DWG 与 4 张旧图 `.pid` 单发同样到 100 %；写出的接口无一不在参考里 | S4、Q3 | 中 |
| S6 ✔ 数据映射读全 | §5 第三阶段 | | | |
| S7 库属性第一批：原样的值 | 从 dump 取、不经枚举与单位：`IPIDTypical.IsTypical`、`IConnector.RepresentationsAreAllZeroLength`、`IInlineComponent.IsFlowDirectional`、`IPipingSpecifiedItem.PipingMaterialsClass`、`IInsulatedItem.InsulSpec`、`INamedPipingConnector.PipingConnectorSeqNo`、`IPipeCrossSectionItem.NominalDiameter`（补标签没给的）、`IObject.Name`（`ITEMTAG`）、`IObject.Description`（设备取 `NAME`）。每个属性一个提交，同时把它从 `EXCLUDED_ATTRIBUTE_CLASSES` 里拿掉——拿掉之后 7 张图逐值对参考 | 每片：该属性在 7 张图 exp 模式「缺 / 参考」归零、值错 0；`.pid` 单发不变。7 张图合计缺 19 437 个非空值，这一批占 7 667 个 | S6 | 大（9 个小片） |
| S8 库属性第二批：枚举 | `IPBSItem.ConstructionStatus(2)`、`IPipingConnector.PipingConnectorType`、`IConnector.FlowDirection`、`IPipingComponent.*`、`IInstrument.*`、`IInstrumentOcc.InstrumentLocation`、`IEquipment.EqType0–5` 与 `EqTypeDescription`、`IEquipmentComponent.*`、`IDocument.DocCategory / DocType`。要先定层级枚举的规则（`ConstructionStatus` 写 `@NewConstruction`、`ConstructionStatus2` 写 `@{GUID}`；`EqType0…5` 是一条链）。S6 读出来的结构与这里原先的设想不一样，见 §5 第三阶段「层级」：映射里的 `MapEnumMapEnum` 是库这一侧的层级（类 → 子类 → 型），发布件各级跟的是目标枚举在 schema 一侧的层级，映射里没有 | 规则先用探针对 7 张图量：每个 `(类, 属性)` 预测值与参考逐值相同才进代码。这一批占 11 610 个；再剩 160 个零散的（仪表位号 102、`LongMaterialDescription3` 34、`EquipPrefix` 17、`ReliefPressure` 6、`EquipmentTrimSpec` 1）随 S7 / S9 走 | S6；规则若量不出要 `EFAdapter.dll` 的反汇编 | 大 |
| S9 带单位的值 | `TotalInsulThick`（`50 mm`）、`InsulThickSrc`、`SlopedPipingAngle`（`2.9999910000486E-03 rad`）、`HeightRelativeToGrade`（`3 m`）、`ProcessVessel_VesselVolumetricCapacity`（`27 m^3`）、`CatalogPartNumber` | 样本只有 DWG 一张图加 A01；规则写成「SI 列 + 单位名」，对这两张逐值相同 | S6；样本少，建议等 Q6 ② 的图 | 中 |
| S10 管线用 `T_PIPELINE` 的 UID | 库里有 `T_PIPELINE` 行时，管线的 UID 取 `SP_ID`、名字取它的 `ITEMTAG`，不再合成；管段归哪条管线的规则待定（`SP_PARTOFID` 在 DWG 包里全空） | 需要同版参考 | Q4 的数据 | 中 |
| S11 数据库线门禁 | Q5 (a)：`report_diff` 改走 `diff_pid_export`；基线加 `A01 · Export.mdf` 与 `A01 · TEST02_p 目录` 两项 | A01 两种入口打印的每一行都在基线里；删改回归仍退出 1 | Q5 | 中 |
| S12 本图 stockpile 里的 OPC（未决，样本 1 例） | 现状：只发表示在 `.pid` 里的 OPC。DWG 的参考多一个 `799B9008…`，是没有表示、没有关系的孤立 `PIDBranchPoint`；dump 里它的配对 `9B50CC42…` 只有一条表示，在本图上、`INSTOCKPILE = 2`（dump 自己的数据字典：布尔代码表，2 = `True`）、没有 `GRAPHICOID`，即本图 stockpile 里的条目。dump 现在的状态就解释了参考，所以不是版本差（本行原先写「dump 里它的配对已画到本图」，错了，独立审查指出）。候选规则：本图 stockpile 里的 OPC 照发一个对象，UID 取配对里较小的那个，带 `IObject.Name`（`OPCTAG`）与类的固定接口，不发表示、不发关系。管件已有一条相近的规则（`ExpTables::dump_only_piping_components`：本图上有表示而 `.pid` 没画的管件照发，带端口与表示）；OPC 这 1 例不能直接套它，因为那条表示没有 `GRAPHICOID`，参考发出的对象不带表示，UID 又是配对的另一端。这一轮不改发布规则 | 要第二个样本才写进代码：一张 stockpile 里有 OPC 的图的原厂发布件，加同一时刻的 dump。用它定四件事：① 是不是每个这样的 OPC 都发；② UID 是「配对里较小的」还是「不在本图的那一端」——现在这 1 例两种说法给出同一个 UID，要一例 stockpile 那一端 UID 较小的；③ 带哪些接口与属性（有 `ToFromText` 时写不写 `IOffpageConnector`）；④ 两端都在 stockpile 里时发几个。53 张那个库没有样本（476 对 OPC 都是一端在图上、另一端在 `SP_DRAWINGID = 0`）；DWG 包里另有 2 个在 `DWG-0201GP06-01` 上，没有参考。整包用例钉的 4 个 `PIDBranchPoint` 同理只是现状 | Q6 ④ | 小 |
| S13 DWG 进基线 | `publish-diff-accepted.json` 加 `DWG-0202GP06-01 · .pid`（S5–S9 之后剩下的才登记）与版本差说明；`tests/publish_diff_accepted_baseline.rs` 加一条 | 与 `tests/publish_from_pid.rs` 的三条钉值一致 | S5–S9、Q4 | 小 |
| S14 14 条 DWG 软跳过 | Q1 (a)：删掉这 14 条等 `Export.mdf` 的用例，换成对 Oracle 包真跑的；Q1 (b)：让它们读 Oracle 行 | 没有一条 `publish_*` 用例在仓库夹具齐全时软跳过 | Q1 | 小 / 大 |
| S15 数据库线的注释正文与 OPC 接口（只记录） | 读代码所见（独立审查提出，已对照代码）：`xml_writer::write_item_note` 的 `INote.NoteText` 取对象的 `NoteText` 字段，没有就取 `T_ModelItem.Description`；`mdf_load` 暂存的 24 张表里有 `T_ItemNote`、没有放正文的 `T_Note`。`.pid` 线取的是标签的 `Note.Body`，库里对应 `T_NOTE.BODY`；Oracle dump 的 `T_ITEMNOTE` 只有 2 列、没有正文，7 张同版图参考里的 178 个注释在库里的 `T_MODELITEM.DESCRIPTION` 全空。照这样，数据库线遇到带正文的注释会写成空的 `<INote/>`。`write_pid_branch_point` 从不写 `IOffpageConnector`，`T_OPC` 也不在暂存的表里：没有 `ToFromText` 时两条线一致，有值时数据库线缺这个接口 | 仓库里没有带注释或 OPC 的 MDF 夹具（A01 的 12 个对象里这两类都没有），这一轮不改；有夹具之后先写红测 | 一份带注释与 OPC 的 MDF / MTF 备份 | 小 |
| S16 管线名回退时的流体代号（只记录） | `codelist_short_text(19)` 只读 `CODELIST_SHORT_TEXT`。19 号代码表里出厂自带的代号只填 `CODELIST_TEXT`（两个包都是 173 行），所以 `fluid codes missing` 把它们计成缺失：DWG 包 5 条管段（代号 493 = `OD`），53 张那个库 70 条（代号 20 = `AI`）。管段在 `T_PLANTITEM` 里有行时名字取 `ITEMTAG`，输出不受影响；dump 没有这张表或这一行时，回退拼出的名字会少掉流体这一格 | 找到一条走回退拼名、代号又只有 `CODELIST_TEXT` 的管段并对上参考之后再改 | 那样的样本 | 小 |
| S17 库里画在图上、`.pid` 没画的非管件对象（只记录，2026-11-12） | 现状：`ExpTables::dump_only_piping_components` 只对管件做「库里在本图上有表示、`.pid` 没画就照库发」，依据是 J K井场 的 3 例。53 张整包对库普查（`docs/analysis/2026-11-12-what-the-plant-database-draws-that-a-publish-leaves-out.md`）量出另有 23 条同样情形的表示没有发：C井场 21（仪表 5、注释 7、管嘴 5、机械设备 3、容器 1），Q井场 2（仪表）。库里它们与发出的行没有区别，`.pid` 里没有这些图元。这一轮不改：别的类没有任何参考样本，而且各有连带内容（管嘴的组成关系、注释正文、仪表位号与信号端口） | 探针 `probe_which_database_representations_a_publish_leaves_out` 的「left out of a class the publish states elsewhere」从 23 到 0，「states … the database does not draw」保持 0；写出的对象对上 C井场 的原厂发布件 | Q6 ② 的 C井场 发布件 | 中 |

S5 做完，接口元素这一栏就满了；S7 + S8 做完，7 张图的非空值从 19 734 / 39 171（50.4 %）到 39 011 / 39 171（99.6 %），豁免清单只剩合成值。

## 4. 风险

- **规则量不出来的属性。** 层级枚举和单位写法现在只有参考可对，原厂的实现在 `EFAdapter.dll`（2026-11-05 已有 IDA 库在 `D:\ida_scratch\publish-native\efadapter-fluid\`）。S8 / S9 按「先量、全对才写」做，量不出的留在豁免清单里并写明。
- **只有一个库有全量参考。** 7 张同版参考都来自同一个工厂库。数据映射是三个包共用的同一份，所以按映射写的属性换库也该成立，但只有 DWG（版本差）和 A01（MTF）能旁证。
- **`.pid` 单发的输出会变**（S5）。交付目录要重发并出逐字节对比，照 2026-11-05 的做法。（`publish-v4` 已在 2026-11-12 发出，与 `publish-v3` 只差 S3b 的 20 行，见 §5 第四阶段；S5 之后要发的是下一个号。）
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

新增的门：`tests/publish_from_pid.rs` 的 `the_dwg_fixture_alone_states_nothing_the_reference_lacks`、`the_dwg_fixture_reads_its_own_packages_exp_dump`（第三阶段改名为 `the_dwg_fixture_publishes_against_its_packages_later_dump`）、`the_dwg_package_publishes_every_drawing_it_lists`，全部读仓库内夹具，任何检出都真跑；`oracle_exp` 3 条、`pid_load` 2 条单测，`pid_writer` 一条单测加了断言。

停在这里的原因：剩下影响大的切片（S4 之后）都要先有 Q1–Q3 的答复；S0 不依赖批复，留给下一阶段开头。

### 第二阶段（2026-11-10）：S0 与测试的临时目录

Q1–Q6 答复之前做的两件不依赖答复的事。门与第一阶段相同，每个提交另跑一次全量（`--all-targets`、`--no-default-features --lib`、wasm32、`missing documentation`）。

| 提交 | 做了什么 |
|---|---|
| `624916d` test(publish): remove a test's scratch directory when the test ends, passing or not | `tests/common/mod.rs` 加 `Scratch`（值 drop 时删目录），在临时目录下建目录的三个 `publish_*` 测试文件都改用它 |
| `0c8f9dc` feat(publish): measure how much of a reference a generated document states | `publish::completeness::measure` 与探针 `examples/probe_how_much_of_a_reference_does_a_publish_state.rs` |
| 本提交 | `tests/publish_completeness.rs`：10 条用例、17 个钉 |

**完整度的口径。** `measure` 只读两份文档：参考的 `<Container>` 属性、对象、接口元素、非空值、空值属性、关系各有多少，我方写出其中多少。它不读豁免清单。`Completeness::reference_attributes` 把各分母加起来，测试拿它和参考正文里数出的属性个数比：13 份参考共 200 860 个属性（不含 XML 声明），逐份相等，所以参考里没有落在分母之外的内容。对象要同 UID 且同类才算写出；UID 给不出来的 `PIDPipeline` 按 `IObject.Name` 配对（与门禁相同），配对数单列。关系按两端与 `DefUID` 算，我方的一条关系只顶参考的一条。

**与 `census.py` 的对照。** 分析页的完整度数字出自 `target/publish-audit/census.py`。探针对那 23 次运行（13 份参考）逐次重算，22 次的总数与全部分项相同。A01 `.pid` 单发不同：脚本只按 UID 配对，把那个已登记的错类对象（参考 `PIDPipeline` 的 UID 下我方写的是 `PIDPipingConnector`）算作写出，并把连接件的三个接口算作「参考没有的接口」；探针读作对象 6 / 12、接口元素 9 / 105、参考没有的接口 1 个，脚本是 7、10、4。给脚本补上「同类才算」之后 23 次全部相同（分项 5 705 行）。对照表在 `target/publish-audit/s0/probe-vs-census-s4*.txt`。

**钉法。** 分母是参考自己的数，精确钉。分子只许升：降了失败，升了也失败，直到把钉抬到新值，所以钉永远等于今天的读数。「写得与参考不同的值」「参考对象没有的接口元素」只许降。DWG-0202GP06-01 三种模式读仓库内夹具，没有软跳过分支；7 张同版参考图的两种模式要语料，缺的时候打印是哪个目录不在再跳过。7 张图的钉就是分析页 §2.4 表里的数。

| DWG-0202GP06-01 | 对象 | 接口元素 | 非空值 | 空值属性 | 关系 |
|---|---|---|---|---|---|
| `.pid` 单发 | 78 / 108 | 164 / 747 | 71 / 236 | 0 / 93 | 81 / 133 |
| `--oracle-exp` | 102 / 108 | 332 / 747 | 99 / 236 | 0 / 93 | 127 / 133 |
| 备份包目录 | 100 / 108 | 328 / 747 | 58 / 236，另 39 个写得不同 | 0 / 93 | 123 / 133 |

包目录那一行低，是因为包里的 `.pid` 比参考新：39 个不同的值里 37 个是 `GraphicOID`、2 个是 `PipingConnectorSeqNo`；那台设备（`PIDProcessVessel` F1081777…）后来有了 `EquipSeqNo`，而参考对象没有 `INamedEquipment`，这是唯一一个「参考对象没有的接口元素」。

**红绿。** 三次运行的输出在 `target/publish-audit/s0-redgreen/`（不入库）：

1. 把 `pid_load::published_attribute` 里的 `PipingMaterialsClass` 标签关掉：10 条用例全部失败（DWG `.pid` 单发接口元素 164 → 158、非空值 71 → 65；I井场 `.pid` 单发 3559 → 3516、1530 → 1487）。同一棵树上 `--diff-against` 的结论没变：DWG 仍是 17 条 `missing value`、退出 1，I井场 `--oracle-exp` 仍是 clean、退出 0，因为这个属性本来就在豁免清单里。
2. 恢复后，把 DWG 那 17 条 `missing value` 背后的 7 个 `(接口, 属性)` 加进 `EXCLUDED_ATTRIBUTE_CLASSES`：DWG `.pid` 单发的门禁从退出 1 变成 clean、退出 0，10 条用例照原钉通过，没有一个数字动。
3. 再恢复：与改前相同。

**测试的临时目录。** 改前逐个测试文件对着清空的临时目录跑，只有 `publish_from_pid` 在通过时留下东西：每轮 12 个目录、43.8 MB（整包 53 张的输出 `sqplant` 占 42 MB），都是成功路径上从来没删。其余用到临时目录的用例只在函数最后一行删，断言失败时留下（`publish_from_pid` 与 `publish_xml_cli` 各强制失败一条验过）。改后 15 个文件各留 0 个，强制失败的两条也留 0 个；110 条用例的名字、结果与 21 行 skip 输出同改前。

库自己写临时文件的地方只有一处：MTF 包暂存的 `.mdf`（`mdf_load::open_mtf_dump_as_sqlite`）。正常结束、包门禁退出 1、暂存后读库失败、4 种截断或损坏的 dump，共 7 种情形下临时目录都是空的（`target/publish-audit/tmp-leak/lib/`）。读代码可见一条没能复现的路径：写暂存文件中途失败（例如盘满）时 `?` 直接返回，写了一半的文件留在临时目录里。主检出 `stash@{0}` 的 `StagedMdf` 补的正是这一条。publish 线之外还有三处测试留临时目录，本轮没动：`src/symbol_library.rs` 的两条单测、`tests/inspect_cli.rs` 的四条、`src/inspect/commands.rs` 的一条。

### 第三阶段（2026-11-10）：独立审查的十条与 S6

Q1–Q6 仍未答复。这一阶段做两件不依赖答复的事：处理一个只读的独立审查对第一阶段四处修复与分析页的十条结论（报告在 `target/publish-audit/review-5c61ac4.md`，不入库），以及 S6。每一条先用自己的脚本或读法复现，再改。门比前两阶段多一步：全量门禁对着**暂存的改动**跑（`fmt`、`clippy -D warnings`、`--lib`、`--all-targets`、15 个 `tests/publish_*.rs` 逐个带 `--nocapture`、`parser_panic_safety`、`--no-default-features --lib`、wasm32、`missing documentation`），提交后核对提交的 diff 与过门的那一份指纹相同。

| 提交 | 审查项 | 做了什么 |
|---|---|---|
| `b2576fc` fix(parser): refuse an exp INSERT whose column list follows VALUES | ⑥ | `column_names` 遇到「`(` 在 `VALUES` 之后」的 INSERT 行按下标切片，panic；改成 `get` 并报错。`publish::oracle_exp` 的 7 个字节级入口进了 `tests/parser_panic_safety.rs`，另有一条用例把一份 2 530 字节的合成 dump 逐前缀截断、逐字节替换。加进去之后对抗语料又打出一个：`nominal_spelling` 把公称直径代码乘 100 时 `i64` 溢出，改成 `checked_mul` |
| `51521ef` fix(publish): refuse every exp column a stage follows when a cell of it was not read | ① ⑦ | 取列的两个函数 `column_index` / `optional_column_index` 拒绝「列在、但没读懂」的列：不是 AL16UTF16 的字符列，或有一格没读出来的列（`Column::unread_row` 记第一处），报错点名表、列与行。「dump 里没有这一列」对可选列仍然合法。连接关系的端点集合改从端口阶段的投影取，同一行两个阶段给同一个结论 |
| `71e02a2` test(publish): hold the exp reader's charset-id check and its port-column checks | ④ | 两条用例：国家字符集 UTF8（871 / 2）的列留作没读；端口表先为它的列负责、再看行 |
| `ba5a83f` test(publish): pin which findings the DWG fixture prints, not how many | ⑤（③ 的测试注释） | `("missing value", 17)` 换成逐行钉：`DWG_VALUES_NOT_STATED` 17 行、`DWG_EXP_OBJECT_AND_REL_FINDINGS` 18 行，原样比对。`5 fluid codes missing` 的原因、缺的那个 `PIDBranchPoint` 是未决的 stockpile 规则、整包那 4 个 `PIDBranchPoint` 同理，都写在钉旁边。`…reads_its_own_packages_exp_dump` 改名 `the_dwg_fixture_publishes_against_its_packages_later_dump` |
| `0569645` docs(publish): name the cell the strict exp read stopped on, and the samples behind three rules | ② ⑨ | 拦住旧读法的是第 0 行 `SP_INSULTHICKSI` 的 `c0 06`（0.05），不是 `SLOPE`：代码注释、测试、CHANGELOG、分析页四处改对，CHANGELOG 加一句更正。三条规则各自的样本量写进代码注释与分析页 §3.2 |
| `28a8e29` feat(backup): read the class and property maps of SPPIDDataMap.xml | S6 | 见下 |
| `8f256a2` docs(publish): record the stockpile OPC as an open rule, what the 87.5 % counts, and the third phase | ③ ⑧ ⑩ | 分析页 §3.4、§3.6、§6 与本计划的 Q1、Q6、S8、S12、S15、S16 |
| 本提交 | S6 的收尾 | 探针把「两种算法都叫不出名字」的值单独数：原来用总数减第一种算法的数，两种算法点到的值不同时这个数就名不副实。11 份与 7 份参考上数字没变（5 755 与 4 488），所以两种算法在这些参考上点到的是同一批值。`MapProperty::tool_oid` 的注释改对，单测把共用 `ToolOID` 的 10 个逐个钉住（见下第 6 条） |

**① 的复现与证明。** 把 DWG 包那份 dump 的 `T_PLANTITEM.ITEMTAG` 列头改成数据库字符集（873 / 1）：改前的二进制退出 0，管线从 6 条变 0 条、关系从 133 条变 127 条，一行报错都没有。审查只指出了「整列不是 AL16UTF16」这一种；同样静默的还有格子一级的，例如 `T_CONNECTOR.STARTOBJECT` 里出现一个小数，少一条连接关系。19 份各坏一处的 dump（10 份改字符集、9 份放一个小数）里，改前 11 份退出 0（4 份输出变了），改后 19 份全部退出 1、一个文件不写、报错点名表与列。单测 `every_followed_column_is_refused_when_its_cells_were_not_read` 把 16 张表里被跟的 66 个列逐个弄坏，改前的读法放过其中 24 个。真实数据不受影响的证明有两层：两份 Oracle dump（DWG 包、53 张那个库）里被跟的列没有一个不是 AL16UTF16、被跟的格没有一个没读出来（`target/publish-audit/review2/followed-census-*.txt`）；整张对照矩阵 30 次运行、253 个文件逐字节相同，退出码与标准输出也相同。第三个包 `TEST02_p` 的 `Export.dmp` 是 SQL Server 的 MTF，不走这个读法，它在矩阵里的输出同样没变。

**⑦。** `connectivity_ends` 原来自己读点号，读不懂的行跳过；端口阶段对同一行报错。现在它用端口阶段的投影，小数与 NULL 的点号两个阶段都报错、说法相同（`the_connectivity_stage_refuses_the_port_rows_the_port_stage_refuses`）。主检出 `stash@{0}` 把端口投影改成「跳过没有 id 或点号不是整数的行」；并入（Q2）时列一级的拒绝仍然成立，因为它在 `column_index` 里，行一级的要重新对一遍。

**变异验证。** 每个变异只改生产代码、不碰测试，跑完把文件逐字节还原（`target/publish-audit/review2/mutate.py`，日志在同目录的 `mutations/`）：

| 变异 | 结果 |
|---|---|
| `column_names` 换回下标切片 | `a_column_list_after_values_is_refused_not_a_panic` 红；`parser_panic_safety` 两条红 |
| `nominal_spelling` 换回不检查的乘法 | `a_nominal_diameter_code_at_the_far_end_of_i64_is_not_spelled` 红；`exp_dump_readers_do_not_panic_on_a_damaged_dump` 红 |
| 取列时放过别的字符集 / 放过没读的格 / 读表时不记没读的行 / 没读懂的 `ITEMTAG` 当作没有这一列 | 四个都红（`every_followed_column_…`，前三个另有一条） |
| 连接阶段丢掉端口阶段拒绝的点 | `the_connectivity_stage_refuses_…` 红 |
| 列头只看 form、不看字符集 id | 补用例之前**绿**（审查说得对），补之后 `national_text_in_another_charset_is_kept_unread` 红 |
| `project_piping` / `project_signal` 跳过开头的列检查 | 审查指出它在 `5c61ac4` 上没人守（没有回到那个提交上重跑）；到 `51521ef` 时矩阵用例已经让它红，`71e02a2` 再加一条专门的 `a_port_table_answers_for_its_columns_before_its_rows` |
| 把一条 `missing value` 换成另一条（豁免清单里一进一出，总数仍是 17） | 旧的计数钉**绿**，逐行钉两条用例都红 |

**③ 复核。** 属实，见分析 §3.6 与 S12。读 dump 用的是自己的脚本（`review2/opc_stockpile.py`），`InStockpile` 的含义取自 dump 自己的数据字典，不靠猜。

**⑧ 复算。** 属实，数字见分析 §3.4；`review2/item8_breakdown.py` 与入库的探针两条路算出同样的数。

**⑩。** 属实，记为 S15。

**矩阵。** `51521ef` 之后与 S6 之后各跑一遍（`audit.py` + `compare_runs.py`，30 次运行）：253 个文件逐字节相同，退出码与标准输出相同。这一阶段没有一个输出文件变过。`tests/publish_completeness.rs` 的 17 个钉没有动。

**门禁结果。** 表里前六个提交各跑一遍全量，每次都是：`fmt` 与 `clippy` 通过；15 个 `publish_*` 文件 116 过 / 2 ignored，`parser_panic_safety` 3 过；`--no-default-features --lib` 937 过；wasm32 通过；`missing documentation` 0 = 基线 0；`publish_*` 文件打印的 skip 行 21 行（分布在 5 个文件里），逐次相同。`--lib` / `--all-targets` 的通过数依次是 1 420 / 1 838、1 424 / 1 842、1 426 / 1 844、1 426 / 1 844、1 426 / 1 844、1 432 / 1 850，都是 0 败、3 ignored。`28a8e29` 那一次的 `--all-targets` 第一遍在 `tests/plant_backup_census.rs` 里中止（`memory allocation of 1572864 bytes failed`，机器上当时有别的构建在跑）；这条用例单独跑通过，峰值提交内存 2.6 GB，同一份暂存改动整步重跑是上面的 1 850。只改文档的 `8f256a2` 同样是 1 432 / 1 850，其余各项相同。本提交的门禁结果写在交回的报告里。

#### S6：数据映射读全

`src/backup/datamap.rs` 原来只读 `MapEnumToEnum` 与枚举表的名字。现在 `DataMap` 另外带：`classes`（`SPMapClassDef`，物项类型，连同 `MapClassToClass` 发成的类）、`properties`（`SPMapPropertyDef`，连同所属物项类型、`MapPropertyToProperty` 的每个目标、`MapPropertyMapEnumList` 的值表、`ProcessPropertyCriteria`）、`enum_parents`（`MapEnumMapEnum`）、`unit_list_names`、`rel_counts`。`DataMap::sources(类, 属性)` 回答「这个类上的这个属性由哪些库属性发出来」。没有一条写出路径用它；原有两个字段由原来那几句语句填，矩阵逐字节不变。

**钉值**（单测读入库的 `test-file/backup-test/DWG-0202GP06-01_p/RefData~4~804.zip`，没有软跳过分支；三个包里是同一个 3 678 496 字节的 `SPPIDDataMap.xml`，SHA-256 `248c6a3b…06794548`）：31 个物项类型、1 818 个属性定义、12 087 条 `<Rel>`，分属 18 种 `DefUID`，每一种的条数都钉了——计划原先写的 1 818 与 12 087 复核无误。其中 `MapClassToClass` 29、`MapClassMapProperties` 1 818、`MapPropertyToProperty` 694（693 个属性，一个属性有两个目标）、`MapPropertyMapEnumList` 1 022、`MapEnumMapEnum` 1 573、`MapEnumToEnum` 1 921。

**抽查**（`the_committed_map_sends_an_equipment_description_to_its_material_description` 等四条）：`Vessel.Description` 与 `Exchanger.Description → LongMaterialDescription`；`EquipmentOther.Description → LongMaterialDescription3`；四类设备的 `Name → Description`；`SPMP_Mechanical_Description` 与 `SPMP_ItemNote_Description` 没有目标；`PipeRun.TagSequenceNo → PipingConnectorSeqNo`；`PipeRun.InsulThick → TotalInsulThick`（单位表 `Thickness`）；`PipeRun.Slope → SlopedPipingAngle`（单位表 `Angle`）；`PipeRun.ConstructionStatus → ConstructionStatus`（`EFAdapter.EnumConversions`，枚举表 `SP_93`）。

**读到的结构**（只读出来，不解释；与原先设想不同的地方照实写）：

1. **每条 `MapPropertyToProperty` 自己点名发布类。** 694 条都带 `IMapRel End2MapValue`。637 条点的是所属物项类型的发布类；另外 57 条是 `PipeRun` 的工艺属性，点的是 `PIDProcessPoint`（`PipeRun` 自己发成 `PIDPipingConnector`）。所以「物项类型发成哪个类」加「属性发成哪个名字」不足以确定对象，要看这条关系自己的类。48 条另带 `EndEdgeDef = ProcessPointCollection_21`，都属于 `DuctRun`。
2. **3 个物项类型不发成任何类**（`ControlSysIO`、`Equipment`、`Revision`）；`Pipeline` 发成两个：`PIDPipeline` 与 `IProcessPoint`。
3. **`ProcessPropertyCriteria` 有 17 种写法**，另有 1 377 个属性没有这个属性、110 个是空串。最多的是 `EFAdapter.EnumConversions`（215）、`UoMToDoubleConv`（30）、`EnumToBoolConv`（28）；管段名字的拼法写在里面：`EFAdapter.PpConnNameConv, <ItemTag>-<NominalDiameter>-<PipingMaterialsClass>-<InsulPurpose>-<InsulThick>`。
4. **值表。** 1 022 条里 565 条指向枚举表、451 条指向单位表、6 条指向文件里没有定义的表（`SPMU_4`、`SPMU_37`、`SPMU_21`）。单位怎么写出来，映射只给到「这个属性的值在哪张单位表里」，换算与写法不在这个文件里。
5. **层级——与原先的设想不同。** S6 原来把 `MapEnumMapEnum` 写作「层级」，S8 设想用它定 `ConstructionStatus` / `ConstructionStatus2`、`EqType0…5` 这类属性各级的值。读出来的是：`MapEnumListMapEnumList` 有 11 对枚举表（设备的类 → 子类 → 型，管件的类 → 子类 → 型，仪表的类 → 型，设备部件的类 → 型，`FluidSystem → FluidCode`，文档的类别 → 类型，状态的类型 → 值，风管的类 → 子类 → 型），`MapEnumMapEnum` 的 1 573 条是这些表之间「上一级枚举 → 下一级枚举」，每个下级只有一个上级（设备部件那一对之间一条都没有）。这是**库这一侧**的层级。发布件各级的值对不上它：
   - `ConstructionStatus` 的值表 `SP_93` 不在任何一对里，参考却有两级。11 份参考里 `ConstructionStatus2` 的 3 255 个值全是 `SP_93` 某个枚举的 `MapEnumToEnum` 目标；`ConstructionStatus` 的 3 255 个值（`@NewConstruction`）不是任何枚举的目标。
   - 映射点名的是 `PipingComp.PipingCompClass → PipingComponentType1`，值表是「类」那张（`SP_27`）；参考里 `PipingComponentType1` 的 869 个值有 866 个是「子类」表（`SP_26`）枚举的目标，`Type2` 的 859 个里 738 个、`Type3` 的 661 个全部是「型」表（`SP_25`）枚举的目标。
   - `EqType0` 的 102 个值是 `SP_22` / `SP_23` / `SP_24` 里枚举的目标（同一个 GUID 是好几张表里枚举的目标）；`InstrumentType` 的 159 个值不是任何枚举的目标，`InstrumentType3` 的 159 个是 `SP_28` 的目标。
   - 映射给这四条链各点了一个名字（`ConstructionStatus`、`EqType0`、`PipingComponentType1`、`ProcessEqCompType1`），其余各级（`ConstructionStatus2`、`EqType1–5`、`PipingComponentType2/3`、`ProcessEqCompType2`）没点；仪表的 `InstrumentType` / `InstrumentType3` 与图纸的 `DocCategory` / `DocType` 一个都没点。

   所以各级的值跟的是**目标枚举在 schema 一侧的层级**，这份映射里没有。S8 的规则不能只从 `MapEnumMapEnum` 推出来，要么从参考里量（每个下级值配哪个上级值），要么读 schema 文件或 `EFAdapter.dll`。
6. **`ToolOID` 不是键。** 1 818 个属性只有 1 808 个不同的 `ToolOID`，10 对属性共用一个：9 对是同一个物项类型把同一个属性列了两次（例如 `Nozzle.NozzleType`，一次发成 `ProcessEqCompType1`，一次按 `EnumToStringConv` 发成 `ProcEqpCompTypeDescription`），1 对是 `PipeRun` 与 `DuctRun` 共用 `SPMP_PipeRun_ProcessDesign.Max.Temperature`。`DataMap` 按 `UID` 关联。
7. **库属性落到表和列，要靠 dump 自己的数据字典。** 映射只有物项类型名与属性名。dump 里的 `ITEM` / `ITEMATTRIBUTIONS` / `UNIQUEATTS` / `ENTITIES` / `ATTRIBUTES` 五张表把它们接到 `T_表.列`：694 条属性关系里 572 条接得上；119 条属于字典里没有同名物项类型的映射类（`InstrumentInline` 92、`DuctNozzle` 17、`InstrumentCtrlSysFunc` 10）；3 条的物项类型没有同名属性（`InstrLoop.TagPrefix`、`Instrument.TagPrefix`、`PipingComp.IsFlowDirectional`）。这一步现在只在探针里，没有进库。

**探针**：`examples/probe_where_the_data_map_says_a_published_value_comes_from.rs`。对参考的每个 `(类, 接口, 属性)` 给出映射里的来源、换算与值表；`--ours` 按我方写没写出来分开；`--dump` 再给出表和列。它复现了审查者的两组数：

| 输入 | 输出 |
|---|---|
| 11 份验收参考 | 45 963 个非空值，叫得出名字的 40 208 个（87.5 %）；按物项类型的发布类算与按关系自己点名的类算是同一个数 |
| 7 份同版参考，`--ours` 给 53 张整包的输出（这 7 张与单图 `--oracle-exp` 的输出逐字节相同） | 已写出 19 734 个（50.4 %），其中叫得出名字的 19 363 个；没写出 19 437 个，其中叫得出名字的 14 949 个（76.9 %）、叫不出的 4 488 个 |

DWG 那 17 个缺的值，探针逐个给出表和列，例如 `PIDPipingConnector IInsulatedItem.TotalInsulThick <- PipeRun.InsulThick [unit list Thickness] => T_PipeRun.InsulThick`。

**这一阶段没做的。** S4、S5 及之后的切片（等 Q1–Q3）；不并 stash；不动已接受基线；`.pid` 单发的输出没变；stockpile 的发布规则（S12，只有 1 例）；`StagedMdf` 那条「写到一半失败」的路径（随 Q2）；数据库线的两处不一致（S15，没有夹具）；流体代号只读 `CODELIST_SHORT_TEXT`（S16，没有样本）。没核实的：没有用 CI 的 stable 工具链跑（门禁用的是 `rustup run nightly cargo`）；`vendor/oxidized-mdf` 自己的测试没有单独跑；「层级跟 schema 一侧」是从值对不上映射推出来的，schema 文件本身没有读。

### 第四阶段（2026-11-12）：并入 main，重发 publish-v4，对库普查

Q1–Q6 仍未答复。这一阶段做三件不依赖答复的事。

**并入 `main`。** `main` 到 `7f71586` 有 87 个分支没有的提交，分支有 16 个 `main` 没有的。合并提交 `7de1ef1`（不 rebase）。`main` 那边与 publish 有关的是三个提交：记录链立不住时 `pid_publish_xml` 在 stderr 上出声（`5a7bc12`），`.pid` 只读一遍、告警说出这条流让 XML 少了多少条记录（`838a556`，`pid_load::read_publish_from_reader` 与 `PublishChainFault::describe`），整包发布点名是哪张图（`27e434c`，`BackupPublishReport::chain_faults`）；其余是解析线的记录链绊线与四个退役族。两边都改过的文件 5 个：`src/publish/mod.rs`、`src/publish/pid_load.rs` 自动合并；`tests/parser_panic_safety.rs`（模块注释）、`AGENTS.md`（门禁表）、`CHANGELOG.md`（Unreleased 顶部）各一处冲突，两边的内容都留。合并树相对 `main` 只在本分支改过的 16 个代码文件上不同；`pid-plot` 编 `pid-parse` 时不开 `backup-exp`，这 16 个文件它一个都不编，所以没有跑它的页面门禁。

合并树上的门禁（第一次用 CI 那一档工具链：stable 1.99.1，`--locked --offline`，`RUSTFLAGS=-Dwarnings`；前三个阶段用的是 nightly）：

| 门 | 结果 |
|---|---|
| `cargo fmt --all -- --check` | 通过 |
| `cargo clippy -p pid-parse --all-targets -- -D warnings` | 通过 |
| `cargo test -p pid-parse --all-targets` | 1 896 过 / 0 败 / 3 ignored（库 1 423，47 个集成测试文件 463，示例 10） |
| `cargo test -p pid-parse --no-default-features --lib` | 928 过 |
| `cargo check --lib -p pid-parse --no-default-features --target wasm32-unknown-unknown` | 通过 |
| `.github/scripts/check-missing-docs.sh` | rustdoc 退出 0；缺文档 0 = 基线 0 |
| `cargo build --workspace --all-targets` | 通过（`pid-plot` 编得过） |
| 16 个 `tests/publish_*.rs` 逐个带 `--nocapture` | 121 过 / 2 ignored；skip 行 21 行，与前三个阶段相同；临时目录里不留东西 |
| `vendor/oxidized-mdf`：`cargo test --lib` | 37 过 |

`tests/publish_completeness.rs` 的 17 个钉原样通过，所以每种模式写出参考的多少与合并前相同。整张对照矩阵（`audit.py`，30 次运行）用合并树的调试构建重跑：253 个文件与合并前 `bc813ef` 的输出逐字节相同，退出码与标准输出也相同；发布构建再跑一遍，与调试构建同样逐字节相同。A01 没有钉，探针今天的读数是：`.pid` 单发 对象 6 / 12、接口元素 9 / 105、非空值 1 / 29（另 3 个写得不同）、关系 0 / 15；`Export.mdf`、`Export_v2.sqlite`、`TEST02_p` 目录三种入口都是 8 / 12、63 / 105、14 / 29（另 4 个不同）、空值属性 4 / 15、关系 9 / 15，与 11-10 的读数相同。

**`publish-v4`。** `D:\work\cad\pid-test-data` 与 `D:\work\plant-code\cad\pid-test-data` 是同一个包（16 个文件 SHA-256 全同），只导一次。命令与 `publish-v3` 相同：`pid_publish_xml D:\work\cad\pid-test-data --out-dir D:\work\cad\pid-test-data\publish-v4`，提交 `7de1ef1`、工作树干净、发布构建，19 秒。53 张写出、0 张失败、2 张没有 `DrawingNo`（`D03`、`D04`）；`_Index.json` 的 `problems` 0 条；stderr 一行，没有断链告警。目录里另有 `BUILD.txt`、`RUN-LOG.txt` 与三个子目录：

- `vs-publish-v3\`：160 个文件里 158 个与 `publish-v3` 逐字节相同（53 个 `_Meta.xml`、53 个 `_Coverage.json`、`_Index.json`、51 个 `_Data.xml`）；2 个 `_Data.xml` 一共少 20 行空的 `<IOffpageConnector/>`（轻烃分离器I 14、低压气压缩机入口改造 6），就是 S3b（`dc78a7c`）。`main` 并进来的提交与分支上其余 15 个都不改变这个包写出的字节。
- `vs-reference\`：整包门禁对 `验收数据` 是 7 张 clean、4 张另一版、42 张没有参考；11 张图各两种单图模式的输出；完整度的表。7 张同版图在交付目录里的文件：对象 18 197 / 18 197、关系 24 143 / 24 143、写得不同的值 0、接口元素 76 646 / 134 983（56.8 %）、非空值 19 734 / 39 171（50.4 %）、空值属性 0 / 16 680——就是钉值。这 7 张单图 `--oracle-exp` 的 `_Data.xml` 与 `_Meta.xml` 和交付文件逐字节相同。
- `vs-database\`：下面那项普查的输出。

同一命令再跑一次 160 / 160 相同；调试构建跑同一个包 160 / 160 相同。交付件里每条关系的两端都是同一个文件里的对象（0 条悬空，11 份原厂参考也是 0），没有重复的 UID。53 个 `_Data.xml` 的 `<Container>` 各带 `LoginUser`、`LoginPWD` 两个属性，值全部为空，原厂参考也是空的。

**对库普查。** 46 张没有同版参考的图靠什么保证，此前只有「写出来、不失败、自洽、可重复」。这一轮拿包里的数据库对了表示这一层（`3411263`，探针 `examples/probe_which_database_representations_a_publish_leaves_out.rs`；分析页 `docs/analysis/2026-11-12-what-the-plant-database-draws-that-a-publish-leaves-out.md`）：51 张有内容的图上库里画着 42 048 条表示，写出 23 762 条，没有一条是库里没有的；没写出的 18 286 条里 18 263 条属于原厂自己也不发的三类（标签、管段上的符号、信号线上的符号）；**剩下 23 条是库里画着而 `.pid` 没画的非管件对象，现在不发**（C井场 21、Q井场 2）。记为 S17，等 C井场 的原厂发布件；这一轮没有改发布规则。`D03`、`D04` 在库里没有任何表示，所以那两个壳与库一致。

**这一阶段没做的。** S4、S5 及之后的切片（等 Q1–Q3）；不并 stash（Q2；并入 `main` 之后它要三方合并才套得上）；S17 的规则（没有样本）；发布时断链进 `_Index.json`、`/Unclustered Dynamic Attributes` 的告警措辞（工单 #49，解析线在做）；普查只到表示这一层，属性值与关系没有对库。没核实的：分支上 `624916d` 之后的提交仍然没有独立审查；`pid-plot` 的页面门禁在合并树上没有跑（理由见上）。

**一处环境上的坑。** `cargo fmt --all -- --check` 把一个包的全部 target 根文件以 `\\?\` 开头的绝对路径放在一条命令行上交给 rustfmt。根包现在有 355 个根文件，在这个 worktree 里路径合计 31 006 个字符，加上前缀与程序名就到了 Windows 的 32 767 上限：合并提交上它还过，加了一个示例之后报 `os error 206`（文件名或扩展名太长），rustfmt 一个文件都没读。主检出的路径短 19 个字符，同一条命令行短约 6 700 个字符（355 × 19），那里没有试。在这个 worktree 里 fmt 这道门改成对同一批文件分批跑 `rustfmt --check --edition <版本>`：根包与 `vendor/oxidized-mdf` 是 2021，`pid-plot` 是 2024，共 449 个根文件，0 处差异。`vendor/pdfcraft-pin` 的四个第三方 crate 不在其内：对它们直接跑 rustfmt 有 64 处差异，而 `cargo fmt --all -- --check` 在合并提交上是过的，所以它没有读它们。

### 第五阶段（2026-11-12）：GitHub #70，入库的 DWG 包整包发布

`happyrust/pid-parse` #70「入库的 `DWG-0202GP06-01_p` 整包发布以 `T_PIPERUN … is not an integer` 退出」是解析线在 `main` 上做 #49 时看到的。**它就是第一阶段 S1 修掉的那一处。** `main`（`7f71586`）的 `ExpTables::read` 对 `T_PIPINGPOINT`、`T_SIGNALPOINT`、`T_PIPERUN`、`T_SIGNALRUN`、`CODELISTS` 五张表用严格读法，遇到第一个不是整数的 `NUMBER` 就整表拒绝；本分支在 `80a5f73` 把这五张表改成逐格读，在 `51521ef` 补上「阶段跟的列里有格子没读懂就报错」。分支没有合回 `main`，所以工单在 `main` 上成立，在本分支上从 2026-11-10 起不成立。这一阶段没有改发布的代码：复核，把工单问的几件事答清楚，补两条库内用例，收紧整包那条用例。

**复现。**

| 程序 | 对入库包 `test-file/backup-test/DWG-0202GP06-01_p` 整包发布 |
|---|---|
| `main` `7f71586`（把那棵树另放一处编的；`src/**`、`Cargo.toml`、`Cargo.lock` 逐个对过 blob） | 退出 1，`error: read …\Export.dmp: T_PIPERUN: row 0 NUMBER at 0x7a4c07 is not an integer`，不建输出目录 |
| 本分支 | 退出 0，7 张写出、0 张失败、1 张没有 `DrawingNo`；`_Index.json` 的 `problems` 0 条；stderr 只有汇总那一行 |

**是哪一格。** 用另写的读法把整份 dump 读了一遍（152 张表；另有 2 张带大对象列的框不出来，发布都不读）。不是整数的 `NUMBER` 一共 780 格，分在 27 个（表, 列）里，绝大多数是图形坐标、角度与比例，都在发布不读的表里。发布读的 15 张表里只有 `T_PIPERUN` 有，两列共 10 格：

| 列 | 格数 | 第一格 | 字节 | 值 | 同一行带单位的文字列 |
|---|---:|---|---|---|---|
| `SP_INSULTHICKSI` | 8 | 第 0 行，载荷在 `0x7a4c07`（前两个字节是长度 `02 00`） | `c0 06` | 0.05 | `INSULTHICK` = `50mm`（两行写作 `50 mm`） |
| `SP_SLOPESI` | 2 | 第 21 行，载荷在 `0x7a82e4` | `bf 1e 64 64 0b 01 05 57` | 0.0029999910000486 | `SLOPE` = `0.3 %` |

第 0 行是管段 `7042E6B0…`（序号 `01`、等级 `1.6AR12`、位号 `OD-50 mm-03-0401-1.6AR12-WE50mm`），`DWG-0202GP06-01` 上发成管线的六条之一。

**含义能确定。** dump 自己的数据字典（`ATTRIBUTES` / `UNIQUEATTS`）把这两个属性叫作 `SP_InsulThickSI`「Insulation Thickness SI Value」与 `SP_SlopeSI`「Slope SI Value」：类型 `D`（双精度），`ATTRIBUTE_UOM` 指回带单位的那个文字属性（`InsulThick`、`Slope`），不显示、只读。字典里 244 个 `SP_…SI` 属性全部是 `D`，`T_PIPERUN` 一张表有 104 列。也就是带单位的文字属性旁边另存一份换成 SI 单位的数：0.05 是 50 mm 的米数，0.0029999910000486 是 0.3 % 坡度的反正切（`atan(0.003)` = 0.0029999910000485996 弧度）。参考发布件里 `SlopedPipingAngle "2.9999910000486E-03 rad"` 就是后一个数。小数是这类列的正常内容；严格读法的错是把没人跟的列也要求成整数。

**怎么处理的（已在分支上）。** 逐格读，没人跟的列不判，这 10 格留作没读，表照读。把这两个值发出来是 S9，没有动。

**「一格没读懂就拒收整列」适用不适用。** 不适用。那条规则只管阶段跟的列；`T_PIPERUN` 被跟的是 7 列，这份 dump 里被跟的列没有一格没读懂。15 张表里留作没读的一共 5 列：上面两列，加三个数据库字符集的文字列（`T_REPRESENTATION.FILENAME`、`T_OPC.MATINGOPCPATH`、`T_NOTE.BODY`），没有一列是阶段跟的。53 张那个库的小数（94 658 格）落在发布读的表里的只有 `T_CONNECTOR.MINIMUMSEGMENTLENGTH` 与 `RANGECLEARANCE`，也没人跟。

但这条规则真适用的时候，后果与工单的口径不同，见 Q7。拿入库包做了试验（副本，换一份只坏一处的 dump）：

| 弄坏的一处（都是阶段跟的列） | 整包发布 |
|---|---|
| `T_PIPERUN` 第 0 行 `NOMINALDIAMETER` 改成 0.3 | 退出 1，7 张图失败 6 张，`_Index.json` 每张一条 `T_PIPERUN: row 0 NOMINALDIAMETER is not an integer or NULL` |
| `T_PIPINGPOINT` 第 0 行 `NOMINALDIAMETER` 改成 0.3 | 同上，`T_PIPINGPOINT: row 0 NOMINALDIAMETER is not an integer or NULL` |
| `T_PIPERUN.TAGSEQUENCENO` 的列头改成数据库字符集 | 同上，`T_PIPERUN: column TAGSEQUENCENO is not AL16UTF16 text; its cells were not read` |

失败的 6 张里有 5 张与那条管段或那个端口无关（它们只画在 `DWG-0202GP06-01` 上）：阶段对整张表投影，一格拒收、整个阶段报错，凡是要挂库内容的图都过不去（没失败的那一张是没有 `DrawingNo`、只发一个壳的 `001`）。

**新加的用例**（`cdabe04`）。

- `src/publish/oracle_exp.rs` 两条单测，读入库的 dump，任何检出都真跑（这个模块原有的真 dump 单测读的都是仓库外的 53 张那个库，CI 上软跳过）：
  - `the_committed_dwg_dump_projects_through_every_stage`：`0x7a4c05` 起的四个字节是 `02 00 c0 06`；每个投影都读得过；7 张图、3 384 行代码表、93 个管道端口、151 个信号端口、50 条管段其中 20 条有序号；第 0 行那条管段被跟的每一格都读出来。
  - `the_committed_dwg_dump_keeps_unread_only_columns_no_stage_follows`：15 张表里留作没读的列逐个钉住（表、列、类型、第一行、格数），就是上面那 5 列。预期值先用另写的读法算出来，再写进断言，第一次运行即相符。
- `tests/publish_from_pid.rs::the_dwg_package_publishes_every_drawing_it_lists` 收紧：`_Index.json` 的 `problems` 与每张图的 `problems` 都是空的；stderr 没有 `warning:` / `error:` 行；逐图钉对象、值、关系的个数；钉全包的合计。原来只断言退出码、文件在不在与 `DWG-0202GP06-01` 的 4 个 `PIDBranchPoint`。

**红绿。** 把 `ExpTables::read` 那五个调用换回 `main` 的写法（测试不动）：这个夹具的 8 条用例（两条新单测，`publish_from_pid` 3 条，`publish_completeness` 3 条）红 6 条——两条新单测、`publish_from_pid` 的 `--oracle-exp` 与整包两条、`publish_completeness` 的带 dump 与整包两条——报的都是 `T_PIPERUN: row 0 NUMBER at 0x7a4c07 is not an integer`；不读 dump 的 2 条（`.pid` 单发）照过。换回来（文件逐字节还原、修改时间更新到当时）之后 8 条全过。

**这个包现在导出什么。** 7 张图各三件加 `_Index.json`，22 个文件；合计 786 个对象、643 个值、1 070 条关系（不含 `001` 那个壳），93 个管道端口、151 个信号端口（dump 里的端口行一行不少）、192 条连通关系、5 个 OPC、12 条管线、20 条 `PipingConnectors`。

| 图 | 对象 | 值 | 关系 | 说明 |
|---|---:|---:|---:|---|
| `001` | 1 | 1 | 0 | 没有 `P&IDAttributes` 行，只发图纸对象；库里这张图上也没有任何表示 |
| `002`、`222` | 3 | 2 | 2 | 各一台设备（`PIDProcessEquipment`）与它的表示 |
| `DWG-0104GP01-08` | 5 | 6 | 4 | 一条管段、它的工艺点与 1 条管线 |
| `DWG-0201GP01-01` | 492 | 362 | 688 | 79 个管道端口、103 个信号端口、137 条连通关系；27 条管段没有序号，不成管线 |
| `DWG-0201GP06-01` | 170 | 150 | 233 | 5 条管线、13 条 `PipingConnectors` |
| `DWG-0202GP06-01` | 113 | 121 | 141 | 6 条管线；包里的 `.pid` 与库比参考新，对参考的读数见第二阶段的表 |

不靠参考的核对（另写的脚本，与入库的探针 `probe_which_database_representations_a_publish_leaves_out` 两条路，数字相同）：22 个文件里没有重复的 UID，每条关系的两端都是同一个文件里的对象；库里画在这 7 张图上的 392 条表示，写出 276 条，没写的 116 条属于原厂也不发的三类（标签 64、管段上的符号 50、信号线上的符号 2），没有一条写出的表示是库里没有的，也没有「同类别处都发、这里漏了」的行；另有 3 条 OPC 在图的 stockpile 里（`DWG-0201GP06-01` 2 条、`DWG-0202GP06-01` 1 条），不发，是 S12。

**其它输出不变。** 发布的代码这一阶段没有动（`src/` 的改动全在 `oracle_exp.rs` 的 `mod tests` 里）。用尖端的调试构建重跑：53 张整包 160 个文件与 `publish-v4` 逐字节相同；对照矩阵 30 次运行 253 个文件与上一阶段的输出逐字节相同，退出码与标准输出也相同；入库包的 22 个文件与上一阶段发布构建的输出相同。

**门禁。** 工具链 stable 1.99.1，`--locked --offline`，`CARGO_INCREMENTAL=0`。从这一阶段起环境里不再设 `RUSTFLAGS`（它会让全局配置里的 rustflags 整个失效、把所有 crate 另编一套）：告警由 clippy 的 `-- -D warnings` 与「编译日志里 `^warning` 的行数为 0」守，共用的 sccache 开着。读数：`rustfmt --check` 449 个根文件 0 处差异；clippy 通过（第一遍在新单测上报了两处 `unreadable_literal`，偏移要写成 `0x007a_4c05`，改了再过）；根 crate 全部测试 1 898 过 / 0 败 / 3 ignored（库 1 425，47 个集成测试文件 463，300 个示例按测试编译其中 10 条）；16 个 `publish_*` 文件逐个带 `--nocapture` 是 121 过 / 2 ignored，skip 行 21 行，与前几个阶段相同，跑完临时目录里不留东西；`parser_panic_safety` 5 过；`--no-default-features --lib` 928 过；wasm32 通过；`missing documentation` 0 = 基线 0，rustdoc 退出 0；每一步的编译日志都没有告警行。A 部分（fmt、测试目标上的 clippy、库的单测、`publish_*`、精简特性、wasm32、缺文档）对着暂存的改动跑，提交后 `cdabe04` 的 diff 与过门的那一份指纹相同。

根 crate 的测试与 clippy 是**分片**跑的——库的单测、47 个集成测试文件各一条命令、全部示例一条命令，每条 2 个 job——不是一条 `--all-targets`：这段时间机器的内存提交余量一直在 1.5–12.9 GB，到不了大构建要的 16 GB，而且多数时候机器上一个编译器都没有在跑。分片合起来就是 `cargo test -p pid-parse --all-targets` 与 `cargo clippy -p pid-parse --all-targets` 的全部目标。`cargo build --workspace --all-targets` 没有重跑：它是大构建，这一阶段的改动又只在根 crate 的测试里，`pid-plot` 不编它们。

**这一阶段没做的。** Q7（没有改拒收的粒度）；把 `SP_INSULTHICKSI` / `SP_SLOPESI` 的值发出来（S9；读法要先学会读双精度，那两条单测钉的「留作没读的列」届时跟着改）；workspace 构建。工单在 `main` 上要等这条分支合回去才不成立。

### 第六阶段（2026-11-15）：S4、S5、S7、S8、S9、S11 进 `main`，重发 `publish-v6`

Q1、Q3、Q5 按推荐的 (a) 做；Q2 的 stash 里的 publish 改动由 `c0f191c` 落到 `main`；Q4、Q6 等现场数据（GitHub #71）；Q7 维持 (a)。这一阶段在 `main` 上直接并入，每片一个合并提交（括号里是被并入的提交）：

| 片 | 提交 | 做了什么 |
|---|---|---|
| — | `1b5e443`（`d10c529`） | 本计划的分支（第一至第五阶段）并入 `main` |
| S4 | `c0f191c`、`81920d4` | 门禁看接口元素、`<Container>` 与重复 UID（stash 里的那份）；A01 那次运行钉住这三项 |
| S11 | `8d7cfe7`（`c51b750`） | 数据库线的命令行门禁改走 `diff_pid_export` |
| — | `18d3f90`（`f15ece6`） | exp 的 NUMBER 列读法（S9 要读的双精度） |
| S5 | `aa3fa99`（`5db7684`） | 类的固定接口表 |
| S9 | `9f94aa3`（`46ecb18`） | 带单位的值 |
| S8 | `11487b3`（`331b664`） | 代码表的枚举值，不分层级的那部分 |
| S7 | `1317c0b`（`73cf1b2`） | `RepresentationsAreAllZeroLength`、`PipingMaterialsClass`、`InsulSpec`、`PipingConnectorSeqNo`、`NominalDiameter` |
| S7 | `2681ccd`（`858fe49`） | `IsTypical`、`IsFlowDirectional` |

**读数。** 7 张同版图、整包、带 dump（`publish-v6`，探针是 `2681ccd` 的发布构建）：对象 18 197 / 18 197、关系 24 143 / 24 143、接口元素 134 827 / 134 983（99.9 %）、非空值 35 519 / 39 171（90.7 %）、空值属性 15 643 / 16 680，写得与参考不同的值 0，参考对象没有的接口元素 0。`tests/publish_completeness.rs` 的钉与这些数相同。与 `publish-v5`（`aa3fa99`，只有 S5：接口元素 133 445、非空值 19 734）比，§3 里 S7 那一批的 7 667 个写出 6 567 个，S8 的 11 610 个写出 9 218 个，合起来正好是增加的 15 785 个；S9 的属性在这 7 张参考里没有（样本只在 DWG 与 A01）。

**还没写出的 3 652 个非空值**（数据地图探针 `--ours publish-v6` 逐行列来源，合计对得上）：

- S8 的层级枚举 2 392：`PipingComponentType1–3` 2 021、`EqType0–5` 190、`ProcessEqCompType1` 45、`InstrumentType` 136。其中 1 511 个（`PipingComponentType2/3`、`EqType1–5`、`InstrumentType`）映射里指不到来源，就是第三阶段「层级」一条说的 schema 一侧的层级。
- S7 的 `IObject.Name` / `IObject.Description` 1 100：名称是管段 631、管件 395、在线仪表 43、仪表 8，描述是设备 17、机械设备 6。
- §3 S8 一行列的 160 个零散值，一个没动：仪表位号 102、`LongMaterialDescription3` 34、`EquipPrefix` 17、`ReliefPressure` 6、`EquipmentTrimSpec` 1。

前两类做完就是 §3 末尾算的 39 011 / 39 171。还没写出的 156 个接口元素：105 个由设备类型码决定（泵、电机、低压罐、空冷器几类，跟着 `EqType` 那条链），51 个是只在有值时才写的 `ISpecifiedMatlItem` 34 与 `INamedEquipment` 17，跟着上面的 `LongMaterialDescription3` 与 `EquipPrefix`。

**`publish-v6`。** `D:\work\cad\pid-test-data\publish-v6`，`2681ccd` 的发布构建：53 张、0 失败，同一命令重跑 160 / 160 逐字节相同。与 `publish-v5` 比 57 个文件相同，不同的是 51 个 `_Data.xml`、51 个 `_Coverage.json` 与 `_Index.json`；`_Meta.xml` 全同，D03、D04 没有 dump 段，两张不变。53 张的对象 80 181、关系 106 812 与 v5 相同，非空值 87 600 → 160 857。覆盖报告的 `oracle_exp` 段多一项 `placed_column_values`。整包门禁 7 张 clean、0 张有发现项、4 张另一版、42 张没有参考。细节在交付目录的 `BUILD.txt`。

**门禁（S7 的后两个属性）。** `858fe49` 在游离的工作树里过 AGENTS.md 的全套：build、test（2 518 过 / 0 败 / 3 ignored，skip 提示 21 条，与 `1317c0b` 相同）、`clippy -D warnings`、fmt、`missing documentation` 0 = 基线 0、字节审计基线、`--no-default-features --lib` 985 过、wasm32。`main` 自分支起没有动过，合并提交的树就是 `858fe49` 的树。

**主检出的 stash（Q2）。** 逐块比过：116 块里 81 块已在 `main`，30 块 `main` 改成了别的写法。真正只在 stash 里的是三处：`attach_dump_only_components` 的去重（带一条测试）；端口投影跳过非端口行（`main` 保持报错，见 §2 Q2）；一行帮助文字。stash 存成本地分支 `publish-stash-20261108`（`9b4d60b`，要看用 `git stash apply publish-stash-20261108`）后删掉。

**这一阶段没做的。** S7 的 `IObject.Name` / `IObject.Description`；S8 的层级枚举（规则要从参考量，或读 schema / `EFAdapter.dll`）；160 个零散值；S10、S12、S17 等 Q4 / Q6 的数据（#71）；S13、S14；Q7 不改。`src/publish/mod.rs` 模块文档里的 Outstanding 一节还是 `048a842` 时的样子，没有跟着改。
