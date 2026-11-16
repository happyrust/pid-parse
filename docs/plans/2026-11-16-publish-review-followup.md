# publish 导出审核后的整改计划（2026-11-16）

> 依据：2026-11-15 对 `src/publish/*` 与 `src/bin/pid_publish_xml.rs` 的只读审核，结论在 `docs/analysis/2026-11-16-publish-export-review.md`；本文 §2 把每条结论、证据位置和去向列全，单看本文就够。
> 承接：`docs/plans/2026-11-10-publish-export-completion.md`。那份计划里等现场数据的 S10、S12、S17 和只做记录的 S15、S16 不在本文范围内。
> 基线：`main` = `a4b490e`。审核在 `d5e3a0a`（`main` 加 6 个 pid-plot 提交）上做，`src/publish/`、`src/bin/pid_publish_xml.rs`、`vendor/oxidized-mdf/` 三处与 `main` 没有差异。
> 状态：2026-11-16 批准（Plannotator 审阅页，决定是 approved，没有附言）。§3 的五个问题批准时没有选，按各题的推荐执行。执行在工作区 `D:\work\plant-code\cad\pid-parse-wt\pub-r`、分支 `publish-review-followup-20261116` 上，进度记在 §9。

## 一句话

审核没有发现会让 `.pid` + exp 这条线写错内容的缺陷。要改的有三件：一个接错了入口的修复（8.3 短文件名，已实测为红）；数据库线写不出来的东西不说（4 类对象和目录外的一切发成 `<PIDItem>` 占位，退出码 0，报告里没有记录）；MDF 读取在大库上的时间和内存（每张表重开一次文件，页缓存不淘汰）。其余是六个小修和两处重构。对现有语料，除了数据库线的覆盖报告多一段，本计划不改任何写出文件的字节。

## 1. 现状读数

| 项 | 读数 | 怎么得到的 |
|---|---|---|
| clippy | 0 条告警 | `cargo clippy --locked -p pid-parse --all-targets -- -D warnings` |
| 库测 | 420 过，0 败 | `cargo test --locked -p pid-parse --lib publish::` |
| 集成测试 | 17 个 `tests/publish_*.rs`：133 过，0 败，2 ignored | 逐个文件跑 |
| 8.3 短名 | 长名退出 0、`different 0`；同一文件的短路径退出 1、`different 1`，图名写成 `IB031~1` | 见下面两条命令 |
| A01 数据库线 | 占位对象 0，兜底关系 0，关系 15 条 | `Export.mdf`（19.9 MB）`--drawing … --stdout` |
| 已交付的 `publish-v6` | 53 个 `_Data.xml`、186 993 个 UID，重复 0 个 | 逐文件数 `IObject UID` |

前三行是审核时用本机默认工具链（nightly 1.101，`--offline`）跑的；T0.1 会用 CI 的 stable 把全套门禁重跑一遍。

8.3 短名的两条命令（`target/debug/pid_publish_xml.exe`，语料在 `D:\work\cad\PID自研软件验证`，两条的 `--diff-against` 都指向同一份 `I井场 注采阀组工艺及自控流程图_Data.xml`，`--plant P01`）：

```text
输入 <语料>\I井场 注采阀组工艺及自控流程图.pid
  -> 退出 0   values 1530 (different 0, not in reference 0)

输入 D:\work\cad\PID~1\IB031~1.PID        （同一个文件的短路径）
  -> 退出 1   values 1530 (different 1, not in reference 0)
     container DocName: "IB031~1" reference "I井场 注采阀组工艺及自控流程图"
     value PIDDrawing 5633AF41… IObject.Name: "IB031~1" reference "I井场 注采阀组工艺及自控流程图"
```

`docs/baselines/publish-diff-accepted.json:323` 写的是短名「now resolves to the stored long name … the run is clean」，与第二条不符。

## 2. 审核结论与去向

| # | 级别 | 结论 | 证据 | 去向 |
|---|---|---|---|---|
| 1 | 应修 | 8.3 短名的修复加在 `load_publish_from_pid` 上，命令行走的是 `read_publish_from_pid`，没有生效 | `src/publish/pid_load.rs:831–832` 与 `:992`；`src/bin/pid_publish_xml.rs:646` | T1.1、T1.2 |
| 2 | 应修（既有） | 数据库线对没有专用写手的类型写 `<PIDItem>` 占位，对判不出的关系写 `DefUID="Relationship"`，退出码 0，stderr、`_Coverage.json`、`_Index.json` 都不记 | `src/publish/catalog.rs:334`、`:347`、`:386`、`:398`、`:414`；`src/publish/xml_writer.rs:164–173`、`:1945`、`:2591–2599`；`src/publish/pid_writer.rs:784` | T2.1–T2.3 |
| 3 | 建议优先（未量） | MDF 暂存每张表重开一次文件（24 次）；读取器只进不退、页缓存不淘汰、每页解析两遍；MTF 包整个 `Export.dmp` 进内存后再拷一份 | `src/publish/mdf_load.rs:62`、`:112`；`vendor/oxidized-mdf/src/lib.rs:459–522`；`src/publish/backup_package.rs:1106` | T3.1–T3.3 |
| 4 | 建议 | exp 增补的编排写了两份，各有一条 `retain` 规则 | `src/bin/pid_publish_xml.rs:658–790`（`:711`）；`src/publish/backup_package.rs:872–900`（`:888`） | T5.1 |
| 5 | 建议 | `attach_dump_only_components` 的 `known` 在循环里不更新，同一对象有两条表示时会推两次 | `src/publish/oracle_exp.rs:1386–1410` | T4.1、T4.2 |
| 6 | 建议 | 写文件不是原子的，三件套写到一半失败会留下半套 | `src/publish/backup_package.rs:914–940`；`src/bin/pid_publish_xml.rs:1160` | T4.3 |
| 7 | 建议 | zip 条目整个读进内存没有上限；dump 里每张表都从头找一遍 | `src/publish/backup_package.rs:474`；`src/publish/oracle_exp.rs:1866`、`:2435` | T4.4、T4.5 |
| 8 | 建议 | `publish_one` 难读：`uid_from` 是字符串且有两种拼法，失败记账重复约 8 处 | `src/publish/backup_package.rs:494–731`、`:1460` | T5.2 |
| 9 | 建议 | 暂存的表清单与目录表的子表清单是两份手写的，没有一致性检查；缺表静默跳过 | `src/publish/mdf_load.rs:20`；`src/publish/catalog.rs` 的 `db_subtables`；`src/publish/sqlite_load.rs:328` | T4.6 |
| — | 细节 | 序列化失败被吞成 `{}`、`_Meta.xml` 渲染三遍、`print!` 等十来处 | 见 T6.1–T6.3 | T6.1–T6.3 |

第 2 条的范围比审核时说的宽一点：没有专用写手的不只是 `Exchanger` 和 `Mechanical`，还有 `EquipmentOther` 和 `OPC`，再加目录里没有的任何类型。仓库里唯一的数据库线夹具 A01 一处都不触发（§1），所以这条路现在没有被任何用例走到。带 `--diff-against` 时门禁会把占位报成差异；不带参考时才是静默的。

## 3. 要你拍板的

批准时五题都没有选，按各题的推荐定了，下面打了勾的就是。哪一题要改，在用到它的任务开始之前说一声就行：Q1 已经用上，Q2 用在 T1.2，Q3 用在 T2.2，Q4 用在阶段 3，Q5 用在阶段 5。

:::question
这些改动在哪个分支、哪个工作区上做？

当前检出在 `export-closeout-20261115`（`main` 加 6 个 pid-plot 提交），已提交的 publish 代码与 `main` 相同。写这份计划时，这个检出里另有未提交的改动正在进行（`src/publish/stored_values.rs` 与 `src/bin/pid_publish_xml.rs`，给 dump 里的 `IObject.Name` 加写出规则，不是本计划的内容，我没有动）。以往的 publish 切片是在 `D:\work\plant-code\cad\pid-parse-wt` 下另开工作区、逐片过门后并回 `main`，不 push。

- [ ] 就在当前检出的 export-closeout-20261115 上做 — 少一步；这个分支若就是为这件事建的，选它
- [x] 另开工作区，从 main 切新分支 — 与这个检出里别的工作互不影响，照以往的做法逐片并回 main

Recommended: 另开工作区，从 main 切新分支
:::

:::question
I井场 那条用例要不要从「退出 0 或 1 都算对」收紧成「必须退出 0」？

`docs/baselines/README.md` 现在明文写着这条断言是析取、不要写成必须 clean。T1.1 修好之后，长名和它的 8.3 短名都得到长名，结果不再取决于路径是怎么传进来的；会退出 1 的只剩文件真名就叫 `IB031~1.pid` 的拷贝，它有自己的一条用例。

- [ ] 保持析取 — 只加真实短路径的用例，README 的规则不动
- [x] 收紧成必须退出 0 — README、基线说明、用例三处一起改，并加一段用真实短路径跑的检查

Recommended: 收紧成必须退出 0
:::

:::question
数据库线有占位对象或兜底关系时，退出码怎么定？

今天是退出 0，哪里都不记。`.pid` 线上相近的情形（断链）现在的处理是：图照写、stderr 告警、`_Index.json` 的 `problems` 记一句、退出码不变。

- [ ] 只告警，不加开关 — 改动最小；要卡住只能自己读报告
- [x] 默认只告警，带 --strict 才退出 1 — 与断链的处理一致，现有脚本不受影响，交付流程可以加开关卡住
- [ ] 默认就退出 1 — 最严；现有对 SQL Server 包的调用只要有一台换热器就会从 0 变成 1

Recommended: 默认只告警，带 --strict 才退出 1
:::

:::question
MDF 读取那一段（阶段 3）做到哪一步？

这一条是读代码得出的，没有量过；仓库里最大的 MDF 只有 19.9 MB，调试构建打开一次约 1.4 秒。T3.1 先量两个数：读入的字节是文件大小的几倍，峰值内存是文件大小的几倍。

- [ ] 这一轮不做 — 只把结论记进文档
- [ ] 先量，读数出来停下等你看
- [x] 先量，过线就接着改 — 线定在读入字节达到文件大小的 3 倍，或峰值内存达到 2 倍；过线就做 T3.2、T3.3，不再回来问

