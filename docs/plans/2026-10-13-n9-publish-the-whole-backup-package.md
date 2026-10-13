# N9：整包发布 · 从备份包目录一次发出 53 张图，管线按 PipeLinePublisher 的口径合成（2026-10-13）

> 承接 N5（`docs/plans/2026-10-12-n5-ports-from-oracle-exp.md`，N5-1 / N5-2 已交付）、N7（`c1ad13e` 连通）、N8（`353186a` 管口 / 注释 / OPC）与 N6 分析页（`docs/analysis/n6-pipeline-rows.md`：这份 dump 里没有管线行）。
> 用户 10-13 14:03「继续制定解析 `D:\work\cad\pid-test-data` 这里的备份数据包的功能。使用 plannator 制定下一步的开发方案」。开单只写本文件，**没改任何代码**，没提交。
> 基线：pid-parse `353186a`（`codex/phase32c-bundle-closeout`）。工作树上别的会话的未提交文件（pid-plot、`src/backup/folder.rs`、dash-scale / line2 探针、`n6-pipeline-rows.md` 全表搜一节等）一字不碰。
> 编写：zhimo 会话 fable-5-1-19。§1 的数字是 10-13 14:10–14:45 用临时 Python 脚本和 `D:\Rust\target\debug\pid_publish_xml.exe`（10-13 11:50 构建，= `353186a`）在备份包、5 份参考 `_Data.xml` 上实测的；脚本不入库。

## 一句话

备份包 = `Manifest.txt` + `Export.dmp`（Oracle exp，4 个 schema）+ `PlantData~2~711.zip`（53 张 `.pid`）+ 10 个 `RefData`（`RefData~4~804.zip` 里有 `SPPIDDataMap.xml`——SPPID → SPF 的发布映射）+ `PlantConfig.xml`。今天的发布器一次只吃一张 `.pid` 加 `--oracle-exp`，每次把 dump 的表重读一遍；I井场对象 3 014 / 关系 4 001，差参考的只剩 79 个 `PIDPipeline` 和 106 条 `PipingConnectors`。
这 79 个 UID 不在 dump（N6），本轮再测也不在 `.pid` 里，也不是名字的哈希；`SPPIDDataMap.xml` 写明管线是发布时由 `EFAdapter.PipeLinePublisher` 从管段合成的。本轮三件事：**N9-1** dump 的表读一次、挂多张图；**N9-2** 按 PipeLinePublisher 的口径从 `T_PIPERUN` + `CODELISTS` + `SPPIDDataMap.xml` 合成管线（名字规则 106/106 对上，UID 只能自造，对照改按名字对）；**N9-3** 给发布器一个「目录输入 = 备份包」的入口，一次发 53 张图并写 `_Index.json`。做完 I井场对象 3 093 / 关系 4 107，与参考同数。

## 1. 现状

### 1.1 备份包里有什么，今天读到哪一层

| 成员 | 大小 | 内容（实测） | 今天 |
|---|---:|---|---|
| `Manifest.txt` | 34 730 | BackupType 2、Version 7.02、DateCreated 2026-07-19 20:13、Plant `SQPlant`、`DBUids` 4 个 schema、`File` 行逐个列出 PlantData / RefData | `backup::manifest` 已解析 |
| `Export.dmp` | 74 489 856 | Oracle 12.1 conventional exp（`EXPORT:V12.01`）；schema SQPLANTPIDD / SQPLANTD / SQPLANT / SQPLANTPID；154 个 `INSERT`、126 张表 | `publish::oracle_exp` 读 `APPLICATIONS`、`T_PIPINGPOINT`、`T_SIGNALPOINT`、`T_CONNECTOR`、`T_REPRESENTATION`、`T_PIPERUN`、`T_SIGNALRUN`、`T_OPC`、`T_NOZZLE`、`T_NOTE`、`T_ITEMNOTE`、`T_MODELITEM`；**每张图每个入口各读一遍**（`T_PIPINGPOINT` / `T_SIGNALPOINT` / `T_OPC` 各两遍、`T_REPRESENTATION` 42 540 行两遍、`APPLICATIONS` 四遍） |
| `PlantData~2~711.zip` | 21 412 367 | 53 张 `.pid`：`00/00/` 43、`00/02/` 4、`test/U01/` 6（D01 / D03 / D04 / D06 / D07 / TESTDWG-0002TP03-01，与仓库 `test-file/` 的 fixture 同名）；另有 `SmartPlant Resources/SPEMDataMap.xml`。条目名是 **GBK、没有 UTF-8 标志位**，`zip` crate 的 `name()` 会读成 cp437 乱码 | `zip_index` 只列中央目录，不解压 |
| `RefData~4~804.zip` | 4 357 748 | `SPPIDDataMap.xml` 3 678 496 字节 + 6 份带日期的旧版 + `CatalogIndex.mdb` | 文件名 / magic 分类，没读内容 |
| 其余 9 个 `RefData` | — | 681 符号库（Manifest：645 个文件 / 97 个目录）、682 模板、684 报表、685 装配、809 导入映射、680 `rules.rul`、683 `ProjectStyles.spp`、703 `exportlayer.xlsx`、709 `InsulationSpec.isl` | 同上 |
| `PlantConfig.xml` | 5 085 | 图纸样式配置 | 没读 |

