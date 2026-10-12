# N5：I井场端口从 Oracle exp 发出 · 开发计划（2026-10-12）

> 承接 happyrust/pid-parse#29 与 `a287078`（`docs/analysis/n5-exp-rows.md`，t-1831 已验收）。
> 用户 10-12 14:04「下一轮从 exp 发端口，先列任务给我看」；14:32「下一步要做什么，使用 plannator 制定计划」。
> 基线：pid-parse `a287078`（`codex/phase32c-bundle-closeout`，与 origin 同步）。工作树上别的会话的未提交文件（pid-plot、dash-scale、line2 等）一字不碰。
> 编写：协同组 publish 指挥官 opus-5-5-15（接 grok-4.7-55 / grok-4.7-65）。§1 的数字是 10-12 16:4x 在参考 XML、`publish\` 和 dump 上实测的。

## 一句话

I井场参考 3 093 个对象，今天 `.pid` 模式发 1 735 个，错 0。差的 1 358 个里 1 277 个是端口（管道 362、信号 915）。它们的父项 310 个今天已经全部发出；点号、流向、公称直径都在 dump 的 `T_PIPINGPOINT` / `T_SIGNALPOINT` 里，t-1831 已逐条对上。
本轮把这 1 277 个端口和 1 277 条 `PPC` / `SPC` 发出去：I井场对象 1 735 → 3 012、值 1 530 → 3 182、关系 1 971 → 3 248，错仍为 0。另外 4 张图不在这份 dump 里，本轮不动。

## 1. 现状

### 1.1 I井场还差什么（参考 vs `publish\` 已发）

| 项 | 参考 | 已发 | 差 | 本轮 |
|---|---:|---:|---:|---|
| `PIDSignalPort` | 915 | 0 | 915 | 发 |
| `PIDPipingPort` | 362 | 0 | 362 | 发 |
| `SignalPortComposition` | 915 | 0 | 915 | 发 |
| `PipingPortComposition` | 362 | 0 | 362 | 发 |
| `PIDPipeline` / `PipingConnectors` | 79 / 106 | 0 / 0 | 79 / 106 | 不发（§6 N6） |
| 连通：`PipingTapOrFitting` 369、`PipingEnd1Conn` 129、`PipingEnd2Conn` 143、`SignalEnd1Conn` 38、`SignalEnd2Conn` 46、`SignalTapOrFitting` 13 | 738 | 0 | 738 | 不发（§6 N7） |
| `EquipmentComponentComposition` / `PBSItemNotes` | 7 / 6 | 0 / 0 | 7 / 6 | 不发（§6 N8） |
| `PIDBranchPoint` / `DrawingItems` | 2 / 902 | 0 / 900 | 2 / 2 | 不发（与 `_Coverage.json` 按设计不发的 2 个 OPC 同数） |

### 1.2 端口要的东西都在 dump 里（t-1831 实测）

- 参考端口上的值只有三种：`IObject.Name`（1 277 个端口全有）、`IConnection.ConnectionFlowDirection`（管道 152）、`IPipeCrossSectionItem.NominalDiameter`（管道 223），共 1 652 个。接口集是固定的：管道端口 6 个接口，信号端口 7 个（见 N5-2）。
- `T_PIPINGPOINT` 9 628 行、`T_SIGNALPOINT` 22 057 行。UID = `SP_PLANTITEMID` + `.` + `*POINTNUMBER`：管道 362/362、信号 915/915，错 0；`Name` = 点号，错 0。
- 父项：管道 183 个、信号 127 个，**今天的 `.pid` 模式已全部发出**（183/183、127/127），端口不会挂空。
- 流向：152 对上、210 两边都空、0 不符。I井场只出现序号 2 和 3。
- 公称直径：223 两边都有、139 两边都空。16 个代码，规则（小于 5000 → `{code} mm`；大于 5000 → `(code − 5000) / 32` 英寸）命中 223、落空 0。

### 1.3 另外 4 张图不在这份 dump 里

4 张集注站图的 `DocUID` 在 dump 里按 UTF-16BE 和 ASCII 搜都是 0 处；它们参考端口的父项（管道 + 信号：33 + 15、28 + 15、37 + 27、89 + 57）在 dump 里一个都没有。dump 的 `T_RELATIONSHIP` 有 51 张图，但不含这 4 张。要发它们的端口，得有集注站那套库的备份（§3 Q5）。

## 2. 范围

**做**：N5-0 收 N4 的文档尾巴；N5-1 把 exp 行读进库；N5-2 发 I井场端口与 `PPC` / `SPC`，重新交付。
**不做**：管线、连通、设备↔管口、注释（§6，各自先出对照）；另外 4 张图；`imp`、连 Oracle；改 `examples/oracle_exp_schema.rs`、`examples/probe_oracle_exp_rows.rs`；不给 dump 时的 `.pid` 模式输出，一个字节都不变；不 push（等你说）。

## 3. 要你拍板的

| # | 问题 | 选项 | 推荐 |
|---|---|---|---|
| Q1 | dump 怎么进导出器 | (a) `pid_publish_xml <file.pid> --oracle-exp <Export.dmp>`，不给就是今天的 `.pid` 模式；(b) 在 `.pid` 旁边自动找 dump | **(a)**：显式；`.pid` 模式和五组钉数不受影响 |
| Q2 | 本图的端口行按什么挑 | (a) 父项 = 本 `.pid` 已发出的物项 UID；(b) 父项在参考 XML 里 | **(a)**：(b) 是拿答案键挑行。今天两者结果相同（310/310 父项已发）；(a) 另报「父项已发、参考里却没有」的多发数，非 0 就停下报你 |
| Q3 | 出处口径 | (a) `PidValueSource` 加一种「库备份行」（表 + 列），`_Coverage.json` 单独数；(b) 记成 `DerivedByNativeRule` | **(a)**：值来自库备份，不来自 `.pid`，也不是规则推出来的；UID 和 `PPC` / `SPC` 的拼法仍引 n2a 页的原生规则 |
| Q4 | 交付目录 | (a) `publish\` 里 I井场三件改用 `--oracle-exp` 重写，`_Coverage.json` 记 dump 路径与计数；其余 4 张不变；(b) `publish\` 保持纯 `.pid`，exp 版另写到 `publish-exp\` | **(a)**：目标是对上参考；出处在 `_Coverage.json` 里分得开 |
| Q5 | 另外 4 张图 | (a) 你能拿到集注站那套库的备份就给路径，N5 之后照同一套办法补；(b) 4 张图的端口维持 0 | 你定。只是告知，不影响本轮 |

## 4. 步骤

一条一单，一个提交。在线能派的 1 人（grok-4.7-77），按 N5-0 → N5-1 → N5-2 的顺序做。

### N5-0 收 N4 的文档尾巴（纯文档）

t-1490 交付后，工作树里留着三段没提交的 N4-1 文字：`CHANGELOG.md`、`task_plan.md`、`docs/plans/2026-10-11-publish-native-evidence-via-ida-bridge.md` §7（代码在 `b39ff7e` 已提交）。N4-2（`fa16824`）、N4-3（`cec947e`）、N4-4（`7fdeae8`）在这三处都没有记录。

- 核对三段 N4-1 文字与 `b39ff7e` 一致后原样保留；按这三个提交与 `docs/analysis/n2a-port-relation-filter.md` 补 N4-2 / N4-3 / N4-4 的 §7、`CHANGELOG.md`、`task_plan.md` 条目，只写能从提交和分析页核到的数，不重跑。
- 本计划文件（按你的批注改完）一起进这个提交。
- 验收：`git diff --cached --stat` 只有这 4 个文件。不跑 cargo，标 `NOT RUN：纯文档`。

### N5-1 exp 行读进库（不写 XML）

- 新文件 `src/publish/oracle_exp.rs`，和 `sqlite_load` / `mdf_load` 在同一个特性门下。把 `probe_oracle_exp_rows.rs` 钉住的行格式搬进库：`INSERT INTO` 之后按列长度前缀；`0xFFFE` 是 NULL；`0xFFFF` 是表尾；NVARCHAR2 是 UTF-16BE；Oracle NUMBER 解码。先读 `APPLICATIONS` 把格式钉住，钉不住就返回错误，不往下读。
- 对外：把 `T_PIPINGPOINT`（`SP_PLANTITEMID`、`PIPINGPOINTNUMBER`、`FLOWDIRECTION`、`NOMINALDIAMETER`）和 `T_SIGNALPOINT`（`SP_PLANTITEMID`、`SIGNALPOINTNUMBER`）读成行结构，并按 Q2 给出一张图的端口行。
- 单测：手造的小 exp 片段，覆盖 NULL、UTF-16BE、NUMBER 样例（`80` = 0、`c1 02` = 1、`c2 1a 34` = 2551、`3e 64 66` = −1）、表尾、格式钉不住时报错。
- 集成测（dump 不在就 soft-skip）：全表 9 628 / 22 057 行；I井场挑出管道 362、信号 915，拼出的 UID 与参考错 0；报「父项已发、参考里却没有」的多发数，期望 0，非 0 停下报指挥官。
- 验收：上面的数字；`cargo test --locked --workspace --lib --bins --tests` 0 败；`--no-default-features` 0 败；clippy `-D warnings` 两种特性零告警；rustfmt；missing-docs 不升；wasm32 lib 过。`--all-targets` 若仍被别的会话未提交的 example 卡住，标 NOT RUN 并写原因。

### N5-2 发端口与 `PPC` / `SPC`，重新交付

- 装载：`load_publish_from_pid` 不变；新加一步，把 N5-1 的端口行挂到已装载的图上。
  - 端口 UID = 父 UID + `.` + 点号；`Name` = 点号。
  - 管道端口的 `ConnectionFlowDirection` 按 `docs/analysis/n2a-port-relation-filter.md` 的序号表，写成 `@{GUID}`：1 和 5 → `{23F94152-B87D-11D6-80D4-00A0C92487AF}`，2 → `{23F94146-B87D-11D6-80D4-00A0C92487AF}`，3 → `{23F9414C-B87D-11D6-80D4-00A0C92487AF}`，4 → `{BCB3F097-66CA-487B-9AF4-D8FCF18624A3}`；空就不写。
  - 管道端口的 `NominalDiameter` 按 §1.2 的规则，写法照参考（`1 in`、`0.75 in`、`50 mm`）；空就不写。
  - 信号端口只有 `Name`。
- 写出：`PIDPipingPort` 的接口是 `IObject` `IConnection` `IPipingPort` `IPipingConnection` `IPort` `IPipeCrossSectionItem`；`PIDSignalPort` 是 `IObject` `IConnection` `ISignalConnection` `ISignalPort` `IPipingConnection` `IFacilityPoint` `IPIDTypical`。关系 `PPC-{端口}-{父}`（`PipingPortComposition`）、`SPC-{端口}-{父}`（`SignalPortComposition`），UID1 = 端口，UID2 = 父。
- CLI：`--oracle-exp <dump>`，`--out`、`--out-dir`、`--diff-against` 都认；`--diff-against` 的 clean 提示注明 exp 模式。不给 `--oracle-exp` 时输出逐字节不变。
- `_Coverage.json`：加管道端口 / 信号端口数、两种组成关系数、Q3 的出处计数；`left_out_by_design` 里端口那一句在 exp 模式下改写。
- 测试：`tests/publish_from_pid.rs` 的 `.pid` 模式五组钉数不动；新加 I井场 exp 模式一组，钉对象 3 012、值 3 182、关系 3 248，错类 / 错值 / 多发都是 0（dump 不在就 soft-skip）；`--diff-against --oracle-exp` 退出 0。
- 交付（按 Q4）：I井场三件重写到 `D:\work\cad\PID自研软件验证\publish\`，另外 4 张不重写。
- 文档：`n5-exp-rows.md` 加「导出器实测」一节；本计划 §7；`CHANGELOG.md`；`task_plan.md`；#29 加一条评论贴提交号和数字。
- 验收：I井场端口 1 277 个、`PPC` / `SPC` 1 277 条，与参考错 0；另外 4 张图的 `_Data.xml` 与本单之前逐字节相同；全量门同 N5-1。

## 5. 风险

- **dump 与参考不是同一时刻**：dump 是 2026-07-19 的备份，I井场参考是 06-22 发布的。N5-1 的多发数就是为这个准备的：非 0 先停，按行列给你看，不拿参考去删行。
- **流向只验过 2 和 3**：I井场没出现 1、4、5。照 n2a 的表写，分析页注明本图只实测了 2 / 3。
- **公称直径规则只见过 16 个代码**：新代码照规则算；算出来的写法和参考的写法（小数位、分数）对不上时就不写，并计数。
- **共享工作树**：只碰 §4 列出的文件；只 `git add` 明确路径；改前拿写锁，改完还；cargo 目录锁被占就等。别的会话未提交的 example 会让 `--all-targets` 编不过，照旧标 NOT RUN。
- **内存**：dump 74 MB 一次读入，测试里只读一次。

## 6. N5 之后（不在本轮，供你排序）

dump 里有这些表：`T_PIPELINE`、`T_PIPERUN`、`T_PLANTITEMGROUP`、`T_CONNECTOR`、`T_NOZZLE`、`T_ITEMNOTE`、`T_NOTE`、`T_OPC`、`T_MODELITEM`、`T_REPRESENTATION`。每一条都照 t-1831 的路子走：先出探针对照，你看过数再开发。

- **N6 管线**：I井场 `PIDPipeline` 79、`PipingConnectors` 106。候选表 `T_PIPELINE` / `T_PIPERUN` / `T_PLANTITEMGROUP`。
- **N7 连通**：I井场 738 条（`PipingTapOrFitting`、管道 / 信号的 `End1Conn` / `End2Conn`、`SignalTapOrFitting`）。两端多是端口，**依赖 N5**。t-1831 已证实 `T_RELATIONSHIP` 里没有；候选 `T_CONNECTOR`。
- **N8 小类**：`EquipmentComponentComposition` 7（候选 `T_NOZZLE`）、`PBSItemNotes` 6（候选 `T_ITEMNOTE`）；那 2 个 `PIDBranchPoint` 和 2 条 `DrawingItems` 可以顺带用 `T_OPC` 查。
- N5 到 N8 都做完，I井场的上限是对象 3 091 / 3 093、关系 4 105 / 4 107，差的就是那 2 个 OPC。

## 7. 执行记录

### 开单（2026-10-12，会话 opus-5-5-15，publish 指挥官）

- 接 grok-4.7-55（用户 14:32 那句之后掉线，计划没写）与 grok-4.7-65（16:26 建组后即离线）。核对：池子 21/21 完成；仓库 `a287078` 与 origin 同步；#29 开着；工作树里 N4-1 的三段文档没提交（→ N5-0）。
- §1 的数字：临时 Python 脚本只读 5 份参考 `_Data.xml`、`publish\` 下 5 份 `_Data.xml` 和 `Export.dmp`，脚本不入库。
- 没改 `src`，没跑 cargo，没提交。

### N5-1 端口行读进库（2026-10-12，任务 t-1966）

用户已通过，Q2 选 (a)：父项是本 `.pid` 已发出的物项，不读参考 XML 来挑行。`src/publish/oracle_exp.rs` 在 `publish` 模块里登记，和 `sqlite_load` 同一个 `backup` 特性门。不写 XML，不改 `pid_load` 的发出口径、`pid_writer`、CLI。

- `APPLICATIONS` 第一行不是 `ID` 1、`SmartPlant PID`、`SPPID` 就返回 `FormatUnpinned`，端口表不读。字符列字符集不是 2000 或形式不是 2 就报错。
- 全表 `T_PIPINGPOINT` 9 628 行、`T_SIGNALPOINT` 22 057 行，点号都能解成整数。I井场按已发物项挑出管道 362、信号 915。UID（父 UID + `.` + 点号）与参考 `PIDPipingPort` / `PIDSignalPort` 错 0。父项已发、参考里却没有：0。
- 手造片段的单测盖了 NULL、UTF-16BE、`NUMBER`（`80` = 0、`c1 02` = 1、`c2 1a 34` = 2551、`3e 64 66` = −1）、表尾、`APPLICATIONS` 钉不住。I井场对照在同一个模块的 `ijingchang_ports_match_the_reference`：dump、`.pid` 或参考 XML 缺一就 soft-skip。没有另建 `tests/publish_oracle_exp.rs`，因为 `Cargo.toml` 里别人未提交的改动还在，而显式 `[[test]]` 关掉了自动发现。
- **验证**（2026-10-12 实跑）：`cargo test --locked --workspace --lib --bins --tests` 0 败（1 538 过 / 3 忽略，`--lib` 1 184，`publish_from_pid` 4 / 4，钉数不动）；`--no-default-features` 0 败（1 094 过 / 1 忽略，`--lib` 850）。clippy `-D warnings` 两种特性在 lib / bins / tests 上零告警。改动的 `.rs` rustfmt 干净。`cargo rustdoc --lib --locked -- -W missing-docs` 0。wasm32：`cargo check --lib --locked --no-default-features --target wasm32-unknown-unknown` 过。`--all-targets` 测试 NOT RUN：`--no-run` 退出 0，未提交的 example 没有挡住编译，本单不跑那些探针。

### N5-2 发出 I井场端口（2026-10-12，任务 t-1967）

用户已通过，Q1 / Q3 / Q4 都选 (a)。`load_publish_from_pid` 不改。`--oracle-exp <dump>` 把 N5-1 的行挂到已发出的父项上再写 XML。不给这个参数时输出与原来逐字节相同。

- 端口 UID = 父 UID + `.` + 点号，`Name` = 点号。流向写成 `@{GUID}`，本图只实测了序号 2 和 3（1 / 4 / 5 的写法在表里，这张图没有）。公称直径按 §1.2：小于 5000 → `{代码} mm`，大于 5000 → `(代码 − 5000) / 32` 英寸。对不上的不写，计入 `nominal_diameter_unspelled`，这张图是 0。信号端口只有 `Name`。出处 `PidValueSource::BackupRow { table, column }`。
- 关系 `PPC-{端口}-{父}`、`SPC-{端口}-{父}`。`_Coverage.json` 在 exp 模式下多了 `oracle_exp`（dump 路径、端口数、组成关系数、库备份行数、没拼出来的公称直径），并把 `left_out_by_design` 里端口那一句改成「从 exp 发出」。
- I井场：对象 3 012、值 3 182、关系 3 248，错类 / 错值 / 多发 0。管道 362、信号 915，`PPC` 362、`SPC` 915。`--diff-against --oracle-exp` 退出 0，clean 行写 oracle-exp mode。另外 4 张图的 `publish\` 三件没有重写。
- **验证**（2026-10-12 实跑）：`pid_publish_xml` 对 I井场 `--oracle-exp D:\work\cad\pid-test-data\Export.dmp --diff-against` 参考 `_Data.xml` `--plant P01` 退出 0。stderr：`clean in oracle-exp mode (3012 objects, 3182 values, 3248 rels)`。stdout：extra 0、wrong class 0、different 0、not in reference 0、rels not in reference 0。`publish_from_pid` 6 过 / 0 败，含五组 `.pid` 钉数和 I井场 exp 这一组。已在 `publish\` 的 I井场三件与这次写到临时目录的输出 SHA256 相同，时间戳仍是 20:51:39，没有再重写。另外 4 张图的三件时间戳仍是 11:40:21。`cargo test --locked --workspace --lib --bins --tests` 退出 101：1 556 过 / 1 败 / 3 忽略，`--lib` 1 185 过。那 1 败是别人未提交的 `pid-plot/tests/write_five_drawings.rs` `five_drawings_round_trip_through_pdf`（低温分离器 A span 37 对 36），本单没改 pid-plot。`--no-default-features` 的 `-p pid-parse --lib --bins --tests` 0 败（1 094 过 / 1 忽略）。clippy `-D warnings`：`-p pid-parse` 两种特性在 lib / bins / tests 上零告警；`--workspace` 同范围退出 101，告警是已提交的 `pid-plot/src/compare.rs:794` 未使用的 `template_masks`（`150ab41`），不在本单路径里。改动的 5 个 `.rs` `rustfmt --check --edition 2021` 干净。`cargo rustdoc --lib --locked -p pid-parse -- -W missing-docs` 0。`cargo check --lib --locked -p pid-parse --no-default-features --target wasm32-unknown-unknown` 过。`--all-targets` 测试 NOT RUN：`cargo test --locked --workspace --all-targets --no-run` 退出 0，未提交的 example 没有挡住编译；那些 example 仍是别的会话的文件，本单不跑。
