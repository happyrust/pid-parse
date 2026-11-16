# 阶段 2 开发计划：数据库线的降级看得见（2026-11-16）

> 承接：`docs/plans/2026-11-16-publish-review-followup.md`（已批准）的阶段 2，对应审核第 2 条。本文把那里的 T2.1–T2.3 照现在的代码写细，补上当时没有的读数（§1），并列出与原计划不同的四处（§2）。
> 已经定了、本文不再问的两件：默认只告警，带 `--strict` 才退出 1（原计划 Q3）；`_Coverage.json` 多出的那一段恒有，没有降级时各项为 0（原计划 T2.3）。
> 起点：分支 `publish-review-followup-20261116`，阶段 1 的两个提交 `d65dc74`、`acc3f09` 之后；检查点 1 已过（八道门禁和对照矩阵，记录在主计划 §9，`180bee8`）。文中行号以 `acc3f09` 为准。
> 状态：2026-11-16 批准（Plannotator 评审页，06:05，决定是 approved，没有附言）。§7 的三个问题批准时没有选，按各题的推荐执行；§4.2 里暂定的名字没有人提出要改，就这样定了。进度记在主计划 §9。

## 一句话

数据库线（SQL Server 备份包、`.mdf`、`.sqlite`）碰到没有专用写手的条目类型，会写一个只有 UID 的 `<PIDItem>` 顶上，退出码是 0，stderr 和报告里都没有记录。照真实工厂的库算，这样的对象约占 8%：53 张图、15 439 个对象里有 1 283 个，49 张图有份。阶段 2 不改写出的 XML，只做三件事：数出来；说出来（stderr、`_Coverage.json`、`_Index.json`）；要求严格时退出 1（`--strict`）。

## 1. 现状

### 1.1 三种降级

| 降级 | 发生在哪 | 写出来的 | 丢了什么 |
|---|---|---|---|
| 占位对象 | `write_business_objects`（`src/publish/xml_writer.rs:164–173`）：目录行没有 `db_writer`，或类型不在目录里，就用 `*` 行的 `run_generic`（`catalog.rs:201`），它调 `write_generic_object`（`xml_writer.rs:1945`） | 一行注释加 `<PIDItem><IObject UID="…"/></PIDItem>` | 类名和全部属性。它的图形表示和两条推导关系（`DwgRepresentationComposition`、`DrawingItems`）照写 |
| 兜底关系 | `classify_relationship`（`xml_writer.rs:2591`）的 `_ => "Relationship"`：两端的类型对不在点名的三对里，或有一端不在本图里 | `<Rel DefUID="Relationship" …>` | 关系的定义 |
| 猜端别 | 同一个函数：`PipeRun` 对 `Nozzle` 一律判成 `PipingEnd1Conn` | `<Rel DefUID="PipingEnd1Conn" …>` | 没丢东西，但是 1 端还是 2 端是猜的 |

没有专用写手的类型：`Exchanger`（`catalog.rs:334`）、`Mechanical`（`:347`）、`EquipmentOther`（`:386`）、`OPC`（`:398`），再加目录里没有的任何类型。点名的三对关系是 `Nozzle`–`Vessel`、`PipeRun`–`Nozzle`、`PipeRun`–`Pipeline`。关系行有一端为空、或两端都是图形表示的，写手直接跳过（`xml_writer.rs:2525–2540`），不算降级。

目录里标了 `provisional` 的两行（`BranchPoint`、`PipingBranchPoint`）有写手，只是没有拿参考对过。它们不算降级，本阶段不记。

### 1.2 现在哪里都不说

- **命令行单图**（`src/bin/pid_publish_xml.rs:475` 的 `run`）：stderr 只有一行 `Loaded drawing … objects, … representations, … relationships`。退出码由 `--diff-against` 决定，不带参考就是 0。
- **SQL Server 整包**（`src/publish/backup_package.rs:1098` 的 `publish_mtf_package`）：`_Coverage.json` 由 `write_database_coverage_json`（`pid_writer.rs:784`）写，只有 `source`、`drawing`、`template`、`path`、`files`、`document_counts` 六项；`_Index.json` 里这张图的 `problems` 是空的；返回的 `BackupPublishReport`（`:59`）没有对应字段。
- **导出包**（`src/export_bundle.rs:359` 的 `export_bundle_publish_xml`）：`publish/status.json` 写 `state: "written"`。这个入口上次审核没有看（审核范围是 `src/publish/*` 和命令行），它走的是同一个写手（`:372–379`）。
- 对照：`.pid` 线的 `_Coverage.json` 有 `items_left_out_without_a_class` 和逐条的 `left_out`（`pid_writer.rs:601–652`），漏掉什么是写明的。
- 带 `--diff-against`、而且参考里有这些对象时，差异报告会把占位显示成差异。不带参考时完全静默。

