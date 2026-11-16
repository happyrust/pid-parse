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

- [x] 全套门禁与 vendored 件自己的测试通过（vendored 件按 T3.2 写的 `cargo test --lib`；它的集成测试有 2 条改前就不过，见 §9）
- [x] 对照矩阵逐字节不变
- [x] 改前改后的读数发给你

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

- [x] 全套门禁通过
- [x] 对照矩阵逐字节不变（各项发布写出的文件和它们的 stdout、stderr、退出码都没变；`help`、`no-args` 两项里的用法说明多了 T4.3 按「做什么」加的两段，717 个文件里差 3 个，见 §9）
- [x] 结果发给你

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

### T3.1 先量（2026-11-16，`6a47804`）

**怎么量的。** 探针 `examples/probe_what_staging_an_mdf_costs.rs` 照 `open_mdf_as_sqlite` 的做法走一遍：`PUBLISH_TABLES` 的每张表各开一个 `MdfDatabase`，文件包在一个计字节的 `Read` 里，把这张表的行读完。它打印每张表开库读了多少字节、读行又读了多少、合计是镜像大小的几倍。然后跑真正的装载函数并计时，再逐表拿它暂存的行数对探针读到的行数，对不上就退出 1：读的行不一样，量的就不是同一件事。带 `--mtf` 时输入是 MTF 的 `Export.dmp`，镜像照整包的做法用 `mdf_bytes_from_mtf_dump` 取出来；带 `--stage-only` 时只跑装载函数，留给外面取峰值内存。

峰值内存是从进程外面取的。`measure.ps1`（在 `target/publish-audit/review/`，不入库）起进程，等它退出，再对进程句柄调 `GetProcessMemoryInfo` 读 `PeakWorkingSetSize`。它和 `PeakWorkingSet64` 是同一个计数；`System.Diagnostics.Process` 的那个属性在进程退出之后读不出来，所以直接调了 API。每组跑 5 次，下面写的是 5 次的范围。

都是发布构建，stable 1.99.1：探针 2 280 448 字节，SHA-256 `13a46933…28cda8ac`；`pid_publish_xml.exe` 4 722 176 字节，SHA-256 `ebe41585…a45ca7e0`。输出在 `target/publish-audit/review/` 下：探针的两份是 `runs/t31-probe-mdf-2.log`、`runs/t31-probe-mtf-1.log`，峰值内存的六组是 `measure/t31-*/summary.txt`。

**读数。**

| | A01 的 `Export.mdf` | `TEST02_p` 整包 |
|---|---|---|
| 文件 | 19 922 944 字节，2 432 页 | `Export.dmp` 20 012 544 字节，里面的镜像 19 922 944 字节 |
| 暂存从镜像读入 | 435 953 664 字节 | 435 953 664 字节，逐表与左边相同；另外 dump 整个读一次，镜像往临时文件写一次 |
| 读入比文件大小 | **21.88 倍** | 镜像的 **21.88 倍**，dump 的 21.78 倍 |
| 峰值工作集，只跑暂存（探针 `--stage-only`） | 29 642 752–29 868 032 字节，**1.49–1.50 倍** | 69 496 832–69 918 720 字节，**3.47–3.49 倍** |
| 峰值工作集，命令行整个跑完 | 29 802 496–29 949 952 字节，1.50 倍 | 70 283 264–70 594 560 字节，3.51–3.53 倍 |
| 暂存用时（探针里计的，一次） | `open_mdf_as_sqlite` 459.6 ms | `open_mtf_dump_as_sqlite` 490.2 ms，另读 dump 6.4 ms |
| 命令行从起到退 | 460–533 ms | 520–548 ms |

命令行的两项就是矩阵里的 `a01-out-meta` 和 `pkg-test02`。整包那一列的倍数是对 `Export.dmp` 的大小算的。什么都不做的进程也有 4.2–4.7 MB 的峰值工作集（探针不带参数、命令行 `--help`），合文件大小的 0.21–0.24 倍，上面的数里都含着它。

**21.88 倍是怎么来的。** 读取器只进不退：要第 N 页，就把第 N 页之前还没读过的页全读进来。开库要读引导页和五张系统表，其中排得最靠后的一页是第 2 145 页（从 0 数），所以每开一次库，读到第一行之前先读 2 146 页，17 580 032 字节，文件的 88%。24 张表各开一次，是 421 920 768 字节。读行另外读了 14 032 896 字节（1 713 页）：11 张表的数据页排在更后面，接着往后读；12 张表的数据页开库时已经读过，读行不再碰文件；`T_ProcessEquipment` 这个库里没有，装载跳过它，开库的 2 146 页照样读了。23 张表共 243 行，逐表与装载暂存的行数一致。

探针那一遍只读不存，24 张表合计 407.8 ms；装载函数连读带存 459.6 ms。时间几乎都花在一遍遍重读上。

**过线了。** Q4 的线是读入字节达到文件大小的 3 倍，或峰值内存达到 2 倍。读入两组都是 21.88 倍，过线。峰值内存，单个 `.mdf` 是 1.5 倍，没过；整包是 3.5 倍，过线。照 Q4，接着做 T3.2、T3.3。

整包的 3.5 倍，按代码看是三份叠在一起：`publish_mtf_package` 把 `Export.dmp` 整个读进内存（`backup_package.rs:1158`），`mdf_bytes_from_mtf_dump` 再拷出一份镜像，读取器又把读过的页全留在缓存里。这是照代码推的，量到的只有总数。T3.2 给缓存设上限，T3.3 去掉那份拷贝；dump 本身那一份，T3.3 写的做法去不掉，所以改完以后整包这条路预计还剩 1 倍多一点，以到时的读数为准。

**超出计划原文的。** 计划写的是 T3.1 只动一个 example 和 `Cargo.toml`。探针要「按 `PUBLISH_TABLES` 逐表」，而这张清单原来是 `mdf_load.rs` 私有的。把它改成了 `pub`，加了文档注释，那个文件里别的没有动。在探针里抄一份的话，清单以后一改，两边对不上也看不出来。探针还多做了两件计划没写的事：逐表核对行数，和 `--mtf`（整包那一组读数要用）。

**没有验的。** 探针没有自己的测试。它的数靠两处对得上：逐表的行数与装载一致；每张表读入的字节都是 8 192 的整数倍，开库那一段 24 次一字不差。`extracted/Export.mdf` 的全部字节在 `Export.dmp` 里连续出现一次（偏移 8 704，SHA-256 相同），长度与探针从 dump 里取出的镜像相同；取出的镜像本身没有拿来和这个文件直接比。用时只在这一台机器上量过，文件当时都在系统缓存里。

**顺带记下的。**

1. 系统表读到一半断掉时，读取器不报错。`vendor/oxidized-mdf/src/sys.rs` 里读系统表的循环是 `while let Some(Ok(page)) = …`，读页出错就停，停下之前读到的照用。把 `Export.mdf` 截到前 2 000 页试了（开库要的第 2 145 页不在了）：`MdfDatabase::open` 成功，24 张表里只认得 `T_InlineComp`、`T_SignalRun` 两张，其余 22 张被当成库里没有，`open_mdf_as_sqlite` 照常返回，探针退出 0。命令行 `--list-drawings` 退出 1，报的是 `no such table: T_Drawing`，没有提文件不完整。截到前 1 000 页也是这样。截到前 2 146 页（系统表完整，后面的数据页没了）则照实报 `IO Error: failed to fill whole buffer`，退出 1。`sqlite_load.rs` 里有几处查询把缺表当成空表（`prepare_optional`），所以要是截断处恰好留下了 `T_Drawing` 而丢了这样的表，图会少东西而不报错；这一句是照代码推的，没有造出这样的文件来试。这是改之前就有的行为，T3.2 不改变它。试的文件和输出在 `target/publish-audit/review/t31-truncated/`。
2. 检查点 2 的记录说第八道门禁在 `summary.txt` 里的那一行一直没有出来。后来那个 `VCTIP.EXE` 退了，行补了出来：`08-byte-audit  exit 0    921 s`。退出码与补跑的一致；921 s 里门禁自己只有 19 s，其余是脚本在等。`gates.ps1`、`run-stable.ps1` 已经改成只等自己起的那个命令（`WaitForExit()`，不再用 `Start-Process -Wait`）。这次量之前的第一次发布构建又碰上了同一件事，改了以后脚本不再停在这上面。

### T3.2 读取器按页寻址（2026-11-16，`9dbfff8`）

**红。** 在 `vendor/oxidized-mdf/src/lib.rs` 里先写了 8 条单元测试，在没改的读取器上跑（只多了一个还没人用的常量 `PAGE_CACHE_LIMIT = 256`）：5 条红，3 条本来就绿（`runs/t32-red-1.log`）。

| 测试 | 在没改的读取器上 |
|---|---|
| `a_page_reads_without_the_pages_before_it` | 红：要第 12 页，从文件读了 106 496 字节（13 页），该读 8 192 |
| `a_chain_is_followed_to_wherever_its_pages_lie` | 红：链 12 → 3 → 7 的页读对了，读了 106 496 字节，该读 24 576 |
| `a_page_the_reader_keeps_is_not_read_again` | 红：第 5 页要两次，读了 49 152 字节（6 页），该读 8 192 |
| `the_reader_keeps_no_more_pages_than_its_limit` | 红：读过 264 页，缓存里留着 264 页，上限 256 |
| `a_page_the_reader_let_go_reads_again` | 红：读过 264 页以后再要第 0 页，没有去读文件（0 字节，该是 8 192）：它从来没被放掉 |
| `an_earlier_page_reads_after_a_later_one` | 绿 |
| `a_page_past_the_end_is_an_io_error_and_the_ones_before_it_still_read` | 绿 |
| `an_image_that_ends_before_its_boot_page_is_an_io_error` | 绿 |

**计划里有一句不成立。** T3.2 的验收写的是「先读后面的页、再读前面的页，能读到（今天报 `forward-only reader cannot re-read an earlier page`）」。今天读得到，上表第六条就是它。`Page::try_from` 对任何 8 192 字节都成功（`pages.rs:893`，只取页头里的两个字段），所以读过的页一页不落都进了缓存；缓存没有上限；`read_page` 先查缓存，查的时候还不看文件号。那条错误走不到。这条测试留下了，改完以后它守的是「往回读能读对」；红的是数字节的三条和管缓存的两条。后两条绿的把住不该变的：页在文件末尾之后，还是 `IoError`（`UnexpectedEof`）；镜像不到引导页那么长，也还是。

**改。** `PageReader` 的源从 `Box<dyn Read>` 换成能 seek 的（私有 trait `PageSource: Read + Seek`）。`read_page` 先查缓存，没有就 seek 到 `page_id × 8192`，读 8 192 字节，解析一次，放进缓存。原来是从当前位置一页页读到要的那一页，每页解析两遍（`read_next_page` 里一遍，回到循环里又一遍）。缓存 `PageCache` 按页号建键，最多留 256 页（2 MiB），多一页就放掉最早读进来的那一页；放掉的页再要，就再读一次。`MdfDatabase::open(path)` 的签名没动。`from_read` 现在要 `Read + Seek`，只读引导页（第 9 页），不再把第 0–8 页读一遍。文件头的改动清单补了 `Modified: 2026-11-16` 一段。

**绿。**

- vendored 件自己的测试（`cd vendor/oxidized-mdf && cargo test --no-fail-fast`，`runs/t32-green-1.log`）：单元 46 过（原有 38，新增 8）；集成 20 条里 18 过、2 不过；文档 4 过。改之前（`runs/t32-vendored-before-all-2.log`）：单元 38 过，集成同样 18 过 2 不过，文档 4 过。不过的是同样两条、同样的读数，见顺带记下的第 1 条。计划要的 `cargo test --lib` 就是其中单元那一组。
- 在 vendored 目录里 `cargo clippy --all-targets`：0 条警告。
- 根 crate：`tests/publish_mdf_load.rs` 5 过，`tests/publish_a01_raw_residual.rs` 4 过 2 忽略，断言没动；`cargo clippy --locked --workspace --all-targets -- -D warnings` 退出 0；`cargo fmt --all -- --check` 退出 0。
- 对照矩阵 `t32` 对 `cp2`：717 / 717 个文件逐字节相同。矩阵是提交之前在工作区上跑的，工作区当时的内容就是 `9dbfff8`。

**逐行核对（计划之外加的）。** A01 的发布表每张只有一到三页，靠它看不出长链上、往回读的时候对不对。所以写了个临时程序（`target/publish-audit/review/t32-old-reader/dumper.rs`，不入库）：一个 `MdfDatabase` 开到底，按表名顺序把库里每张表的每一行写出来，`rows()` 一遍，`try_rows()` 再一遍。分别接上改前的读取器（`fe52012` 那一版的 `vendor/oxidized-mdf`，另放了一份）和改后的，依赖版本相同（两边的 `Cargo.lock` 除了包名一字不差），各跑三个库，输出逐字节比：

| 库 | 表 | `rows()` | `try_rows()` | 输出 | 改前改后 |
|---|---|---|---|---|---|
| A01 的 `Export.mdf` | 156 | 2 569 行 | 2 568 行，1 条错误 | 1 748 944 字节，SHA-256 `95e3fd49…271262b8` | 相同 |
| `spg_verein_TST.mdf`（vendored 件自带，14 745 600 字节） | 134 | 7 753 行 | 7 403 行，352 条错误 | 3 907 179 字节，SHA-256 `ecdc8cba…cac0a84e` | 相同 |
| `AWLT2005.mdf`（vendored 件自带，5 242 880 字节） | 读到 `SalesOrderDetail` 时 panic | | | panic 之前写出的 7 997 945 字节，SHA-256 `25634a4d…fde007db` | 相同，panic 在同一处 |

panic 见顺带记下的第 3 条。输出在 `target/publish-audit/review/t32-old-reader/rows/`。

**读数。** 发布构建，stable 1.99.1：探针 2 294 784 字节，SHA-256 `d31b2053…c868afbf`；`pid_publish_xml.exe` 4 723 712 字节，SHA-256 `dd024f16…1c9d5993`。量法和 T3.1 一样，峰值工作集各跑 5 次。输出：`runs/t32-probe-mdf-1.log`、`runs/t32-probe-mtf-1.log`、`measure/t32-*/summary.txt`。

| | 改前（T3.1） | T3.2 之后 |
|---|---|---|
| 开一次库读入 | 17 580 032 字节（2 146 页） | 999 424 字节（122 页） |
| 24 张表读行读入 | 14 032 896 字节（1 713 页） | 221 184 字节（27 页） |
| 合计，比文件大小 | 435 953 664 字节，21.88 倍 | 24 207 360 字节，**1.22 倍** |
| 同一个读取器把一页读了两次 | 没数 | 2 955 次读页里 0 次 |
| `open_mdf_as_sqlite` | 459.6 ms | 69.9 ms |
| `open_mtf_dump_as_sqlite` | 490.2 ms | 99.5 ms |
| 峰值工作集，只跑暂存，`.mdf` | 29 642 752–29 868 032 字节，1.49–1.50 倍 | 9 658 368–9 973 760 字节，0.48–0.50 倍 |
| 峰值工作集，只跑暂存，整包 | 69 496 832–69 918 720 字节，3.47–3.49 倍 | 49 762 304–50 003 968 字节，**2.49–2.50 倍** |
| 峰值工作集，命令行，A01 | 29 802 496–29 949 952 字节，1.50 倍 | 9 728 000–10 264 576 字节，0.49–0.52 倍 |
| 峰值工作集，命令行，整包 | 70 283 264–70 594 560 字节，3.51–3.53 倍 | 50 495 488–50 737 152 字节，2.52–2.54 倍 |
| 命令行从起到退，A01 | 460–533 ms | 85–98 ms |
| 命令行从起到退，整包 | 520–548 ms | 127–133 ms |

整包那一组逐表的读入与 `.mdf` 相同。两条路的峰值各降了约 20 MB。整包还在 2.5 倍，线以上：dump 整个在内存里一份，拷出的镜像又一份，T3.3 去掉后一份。

256 页在这个库上够用：开一次库 122 页，24 张表的行一共 27 页，没有一页要了第二次，所以上限没有让它多读一次。

**超出计划原文的。**

1. `from_read` 成了泛型：`from_read<R: Read + Seek + 'static>(read: R)`。`Box<dyn Read + Seek>` 写不出来（一个 trait 对象只能带一个非自动 trait），所以接泛型，里面再装箱。调用处从 `from_read(Box::new(file))` 变成 `from_read(file)`。
2. 计划写 `from_read`「仓库里只被 `open` 调用」，那是 T3.1 之前；现在 T3.1 的探针也调它。探针里计数的源要能 seek 才编得过，所以 `examples/probe_what_staging_an_mdf_costs.rs` 也在这个提交里；顺手让它数「同一个读取器把一页读了两次」的次数，上面那一行 0 次就是它数的。
3. 测试写了 8 条，计划点了两条。逐行核对也是计划之外的。
4. `PagePointer::with_page_id` 现在没有人调用了。它是上游原有的函数，在 `pages.rs`，这个 crate 整体 `allow(dead_code)`；没有删。