Recommended: 先量，过线就接着改
:::

:::question
两处重构（阶段 5）要不要做？

T5.1 把写了两份的 exp 增补编排合成一处：两份规则以后改一边忘一边，单图和整包的输出就会不一样。T5.2 只是把 `publish_one` 收拾得好读。两处都以对照矩阵逐字节不变为验收，放在最后，砍掉不影响前面。

- [ ] 都不做
- [ ] 只做 T5.1 — 去掉会走样的那一份，T5.2 不动
- [x] 两处都做

Recommended: 两处都做
:::

## 4. 做法上的决定

- **先红后绿。** 每个改行为的任务先有一条失败的用例或一次实测的红，输出记进 §9，再改；红和绿在同一个提交里。
- **降级只记账，不改写法。** 给 `Exchanger` 这四类补专用写手要有原厂参考，仓库里没有；这一轮只让占位和兜底看得见。
- **记账用写手自己的判定。** 「这个类型有没有专用写手」「这条关系判成了什么」各抽成一个函数，写手和记账都调它，不另写一套会走样的规则；再用一条用例去对 XML 里实际的个数。
- **vendored 读取器按页寻址。** `MdfDatabase::open(path)` 的签名不变；这是 GPL-3.0 的本地修改件，文件头的改动清单要补一行。
- **原子写 = 同目录临时名 + `rename`。** 三件套要么都在，要么都不在。
- **重构只认字节。** 两处重构的验收就是对照矩阵逐字节不变，不改任何断言。
- **不加新依赖。** 取短路径在测试里用 `cmd` 的 `%~sI`；`memchr` 已经在 `Cargo.lock` 里，用到时只在 `Cargo.toml` 点名。

## 5. 验收口径

1. 门禁照 `AGENTS.md` 的「Pre-commit gates」，五条加两条，用 CI 的工具链（stable）跑。本机默认是 nightly，`PATH` 上排第一的 `cargo` 也不是 stable 的，具体跑法见 §9 的 T0.1：

   ```bash
   cargo build --locked --workspace --all-targets
   cargo test  --locked --workspace --all-targets
   cargo clippy --locked --workspace --all-targets -- -D warnings
   cargo fmt --all -- --check
   bash .github/scripts/check-missing-docs.sh
   cargo test --locked -p pid-parse --no-default-features --lib
   cargo check --lib --locked -p pid-parse --no-default-features --target wasm32-unknown-unknown
   ```

   动了 `vendor/oxidized-mdf` 的任务另跑 `cd vendor/oxidized-mdf && cargo test --lib`。

   执行中补充（T0.1）：CI 在这些之后还有一步 `bash .github/scripts/check-byte-audit-baselines.sh`，检查点一并跑，见 §9。
2. 对照矩阵（T0.1 建立）每个任务之后重跑，逐字节比。任务说明里没有写「输出会变」的，矩阵必须一个字节不差。
3. 依赖语料的用例跑的时候带 `-- --nocapture`，输出里要看得到它真的跑了，不是跳过。
4. 每个任务一个提交，提交说明沿用仓库现有写法；不 push，除非你说。

## 6. 任务清单

| 阶段 | 内容 | 任务 | 写出的文件会不会变 |
|---|---|---|---|
| 0 | 准备 | T0.1、T0.2 | 不变 |
| 1 | 8.3 短名 | T1.1、T1.2 | 只在用短路径调用时变（改对） |
| 2 | 数据库线的降级看得见 | T2.1–T2.3 | 数据库线的 `_Coverage.json` 多一段 |
| 3 | MDF 读取 | T3.1–T3.3 | 不变 |
| 4 | 小修 | T4.1–T4.6 | 不变 |
| 5 | 重构 | T5.1、T5.2 | 不变 |
| 6 | 细节 | T6.1–T6.4 | 不变 |

每个检查点之后的状态都可以交付，后面的阶段可以按需砍掉。规模：XS = 1 个文件，S = 1–2 个，M = 3–5 个；没有比 M 大的任务。阶段 3、阶段 4 与阶段 2 互不依赖，可以并行；T2.3、T4.2–T4.4、T5.1、T5.2 都动 `backup_package.rs`，按编号顺序做。

### 阶段 0：准备（不改源码）

#### T0.1 对照矩阵与门禁基线

- **做什么：** 在起点提交上出发布构建（`rustup run stable cargo build --release --locked`），照 `publish-v6` 的做法拷成 `target/publish-audit/bin/pid_publish_xml-<提交>-release.exe`。跑一遍对照矩阵，每一项的输出文件、退出码、stdout、stderr 留在 `target/publish-audit/review/base/`（不入库）：
  1. 整包 `D:\work\cad\pid-test-data`：53 张 × 3 个文件加 `_Index.json`，共 160 个；
  2. 仓库内两个备份包整包：`test-file/backup-test/DWG-0202GP06-01_p`（Oracle）与 `test-file/backup-test/TEST02_p`（SQL Server）；
  3. `tests/publish_completeness.rs` 钉的 7 张同版参考图，`.pid` 单发与 `--oracle-exp --datamap` 各一次，`--out-dir`；
  4. 数据库线单图：A01 的 `Export.mdf --drawing … --out … --meta-out …`。
- **验收：** 同一矩阵连跑两次逐字节相同；全套门禁在起点提交上的结果（通过数、ignored 数、skip 行数）记进 §9。
- **依赖：** Q1。**文件：** 无，脚本与输出都在 `target/` 下。

#### T0.2 审核结论入库

- **做什么：** 把审核结论存成 `docs/analysis/2026-11-16-publish-export-review.md`，行号以 `a4b490e` 为准，带上 §1 的读数和核对后排除掉的那几条。
- **验收：** 本文头部的「依据」指向它；文中每条结论都有文件与行号。
- **依赖：** 无。**文件：** 1 个文档。**规模：** XS。

### 阶段 1：8.3 短名（审核第 1 条）

#### T1.1 两个入口合一

- **做什么：** 先加用例：把仓库内的 `test-file/DWG-0202GP06-01.pid` 拷进测试的临时目录，起一个超出 8.3 的长名，用 `cmd /c for %I in ("…") do @echo %~sI` 取它的短路径，把短路径交给 `pid_publish_xml --stdout`，断言 `<Container>` 的 `DocName` 和 `PIDDrawing` 的 `IObject.Name` 是长名。不是 Windows、或卷上没有短名时打印原因后跳过。在没改的代码上看它红。再改：`load_publish_from_pid` 变成 `read_publish_from_pid(path)?.drawing`，图名的推导（`canonicalize` 之后取文件名主干）只留在 `read_publish_from_pid` 一处。
- **验收：**
  - 新用例由红转绿；
  - §1 的第二条命令（I井场 的真实短路径）退出 0、`different 0`；
  - `the_ijingchang_alias_stem_reproduces_the_registered_item`（文件真名就叫 `IB031~1.pid` 的拷贝）不改，仍得到登记的那一处差异；对照矩阵逐字节不变。
- **验证：** `cargo test --locked -p pid-parse --test publish_xml_cli -- --nocapture`，`--test publish_diff_accepted_baseline -- --nocapture`。
- **依赖：** T0.1。**文件：** `src/publish/pid_load.rs`、`tests/publish_xml_cli.rs`。**规模：** S。

#### T1.2 说明与用例对齐

- **做什么：** `docs/baselines/README.md:123–129`、`docs/baselines/publish-diff-accepted.json:323` 的 `why`、`tests/publish_diff_accepted_baseline.rs:827–828` 的注释，三处改成修复后的事实：长名和它的短名都得到长名、退出 0；只有文件真名是 `IB031~1.pid` 的拷贝退出 1。Q2 选收紧时，`the_ijingchang_run_never_leaves_the_registered_item` 去掉退出 1 那一支，并用真实短路径再跑一遍，同样要求退出 0。
- **验收：** 三处说法一致；`publish_diff_accepted_baseline` 全过，`--nocapture` 的输出里 I井场 两种模式都真的跑了。
- **依赖：** T1.1、Q2。**文件：** 上述 3 个。**规模：** S。

#### 检查点 1

- [x] 全套门禁通过
- [x] 对照矩阵逐字节不变（用短路径调 I井场 的两项除外，那正是改对的地方；见 §9）
- [x] 红与绿的输出记进 §9，结果发给你

### 阶段 2：数据库线的降级看得见（审核第 2 条）

> 细化计划：`docs/plans/2026-11-16-publish-review-phase-2.md`（2026-11-16 批准），执行以它为准。下面的 T2.1–T2.3 是批准主计划时的写法，留作对照；不同的四处见该文 §2：多一个导出包入口（T2.4），关系合成一个数，整包拆出一段端到端验，检查点 2 的矩阵多变几项。

#### T2.1 写手侧记账

- **做什么：** 把两处判定各抽成一个函数，写手与记账共用：对象类型有没有专用写手（今天写在 `write_business_objects` 的 `.or_else` 里，`xml_writer.rs:164–173`）；一条关系是点名判出来的、落到 `_ => "Relationship"` 的，还是只凭类型对判成 `PipingEnd1Conn` 的（`xml_writer.rs:2591–2599`）。新增一个只读的汇总（暂名 `DatabaseShortfalls`）：占位对象按类型计数，兜底关系按「源类型、目标类型」计数，按类型对判端别的关系数。`write_data_xml` 的签名与输出不变。
- **验收：**
  - 合成一张含 `Exchanger`、`Mechanical`、`EquipmentOther`、`OPC` 和一个目录外类型的图：汇总的占位数等于 XML 里 `<PIDItem>` 的个数，兜底关系数等于 `DefUID="Relationship"` 的个数；
  - 目录里每一行没有 `db_writer` 的业务类型都被这条用例点到，以后加类型漏记会失败；
  - A01 三项都是 0，`_Data.xml` 逐字节不变。
- **验证：** `cargo test --locked -p pid-parse --lib publish::xml_writer`，`publish::catalog`。
- **依赖：** 检查点 1。**文件：** `src/publish/xml_writer.rs`、`src/publish/catalog.rs`、`src/publish/model.rs`。**规模：** M。