### 1.3 读数

| 看的是什么 | 结果 |
|---|---|
| 仓库里唯一的 SQL Server 库（`TEST02_p` 的 `Export_v2.sqlite`） | 1 张图，4 个条目（`Vessel`、`Nozzle`、`PipeRun`、`PipingPoint` 各 1），6 个图形表示，3 行关系：2 行两端都是本图的图形表示，1 行一端为空 |
| 真实工厂的库（Oracle 包 `Export.dmp`，53 张图）里的条目，照数据库线的载入规则算 | 15 439 个对象，没有写手的 1 283 个：`OPC` 952、`EquipmentOther` 225、`Mechanical` 94、`Exchanger` 12。53 张图里 49 张有份，最多的一张 476 个 |
| 同一个库的关系表（`T_RELATIONSHIP`） | 24 664 行：22 353 行两端都是本图的图形表示，2 311 行有一端为空。以条目作端点的行是 0 |
| 对照矩阵的全部输出 | 323 份 XML 里 `<PIDItem>` 0 个，`DefUID="Relationship"` 0 条 |
| SmartPlant 自己发布的参考 XML | 14 份、28 384 条关系：`<PIDItem>` 0 个，`DefUID="Relationship"` 0 条。其中 11 份真实图里有 `PIDHeatTransferEquipment` 4 个、`PIDMechanicalEquipment` 15 个、`PIDProcessEquipment` 71 个 |

读数都是只读脚本数的，脚本在 `target/publish-audit/review/`（`db_shape.py`、`rel_ends.py`、`ref_scan.py`），没有入库。第二、三行的做法：把 `examples/probe_oracle_exp_connectivity.rs` 里解表的那一段移植成脚本，读 `T_DRAWING`、`T_REPRESENTATION`、`T_MODELITEM`、`T_RELATIONSHIP` 四张表（114 381 行，每行的分隔和表尾标记都对得上；`T_DRAWING` 53 行与整包的 53 张图一致；图形表示指向的条目都在 `T_MODELITEM` 里）。「对象」照 `load_drawing_graph`（`sqlite_load.rs:611`）的规则取：一张图的图形表示所指向的条目。输出留了一份在 `target/publish-audit/review/runs/rel-ends-export-dmp.txt`。

从这些数得到三点：

1. **能直接喂给数据库线的真实数据只有 TEST02 那一张图，它不触发任何降级。** Oracle 包走的是另一个载入器，喂不进数据库线；上表第二、三行是照规则算的，不是跑出来的。所以阶段 2 的行为只能用现造的小库来验，真实数据能验的只有「没有降级时一个字节都不变」。
2. **占位是常态，不是特例。** 同一个工厂如果以 SQL Server 备份交来，53 张图里 49 张会有占位，合计 1 283 个对象只剩一个 UID。SmartPlant 自己从不发布 `<PIDItem>`，换热器、机械设备、其他设备在参考 XML 里各有自己的类。`.pid` 线对 64 张真实图的覆盖报告是同一个量级（换热器 13 个、机械设备 91 个、其他设备 229 个）。
3. **关系那两种降级，手上的库走不到。** 两个库（3 行、24 664 行）没有一行以条目作端点，`classify_relationship` 一次都轮不到。这张表连的是图形表示，按条目类型判定义的那段代码是照「两端是条目」写的。别的工厂是不是也这样，手上的数据证明不了。

## 2. 与已批准计划不同的地方

| # | 原计划 | 现在 | 为什么 |
|---|---|---|---|
| 1 | 两个入口：单图命令行、整包 | 加第三个：导出包（T2.4） | 它走同一个写手，不记的话同一张图从这里出去仍然是静默的 |
| 2 | 关系记两个数：兜底的按类型对计数，猜端别的计条数 | 合成一个数：关系表里被写成 `<Rel>` 的行，带两端的类型和写成的定义 | §1.3 第 3 点：这样的行在真实库里是 0，出现一行就值得看，不必再分是兜底的还是猜的 |
| 3 | T2.3 只做纯函数单测，说整包这一支「只能这样验」 | 把暂存之后的那一段拆成一个函数，用现造小库把整包这一支从头到尾跑一遍 | 否则「汇总写进三个地方」这段接线没有任何用例走到 |
| 4 | 检查点 2 的矩阵只有 `_Coverage.json` 变 | 打印用法说明的那几项也会变（`--help`，和按用法错误退出 2 的项） | 用法说明加了 `--strict` 一段，删了一句走不到的话；用法错误会把整段用法说明打到 stderr |