**没有验的。**

- 「同一页不再解析两遍」没有测试守着：解析没有副作用，从外面看不出做了几遍。依据是代码里现在只有一处调用。
- 峰值内存「不跟着文件大小走」没有第二种大小的发布库来验。依据是缓存有上限这一条，加上这一个样本上降下来的读数。
- 一趟要读 256 页以上的表，缓存放掉前面的页以后会不会回头再要：照代码不会（顺着链往下，每页要一次）；逐行核对的三个库里有没有这么长的链，没有去数。
- 多文件的库（带 `.ndf`）：页指针里的文件号和以前一样不看，所有页都从这一个文件读。没有这样的样本。
- 用时只在这一台机器上量过，文件当时都在系统缓存里。

**顺带记下的。** 下面几条都是改之前就有的，改前改后一样，T3.2 没有动它们。

1. vendored 件自己的集成测试有 2 条不过，都在 `spg_verein_TST.mdf` 上：`number_of_rows::case_3`（`tbl_Bankleitzahlen` 期望 3 549 行，读到 3 548；测试里自己写着 TODO，说正确的数应该是 3 643）和 `rows::case_5`（`tbl_Mitglied` 第一行的 `Kontosaldo` 期望 `Some(Null)`，读到 `None`，就是这一列没有解析出来）。门禁只跑 `cargo test --lib`，所以平时看不到。没有查是哪次改动之后开始不过的。
2. **空表会读出一行假数据。** 一张表在库里一页都没有时，系统表里记的首页指针是 `0:0`。读取器把它当成真的页指针，去读第 0 页——文件头页——把上面那条记录按这张表的列解析成一行。A01 的 `Export.mdf` 里，24 张发布表有 10 张的起始指针全是 `0:0`：`T_EquipComponent`、`T_Pipeline`、`T_InlineComp`、`T_PipingComp`、`T_Instrument`、`T_InstrFunction`、`T_ItemNote`、`T_Exchanger`、`T_Mechanical`、`T_SignalRun`。每张读出一行、暂存进一行，行里是同一串乱码；全库 156 张表里有 56 张读出了带这串乱码的行。说这 10 张表本来是空的，依据是这个库的 `T_ModelItem` 只有 4 行，类型是 `Vessel`、`Nozzle`、`PipeRun`、`PipingPoint`，没有这些表管的东西；没有拿 SQL Server 去查。所以 T3.1 写的「23 张表共 243 行」里，真的是 13 张表 233 行，另外 10 张表读行读的那一页是第 0 页。发布结果没有看到受影响：对照矩阵 `t32` 的 717 个文件里，没有一个出现这串乱码，也没有一个提到这些表的表名（查了其中四个）；A01 对参考的四条测试（`tests/publish_a01_raw_residual.rs`）一直是过的。但装载的查询里有没有哪一条会把这一行带出去，没有逐条查。起始指针是用一条临时测试打出来的（`runs/t32-scratch-iter-1.log`），那一行的内容在逐行核对的输出里。页头解析已经把「下一页」指针的页号 0 当成没有下一页（`pages.rs:782`），起始指针没有这样处理。现有的一条测试是靠这一行过的：`tests/publish_mdf_load.rs` 的 `strict_rows_load_a01_equip_component_table` 断言 `T_EquipComponent`「有行」，而这张表读出来的只有这一行。哪天把起始指针 `0:0` 改成「没有页」，这条断言要跟着改。
3. 读取器在自带的样本上会 panic：读 `AWLT2005.mdf` 的 `SalesOrderDetail` 时，`rust_decimal` 报 `Number exceeds maximum value that can be represented.`（`rust_decimal-1.43.0` 的 `decimal.rs:481`，在 `Decimal::from_i128_with_scale` 里：数超出 96 位它就 panic，不返回错误）。vendored 件里调它的有三处：`money`、`smallmoney` 传的是 `i64`、`i32`，超不出；剩下 `pages.rs:292`（`parse_decimal_opt`），`decimal` / `numeric` 的精度超过 28 位时取 16 个字节当 `i128`，会超出。检查点 3 时确认了就是这一处：在读取器的临时副本里、这一句前面加一行打印，panic 之前最后打出的是精度 38、小数 6 位，取到的数是 253 605 848 548 607 286 669 085 476 477 182 361，超出 96 位。（带回溯跑，栈帧全是 `<unknown>`：这台机器上的构建目录里没有 `.pdb`。）发布表里要是有这样的值，命令行会直接崩。A01 的库里没有这样的列：同样的打印加在 `parse_decimal_opt` 的开头，把库里有的 23 张发布表读一遍（243 行，0 条错误），一次也没有打出来，也就是这 23 张表没有 `decimal` / `numeric` 列。拿 `AWLT2005.mdf` 做了对照，`Product` 的 295 行各打出一次精度 8、小数 2 位。`T_ProcessEquipment` 这个库里没有，它的列不知道；别的版本的库也没有看过。输出在 `target/publish-audit/review/cp3-checks/`（`awlt-panic-callsite.txt`、`a01-decimal-columns.txt`、`awlt-decimal-columns.txt`）。
4. 一张表有几个起始页指针时，只读第一条链。`PageIter` 在第一条链读完时返回 `None`，`rows()` / `try_rows()` 用的 `flat_map` 到这里就停（临时测试：给两个指针，连着取六次得到 `Some(2), None, Some(5), None, None, None`）。A01 的库里 156 张表有 89 张不止一个起始指针，每个行集一个，索引的行集也算在内。只读第一条大概正是读出来的是数据行而不是索引行的原因，但这靠的是数据的那个行集排在最前；分区表的后几个分区会读不到。这两句是照代码推的，没有造这样的库来试。
5. vendored 件自带的例子 `print_all` 在这台机器上把输出重定向到文件时会崩（退出码 `0xC0000005`，打完第一张表的表头就崩），所以逐行核对没有用它。

### T3.3 暂存只开一次（2026-11-16，`67529b7`）

**先钉住结果。** 这一条改的是做法，结果不该变，没有哪条测试能在改之前红。所以先把结果钉住，再改：

- 探针加了一段暂存清单。装载返回的 SQLite 连接里，`PUBLISH_TABLES` 的每张表打一行：几列、几行，再加一个 SHA-256——列数、各列的名字，然后按 `rowid` 的顺序把每一行的每一个值喂进去（每个值前面带「有没有」的标记和长度）；最后一行是整张清单的摘要。改之前先跑一遍留底（`runs/t33-probe-mdf-before.log`、`runs/t33-probe-mtf-before.log`）。
- `tests/publish_mdf_load.rs` 加了 3 条测试，守整包那条路。这条路以前没有自己的测试：`TEST02_p` 整包的用例走得到它，但没有一条拿它和 `.mdf` 那条路直接比。
  - `an_mtf_dump_stages_what_its_mdf_stages`：从 `Export.dmp` 暂存出的 24 张表，逐表的列名和每一行，与从 `Export.mdf` 暂存出的相同。
  - `the_image_inside_the_dump_is_the_extracted_mdf`：`mdf_bytes_from_mtf_dump` 取出的镜像与 `extracted/Export.mdf` 逐字节相同。
  - `staging_a_dump_leaves_no_image_in_the_temp_directory`：暂存完，临时目录里没有这个进程留下的镜像。

三条在没改的装载上都是绿的（`runs/t33-tests-old-loader.log`，8 条全过，2.54 s）。再把装载故意改坏，看它们红不红。下面这张表是检查点 3 在提交了的装载（`67529b7`）上做的：改一处跑一次，跑完用 `git checkout` 还原。

| 故意改坏 | `an_mtf_dump_stages…` | `the_image_inside…` | `staging_a_dump_leaves…` | 输出 |
|---|---|---|---|---|
| 暂存完从 `T_Representation` 删掉一行；`mdf_bytes_from_mtf_dump` 返回的镜像翻一位 | 红，在逐表比较那一条（第 237 行）：`T_Representation` | 红，在内容那一条（第 256 行）：长度相同，内容不同 | 绿 | `runs/cp3-mutant-rows-and-bytes.log` |
| 临时镜像不删 | 绿 | 绿 | 红（第 285 行），点出留下的两个镜像 | `runs/cp3-mutant-image-left-behind.log` |
| 镜像少取 300 页 | 红，停在暂存这一步（第 231 行，`IO Error: failed to fill whole buffer`），没有走到比行 | 红，在长度那一条（第 254 行）：17 465 344，该是 19 922 944 | 红，同样停在暂存（第 275 行） | `runs/cp3-mutant-image-cut-short.log` |

三次的 8 条用例各用 0.37 s、0.34 s、0.25 s，是改后装载的速度（不改坏时 0.34–0.36 s）。第三次暂存失败以后，临时目录里没有留下镜像。第二次留下的两个镜像（各 19 922 944 字节）按名字删了。还原后 `mdf_load.rs` 的 SHA-256 与提交的相同，8 条全过（`runs/cp3-tests-restored-2.log`）。三条用例里没有见过红的断言还有两条：第 240 行的「至少比了一行」，第 287 行的「暂存出的库不再需要镜像」。

提交之前也改坏过两次，这一节原先的表列的是那两次：`runs/t33-mutant-image-left-behind.log`（7 过 1 红）和 `runs/t33-mutant-image-cut-short.log`（5 过 3 红），红的用例与上表后两行相同。原先说那是在「改后的装载」上做的，不对。那两次 8 条用例用了 2.55 s 和 0.82 s，是没改的装载的速度；紧接着的 `runs/t33-tests-new-loader.log` 失败得和第二次一模一样，`Finished` 只用了 0.94 s（重编这个库要 6–7 s），同一次里的 `publish_a01_raw_residual` 用了 0.78 s（改后是 0.05–0.06 s），可见那时编进去的还是没改的装载；再下一份 `runs/t33-tests-new-loader-2.log` 才是改后的（重编 12.04 s，8 条全过，0.35 s）。所以那两次改坏的是没改的装载：说明用例会红，不说明改后的装载被它们守着。上表是补做的。

**改。** `src/publish/mdf_load.rs`：

- `open_mdf_as_sqlite` 开一次 `MdfDatabase`，24 张表共用；`stage_table` 改成接 `&mut MdfDatabase`，不再自己开库。
- 每张表的行在一个事务里插入（`conn.transaction()`，插完 `commit()`）。`CREATE TABLE` 和原来一样先执行，在事务外面。
- 整包那条路：新的私有函数 `mdf_image_in_mtf_dump` 返回镜像在 dump 里的那段切片，`open_mtf_dump_as_sqlite` 把这段切片直接写进临时文件。公有的 `mdf_bytes_from_mtf_dump` 签名和结果不变，现在是这段切片的一份拷贝，`open_mtf_dump_as_sqlite` 不再调它。

探针跟着改：用一个读取器把 24 张表读完（原来每张表开一个），量的还是装载做的那件事。输出从「每张表开库读了多少、读行读了多少」变成「开库一次读了多少，每张表读行又读了多少」。

**绿。**

- `tests/publish_mdf_load.rs` 8 过（原有 5，新增 3），`tests/publish_a01_raw_residual.rs` 4 过 2 忽略，原有的断言没动（`runs/t33-tests-final.log`）。
- `cargo clippy --locked --workspace --all-targets -- -D warnings` 退出 0；`cargo fmt --all -- --check` 退出 0。
- 暂存清单：`.mdf` 改前与改后相同，dump 改前与改后相同，`.mdf` 与 dump 相同。26 行逐行相同（表头、24 张表、合计），合计 243 行，整张清单的摘要 `179906c7…103504fd`（`runs/t33-probe-mdf-after.log`、`runs/t33-probe-mtf-after.log`）。计划要的「暂存出来的表逐行与改前相同」是靠这个摘要和上面第一条测试两处说的，没有把行一行行打出来比。
- 对照矩阵 `t33` 对 `cp2`、对 `t32`：都是 717 / 717 个文件逐字节相同。矩阵是提交之前在工作区上跑的，工作区当时的内容就是 `67529b7`；检查点 3 在提交上又跑了一遍。

**读数。** 发布构建，stable 1.99.1：探针 2 324 480 字节，SHA-256 `f2ece8aa…9ccb67d3`；`pid_publish_xml.exe` 4 728 832 字节，SHA-256 `7b80540e…596e4f1d`。量法和 T3.1 一样，峰值工作集各跑 5 次。输出：上面两份 `-after.log`、`measure/t33-*/summary.txt`。

| | 改前（T3.1） | T3.2 之后 | T3.3 之后 |
|---|---|---|---|
| 开库读入 | 17 580 032 字节（2 146 页），24 次 | 999 424 字节（122 页），24 次 | 999 424 字节（122 页），1 次 |
| 24 张表读行读入 | 14 032 896 字节（1 713 页） | 221 184 字节（27 页） | 147 456 字节（18 页） |
| 合计，比文件大小 | 435 953 664 字节，21.88 倍 | 24 207 360 字节，1.22 倍 | 1 146 880 字节，**0.06 倍** |
| 读页次数，其中重复的 | 没数 | 2 955 次，0 次 | 140 次，0 次 |
| 探针只读不存的那一遍 | 407.8 ms | 62.8 ms | 4.1 ms |
| `open_mdf_as_sqlite` | 459.6 ms | 69.9 ms | 6.4 ms |
| `open_mtf_dump_as_sqlite` | 490.2 ms | 99.5 ms | 26.2 ms |
| 峰值工作集，只跑暂存，`.mdf` | 29 642 752–29 868 032 字节，1.49–1.50 倍 | 9 658 368–9 973 760 字节，0.48–0.50 倍 | 8 634 368–8 736 768 字节，0.43–0.44 倍 |
| 峰值工作集，只跑暂存，整包 | 69 496 832–69 918 720 字节，3.47–3.49 倍 | 49 762 304–50 003 968 字节，2.49–2.50 倍 | 28 602 368–28 717 056 字节，**1.43 倍** |
| 峰值工作集，命令行，A01 | 29 802 496–29 949 952 字节，1.50 倍 | 9 728 000–10 264 576 字节，0.49–0.52 倍 | 8 810 496–8 896 512 字节，0.44–0.45 倍 |
| 峰值工作集，命令行，整包 | 70 283 264–70 594 560 字节，3.51–3.53 倍 | 50 495 488–50 737 152 字节，2.52–2.54 倍 | 29 483 008–29 618 176 字节，**1.47–1.48 倍** |
| 命令行从起到退，A01 | 460–533 ms | 85–98 ms | 20–25 ms |
| 命令行从起到退，整包 | 520–548 ms | 127–133 ms | 58–67 ms |

「T3.2 之后」那一列的 62.8 ms 是这次改之前用带清单的探针量的（`runs/t33-probe-mdf-before.log`），其余是 T3.2 记的。同一份没改的装载这次也又量了一遍：`open_mdf_as_sqlite` 73.7 ms，`open_mtf_dump_as_sqlite` 89.2 ms，T3.2 记的是 69.9 和 99.5。同样的代码，单次计时两次之间能差一成，上表里的毫秒数照这个精度看。

Q4 的两条线（读入 3 倍、峰值 2 倍）现在都在线以下。

读入是文件的 0.06 倍，比计划写的「约 1 倍」低得多。计划是照「把文件从头读一遍」估的；T3.2 以后读取器只读要的页，开库 122 页，读行 18 页：11 张表各 1 页，`codelists`、`attributes` 各 3 页，还有 1 页是第 0 页——空表读出的那一行假数据（T3.2 顺带记下的第 2 条），10 张空表里第一张读了它，后面 9 张用的是缓存里的。T3.2 之后是 27 页，多出的 9 页就是那 9 张空表各自再读的一次第 0 页。

整包那条路的峰值比 `.mdf` 那条路多 19 968 000–19 980 288 字节（只跑暂存的两组，最小对最小、最大对最大），`Export.dmp` 是 20 012 544 字节：多出来的就是整个读进内存的那一份 dump，拷出来的那份镜像不在了。T3.1 的记录估这条路「预计还剩 1 倍多一点」，估低了，实际是 1.43–1.48 倍。dump 一份是 1 倍；其余的，只跑暂存时是 8.6–8.7 MB，和 `.mdf` 那条路的峰值一样多，命令行整包是 9.5–9.6 MB（整包在暂存之外还要读 zip 的目录、逐图发布）。什么都不做的进程就有 4.2–4.7 MB（T3.1 量的），这几个数里都含着它。