#### T2.2 单图命令行说出来

- **做什么：** 数据库线的 `run`（`pid_publish_xml.rs:475`）写完之后，汇总不为空就往 stderr 每类打一行，例如 `warning: 3 objects written as <PIDItem> placeholders (Exchanger 2, Mechanical 1)`。Q3 选了开关时，带 `--strict` 且汇总不为空：文件照写，退出 1。用法说明补一段，并删掉第 121 行那句走不到的话（`without it the input still exits 2`）。
- **验收：**
  - 用例里用 `rusqlite` 现造一份只有一台换热器的最小 `.sqlite` 镜像（`sqlite_load.rs` 的单测里有建表的写法）：不带开关退出 0、stderr 有那一行、stdout 的 XML 与不记账时相同；带 `--strict` 退出 1；
  - A01 的 stdout、stderr、退出码与基线相同。
- **依赖：** T2.1、Q3。**文件：** `src/bin/pid_publish_xml.rs`、`tests/publish_xml_cli.rs`。**规模：** S。

#### T2.3 整包记下来

- **做什么：** 数据库线整包（`backup_package.rs:1195` 起的逐图循环）每张图把汇总写到三处：`_Coverage.json` 多一段（恒有，没有降级时各项为 0）；`_Index.json` 里这张图的 `problems` 与顶层 `problems` 各一句；`BackupPublishReport` 多一个字段，命令行照 `chain_faults` 的先例（`pid_publish_xml.rs:573`）往 stderr 说。图仍算写出，`failed` 不变；带 `--strict` 时整包退出 1。**输出会变**：数据库线的 `_Coverage.json` 多一段。
- **验收：**
  - 「汇总 → 覆盖报告那一段、`problems` 的句子」做成纯函数，单测点到有降级的情形。带换热器的 SQL Server 包仓库里没有，整包这一支只能这样验；
  - `TEST02_p` 整包真跑：除 `_Coverage.json` 多出的那一段，其余逐字节不变；
  - Oracle 的两个整包（160 个文件与 DWG 包）逐字节不变。
- **依赖：** T2.1。**文件：** `src/publish/backup_package.rs`、`src/publish/pid_writer.rs`、`src/bin/pid_publish_xml.rs`、一个测试文件。**规模：** M。

#### 检查点 2

- [x] 全套门禁通过
- [x] 对照矩阵：只有数据库线的 `_Coverage.json` 变了，变的只是多出的那一段（打印用法说明的两项也跟着变了，细化计划 §2 第 4 处写过；见 §9）
- [x] 结果发给你

### 阶段 3：MDF 读取（审核第 3 条，依 Q4）

#### T3.1 先量

- **做什么：** 加探针 `examples/probe_what_staging_an_mdf_costs.rs`：对给定的 `.mdf`，按 `PUBLISH_TABLES` 逐表把文件包进一个计字节的 `Read` 交给 `MdfDatabase::from_read`，打印每张表读入的字节、合计是文件大小的几倍、各表用时，以及 `open_mdf_as_sqlite` 的总用时。峰值内存从外面取（PowerShell 的 `PeakWorkingSet64`）。
- **验收：** A01 的 `Export.mdf`（19.9 MB）与 `TEST02_p` 整包两组读数（发布构建）记进 §9：读入字节比文件大小、峰值内存比文件大小、总用时。
- **依赖：** 无。**文件：** 一个 example、`Cargo.toml` 的 `[[example]]`。**规模：** S。

#### T3.2 读取器按页寻址

- **做什么：** `PageReader`（`vendor/oxidized-mdf/src/lib.rs:459–522`）从只进不退的 `Read` 改成 `Read + Seek`，要哪一页就 `seek` 到 `page_id × 8192` 读哪一页；页缓存设上限；同一页不再解析两遍。`MdfDatabase::open(path)` 的签名不变，`from_read` 改为要 `Read + Seek`（仓库里它只被 `open` 调用）。文件头的本地改动清单补一行。
- **验收：**
  - 新单测：先读靠后的页再读靠前的页读得到（今天报 `forward-only reader cannot re-read an earlier page`）；缓存的页数不超过上限；
  - `cd vendor/oxidized-mdf && cargo test --lib` 与改前同样通过；
  - `tests/publish_mdf_load.rs`、`tests/publish_a01_raw_residual.rs` 不改断言全过，A01 与 `TEST02_p` 的输出逐字节不变。
- **依赖：** T3.1 的读数过线。**文件：** `vendor/oxidized-mdf/src/lib.rs`。**规模：** S。

#### T3.3 暂存只开一次

- **做什么：** `open_mdf_as_sqlite`（`mdf_load.rs:56`）24 张表共用一次 `MdfDatabase::open`；每张表的插入包在一个事务里；MTF 那条路把 MDF 从 dump 的切片直接写进临时文件，去掉 `mdf_load.rs:112` 的整份拷贝。
- **验收：**
  - T3.1 的探针重跑，读入字节比文件大小降到约 1 倍，峰值内存不再跟着文件大小走，前后的数字记进 §9；
  - 暂存出来的表逐行与改前相同；A01 与 `TEST02_p` 的输出逐字节不变。
- **依赖：** T3.2。**文件：** `src/publish/mdf_load.rs`、T3.1 的探针。**规模：** S。

#### 检查点 3

- [ ] 全套门禁与 vendored 件自己的测试通过
- [ ] 对照矩阵逐字节不变
- [ ] 改前改后的读数发给你

### 阶段 4：小修

#### T4.1 同一对象只发一次（审核第 5 条）

- **做什么：** 把分支 `publish-stash-20261108`（`9b4d60b`）里 `attach_dump_only_components` 的那一块（`known.insert(item.uid.clone())`）和它的用例 `a_component_placed_twice_is_published_once` 落到 `main` 现在的代码上（`oracle_exp.rs:1386–1410`）。
- **验收：** 用例改前红、改后绿；对照矩阵逐字节不变（`publish-v6` 的 53 张里今天没有重复的 UID）。
- **依赖：** 无。**文件：** `src/publish/oracle_exp.rs`。**规模：** XS。

#### T4.2 整包没有参考时也查重复 UID

- **做什么：** `diff.rs` 解析写出的文档时已经记了重复的 UID，但 `pid_publish_counts`（`diff.rs:1541`）不带出来。带出来；整包里有重复的图照「写出但点名」处理：`_Index.json` 的 `problems` 一句，stderr 一行。
- **验收：** 单测里人为重复一个对象块，被点名；对照矩阵逐字节不变。
- **依赖：** T2.3。**文件：** `src/publish/diff.rs`、`src/publish/backup_package.rs`。**规模：** S。

#### T4.3 原子写（审核第 6 条）

- **做什么：** 加 `write_atomic`：写到同目录的临时名再 `rename` 到位。整包的三件套（`backup_package.rs:914`）、两处 `_Index.json`、命令行的 `write_file`（`pid_publish_xml.rs:1160`）都用它；三件套里有一个没写成，就把这张图已经到位的删掉。用法里写明 `--out-dir` 不清理旧文件，以 `_Index.json` 为准。
- **验收：**
  - 用例：让某张图的第三个文件写不成（目标名预先建成目录），这张图一个文件都不留，`_Index.json` 点名，其它图照写；
  - 临时名不以 `_Data.xml`、`_Meta.xml`、`_Coverage.json` 结尾，中断的运行不会留下看起来完整的文件；
  - 对照矩阵逐字节不变。
- **依赖：** T4.2。**文件：** `src/publish/backup_package.rs`、`src/bin/pid_publish_xml.rs`、一个测试文件。**规模：** M。

#### T4.4 zip 条目设上限（审核第 7 条）

- **做什么：** `publish_zip`（`backup_package.rs:474`）读 `.pid` 条目之前看解压后的大小，读的时候再用 `take` 兜住（条目头里的大小可以不实）。上限暂定 512 MiB，语料里的 `.pid` 是几 MB。超限的那张图失败并点名，其余照发。
- **验收：** 单测造一个超过上限的条目（测试里把上限调小）：这张图进 `problems`，别的图照写；对照矩阵逐字节不变。
- **依赖：** T4.3。**文件：** `src/publish/backup_package.rs`。**规模：** XS。

#### T4.5 dump 里的表只找一遍（审核第 7 条，按读数）

- **做什么：** 先量 `ExpTables::read` 在 74 MB 的 `Export.dmp` 上的用时占整包那 19 秒的多少。不到一成就只把读数记下，不改。过了一成：对 dump 扫一遍建「表名 → 位置」的索引，19 张表（6 张可选）各自直接跳过去，替掉每张表从头的 `find_slice`（`oracle_exp.rs:1866`）和可选表的第二遍（`:2435`）。
- **验收：** 读数记进 §9；改了的话对照矩阵逐字节不变，`tests/parser_panic_safety.rs` 全过。
- **依赖：** T4.1。**文件：** `src/publish/oracle_exp.rs`。**规模：** S。

#### T4.6 表清单的一致性（审核第 9 条）

- **做什么：** 加一条用例：目录表各行 `db_subtables` 点到的表都在 `PUBLISH_TABLES`（`mdf_load.rs:20`）里，`PUBLISH_TABLES` 的每张表也都有读它的地方。`prepare_optional`（`sqlite_load.rs:328`）跳过缺表时，在 `-v` 的诊断里说一声。
- **验收：** 从 `PUBLISH_TABLES` 里删掉一张表，或给目录表加一张没暂存的子表，用例失败；不带 `-v` 时 stderr 不变。
- **依赖：** T3.3（同一个文件）。**文件：** `src/publish/mdf_load.rs`、`src/publish/sqlite_load.rs`。**规模：** S。

#### 检查点 4

- [ ] 全套门禁通过
- [ ] 对照矩阵逐字节不变
- [ ] 结果发给你

### 阶段 5：重构（依 Q5）

#### T5.1 exp 增补只编排一处（审核第 4 条）

