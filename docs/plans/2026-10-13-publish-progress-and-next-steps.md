# publish 文件生成 · 进度盘点与下一步开发计划（2026-10-13）

> **状态：本篇 §1 / §2 是 2026-10-13 的快照，已被后续落地取代（2026-10-21 复核）。** §6 执行记录保留原始时间线。
>
> 已落地：N9-1 `b51f18d`；N9-2 管线合成 + N9-3 整包目录 `7f1d891`；N10 `_Meta.xml` 派生对象 `4866b81`；N12 `SQL Server` MTF 整包入口 `3722c2f`；A1–A4 把 `PublishItemCatalog` 接进 `sqlite_load` / `xml_writer` / `diff`（`60f1b25` / `7c79749` / `4735be9` / `6881c18`，收尾 `595ce19`）。全部在 `codex/phase32c-bundle-closeout` 上，`main` 未合。
>
> 交付目录 `D:\work\cad\pid-test-data\publish\`：160 个文件（53 张 × 三件 + `_Index.json`），`_Index.json` 记 53 / 53 写出、失败 0、空图 2、对象 **80 085**、值 **87 419**、关系 **106 705**、管线 **2 395**（`synthesized`）、`PipingConnectors` **3 128**。
>
> 2026-10-21 复跑门禁：`publish_from_pid` 9 / 9（整套 60.7 s，整包 53 张那条占绝大部分）、`publish_xml_cli` 20 / 20、`publish_mdf_load` 3 / 3、`publish_meta_parity` 6 / 6，退出码均 0。
>
> 仍登记未做：N11（符号库 / 模板 / `PlantConfig.xml` 内容解析）、集注站 4 张的参考对照（等那套库的备份）、DWG 侧 `Export.mdf` 夹具缺失导致的 soft-skip。
>
> 2026-10-22 目录收口：`pid_load` 里的 `CLASS_OF_ITEM_TYPE` / `UNCLASSED_ITEM_TYPES` 两张手写表删除，`.pid` 线的分类改由 `PublishItemCatalog` 的 `pid_class` 决定；`diff::supported_pid_tags` 改成数据库线与 `.pid` 线标签集的并集，`.pid` 独有的 6 个类（`PIDInstrument` / `PIDInlineInstrument` / `PIDProcessEquipment` / `PIDMechanicalEquipment` / `PIDHeatTransferEquipment` / `PIDSignalBranchPoint`）不再被 `coverage_against_reference` 报成 backlog。整包 53 张重跑比对：106 个 `_Data.xml` / `_Meta.xml` 字节全同，53 份 `_Coverage.json` 除记录下的输入路径分隔符外全同。紧接着 `_Coverage.json` 的派生子项（`PIDProcessPoint` / 两个 branch point / 两个 port）也不再在 `pid_writer` 里硬写：目录的每行 `.pid` 标签带上 `PidTagRole`，报表按角色决定去哪张集合取数；重发后 53 张的 160 个文件与改动前逐字节相同。

> 承接 `docs/plans/2026-10-10-publish-from-the-pid-alone.md`（P0–P5）、`docs/plans/2026-10-11-publish-native-evidence-via-ida-bridge.md`（N0–N4）、`docs/plans/2026-10-12-n5-ports-from-oracle-exp.md`（N5）、`docs/plans/2026-10-13-n9-publish-the-whole-backup-package.md`（N9，Q1–Q7 已批，N9-1 已写在工作树）。
> 用户 10-13 19:0x「分析现在 publish 文件生成的进度。使用 plannotator 生成开发计划」。本文件只盘点和开单，**没改任何代码，没提交**。
> 基线：pid-parse `353186a`（`codex/phase32c-bundle-closeout`，与 `origin` 同步，无 ahead / behind）。工作树上别的会话的未提交文件（§1.4）一字不碰。
> 编写：zhimo 会话 fable-5-1-30。§1 的数字出自：仓库 `git log` / `git status` / `git diff --stat`；`D:\work\cad\PID自研软件验证\publish\` 15 个文件的时间戳与 I井场 `_Coverage.json`；`tests/publish_from_pid.rs` 的钉数；N5 / N7 / N8 / N9 四份文档的实测记录；以及本会话 10-13 19:2x 在**当前工作树**（含未提交的 N9-1）实跑的两组测试（§1.4）。
> `acquire_file_lock` 回「没有这条会话」，锁没拿到；本文件是新建的未跟踪文件，不与任何人的改动重叠。

## 一句话

publish 线今天的位置：**I井场在 `--oracle-exp` 模式对象 3 014 / 3 093（97.4 %）、关系 4 001 / 4 107（97.4 %），错 0**；差的 79 个 `PIDPipeline` + 106 条 `PipingConnectors` 不在 dump、不在 `.pid`，N9 已定由 `T_PIPERUN` + `CODELISTS` + `SPPIDDataMap.xml` 按 `PipeLinePublisher` 的口径合成。另外 4 张集注站图只有 `.pid` 模式（1 723 / 3 038 对象，56.7 %），它们的参考是别的库发布的，这份备份里对不上，暂不推进。整包 53 张图的目录入口（N9-3）还没开工，`D:\work\cad\pid-test-data\publish\` 还不存在。**N9-1（dump 的表读一次）代码已在工作树、13 条单测 + 6 条集成钉数全过，但没提交、没进 CHANGELOG。** 下一步按顺序：S0 把 N9-1 收尾提交 → S1 N9-2 合成管线（I井场到 3 093 / 4 107）→ S2 N9-3 一次发 53 张 → S3 `_Meta.xml`（N10）先出数再定口径。

## 1. 现状

### 1.1 两条发布线

| 线 | 输入 | 路径 | 状态 |
|---|---|---|---|
| 数据库线（SQL Server） | MTF 备份 → `pid_backup_extract` → `Export.mdf` | `mdf_load`（MDF → SQLite）→ `sqlite_load` → `xml_writer` → `pid_publish_xml <mdf> --drawing UID` | 自 05-13 后 `mdf_load.rs` / `sqlite_load.rs` 没动；门 `publish_mdf_load` / `publish_xml_cli` / `publish_meta_parity` / `publish_a01_raw_residual`（TEST02_p / A01 fixture，缺就 soft-skip）。开着一个 bug **#27**（codelist join 把 `attribute_codelisted` 当表号，正确的链是 `attribute_datatype = C<n>`，10-09），没人接 |
| `.pid` 线（本轮主线） | 单张 `.pid`（+ 可选 Oracle `exp` dump） | `pid_load`（`.pid` → `PidPublishDrawing`）→ `oracle_exp`（挂端口 / 连通 / 组成 / OPC）→ `pid_writer` → `pid_publish_xml <file.pid> --out-dir DIR [--oracle-exp dump] [--diff-against ref]` | 10-10 开线，三天 12 个提交到 N8；N9-1 在工作树 |

两条线写出的都是 SmartPlant 发布的 `<图名>_Data.xml` + `_Meta.xml`，`.pid` 线旁置 `_Coverage.json`。`--diff-against` 的口径：发出的每个对象 / 值 / `<Rel>` 必须在参考里逐项相同（错 0），整类有意不发不算 `MISSING`。

### 1.2 `.pid` 线的里程碑（全部与参考错 0）

| 步 | 日期 | 落点 | 发出什么 | I井场对象 / 值 / 关系 |
|---|---|---|---|---|
| P0–P1 | 10-10 | 探针 + 分析页 | 只凭 `.pid` 能给回多少：对象 UID 2 948 / 6 131（48.1 %），关系自己的 UID 0 | — |
| P2–P5 + 小单 | 10-10 | `pid_load` / `pid_writer` / CLI `--out-dir` / `_Coverage.json`；无 `SymbolAttr` 的仪表按符号库目录归类 | 图纸、物项（设备 / 仪表 / 管件 / 管段 / 注释）、表示、标签值、`DwgRepresentationComposition` / `DrawingItems` | 5 图 2 918 / 2 822 / 3 389 |
| N4-0 … N4-4 | 10-12 | `t-1489` / `t-1490` / `fa16824` / `cec947e` / `7fdeae8` | `DerivedByNativeRule` 出处；公称直径 `N in` / `N mm`；工艺点 + `PRP`；分支点 `.BPT`；**连通关系判定不发**（`.pid` 里配不成对） | 1 735 / 1 530 / 1 971（5 图钉数见 `tests/publish_from_pid.rs::EXPECTED`） |
| N5 对照 / N5-1 / N5-2 | 10-12 | `a287078` / `75e1c44` / `e2e205d` | `--oracle-exp`：`PIDPipingPort` 362 + `PIDSignalPort` 915，`PPC` / `SPC` | 3 012 / 3 182 / 3 248 |
| N6 | 10-12 | 分析页 `n6-pipeline-rows.md` | **不发**：`T_PIPELINE` / `T_PLANTITEMGROUP` 空表，79 个管线 UID 在 dump 全文 0 命中 | 不变 |
| N7 | 10-13 01:25 | `c1ad13e` | 六种连通 `<Rel>`（规则 A：物项在同段两侧都出现 → Tap；只起始侧 End1；只结束侧 End2），738 对定义逐条同参考 | 3 012 / 3 182 / 3 984 |
| N8 | 10-13 12:03 | `353186a` | `EquipmentComponentComposition` 7（`T_NOZZLE`）、`PBSItemNotes` 6（`T_NOTE`）、2 个 OPC `PIDBranchPoint` 及其 `DrawingItems` 与 2 条连通 | **3 014 / 3 188 / 4 001** |
| N9 开单 + 批复 | 10-13 14:0x–16:xx | `docs/plans/2026-10-13-n9-…md`（未跟踪） | Q1–Q5 批 (a)，Q6 批 (a)，Q7 按 (b) 收下 | — |
| N9-1 | 10-13 | **工作树**（`src/publish/oracle_exp.rs` +657 / −255 行，未提交） | `ExpTables::read` 一次读入 12 张表 + `CODELISTS` + `T_DRAWING`；四个入口改为薄封装，行为不变 | 不变（SHA256 与 `publish\` 三件相同，N9 §7） |

### 1.3 距参考还差什么

**I井场**（参考 `DocUID 5633AF41…`，3 093 对象 / 4 107 关系；`_Coverage.json` 10-13 11:50:22）：

| 项 | 已发 | 参考 | 差 | 去向 |
|---|---:|---:|---:|---|
| 对象 | 3 014 | 3 093 | 79 | 全是 `PIDPipeline`；UID 哪儿都没有（N6 + N9 §1.2），N9-2 按名合成、Q1 定 UUID v5 |
| 关系 | 4 001 | 4 107 | 106 | 全是 `PipingConnectors`（管段 → 管线），N9-2 随管线一起发 |
| 值 | 3 188 | — | — | 发出的逐值相同；N9-2 之后会比参考**多 23 个** `FluidSystem`（码 7，dump 比参考新，Q2 (a) 照发并单列） |

已发对象的构成（`_Coverage.json`）：`.pid` 命名物项 556 / 558（2 个 OPC 物项在 `.pid` 侧不定类，由 exp 侧补成 `PIDBranchPoint`）+ 表示 902 + 工艺点 169 + 分支点 107 + 管道端口 362 + 信号端口 915 + 图纸 1。关系 13 种，连通六种 738 条、`left_out` 0。

**另外 4 张集注站图**（`.pid` 模式钉数）：分离器 A 353 / 326 / 397，分离器 B 283 / 256 / 318，低温分离器 A 374 / 347 / 415，换热器 CD 713 / 636 / 828；合计对象 1 723 / 3 038（56.7 %）、关系 1 958 / 4 078（48.0 %）。缺的整类是端口 / 连通 / 管口组成 / 注释 / 管线——都要库。这份备份里同名 `.pid` 是另一版、另一套 UID（N9 §1.1），参考对照只能等集注站那套库的备份（N5 Q5 / N9 Q7）。

**五图合计**：对象 4 737 / 6 131（77.3 %），关系 5 959 / 8 185（72.8 %）。

**整包 53 张**（N9 §1.3，10-13 14:xx 临时实测，`353186a` 的 debug 二进制逐张跑 `--oracle-exp`）：51 张退出 0，2 张（`test/U01/D03`、`D04`，`.pid` 里没有 `DrawingNo`）退出 1；合计对象 77 690 / 关系 103 577；每张 8.3–8.8 s，共 431 s——差的全是每张重读 dump 的 12 张表，N9-1 正是为此。

### 1.4 工作树与验证（10-13 19:2x 实查）

- `HEAD` = `353186a`，与 `origin/codex/phase32c-bundle-closeout` 同步；10-12 之后的 N5-2 / N7 / N8 都已在远端。
- 未提交：23 个修改 + 21 个未跟踪。按归属分四摊：
  - **N9-1**：`src/publish/oracle_exp.rs`（+657 / −255 行）；N9 计划本身（`docs/plans/2026-10-13-n9-…md`，未跟踪）。
  - **`src/backup/folder.rs` 组**（另一会话，N9 Q6 (a) 等它自己提交）：`Cargo.toml`（一条 `[[test]]`）、`src/backup/mod.rs`、`src/backup/folder.rs`、`tests/backup_folder_real_dir.rs`、`CONTEXT.md`、`docs/current-architecture-principles.md`、`docs/agents/domain.md`。
  - **pid-plot 组**（出图对比线，#30–#37）：`pid-plot/` 下 7 个修改 + 3 个未跟踪、`docs/plans/2026-10-12-plot-text-match-and-ratchet.md`、2 个 10-12 分析页。
  - **探针组**：`examples/` 5 个修改 + 5 个未跟踪、7 个 10-11 分析页、`n6-pipeline-rows.md` 的「全表搜」一节等。
- 10-13 18:53:43 有 37 个文件被同一秒重写（含与 `HEAD` 相同的 `xml_writer.rs` / `pid_writer.rs` / `task_plan.md` 等），像是某个会话恢复了检查点；内容核对：被重写的已提交文件与 `HEAD` 无差异，未提交的四摊都还在。`git stash` 里只有 07-28 的旧条目，与此无关。
- **本会话在当前工作树实跑**（含未提交的 N9-1）：
  - `cargo test --locked -p pid-parse --lib publish::oracle_exp`：13 过 / 0 败（含 `exp_tables_reads_the_backup_dump_once`、`one_exp_tables_serves_two_drawings`，以及 I井场端口 / 连通 / 管口注释 / OPC 四条对参考的库测），编译 5.5 s——说明这棵树刚被谁编过。
  - `cargo test --locked -p pid-parse --test publish_from_pid`：6 过 / 0 败，11.6 s。`.pid` 模式五组钉数、I井场 exp 组 3 014 / 3 188 / 4 001 与六种连通 `[369, 129, 143, 38, 46, 13]`、`--diff-against --oracle-exp` 退出 0 都在里面。
  - 没跑 `--workspace` 全量门、clippy、wasm32（N9 §7 的 N9-1 记录 10-13 已跑过一轮全绿；本会话不改代码，不重复）。
- 文档尾巴：`task_plan.md`「当前阶段」最新一段还是 N7，**N8 和 N9-1 都没记**；`CHANGELOG.md` 有 N8 两段、**没有 N9-1**；N9 计划 §7 有 N9-1 落地记录。issue **#29**（N5）标题仍是「发端口还没开工」，10-12 15:42 后没更新——N5-1 / N5-2 / N7 / N8 都已交付，差一条收尾评论。

### 1.5 交付目录

| 目录 | 内容 | 时间 |
|---|---|---|
| `D:\work\cad\PID自研软件验证\publish\` | 5 张 × 三件 = 15 个文件 | I井场三件 10-13 11:50:22（= N8 OPC 步，`353186a`）；4 张集注站 10-12 11:40:21（N4 末态 `.pid` 模式） |
| `D:\work\cad\pid-test-data\publish\` | **不存在** | N9-3 的落点（Q5 (a)），未开工 |

## 2. 范围

**做**（本计划的单）：S0 N9-1 收尾提交；S1 N9-2；S2 N9-3；S3 N10 `_Meta.xml` 先只读出数；S4 文档与 issue 收口。S1 / S2 的具体做法、测试、验收数字**以 N9 计划 §4 为准**，这里只列顺序、前置条件和交付的数。
**不做**：集注站 4 张的参考对照（等那套库的备份，N9 Q7）；#27 codelist bug（数据库线，另开单）；pid-plot 线（#30–#37，自己的计划）；符号库 / 模板 / `PlantConfig.xml` 内容解析（N11）；SQL Server 整包入口（N12）；push（等你说）。

## 3. 要你拍板的

| # | 问题 | 选项 | 推荐 |
|---|---|---|---|
| Q1 | N9-1 什么时候提交 | (a) 现在单独一个提交（只 `git add src/publish/oracle_exp.rs docs/plans/2026-10-13-n9-…md CHANGELOG.md task_plan.md`），N9-2 在它之上开工；(b) 攒到 N9-2 一起提 | **(a)**：它已过 13 + 6 条测试、行为不变、只碰一个 `.rs`；共享工作树上多攒一天就多一天被检查点恢复 / 别人的 `git add` 卷走的风险 |
| Q2 | `src/backup/folder.rs` 组一直没提交，N9-3 要等多久 | (a) 等到 N9-2 交付那一刻还没提交，就按 N9 Q6 (b) 由 N9-3 的会话核对后原样代交（只 `git add` 它那 7 个路径——`git status` 今天数出 7 个，N9 §1.4 写的是 8，以提交当时的 `git status` 为准——提交说明写明代交）；(b) 一直等它自己的会话 | **(a)**：Q6 (a) 的前提是那个会话还在；18:53 的检查点恢复之后看不出它还活着。到时候再等就卡住整包入口 |
| Q3 | N9 之后先做哪个 | (a) N10 `_Meta.xml`（先只读出数：`T_DRAWING` 53 行的 `REVISION` / `VERSION` / `LASTPUBLISHTIME`、`T_DRAWINGVERSION` 15 行、参考三件对象 + 3 条关系的 UID 构成，再定口径）；(b) N12 SQL Server 整包入口 | **(a)**：53 张的 `_Meta.xml` 现在都是 384 字节空壳，三件缺着交付就不完整；N12 需要另一份 MTF 整包，TEST02_p 只有 MDF 一半 |
| Q4 | issue #29 | (a) 评论 N5-1 / N5-2 / N7 / N8 四个提交和 I井场的数，关闭；N9 的事挂到新 issue；(b) 改标题留作 N 线总 issue | **(a)**：一事一 issue，和 #31–#37 的用法一致 |

### 3.1 N10 `_Meta.xml` 口径问题（S3 出数后追加，2026-10-14，详见 `docs/analysis/n10-meta-rows.md`）

S3 只读探针 `examples/probe_oracle_exp_drawing_meta.rs` 查明：`T_DRAWING` 53 行的 `REVISION` / `VERSION` / `LASTPUBLISHTIME` / `SP_LASTEFPUBLISHTIME` 全部是 `0/53`（全 NULL），`T_REVISION` 是 `0` 行空表，`T_DRAWINGVERSION` 的 `15` 行全是已删除旧图的快照留底（`SP_COMMENTS="Created for Delete"`，与现存 53 张图的 `SP_ID` 命中 `0/15`）。5 份参考 `_Meta.xml` 的 30 个 UID（前 4 个为不带连字符的 v4 随机 GUID，后 2 个 `File` / `FileComposition` 为 `AAAD` + 掉前导 `0` 的带连字符 v4 GUID）以及 `DocVersionDate`（发布当日挂钟日期 `2026/06/22` / `2026/07/06`）、`FilePath`（发布机器 UNC 路径 `\\Glpid-7\pidsq\PID`）在 `Export.dmp` 与 `.pid` 里全部是 `0` 命中。提交 `4866b81` 已将 `write_pid_meta_xml` 改调 `write_meta_xml`（`derive_meta_uid` 派生 6 个 UUID v5，`DocRevision="0"`、`DocVersion="1"`、`MajorRev_ForRevise="0"`，`DocVersionDate=""`、`FilePath=""`）。待拍板：

| # | 问题 | 选项 | 推荐 |
|---|---|---|---|
| Q5 | `_Meta.xml` 的 6 个 UID 与默认版本/修订号 | (a) 保持 `4866b81` 的 `derive_meta_uid`（32 位大写十六进制 UUID v5）与 `DocRevision="0"` / `DocVersion="1"` / `MajorRev_ForRevise="0"`；若后续遇到 `T_DRAWING.VERSION` / `REVISION` 非空的备份包再优先取库列；(b) 给 `File` 与 `FileComposition` 的派生 UID 仿照参考加 `AAAD` 前缀与连字符 | **(a)**：参考 UID 是发布时随机 v4 GUID 且含 `%X` 掉前导 `0` 的历史偶发格式，数据库线（`xml_writer`）本就用 `derive_meta_uid`，两条线保持同一规则 |
| Q6 | `IDocumentVersion.DocVersionDate` 写什么 | (a) 保持 `4866b81` 的空串 `""`（库里 `LASTPUBLISHTIME` 与 `SP_LASTEFPUBLISHTIME` 全 NULL，不拿 `T_DRAWING.DATECREATED` 冒充发布日期，也不用运行时的系统时钟破坏重跑幂等）；(b) 在 `--oracle-exp` / 备份包模式下若 `LASTPUBLISHTIME` 非空则填 `YYYY/MM/DD`，否则留空 `""` | **(b)**（兼容 **(a)**）：在这份 53 张备份包上输出仍是 `""`（与 `4866b81` 逐字节相同），同时对未来带 `LASTPUBLISHTIME` 的库如实写出 |
| Q7 | `IFile.FilePath` 写什么 | (a) 保持 `4866b81` 的空串 `""`（参考的 `\\Glpid-7\pidsq\PID` 是发布机器的局域网共享路径，不在库和 `.pid` 里）；(b) 在 `--oracle-exp` / 备份包模式下填 `T_DRAWING.PATH` 的父目录（如 `\00\00`） | **(a)**：`T_DRAWING.PATH` 是植物组相对路径而非 `IFile.FilePath` 的 UNC 目录，且 `_Index.json` 已记录 `path`；保持 `""` 不造假路径 |

**批复（2026-10-14，用户对 gemini-4-argon-14）**：Q5–Q7 按推荐项定下，保持 `4866b81` 现行 `_Meta.xml` 口径（`derive_meta_uid` 派生 6 个 32 位大写十六进制 UUID v5，`DocRevision="0"`、`DocVersion="1"`、`MajorRev_ForRevise="0"`，`DocVersionDate=""`、`FilePath=""`）。

## 4. 步骤

一条一单，一个提交。顺序 S0 → S1 → S2 → S3；S4 的文档随各单一起进，不单独提交。

### S0 N9-1 收尾提交（Q1 (a)）

- 不改 `src/publish/oracle_exp.rs` 的内容。补 `CHANGELOG.md` 一段「publish 从 Oracle exp 一次读表、挂多张图（N9-1）」：`ExpTables::read` 读 12 张表 + `CODELISTS` + `T_DRAWING`，四个入口薄封装，行为不变，I井场三件 SHA256 不变；补 `task_plan.md`「当前阶段」N8 与 N9-1 两段（N8 那段按 `353186a` 的 CHANGELOG 两段压缩）。
- 提交前复核：`git diff --stat` 仍只有 `oracle_exp.rs` 一个 `.rs`；重跑 `cargo test --locked -p pid-parse --lib publish::oracle_exp` 与 `--test publish_from_pid`；clippy `-D warnings` 两种特性 `-p pid-parse --lib --bins --tests`；rustfmt；missing-docs 不升。
- 只 `git add` 四个明确路径；提交说明 `feat(publish): read the Oracle exp tables once for many drawings`。
- 验收：`git status` 里 N9-1 的两份文件消失；`publish\` 不动。

### S1 N9-2 合成管线与 `PipingConnectors`（按 N9 §4「N9-2」）

- 前置：S0 已提交。
- 装载 / 写出 / 对照 / 测试 / 文档全按 N9 §4。新增文件 `src/backup/datamap.rs`（`SPPIDDataMap.xml` 只解 `MapEnumToEnum` 与 `SPMapEnumListDef` 名字）；CLI 新参数 `--datamap`。
- 交付数（I井场 exp 模式）：对象 **3 093**、关系 **4 107**，`PIDPipeline` 79、`PipingConnectors` 106，按名配对 79 / 106，错类 / 错值 / 多发 0，`FluidSystem` 比参考多 23（单列）。`tests/publish_from_pid.rs` 的 exp 组钉数改到这组数。`.pid` 模式五组钉数不动。
- 交付（Q5 (a)）：I井场三件重写到 `PID自研软件验证\publish\`，另外 4 张不动。
- 验收：`--diff-against --oracle-exp` 退出 0；全量门同 N9-1（`--workspace --lib --bins --tests`；`--no-default-features`；clippy 两种特性；rustfmt；missing-docs；wasm32 lib；`--all-targets --no-run`）。

### S2 N9-3 目录入口：一次发 53 张（按 N9 §4「N9-3」）

- 前置：S1 已提交；`src/backup/folder.rs` 组已提交（它自己的会话，或按 Q2 (a) 代交）。
- 入口 `pid_publish_xml <目录> --out-dir DIR [--plant]`（Q4 (a)）：`scan_backup_folder` → `Export.dmp` 必须是 Oracle exp（MTF 退 2）→ `ExpTables` 读一次 → `SPPIDDataMap.xml` 读一次 → 逐个 `.pid` 条目在内存里装载（`load_publish_from_pid` 要有吃 `Read + Seek` 的入口）→ 图名取 `T_DRAWING.NAME`，没有行时取 GBK 解码的 zip 条目名 → 三件 + `_Index.json`。一张失败不中断，最后退 1。
- 交付数：53 / 53 写出（含 D03 / D04 两张只有 `PIDDrawing` 的空图，`uid_from: T_DRAWING.PATH`）；`_Index.json` 的 51 张合计与 N9 §1.3 一致再加 S1 的管线与 `PipingConnectors`；I井场三件与单张模式 SHA256 相同；debug 整包用时 < 431 s 的一半。
- 交付（Q5 (a)）：`D:\work\cad\pid-test-data\publish\`（新建）。
- 文档：新页 `docs/analysis/n9-backup-package-publish.md`、`README.md` 用法一段。
- 验收：`tests/publish_from_pid.rs` 新加目录模式一组（备份包不在就 soft-skip）+ 手造最小备份目录走 `.pid` 模式；全量门同 S1。

### S3 N10 `_Meta.xml` · 先只读出数（Q3 (a)）

- 只读探针（`examples/probe_oracle_exp_drawing_meta.rs`）+ 一页分析 `docs/analysis/n10-meta-rows.md`：`T_DRAWING` 53 行的 `REVISION` / `VERSION` / `LASTPUBLISHTIME` / `PATH` 非空计数；`T_DRAWINGVERSION` 15 行对到哪 15 张图；`T_REVISION` 0 行；5 份参考 `_Meta.xml` 里 `DocumentVersion` / `DocumentRevision` / `File` 三个对象的属性与 3 条关系的 `DefUID`，以及它们 UID 的构成（`AAAD` 前缀 + 什么）。
- 出数后在本计划 §3 追加 N10 的口径问题（UID 怎么造、I井场 `REVISION` / `VERSION` 都空时写什么、`File` 的路径写备份包里的还是参考那台机器的），批了再开发。
- `src` 不动。

### S4 文档与 issue 收口

- 随 S0：`task_plan.md`「当前阶段」补 N8 / N9-1；`CHANGELOG.md` 补 N9-1。
- 随 S0 或单独：#29 按 Q4 处理；给 N9 开一条 issue（标题「N9：整包发布」，正文链到 N9 计划与 `353186a`），S1 / S2 的提交各加一条评论。
- 随 S1 / S2：N9 计划 §7、本计划 §6。

## 5. 风险

- **共享工作树**：四摊未提交改动并存，`--all-targets` 可能被别人的 example 卡住（照旧标 NOT RUN）；`--workspace` 测试里 pid-plot 的那 1 败是别人的，不在本线修。每单只 `git add` 明确路径。
- **检查点恢复会重写文件**：18:53 那次重写了 37 个文件；提交前一定重跑测试，不信任早先的绿。
- **`folder.rs` 组的会话可能已不在**：Q2 决定等多久；代交要先核对它的路径清单与 N9 §1.4 描述一致、`cargo test --locked -p pid-parse --test backup_folder_real_dir` 能过。
- **管线 UID 自造**（N9 Q1）：下游若以参考 UID 为键要换键；`_Coverage.json` / `_Index.json` 写明 `synthesized`。
- **dump 比参考新**：N9-2 会比参考多 23 个 `FluidSystem`，这是第一处「多发不算错」的例外，`--diff-against` 要单列而不是放宽总口径。
- **集注站 4 张**：这份备份对不上它们的参考，五图合计的 77.3 % / 72.8 % 在拿到那套库之前不会动。

## 6. 执行记录

### 开单（2026-10-13，会话 fable-5-1-30）

- 读了交接记录与 N5 / N7 / N8 / N9 文档、`CHANGELOG.md` 头两段、`task_plan.md`「当前阶段」、`tests/publish_from_pid.rs`、I井场 `_Coverage.json`、`src/bin/pid_publish_xml.rs` 用法；核了 `git log` / `status` / `diff --stat` / `stash list` / 远端同步状态、两个交付目录、`gh issue list`。
- 在当前工作树实跑 `cargo test --locked -p pid-parse --lib publish::oracle_exp`（13 过）与 `--test publish_from_pid`（6 过），见 §1.4。
- 没改 `src`，没动别人的文件，没提交。本文件经 Plannotator 开给用户批阅。

### 批复与 S0（2026-10-13，会话 fable-5-1-37）

- Plannotator 20:36:47 落 `approved`（Q1–Q4 按推荐项 (a)）。用户 22:2x 点「开工 S0」。
- S0：`CHANGELOG.md` 补 N9-1 一段、`task_plan.md`「当前阶段」补 N8 / N9-1、N9 计划 §7 补「N9-1 提交」；`oracle_exp.rs` 内容不改。只 `git add` 四个路径提交 `feat(publish): read the Oracle exp tables once for many drawings`（`b51f18d`）。提交前复跑 `--lib publish::oracle_exp` 13 过、`publish_from_pid` 6 过、clippy 两种特性零告警、`cargo fmt --all -- --check` 干净、missing-docs 0。本文件仍未跟踪，不进这笔提交。

### S1 / S2 落地与 `_Meta.xml` 派生对象（2026-10-14，会话 grok-4.7-109 / grok-4.7-1 / gemini-4-argon-14）

- **S1（N9-2 管线合成）与 S2（N9-3 整包目录发布）**：已合并提交于 `7f1d891`（`feat(publish): synthesize PIDPipeline rows from pipe runs`），文档记录见 `8ae5f4f` / `cea805d` / `20db6cb` 及 `docs/analysis/n6-pipeline-rows.md`「合成」、`docs/analysis/n9-backup-package-publish.md`。
  - 新增 `src/backup/datamap.rs`（读取 `SPPIDDataMap.xml` 或 `RefData~4~804.zip` 的 `MapEnumToEnum`）、`src/publish/pipeline.rs`（`attach_pipelines`）、`src/publish/backup_package.rs`（目录模式一次读 `ExpTables` 与 `SPPIDDataMap.xml`，遍历 `PlantData` zip 写出 53 张三件与 `_Index.json`）。`src/backup/folder.rs` 一并随 `7f1d891` 入库。
  - **I井场（`--oracle-exp` + `--datamap`）**：对象 **3 093**、值 **3 346**、关系 **4 107**，`PIDPipeline` 79、`PipingConnectors` 106，按名配对 79 / 106，错类 / 错值 / 多发 0，`FluidSystem` 参考为空而 dump 有值 23（码 7，单列）。
  - **整包 53 张（`D:\work\cad\pid-test-data`）**：`pid_entries` / `drawing_rows` / `paired` 均为 53，`failed: 0`，空图 `shells: 2`（`test/U01/D03.pid`、`D04.pid`，`uid_from: "T_DRAWING.PATH"`，管线 0）；51 张非空图合计对象 **80 085**（比未挂管线时的 77 690 多 2 395）、值 **87 419**（多 2 395 个 `Name` + 2 298 个 `FluidSystem`）、关系 **106 705**（比 103 577 多 3 128）、管道端口 9 557、信号端口 22 021、连通 19 371、OPC 分支点 476、设备↔管口 372、注释 114、管线 **2 395**、`PipingConnectors` **3 128**。
- **`_Meta.xml` 派生对象与 `_Index.json` 标记（`4866b81`）**：
  - `write_pid_meta_xml` 改调 `write_meta_xml`，用 `derive_meta_uid`（图纸 UID + 角色的 UUID v5）写出 `DocumentVersion`、`DocumentRevision`、`File` 及 3 条关系（`VersionedDoc`、`RevisedDocument`、`FileComposition`），`DocVersionDate` 与 `FilePath` 留空；`_Index.json` 增加 `"pipeline_uids": "synthesized" | "not synthesized"`。
- **交付目录核对（Q5 (a)）**：
  - `D:\work\cad\pid-test-data\publish\`：160 个文件（53 张 × 三件 + `_Index.json`），`_Index.json` 为 80 085 / 87 419 / 106 705 / 2 395 / 3 128 / `"synthesized"`。
  - `D:\work\cad\PID自研软件验证\publish\`：I井场 `_Data.xml` 与 `_Meta.xml` 与整包对应文件逐字节相同，`_Coverage.json` 去掉 `source` 后相同；其余 4 张集注站图未动（仍是 2026-10-12 11:40:21）。
- **本会话（gemini-4-argon-14）复跑验证**：`cargo test --locked -p pid-parse --test publish_from_pid` 8 过 / 0 败（46.75 s）；`docs/analysis/n9-backup-package-publish.md` 已同步为含管线合成的 80 085 / 87 419 / 106 705 口径。下一项进入 **S3（N10 `_Meta.xml` 库表字段只读出数）**。

### S3 N10 `_Meta.xml` 只读出数（2026-10-14，会话 gemini-4-argon-14）

- 编写只读探针 `examples/probe_oracle_exp_drawing_meta.rs` 并实跑 `cargo run --locked --example probe_oracle_exp_drawing_meta`（退出 0）；产出分析页 `docs/analysis/n10-meta-rows.md`，并在本计划 §3.1 追加 Q5–Q7。
- 实测结论：
  - `T_DRAWING`（53 行，33 列）：`REVISION`、`VERSION`、`LASTPUBLISHTIME`、`LASTARCHIVETIME`、`SP_LASTEFPUBLISHTIME`、`DRAWINGDATE`、`DATEOFDRAWING` 全为 **0 / 53**（全 NULL）；`DATECREATED`、`PATH`、`DRAWINGNUMBER`、`NAME`、`SP_ID` 为 **53 / 53** 非空（I井场 `DATECREATED = 2026/06/17 23:05:20`、`PATH = "\00\00\I井场 注采阀组工艺及自控流程图.pid"`）。
  - `T_DRAWINGVERSION`（15 行，12 列）：全部 15 行都是删图前自动留底的快照（`SP_VERSIONPURPOSE = 3`、`SP_COMMENTS = "Created for Delete"`，`SP_REVISIONID` 0/15），`SP_DRAWINGID` 在现存 `T_DRAWING` 53 行中命中 **0 / 15**（其中第 12 行是 I井场旧图 `E876A5B1…` 在 `2026/06/17 23:04:16` 删除时的留底，64 秒后新建了现存 `5633AF41…`）。
  - `T_REVISION`（0 行，11 列）：空表。
  - 5 份参考 `_Meta.xml`：3 个对象 + 3 条关系的 30 个 UID（前 4 个为不带连字符的 v4 随机 GUID，I井场 `DocumentVersion.UID` 因 `%X` 少前导 `0` 为 31 位；`File` 与 `FileComposition` 为 `AAAD` + 掉前导 `0` 的带连字符 v4 GUID）在 `Export.dmp` 全表与原字节、以及 5 个 `.pid` 原字节中全部 **0 命中**；`DocVersionDate`（`2026/06/22` / `2026/07/06`）与 `FilePath`（`\\Glpid-7\pidsq\PID`）在库和 `.pid` 中同样 **0 命中**。
  - 提交 `4866b81` 已用 `derive_meta_uid` 写出 3 个对象 + 3 条关系（`DocVersionDate=""`、`FilePath=""`），结构与固定属性已对齐参考。`src/` 与 `publish\` 未改。

### N10 Q5–Q7 批复与 clippy 清理（2026-10-14，会话 gemini-4-argon-14）

- 用户批准 Q5–Q7 保持 `4866b81` 现行口径（`derive_meta_uid` 派生 6 个 32 位大写十六进制 UUID v5，`DocRevision="0"`、`DocVersion="1"`、`MajorRev_ForRevise="0"`，`DocVersionDate=""`、`FilePath=""`），`publish\` 交付件无需重写。
- 清理 `4866b81` / `7f1d891` 遗留的 3 条 clippy 告警：
  - `src/publish/pid_writer.rs`：`derive_meta_uid` 导入加 `#[cfg(test)]`，`write_pid_meta_xml` 文档注释改为 `` `derive_meta_uid` `` 与 `` `SmartPlant` ``（同时满足 `unused_imports`、`clippy::doc_markdown` 与 `rustdoc::private_intra_doc_links`）。
  - `src/backup/datamap.rs`：`attr` 内 `.map(|value| value.into_owned())` 改为 `.map(std::borrow::Cow::into_owned)`（消除 `clippy::redundant_closure_for_method_calls`）。