整包暂存比 `.mdf` 暂存多 20 ms（26.2 对 6.4）。照代码，两个函数差的只有三步：在 dump 里找到镜像，把 19.9 MB 写成临时文件，用完删掉。没有分开计时。

**超出计划原文的。**

1. 计划列的文件是装载和探针，另外动了 `tests/publish_mdf_load.rs`：上面那 3 条测试。
2. 探针多了暂存清单，那一遍读的输出格式也变了。清单用的 `sha2` 是 `backup-mdf` 特性本来就带的依赖，`Cargo.toml` 没有动。
3. 新的私有函数 `mdf_image_in_mtf_dump`；`open_mdf_as_sqlite`、`open_mtf_dump_as_sqlite` 的文档注释各改了一句，说的是现在的做法。

**没有验的。**

- 事务这一项单独省了多少，分不出来：只开一次库和事务是一起改的，量到的是合起来的数，这个库的发布表一共才 243 行。
- 「峰值内存不再跟着文件大小走」只有一个大小的库可量。`.mdf` 这条路照做法是这样：读取器最多留 256 页（2 MiB），与文件多大无关；内存里的 SQLite 存着发布表的全部行，跟着的是发布表的数据量。整包这条路还跟着 dump 的大小走，见顺带记下的第 1 条。
- 发布表上千行、一张表跨很多页的库上会怎样，没有样本。
- 一张表读到一半出错时，照代码事务随 `Transaction` 一起回滚，错误和原来一样往上返回，整个暂存失败。没有专门的测试。
- 用时只在这一台机器上量过，文件当时都在系统缓存里。

**顺带记下的。**

1. **整包这条路，dump 还是整个读进内存。** `publish_mtf_package`（`backup_package.rs:1158`）用 `fs::read` 读整个 `Export.dmp`，只为了交给 `open_mtf_dump_as_sqlite`；这份字节一直留到整包发布完才放。计划写的做法（从切片直接写临时文件）去不掉它，剩下的 1.43–1.48 倍里有 1 倍是它。再往下有两种做法，都不在这一轮计划里，没有做：
   - 暂存完就放掉这份字节。改一处，峰值不变（峰值就在暂存那一刻），后面逐图发布时少占 20 MB。
   - 不整个读：从 dump 文件里按偏移直接读镜像，把一个带偏移的 `Read + Seek` 交给 `MdfDatabase::from_read`。峰值降到和 `.mdf` 那条路差不多，临时文件也不用写了，上面那 20 ms 跟着没有。要动的是找镜像的那一段（`backup/mtf.rs:359` 的 `msda_stream_body` 现在接的是整份字节）和 `publish_mtf_package`，源码里调 `open_mtf_dump_as_sqlite` 的只有它这一处；`open_mtf_dump_as_sqlite(&[u8])` 是公有函数，得留着，另加一个接路径的。
2. 暂存清单里，10 张空表各有 1 行，就是 T3.2 顺带记下的第 2 条说的那一行假数据。清单把它也算进了摘要，所以「改前改后相同」这句话连这 10 行也包括在内。

### 检查点 3（2026-11-16）

**门禁。** `gates.ps1 -Label cp3`，在 `67529b7` 上跑。开跑时工作区只有这份计划文档有未提交的改动（T3.3 的记录）；跑的中间又改了 `CHANGELOG.md`、`AGENTS.md`，源码没有动。这三份文件没有哪段代码去读（`git grep` 过 `src`、`tests`、`examples`、`benches`、`build.rs`、`pid-plot`、`.github/scripts`：提到它们的 49 处都在注释里）。日志在 `target/publish-audit/review/gates/cp3/`：

| 门禁 | 退出码 | 用时 | 读数 |
|---|---|---|---|
| `cargo build --locked --workspace --all-targets` | 0 | 87 s | |
| `cargo test --locked --workspace --all-targets` | 0 | 1 583 s | 475 个测试程序；2 555 通过，0 失败，3 忽略；`skipping: ` 提示 21 行 |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | 0 | 1 s | 0 条警告 |
| `cargo fmt --all -- --check` | 0 | 4 s | |
| `bash .github/scripts/check-missing-docs.sh` | 0 | 6 s | `current=0, baseline=0` |
| `cargo test --locked -p pid-parse --no-default-features --lib` | 0 | 1 s | 985 通过 |
| `cargo check --lib --locked -p pid-parse --no-default-features --target wasm32-unknown-unknown` | 0 | 0 s | |
| `bash .github/scripts/check-byte-audit-baselines.sh` | 0 | 15 s | 3 份基线都比了，各 `Regressions: 0`，跳过 0 |

与检查点 2 比，通过的测试多 3 条（2 552 → 2 555），是 T3.3 在 `tests/publish_mdf_load.rs` 加的 3 条；测试程序多 1 个（474 → 475），是 T3.1 的探针 `examples/probe_what_staging_an_mdf_costs.rs`，里面没有用例。忽略数、`skipping: ` 行数、缺文档数、不带默认特性的 985 条都没变。第八道在 `summary.txt` 里的那一行这次正常出来了（检查点 2 那次没有）。

**vendored 件自己的测试。** `vendor/oxidized-mdf` 不在工作区里，上面的门禁不跑它，另跑（`runs/cp3-vendored-all.log`，`cargo test --no-fail-fast`）：单元测试 46 条全过，其中 8 条是 T3.2 加的；文档测试 4 条全过；集成测试 20 条里 18 条过、2 条失败（`rows::case_5`、`number_of_rows::case_3`），就是 T3.2 的记录里改之前已经失败的那两条。计划给这一项写的命令是 `cargo test --lib`，即单元测试那一组，它是全过的。

**对照矩阵。** `67529b7` 的发布构建（`target/publish-audit/bin/pid_publish_xml-67529b7-release.exe`，4 728 832 字节，SHA-256 `7b80540e…596e4f1d`，和 T3.3 量读数的是同一份：源码没有再变，cargo 没有重编）跑了一遍矩阵（`target/publish-audit/review/cp3/`，61 项，173 s，`runs/cp3-matrix.log`）。`compare.ps1 -A cp2 -B cp3`：717 个文件全部相同。阶段 3 的三个任务都没有改写出的东西。

**读数。** 全部读数在 T3.3 一节的表里，这里只列几行（发布构建；`Export.mdf` 19 922 944 字节，`Export.dmp` 20 012 544 字节）：

| | 改前（T3.1） | T3.2 之后 | T3.3 之后 |
|---|---|---|---|
| 暂存从镜像读入 | 435 953 664 字节，21.88 倍 | 24 207 360，1.22 倍 | 1 146 880，0.06 倍 |
| `open_mdf_as_sqlite` | 459.6 ms | 69.9 ms | 6.4 ms |
| `open_mtf_dump_as_sqlite` | 490.2 ms | 99.5 ms | 26.2 ms |
| 命令行发布 A01，峰值工作集比 `Export.mdf` | 1.50 倍 | 0.49–0.52 倍 | 0.44–0.45 倍 |
| 命令行发布 `TEST02_p` 整包，峰值工作集比 `Export.dmp` | 3.51–3.53 倍 | 2.52–2.54 倍 | 1.47–1.48 倍 |

Q4 的两条线（读入 ≥ 3 倍，峰值 ≥ 2 倍）现在都在线下。整包剩下的 1.47–1.48 倍里有 1 倍是整个读进内存的 `Export.dmp`（T3.3 顺带记下的第 1 条）。只有这一个大小的库可量。

**截断的文件。** T3.1 的三份截断样本在改后的代码上重跑了一遍（`cp3-checks/`），行为与 T3.1 记下的相同：

- 前 1 000 页、前 2 000 页：探针退出 0，只认出 `T_InlineComp`、`T_SignalRun` 两张表，各 1 行（那一行假数据）；命令行 `--list-drawings` 退出 1，`error: prepare T_Drawing query: no such table: T_Drawing`。开库读了 54 页（442 368 字节），改前是把截断后的文件整个读进来。
- 前 2 146 页：探针和命令行都退出 1，`IO Error: failed to fill whole buffer`。

**故意改坏。** T3.3 提交前做的两次，改坏的是没改的装载，那一节原先写错了。这次在提交了的装载上补做了三次，三条新用例该红的时候都红了。经过和表在 T3.3 一节。

**decimal 的 panic。** 位置实际看到了（T3.2 顺带记下的第 3 条已经更新）：`parse_decimal_opt`（`pages.rs:292`），精度 38、小数 6 位、值超出 96 位。A01 的库里有的 23 张发布表没有 `decimal` / `numeric` 列，这个库的发布走不到它；别的库版本没有看过。

**说明。** `CHANGELOG.md` 的 `[Unreleased]` 加了阶段 3 一条：读数、公有接口变了什么、结果没变的依据、读取器原有而没改的五处。`AGENTS.md` 改了五处：页读取器那一段照现在的做法重写，新加「Known limits」五条，用例表改了两行，vendored 单测的条数 37 → 46，「Panic-free」那一条补了 decimal 这个例外。提交前把两处的数字和名字对着上面的记录过了一遍；`SalesOrderDetail` 是 panic 的那张表这一句，对的是 `cp3-checks/awlt-decimal-columns.txt`。

**顺带记下的。**

1. 读取器原有的五处（空表读出一行假数据、decimal 的 panic、只读第一条链、截断的系统表不报错、vendored 件的集成测试 2 条不过）这一阶段都没有动，改前改后一样。要不要改、先改哪一条，等你定。第一条改起来小（`0:0` 当作没有页），但会改变暂存出来的内容，`strict_rows_load_a01_equip_component_table` 要跟着改。
2. `AGENTS.md` 里「297 of the 305 tracked root examples」这个数在 `a4b490e` 上就已经不对（当时是 301 / 309，加了 T3.1 的探针后是 302 / 310），留给 T6.4 一起对。
3. 这台机器的构建不出 `.pdb`，panic 的回溯里全是 `<unknown>`。decimal 那一处是在读取器的一份临时拷贝里加打印看出来的，拷贝在 `target/publish-audit/review/t32-old-reader/`，不在仓库里。
4. 换进一个源文件之后要确认 cargo 真的重编了：`Copy-Item` 保留原文件的修改时间，cargo 当它没变。T3.3 提交前那一次「新装载」的运行（`runs/t33-tests-new-loader.log`）跑在了旧的库上，应当就是这个原因。这一轮的故意改坏都用编辑和 `git checkout`，每次的日志里都有重编的那一行。

### T4.1 同一对象只发一次（2026-11-16，`b3d3fd1`）

**红。** 先把用例 `a_component_placed_twice_is_published_once` 从 `9b4d60b` 搬过来（`src/publish/oracle_exp.rs:4633`，紧跟在 `dump_only_piping_components_come_from_this_drawings_representations` 后面）。一份造出来的 dump：`T_REPRESENTATION` 里两行 13 型的表示 `REPA`、`REPB`，都在图 `DRAW1` 上，指的是同一个 `PipingComp` `COMP1`；图本身是空的。没改时（`runs/t41-red.log`）：

```text
thread 'publish::oracle_exp::tests::a_component_placed_twice_is_published_once' panicked at src\publish\oracle_exp.rs:4685:9:
assertion `left == right` failed
  left: 2
 right: 1
```

加进去了两个对象，该是一个。

**改。** `attach_dump_only_components`（`oracle_exp.rs:1387`）：`known` 改成可变的，`!known.contains(&item.uid)` 换成 `known.insert(item.uid.clone())`（`:1404`），加进去一个就记一个。`item.uid` 是 `dump_only_piping_components` 给的，已经是 `norm_uid` 规整过的写法，和 `known` 里的一致。文档注释补了一句。就是 `9b4d60b` 里的那一块。

**绿。** `cargo test --locked --lib`：1 564 条全过（`runs/t41-green.log`），比改前多的一条就是它。`cargo fmt --all -- --check` 和 `cargo clippy --locked --workspace --all-targets -- -D warnings` 退出 0，0 条警告（`runs/t41-fmt-clippy.log`）。

**对照矩阵。** `b3d3fd1` 的发布构建（`pid_publish_xml-b3d3fd1-release.exe`，4 728 320 字节，SHA-256 `acbef23e…8580cef7`），61 项，163 s；`compare.ps1 -A cp3 -B t41`：717 个文件全部相同（`runs/t41-matrix.log`）。

**与 `9b4d60b` 的出入。** 用例里 `PidPublishDrawing` 多填一个字段 `drawing_attributes`（`main` 后来加的）。结尾两条断言从「对象 1 个、表示 2 个」改成点名：对象是 `["COMP1"]`，表示是 `["REPA", "REPB"]`。用例前面加了三行说明。

**没有验的。** 矩阵不变，说的是矩阵里没有哪张图把一个只在 dump 里的管件放了两次；这一处只在造出来的 dump 上见过红和绿。改之前这样的图写出来是什么样（审核 §4.2 说是两个同 UID 的对象），没有拿真实的包看过。

### T4.2 整包没有参考时也查重复 UID（2026-11-16，`b77384f`）

**红。** 先加用例 `a_staged_drawing_that_states_a_uid_twice_is_written_and_named`（`src/publish/backup_package.rs:2005`），当时只对 `_Index.json` 和写出的文件下断言，所以在没改的代码上能编。夹具是 `tests/fixtures/publish-shortfalls.sql` 去掉换热器和那条关系（写全的那一版），再给管嘴 `N-1` 加第二条表示，`SP_ID` 取成容器的 `V-1`。数据库这条线于是把 `V-1` 写在两个块上：容器的块，和这条表示的 `PIDRepresentation`。真实的库里 `SP_ID` 在各张表之间不重复，写不出这样的文档，所以是人为造的。没改时（`runs/t42-red.log`）前面的断言都过：文档里 `<IObject UID="V-1"` 出现 2 次，`failed` 是 0，三个文件都在，`written` 是 1。失败在 `problems` 上：

```text
thread 'publish::backup_package::tests::a_staged_drawing_that_states_a_uid_twice_is_written_and_named' panicked at src\publish\backup_package.rs:1911:9:
assertion `left == right` failed: {"package":…
  left: Array []
 right: Array [String("SHORT-01: 1 UID written more than once: V-1 ×2")]
```

图写出来了，带着两个同 UID 的块，索引里一个字没说。

**改。**

- `src/publish/diff.rs`：加 `PidPublishCensus`（`:1548`：`objects`、`values`、`rels`、`duplicate_uids`，外加 `counts()`）和 `pid_publish_census`（`:1582`），一遍解析把三个计数和重复的 UID 一起带出来。`duplicate_uids` 就是读文档时本来记着的那张表：对象和 `<Rel>` 的 UID 一起算，后面的块每重述一次记一条，按文档顺序，所以写了三次的 UID 记两条。`pid_publish_counts`（`:1541`）签名不变，现在取的是它的三个计数；原有的断言一条没动。两个新名字在 `src/publish/mod.rs` 里和 `pid_publish_counts` 一起导出。
- `src/publish/backup_package.rs`：加 `PackageDuplicateUids`（`:175`：`drawing_uid`、`drawing_name`、`uids`，每个 UID 带上写它的块数），`BackupPublishReport` 加字段 `duplicate_uids`（`:113`）。两条分支都改用 `pid_publish_census`（`.pid`／Oracle 分支 `:796`，数据库分支 `:1427`），三件套写成之后点名（`:828`、`:1493`）：`_Index.json` 顶层 `problems` 一句 `<图名>: 1 UID written more than once: <UID> ×2`，这张图自己的 `problems` 里是不带图名的同一句，报告里一条。图照写，`written` 照加，`failed` 不动，对象数里两个块都算（`pid_publish_counts` 原来就这么算）。带不带 `--diff-against` 都查；带了、这张图又有可比的参考时，门禁照旧把每个重复的 UID 记成一条 finding。
- `src/bin/pid_publish_xml.rs`：`package_warnings`（`:734`）把这一句接在链故障和「没写全」之后，stderr 上是一行 `warning: <图名>: …`。

**绿。** `runs/t42-precommit.log`（退出 0，52 s）：`cargo fmt --all -- --check` 和 `cargo clippy --locked --workspace --all-targets -- -D warnings` 过，0 条警告；`cargo test --locked --lib` 1 567 条全过（比 T4.1 多 3 条）；`--bin pid_publish_xml` 5 条全过（多 1 条）；`--test publish_chain_faults` 7 条全过。文档门禁 `gates/t42-docs`：`current=0, baseline=0`。`--no-default-features` 的库内测试 `gates/t42-nodef`：985 条全过，数目没变（那个构建里没有 `publish` 模块）。

新加的用例：