- **做什么：** 命令行的 `run_pid`（`pid_publish_xml.rs:658–790`）和整包的 `attach_exp`（`backup_package.rs:872–900`）各自把 dump 的内容一步步挂到图上，两条 `retain` 规则（`:711`、`:888`）各写一遍。抽成库里的一个函数，两边都调它。
- **验收：** 对照矩阵逐字节不变，单图和整包两种入口都在矩阵里；`tests/publish_completeness.rs` 的钉一个不动。
- **依赖：** T4.1、T4.4；另一条线对 `run_pid` 的改动（见 §8 第一行）先进 `main`。**文件：** `src/publish/oracle_exp.rs`、`src/publish/backup_package.rs`、`src/bin/pid_publish_xml.rs`。**规模：** M。

#### T5.2 `publish_one` 收拾（审核第 8 条）

- **做什么：** `uid_from` 从字符串换成枚举，写进 `_Index.json` 的文字原样保留（包括 `:1460` 那种拼法）；重复约 8 处的失败记账收成一个函数；`PublishTally` 的 24 个 `&mut` 字段收成方法。
- **验收：** 对照矩阵逐字节不变；不改任何断言。
- **依赖：** T5.1。**文件：** `src/publish/backup_package.rs`。**规模：** XS。

#### 检查点 5

- [ ] 全套门禁通过
- [ ] 对照矩阵逐字节不变
- [ ] 结果发给你

### 阶段 6：细节

#### T6.1 序列化失败不再吞掉

- **做什么：** `_Index.json` 序列化失败时现在写出一个 `{}`（`backup_package.rs:347`、`:1346`），覆盖报告失败时写出空串（`pid_writer.rs:764`、`:807`）。改成把错误报出去。
- **验收：** 对照矩阵逐字节不变。
- **依赖：** T5.2。**文件：** `src/publish/backup_package.rs`、`src/publish/pid_writer.rs`。**规模：** S。

#### T6.2 命令行小项

- **做什么：** `_Meta.xml` 渲染一次，不是最多三次（`pid_publish_xml.rs:816`、`:824`、`:844`）；参考的 `_Data.xml` 只读一次；`print!` 换成会把写入失败报出来的写法（`:866`、`:941`、`:1061`）；输入必须是第一个参数这件事写进用法（`:196`）。
- **验收：** 矩阵里每一项的 stdout、stderr、退出码不变。
- **依赖：** T5.1。**文件：** `src/bin/pid_publish_xml.rs`。**规模：** XS。

#### T6.3 库内小项

- **做什么：** `pipeline_uid` 的命名空间解析失败时静默用全零 UUID（`pipeline.rs:203`），改成编译期常量；`list[0] = json!(…)` 的事后修补（`pid_writer.rs:660`）改成直接构造；`fmt_err` 把格式化错误装进 `PublishError::Sqlite`（`xml_writer.rs:2020`），而 `PublishError` 不是 `non_exhaustive`，加变体对外是破坏性改动，这一轮只把注释写准；`failed` 把打不开的 zip 和条目也算成图，在 `_Index.json` 的说明里写清。
- **验收：** 对照矩阵逐字节不变。
- **依赖：** T6.1。**文件：** `src/publish/pipeline.rs`、`src/publish/pid_writer.rs`、`src/publish/xml_writer.rs`、`src/publish/backup_package.rs`。**规模：** M。

#### T6.4 文档

- **做什么：** `src/publish/mod.rs` 模块文档的 Outstanding 一节还是 `048a842` 时的样子，照现状改；`README.md`、`AGENTS.md`、`CHANGELOG.md` 补上 `--strict`、覆盖报告的新段、原子写。
- **验收：** `bash .github/scripts/check-missing-docs.sh` 通过；文档里的命令照着跑得出文档里的结果。
- **依赖：** 前面各阶段。**文件：** 上述 4 个。**规模：** M。

#### 检查点 6（收尾）

- [ ] 全套门禁通过
- [ ] 对照矩阵与基线逐项比过，差异只有 §6 表里写明的
- [ ] 每个任务的提交号和读数填进 §9

## 7. 不在本计划内

- 给 `Exchanger`、`Mechanical`、`EquipmentOther`、`OPC` 写数据库线的专用写手：要原厂参考。
- 2026-11-10 计划里的 S10、S12、S15、S16、S17。
- Oracle 的 `Export.dmp` 整个读进内存（`pid_publish_xml.rs:660`、`backup_package.rs:374`）：改成流式要重写读取器；现在最大的样本 74 MB，先记着。
- `_Index.json`、`_Coverage.json` 里的本机绝对路径：一改，交付文件的字节就变，留到下一次重发交付时一起定。
- 拆大文件（`xml_writer.rs` 6 143 行、`oracle_exp.rs` 5 492 行、`diff.rs` 4 015 行）和输出里的中英混排（`diff.rs:1424`）。
- 重发交付：本计划对 Oracle 整包的 160 个文件没有改动。

## 8. 风险

| 风险 | 影响 | 应对 |
|---|---|---|
| 当前检出里另有一条线正在改 `stored_values.rs` 和 `run_pid` 的 stderr 输出（未提交） | 互相踩工作区；本文的行号以 `a4b490e` 为准，那条线并入后会挪动；T5.1 要改的正是同一段 `run_pid` | Q1 推荐另开工作区；每个阶段开工前先把 `main` 并进来、重建对照矩阵；T5.1 等那条线进了 `main` 再做 |
| 数据库线只有 A01 一个夹具，12 个对象，没有一处降级 | 记账的路只能用合成数据验 | T2.1 用合成图去对 XML 里的实际个数；整包那一支拆出纯函数单测 |
| 8.3 短名只在 Windows 上有，CI 在 Linux 上跑 | 新用例在 CI 上是跳过的 | 本机带 `--nocapture` 跑，输出记进 §9；跳过时打印原因 |
| `canonicalize` 会跟随符号链接 | 用链接名调用时，图名取的是目标文件名 | 写进 `read_publish_from_pid` 的文档注释；Windows 上建符号链接要权限，不加用例 |
| 改的是 GPL-3.0 的 vendored 件 | 许可要求标明修改 | 文件头的改动清单补一行；对外签名尽量不动 |
| 未并入的 `backup-store-v2` 把 `mdf_load.rs` 换成了 `store_load.rs` | 那条分支以后要并的话，T3.3 会冲突 | T3.2（读取器）两条路都受益；T3.3 只动打开次数与事务，改动面小 |
| 数据库线的 `_Coverage.json` 多一段 | 钉了它字节的用例要改钉值 | 只增不改；T2.3 的提交说明里点名改了哪些钉 |
| 本机默认工具链是 nightly，`PATH` 上的 `cargo` 也不是 stable 的；CI 是 stable | 本机绿不等于 CI 绿 | 门禁统一用 stable 工具链，跑法见 §9 的 T0.1 |

## 9. 执行记录

工作区 `D:\work\plant-code\cad\pid-parse-wt\pub-r`，分支 `publish-review-followup-20261116`，起点 `a4b490e`。脚本与输出都在 `target/publish-audit/review/` 下，不入库。

### T0.1 对照矩阵与门禁基线（2026-11-16）

**工具链。** 本机 `PATH` 上排第一的 `cargo`（`D:\Rust\.cargo\bin\cargo.exe`）不是 rustup 的代理，是单独装的一个 cargo 1.101.0。`RUSTUP_TOOLCHAIN=stable` 和 `cargo +stable` 都换不掉它，只把它调的 `rustc` 换成了 stable；`rustup run stable cargo …` 换得掉直接敲的这一条，换不掉脚本里再调的 `cargo`（`check-missing-docs.sh`）。所以门禁的跑法是把 stable 工具链自己的 `bin` 目录（`rustup which cargo --toolchain stable` 所在的目录）放到 `PATH` 最前面，再照 §5 的原样命令跑；每次运行的记录头写着实际用到的两个程序：`cargo 1.99.1 (5f94df478 2026-08-27)`、`rustc 1.99.1 (58cf8f9c3 2026-10-13)`。`bash` 用 Git 自带的那个，`PATH` 上的 `bash` 是 WSL 的启动器。

**发布构建。** `rustup run stable cargo build --release --locked`，拷成 `target/publish-audit/bin/pid_publish_xml-a4b490e-release.exe`（4 598 272 字节，SHA-256 `2b36c7a2…bc2bb6`）。

**矩阵。** `matrix.ps1`，59 项：计划里列的四组，加上用法、`.pid` 的各种输出方式、I井场 的长短路径、数据库线的其它开关。每一项留下写出的文件、stdout、stderr、退出码和命令行，共 703 个文件（134.1 MB），基线在 `target/publish-audit/review/base/`。比对用 `compare.ps1`，每个文件算 SHA-256。

| 组 | 项 | 退出码 |
|---|---|---|
| 用法 | `--help`；不带参数 | 0；2 |
| 整包 `D:\work\cad\pid-test-data` | 发布（160 个文件）；再加 `--diff-against` | 0；1（门禁有发现，是现状） |
| 仓库内备份包 | `DWG-0202GP06-01_p`（22 个文件）与 `TEST02_p`（4 个文件）：发布；再加 `--diff-against` | 0；1（同上） |
| 验收数据的 11 张图 | `.pid` 单发；加 `--oracle-exp --datamap`；再加 `--diff-against` | 单发 11 张都是 0；带 dump 的 7 张同版图（01 02 03 06 08 09 11）两项都是 0，4 张异版图（04 05 07 10）两项都是 2：dump 里没有这张图的 `T_DRAWING` 行，按设计拒绝 |
| DWG 夹具 | 单发、带 dump、`--stdout`（夹具目录里的和 `test-file/` 顶上的两份 `.pid` 各一次）、`--out` 加 `--meta-out`；两项带 `--diff-against` | 0；1（与参考有差异，是现状） |
| I井场 | 长路径 `--diff-against`；短路径 `--diff-against`；短路径 `--out-dir` | 0；**1**（`different 1`，图名 `IB031~1`）；0，但写出的三个文件叫 `IB031~1_*` |
| A01 数据库线 | `Export.mdf` 与 `.sqlite` 镜像的发布、`--list-drawings`、`--style dwg`；`--diff-against`；不存在的图 | 0；1（与参考有差异，是现状）；1 |