`T_DRAWING` 53 行 = 53 张 `.pid`：`SP_ID` 就是 `DocUID`，`NAME` 就是图名（I井场行 `NAME` = `I井场 注采阀组工艺及自控流程图`），`PATH` 就是 zip 里的路径（`\00\00\I井场 ….pid`），另有 `DRAWINGNUMBER`（`DWG-0000集05-08`）、`TEMPLATE`、`SP_PLANTGROUPID` → `T_PLANTGROUP` 6 行给出 `SQPlant \ 00 \ 00`、`\ 00 \ 02`、`\ test \ U01` 三层。今天 `.pid` 模式的图名取自**文件名**（`pid_load.rs:916 file_stem`），从 zip 里解出来跑时图名会带上路径（§1.3 实测 `00__00__I井场 …`，`IObject.Name` 与参考差 1）。

另外 4 张集注站参考图在 zip 里都有同名 `.pid`，但不是同一版：一级闪蒸分离器 B 和低温分离器 A 大小相同、SHA256 不同；一级闪蒸分离器 A（1 118 208 对 1 146 880）和换热器 CD（1 486 848 对 1 699 840）大小都不同。I井场的 `.pid` 与参考旁那份 SHA256 相同。N5 §1.3「4 张图的 DocUID 和父项不在 dump」仍成立：它们的参考是别的库发布的，这份备份里的是另一套 UID。

### 1.2 I井场还差的 79 + 106：UID 没处找，名字和归属都能算

- **UID 不在任何地方**。N6 已测 dump 全部 264 个标识列和整份字节 0 命中。本轮测 I井场 `.pid`：79 个 UID 按 ASCII、小写、UTF-16LE / BE、带连字符、原始 16 字节（直排 / 混合端）都是 0（对照组：`DocUID` ASCII 5 028 处、任一连接器 UID 10 处）。不是名字的 MD5 / SHA-1 / SHA-256（UTF-8 / UTF-16 / GBK、大小写都试）。79 个全是 v4 随机 GUID（版本位 79/79 = 4）。
- **UID 是整厂按名字持有的**。集注站 4 份参考里跨图重名的管线 2 个，2/2 两张图 UID 相同 → 适配器在 SPF 侧维护「名字 → UID」，备份包里没有这张表，所以我们只能自造，且**同厂同名必须同 UID**。
- **`SPPIDDataMap.xml` 写明了口径**：`SPMapClassDef SPMC_20 "Pipeline"`，`ProcessClassCriteria="EFAdapter.PipeLinePublisher"`，`MapClassToClass → PIDPipeline`；属性 `Name`、`FluidSystem`（枚举表 `SP_20`，`MapEnumToEnum` 1 280 对，`SP_20_5 → {D8598A99-A0B6-4216-B8DD-1E60262E91EE}`）、`OperFluidCode`（`SP_19`）。n2a 表里端口流向的 GUID（`23F94146` 等）不在这份映射里，仍照 n2a 的表写。
- **名字规则，实测 106/106**：`{公称直径}-{流体代码}-{TAGSEQUENCENO}-{PIPINGMATERIALSCLASS}[-{INSULATIONSPEC}]`，全部取自 `T_PIPERUN` 同一行（连接器 UID = `SP_ID`）。公称直径码 < 5000 → `{码} mm`；≥ 5000 → `(码 − 5000) / 32` 英寸写成**带分数 + 英寸号**（5032 → `1"`，5024 → `3/4"`，5048 → `1-1/2"`；N5 端口的 `1 in` 写法在这里不适用，用它只对 105/106）。流体代码 = `CODELISTS`（SQPLANTPIDD）19 号表该码的 `CODELIST_SHORT_TEXT`（本图 CI / PG / HFL / WO / PC 五个码）。空列留空，所以有 `-PG-1001105-F9` 这种名字。
- **哪些管段成管线**：本图 169 个 `PIDPipingConnector`，`TAGSEQUENCENO` 非空 106 个，正好是参考 106 条 `PipingConnectors` 的 `UID1`；空的 63 个参考里也没有关系（63/63）。同名的管段并成一个管线：106 → 79，与参考 79 个同数，一个管线下的管段名字没有不一致的。
- **FluidSystem**：管段 `FLUIDSYSTEM` 码 → `SP_20_{码}` → GUID。79 个管线按码分：5 → 45、7 → 23、14 → 5、8 → 4、13 → 2。参考 56 个带 `FluidSystem` 的（码 5 / 8 / 13 / 14）56/56 对上；参考为空的 23 个全是码 7。码 7 的 `{A729B087-24B7-4418-A185-53CAA90C2286}` 在 7 份 `SPPIDDataMap` 版本里都有，在集注站参考里出现 4 次 → 不是映射缺，是 06-22 发布之后数据改过（dump 是 07-19 的）。按 dump 发会比参考多 23 个值（§3 Q2）。
- 流体代码本身（`OperFluidCode`）参考 `PIDPipeline` 上没写，只进名字；本轮也不写。