| 用例 | 位置 | 看的是什么 |
|---|---|---|
| `the_uids_a_document_states_twice_come_out_with_its_counts` | `diff.rs:2761` | 一个对象块写了两次、一条 `<Rel>` 又占了这个对象的 UID：计数 `(3, 1, 1)`，重复的 UID 是两条 `A`；每个 UID 只出现一次的文档，这张表是空的 |
| `a_staged_drawing_that_states_a_uid_twice_is_written_and_named` | `backup_package.rs:2005` | 上面那张图：照写、照算写出，索引顶层和这张图自己的 `problems` 各一句，报告里一条（`V-1`，2 个块） |
| `the_uids_stated_again_are_counted_by_block_and_said_under_the_drawings_name` | `backup_package.rs:2043` | 重述表 `B, A, B` 归成 `B ×3, A ×2`；句子的单复数和带图名的写法；空表不出条目 |
| `a_package_drawing_that_states_a_uid_twice_is_warned_of_and_the_exit_code_stays` | `pid_publish_xml.rs:1356` | 警告行的写法和次序；带不带 `--strict` 退出码都是 0，`--strict` 的结论行不出 |

另有两处原有用例各加一条断言：写全的那张图 `duplicate_uids` 是空的（`backup_package.rs:1978`）；`tests/publish_chain_faults.rs` 里那两个 `.pid` 包（好的和坏的）`duplicate_uids` 都是空的（`:937`）。

**对照矩阵。** `b77384f` 的发布构建（`pid_publish_xml-b77384f-release.exe`，4 725 248 字节，SHA-256 `10b724c1…7b90aff0`），61 项，138 s；`compare.ps1 -A t41 -B t42`：717 个文件全部相同（`runs/t42-matrix.log`）。

**故意改坏。** 在 `b77384f` 上改一处、跑、`git checkout` 还原，每份日志里都有重编的那一行。

| 改坏的地方 | 日志 | 红在哪 |
|---|---|---|
| `pid_writer.rs` 的 `write_document` 把 `PIDDrawing` 块写两遍 | `runs/t42-mutant-pid.log` | `tests/publish_chain_faults.rs:800`「a sound package」：两张图各一行 `warning: <图名>: 1 UID written more than once: <图 UID> ×2`，后面跟着 `Published 2 drawings … (0 failed, …)` |
| 数据库分支去掉 `own_problems.push(duplicates.sentence())` | `runs/t42-mutant-db-own.log` | `backup_package.rs:2031`：这张图自己的 `problems` 是空的 |
| 数据库分支去掉 `duplicate_uids.push(duplicates)` | `runs/t42-mutant-db-report.log` | `backup_package.rs:2038`：报告的 `duplicate_uids` 是空的 |
| `package_warnings` 去掉 `.chain(duplicate_uids)` | `runs/t42-mutant-bin.log` | `pid_publish_xml.rs:1363`：一行警告都没有 |
| `pid_publish_census` 的 `duplicate_uids` 换成空表 | `runs/t42-mutant-diff.log` | `diff.rs:2770`（`duplicate_uids: []`，该是两条 `A`）和 `backup_package.rs:2026`（顶层 `problems` 是空的） |

行号是提交里的；改坏时删了一行的那几次，日志里的行号小 1。数据库分支往顶层 `problems` 里放的那一句没有单独改坏：红的那一次停的就是这条断言。

第一处改坏时，还用那份改坏的 debug 程序手工跑了仓库里的 `test-file/backup-test/DWG-0202GP06-01_p`（7 张图，走 `.pid`／Oracle 分支），和矩阵里没改坏的 `pkg-dwg` 比：

| | 没改坏（矩阵 `t42` 的 `pkg-dwg`） | 改坏 |
|---|---|---|
| 退出码 | 0 | 0 |
| 写出的文件 | 22（7 张图的三件套加 `_Index.json`） | 22 |
| stderr 的 `warning:` 行 | 0 | 7，每张图一行 `warning: <图名>: 1 UID written more than once: <图 UID> ×2` |
| `_Index.json` 顶层 `problems` | 0 句 | 7 句，和警告行同文 |
| 每张图自己的 `problems` | 0 句 | 各 1 句，不带图名 |
| `failed` | 0 | 0 |
| 每张图的 `objects` | 1、3、3、492、170、113、5 | 各多 1（图纸块算了两次）；`values`、`relationships` 不变 |

改坏那次的 stderr 和索引留在 `runs/t42-mutant-pid-pkg-stderr.txt`、`runs/t42-mutant-pid-pkg-index.json`。五处都还原之后重跑 `cargo test --locked --lib`、`--bin pid_publish_xml`、`--test publish_chain_faults`：1 567、5、7 条全过（`runs/t42-restored.log`）。

**超出计划原文的。**

1. 计划列的文件是 `diff.rs` 和 `backup_package.rs`。stderr 那一行是命令行程序打的，所以还动了 `src/bin/pid_publish_xml.rs`；另外 `src/publish/mod.rs` 多导出两个名字，`tests/publish_chain_faults.rs` 加了一条断言。
2. 计划说让 `pid_publish_counts` 把重复的 UID 带出来。没有改它的返回值（它是公开函数，现有断言按三元组写的），而是在旁边加了 `pid_publish_census`。一开始写的是返回「三元组加一张表」的函数，clippy 的 `type_complexity` 不让过（`runs/t42-fmt-clippy.log`），才改成带字段说明的结构。
3. 计划说 `_Index.json` 的 `problems` 一句。这张图自己那一条的 `problems` 里也放了一句（不带图名），和 T2.3「没写全」的做法一致。
4. 句子按图名点名，两条分支一样。`.pid`／Oracle 分支别的 `problems` 是按「zip 文件名 条目名」点名的；这一句说的是写出来的 `<图名>_Data.xml`，所以用图名。
5. 句子不截断，重复的 UID 有多少列多少。

**没有改的。** 退出码和 `--strict`。计划写的是「写出但点名」，`--strict` 现在的定义只管数据库这条线「没写全」的图。所以不带 `--diff-against` 时，一个有重复 UID 的包退出码仍是 0，只有警告行和索引里的那一句。用例 `a_package_drawing_that_states_a_uid_twice_is_warned_of_and_the_exit_code_stays` 把这一点写死了，要改就改它。单图模式（不是整包）没有参考时仍然不查，计划没有要求。

**没有验的。**

- 仓库里没有一条提交了的用例让 `.pid`／Oracle 分支真的遇到重复的 UID：拿真实的 `.pid` 写不出重复，也没有为了测试去改生产代码的接口。这条分支会点名，只在上面改坏写出器的那一次见过（集成用例变红，加手工跑的那个包）。提交了的用例对这条分支只守住「没有重复时不说话」，以后有人把这条分支里点名的那几行删掉，没有用例会红。
- 真实的包上没有见过这一句：矩阵里 7 个整包项的输出没有变，说明这些包里没有哪张图把一个 UID 写两次。

### T4.3 原子写（2026-11-16，`9d24529`、`ecfc5c4`、`b69ce84`）

**红。** 先加两条验收用例，两条分支各一条，都是把一张图第三个文件的名字预先建成目录：

- 数据库分支：`a_staged_drawing_one_of_whose_files_cannot_be_written_leaves_none_of_them`（`src/publish/backup_package.rs:2204`）。夹具是 `tests/fixtures/publish-shortfalls.sql` 再加一张空图 `OTHER-01`，输出目录里先建一个叫 `SHORT-01_Coverage.json` 的目录。
- `.pid`／Oracle 分支：`a_package_drawing_one_of_whose_files_cannot_be_written_leaves_none_of_them`（`tests/publish_from_pid.rs:1862`）。用命令行发仓库里的 `test-file/backup-test/DWG-0202GP06-01_p`（7 张图），目录建在 `DWG-0201GP06-01_Coverage.json` 这个名字上。

没改时两条都红在输出目录的清单上（`runs/t43-red-db.log`、`runs/t43-red-pid.log`）。数据库分支的：

```text
thread 'publish::backup_package::tests::a_staged_drawing_one_of_whose_files_cannot_be_written_leaves_none_of_them' panicked at src\publish\backup_package.rs:2121:9:
assertion `left == right` failed: {"package":…
  left: ["OTHER-01_Coverage.json", "OTHER-01_Data.xml", "OTHER-01_Meta.xml", "SHORT-01_Coverage.json", "SHORT-01_Data.xml", "SHORT-01_Meta.xml", "_Index.json"]
 right: ["OTHER-01_Coverage.json", "OTHER-01_Data.xml", "OTHER-01_Meta.xml", "SHORT-01_Coverage.json", "_Index.json"]
```

命令行那一条（当时的行号是 `tests\publish_from_pid.rs:1913`）是 22 个名字，该是 20 个，多出来的是 `DWG-0201GP06-01_Data.xml` 和 `DWG-0201GP06-01_Meta.xml`。两份日志里都看得到旧代码已经把这张图记成了失败：索引里是 `"failed":1`，stderr 是 `Published 6 drawings … (1 failed, 1 without a DrawingNo)`。记成失败的图，前两个文件却留在目录里，按文件名取文档的人会把它当成写成了的。

**改。**

- `src/publish/backup_package.rs`：
  - 加 `StagedFile`（`:1155`）。`stage`（`:1166`）把内容整份写到目标旁边的临时名 `<文件名>.<进程号>.tmp`；`commit`（`:1187`）`rename` 到位；没到位就丢掉的，`Drop`（`:1194`）把临时文件删掉。临时文件建不出来时什么都不删：那个名字下如果有文件，不是这一次建的。
  - 加公开函数 `write_atomic`（`:1136`）：目标名空着，或者名字下是个普通文件，就走 `StagedFile`；是目录、设备（`NUL`、`/dev/null`）、链接，或者查不出是什么，照旧 `fs::write`。判断在 `takes_its_name_by_rename`（`:1145`）。
  - `write_three`（`:1088`）重写：三个文件先都写到临时名，再依次改名。哪一个改不成，已经到位的删掉，还没到位的临时文件随 `Drop` 删掉。错误是 `write <路径>: <原因>`；已到位的里有删不掉的，后面接 `; <路径> is left behind: <原因>`。两条分支的调用点（`:820`、`:1567`）和失败之后的记账都没动。原来给三件套用的私有函数 `write_file` 删了。
  - 两处 `_Index.json`（`:488`、`:1671`）改用 `write_atomic`。
  - 模块文档加一段，说的就是这几条。
- `src/bin/pid_publish_xml.rs`：`write_file`（`:1308`）改用 `write_atomic`（`:1314`）；命令行写文件都经过它（`:598`、`:965`、`:980`、`:1016`）。用法说明加两段（`:141`–`:151`）：`--out-dir` 不先清空，整包以 `_Index.json` 为准；每个文件先写到 `<name>.<process id>.tmp` 再改名，中断的运行不留写了一半的文档，可能留下 `.tmp` 文件（可以删）和三件套不齐的图。

**绿。** 三个提交之前各跑的：

- `9d24529`：`runs/t43-precommit.log`，库内 1 573 条、`--bin pid_publish_xml` 5、`cli_help_streams` 2、`publish_xml_cli` 36、`publish_chain_faults` 7、`publish_completeness` 10、`publish_diff_accepted_baseline` 19、`publish_unit_values` 1、`publish_from_pid` 20，全过。之后又加了 `only_a_free_name_or_a_plain_file_is_renamed_over`，只按 `backup_package` 过滤重跑（17 条全过），连同 fmt 和 clippy（`runs/t43-green-3.log`）。文档门禁 `gates/t43-docs`：`current=0, baseline=0`（后两个提交没有加公开项）。
- `ecfc5c4`：`runs/t43-tests-green.log`，`backup_package` 过滤 18 条、`publish_from_pid` 里相关的 3 条，全过，连同 fmt 和 clippy。
- `b69ce84`：`runs/t43-wording-green.log`，`--bin pid_publish_xml` 5、`cli_help_streams` 2、`publish_xml_cli` 36，全过，连同 clippy。

最后在 `b69ce84` 上全量跑了一遍（`runs/t43-restored.log`，退出 0，277 s）：库内 1 575 条（比 T4.2 多 8 条）、`--bin pid_publish_xml` 5、`cli_help_streams` 2、`publish_xml_cli` 36、`publish_chain_faults` 7、`publish_completeness` 10、`publish_diff_accepted_baseline` 19、`publish_unit_values` 1、`publish_from_pid` 22（多 3 条），全过；`cargo fmt --all -- --check` 过；`cargo clippy --locked --all-targets -- -D warnings` 0 条警告。这一次的 clippy 没带 `--workspace`，只查了根 crate；提交前的那三次带了。

新加的用例：

| 用例 | 位置 | 看的是什么 |
|---|---|---|
| `a_staged_drawing_one_of_whose_files_cannot_be_written_leaves_none_of_them` | `backup_package.rs:2204` | 数据库分支的验收：`SHORT-01` 一个文件不留，目录里只有 `OTHER-01` 的三件套、索引和那个挡路的目录（还是空的）；`written`、`failed` 是 1、1；顶层 `problems` 一句 `SHORT-01: write …SHORT-01_Coverage.json: …`，这张图自己的 `files` 是空的；`OTHER-01` 照写 |
| `a_staged_package_written_again_takes_each_name_as_a_new_file` | `backup_package.rs:2284` | 输出目录里先放上一轮的三件套和索引，各带一个硬链接；发完之后硬链接读到的还是原来的内容，四个名字下是新内容，目录里没有别的 |
| `a_staged_file_waits_under_a_name_no_document_is_looked_for_by` | `backup_package.rs:2367` | 四种后缀各一次：改名之前目录里只有 `A01<后缀>.<进程号>.tmp`，不以四种后缀的任何一种结尾，内容已经是整份；改名之后只有目标；没改名就丢掉的，什么都不留 |
| `write_atomic_gives_the_name_to_a_new_file` | `backup_package.rs:2403` | 新建、覆盖、再覆盖：每次目标都是整份新内容；覆盖前给旧文件加的硬链接读到的还是 `first`；旁边不留东西 |
| `write_atomic_fails_without_leaving_a_file_behind` | `backup_package.rs:2429` | 目标是目录、上级目录不存在：都报错，不留临时文件，那个目录还是空的 |
| `only_a_free_name_or_a_plain_file_is_renamed_over` | `backup_package.rs:2455` | 普通文件和空着的名字走改名；目录和空设备不走 |
| `write_atomic_writes_a_device_in_place` | `backup_package.rs:2476`（只在 Windows 上编） | `NUL` 和 `\\.\NUL` 都不走改名，写得进去 |
| `three_files_are_written_all_of_them_or_none` | `backup_package.rs:2485` | 三个都写成；第三个名字是目录：前两个删掉，错误点的是第三个，没有 `left behind`；第一个名字是目录：一个都没到位 |
| `a_package_drawing_one_of_whose_files_cannot_be_written_leaves_none_of_them` | `publish_from_pid.rs:1862` | `.pid`／Oracle 分支的验收：退出码 1，`Published 6 drawings … (1 failed, …)`；目录里是另外六张图的 18 个文件、索引和那个目录，共 20 个名字；顶层 `problems` 一句，按 zip 条目点名，说的是 `write …DWG-0201GP06-01_Coverage.json: …`；这张图的 `files` 是空的，其余六张各 3 个 |
| `a_document_written_over_an_earlier_one_takes_its_name_as_a_new_file` | `publish_from_pid.rs:1983` | 单图 `--out`、`--meta-out` 写到上一轮留下的两个文件上：硬链接读到的没变，两个名字下是新文档，目录里没有别的 |
| `a_package_written_over_an_earlier_one_takes_each_name_as_a_new_file` | `publish_from_pid.rs:2017` | 整包发进留有上一轮 `DWG-0202GP06-01` 三件套和索引的目录：四个硬链接读到的都没变；目录里 22 个名字，都以四种后缀之一结尾 |

前八条在库里（`9d24529` 七条，其中 `write_atomic_gives_the_name_to_a_new_file` 当时叫 `write_atomic_puts_the_whole_contents_in_the_place_of_a_file`，没有硬链接那几句；`ecfc5c4` 加第二条），后三条在 `tests/publish_from_pid.rs`（`9d24529` 一条，`ecfc5c4` 两条）。

**对照矩阵。** 跑了两遍。`9d24529` 的发布构建（`pid_publish_xml-9d24529-release.exe`，4 744 192 字节，SHA-256 `7687675c…07953874`），61 项，133 s；`compare.ps1 -A t42 -B t43`：717 个文件里 714 个相同、3 个不同（`runs/t43-matrix.log`）。`b69ce84` 改了用法说明的措辞，又跑一遍：`pid_publish_xml-b69ce84-release.exe`（4 744 704 字节，SHA-256 `8079b1b1…54d2418d`），61 项，135 s；`compare.ps1 -A t42 -B t43b`：还是 714 个相同、同样的 3 个不同（`runs/t43b-matrix.log`）。