图的编号与文件名的对应在 `base/names.txt`，59 项的退出码、文件数与输出大小在 `base/summary.txt`。I井场 那一行就是审核第 1 条的现状；`--out-dir` 写出的文件名也跟着短名走，这一点审核时没有写到。

**连跑的结果。** 同一个程序连跑两遍，703 个文件逐个相同；补了符号库联接（见下）之后的第三遍也相同。工作区改名（见下）之后重跑的第四遍是现在的基线：与前三遍比，写出的文件和 stdout 一个字节不差，不同的 85 个全是带着输出目录路径的记录（46 个 `command.txt`、38 个 `stderr.bin`、1 个 `summary.txt`）。

**新工作区的两处环境问题。** 都不是代码的问题，记在这里是因为第一轮门禁正是停在这两处：

1. 新检出里没有 `test-file/symbols-full`（gitignore 的本机符号库）。没有它，`pid-plot/tests/export_plant_backup.rs` 的 `every_backup_drawing_exports_a_readable_page` 失败，`docs/baselines/README.md` 写着这种检出要补上该目录。照别的工作区的做法补了一个联接，指向主检出的 `test-file\symbols-full`。
2. `cargo fmt --all -- --check` 把各包的目标文件按 edition 分组，每组用完整路径一次交给 `rustfmt`，而 Windows 的命令行不能超过 32 767 个字符。`AGENTS.md` 的「Pre-commit gates」一节写着这个上限，读数是 2026-11-13 取的：edition 2021 这一组 367 个文件，检出路径不超过 47 个字符就能跑。现在这一组是 373 个文件（edition 2024 那一组从 98 个到了 104 个），那个 47 已经过时：工作区起初叫 `pub-review`，路径 46 个字符，这条命令报 `os error 206`（文件名或扩展名太长）。不是格式有差异：逐包跑 `cargo fmt -p pid-parse -- --check`、`-p pid-plot` 都是退出 0。工作区改名为 `pub-r`（41 个字符）之后照原样的命令通过，`cargo fmt --all -v -- --check` 打印的 edition 2021 那条命令行是 30 912 个字符（edition 2024 那条 9 949 个）。路径每长一个字符这条命令行长 373 个，按这个算 45 个字符时是 32 404、46 个字符时是 32 777（超出 10 个，与实测的失败对得上），所以现在的上限是 45；主检出（32 个字符）是 27 555，离上限 5 212 个字符，相当于再加五六十个目标文件（45 与 32 这两个数是算的，没有跑）。Linux 的 CI 没有这个上限。本计划的新用例都加在已有的测试文件里，新增的目标文件只有 T3.1 的一个探针（这条命令行因此长 90 个字符上下）；`AGENTS.md` 那段读数的更新放在阶段 6 的文档收口里。

**门禁。** `gates.ps1 -Label base`，在 `a4b490e` 上跑（工作区里只多本计划与审核文档两个未入库的文件），日志在 `target/publish-audit/review/gates/base/`：

| 门禁 | 退出码 | 用时 | 读数 |
|---|---|---|---|
| `cargo build --locked --workspace --all-targets` | 0 | 139 s | |
| `cargo test --locked --workspace --all-targets` | 0 | 1 682 s | 474 个测试程序；2 519 通过，0 失败，3 忽略；`skipping: ` 提示 21 行 |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | 0 | 40 s | 0 条警告 |
| `cargo fmt --all -- --check` | 0 | 4 s | |
| `bash .github/scripts/check-missing-docs.sh` | 0 | 10 s | `current=0, baseline=0` |
| `cargo test --locked -p pid-parse --no-default-features --lib` | 0 | 27 s | 985 通过 |
| `cargo check --lib --locked -p pid-parse --no-default-features --target wasm32-unknown-unknown` | 0 | 25 s | |
| `bash .github/scripts/check-byte-audit-baselines.sh` | 0 | 23 s | 3 份基线都比了，跳过 0 |

最后一行不在 §5 的清单里：它是 CI（`.github/workflows/ci.yml`）在那五条之后的一步，`AGENTS.md` 的清单没有列，上一次并入 `main`（`2681ccd`）的门禁也跑了它。它比的是解析一侧的字节审计基线，与导出无关；为了和 CI 一致，以后每个检查点都带上它（`gates.ps1` 的第八道；这一次是单独补跑的，日志在 `gates/base-08/`）。

第一轮是在工作区还叫 `pub-review`、还没有符号库联接时跑的（日志在 `gates/base-first-round/`），只在上面两处失败：测试退出 101，失败的只有 `every_backup_drawing_exports_a_readable_page` 一条（2 495 通过、1 失败；`cargo test` 停在第一个失败的测试程序上，后面的没有跑，所以只用了 808 s）；`cargo fmt` 退出 1，`os error 206`。其余五道当时也是 0。

### T0.2 审核结论入库（2026-11-16，`192f7fe`）

审核结论存成 `docs/analysis/2026-11-16-publish-export-review.md`，行号以 `a4b490e` 为准；本文头部的「依据」指向它。本文自己是 `493b3b7`。

### T1.1 两个入口合一（2026-11-16，`d65dc74`）

**红。** 先加两条用例（`tests/publish_xml_cli.rs`），在没改的代码上跑（`runs/t11-red-both.log`）：

```text
test cli_pid_given_by_its_8_3_alias_is_published_under_its_long_name ... FAILED
  DocName should be the file's stored name, not the alias; got: … DocName="ADRAWI~1"
test cli_pid_given_in_another_letter_case_is_published_under_its_stored_name ... FAILED
  DocName should be spelled as the file is stored; got: … DocName="A DRAWING STORED IN MIXED CASE"
test result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 25 filtered out
```

两条都把 `test-file/DWG-0202GP06-01.pid` 拷进临时目录，起一个超出 8.3 的长名。第一条用 `cmd` 的 `%~sI` 取它的短路径交给命令行（辅助函数 `short_path_of` 在 `tests/common/mod.rs`；不是 Windows、或卷上没有短名时打印 `skipping:` 后跳过）。第二条把同一个路径换一种大小写交给命令行。

**改。** `read_publish_from_pid` 先 `std::fs::canonicalize`，图名取它给出的文件名主干；`load_publish_from_pid` 变成 `read_publish_from_pid(path)?.drawing`。图名的推导只剩这一处。`canonicalize` 也跟随符号链接，所以经符号链接给的路径，图名取的是链接指向的那个文件的名字；这是标准库写明的行为，没有单独的用例，`CHANGELOG.md` 里写了。解析不了的路径照原来的取法，取路径上写的文件名主干。

**绿。** 同两条用例（`runs/t11-clippy-2.log`）：`2 passed; 0 failed; 0 ignored; 0 measured; 25 filtered out`。`publish_xml_cli` 整个文件 27 条、`publish_diff_accepted_baseline` 19 条都过。

**超出计划原文的一处。** 计划只说了 8.3 短名。`canonicalize` 给的是文件存的名字，大小写也照存的来，所以路径换一种大小写写时，图名也归到存的那个写法（第二条用例）。改之前图名跟着命令行的写法走，同一个文件能发布出两个名字。这一点 2026-11-16 问过，按推荐做。

**对照矩阵。** T1.1 改完之后的发布构建（`target/publish-audit/bin/pid_publish_xml-t11-release.exe`，4 598 784 字节，SHA-256 `6912dc7b…fabf752e`）跑了一遍矩阵（`target/publish-audit/review/t11/`）。`compare.ps1 -A base -B t11`：703 个文件，695 个逐字节相同；内容不同的 5 个，只在一边有的各 3 个，全出在用短路径调 I井场 的两项上（第 5 个是汇总各项退出码和大小的 `summary.txt`，跟着这两项变）：

| 项 | 基线 | T1.1 之后 |
|---|---|---|
| `ij-short-diff`（短路径加 `--diff-against`） | 退出 1；stdout 17 812 字节，摘要里 `different 1`，后面是整份差异报告；stderr 报的图名是 `IB031~1` | 退出 0；stdout 134 字节，只有摘要一行，`different 0`；stderr 报的图名是长名，结论是 `is clean in .pid mode (1735 objects, 1530 values, 1971 rels)` |
| `ij-short-outdir`（短路径加 `--out-dir`） | 写出 `IB031~1_Data.xml`、`IB031~1_Meta.xml`、`IB031~1_Coverage.json` | 三个文件改叫 `I井场 注采阀组工艺及自控流程图_*` |

`--out-dir` 写出的三个文件除了图名没有别的变化：长名在 `_Data.xml` 里出现 2 次、`_Meta.xml` 里 4 次、`_Coverage.json` 里 3 次，把这 9 处换回 `IB031~1` 之后，三个文件与基线逐字节相同。其余 57 项的写出文件、stdout、stderr、退出码一个字节不差。

### T1.2 说明与用例对齐（2026-11-16，`acc3f09`）

**用例（Q2 收紧）。** `the_ijingchang_run_never_leaves_the_registered_item` 原来接受退出 0 或退出 1 两种结果。现在用长名和它的 8.3 短路径各跑两种模式，四次都必须退出 0，摘要等于基线里记的 `clean_summary`，没有登记项也没有未登记项。

**超出计划原文的一处。** 去掉退出 1 那一支之后，基线里 `modes.oracle-exp.alias_summary` 这个字段就没有用例读了。实测文件真名叫 `IB031~1.pid` 的拷贝在 `--oracle-exp` 模式下退出 1，摘要正是这个字段记的值，于是把 `the_ijingchang_alias_stem_reproduces_the_registered_item` 从只跑 `.pid` 单发扩成两种模式都跑，两种模式都要求恰好那两行登记的差异。

**输出**（`runs/t12-baseline-2.log`：`cargo test --locked -p pid-parse --test publish_diff_accepted_baseline -- --nocapture`，19 过）：