第 1、2、3 处各对应 §7 的一个问题；批准时三处都按推荐留下了。

## 3. 不做的

- **不补这 8%。** 阶段 2 只让占位看得见，不让它消失。要补有两条路：给 `Exchanger`、`Mechanical`、`EquipmentOther`、`OPC` 各写一个数据库线的写手；或者让 SQL Server 整包也走 `.pid` 线（包里本来就带着每张图的 `.pid`），像 Oracle 整包那样拿库里的表做增补。两条路都要一份含这些设备的 SQL Server 包、加上 SmartPlant 对同一张图发布的 XML 才能验，手上没有；第二条路的工作量也没有调研过。阶段 2 做完以后，拿到这样的包跑一遍就有逐图的数字，到时候再定。
- **不改写出的 XML。** 占位照写，注释照写，关系判成什么不变。`_Data.xml`、`_Meta.xml` 在任何输入下逐字节不变。
- **不动 `.pid` 线和 Oracle 整包。** 输出逐字节不变。
- **不改默认的退出码。** 不带 `--strict` 时，今天退出 0 的仍然退出 0。
- **不给 `_Index.json` 加新键。** SQL Server 和 Oracle 两支的索引是同一个形状（`backup_package.rs:1319–1344` 里那些恒为 0 的键就是为了这个），句子放进已有的 `problems`。
- **不删 `classify_relationship`。** 它在真实库上走不到（§1.3），但删不删是另一件事，本阶段只记下这个事实。

## 4. 设计

下面按 Q1 的推荐项写。Q1 选别的，差别写在那道题里。

### 4.1 判定只写一处，写手和记账共用

- **对象：** `catalog.rs` 加 `dedicated_db_writer(item_type) -> Option<DbWriterFn>`。`write_business_objects` 用它，取不到就用占位写手；记账也用它，取不到就记一笔。
- **关系：** 「关系表的哪些行会写成 `<Rel>`、各写成什么定义」收进一个迭代器，跳过的规则和对 `classify_relationship` 的调用都在里面。写手照它写，记账照它数。`classify_relationship` 本身不改。
- 写手改完输出不变，对照矩阵逐字节比。

### 4.2 汇总和它的句子

```rust
/// 一张图在数据库线上写得不完整的地方。
#[non_exhaustive]
pub struct DatabaseShortfalls {
    /// 占位对象：条目类型 → 个数
    pub placeholder_objects: BTreeMap<String, usize>,
    /// 关系表里只凭两端的条目类型定出定义的行：
    /// （Item1 的类型, Item2 的类型, 写成的定义）→ 行数
    pub relationship_rows_by_item_type: BTreeMap<(String, String, String), usize>,
}

pub fn database_shortfalls(drawing: &PublishDrawing) -> DatabaseShortfalls;
```

另有 `is_empty()` 和 `describe()`。`describe()` 每类给一句英文（命令行的输出都是英文），三个入口用同一份：

```text
3 objects written as <PIDItem> placeholders, class and attributes left out: Exchanger 2, Mechanical 1
2 relationship rows defined from the item types of their ends alone: Nozzle -> Exchanger as Relationship 2
```

端点不在本图里的，类型记成 `(not in the drawing)`。名字（`DatabaseShortfalls`、`shortfalls`、`--strict`）批准时没有人提出要改，就这样定了。

### 4.3 `--strict`

- 数据库输入（`.mdf`、`.sqlite`）和目录输入接受它。`.pid` 输入带它、或者和 `--list-drawings` 一起用，按用法错误退出 2。这是照 `--style` 的先例：不适用就拒绝，不默默忽略。
- 汇总不为空时：文件照写，差异报告照出，最后退出 1，stderr 多一行说明是 `--strict` 让它退出 1 的。
- 目录是 Oracle 包时没有数据库线的图，开关不起作用，用法说明里写明。命令行要读完包才知道它是哪一种，所以这里不拒绝。
- 退出码的含义不变：0 干净，1 有发现或失败，2 用法错误。