变的 3 个是 `help\stdout.bin`（114 → 127 行）、`no-args\stderr.bin`（115 → 128 行）和汇总各项大小的 `summary.txt`（只有 `help`、`no-args` 两行的字节数），都是用法说明带来的：多出的 13 行就是新加的那两段，分别从第 28、29 行起，去掉之后和 `t42` 的逐行相同（`t43` 那一遍多的是 12 行，措辞是改之前的）。其余 714 个，各项发布出来的文件和它们的 stdout、stderr，逐字节没变。两遍都没有只在一边出现的文件：各项的输出目录里没有多出 `.tmp`。

**手工核对。** 矩阵里没有「目标写不进去」「目标不是普通文件」「往旧文件上写」这几种情况，拿发布构建另比了一遍。旧的是 `b77384f` 的（矩阵 `t42` 用的那个），新的是 `9d24529` 的；之后两个提交只动了用例、文档和用法说明的措辞，没有重做。各次的 stderr 留在 `target/publish-audit/review/hand-t43/`。

| 情况 | 旧 | 新 |
|---|---|---|
| 单图 `--out` 指向只读文件 | 退出 1，`error: write …\ro.xml: 拒绝访问。 (os error 5)`，文件内容没变 | 同一句，退出 1，内容没变，旁边没有 `.tmp` |
| 单图 `--out NUL` | 退出 0，`Wrote NUL (41975 bytes).`，当前目录里没有多出文件 | 一样 |
| 单图 `--out` 指向一个目录 | 退出 1，`error: write …\adir: 拒绝访问。 (os error 5)`，目录没动 | 一样，旁边没有 `.tmp` |
| `DWG-0202GP06-01_p` 整包往同一个目录连发三次（旧、新、新） | 退出 0，22 个文件 | 两次都退出 0，22 个文件，没有 `.tmp`；三次的 22 个文件哈希逐个相同，stderr 相同 |
| 全量包（矩阵 `pkg-full` 那个，53 张图）发到 6.9 s 时掐掉进程 | 没有做 | 目录里 42 个文件：14 张图各三个，没有缺件的；没有 `_Index.json`；没有 `.tmp`；42 个都和发完那一次的逐字节相同 |

**故意改坏。** 在 `b69ce84` 上，脚本 `target/publish-audit/review/mutants-t43.ps1` 一处一处来：改一行，跑 `cargo test --locked --lib backup_package` 和 `cargo test --locked --test publish_from_pid -- a_package_ a_document_written`，`git checkout` 还原。十份日志里都有重编的那两行和两行 `test result:`，没有编译错误；全部还原之后工作区只有计划文档有改动，再跑的全量就是上面的 `runs/t43-restored.log`。

| 改坏的地方 | 日志 | 红在哪 |
|---|---|---|
| `write_three` 里已到位的文件不删 | `runs/t43-mutant-keep.log` | `three_files_are_written_all_of_them_or_none`（`backup_package.rs:2509`）、数据库分支验收（`:2226`）、命令行验收（`publish_from_pid.rs:1902`） |
| 临时名改成前缀 `<进程号>.tmp.<文件名>`，于是以 `_Data.xml` 这些后缀结尾 | `runs/t43-mutant-name.log` | `a_staged_file_waits_under_a_name_no_document_is_looked_for_by`（`:2376`） |
| `Drop` 不删临时文件 | `runs/t43-mutant-drop.log` | 同一条用例的最后一句（`:2396`）、`three_files…`（`:2509`）、两条验收（`:2226`、`publish_from_pid.rs:1902`） |
| 名字下查得到东西的一律改名（目录、设备也改） | `runs/t43-mutant-route-ok.log` | `only_a_free_name_or_a_plain_file_is_renamed_over`（`:2464`，目录那一句） |
| 查询报错的也改名 | `runs/t43-mutant-route-err.log` | `only_a_free_name…`（`:2465`，空设备那一句）、`write_atomic_writes_a_device_in_place`（`:2479`） |
| `write_atomic` 对普通文件和空着的名字也 `fs::write` | `runs/t43-mutant-inplace.log` | `write_atomic_gives_the_name_to_a_new_file`（`:2418`）、`a_staged_package_written_again…`（`:2315`）、命令行的两条（`publish_from_pid.rs:2003`、`:2040`） |
| 命令行的 `write_file` 退回 `fs::write` | `runs/t43-mutant-cli.log` | 只有 `a_document_written_over_an_earlier_one…`（`publish_from_pid.rs:2003`） |
| `.pid`／Oracle 分支的 `_Index.json` 退回 `fs::write` | `runs/t43-mutant-index-pid.log` | 只有 `a_package_written_over_an_earlier_one…`（`publish_from_pid.rs:2040`） |
| 数据库分支的 `_Index.json` 退回 `fs::write` | `runs/t43-mutant-index-db.log` | 只有 `a_staged_package_written_again…`（`:2315`） |
| `write_three` 退回逐个 `fs::write` | `runs/t43-mutant-old.log` | `three_files…`（`:2509`）、数据库分支验收（`:2226`）、`a_staged_package_written_again…`（`:2315`）、命令行验收（`publish_from_pid.rs:1902`）、`a_package_written_over…`（`publish_from_pid.rs:2040`） |

行号是提交里的，每处改坏都是一行换一行。

**超出计划原文的。**

1. 三个提交，计划是一个任务一个。`9d24529` 提交之后准备故意改坏时发现一个洞：把 `write_atomic`、命令行的 `write_file` 或者哪一处索引退回原地写，没有一条用例会红，验收用例管的只是三件套失败之后的清理。补了三条带硬链接的用例，没有 amend，另起 `ecfc5c4`。之后做掐进程那次核对时对着代码重读用法里的一句「至多留下一个 `.tmp` 文件」，它不对：三件套先都写到临时名再依次改名，掐在中间留下的 `.tmp` 可以不止一个，也可以留下三件套不齐的图。`b69ce84` 把用法、模块文档和 `write_three` 的文档改成现在的说法。单独检出 `9d24529` 或 `ecfc5c4` 会看到那句不对的话。要合成一个提交得改写历史，没有自己做。
2. `write_atomic` 不是无条件改名。改名会把名字下的东西换成一个普通文件：是符号链接，换掉的是链接本身；是设备，有权限时换掉的是设备节点。所以只有名字空着或者名字下是普通文件才改名，其余照旧 `fs::write`，`--out NUL` 这样的用法不变。三件套不做这个判断，一律改名：名字是程序自己拼的 `<图名>_Data.xml`，有别的东西占着就该失败。带来一处行为差别：三件套的名字下如果是符号链接，旧代码顺着链接写进它指的文件，新代码把链接换成文件。
3. 三件套没有逐个调 `write_atomic`，而是三个都先写到临时名再依次改名。这样写不成（磁盘满、临时名建不出来）时一个名字都还没动，要回头删的只剩「改名改到一半失败」这一种。
4. 错误句子的开头变了。旧代码里三件套建不了文件时说 `create <路径>: …`，写到一半失败才说 `write <路径>: …`；现在都是 `write <路径>: …`，原因部分是 `rename` 的报错。`_Index.json` 的 `problems` 里那一句跟着变。命令行的 `write_file` 本来就说 `write`，没变。
5. 用法说明比计划要的多一段。计划只要「`--out-dir` 不清理旧文件，以 `_Index.json` 为准」；临时名那一段是加的，因为用户会在目录里见到 `.tmp`。
6. 带硬链接的四条用例，计划的验收里没有。

**没有改的。**

1. 单图模式的 `--out-dir`。三个文件各自整份到位，但不是「要么都有要么都没有」：第三个写不成时前两个留着，退出码 1。计划说的是整包的三件套。
2. 运行开始时不删旧的 `_Index.json`。掐掉的运行之后，目录里是上一轮的索引加这一轮已经到位的文件；用法说以索引为准，那时的索引说的是上一轮。
3. 这一轮失败的图，上一轮留下的文件不清。写临时名时就失败的，上一轮的三个都还在；改名改到第三个失败的，前两个已经换掉又删了，上一轮的第三个还在。用法里「a drawing that fails this time may still have files of an earlier run there」说的是这个。
4. 不 `fsync`，改名失败不重试。`write_atomic` 的文档里写了不防掉电。

**没有验的。**

1. 掐在一张图的三次改名之间。做的那一次掐在了两张图之间。「每个文件都是整份」靠的是改名这个机制；「三件套可能不齐」是照代码说的，没有见到过。
2. 掉电、系统崩溃。
3. Unix 上的行为只有 CI 跑得到，这台机器是 Windows。已知的一处差别没有用例：只读的目标在 Unix 上会被改名换掉（看的是目录的权限），在 Windows 上和以前一样被拒绝（手工核对的第一行）。
4. 符号链接那一路没有用例。`/dev/null` 只有 `only_a_free_name…` 里的那一句判断，没有真的往里写：写设备的那条用例只在 Windows 上编。
5. `takes_its_name_by_rename` 里「查询报错就不改名」那一支，只在 Windows 上有用例守着。CI 在 Linux 上，那里 `/dev/null` 查得到（是字符设备），走的是另一支，所以在 CI 里把这一支改坏不会红。
6. 带硬链接的四条用例要求临时目录所在的文件系统支持硬链接。不支持的地方（FAT、exFAT、有些网络盘）`fs::hard_link` 失败，用例是红，不是跳过。
7. 别的程序正开着目标文件时改名会怎样。照 Windows 的规则，对方没给删除共享时改名失败，这张图记成失败；以前原地写，对方给了写共享就写得进去。没有试。
8. 被换掉的文件的权限、属性、ACL。新文件按新建文件的默认来，不继承旧文件的，没有比对过。
9. 两个进程同时往一个输出目录发。临时名带进程号，互相不踩临时文件，同名的目标谁改名晚谁留下。没有试。

**顺带记下的。**

1. 这台机器上 `NUL` 的元数据查不出来（`symlink_metadata` 报错，报的不是「找不到」），`takes_its_name_by_rename` 里才有「报错就不改名」那一支。
2. 汇总 `runs/t43-mutants-summary.txt` 里每项的「compile errors: N」是脚本的标签写错了，数的是 `error: test failed, to rerun pass …` 那几行。
3. 向你汇报过一次「库测试 1 574 条全过」，那时全量跑的是 1 573 条，最后加的一条只过滤着跑过；已经更正，全量的实测是上面的 1 575。

### T4.4 zip 条目设上限（2026-11-16，`7b98510`）

**红。** 先只把上限做成参数，从 `publish_backup_package` 一路传到 `publish_zip`，传到了不用；再加验收用例 `a_pid_entry_over_the_limit_fails_by_name_and_the_rest_are_published`（`src/publish/backup_package.rs:2663`）。用例造一个只有 `.pid` 的包：`Manifest.txt` 加一个 `PlantData~2~1.zip`，里面按下面的顺序放四个条目，装的都是仓库里已有的图，上限调到第三个的大小（249 856 字节）：

| 条目 | 装的是什么 | 该怎样 |
|---|---|---|
| `U01/OVER.pid` | `test-file/DWG-0201GP06-01.pid`，389 120 字节，条目头如实 | 声明的大小超限：不解压，失败并点名 |
| `U01/UNDERSTATED.pid` | 同一份 389 120 字节，条目头（本地头和中央目录两处）改成声明 1 000 字节 | 实际超限：读不完，失败并点名 |
| `U01/AT-THE-LIMIT.pid` | `test-file/DWG-0202GP06-01.pid`，249 856 字节 | 正好等于上限：照写 |
| `U01/UNDER.pid` | `test-file/D06.pid`，118 784 字节 | 照写 |

没设上限时红在写出和失败的张数上（`runs/t44-red.log`，当时的行号是 `:2642`）：

```text
thread 'publish::backup_package::tests::a_pid_entry_over_the_limit_fails_by_name_and_the_rest_are_published' (7776) panicked at src\publish\backup_package.rs:2642:9:
assertion `left == right` failed: {"package":…
  left: (3, 1)
 right: (2, 2)
```

日志里打出来的索引还看得到两件事：`OVER` 被照常解压，三件套都写出来了；`UNDERSTATED` 在没改的代码上就已经失败，句子是 zip 库自己的：`PlantData~2~1.zip U01/UNDERSTATED.pid: File is larger than its declared uncompressed size`。

**改。** 都在 `src/publish/backup_package.rs`：

- `MAX_PID_ENTRY_BYTES`（`:610`）：私有常量，512 MiB。
- `read_entry_within(entry, stated, limit)`（`:619`）：
  - `stated`（条目头声明的解压后大小）大于 `limit`，一个字节都不读，报 `would unpack to <stated> bytes, over the limit of <limit> for a .pid entry`；
  - 否则经 `take(limit + 1)` 读到底，读出来的多于 `limit` 就报 `states <stated> bytes and unpacks to more than the limit of <limit> for a .pid entry`；
  - 读的过程出错，`io::Error` 的话原样带出；
  - 装内容的 `Vec` 不按声明的大小预留，随读随涨。
- `publish_zip`（`:639`）取 `entry.size()`（`:681`）之后调它。失败之后的记账就是原来读失败时的那一套：顶层 `problems` 加一句 `<zip 文件名> <条目名>: <原因>`，`failed` 加一，`drawings` 里记一张没有文件的图（它自己的 `problems` 是 `read: <原因>`），接着处理下一个条目。原来就有的读失败，句子一个字没变。
- `publish_backup_package`（`:299`）的签名没变，它把常量交给新的私有函数 `publish_package`（`:316`），后者多一个 `max_pid_entry_bytes`，传给 `publish_zip`（`:421`）。测试经 `publish_package` 把上限调小。
- 模块文档（`:22`）和 `publish_backup_package` 的文档（`:285`）各加一段。没有加公开项；`src/bin/pid_publish_xml.rs` 和用法说明没动。

**绿。** `runs/t44-green.log`：`cargo fmt --all` 之后按 `the_limit` 过滤，三条新用例全过。提交前全跑了一遍（`runs/t44-precommit.log`，退出 0，213 s）：`cargo fmt --all -- --check` 过；`cargo clippy --locked --workspace --all-targets -- -D warnings` 没有警告；库内 1 578 条（比 T4.3 多 3 条）、`--bin pid_publish_xml` 5、`cli_help_streams` 2、`publish_xml_cli` 36、`publish_chain_faults` 7、`publish_completeness` 10、`publish_diff_accepted_baseline` 19、`publish_unit_values` 1、`publish_from_pid` 22，全过。fmt 和 clippy 这两步过没过是从退出码看的，日志里没有它们的输出，原因见「顺带记下的」第 2 条。

新加的用例：

| 用例 | 位置 | 看的是什么 |
|---|---|---|
| `a_pid_entry_over_the_limit_fails_by_name_and_the_rest_are_published` | `backup_package.rs:2663` | 验收，包就是上面那四个条目。`written`、`failed` 是 2、2，`pid_entries` 是 4；顶层 `problems` 两句，第一句整句是 `PlantData~2~1.zip U01/OVER.pid: would unpack to 389120 bytes, over the limit of 249856 for a .pid entry`，第二句以 `PlantData~2~1.zip U01/UNDERSTATED.pid: ` 开头（后半句谁拦下的谁说，不钉）；`drawings` 四张按条目的顺序，前两张 `files` 是空的、自己的 `problems` 各一句 `read: …`，后两张各三个文件、没有 `problems`；输出目录里正好是那六个文件加 `_Index.json`。同一个包再用 `publish_backup_package` 发一遍（出厂的上限）：3、1，只剩 `UNDERSTATED` 那一句 |
| `the_limit_a_package_is_published_with_is_512_mib` | `backup_package.rs:2762` | 经 `publish_backup_package`。两个条目装的都是 `D06.pid`，条目头分别改成声明 536 870 913 和 536 870 912 字节。前一个被拒，句子是 `PlantData~2~1.zip U01/A-BYTE-OVER.pid: would unpack to 536870913 bytes, over the limit of 536870912 for a .pid entry`；它后面没有哪一句提到上限 |
| `an_entry_is_read_up_to_the_limit_and_not_past_it` | `backup_package.rs:2822` | `read_entry_within` 本身，读取器是一个数着自己交出多少字节的 `Counted`。等于上限、低于上限、空条目：整份读出；声明超限：报错，交出 0 个字节；装了 1 MiB 却只声明 10 字节、上限 100：报错，正好交出 101 个字节；上限 0、装了 1 个字节：报错，交出 1 个；声明和实际不一致但都在上限以内：读到多少是多少；上限是 `u64::MAX`：不回绕；读取器报错：原话带出 |