- 门禁验证（全部退出 0）：
  - `cargo clippy --locked -p pid-parse --lib --bins --tests -- -D warnings`：零告警
  - `cargo clippy --locked -p pid-parse --lib --no-default-features -- -D warnings`：零告警
  - `cargo clippy --locked -p pid-parse --example probe_oracle_exp_drawing_meta -- -D warnings`：零告警
  - `rustfmt --check --edition 2021 src/publish/pid_writer.rs src/backup/datamap.rs examples/probe_oracle_exp_drawing_meta.rs`：干净
  - `cargo rustdoc --locked -p pid-parse --lib -- -W missing-docs`：0 告警
  - `cargo check --locked -p pid-parse --lib --no-default-features --target wasm32-unknown-unknown`：通过
  - `cargo test --locked -p pid-parse --lib publish::`：237 passed / 0 failed；`cargo test --locked -p pid-parse --lib backup::`：107 passed / 0 failed

### N12 SQL Server MTF 整包入口落地（2026-10-19）

- `pid_publish_xml <备份目录> --out-dir` 对 `Manifest.txt` 目录里 MTF 形态的 `Export.dmp` 不再退出 2，与 Oracle `exp` 分支同一套目录遍历与 `_Index.json` 口径。链路：`backup::mtf::msda_stream_body` 取 MSDA 体 → `backup::mdf_page::detect_backup_stream_header_len`（原 `pid_backup_extract` 私有实现上提为库函数）剥备份流头 → `mdf_load::open_mtf_dump_as_sqlite` 落临时 MDF（`oxidized-mdf` 只接受路径）登台 → `T_Drawing` 逐行 `sqlite_load::load_drawing_graph` + `write_{data,meta}_xml` 写 `<NAME>_Data.xml` / `_Meta.xml`。`PlantData` zip 只列目录不解压；逐图失败不拖垮整包。`PublishError` 新增 `Io` 变体。
- `_Index.json`：`export = "SqlServerMtf"`、`pipeline_uids = "database"`（MTF 数据自带管线，不合成）、新增 `unmatched_pid_entries`（无 `T_Drawing` 行的 `.pid` 条目）；`BackupPublishReport` 增 `written`，CLI 摘要改报真实写出数。无 `backup-mdf` 的构建保持原退出 2 语义。
- TEST02_p 实测：`drawing_rows 1`（`A01`，UID `D9635C3C898840D1990B7E8BEE1D55DA`，`PATH \01\01\A01.pid`）发出，对象 12 / 值 29 / 关系 15 / `PIDPipeline` 1 / `PipingConnectors` 1；包路径产物与 `pid_publish_xml Export.mdf --drawing` 直发逐字节相同。12 个 `.pid` 条目里 11 个是 `Template Files/*.pid` 模板，全部进 `unmatched_pid_entries`。
- 测试：`tests/publish_from_pid.rs::sql_server_mtf_directory_publishes_t_drawing_rows`（夹具缺失 soft-skip）。验证：`publish_from_pid` 9/9、`backup_mtf` 8/8、`publish_mdf_load` 3/3、`publish_xml_cli` 20/20、`--lib` 1223/1223、`--no-default-features --lib` 861/861；`backup-mdf` 与 `backup-exp` 两种特性 check 均过；`clippy --lib -D warnings` 零告警、rustfmt 干净。
- 范围外保持原样：N11（符号库 / 模板 / `PlantConfig.xml` 内容解析）登记未做；集注站 4 张参考对照仍等那套库的备份。