### 4.4 写到哪些地方

| 入口 | stderr | 文件 | 返回值 | 带 `--strict` |
|---|---|---|---|---|
| 单图命令行 | 每类一行 `warning: …` | 不变 | — | 退出 1 |
| SQL Server 整包 | 有降级的图每类一行 `warning: <图名>: …` | `_Coverage.json` 多 `shortfalls` 一段（恒有）；`_Index.json` 里这张图的 `problems` 和顶层 `problems` 各加句子 | `BackupPublishReport` 多一个字段 | 整包退出 1 |
| 导出包 | —（库函数，不打印） | `publish/status.json` 多 `shortfalls`（句子的数组；写出了 XML 就有这个键，没有降级时是空数组） | `ExportBundlePublishStatus` 多一个字段 | — |

`_Coverage.json` 多出的那一段，有降级时：

```json
"shortfalls": {
  "placeholder_objects": { "total": 3, "by_item_type": { "Exchanger": 2, "Mechanical": 1 } },
  "relationship_rows_defined_by_item_type": {
    "total": 2,
    "rows": [ { "item1": "Nozzle", "item2": "Exchanger", "written_as": "Relationship", "count": 2 } ]
  }
}
```

没有降级时（A01 就是这样）两个 `total` 都是 0，`by_item_type` 是 `{}`，`rows` 是 `[]`。图仍算写出，`failed` 不变。

## 5. 任务

每个任务一个提交，不 push。每个任务都先写用例、看它红，再改到绿；提交前跑 clippy 和 fmt，跑一遍对照矩阵。任务说明里没有写「输出会变」的，矩阵必须一个字节不差。

| 任务 | 内容 | 规模 | 写出的文件 | stderr 和退出码 |
|---|---|---|---|---|
| T2.1 | 写手侧记账 | M | 不变 | 不变 |
| T2.2 | 单图命令行说出来，`--strict` | S | 不变 | 有降级时多 1–2 行告警；用法说明变；不带 `--strict` 退出码不变 |
| T2.3 | 整包记下来 | M | SQL Server 包的 `_Coverage.json` 多一段 | 有降级时多告警行 |
| T2.4 | 导出包也记（Q3） | S | 导出包的 `publish/status.json` 多一个键 | — |

顺序：T2.1 → T2.2 → T2.3 → T2.4。原来想放在最前面的「先量一次真实工厂的关系表」已经量了，结果在 §1.3，不再列成任务。

### T2.1 写手侧记账

- **先写用例**（`database_shortfalls` 先留一个返回全空的桩，看前三条因断言失败而红）：
  1. 合成一张图：`Exchanger` 2 个、`Mechanical`、`EquipmentOther`、`OPC` 各 1 个、一个目录外类型 1 个，再加有写手的 `Vessel`、`Nozzle`。汇总里五种类型各自的个数对；合计等于 `write_data_xml` 的输出里 `<PIDItem>` 的个数。
  2. 合成六行关系：`Nozzle`–`Exchanger`、一端不在本图里的、`Nozzle`–`Vessel`、`PipeRun`–`Nozzle`、两端都是图形表示的、一端为空的。前四行各记一笔，类型对和写成的定义都对；合计等于输出里比不带这六行时多出来的 `<Rel>` 条数；后两行不计。
  3. 遍历目录，每一行没有 `db_writer` 的业务类型各造一个对象，汇总都要点到。以后目录加了类型而漏记，这条会失败；以后给某个类型补了写手，这条不用改。
  4. 只有 `Vessel`、`Nozzle`、`PipeRun`，关系行都是图形表示对图形表示的图：`is_empty()`。
  5. 真实的 A01（`Export.mdf`）：`is_empty()`。夹具不在时打印 `skipping:` 后跳过。
- **再改：** §4.1、§4.2。新的公有项都写文档（缺文档的门禁是只减不增的）。
- **验收：** 五条用例绿；`unsupported_item_types_still_fall_back_to_generic_placeholder`（`xml_writer.rs:4140`）不改仍然绿；对照矩阵逐字节不变。
- **验证：** `cargo test --locked -p pid-parse --lib publish::`。
- **文件：** `src/publish/xml_writer.rs`、`src/publish/catalog.rs`、`src/publish/model.rs`、`src/publish/mod.rs`，一个集成测试文件（第 5 条）。

### T2.2 单图命令行说出来