**对照矩阵。** `t44`（`7b98510`，`pid_publish_xml-7b98510-release.exe`，4 750 848 字节，SHA-256 `010ce0a0…1553a548`）对 `t43b`：61 项，155 s，717 个文件全部相同，没有只在一边的。

**故意改坏。** 在 `7b98510` 上逐处改一行，跑 `cargo test --locked --lib backup_package`（21 条），再 `git checkout` 还原（脚本 `mutants-t44.ps1`，汇总在 `runs/t44-mutants-summary.txt`，各处的日志是 `runs/t44-mutant-<名字>.log`）。每份日志里都有 `Compiling pid-parse` 那一行，都没有编译错误。

| 改坏的地方 | 红的用例 | 红在哪里 |
|---|---|---|
| `stated`：不看声明的大小 | 三条都红 | 验收：`OVER` 的句子成了 `states 389120 bytes and unpacks to more than the limit of 249856 for a .pid entry`，也就是解压到上限多一个字节才停；512 MiB 那条：多声明一个字节的条目被发布了，`left: None`；函数层：`left: Err("states 101 bytes and unpacks to more than the limit of 100 for a .pid entry")` |
| `at-limit`：`>` 改成 `>=` | 三条都红 | 验收：`left: (1, 3)`、`right: (2, 2)`，正好等于上限的那张被拒了 |
| `take`：`take(u64::MAX)`，读不设界 | 函数层 | 交出的字节数：`left: 1048576`、`right: 101` |
| `one-short`：`take(limit)`，少一个字节 | 函数层 | 超限的条目被截成 100 个字节、当成整份返回：`left: Ok([7, 7, …`，该是 `Err` |
| `past`：读过了界不拒 | 函数层 | 101 个字节被返回：`left: Ok([7, 7, …`，该是 `Err` |
| `sentence`：顶层那一句不带原因 | 验收、512 MiB | `left: "PlantData~2~1.zip U01/OVER.pid"` |
| `count`：`failed` 不加一 | 验收 | `left: (2, 0)`、`right: (2, 2)` |
| `drawing`：图自己那一句去掉 `read: ` | 验收 | `left: Array [String("would unpack to 389120 bytes, …")]` |
| `unthreaded`：`publish_zip` 不用传进来的上限 | 验收 | `left: (3, 1)`、`right: (2, 2)`，和红的时候一样 |
| `shipped`：公开入口传 `u64::MAX` | 512 MiB | `left: None` |
| `value`：常量多一个字节 | 512 MiB | `left: None` |

`take`、`one-short`、`past` 三处，两条整包的用例都照过，只有函数层那一条红：原因是「没有验的」第 1 条。

还原之后 `src`、`tests` 与 `7b98510` 一致（`git diff --quiet HEAD -- src tests` 退出 0）。没有再全量跑一遍：提交前那一遍跑的就是这份内容。

**超出计划原文的。**

1. 计划写的是「读的时候再用 `take` 兜住」。只兜不判是不够的：`take(limit)` 会把超限的条目悄悄截短，当成一份完整的 `.pid` 交下去（上表的 `one-short`）。所以读的是 `limit + 1` 个字节，读满了就拒绝。
2. 计划写「测试里把上限调小」，没说怎么调。做法是公开函数不变，里面多一层带上限参数的私有函数。
3. 句子里带了声明的大小和上限两个数。
4. 多了两条用例：出厂的上限是不是 512 MiB，和函数层的那一条。

**没有改的。**

1. 上限不能从命令行调，也不读环境变量。512 MiB 是计划里暂定的数。
2. 别处解压 zip 条目的地方没有设上限：`src/refdata_archive.rs:198`、`src/backup/datamap.rs:384`、`src/publish/catalog_index.rs:443`，都是 `read_to_end`。它们读的不是 `.pid` 条目，计划这一条没有点到。`src/refdata_archive.rs:162`、`src/backup/zip_index.rs:95` 只列条目，不解压。
3. SQL Server 分支没有动：它只列 zip 里的条目名，图是从库里的行发的，不解压 `.pid`。
4. 条目头声明的大小和实际不一致、但都在上限以内的，这里不管，由 zip 库管（见「顺带记下的」第 1 条）。

**没有验的。**

1. `take` 这道保险，用真实的 zip 触发不到。zip 9.1.0 的 `Crc32Reader`（它的 `src/crc32.rs:43`–`:52`、`:75`–`:86`）读到超过声明大小的那一刻就报 `File is larger than its declared uncompressed size`；能走到 `take` 的条目，声明的大小都不超过上限，所以总是它先报。实测也是：`runs/t44-mutant-at-limit.log` 打出来的索引里，`UNDERSTATED` 那一句还是 zip 库的话。它不管的只有大小未知的条目（`size_unknown`，只在流式读取里置位，它的 `src/read/stream.rs:785`），`ZipArchive::by_index` 走不到那里。所以 `take` 和读完之后的判断只有函数层那一条用例在测；它防的是依赖换了版本、或者这里换了读法以后没人把关。这次真正补上的是「声明得很大」的条目，原来没有任何东西拦它。
2. 没有造过真的解压出 512 MiB 的条目。512 MiB 那条用例改的是条目头，条目里装的是 118 784 字节的 `D06.pid`。
3. 内存占用没有量。`Vec` 随读随涨，贴着上限的条目读完时容量可以比内容大出不少。
4. 声明的大小超过 4 GiB（zip64）的条目没有造过。
5. 没有在 Linux 上跑（CI 在 `ubuntu-latest`）。用例只用了仓库里的文件和临时目录。

**顺带记下的。**

1. zip 9.1.0 对「装的比声明的少」的条目不报错，只在读完时校验 CRC。`stated` 那处改坏（不看声明的大小）里，512 MiB 那条用例的两个条目都被发布了，`problems` 是空的：声明 512 MiB 和多一个字节、实际 118 784 字节的条目，zip 库都读得下来。
2. `run-stable.ps1` 起的是 `cmd /c <命令行> 2>&1`。命令行是 `a && b && c` 时，`cmd` 只把最后一条的 stderr 并进日志，前面几条的留在起它的那个 shell 里。所以 `runs/t44-precommit.log` 里没有 fmt、clippy 的输出，也没有库测试那一步的 `Compiling`、`Running` 行；这一次 clippy 的 stderr 是从那个 shell 里读的（`Checking pid-parse`、`Checking pid-plot`、`Finished`，没有警告）。`runs/t42-precommit.log`、`runs/t43-precommit.log` 也是这样，那两次 clippy 过没过同样是从退出码看的：带着 `-D warnings`，有警告 clippy 就不是 0，`&&` 后面的测试也不会跑。以后要整串都进日志，命令行得写成 `(a && b && c)`。

### T4.5 dump 里的表只找一遍（2026-11-16，`79250b8`）

**量。** 包是 `D:\work\cad\pid-test-data`，`Export.dmp` 74 489 856 字节；程序是 `7b98510` 的 release 构建（T4.4 对照矩阵用的那一份）。整包跑三次；`load_dump` 的各步用一个临时的 example 在同一个进程里各计五轮（`examples/t45_time_exp_read.rs`，量完已删，没有进提交）。读数在 `runs/t45-measure.txt`、`runs/t45-searches.log`：

| 项 | 用时 | 备注 |
|---|---|---|
| 整包，三次 | 19.46、18.82、19.28 s | 53 张图，160 个文件 |
| `fs::read` | 0.022–0.025 s | |
| `ExpTables::read` | 2.018、2.200、2.338、2.470、2.597 s | 另起一次进程是 2.092–2.788 s |
| `drawings()` | 不到 0.001 s | 53 行 |
| `read_plant_groups` | 0.019–0.027 s | 6 行 |

`ExpTables::read` 占整包的 10.5 %–13.5 %，过了一成，按计划改。它的时间几乎都在找表上：把它做的那些搜索原样抄出来单独计时，是 1.96–2.61 s。它为 25 个表名从头搜了 36 次：

| 谁找的 | 表 | 每张找几次 |
|---|---|---|
| `pin`（`oracle_exp.rs:816`） | `APPLICATIONS` | 1 |
| 必需的 13 张 | `T_PIPINGPOINT` … `T_DRAWING` | 1 |
| 可选的 6 张 | `T_PLANTITEM`、`T_INLINECOMP`、`T_EQUIPMENT`、`T_VESSEL`、`T_PIPINGCOMP`、`T_INSTRUMENT` | 2：先看有没有，再读 |
| 数据字典的 5 张 | `ITEM`、`ITEMATTRIBUTIONS`、`UNIQUEATTS`、`ENTITIES`、`ATTRIBUTES` | 2：同上 |

一次搜索的用时大致和表在文件里的位置成正比：`T_VESSEL` 在第 74 404 351 字节，一次 0.151 s，找两次；`APPLICATIONS` 在第 4 462 字节，不到 0.0001 s。同一种朴素搜索对整份 dump 找一遍所有的 `INSERT INTO "`，只要 0.031–0.041 s，找到 154 处。

**改。** `src/publish/oracle_exp.rs`：

- `Inserts`（`:2444`，`pub(crate)`）。`Inserts::of(dump)`（`:2451`）对 dump 走一遍，记下每一处 `INSERT INTO "` 的位置；`first(name)`（`:2462`）在这些位置里按顺序找第一处后面紧跟着 `name` 和 `"` 的。它给出的和原来 `find_slice(dump, b"INSERT INTO \"NAME\"")` 给出的是同一个位置：原来搜到的每一处都以 `INSERT INTO "` 开头，所以都在记下的位置里；记下的位置按文件顺序排，第一处符合的就是最靠前的那一处。
- `Inserts::table`（`:2471`）、`optional_table`（`:2480`）、`text_cells`（`:2496`）替掉 `ExpTables::read` 里的 13 处 `read_table_tolerant(dump, …)`，以及 `read_optional_table`、`read_text_cells` 两个函数（删了）。缺表时的行为没变：必需的表报 `<表名>: INSERT not found`；可选的表读成没有列、没有行的空表；字典的表给 `None`（五张都缺是空字典，只缺一部分报错）。
- `read_table_tolerant`（`:2534`）拆成两步：找位置，和从位置读表的 `read_table_tolerant_at`（`:2542`）。后者的函数体就是原来的后半段，没有改。
- `ExpTables::read`（`:343`）：`pin` 之后建一次 `Inserts`，19 张表按原来的字段顺序从它取，所以第一个出错的表还是原来那一张；字典经 `DumpDictionary::read_inserts` 也从它取。`pin` 排在建索引之前，不是这种 dump 的文件还是一上来就被拒，不会先被走一遍。
- `read_plant_groups`（`:1131`）：原来先搜一遍看有没有 `T_PLANTGROUP`，再让 `read_table_tolerant` 搜第二遍；现在搜一遍，从搜到的位置读。
- 只在测试里用的 `load_components`（`:850`）改用 `Inserts`，因为 `read_optional_table` 删了。

`src/publish/placement.rs`：`DumpDictionary::read(dump)`（`:130`）的签名和文档没变，函数体挪进新的 `pub(crate) fn read_inserts`（`:135`），`read` 自己建一个 `Inserts` 再调它。

没有新的公开项，没有改任何错误信息。

**绿。** 这一条不改行为，没有「先红」那一步。新加的三条用例钉的是「和原来一样」，它们红不红在「故意改坏」里看。

- `runs/t45-try1.log`：`cargo fmt --all` 之后按 `oracle_exp`、`placement` 过滤，58 条全过。
- 提交前（`runs/t45-precommit.log`，退出 0，148 s；这次整串命令加了括号，fmt 和 clippy 的输出都在日志里）：`cargo fmt --all -- --check` 过；`cargo clippy --locked --workspace --all-targets -- -D warnings` 没有警告；库内 1 580、`--bin pid_publish_xml` 5、`cli_help_streams` 2、`parser_panic_safety` 6（计划点名的验收）、`publish_chain_faults` 7、`publish_completeness` 10、`publish_diff_accepted_baseline` 19、`publish_from_pid` 22、`publish_unit_values` 1、`publish_xml_cli` 36，全过。
- 之后又加了第三条用例，重跑了 fmt、clippy 和库内用例（`runs/t45-precommit2.log`，退出 0，42 s，库内 1 581 条，比 T4.4 多 3 条）。集成用例没有再跑：后加的只是 `#[cfg(test)]` 里的一条用例，集成用例编进去的库代码没有变。

新加的用例都在 `oracle_exp.rs` 测试模块的末尾：

| 用例 | 位置 | 看的是什么 |
|---|---|---|
| `a_table_is_found_where_a_search_from_the_start_finds_it` | `:5621` | 一份手写的 dump：`ITEMATTRIBUTIONS` 排在 `ITEM` 前面；数据里有一句 `INSERT INTO "T_NOTE"`，排在真正的语句前面；`T_NOTES` 排在 `T_NOTE` 前面；`CODELISTS` 出现两次；结尾是截断的 `INSERT INTO "T_VES`。10 个名字（有不存在的、只是别的名字开头一段的、空的）逐个比 `first` 和 `find_slice`。再直接钉几处：`ITEM` 不是 `ITEMATTRIBUTIONS`；`T_NOTE` 取的是数据里那一句（原来也是）；`CODELISTS` 取第一处；截断的名字找不到。另有四份极短的 dump（空的、`INSERT INTO`、`INSERT INTO "`、`INSERT INTO ""`）也逐个比 |
| `the_inserts_of_the_committed_dump_are_where_a_search_finds_them` | `:5690` | 仓库里的 `test-file/backup-test/DWG-0202GP06-01_p/Export.dmp`。记下的位置和用 `find_slice` 一处一处往后找出来的完全一样；28 个名字（`ExpTables::read` 和字典要的 25 个，加 `T_PLANTGROUP`、`T_`、一个不存在的）逐个比 `first` 和 `find_slice`，其中 `APPLICATIONS` 和必需的 13 张还要求找得到 |
| `a_table_the_dump_lacks_is_an_error_unless_the_reader_can_do_without_it` | `:5750` | 测试里现成的合成 dump。13 张必需的表各去掉一次，`ExpTables::read` 都报 `<表名>: INSERT not found`；6 张可选的表缺了（两张本来就不在这份 dump 里，四张各去掉一次），都读成名字对、没有列、没有行的空表 |

**改后再量。** 同一个包，改动后的 release 构建（提交前从工作树构建的，非测试代码和 `79250b8` 相同）。各步在 `runs/t45-measure-after.txt`：

| 项 | 改前 | 改后 |
|---|---|---|
| `ExpTables::read` | 2.018–2.597 s | 0.275–0.294 s |
| `read_plant_groups` | 0.019–0.027 s | 0.003 s |

整包的用时要两版轮流跑才能比。改后单独量的三次是 14.47、15.20、14.07 s，可是这时候机器本身就比量「改前」那一次快：同一段搜索代码当时要 1.96–2.61 s，这时只要 1.66–1.77 s；旧版程序这时跑下来也只要 15.4–15.6 s，不是 19 s。所以用 `t45-ab.ps1` 让 `7b98510` 和改动后的程序交替各跑四次（`runs/t45-ab.txt`）：

| 程序 | 四次 | 中位数 |
|---|---|---|
| `7b98510` | 15.60、15.64、15.36、15.64 s | 15.62 s |
| 改动后 | 13.96、14.12、16.55、14.21 s | 14.16 s |

整包省约 1.5 s，一成左右。改动后的那一次 16.55 s 是怎么来的，没有查。两版各自第一次写出的 160 个文件逐个比 SHA-256，全部相同。

**对照矩阵。** `t45`（`79250b8`，`pid_publish_xml-79250b8-release.exe`，4 755 456 字节，SHA-256 `e0989a20…c8e74a51`）对 `t44`：61 项，96 s，717 个文件全部相同，没有只在一边的。

**故意改坏。** 在 `79250b8` 上逐处改，跑 `cargo test --locked --lib`（1 581 条），再 `git checkout` 还原（脚本 `mutants-t45.ps1`，汇总在 `runs/t45-mutants-summary.txt`，各处的日志是 `runs/t45-mutant-<名字>.log`）。每份日志里都有 `Compiling pid-parse` 那一行，都没有编译错误。21 处里 20 处红，1 处是等价的改动。