```text
I井场 pid-alone by its long name (…\I井场 注采阀组工艺及自控流程图.pid): exit 0, pid export diff: objects 1735 (extra 0, wrong class 0), values 1530 (different 0, not in reference 0), rels 1971 (not in reference 0)
I井场 oracle-exp by its long name (…\I井场 注采阀组工艺及自控流程图.pid): exit 0, pid export diff: objects 3093 (extra 0, wrong class 0), values 6506 (different 0, not in reference 0), rels 4107 (not in reference 0)
I井场 pid-alone by its 8.3 alias (D:\work\cad\PID~1\IB031~1.PID): exit 0, pid export diff: objects 1735 (extra 0, wrong class 0), values 1530 (different 0, not in reference 0), rels 1971 (not in reference 0)
I井场 oracle-exp by its 8.3 alias (D:\work\cad\PID~1\IB031~1.PID): exit 0, pid export diff: objects 3093 (extra 0, wrong class 0), values 6506 (different 0, not in reference 0), rels 4107 (not in reference 0)
I井场 pid-alone by a copy stored as IB031~1.pid: exit 1, pid export diff: objects 1735 (extra 0, wrong class 0), values 1530 (different 1, not in reference 0), rels 1971 (not in reference 0)
I井场 oracle-exp by a copy stored as IB031~1.pid: exit 1, pid export diff: objects 3093 (extra 0, wrong class 0), values 6506 (different 1, not in reference 0), rels 4107 (not in reference 0)
```

第三行就是 §1 的第二条命令：审核时它退出 1、`different 1`，现在退出 0、`different 0`。

**说明。** `docs/baselines/README.md`、`docs/baselines/publish-diff-accepted.json`（`what`、`why`、`path_sensitive: false`、`allowed_outcomes`、`forbidden_outcomes`）、`AGENTS.md` 的用例表、`CHANGELOG.md` 改成同一个说法：图名是文件存的名字；长名或它的短路径退出 0；只有文件真名就是短名的拷贝退出 1，而且恰好两行。README 原来写的「恰好一处差异」不准，实际是两行（`IObject.Name` 和容器的 `DocName`），一并改了。登记的差异项本身没有动。

### 检查点 1（2026-11-16）

**门禁。** `gates.ps1 -Label cp1`，在 `acc3f09` 上跑，工作区没有未提交的改动；日志在 `target/publish-audit/review/gates/cp1/`：

| 门禁 | 退出码 | 用时 | 读数 |
|---|---|---|---|
| `cargo build --locked --workspace --all-targets` | 0 | 73 s | |
| `cargo test --locked --workspace --all-targets` | 0 | 1 703 s | 474 个测试程序；2 521 通过，0 失败，3 忽略；`skipping: ` 提示 21 行 |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | 0 | 1 s | 0 条警告 |
| `cargo fmt --all -- --check` | 0 | 4 s | |
| `bash .github/scripts/check-missing-docs.sh` | 0 | 7 s | `current=0, baseline=0` |
| `cargo test --locked -p pid-parse --no-default-features --lib` | 0 | 2 s | 985 通过 |
| `cargo check --lib --locked -p pid-parse --no-default-features --target wasm32-unknown-unknown` | 0 | 1 s | |
| `bash .github/scripts/check-byte-audit-baselines.sh` | 0 | 11 s | 3 份基线都比了，跳过 0 |

与基线比，通过的测试多 2 条（2 519 → 2 521），是 T1.1 加的那两条；忽略数、`skipping: ` 行数、缺文档数、不带默认特性的 985 条都没变。第 3、6、7 道只用了一两秒：日志里没有 `Compiling` 或 `Checking` 行，这份源码的编译产物是现成的，cargo 没有重编；第 6 道的 985 条测试是真跑了的。

**对照矩阵。** `acc3f09` 的发布构建（`target/publish-audit/bin/pid_publish_xml-acc3f09-release.exe`，4 598 784 字节，SHA-256 `fba71594…2d9f3dd8`）跑了一遍矩阵（`target/publish-audit/review/cp1/`，59 项，157 s）。它和 T1.1 那个可执行文件大小相同，只差 5 个字节（3 处，其中一处是 PE 头里的链接时间戳）：两次构建之间只有用例和说明的改动。`compare.ps1 -A t11 -B cp1`：703 个文件逐字节相同。`compare.ps1 -A base -B cp1`：703 个文件，695 个相同，内容不同的 5 个、只在一边有的各 3 个，就是 T1.1 一节列的那一批。

计划里 T1.1 的验收写的是「对照矩阵逐字节不变」。实际是：除了用短路径调 I井场 的两项，其余逐字节不变；那两项正是要改对的地方。

**顺带记下的。** `AGENTS.md` 里 `cargo fmt` 命令行长度的那组读数已经过时（见 T0.1），放在阶段 6 的文档收口里一起改。阶段 2 的细化计划另成一份，等评审。

### 阶段 2 的读数（2026-11-16）

阶段 2 照细化计划 `docs/plans/2026-11-16-publish-review-phase-2.md`（`6e93415`）做。它 §1.3 的读数抄在这里：

| 看的是什么 | 结果 |
|---|---|
| 仓库里唯一的 SQL Server 库（`TEST02_p` 的 `Export_v2.sqlite`） | 1 张图，4 个条目（`Vessel`、`Nozzle`、`PipeRun`、`PipingPoint` 各 1），6 个图形表示，3 行关系：2 行两端都是本图的图形表示，1 行一端为空 |
| 真实工厂的库（Oracle 包 `Export.dmp`，53 张图）里的条目，照数据库线的载入规则算 | 15 439 个对象，没有专用写手的 1 283 个：`OPC` 952、`EquipmentOther` 225、`Mechanical` 94、`Exchanger` 12。53 张图里 49 张有份，最多的一张 476 个 |
| 同一个库的关系表（`T_RELATIONSHIP`） | 24 664 行：22 353 行两端都是图形表示，2 311 行有一端为空。以条目作端点的行是 0 |
| 检查点 1 的对照矩阵的全部输出 | 323 份 XML（308 个文件，15 份写在 stdout 上）里 `<PIDItem>` 0 个，`DefUID="Relationship"` 0 条 |
| SmartPlant 自己发布的参考 XML | 14 份、28 384 条关系：`<PIDItem>` 0 个，`DefUID="Relationship"` 0 条 |

第二、三行是只读脚本照载入规则从 Oracle 包的表里数的，不是数据库线跑出来的（Oracle 包喂不进数据库线）。输出留在 `target/publish-audit/review/runs/rel-ends-export-dmp.txt`，检查点 2 时重看过，数字对得上。第四行在检查点 2 的矩阵上重数了一遍（见「检查点 2」）。第一、五行没有重数。

这些数定下了阶段 2 的验法。能喂给数据库线的真实数据只有 `TEST02_p` 那一张图，它不触发任何降级。所以降级时的行为只能用现造的小库验，真实数据上验的是「没有降级时一个字节都不变」。

**矩阵的跑法。** 阶段 2 每个任务的矩阵都是改完、提交之前跑的（`cp-matrix.ps1`：发布构建，跑 59 项，与上一份比）。脚本用当时 `HEAD` 的提交号给可执行文件起名，所以名字里的提交号比任务自己的早一个：T2.1 的叫 `pid_publish_xml-6e93415-release.exe`，T2.2 的叫 `pid_publish_xml-a3908ac-release.exe`，依此类推。每次的记录 `runs/t2N-matrix.log` 头一行写着当时的 `commit` 和未提交的文件数。

### T2.1 写手侧记账（2026-11-16，`a3908ac`）

**红。** `database_shortfalls` 先留一个返回空汇总的桩，加上用例，跑 `cargo test --locked -p pid-parse --lib publish::`（`runs/t21-red.log`）：

```text
test publish::xml_writer::tests::shortfalls_count_the_placeholders_the_writer_emits ... FAILED
   left: {}
  right: {"EquipmentOther": 1, "Exchanger": 2, "Mechanical": 1, "OPC": 1, "SomethingNew": 1}
test publish::xml_writer::tests::shortfalls_count_the_relationship_rows_defined_by_item_type ... FAILED
   left: {}
  right: {("Nozzle", "(not in the drawing)", "Relationship"): 1, ("Nozzle", "Exchanger", "Relationship"): 1, ("Nozzle", "Vessel", "EquipmentComponentComposition"): 1, ("PipeRun", "Nozzle", "PipingEnd1Conn"): 1}
test publish::xml_writer::tests::shortfalls_name_an_item_without_a_type_and_a_representation_end ... FAILED
   left: None
  right: Some(1)
test publish::xml_writer::tests::shortfalls_report_every_catalog_type_without_a_writer_of_its_own ... FAILED
  the account must name exactly the catalog types that have no writer of their own
   left: []
  right: ["EquipmentOther", "Exchanger", "Mechanical", "OPC"]
test result: FAILED. 424 passed; 4 failed; 0 ignored; 0 measured; 1126 filtered out
```

四条都是断言失败：左边是桩给的空汇总，右边是该有的数。

**改。** 汇总是 `DatabaseShortfalls`（`src/publish/model.rs`，`#[non_exhaustive]`）：占位对象按条目类型计数；关系表里只凭两端的条目类型定出定义的行，按（1 端的类型，2 端的类型，写成的定义）计数。它有 `is_empty()`、两个合计和 `describe()`；`describe()` 每类给一句英文，后面三个入口用的是同一份。`database_shortfalls(&PublishDrawing)`（`src/publish/xml_writer.rs`）取这份汇总。

判定只写一处，写手和记账共用，所以汇总里的数就是文档里标签的数：

- 对象：`catalog::dedicated_db_writer` 回答一个类型有没有自己的写手。`write_business_objects` 照它分派，取不到就用 `catalog::placeholder_db_writer`（分派里原来的那个 `expect` 也就没有了）；记账照它数。
- 关系：`published_relationship_rows` 给出关系表里会写成 `<Rel>` 的行和各自的定义，两条跳过的规则（一端为空、两端都是图形表示）和对 `classify_relationship` 的调用都在里面。`write_relationships` 照它写，记账照它数。`classify_relationship` 本身没有动。

这个提交里还没有人读这份汇总，命令行、整包、导出包在后面三个任务里接上。

**绿。** 同一条命令（`runs/t21-green.log`）：`430 passed; 0 failed; 0 ignored; 0 measured; 1126 filtered out`，比红的那次多的 2 条是之后加的 `catalog.rs` 两条。`unsupported_item_types_still_fall_back_to_generic_placeholder` 没有改，两次都过。`tests/publish_mdf_load.rs` 整个文件 5 条都过（`runs/t21-mdf.log`），新加的那条打印 `A01: 3 items, 3 relationship rows, no shortfalls`。clippy 没有警告；`cargo fmt` 的检查第一遍报了新用例里的两处排版，排好之后是空的；不带默认特性的 985 条都过。