- **夹具：** 一份 SQL 脚本（建表加插入，几十行）：一张图，一台带图形表示的换热器，一个容器，一个管口，一行 `Nozzle`–`Exchanger` 的关系。命令行用例拿它在临时目录里建 `.sqlite`，T2.3 的单测拿它建内存库，建表的代码只有这一份。建表照 `sqlite_load.rs:1343` 那条单测的最小写法，缺哪张表、哪一列以载入报错为准补齐。
- **先写用例**（`tests/publish_xml_cli.rs`，在没改的代码上看红）：
  1. 现造的库，不带开关：退出 0；stderr 有占位那一句和关系那一句；写出的 XML 里 `<PIDItem>` 1 个。
  2. 同上带 `--strict`：退出 1；文件照样写出来了；stderr 有说明是 `--strict` 的那一行。
  3. A01 带 `--strict`：退出 0，stderr 没有这两类告警。
  4. `.pid` 输入带 `--strict`：退出 2。
  5. `--help` 里有 `--strict`，没有 `still exits 2`。
- **再改：** `CliOptions`（`pid_publish_xml.rs:43`）加 `strict`；`run` 在写完之后取汇总、打印、算退出码；用法说明（`:99`）补一段，删掉第 120–121 行那句走不到的话（这个可执行文件必须带 `backup` 特性才编得出来，没有「不带 `backup-mdf`」的情形）。
- **验收：** 五条用例绿；A01 的 stdout、stderr、退出码与基线相同（矩阵）。
- **验证：** `cargo test --locked -p pid-parse --test publish_xml_cli -- --nocapture`。
- **文件：** `src/bin/pid_publish_xml.rs`、`tests/publish_xml_cli.rs`、夹具 1 个。

### T2.3 整包记下来

- **先拆：** `publish_mtf_package` 里从「列出图纸」到「写 `_Index.json`」的那一段（`backup_package.rs:1115–1362`）原样搬进一个 crate 内部的函数，参数是已经暂存好的库连接。`publish_mtf_package` 只剩读包、暂存、调它。这一步单独看是纯搬动，矩阵逐字节不变。
- **先写用例**（`backup_package.rs` 的单测，用 T2.2 的夹具建内存库，包目录用一个空的临时目录）：
  1. 写出的 `_Coverage.json` 有 `shortfalls` 一段，数字与夹具相符；
  2. `_Index.json` 里这张图的 `problems` 和顶层 `problems` 都有那两句，`failed` 是 0，图算写出；
  3. 返回的报告里新字段有这张图；
  4. 没有降级的库：`shortfalls` 一段在，各项为 0，`problems` 为空。
- **再改：** `write_database_coverage_json` 自己从图里取汇总（它本来就拿着图，签名不变）；逐图循环把句子交给 `mtf_drawing_entry` 和顶层 `problems`，把汇总放进报告；命令行的 `run_package`（`pid_publish_xml.rs:558`）照 `chain_faults` 的先例（`:573`）往 stderr 说，带 `--strict` 时算进退出码。Oracle 那一支的新字段恒为空。
- **输出会变：** SQL Server 包的 `_Coverage.json` 多一段。
- **验收：** 四条用例绿；`TEST02_p` 整包真跑，除 `A01_Coverage.json` 多出的那一段外逐字节不变；Oracle 的两个整包（160 个文件的那个和 DWG 包）逐字节不变。
- **验证：** `cargo test --locked -p pid-parse --lib publish::backup_package`，`publish::pid_writer`；矩阵。
- **文件：** `src/publish/backup_package.rs`、`src/publish/pid_writer.rs`、`src/bin/pid_publish_xml.rs`。

### T2.4 导出包也记

- **先写用例：** 用 T2.2 的夹具建 `.sqlite`，调 `export_bundle_publish_xml`：返回的状态和 `publish/status.json` 里都有那两句；A01 的 `shortfalls` 是空数组。
- **再改：** `ExportBundlePublishStatus`（`src/export_bundle.rs:229`）加 `shortfalls: Option<Vec<String>>`。它有两处构造：写出了 XML 的那份（`:387`）是 `Some`，没有降级时是空数组；没有生成 XML 的那份（`write_publish_status`，`:866`）是 `None`，键不写出，免得把「没算过」读成「没有」。读旧文件时缺这个键也是 `None`，和这个结构里其它可缺的键一个写法。
- **输出会变：** 导出包的 `publish/status.json` 多一个键。
- **文件：** `src/export_bundle.rs`。用例加在它自己的测试模块里，挨着已有的 `export_bundle_publish_xml_writes_data_meta_and_success_status`（`:1673`）。这个函数的调用方是 `pid_inspect` 的导出包命令（`src/inspect/commands.rs:820`），它不用改。