| 改坏的地方 | 红了几条 | 新用例里红的 |
|---|---|---|
| `quote`：不看名字后面的 `"` | 3 | 第一、二条 |
| `last`：取最后一处，不取第一处 | 11 | 第一、二条 |
| `skip`：走的时候漏掉第一处 | 5 | 第一、二条 |
| `required`：必需的表缺了读成空表 | 1 | 第三条，只有它 |
| `optional`：可选的表缺了报错 | 14 | 第三条 |
| `sentence`：缺表的那句话换了说法 | 1 | 第三条，只有它 |
| `at`：必需的表从 dump 开头读 | 26 | 没有 |
| `optional-at`：可选的表从 dump 开头读 | 22 | 没有 |
| `cells-at`：字典的表从 dump 开头读 | 19 | 没有 |
| `cells-absent`：字典的表缺了读成没有行的表 | 1 | 没有；红的是 `placement` 的 `a_dump_without_a_dictionary_places_nothing_and_half_of_one_is_refused` |
| `groups-at`：`read_plant_groups` 从 dump 开头读 | 5 | 没有 |
| `groups-absent`：`read_plant_groups` 总是找不到 | 4 | 没有 |
| `dictionary`：`ExpTables::read` 的字典从空 dump 读 | 7 | 没有；7 条都在 `stored_values` |
| `free-0`：`read_table_tolerant` 从 dump 开头读 | 12 | 没有 |
| `name-plantitem`、`name-inlinecomp`、`name-equipment`、`name-vessel`：可选的表换成一个不存在的名字 | 6、9、11、9 | 第三条 |
| `name-pipingcomp`：同上 | 3 | 第三条 |
| `name-instrument`：同上 | 1 | 第三条，只有它 |
| `free`：`read_table_tolerant` 从找到的位置往后一个字节读 | 0 | 等价的改动 |

`free` 没有红不是漏测。`read_table_tolerant_at` 拿到位置只做两件事：找这一行的行尾，在这一行里找 `(` 和 `) VALUES` 之间的列清单。从 `INSERT` 的 `I` 开始还是从 `N` 开始，结果一样。换成从 dump 开头读（`free-0`）就红 12 条。

还原之后 `src`、`tests` 与 `79250b8` 一致（`git diff --quiet HEAD -- src tests` 退出 0）。

**超出计划原文的。**

1. 计划说的是「19 张表（6 张可选）」。实际从头搜索的是 25 个表名、36 次：多出来的是 `pin` 找的 `APPLICATIONS`，和数据字典的 5 张表（它们和可选的表一样各找两遍）。字典的 5 张也改成从索引取，为此动了计划这一条没有列的 `src/publish/placement.rs`。`APPLICATIONS` 没有动，见「没有改的」第 1 条。
2. `read_plant_groups` 也从找两遍改成找一遍。它不在 `ExpTables::read` 里，整包发布时紧接着就调它，是同一种重复。
3. 加了三条用例。计划的验收只有对照矩阵和 `tests/parser_panic_safety.rs`。
4. 多量了一组两版轮流跑的整包用时。

**没有改的。**

1. `pin` 和它用的严格读法 `read_table`（`:1697`）还是自己从头找 `APPLICATIONS`。它排在建索引之前，为的是不是这种 dump 的文件不被走一遍；`APPLICATIONS` 在测试包里位于第 4 462 字节，找它不到 0.0001 s。
2. 只取几张表的入口还是每张表各自从头找：`load_ports`（`:860`）、`load_connectivity`（`:869`）、`load_compositions`（`:882`）、`load_opc`（`:893`）和 `read_backup_drawings`（`:1200`）。它们的用时没有量过。
3. `find_slice`（`:1868`）本身没动，还是逐字节比的朴素搜索。现在用它的是上面这两条和 `read_plant_groups`。
4. `examples/` 下几个 `probe_oracle_exp_*.rs` 各有自己抄的一份搜索，没动。

**没有验的。**

1. 只量了这一份 74 MB 的 dump，只在这一台机器上量。
2. 索引占的内存没有量。每一处 `INSERT INTO "` 记 8 个字节，测试包的 dump 是 154 处。通篇都是 `INSERT INTO "` 的畸形文件，最多记到文件大小的 0.6 倍（每 13 个字节一处）；这种文件得先有一张合格的 `APPLICATIONS` 过了 `pin`，才会走到建索引这一步。
3. 故意改坏只跑了库内用例，没有跑集成用例。
4. 全套门禁在这一条之后没有跑，留到检查点 4。
5. 没有在 Linux 上跑。

**顺带记下的。**

1. 这台机器不同时段快慢差得不少。同一个 `7b98510` 的程序发同一个包，先是 18.8–19.5 s，后来是 15.4–15.6 s；同一段搜索代码先是 1.96–2.61 s，后来是 1.66–1.77 s。`docs/plans/2026-11-10-publish-export-completion.md:219` 记的「19 秒」和先前那一次对得上。比用时要两版轮流跑，不能拿隔了一阵的两次读数相减。
2. 调试构建下搜索更慢，读 dump 的用例也跟着快了。同样的用例，T4.4 提交前那一遍和这一条提交前那一遍：库内 16.05 → 7.96 s，`publish_completeness` 18.41 → 6.32 s，`publish_diff_accepted_baseline` 53.64 → 10.18 s，`publish_from_pid` 93.34 → 72.73 s，`publish_unit_values` 3.32 → 0.98 s。这两遍不是轮流跑的，受上一条影响，只能看个方向。
3. 在库内用例里，「必需的表缺了要报错」和「`instrument` 读的是 `T_INSTRUMENT`」原来没有用例钉着：`required`、`name-instrument` 两处改坏，红的只有这次新加的第三条。集成用例钉没钉，没有查。

### T4.6 表清单的一致性（2026-11-16，`b7eeaf7`）

**量。** 两份清单今天是一致的。

- `PUBLISH_TABLES`（`mdf_load.rs:28`）24 张。
- 目录表（`catalog.rs`）17 行，其中 12 行有 `db_subtables`（`Vessel`、`Nozzle`、`PipeRun`、`PipingPoint`、`Instrument`、`Note`、`PipingComp`、`SignalRun`、`Exchanger`、`Mechanical`、`BranchPoint`、`PipingBranchPoint`），点到的表去重后 18 张，都在 `PUBLISH_TABLES` 里。另外 5 行（`EquipmentOther`、`OPC`、`*`、`$document`、`$derived`）没有子表。
- `PUBLISH_TABLES` 剩下的 6 张由加载器自己的语句读：`T_Drawing`、`T_Representation`、`T_Relationship`、`T_ModelItem` 缺了加载就报错；`codelists`、`attributes` 和那 18 张子表一样经 `prepare_optional`，缺了就跳过，一声不响。

改前带 `-v` 说什么：用 `79250b8` 的发布构建，在仓库里的三种数据库输入上各跑不带与带 `-v` 两遍（脚本 `t46-verbose.ps1`，输出在 `runs/t46-v-before/`；写这一节时又照样跑了一遍，在 `runs/t46-v-79250b8/`，行数相同）：

| 输入 | 不带 `-v` 的 stderr | 带 `-v` 的 stderr |
|---|---|---|
| `TEST02_p/extracted/Export.mdf` | 1 行 | 36 行：info 25 行（暂存的 23 张表各一行，一行 `  T_ProcessEquipment: not found in MDF, skipped`，一行汇总），`oxidized_mdf` 的 warn 10 行，加上不带 `-v` 时就有的那 1 行 |
| `TEST02_p/extracted/Export_v2.sqlite`（旧镜像） | 2 行 | 2 行，一行日志都没有 |
| `TEST02_p` 整包 | 1 行 | 36 行，同 `.mdf` |

旧镜像那一路缺表完全无声；`.mdf` 那一路只有暂存说了一句，加载器这一侧不说。对照矩阵的 61 项都不带 `-v`，每一项的 stdout、stderr、退出码都在逐字节比的文件里，所以「不带 `-v` 时 stderr 不变」直接由矩阵验。

**红。** 先写用例，代码不动（`runs/t46-red.log`）。库内按 `sqlite_load` 过滤 29 条，27 过 2 红；命令行那一条红：

```text
---- publish::sqlite_load::tests::a_skipped_table_is_logged_once_for_all_the_items_that_read_it stdout ----
assertion `left == right` failed
  left: []
 right: ["  codelists: not in the database, skipped", "  attributes: not in the database, skipped", "  T_PlantItem: not in the database, skipped", "  T_Equipment: not in the database, skipped", "  T_ProcessEquipment: not in the database, skipped", "  T_Connector: not in the database, skipped"]

---- publish::sqlite_load::tests::every_staged_table_is_asked_for stdout ----
assertion `left == right` failed: nothing reads T_PipingPoint: the load does not miss it; it logs []
  left: []
 right: ["T_PipingPoint"]

---- cli_verbose_names_each_table_the_database_lacks_and_adds_nothing_else stdout ----
assertion `left == right` failed: -v should name each table the load skipped, once; stderr:
…
  left: []
 right: ["codelists", "attributes", "T_PlantItem", "T_Equipment", "T_Exchanger", "T_EquipComponent", "T_Nozzle", "T_ProcessEquipment", "T_Vessel", "T_Connector"]
```

三条红的都是「该说的没说」。`every_staged_table_is_asked_for` 停在 `T_PipingPoint`，它是清单里第一张可选的表：没有提示，就分不出「跳过了」和「根本没人读」。另外两条新用例（`every_subtable_the_catalog_names_is_staged`、`the_loader_asks_for_no_table_that_is_not_staged`）改前就是绿的，因为两份清单今天一致；它们红不红在「故意改坏」里看。

红的那一遍之后、提交之前，用例又改过说法：命令行那一条从 `…_adds_nothing_else` 改名为 `…_changes_nothing_else`，`every_staged_table_is_asked_for` 的断言说明也换了一句。提交的这一版在「没有提示」时红不红，由「故意改坏」的 `silent` 那一处重看：三条都红。

**改。** `src/publish/sqlite_load.rs`：

- `prepare_optional`（`:333`）：库里没有这张表时，跳过之前记一行 `info!("  {}: not in the database, skipped", …)`，样子和暂存一侧的行对齐。别的错误照旧上抛。
- `absent_table`（`:349`，新）：从 SQLite 的报错 `no such table: <表名>` 里取表名；不是这个样子的消息原样给出。
- `attach_business_columns`（`:442`，私有）：返回值从 `()` 改成 `bool`，表在是 `true`，被跳过是 `false`。
- `load_drawing_graph`（`:628`）里「每个对象 × 每张子表」的循环（`:678`）：记一个集合，这一次加载里已经知道缺的子表，后面的对象不再问。

`src/publish/mdf_load.rs`：`PUBLISH_TABLES` 的文档注释补一段，说它和加载器的关系、两边对不上各是什么后果、由哪里的用例拴着。清单本身没动。

没有新的公开项，没有新依赖（`log` 在 `mdf_load.rs` 里已经在用）。不带 `-v` 时程序不装日志器（`pid_publish_xml.rs:518`），这一行出不来；仓库里装日志器的也只有这一处。

**绿。**

- `runs/t46-green.log`：库内按 `sqlite_load` 过滤 29 条全过，`publish_xml_cli` 37 条全过。
- 提交前跑了三遍，日志里各组都是 `test result: ok`：
  - `runs/t46-precommit.log`：整串（clippy、库内 1 585 条、`--bin pid_publish_xml` 和 13 组集成用例）。
  - `runs/t46-precommit2.log`：用例改名之后，重跑 clippy、库内 1 585 条和 `publish_xml_cli` 37 条。
  - `runs/t46-precommit3.log`：这之后执行中断过一次。接着做时源码没有再动，在同一棵工作树上把整串重跑一遍再提交（退出 0，108 s）：`cargo fmt --all -- --check` 过；`cargo clippy --locked --workspace --all-targets -- -D warnings` 没有警告；库内 1 585（比 T4.5 多 4 条）、`--bin pid_publish_xml` 5、`cli_help_streams` 2、`inspect_cli` 21、`parser_panic_safety` 6、`publish_a01_raw_residual` 4（另 2 条忽略）、`publish_chain_faults` 7、`publish_completeness` 10、`publish_diff_accepted_baseline` 19、`publish_from_pid` 22、`publish_mdf_load` 8、`publish_rel_parity` 9、`publish_sqlite_load` 4、`publish_unit_values` 1、`publish_xml_cli` 37（多 1 条），全过。

新加的用例，前四条在 `sqlite_load.rs` 测试模块的末尾：

| 用例 | 位置 | 看的是什么 |
|---|---|---|
| `a_skipped_table_is_logged_once_for_all_the_items_that_read_it` | `sqlite_load.rs:1743` | 内存库只有 4 张必需的表加 `T_Vessel`，图上两个 `Vessel`。日志逐行钉死：6 行，都是 info 级，依次是 `codelists`、`attributes`、`T_PlantItem`、`T_Equipment`、`T_ProcessEquipment`、`T_Connector`，每张只说一次；在的那张 `T_Vessel` 两个对象都读到了（`Volume` 是 10 和 20） |
| `every_subtable_the_catalog_names_is_staged` | `:1785` | 目录表每一行的每张子表都在 `PUBLISH_TABLES` 里 |
| `the_loader_asks_for_no_table_that_is_not_staged` | `:1801` | 内存库恰好是 `PUBLISH_TABLES` 的 24 张表（每张只有加载器要的列），图上 12 种有子表的类型各一个对象。加载（`load_drawing_graph`，再加它不调的 `load_piping_points_for_objects`）不报错，一张表也不说跳过，12 个对象都载入了 |
| `every_staged_table_is_asked_for` | `:1831` | 同一个库，24 张表逐张删掉各加载一次。删的是 4 张必需的表：加载报错，错误以 `no such table: <表名>` 结尾。删的是别的：加载成功，说跳过的恰好是这一张 |
| `cli_verbose_names_each_table_the_database_lacks_and_changes_nothing_else` | `tests/publish_xml_cli.rs:1607` | 用 `publish-shortfalls.sql` 现造的 `.sqlite`（只有 4 张必需的表；换热器、管口、容器各一个），不带与带 `-v` 各发布一次。带 `-v`：stderr 依次点到 10 张表，每张一次。不带：没有这样的行。两次都退出 0，写出的两个文件逐字节相同，stderr 去掉日志器的行（以 `[` 开头）之后逐行相同 |

库内四条靠测试里的一个日志器看日志：`ThreadLog`（`:1585`）按线程收，`logged`（`:1610`）只在闭包运行期间记本线程的行，并行的用例互不串。

**`-v` 改后实测。** `b7eeaf7` 的发布构建，同一个脚本（`runs/t46-v-after/`）：

| 输入 | 不带 `-v` | 带 `-v` 的 stderr |
|---|---|---|
| `Export.mdf` | stdout、stderr 与改前逐字节相同 | 37 行，多 1 行：`[… INFO  pid_parse::publish::sqlite_load]   T_ProcessEquipment: not in the database, skipped` |
| `Export_v2.sqlite` | 同上 | 5 行，多 3 行：`codelists`、`attributes`、`T_ProcessEquipment` |
| `TEST02_p` 整包 | stdout 相同；stderr 只差输出目录的名字（两次的标签差一个字符） | 37 行，多 1 行，同 `.mdf` |

带 `-v` 时除了多出来的行，和改前比只有时间戳和暂存汇总行不同（毫秒数；整包那一路还有临时目录的名字）。`.mdf` 与 `.sqlite` 四次运行的 stdout 是同一份 `_Data.xml`（8 479 字节，SHA-256 `8c19a7ce…0c60306d`），改前改后、带不带 `-v` 都一样；整包带与不带 `-v` 写出的 4 个文件逐个相同。

**对照矩阵。** `t46`（`b7eeaf7`，`pid_publish_xml-b7eeaf7-release.exe`，4 756 992 字节，SHA-256 `5d311c3b…856df472`）对 `t45`：61 项，89 s，717 个文件全部相同，没有只在一边的（`runs/t46-matrix.log`）。每一项的 stderr 都在这 717 个文件里：不带 `-v` 时 stderr 一个字节没变。这份程序后来被同一提交的又一次构建同名覆盖了，那个文件名下的哈希不再是这个，见检查点 4。

**故意改坏。** 在 `b7eeaf7` 上逐处改，跑 `cargo test --locked --lib`（1 585 条）和 `cargo test --locked --test publish_xml_cli`（37 条），再 `git checkout` 还原（脚本 `mutants-t46.ps1`，汇总在 `runs/t46-mutants-summary.txt`，各处的日志是 `runs/t46-mutant-<名字>.log`）。每份日志里 `Compiling pid-parse` 都出现两次，都没有编译错误。16 处全红。下表的「第几条」照上面用例表的顺序。