### 1.3 整包跑一遍：53 张 `.pid` 各带 `--oracle-exp`

把 zip 按原路径解到 `%TEMP%`，用 11:50 的 debug 二进制逐张跑 `--oracle-exp Export.dmp --out-dir --plant P01`（不带 `--diff-against`）。共 431 s，每张 8.3–8.8 s；同一张 I井场不带 `--oracle-exp` 是 0.1 s，差的都是重读 dump 的 12 张表。

- **51 张退出 0；2 张退出 1**：`test/U01/D03.pid`、`D04.pid`，`no P&IDAttributes row carries a DrawingNo`（`.pid` 里没有一行带图号；`T_DRAWING` 有它们的行，`PATH` 对得上）。
- 51 张的 `DocUID` 全在 `T_DRAWING`（51/51），按原路径解出的文件名 = `T_DRAWING.NAME`（51/51）；51 张 exp 挂接都有命中。
- 合计：对象 77 690、关系 103 577；管道端口 9 557（dump 全表 9 628）、信号端口 22 021（22 057）、连通关系 19 371、OPC 分支点 476、设备↔管口 372（全库 377 对）、注释 114（114 对）；`.pid` 命名的物项 14 894、发出 14 418，`left_out` 476 条（全是 OPC 行上的物项，与 476 个 OPC 分支点同数）。
- 井场 13 张：对象 2 570（A）– 5 543（EF）；I井场 3 014 / 4 001，与 `publish\` 现有三件同数。集注站 34 张：对象 453–1 306；7 张「进出站收发球单元」计数完全相同（496 / 666 / 56 / 156），是同一模板的复制。`test/U01` 成功的 4 张：D01 380 / 411、D06 69 / 81、D07 156 / 155、TESTDWG-0002TP03-01 216 / 307。
- 临时输出只在 `%TEMP%`，`publish\` 没动。

### 1.4 别的会话留在工作树里、本轮要用的东西

- `src/backup/folder.rs`（`scan_backup_folder`：一次清点 `Manifest.txt`、`Export.dmp` 前 4 KiB 嗅探 MTF / Oracle exp、`RefData`、`PlantData` 及其 ZIP 中央目录）+ `tests/backup_folder_real_dir.rs`（TEST02_p 实目录门，缺 fixture 则 soft-skip）+ `Cargo.toml` 一条 `[[test]]` + `src/backup/mod.rs` / `CONTEXT.md` / `docs/current-architecture-principles.md` / `docs/agents/domain.md` 的文字。未提交。Q6 已批 (a)：由它自己的会话提交，N9-3 在那之后开工（§3.1）。N9-3 建在它上面。
- `examples/probe_oracle_exp_pipeline_scan.rs` + `n6-pipeline-rows.md`「全表搜」一节（t-23）。只读探针，N9 不依赖。

## 2. 范围

**做**：N9-1 dump 的表读一次、挂多张图（行为不变）；N9-2 合成 `PIDPipeline` + `PipingConnectors`，`--diff-against` 对管线改按名字对；N9-3 `pid_publish_xml <备份包目录> --out-dir` 一次发 53 张 + `_Index.json`；按 §3 重写交付目录。
**不做**：`_Meta.xml` 的 `DocumentVersion` / `DocumentRevision` / `File` 三件（参考里它们的 UID 也是适配器造的，§6）；符号库、模板、`PlantConfig.xml`、`CatalogIndex.mdb` 的内容解析；SQL Server（MTF）变体的整包入口（TEST02 仍走 `pid_backup_extract` → MDF）；`imp`、连 Oracle；`.pid` 模式（不给 `--oracle-exp`、不给目录）的输出一个字节都不变；`SPEMDataMap.xml`；不 push（等你说）。

## 3. 要你拍板的

| # | 问题 | 选项 | 推荐 |
|---|---|---|---|
| Q1 | 管线 UID 怎么造 | (a) UUID v5（命名空间 = 固定的 pid-parse 管线命名空间 + `Plant` 值，名 = 管线 `Name`），写成 SPPID 式 32 位大写十六进制；同厂同名同 UID，跨图一致，重跑一致；(b) 不发管线，维持 79 / 106 的差 | **(a)**：参考就是整厂按名持 UID；(b) 等于放弃最后 185 条 |
| Q2 | 管线 `FluidSystem` 码 7 的 23 个（参考为空） | (a) 照 dump 发，`_Coverage.json` 和 `--diff-against` 单列「参考为空、dump 有值」的数；(b) 码 7 不发 | **(a)**：值来自库备份，参考为空是时间差；(b) 是拿答案改数据 |
| Q3 | 对照怎么对管线 | (a) `--diff-against` 里 `PIDPipeline` 按 `Name` 配对，`PipingConnectors` 按（连接器 UID，管线 `Name`）配对，UID 不同不计错、单独报「按名配对 N」；其余类不变；(b) 仍按 UID，接受 79 + 106 条「不在参考里」 | **(a)**：UID 自造是 Q1 定下的设计，不该每次都算错 |
| Q4 | 整包入口与图名 | (a) `pid_publish_xml <目录> --out-dir DIR [--plant]`：目录里有 `Manifest.txt` 即备份包模式；`Export.dmp` 是 Oracle exp 就全程 exp 模式，是 MTF 就报错退 2；图名取 `T_DRAWING.NAME`（按 `DocUID`），没有行时取 zip 条目名（GBK 解码）；(b) 新二进制 `pid_backup_publish`，图名只取 zip 条目名 | **(a)**：一个入口三种输入；图名以库为准，不再依赖文件名 |
| Q5 | 交付目录 | (a) 53 张平铺写到 `D:\work\cad\pid-test-data\publish\`（备份包旁，含 `_Index.json`）；`D:\work\cad\PID自研软件验证\publish\` 只按 N9-2 重写 I井场三件，另外 4 张不动；(b) 全部写到 `PID自研软件验证\publish-package\` | **(a)**：整包的输出跟整包放一起；参考对照目录保持 5 张 |
| Q6 | `src/backup/folder.rs` 那组未提交文件 | (a) 由它的会话提交，N9-3 在那之后开工；(b) 那个会话已经没了，由 N9 核对后原样提交（只加 `git add` 它那 8 个路径） | 你定。它的内容我读过，与 N9-3 需要的一致 |
| Q7 | 另外 4 张集注站参考图 | (a) 维持 N5 Q5：拿到集注站那套库的备份再补；(b) 这份备份里同名 `.pid` 是另一版、另一套 UID，N9-3 照常发出，但不与参考对照 | 只是告知：N9-3 会按 (b) 发出，不影响本轮 |

### 3.1 批复（2026-10-13，用户；grok-4.7-70 记入）

用户原话：「Q1 到 Q7 按推荐项批，仍先别写代码」。本轮只把这句话落进计划，**不写实现、不提交**。有推荐项的按推荐项落；Q6 的推荐栏是「你定」，没有可批的那一格，不替用户选边。

| # | 批复 | 落点 |
|---|---|---|
| Q1 | 批 **(a)** | UUID v5（命名空间 = 固定的 pid-parse 管线命名空间 + `Plant`，名 = 管线 `Name`），写成 SPPID 式 32 位大写十六进制；同厂同名同 UID，跨图一致，重跑一致 |
| Q2 | 批 **(a)** | 码 7 的 23 个 `FluidSystem` 照 dump 发；`_Coverage.json` 和 `--diff-against` 单列「参考为空、dump 有值」 |
| Q3 | 批 **(a)** | `PIDPipeline` 按 `Name` 配对，`PipingConnectors` 按（连接器 UID，管线 `Name`）配对；UID 不同不计错，单独报「按名配对 N」；其余类不变 |
| Q4 | 批 **(a)** | 入口仍是 `pid_publish_xml <目录> --out-dir DIR [--plant]`；目录里有 `Manifest.txt` 即备份包模式；`Export.dmp` 是 MTF 就报错退 2；图名取 `T_DRAWING.NAME`（按 `DocUID`），没有行时取 zip 条目名（GBK 解码） |
| Q5 | 批 **(a)** | 53 张平铺写到 `D:\work\cad\pid-test-data\publish\`（含 `_Index.json`）；`D:\work\cad\PID自研软件验证\publish\` 只按 N9-2 重写 I井场三件，另外 4 张不动 |
| Q6 | 补批 **(a)** | 等 `src/backup/folder.rs` 那组由它自己的会话提交；N9-3 在那次提交之后开工。本会话不代交那 8 个路径 |
| Q7 | 按计划告知的 **(b)** 收下 | N9-3 照常发出备份包里那 4 张集注站图，不与参考对照；对照仍等集注站那套库的备份（选项 (a)） |

Q6 补批（2026-10-13，用户对 grok-4.7-68 选 **(a)**）：上一轮「按推荐项批」盖不住 Q6。本轮用户单独选 (a)，上表已改成这个落点。N9-1 / N9-2 仍按「仍先别写代码」未开工。

## 4. 步骤

一条一单，一个提交。按 N9-1 → N9-2 → N9-3 的顺序做；N9-3 等 Q6(a)：`src/backup/folder.rs` 那组由原会话提交之后再开工。

### N9-1 dump 的表读一次（行为不变）

- `src/publish/oracle_exp.rs`：新 `ExpTables`（或等价名）一次读入今天四个入口要的 12 张表，`read_port_tables` / `connectivity_ends_for_drawing` / `composition_relations` / `opc_points_for_drawing` 改为从 `&ExpTables` 取；原来吃 `dump: &[u8]` 的签名保留为薄封装（内部先建表再调），`pid_publish_xml` 单张模式不改调用。
- 顺手把 N9-2 要的 `CODELISTS`（SQPLANTPIDD 那份，`find_slice` 命中的第一个 `INSERT`）和 N9-3 要的 `T_DRAWING`（`SP_ID`、`NAME`、`PATH`、`DRAWINGNUMBER`、`SP_PLANTGROUPID`）读进同一个结构，但这单不用。`T_DRAWING` 有 `DATE` 列，走 `read_table_tolerant`。
- 单测：手造片段盖「同一份 `ExpTables` 挂两张图」；既有的 `oracle_exp` 单测不动。
- 验收：I井场 `--oracle-exp --diff-against` 仍 clean，`_Data.xml` / `_Meta.xml` 与 `publish\` 现有三件 SHA256 相同；`tests/publish_from_pid.rs` 六组钉数不动；全量门同 N5-1（`cargo test --locked --workspace --lib --bins --tests` 0 败；`--no-default-features` 0 败；clippy `-D warnings` 两种特性零告警；rustfmt；missing-docs 不升；wasm32 lib 过；`--all-targets` 被别的会话未提交 example 卡住就标 NOT RUN）。

### N9-2 合成管线与 `PipingConnectors`

- 装载：对本图已发出的 `PIDPipingConnector`，取 `T_PIPERUN` 同 `SP_ID` 行；`TAGSEQUENCENO` 为空的不成管线。名字按 §1.2 规则拼；英寸写法用 32 分之一的带分数（`1-1/2"`，XML 里是 `&quot;`）；流体代码查 `CODELISTS` 19 号表 `CODELIST_SHORT_TEXT`，查不到就不拼这段并计数（本图 0）。同名合并成一个 `PIDPipeline`，UID 按 Q1。
- `FluidSystem`：`RefData~4~804.zip` 里的 `SPPIDDataMap.xml`（按条目名精确取 `SPPIDDataMap.xml`，不取带日期的旧版）→ `MapEnumToEnum` 里 `SP_20_{码}` → `@{GUID}`；码为空或映射缺就不写并计数。`SPPIDDataMap.xml` 的读取放 `src/backup/`（新文件 `datamap.rs`，只解 `MapEnumToEnum` 三元组和 `SPMapEnumListDef` 的名字），`zip` crate 解压这一个条目。单张 `.pid` 模式也要能用：`pid_publish_xml` 新加 `--datamap <SPPIDDataMap.xml 或 RefData~4~804.zip>`，不给就不写 `FluidSystem`，`_Coverage.json` 说明。
- 写出：`PIDPipeline` 的接口按参考（`IObject` `IPBSItem` `IPlannedFacility` `IPBSItemCollection` `IPipeline` `IPipingConnectorComposition` `IFluidSystem` `INoteCollection` `IExpandableThing` `IPIDTypical`，`IFluidSystem` 有值才带属性）；关系 `PCN-{连接器}-{管线}`（`PipingConnectors`，UID1 = 连接器，UID2 = 管线）。出处 `PidValueSource::BackupRow { table: T_PIPERUN, column }`，`FluidSystem` 另记映射文件。
- `--diff-against` 按 Q3 改；`_Coverage.json` 加管线数、`PipingConnectors` 数、按名配对数、`FluidSystem` 多发数、没拼成的名字数；`left_out_by_design` 里「pipelines are still not stated」一句改写。
- 测试：手造片段盖名字规则（mm / 整英寸 / 分数英寸 / 带分数英寸 / 空列 / 带保温规格）、同名合并、UID 稳定；I井场 exp 组钉数改为对象 3 093、关系 4 107，`PipingConnectors` 106，按名配对 79 / 106，错类 / 错值 / 多发 0，`FluidSystem` 多发 23（dump / 参考 / `.pid` 缺一就 soft-skip）。
- 交付（按 Q5）：I井场三件重写到 `PID自研软件验证\publish\`。
- 文档：`n6-pipeline-rows.md` 加「合成」一节（规则、数字、UID 口径）；`CHANGELOG.md`；`task_plan.md`；本计划 §7；#29 加一条评论。
- 验收：上面的数字；全量门同 N9-1。

### N9-3 目录入口：一次发 53 张

- `pid_publish_xml <目录>`（Q4）：`scan_backup_folder` 清点；`Export.dmp` 是 `OracleExp` 才继续（MTF 退 2 并提示走 `pid_backup_extract`）；读一次 `ExpTables`（N9-1）与 `SPPIDDataMap.xml`（N9-2）；逐个 `PlantData~*.zip` 里的 `*.pid` 条目解压到内存，`load_publish_from_pid` 需要一个吃 `&[u8]` / `Read + Seek` 的入口（`Records::read` 现在吃路径；`cfb` 本就支持 `Cursor`）。图名按 Q4。`.pid` 里没有 `DrawingNo` 的（§1.3 的 D03 / D04，空图）：`DocUID` 按 zip 路径 ↔ `T_DRAWING.PATH` 取，仍写出只有 `PIDDrawing` 的 `_Data.xml`，`_Index.json` 记「uid_from: T_DRAWING.PATH」；两边都对不上才算失败。
- 输出平铺到 `--out-dir`（Q5），三件同今天；另写 `_Index.json`：每张图的 `DocUID`、图名、zip 内路径、`T_DRAWING` 的 `DRAWINGNUMBER` / `PATH` / 层级路径、对象 / 值 / 关系数、`oracle_exp` 块、`problems`；包级计数（zip 里 `.pid` 数、`T_DRAWING` 行数、两边对上的数、失败数）。`--diff-against` 在目录模式下不认（退 2）。
- 一张图失败不中断整包：记进 `_Index.json` 的 `problems`，最后退 1；全部成功退 0。
- zip 条目名：`zip` crate 用 `name_raw()` 拿字节，GBK 解码走 `encoding_rs`（已在 `Cargo.lock`，加成 `backup` 特性下的直接依赖），解不出就记原始十六进制。
- 测试：`tests/publish_from_pid.rs` 新加目录模式一组（备份包不在就 soft-skip）：53 / 53 写出，`T_DRAWING` 53 行 53 配对，I井场三件与单张模式 SHA256 相同（`_Coverage.json` 比较时去掉 `source` 字段），包级计数钉 §1.3 的实测数；手造的最小备份目录（一个 stored zip 里放 `test-file` 的 D06.pid，无 dump）走 `.pid` 模式也要能发。
- 交付（按 Q5）：53 张写到 `D:\work\cad\pid-test-data\publish\`。
- 文档：新页 `docs/analysis/n9-backup-package-publish.md`（成员清单、53 张计数、失败清单）；`README.md` 用法一段；`CHANGELOG.md`；`task_plan.md`；本计划 §7。
- 验收：53 / 53 写出（含 D03 / D04 两张空图），`_Index.json` 的合计与 §1.3 的 51 张实测数一致（对象 77 690、关系 103 577、管道端口 9 557、信号端口 22 021、连通 19 371、OPC 476、设备↔管口 372、注释 114；N9-2 之后再加管线与 `PipingConnectors`）；debug 构建整包用时 < 单张 8.5 s × 53 的一半（表只读一次，预期 1 分钟量级）；全量门同 N9-1。

## 5. 风险

- **管线 UID 与参考不同是设计使然**：Q1 定了就不再算错，但下游若拿参考 UID 做键（如 SPF 回灌），这 79 个要换键。`_Coverage.json` 和 `_Index.json` 都写明「synthesized」。
- **dump 与参考不是同一时刻**：码 7 的 23 个 `FluidSystem` 是第一个被它咬到的数（§1.2）。N9-2 单列，不拿参考去删值。
- **英寸写法只见过 1 / 3/4 / 1-1/2**：规则按 32 分之一化简；别的分母（如 1/8）照规则写，分析页注明只实测了这三种。
- **名字里的流体代码只见过 5 个码**：`CODELIST_SHORT_TEXT` 为空时拼不出，计数后停下看，不用 `CODELIST_TEXT` 顶替。
- **zip 条目名 GBK**：别的厂的备份可能是 UTF-8 或别的代码页；图名以 `T_DRAWING.NAME` 为准，zip 名只记在索引里，解不出不致命。
- **别的备份包可能有解析不了的图**：这份 53 张只有 D03 / D04 两张空图要走 `T_DRAWING.PATH`；N9-3 一张失败不中断，失败的进 `_Index.json` 和分析页，不在本轮修。
- **共享工作树与共享 `D:\Rust\target`**：只碰 §4 列出的文件；只 `git add` 明确路径；cargo 目录锁被别的会话（pid-plot 正在构建）占着就等；`--all-targets` 被未提交 example 卡住照旧标 NOT RUN。
- **内存**：74 MB dump 一次读入，`ExpTables` 常驻；53 张图逐张装载、写完即丢。

## 6. N9 之后（不在本轮，供你排序）

- **N10 `_Meta.xml`**：参考有 `DocumentVersion`（`DocVersion` 1、`DocVersionDate` 2026/06/22、`DocRevision` 0）、`DocumentRevision`、`File`（`.pid` 名 + `\\Glpid-7\pidsq\PID` 路径）和 3 条关系；我们的是空壳 384 字节。`T_DRAWING` 有 `REVISION` / `VERSION` / `PATH` / `LASTPUBLISHTIME`（I井场行 `REVISION` / `VERSION` 都空），`T_REVISION` 0 行，`T_DRAWINGVERSION` 15 行；三件的 UID 也是适配器造的（`AAAD` 签名前缀），得先定口径。
- **N11 其余 RefData**：符号库 `RefData~4~681.zip`（Manifest：645 个文件，仓库已有 `.sym` 解析）、`PlantConfig.xml`、`CatalogIndex.mdb`——接到渲染 / 出图那条线上，不在发布线上。
- **N12 SQL Server 变体整包入口**：TEST02_p 走 MTF → MDF → `mdf_load`；给目录模式加 `SqlServerMtf` 分支，与 Oracle exp 同一个 `_Index.json`。
- **集注站那套库的备份**（Q7）：拿到就按同一套办法补 4 张参考图的对照。

## 7. 执行记录

### 开单（2026-10-13，会话 fable-5-1-19）

- 核对：仓库 `353186a`；N5 / N7 / N8 已提交；N6 分析页结论「dump 里没有管线」不变。工作树里别的会话的未提交文件见 §1.4。
- §1 的数字：临时 Python 脚本只读备份包、5 份参考 `_Data.xml`、`publish\` 下 I井场三件；`D:\Rust\target\debug\pid_publish_xml.exe`（11:50 构建）对 53 张解到 `%TEMP%` 的 `.pid` 各跑一次 `--oracle-exp`，输出写在 `%TEMP%`，没写进 `publish\`。脚本和临时输出不入库。
- 没改 `src`，没跑 cargo，没提交。

### 批复落盘（2026-10-13，会话 grok-4.7-70）

- 用户 16:00 对 grok-4.7-57 说「Q1 到 Q7 按推荐项批，仍先别写代码」。57 的落盘在这句截住，没有回执。本会话按交接只读之后，用户点了「把 Q1–Q7 的批复写进 N9 计划，仍不写实现」。
- 只追加 §3.1 和本节。Q1–Q5 批推荐项 (a)；Q7 按计划写明的 (b) 收下；Q6 没有推荐项，仍未拍板。
- 没改 `src`，没跑 cargo，没提交。`acquire_file_lock` 两次回「没有这条会话」，锁没拿到。本文件当时是未跟踪状态。

### Q6 补批（2026-10-13，会话 grok-4.7-68）

- 用户选 Q6 **(a)**：等 `src/backup/folder.rs` 那条会话自己提交，N9-3 在那之后开工。本会话不代交那 8 个路径。
- 第一轮「按推荐项批」盖不住 Q6，因为推荐栏是「你定」。本轮是用户单独点选 (a)。
- N9-1 / N9-2 仍按此前「仍先别写代码」未开工。只改本计划 §1.4、§3.1 的 Q6 行和补批一句、§4 开工条件、本节。没改 `src`，没跑 cargo，没提交。
- `check_paths` / `whoami` / `acquire_file_lock` 都回「没有这条会话」，锁没拿到。共享记忆 `record_decision` 同样报没报到，这条共识只落在本文件里。

### N9-1 落地（2026-10-13，会话 grok-4.7-68）

- 用户点「按已批的 Q1–Q5 开始写 N9-1」。只改 `src/publish/oracle_exp.rs` 和本计划本节。没提交。`src/backup/folder.rs` 没碰。`acquire_file_lock` 仍回「没有这条会话」。
- `ExpTables::read` 先钉 `APPLICATIONS`，再一次读入四个入口用的 12 张表，加上第一份 `CODELISTS` 和 `T_DRAWING`（日期列走 `read_table_tolerant`）。`read_port_tables` / `connectivity_ends_for_drawing` / `composition_relations` / `opc_points_for_drawing` 仍吃 dump，内部建表后调 `ExpTables` 上的同名入口。`pid_publish_xml` 单张调用没改。
- 手造片段：同一份 `ExpTables` 挂 `DRAW1` / `DRAW2`，连通各 1 条 `PipingEnd1Conn`。实包 `Export.dmp`：`T_DRAWING` 53 行，端口 9 628 / 22 057，与 `read_port_tables` 相等。
- I井场 `pid_publish_xml --oracle-exp --diff-against --plant P01` 退出 0，clean（对象 3 014 / 值 3 188 / 关系 4 001）。写到 `target\n9-1-sha` 的三件与 `D:\work\cad\PID自研软件验证\publish\` 现有三件 SHA256 相同，`publish\` 没改。
- **验证**（2026-10-13 实跑，全部退出 0）：`cargo test --locked -p pid-parse --lib --bins --tests`（`--lib` 1 203 过 / 0 败，`publish_from_pid` 6 过）；`--no-default-features` 同范围（`--lib` 850 过 / 0 败）；`cargo test --locked --workspace --lib --bins --tests`。clippy `-D warnings` 两种特性在 `-p pid-parse --lib --bins --tests` 零告警。`src/publish/oracle_exp.rs` rustfmt 干净。`cargo rustdoc --locked -p pid-parse --lib -- -W missing-docs` 过。wasm32：`cargo check --locked -p pid-parse --lib --no-default-features --target wasm32-unknown-unknown` 过。`--all-targets` 测试 NOT RUN：`cargo test --locked -p pid-parse --all-targets --no-run` 退出 0，example 能编过，本单不跑那些探针。

### N9-1 提交（2026-10-13，会话 fable-5-1-37）

- 按 `docs/plans/2026-10-13-publish-progress-and-next-steps.md` S0（Q1 (a)，Plannotator 20:36 批）：`src/publish/oracle_exp.rs` 内容不改；`CHANGELOG.md` 补 N9-1 一段，`task_plan.md`「当前阶段」补 N8 与 N9-1 两段；本节记提交。只 `git add` 这四个路径，提交说明 `feat(publish): read the Oracle exp tables once for many drawings`。工作树里别的三摊（`src/backup/folder.rs` 组、pid-plot、探针）一字不碰。
- 提交前复跑（22:3x，全部退出 0）：`cargo test --locked -p pid-parse --lib publish::oracle_exp` 13 过 / 0 败；`--test publish_from_pid` 6 过 / 0 败；clippy `-D warnings` 两种特性在 `-p pid-parse --lib --bins --tests` 零告警；`rustfmt --check src/publish/oracle_exp.rs` 与 `cargo fmt --all -- --check` 都干净；`cargo rustdoc --lib --locked -p pid-parse -- -W missing-docs` 0 = 基线 0。`publish\` 没动（I井场三件 11:50:22，另外 4 张 11:40:21）。
- `acquire_file_locks` 回「没有这条会话」，锁没拿到。

### N9-3 落地（2026-10-13，会话 grok-4.7-109）

- 用户点「开工 N9-3」。N9-2 还没做，整包按 N8 的口径发（不合成管线）。`src/backup/folder.rs` 已在树上，本轮只调用 `scan_backup_folder`，不改那组文件。
- `pid_publish_xml <目录> --out-dir`：`Manifest.txt` 在场即备份包；`Export.dmp` 是 Oracle exp 时 `ExpTables` 读一次；`PlantData` zip 的 `.pid` 用 `load_publish_from_reader` 在内存里装载；图名取 `T_DRAWING.NAME`；没有 `DrawingNo` 的按 `backup_path_key` 对 `T_DRAWING.PATH`，只写 `PIDDrawing`，`uid_from` 为 `T_DRAWING.PATH`。MTF 或 `--diff-against` 退出 2。没有 dump 的目录走 `.pid` 单独发布。单张 `.pid` 调用没改。
- 交付：`D:\work\cad\pid-test-data\publish\`（53 张三件 + `_Index.json`）。53 / 53，失败 0，空图是 `test/U01/D03.pid` 与 `D04.pid`（图名 `D03` / `D04`，层级 `SQPlant \ test \ U01`）。51 张合计对象 77 690、关系 103 577、管道端口 9 557、信号端口 22 021、连通 19 371、OPC 476、管口关系 372、注释关系 114。`cargo run` 墙钟 53 秒，退出 0。
- 测试：`publish_from_pid` 8 过（整包一组钉上面的数，并对照 I井场单张三件；无 dump 的 stored zip 放 `test-file/D06.pid`）。`cargo clippy -D warnings` 默认特性 `--lib --bins --tests` 与 `--no-default-features --lib` 零告警。`cargo fmt --all -- --check` 在最后一行模块文档之前干净。wasm32 `--no-default-features --lib` check 过。全库 `--lib` / `--workspace` / `--all-targets` 没跑。没提交。
- `acquire_file_lock` 回「没有这条会话」，锁没拿到。

### 整包钉数改成挂管线后的实测（2026-10-14，会话 grok-4.7-1）

- N9-2 的合成已经在工作树：`src/publish/pipeline.rs` 的 `attach_pipelines`。目录模式对 `uid_from != T_DRAWING.PATH` 的图调用。D03 / D04 不挂管线。
- 实跑 `pid_publish_xml D:\work\cad\pid-test-data --plant P01` 退出 0。51 张合计对象 80 085、值 87 419、关系 106 705。按图加总管线 2 395、`PipingConnectors` 3 128，两张空图这两项是 0。比本节 N9-3 的 77 690 / 103 577 多出的对象和关系就是这两项。管道端口 9 557、信号端口 22 021、连通 19 371、OPC 476、管口 372、注释 114 不动。
- `tests/publish_from_pid.rs` 的整包钉是这四个数。`backup_directory_publishes_the_whole_package` 1 过，50.19 秒，I井场整包与单张对照过。I井场库测仍钉 3 093 / 3 346 / 4 107、按名 79 / 106、`FluidSystem` 多 23，本轮没重跑。
- 文档：`docs/analysis/n6-pipeline-rows.md`「合成」、`CHANGELOG.md`、`task_plan.md`、本节。交付目录 2026-10-14 01:57–01:58 已按 Q5 写好，本会话核对：`pid-test-data\publish\_Index.json` 对象 80 085、关系 106 705、管线 2 395、`PipingConnectors` 3 128，I井场 3 093 / 3 346 / 4 107、管线 79、连接器 106。验证目录只换了 I井场三件；`_Data.xml` SHA256 `7653DF6B73B533E959970C9C346602F9F73A2B0BFC8039008F06DA2EBD132708`，`_Meta.xml` SHA256 `C1D4465F97EAFA9EC74A9476AAA2E4C3429719AA3E1A7A457BD34BF064E77B7C`，与整包相同。覆盖去掉 `source` 后相同：单张是 `PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid`，整包是 `PlantData~2~711.zip:00/00/I井场 注采阀组工艺及自控流程图.pid`。另外 12 个集注站文件仍早于 2026-10-13。没再跑发布。没提交。`acquire_file_locks` 回「没有这条会话」，锁没拿到。