### 检查点 2

- [ ] 全套八项门禁通过（约 33 分钟）
- [ ] 对照矩阵（约 3 分钟，加上带 `--strict` 的 A01 单图和 `TEST02_p` 整包两项）：变的只有两处，打印用法说明的那几项（`--help`，和按用法错误退出 2 的项）跟着用法说明变，`TEST02_p` 的 `A01_Coverage.json` 多出那一段；其余逐字节不变
- [ ] `CHANGELOG.md` 一条（写明两个公有结构各加了一个字段）、`AGENTS.md` 的用例表补上新用例
- [ ] §1.3 的读数、每个任务红与绿的输出记进主计划 §9，结果发给你

## 6. 风险

- **只能在现造的数据上验。** 能喂给数据库线的真实数据不触发任何降级（§1.3）。用例把汇总和写手的实际输出绑在一起（个数等于 XML 里的标签数），目录那条用例保证每个没有写手的类型都被点到；真实数据上验的是「什么都不变」。
- **1 283 是算的，不是跑的。** 它是照载入规则从 Oracle 包的表里数出来的。真拿一个这样的 SQL Server 包来跑，数字应当相同，但没有跑过。
- **两个公有结构加字段。** `BackupPublishReport` 和 `ExportBundlePublishStatus` 的字段都是公开的，外面如果有人照字段逐个构造它们，加字段后要跟着改。`chain_faults` 当初也是这样加的。
- **搬动 250 行。** T2.3 的拆分是纯搬动，靠 `TEST02_p` 的现有用例和矩阵逐字节比兜住。
- **命令行用例靠 `.sqlite` 输入。** 这个入口现在标着 deprecated（会多打一行告警，不影响断言）。哪天它被删掉，这几条用例要换入口，而 `.mdf` 没法现造。
- **告警的量。** 真实的 SQL Server 大包里，每张有降级的图最多 2 行，是按图按类合计的，不是每个条目一行。照 §1.3 的那个工厂算，是 49 行。

## 7. 要你拍板的

批准时三题都没有选，按各题的推荐定了，下面打了勾的就是。哪一题要改，在用到它的任务开始之前说一声就行：Q1 用在 T2.1，Q2 用在 T2.3，Q3 用在 T2.4。

:::question
Q1：关系那一类怎么记？

原计划记两个数：落到兜底定义的（按类型对），和只凭类型对猜端别的。量下来，手上两个库（3 行、24 664 行）没有一行以条目作端点，这两种情形一行都没有（§1.3）。别的工厂是不是也这样，证明不了。

- [x] 合成一个数 — 关系表里只要有一行不是「图形表示对图形表示」而被写成了 `<Rel>`，就记一笔，带上两端的类型和写成的定义，不再分是兜底的还是猜的。`classify_relationship` 不用改。带 `--strict` 时有这样的行就退出 1
- [ ] 照原计划记两个数 — `classify_relationship` 的返回值改成枚举，兜底的和猜端别的分开数；报告多一项，改动多一些。猜端别那一类带 `--strict` 时也退出 1
- [ ] 不记关系 — 只记占位对象，改动最小。哪天真有这样的行，仍然是静默的

Recommended: 合成一个数
:::

:::question
Q2：整包这一支怎么验（T2.3）？

带换热器的 SQL Server 包手上没有。要把「汇总写进 `_Coverage.json`、`_Index.json`、报告」这段接线真的跑到，只能把暂存之后的那一段拆出来，喂一个现造的小库。

- [x] 拆出来，端到端验 — 多一次约 250 行的纯搬动（同一个文件内，矩阵逐字节比），接线有用例走到
- [ ] 不拆，只做纯函数单测 — 改动更小，照原计划；接线本身没有用例走到，写错了只有等真实的包才会发现

Recommended: 拆出来，端到端验
:::

:::question
Q3：导出包这个入口要不要一并记（T2.4）？

`export_bundle_publish_xml` 走同一个写手，上次审核没有看到它。

- [x] 一并记 — 一个字段加一条用例；`publish/status.json` 多一个键
- [ ] 这次不动 — 这个入口保持现状，同一张图从这里出去仍然不说；在主计划里记一笔留待以后

Recommended: 一并记
:::