| 改坏的地方 | 库内红 | 命令行红 | 新用例里红的 |
|---|---|---|---|
| `staged-vessel`：从 `PUBLISH_TABLES` 删 `T_Vessel`（目录表的子表） | 3 | 0 | 第二、三、四条 |
| `staged-drawing`：删 `T_Drawing`（必需的表） | 2 | 17 | 第三、四条；命令行红的 17 条都是原有的、要读 `.mdf` 或整包的用例 |
| `staged-codelists`：删 `codelists`（加载器自己读的可选表） | 2 | 0 | 第三、四条 |
| `staged-unread`：往 `PUBLISH_TABLES` 加一张没人读的 `T_Unread` | 1 | 0 | 第四条，只有它 |
| `catalog-unstaged`：给 `Nozzle` 加一张没暂存的子表 `T_NozzleData` | 3 | 1 | 第二、三、四、五条 |
| `catalog-typo`：`Exchanger` 的 `T_Exchanger` 写成 `T_Exchangers` | 3 | 1 | 第二、三、四、五条 |
| `unasked`：读 `attributes` 的那句不再读它 | 4 | 1 | 第一、四、五条；另有原有的 `load_codelist_index_*` 2 条 |
| `silent`：不说 | 2 | 1 | 第一、四、五条 |
| `wording`：换一种说法 | 2 | 1 | 第一、四、五条 |
| `debug`：降到 debug 级 | 2 | 1 | 第一、四、五条 |
| `warn`：升到 warn 级 | 1 | 0 | 第一条，只有它 |
| `name`：行里带的是数据库的整句报错，不是表名 | 2 | 0 | 第一、四条 |
| `every-time`：缺的子表每个对象都再问一遍 | 1 | 1 | 第一、五条 |
| `inverted`：在的子表读过一个对象就当成缺的 | 1 | 1 | 第一、五条 |
| `skipped-true`：跳过的子表报成在 | 1 | 1 | 第一、五条 |
| `read-false`：读到的子表报成缺 | 1 | 0 | 第一条，只有它 |

计划的两条验收都在表里：「从 `PUBLISH_TABLES` 里删掉一张表」是前三行（子表、必需的表、加载器自己读的可选表各一张），「给目录表加一张没暂存的子表」是 `catalog-unstaged`。

汇总里 `staged-drawing` 那一段写着 `rustc errors: 3`，不是编译错误：脚本把日志里以 `error` 开头的行都数了进去，这 3 行是红了的命令行用例带出来的程序自己的报错（`error: load drawing …: SQLite: no such table: T_Drawing`）。16 份日志里都没有 `error[E`，也没有 `could not compile`。

还原之后 `src`、`tests` 与 `b7eeaf7` 一致（`git diff --quiet HEAD -- src tests` 退出 0）。

**超出计划原文的。**

1. 同一次加载里已经知道缺的子表不再问。`prepare_optional` 是「每个对象 × 每张子表」调一次的，照字面只加提示，仓库里的 `Export.mdf`（没有 `T_ProcessEquipment`）上每个 `Vessel` 都会说一遍。为此 `attach_business_columns` 改成返回表在不在，`load_drawing_graph` 的循环里记一个集合。写出的东西不变（对照矩阵）。
2. 计划写的是「加一条用例」，实际是库内 4 条、命令行 1 条：一致性拆成三条（子表都暂存了；暂存的够加载器用；暂存的每张都有人读），一条钉提示本身，命令行那条钉带与不带 `-v` 的差别。
3. 动了计划这一条没有列的 `tests/publish_xml_cli.rs`。
4. 「`PUBLISH_TABLES` 的每张表也都有读它的地方」是按行为验的，不是按源码里的字样：逐张删表，加载要么报错点到它，要么说跳过了它。所以这一行提示不只是给 `-v` 看的，也是这条用例的观察手段。

**没有改的。**

1. 两份清单还是各写各的，没有让一份从另一份生成；现在是用例把它们拴在一起。
2. 提示按「一次加载」去重，不是按「一次运行」。整包里每张图各加载一次，缺的表每张图各说一遍；`codelists`、`attributes` 也是每张图读一次。
3. `T_Connector` 有两处读它：`PipeRun`、`SignalRun` 的子表链，和 `attach_pipe_endpoint_connections`（`:534`）。库里没有它、图上又有管线时，一次加载说两遍。
4. 暂存一侧原有的那一行（`mdf_load.rs` 的 `  <表名>: not found in MDF, skipped`）没有动。`.mdf` 里没有的表，带 `-v` 时暂存说一遍，加载器读到它时再说一遍，两句的说法不一样。
5. 不带 `-v` 时仍然什么都不说。库里缺一张子表，这一类对象就少一截属性，这件事不带 `-v` 还是看不到；它不在 T2 的「写得不完整」那两类里，`--strict` 也不管它。要不要算进去，等你定（仓库里的 `Export.mdf` 自己就缺 `T_ProcessEquipment`，算进去的话 A01 每次都会告警）。
6. 用法里 `-v` 的说明还是 `Print MDF loading diagnostics (table row counts, timing) to stderr.`，没有提加载器跳过的表，也没有提 `.sqlite` 输入现在带 `-v` 也有输出。改它会动 `--help` 的字节，而用法在对照矩阵里；留到阶段 6 改用法的那一条（T6.2）一起改。

**没有验的。**

1. 多张图的 SQL Server 包带 `-v` 是什么样子，没有量：仓库里唯一的 SQL Server 包只有 1 张图。
2. `-v` 的输出不在对照矩阵里（61 项都不带 `-v`）。它由命令行那条用例和上面的实测看，实测只有 A01 这一个库。
3. 故意改坏只跑了库内用例和 `publish_xml_cli`，别的集成用例没有跑。
4. 全套门禁在这一条之后没有跑，留到检查点 4。
5. 没有在 Linux 上跑。

**顺带记下的。**

1. 旧镜像 `Export_v2.sqlite` 缺 3 张表：`codelists`、`attributes`、`T_ProcessEquipment`；`Export.mdf` 只缺 `T_ProcessEquipment`。这是新加的提示说出来的，以前看不到。A01 这张图两条路写出的 `_Data.xml` 逐字节相同，所以在这张图上缺 `codelists`、`attributes` 没有带来差别；别的图没有可比的。
2. `every_subtable_the_catalog_names_is_staged` 和 `the_loader_asks_for_no_table_that_is_not_staged` 没有「先红」：两份清单今天一致，它们只在故意改坏里红过。
3. 库内用例现在有一个全局日志器（`ThreadLog`）。`log::set_logger` 一个进程只能成功一次，以后别的库内用例要看日志，得用它的 `logged`，不能再装一个。它把全局级别设到 info；不在 `logged` 里的线程，行直接丢掉。
4. 提示里的表名是从 SQLite 的报错里取的。`prepare_optional` 认缺表靠的本来就是这句 `no such table`；SQLite 哪天换了说法，跳过和提示会一起失效，缺表变成加载报错，库内这几条用例会红。
5. `warn`、`read-false` 两处改坏，红的只有新加的第一条；`staged-unread` 红的只有第四条。也就是说「提示是 info 级」「在的子表每个对象都要读」「暂存的表都得有人读」这三件事，在库内用例和 `publish_xml_cli` 里只有这次新加的用例钉着。别的集成用例钉没钉，没有查。

### 检查点 4（2026-11-16）

**门禁。** `gates.ps1 -Label cp4`，在 `b7eeaf7` 上跑。开跑时工作区只有这份计划文档有未提交的改动（T4.1–T4.6 的记录）；跑的中间又改了 `CHANGELOG.md`、`AGENTS.md`，源码没有动。这三份文件没有哪段代码去读：照检查点 3 的办法又 `git grep` 了一遍 `src`、`tests`、`examples`、`benches`、`build.rs`、`pid-plot`、`.github/scripts`，提到它们的 51 处都在注释里。日志在 `target/publish-audit/review/gates/cp4/`：

| 门禁 | 退出码 | 用时 | 读数 |
|---|---|---|---|
| `cargo build --locked --workspace --all-targets` | 0 | 117 s | |
| `cargo test --locked --workspace --all-targets` | 0 | 1 626 s | 475 个测试程序；2 582 通过，0 失败，3 忽略；`skipping: ` 提示 21 行 |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | 0 | 13 s | 0 条警告 |
| `cargo fmt --all -- --check` | 0 | 5 s | |
| `bash .github/scripts/check-missing-docs.sh` | 0 | 8 s | `current=0, baseline=0` |
| `cargo test --locked -p pid-parse --no-default-features --lib` | 0 | 1 s | 985 通过 |
| `cargo check --lib --locked -p pid-parse --no-default-features --target wasm32-unknown-unknown` | 0 | 0 s | |
| `bash .github/scripts/check-byte-audit-baselines.sh` | 0 | 19 s | 3 份基线都比了，各 `Regressions: 0`，跳过 0 |

与检查点 3 比，通过的测试多 27 条（2 555 → 2 582），就是阶段 4 加的那 27 条：库内 22 条（1 563 → 1 585），`pid_publish_xml` 的单测 1 条（4 → 5），`tests/publish_from_pid.rs` 3 条（19 → 22），`tests/publish_xml_cli.rs` 1 条（36 → 37）。测试程序还是 475 个：T4.5 量读数用的临时 example 量完就删了，没有进提交。其中一个是不打结果行的基准程序 `benches/pid_pipeline.rs`，所以日志里的 `test result:` 是 474 行，检查点 3 那次也是。忽略数、`skipping: ` 行数、缺文档数、不带默认特性的 985 条都没变。

**vendored 件。** 阶段 4 没有动 `vendor/oxidized-mdf`，也没有动 `Cargo.toml`、`Cargo.lock`：`git diff --stat 9c09065..b7eeaf7` 里是 `src` 下 8 个文件和 `tests` 下 3 个，+2 221／−168 行。它自己的测试这次没有重跑，读数还是检查点 3 的。

**对照矩阵。** 门禁全过之后接着跑 `cp-matrix.ps1 -Label cp4`（`runs/cp4-matrix.log`）。这时工作区有三份文档没提交，源码与 `b7eeaf7` 一致。矩阵 61 项，95 s：

- `compare.ps1 -A t46 -B cp4`：717 个文件全部相同。
- `compare.ps1 -A cp3 -B cp4`：714 个相同，3 个不同，没有只在一边的。不同的是 `help\stdout.bin`（114 → 127 行，6 944 → 7 594 字节）、`no-args\stderr.bin`（115 → 128 行，7 040 → 7 690 字节）和 `summary.txt`（只有这两项各自那一行的字节数）。多出的 13 行是 T4.3 加的那两段用法说明，在前一个文件里是第 28–40 行，在后一个里是第 29–41 行；去掉之后和 `cp3` 的逐行相同。

各任务之间是一环一环比下来的：`cp3` → `t41`（717 个相同）→ `t42`（717）→ `t43b`（714，差的就是上面这 3 个）→ `t44`（717）→ `t45`（717）→ `t46`（717）→ `cp4`（717）。

检查点 4 的第二项写的是「对照矩阵逐字节不变」，照字面这一阶段做不到：T4.3 的「做什么」要往用法里加话，而 `--help` 和不带参数这两项自己就在矩阵里。实际是：各项发布写出的文件，和它们的 stdout、stderr、退出码，逐字节没变；变的只有用法说明。§6 里那一项照这个意思勾的，括号里写明了。

**发布构建的哈希。** 这次矩阵用的 `pid_publish_xml-b7eeaf7-release.exe` 是 4 756 992 字节，SHA-256 `f6099763…87974b30`，和 T4.6 量矩阵、量 `-v` 时的那一份（同名，同样大小，`5d311c3b…856df472`）不是同一份。T4.6 的故意改坏把源文件改过又 `git checkout` 回来，修改时间变了，cargo 重编了一遍（39 s），原来那一份被同名覆盖，没法再拿来比。

为了知道同一份源码两次构建差在哪里，又强制重编了一次（`cargo clean --release -p pid-parse` 再构建，`runs/cp4-rebuild-check.log`，得到的程序 SHA-256 `1c782ccb…ab93027c`），和这次矩阵用的那一份逐字节比：4 756 992 个字节里只有 4 个不同，在 `0x108`–`0x109` 和 `0x406e44`–`0x406e45`。两处记的是同一个链接时间戳：前一处是 PE 头的 `TimeDateStamp`，后一处前后的字段对得上调试目录的一条；两份程序里两处的值各自相等，分别是 2026-11-16 09:07:14Z 和 09:11:22Z。所以在这台机器上，同一个提交每重链接一次，程序的哈希就变一次，内容差的只有时间戳。T4.6 那一份和这一份之间差的是不是也只有这个，没法直接验（文件没了）；两份跑出的矩阵 717 个文件全部相同。强制重编的那一份在 `target/release/` 下，没有拿来跑任何东西。

**读数。** 这一阶段只有 T4.5 有性能读数，都在那一节：`ExpTables::read` 2.018–2.597 s → 0.275–0.294 s；整包两版交替各跑四次，中位数 15.62 → 14.16 s（`Export.dmp` 74 489 856 字节，53 张图）。检查点这里没有重量。

**说明。** `CHANGELOG.md` 的 `[Unreleased]` 加了阶段 4 一条：六个任务各一段，加公有接口、行为上的差别、输出的变化、用例、没有改的、没有验到的。`AGENTS.md` 改了八处：架构表三行（exp 增补、整包、数据库线）各补一句现在的做法；用例表里 `tests/publish_xml_cli.rs` 一行加上 `-v` 那条用例和它的夹具，`tests/publish_from_pid.rs` 一行加上写文件的三条用例和夹具的说明，另加一行讲阶段 4 的 23 条单测（库内 22 条，可执行文件 1 条）。两处的数字和名字对着上面各节的记录和代码过了一遍。

**等你定的。** 各任务「没有改的」里要你拿主意的，收在一处：

1. 整包里把一个 UID 写了两次的图，现在只点名，退出码不变，`--strict` 不管（T4.2）。要不要让它退出 1，或者归 `--strict` 管。
2. T4.3 是三个提交（`9d24529`、`ecfc5c4`、`b69ce84`），计划是一个任务一个。单独检出前两个会看到用法里那句不对的「至多留下一个 `.tmp` 文件」。要合成一个得改写历史，没有做。
3. 单图模式的 `--out-dir` 三个文件不是「都有或都没有」；运行开始时不删旧的 `_Index.json`；这一轮失败的图不清上一轮留下的文件（T4.3）。计划只要求了整包的三件套。
4. `.pid` 条目的上限 512 MiB 是计划里暂定的数，不能从命令行调；别处解压 zip 条目的三处没有设上限（T4.4）。
5. 不带 `-v` 时库里缺子表仍然无声，这一类对象少一截属性；它不算 T2 的「写得不完整」，`--strict` 不管（T4.6）。仓库里的 `Export.mdf` 自己就缺 `T_ProcessEquipment`，算进去的话 A01 每次都会告警。
6. 用法里 `-v` 的说明没有跟着改，留给 T6.2（T4.6）。

**T5.1 的前提。** 计划要「另一条线对 `run_pid` 的改动先进 `main`」再做 T5.1（§8 第一行）。现在：

- `main` 还在 `a4b490e`；刚 `git fetch origin main` 过，`origin/main` 也还在 `a4b490e`。这条分支比它多 24 个提交（记这一节的这个提交不算），少 0 个。
- 那条线已经提交了，在分支 `export-closeout-20261115` 上（主检出目录正检出着它，`d329c2f`，比 `main` 多 11 个提交），没有并进 `main`。其中 `f4c7763` 动了 T5.1 要改的那一段：`run_pid` 里 `oracle-exp stored values` 那一句 `eprintln!` 多报一个数（4 行），另改了 `stored_values.rs`（434 行）。
- 拿 `git merge-tree --write-tree` 把那条分支和这条分支试并了一次（只往对象库里写了一棵树，工作区、索引、引用都没动）：冲突只有 `CHANGELOG.md` 一个文件，`src/bin/pid_publish_xml.rs` 和别的源文件都自动合并。试并时这条分支还没有做 T5.1。

T5.2、T6.1–T6.4 在计划里都排在 T5.1 后面。是等那条线进 `main`，还是不等，等你定。

**顺带记下的。**

1. 这次门禁比检查点 3 慢：构建 117 s 对 87 s，测试 1 626 s 对 1 583 s，clippy 13 s 对 1 s，缺文档 8 s 对 6 s，byte-audit 19 s 对 15 s。原因没有查。可能有关的两件事：T4.5 记过这台机器不同时段快慢差得不少；T4.6 的故意改坏动过源文件的修改时间，构建和 clippy 都得重做。
2. 发布构建的哈希每重链接一次就变（上面那一段）。记录里的程序哈希只能说明是哪一次构建，不能拿两次构建的哈希相不相同去判断源码变没变；要比的是提交号和跑出来的东西。
3. 门禁的 `summary.txt` 开头记的是 `dirty files: 1`，矩阵的日志记的是 `dirty files: 3`：两个时刻之间加的是 `CHANGELOG.md` 和 `AGENTS.md`。