**没有看过红的。** 新用例 11 条（`xml_writer.rs` 5、`catalog.rs` 2、`model.rs` 3、`publish_mdf_load.rs` 1），红过的是上面 4 条。其余 7 条：

- `model.rs` 的三条（句子的写法）在红的那次就过了：桩只桩了 `database_shortfalls`，结构和 `describe()` 当时已经写好。
- `a_drawing_written_in_full_has_no_shortfalls` 和真实 A01 的 `a01_is_written_in_full_and_the_account_says_so` 要的就是空汇总，对着桩也过，红不了。它们防的是多记。
- `catalog.rs` 的两条是绿了之后加的，没有在没改的代码上跑过。

**超出计划原文的。** 条目的类型名为空时，记在 `(no item type)` 名下，不记在空名字下。图形表示作端点、另一端是条目的行，图形表示那一端照写手原有的查表记成 `Representation`。两样都由 `shortfalls_name_an_item_without_a_type_and_a_representation_end` 盯着；它和 `catalog.rs`、`model.rs` 的五条都是计划的五条之外加的。

**对照矩阵。** `compare.ps1 -A cp1 -B t21`：703 个文件逐字节相同。可执行文件 4 658 688 字节，SHA-256 `dd42c3a5…aeb0bba9`。

### T2.2 单图命令行说出来，加 `--strict`（2026-11-16，`c427901`）

**夹具。** `tests/fixtures/publish-shortfalls.sql`（53 行，建表加插入）：一张图 `SHORT-01`，一台换热器、一个容器、一个管口，一行管口到换热器的关系。命令行用例拿它在临时目录里建 `.sqlite`，T2.3、T2.4 的单测拿它建库，建表的代码只有这一份。

**红。** 加 7 条用例，在没改的代码上跑 `cargo test --locked -p pid-parse --test publish_xml_cli`（`runs/t22-red.log`）：

```text
test cli_database_input_warns_of_what_it_wrote_short_and_exits_zero ... FAILED
  stderr should say each kind of shortfall once; stderr:
  warning: `.sqlite` input is deprecated for publish fidelity; prefer `Export.mdf`. …
  Loaded drawing `SHORT-01` (DWG-SHORT-01 UID): 3 objects, 3 representations, 1 relationships
test cli_strict_turns_a_shortfall_into_exit_one_and_changes_no_file ... FAILED
  --strict should exit 1 on a shortfall; stderr:
  argument error: unknown flag: --strict
test result: FAILED. 27 passed; 7 failed; 0 ignored; 0 measured; 0 filtered out
```

第一条的 stderr 就是审核第 2 条说的现状：这张图里有一台写成占位的换热器和一行只凭类型定义的关系，命令行只报了载入的个数。另外五条是 `cli_help_documents_strict_and_drops_the_unreachable_feature_note`（用法说明里没有 `--strict`）和 `cli_strict_on_a_drawing_written_in_full_exits_zero`、`cli_strict_with_a_pid_input_is_an_argument_error`、`cli_strict_with_list_drawings_is_an_argument_error`、`cli_strict_with_a_backup_directory_is_an_argument_error`，后四条都停在 `unknown flag: --strict` 上。

**改。** `CliOptions` 加 `strict`。单图这一支在文件写完之后取汇总，每类往 stderr 打一行：

```text
warning: 1 object written as a <PIDItem> placeholder, class and attributes left out: Exchanger 1
warning: 1 relationship row defined from the item types of its ends alone: Nozzle -> Exchanger as Relationship 1
```

不带 `--strict`，退出码和原来一样。带了而且汇总不空，等别的都做完再退出 1：文件是同样的字节，`--diff-against` 的报告照出，stderr 最后一行是 `--strict: the drawing is not written in full (see the warnings above); exit 1.`。`.pid` 输入带它、或者和 `--list-drawings` 一起用，按用法错误退出 2，照 `--style` 的先例。用法说明加了 `--strict` 一段，删了 `(backup-mdf feature; without it the input still exits 2)` 那句走不到的话。

**绿。** 同一条命令（`runs/t22-green.log`）：`34 passed; 0 failed`。之后又加了一条 `cli_strict_is_decided_after_the_diff_report_not_instead_of_it`，再跑是 `35 passed`（`runs/t22-green2.log`）。clippy 没有警告，fmt 没有差异。

**没有看过红的。** `cli_strict_is_decided_after_the_diff_report_not_instead_of_it` 是绿了之后加的，没有在没改的代码上跑过。

**超出计划原文的。** 细化计划列了 5 条用例，实际 8 条。多的三条：`--strict` 和 `--list-drawings` 一起用退出 2（规则在细化计划 §4.3 里，用例清单没列）；带 `--diff-against` 时差异报告照出、`--strict` 在它之后才定退出码（上面那一条）；目录输入带 `--strict` 退出 2。最后这条是过渡：细化计划 §4.3 定的是目录接受 `--strict`，可整包这一支到 T2.3 才报降级，在那之前收下它就是收下来不管。所以这个提交先拒绝，T2.3 接上之后改成接受，用例跟着换。

**对照矩阵。** `compare.ps1 -A t21 -B t22`：703 个文件，700 个相同。变的 3 个是 `help\stdout.bin`（95 → 105 行）、`no-args\stderr.bin`（96 → 106 行）和汇总各项大小的 `summary.txt`，都是用法说明带来的。A01 各项的 stdout、stderr、退出码没有变。可执行文件 4 676 608 字节，SHA-256 `1eba1425…18592b8f`。

### T2.3 整包记下来（2026-11-16，`ace68c4`）

**先拆。** `publish_mtf_package` 里从列出 `T_Drawing` 到写 `_Index.json` 的那一段原样搬进 `publish_staged_database`（crate 内部，参数是暂存好的库连接），`publish_mtf_package` 只剩读包、暂存、调它。搬完先单独验：`publish::backup_package` 的 4 条单测过（`runs/t23-move-lib.log`），`sql_server_mtf_directory_publishes_t_drawing_rows` 过（`runs/t23-move-mtf.log`），`compare.ps1 -A t22 -B t23-move` 703 个文件逐字节相同（可执行文件 4 676 608 字节，SHA-256 `83583437…39c71cfe`）。搬动和后面的改动在同一个提交里。

**红。** 用例拿夹具建内存库，包目录是一个空的临时目录，把 `publish_staged_database` 从头跑到尾。四份记录：

| 记录 | 跑的 | 结果 | 失败的用例 |
|---|---|---|---|
| `runs/t23-red-lib.log` | `publish::backup_package` 的单测 | 4 过 4 败 | `a_staged_drawing_written_short_says_so_in_its_coverage_report`（`shortfalls` 是 `Null`）、`a_staged_drawing_written_short_is_written_and_listed_among_the_problems`（`problems` 是 `[]`）、`a_staged_drawing_written_short_is_named_in_the_report`（报告的新字段是 `[]`）、`a_staged_drawing_written_in_full_has_the_section_and_no_problem`（`shortfalls` 是 `Null`） |
| `runs/t23-red-writer.log` | `publish::pid_writer` 的单测 | 20 过 1 败 | `the_database_coverage_report_states_what_was_written_short` |
| `runs/t23-red-bin.log` | 可执行文件自己的单测 | 1 过 1 败 | `strict_turns_a_package_drawing_written_short_into_exit_one`（左 0，右 1） |
| `runs/t23-red-cli.log` | `publish_xml_cli` 里筛出来的 8 条（其余 28 条没跑） | 5 过 3 败 | `cli_help_documents_strict_and_drops_the_unreachable_feature_note`（目录那一行用法没有 `--strict`）、`cli_strict_with_a_backup_directory_is_accepted`、`cli_strict_on_a_sql_server_package_written_in_full_exits_zero`（后两条都停在 T2.2 加的那句拒绝上） |

库这一侧先绿（`runs/t23-mid-lib.log` 8 过，`runs/t23-mid-writer.log` 21 过）。可执行文件那边又加了两条，`runs/t23-mid-bin.log` 是 1 过 3 败：`the_strict_verdict_counts_the_drawings_and_is_said_only_with_strict`（左 `None`）、`a_package_drawing_written_short_is_warned_of_under_its_name`（左 `[]`），和表里那一条。

**改。**

- `write_database_coverage_json` 自己从图里取汇总，在 `document_counts` 之后写 `shortfalls` 一段。这一段恒有，写全了的图各项是 0。
- 逐图循环把句子放进 `_Index.json` 里这张图的 `problems`，带上图名放进顶层的 `problems`。图仍算写出，`failed` 不变。
- `BackupPublishReport` 加公有字段 `shortfalls`，元素是新的公有类型 `PackageDrawingShortfalls`：图的 UID、图名和它的 `DatabaseShortfalls`。Oracle 包和没有 dump 的包，这个字段恒为空。
- 命令行的 `run_package` 每句打一行 `warning: <图名>: …`。带 `--strict` 而且有这样的图，退出 1，stderr 最后一行是 `--strict: N drawings are not written in full (see the warnings above); exit 1.`。
- 目录输入从这个提交起接受 `--strict`。包是哪一种要读完才知道，所以 Oracle 包和没有 dump 的包也收下它，只是不起作用；用法说明里写了。

**绿。** `publish::backup_package` 8 过，`publish::pid_writer` 21 过，可执行文件的单测 4 过，`publish_xml_cli` 整个文件 36 过，`publish_chain_faults` 7 过，`sql_server_mtf_directory_publishes_t_drawing_rows` 过（`runs/t23-green-*.log`）。clippy 没有警告；fmt 第一遍报了新用例里的几处排版，排好之后是空的。

**没有看过红的。** 可执行文件单测里的 `a_failed_drawing_or_an_unclean_gate_is_exit_one_whatever_strict_says`：有失败的图或者门禁有发现时，带不带 `--strict` 都退出 1。这是原有的行为，红的那两次它都过。

**没有验到的。** 仓库里没有一个 SQL Server 包有写不全的图，`Export.dmp` 里的 MDF 也没法现造。所以整包的告警行和 `--strict` 的退出码是在 `run_package` 调的三个函数上验的（`package_warnings`、`package_exit_code`、`strict_package_verdict`），没有经过可执行文件。经可执行文件真跑的是写全了的 `TEST02_p`：带 `--strict` 退出 0。

**超出计划原文的。** 报告新字段的元素单成一个公有类型 `PackageDrawingShortfalls`，带 `describe()`，给出带图名的句子；细化计划只说了「多一个字段」。T2.2 那条「目录带 `--strict` 是用法错误」的用例换成了 `cli_strict_with_a_backup_directory_is_accepted`，另加 `cli_strict_on_a_sql_server_package_written_in_full_exits_zero`。`pid_writer.rs` 一条、可执行文件四条单测是计划的四条之外加的；`tests/publish_chain_faults.rs` 加了一句断言：两个从 `.pid` 读图的包，新字段都是空的。

**对照矩阵。** `compare.ps1 -A t22 -B t23`：703 个文件，698 个相同。变的 5 个：`help\stdout.bin`（105 → 114 行）、`no-args\stderr.bin`（106 → 115 行）；`pkg-test02` 和 `pkg-test02-gate` 两项的 `A01_Coverage.json`（397 → 588 字节，原有的内容没动，后面多出全是 0 的 `shortfalls` 一段）；`summary.txt`。Oracle 的两个整包和单图各项逐字节不变。可执行文件 4 723 200 字节，SHA-256 `414d2a8b…ae13117f`。

### T2.4 导出包也记（2026-11-16，`c974f47`）

**红。** 库里 `export_bundle` 的单测（`runs/t24-red-lib.log`），`15 passed; 2 failed`：

```text
test export_bundle::tests::export_bundle_publish_xml_lists_what_was_written_short ... FAILED
   left: None
  right: Some(["1 object written as a <PIDItem> placeholder, class and attributes left out: Exchanger 1", "1 relationship row defined from the item types of its ends alone: Nozzle -> Exchanger as Relationship 1"])
test export_bundle::tests::export_bundle_publish_xml_writes_data_meta_and_success_status ... FAILED
   left: None
  right: Some([])
```

第二条是原有的用例，加了一句断言：A01 写全了，`shortfalls` 应当是空数组。

**改。** `ExportBundlePublishStatus` 加公有字段 `shortfalls: Option<Vec<String>>`，`publish/status.json` 在 `meta_xml_written` 之后多 `shortfalls` 一个键，放的是 `DatabaseShortfalls::describe` 的句子。写出了 XML 就有这个键，`[]` 表示写全了。没有生成 XML 的那份状态是 `None`，不写这个键；读回这个提交之前写的状态文件也是 `None`。没算过和没有不是一回事。图照写，`state` 仍然是 `written`，什么都不打印，`pid_inspect --export-bundle` 不用改。`docs/pid-export-bundle-contract.md` 写了这个键。

**绿。** 同一组 17 条都过（`runs/t24-green-lib.log`），`inspect_cli` 里筛出来的 4 条过（`runs/t24-green-inspect.log`）。clippy 没有警告，fmt 没有差异。

**没有看过红的。** `a_status_file_without_shortfalls_reads_as_not_counted`：读一份没有这个键的状态文件，要的是 `None`。红的那次字段已经加上、恒为 `None`，它当时就过。

**超出计划原文的。** `docs/pid-export-bundle-contract.md` 的说明（细化计划的文件清单里只有 `src/export_bundle.rs`），和上面那条没看过红的用例。

**对照矩阵。** `compare.ps1 -A t23 -B t24`：703 个文件逐字节相同。矩阵跑的都是 `pid_publish_xml`，导出包不在里面，这一比验的是别处没被带动。可执行文件 4 722 176 字节，SHA-256 `baf3e4ac…df77d941`。

### 检查点 2（2026-11-16）

**门禁。** `gates.ps1 -Label cp2`，在 `c974f47` 上跑，开跑时工作区没有未提交的改动；日志在 `target/publish-audit/review/gates/cp2/`：

| 门禁 | 退出码 | 用时 | 读数 |
|---|---|---|---|
| `cargo build --locked --workspace --all-targets` | 0 | 119 s | |
| `cargo test --locked --workspace --all-targets` | 0 | 1 558 s | 474 个测试程序；2 552 通过，0 失败，3 忽略；`skipping: ` 提示 21 行 |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | 0 | 2 s | 0 条警告 |
| `cargo fmt --all -- --check` | 0 | 4 s | |
| `bash .github/scripts/check-missing-docs.sh` | 0 | 7 s | `current=0, baseline=0` |
| `cargo test --locked -p pid-parse --no-default-features --lib` | 0 | 2 s | 985 通过 |
| `cargo check --lib --locked -p pid-parse --no-default-features --target wasm32-unknown-unknown` | 0 | 1 s | |
| `bash .github/scripts/check-byte-audit-baselines.sh` | 0（补跑取的，见下） | 19 s | 3 份基线都比了，各 `Regressions: 0`，跳过 0 |

与检查点 1 比，通过的测试多 31 条（2 521 → 2 552），正是阶段 2 加的：库内单测 17 条（`xml_writer.rs` 5、`model.rs` 3、`catalog.rs` 2、`pid_writer.rs` 1、`backup_package.rs` 4、`export_bundle.rs` 2），可执行文件的单测 4 条，`publish_xml_cli` 9 条，`publish_mdf_load` 1 条。忽略数、`skipping: ` 行数、缺文档数、不带默认特性的 985 条都没变。

第八道的日志是完整的：11:48:02 到 11:48:21，三份基线各一行 `Regressions: 0`，最后一行是 `completed comparisons=3, skipped=0.`；脚本开着 `set -euo pipefail`，走到这一行就是退出 0。但 `summary.txt` 里它那一行，到写这段记录时（12:00）还没有出来。这一道重新编译并链接了 `pid_inspect`（18.6 s），同一时刻（11:48:20）起来了一个 `VCTIP.EXE`（MSVC 工具链的遥测进程），到 12:00 还在；`gates.ps1` 的进程也还在，名下已经没有直接的子进程。`Start-Process -Wait` 要等被起的进程连同它留下的后代都退出，脚本应当是停在这上面。没有去动那个进程，另用 `gates.ps1 -Label cp2-08 -Only 08` 补跑了一次取退出码：退出 0，2 s，不用再编译（日志在 `gates/cp2-08/`；当时工作区多了 `CHANGELOG.md`、`AGENTS.md` 两份还没提交的说明）。表里的 19 s 是第一次那份日志的起止时间。

**对照矩阵。** `c974f47` 的发布构建（`target/publish-audit/bin/pid_publish_xml-c974f47-release.exe`，4 722 176 字节，SHA-256 `baf3e4ac…df77d941`，和 T2.4 那个是同一份：源码没有再变，cargo 没有重编）跑了一遍矩阵（`target/publish-audit/review/cp2/`，136 s）。这次是 61 项：原来的 59 项，加上带 `--strict` 的 A01 单图（`a01-strict`）和 `TEST02_p` 整包（`pkg-test02-strict`）。

- `compare.ps1 -A t24 -B cp2`：原有的 703 个文件里 702 个相同，变的只有 `summary.txt`（多了两项）；新两项带来 14 个文件。
- `compare.ps1 -A cp1 -B cp2`（`runs/cp2-vs-cp1.txt`）：原有的 703 个文件里 698 个相同，变的 5 个就是 T2.2、T2.3 两节列的：`help\stdout.bin`（95 → 114 行）、`no-args\stderr.bin`（96 → 115 行）、两项的 `A01_Coverage.json`、`summary.txt`。
- 新两项都退出 0，stderr 里没有告警，也没有 `--strict` 那一行。写出的文件和不带开关的那一项逐字节相同：`a01-strict` 的两个文件对 `a01-out-meta`，`pkg-test02-strict` 的四个文件对 `pkg-test02`。
- 检查点 2 的全部输出里有 327 份 XML（312 个文件，15 份写在 stdout 上），`<PIDItem>` 0 个，`DefUID="Relationship"` 0 条。整个矩阵里出现 `<PIDItem>` 这几个字的只有 `help\stdout.bin` 和 `no-args\stderr.bin`，是用法说明里讲 `--strict` 的那一段。

这与细化计划检查点 2 预期的两处一致：打印用法说明的两项跟着用法说明变，`TEST02_p` 的 `A01_Coverage.json` 多出那一段（这个包在矩阵里发布了两次，带和不带 `--diff-against` 各一次，所以是两个文件）。

**说明。** `CHANGELOG.md` 的 `[Unreleased]` 加了阶段 2 一条，写明两个公有结构各加了一个字段、输出变了哪些、哪些没有验到；`AGENTS.md` 的用例表补上了新用例。两处的数字和名字逐项对过代码和上面的记录。

**顺带记下的。**

1. 真实数据上，这一阶段验到的只有「没有降级时什么都不变」。1 283 这个数是照载入规则算的，不是跑的。拿到一个带换热器、机械设备的 SQL Server 包，跑一遍就有逐图的数字。
2. 只开 `backup-exp` 特性编库时有一条 `dead_code` 警告：`PublishItemRow` 的 `db_subtables` 字段没有人读（`runs/t21-exponly.log`、`runs/t23-exp-only.log`；两份记录没有留命令行，从警告和编进去的用例看是这个组合）。读它的两处，一处在 `sqlite_load.rs`（属 `backup-mdf`），一处在测试里。`a4b490e` 上读它的也只有这两处，所以不是阶段 2 带来的（基线上没有重跑）。CI 不编这个组合，门禁里看不到它。这一轮没有动。同一个组合下 `publish::backup_package` 原有的 4 条单测过（`runs/t23-exp-only-tests.log`）；T2.3 新加的 4 条要建内存库，只在 `backup-mdf` 下编。
3. 命令行的降级用例走的是 `.sqlite` 输入，这个入口标着 deprecated。哪天它被删掉，这几条要换入口，而 `.mdf` 没法现造（细化计划 §6 已经写了）。
