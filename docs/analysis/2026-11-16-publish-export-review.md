# publish 导出实现的审核结论（2026-11-16）

> 范围：`src/publish/*`（约 3 万行）与命令行 `src/bin/pid_publish_xml.rs`。只读审核，2026-11-15 做，没有改源码。
> 提交：审核在 `d5e3a0a` 上做（`main` `a4b490e` 加 6 个 pid-plot 提交）。`src/publish/`、`src/bin/pid_publish_xml.rs`、`vendor/oxidized-mdf/` 三处与 `a4b490e` 没有差异，文中行号以 `a4b490e` 为准。
> 去向：整改计划是 `docs/plans/2026-11-16-publish-review-followup.md`，每条结论末尾写着它对应的任务号。
> 标「已核实」的是亲自跑到或读到的；标「推断」的是读代码得出、没有量过的。

## 一句话

没有发现会让 `.pid` + exp 这条线写错内容的缺陷，只有一处修复接错了入口（8.3 短文件名）。问题集中在数据库线（`Export.mdf` 与 SQL Server 备份包）：写不出来的内容静默降级，读大库的方式有规模风险。其余是健壮性与可读性上的建议。

## 1. 读数（已核实）

| 项 | 读数 | 怎么得到的 |
|---|---|---|
| clippy | 0 条告警 | `cargo clippy --locked --offline -p pid-parse --all-targets -- -D warnings` |
| 库测 | 420 过，0 败 | `cargo test --locked -p pid-parse --lib publish::` |
| 集成测试 | 17 个 `tests/publish_*.rs`：133 过，0 败，2 ignored | 逐个文件跑 |
| 8.3 短名 | 长名退出 0、`different 0`；同一文件的短路径退出 1、`different 1`，图名写成 `IB031~1` | 见 §2.1 的两条命令 |
| A01 数据库线 | 占位对象 0，兜底关系 0，关系 15 条 | `Export.mdf`（19.9 MB）`--drawing … --stdout` |
| 已交付的 `publish-v6` | 53 个 `_Data.xml`、186 993 个 UID，重复 0 个 | 逐文件数 `IObject UID` |

前三行用的是本机默认工具链（nightly 1.101）。依赖外部语料的用例在语料缺失时会打印原因后跳过并记为通过，这次没有数跳过了多少；全量测试没有跑，只跑了 publish 相关的部分。

A01 的 15 条关系按 `DefUID`：`DwgRepresentationComposition` 4、`DrawingItems` 4、`PipingPortComposition` 2、`EquipmentComponentComposition` 1、`PipingConnectors` 1、`ProcessPointCollection` 1、`PipingEnd1Conn` 1、`PipingEnd2Conn` 1。

## 2. 应修

### 2.1 8.3 短文件名的修复接在了命令行不调用的函数上（结论 1，已核实）

- `src/publish/pid_load.rs:831–832` 的 `load_publish_from_pid` 先 `canonicalize` 再取图名；`:992` 的 `read_publish_from_pid` 没有。命令行的 `run_pid` 调的是后者（`src/bin/pid_publish_xml.rs:646`），前者只剩示例、库内单测和 `tests/publish_from_pid.rs` 在用。
- 用短路径调用时，`IObject.Name`、`<Container>` 的 `DocName` 和 `--out-dir` 下的输出文件名都变成 `IB031~1` 这样的别名。
- 来由：`838a556` 把命令行切到 `read_publish_from_pid`；`c0f191c` 合入旧 stash 时把 `canonicalize` 只加在了老入口上。
- 实测（`target/debug/pid_publish_xml.exe`，语料在 `D:\work\cad\PID自研软件验证`，两条的 `--diff-against` 指向同一份 `I井场 注采阀组工艺及自控流程图_Data.xml`，`--plant P01`）：

```text
输入 <语料>\I井场 注采阀组工艺及自控流程图.pid
  -> 退出 0   values 1530 (different 0, not in reference 0)

输入 D:\work\cad\PID~1\IB031~1.PID        （同一个文件的短路径）
  -> 退出 1   values 1530 (different 1, not in reference 0)
     container DocName: "IB031~1" reference "I井场 注采阀组工艺及自控流程图"
     value PIDDrawing 5633AF41… IObject.Name: "IB031~1" reference "I井场 注采阀组工艺及自控流程图"
```

- `docs/baselines/publish-diff-accepted.json:323` 写的是短名「now resolves to the stored long name … the run is clean」，与第二条不符。`tests/publish_diff_accepted_baseline.rs:791` 的 `the_ijingchang_run_never_leaves_the_registered_item` 因此只能同时接受退出 0 和 1（注释在 `:827–828`）。
- 修法：`load_publish_from_pid` 改成返回 `read_publish_from_pid(path)?.drawing`，`canonicalize` 只留在 `read_publish_from_pid` 一处。
- 审核当时建议把那条用例收紧成只接受一种结果。`docs/baselines/README.md:123–129` 明文写着这条断言是析取、不要写成必须 clean，所以收不收紧留给计划里的 Q2 定。
- 去向：T1.1、T1.2。

### 2.2 数据库线在覆盖不到的地方静默降级（结论 2，已核实；既有问题，不是近期改动引入的）

- 目录表里 `Exchanger`（`src/publish/catalog.rs:334`）、`Mechanical`（`:347`）、`EquipmentOther`（`:386`）、`OPC`（`:398`）四行的 `db_writer` 是 `None`。它们和目录里没有的任何类型一样，落到 `"*"` 行（`:414`）的通用写手 `write_generic_object`（`src/publish/xml_writer.rs:1945`），写成只带 UID 的 `<PIDItem>` 外加一条 XML 注释。判定写在 `write_business_objects` 的 `.or_else` 里（`xml_writer.rs:164–173`）。同样的类型在 `.pid` 线能写成 `PIDHeatTransferEquipment` / `PIDMechanicalEquipment`。审核当时只点了前两类，写计划时核对目录表补上了后两类。
- `classify_relationship`（`xml_writer.rs:2591–2599`）：`T_Relationship` 里认不出的端点组合写成 `DefUID="Relationship"`，这不是 SmartPlant 的关系定义；`(PipeRun, Nozzle)` 一律写成 `PipingEnd1Conn`。
- 发生这些情况时退出码是 0，stderr 不提示，`write_database_coverage_json`（`src/publish/pid_writer.rs:784`）和 `_Index.json` 里都没有计数，唯一的痕迹是 `_Data.xml` 里那条注释。带 `--diff-against` 时门禁会把占位报成差异；不带参考时才是静默的。
- 仓库里唯一的数据库线夹具 A01 一处都不触发（§1），所以这条路现在没有被任何用例走到。`src/publish/mod.rs` 写明数据库线只在 A01 上核对过：换一张带换热器或泵的图，得到的是一份看起来成功、实际缺内容的导出。
- 修法：写出时按类型统计占位对象和兜底关系，汇总打到 stderr，并写进 `_Coverage.json` 和 `_Index.json` 的 `problems`；再加一个严格开关。补齐专用写手要有原厂参考，放在后面；先让降级看得见。
- 去向：T2.1–T2.3。

## 3. 建议优先处理

### 3.1 读 MDF 的方式撑不住工厂级的库（结论 3，推断）

- `src/publish/mdf_load.rs:62` 对 `PUBLISH_TABLES`（`:20`，24 张表）逐张 `MdfDatabase::open(path)`，每次都重新解析系统表，再从文件头往后读。
- 仓库自带的读取器只能向前读，读过的每一页都留在 `page_cache` 里不淘汰，每页解析两遍（`vendor/oxidized-mdf/src/lib.rs:459–522`）。读一张位置靠后的表，等于把它之前的整个文件放进内存。
- SQL Server 备份包那条路上还要叠加两份：整个 `Export.dmp` 读进内存（`src/publish/backup_package.rs:1106`），`mdf_bytes_from_mtf_dump` 又 `to_vec` 复制一份 MDF 镜像（`mdf_load.rs:112`）。峰值最多约为库大小的三倍。
- 仓库里最大的 MDF 只有 19.9 MB，调试构建打开一次约 1.4 秒（已核实），测试看不出来。
- 修法：读取器改成对 `File` 按 `page_id × 8192` 定位读取，缓存设上限，整个库只打开一次；临时文件直接从 dump 的切片写出；暂存的插入包进事务。先量再改。
- 去向：T3.1–T3.3。

## 4. 建议

### 4.1 exp 增补的编排写了两份（结论 4，已核实）

命令行的 `run_pid`（`src/bin/pid_publish_xml.rs:658–790`）和整包的 `attach_exp`（`src/publish/backup_package.rs:872–900`）各自编排同一串步骤，`opc_points.retain(...)` 逐字重复（`:711` 与 `:888`）。现在两边顺序一致；以后加一步容易只改一边，单图和整包的输出就会不一样，而且这段业务逻辑放在了 bin 里。建议库里提供一个函数，两边共用。去向：T5.1。

### 4.2 `attach_dump_only_components` 不对本批结果去重（结论 5，已核实）

`src/publish/oracle_exp.rs:1386–1410`：`known` 在循环前建好，循环里不更新（`:1403` 是 `!known.contains(&item.uid)`）。同一个 `PipingComp` 在这张图上有两条未绘制的表示时，会被加入两次，写出两个同 UID 的对象。已交付的 53 张里没有重复的 UID（§1），所以现有语料没有触发，也没有用例覆盖。分支 `publish-stash-20261108`（`9b4d60b`）里已经有改法（`known.insert(item.uid.clone())`）和用例 `a_component_placed_twice_is_published_once`，没有并进 `main`。另外，整包校验用的 `pid_publish_counts`（`src/publish/diff.rs:1541`）读文档时已经知道哪些 UID 重复，却没有带出来。去向：T4.1、T4.2。

### 4.3 写文件不是原子的（结论 6，已核实）

`write_three`（`src/publish/backup_package.rs:914–940`）和命令行的 `write_file`（`src/bin/pid_publish_xml.rs:1160`）都直接 `fs::write`：三件套写到一半失败会留下孤儿文件，而索引里这张图是 `files: []`；`--out-dir` 也不清理上一次留下的文件。建议先写临时文件再改名，并在用法里写明以 `_Index.json` 为准。去向：T4.3。

### 4.4 大输入的内存和扫描（结论 7，已核实代码，用时没有量）

zip 条目 `read_to_end` 没有上限（`src/publish/backup_package.rs:474`）。dump 整个读进内存（`src/bin/pid_publish_xml.rs:660`、`backup_package.rs:374`）。`find_slice`（`src/publish/oracle_exp.rs:1866`）是朴素扫描，`ExpTables::read` 的 19 张表每张都从头扫一遍 dump，6 张可选表还要先查一遍再读一遍（`:2435`）。建议给 zip 条目设上限，表定位改成一次扫描建索引（先量它占多少用时）。dump 改成流式要重写读取器，这一轮不做。去向：T4.4、T4.5。

### 4.5 `publish_one` 难读（结论 8，已核实）

`src/publish/backup_package.rs:494–731`：`uid_from` 用字符串字面量当枚举，Oracle 分支写 `T_DRAWING.SP_ID`，MTF 分支写 `T_Drawing.SP_ID`（`:1460`）；失败记账的那几行重复了约 8 次；`PublishTally` 是 24 个 `&mut` 字段。建议 `uid_from` 改成枚举，失败路径收成一个函数。去向：T5.2。

### 4.6 两份手写的表清单没有一致性检查（结论 9，已核实）

`PUBLISH_TABLES`（`src/publish/mdf_load.rs:20`）和目录表的 `db_subtables`（`src/publish/catalog.rs`）是两份手工清单，目前一致，但没有用例保证。漏登记的表会被 `prepare_optional`（`src/publish/sqlite_load.rs:328`）当成表不存在而跳过，结果是字段悄悄缺失。建议加一条一致性用例。去向：T4.6。

## 5. 细节

- 报告序列化失败时静默写出空内容：`src/publish/backup_package.rs:347`、`:1346` 回退成 `"{}"`，`src/publish/pid_writer.rs:764`、`:807` 用 `unwrap_or_default()`。（T6.1）
- `run_pid` 里 `_Meta.xml` 最多渲染三次（`src/bin/pid_publish_xml.rs:816`、`:824`、`:844`），参考的 `_Data.xml` 读两次。（T6.2）
- `emit_data` 用 `print!`（`pid_publish_xml.rs:866`，另有 `:941`、`:1061`），下游管道关闭时会 panic。（T6.2）
- `parse_args`（`pid_publish_xml.rs:196`）要求输入路径是第一个参数，先写选项会得到 “unknown flag” 这种误导的报错。（T6.2）
- 用法里 “without it the input still exits 2”（`pid_publish_xml.rs:121`）对命令行不可达，因为这个 bin 本身要求 `backup` 特性。（T2.2）
- `pipeline_uid`（`src/publish/pipeline.rs:203`）对常量字符串 `parse_str` 并回退到全零 UUID，可以改成编译期常量。（T6.3）
- 覆盖报告用 `list[0] = ...` 按位置改写（`src/publish/pid_writer.rs:660`）。（T6.3）
- `fmt_err`（`src/publish/xml_writer.rs:2020`）把格式化错误包成 `PublishError::Sqlite`。`PublishError`（`src/publish/model.rs:112`）不是 `non_exhaustive`，加变体对外是破坏性改动。（T6.3）
- `_Index.json` 的 `failed` 把打不开的 zip 和条目也算在内，从字段名看不出来。（T6.3）
- `_Index.json` 和 `_Coverage.json` 写入本机绝对路径（`package`、`source`、`dump`）。（不在这一轮：一改，交付文件的字节就变）
- `catalog.rs` 的 `provisional`、`dead` 字段和 `publish_item_catalog()` 挂着 `allow(dead_code)`，没有调用方。
- 比对输出中英混排（`src/publish/diff.rs:1424`）。
- `xml_writer.rs` 6 143 行、`oracle_exp.rs` 5 492 行、`diff.rs` 4 015 行，生产代码和测试在同一个文件里。

## 6. 知悉

- 合成管线的 UID 由 `--plant`（默认 `P01`）和管线名算出，plant 传得不同 UID 就不同；同名管线的 `fluid_code` / `fluid_system` 取第一条 run 的值，不检查冲突（`src/publish/pipeline.rs:47`）。
- 数据库线的 `-CNX` 连接件 UID、关系的 `IObject UID`、`GraphicOID` 仍是登记在案的占位值。
- `src/publish/diff.rs:1239` 的允许量表把具体图纸的 DocUID 和图名写在库代码里；这个 crate 若对外发布，建议挪到数据文件或测试一侧。

## 7. 做得好的地方

- `escape_attr`（`src/publish/xml_writer.rs:2655`）覆盖完整：五个实体、CR/LF/TAB、非法控制字符；占位注释里的 `--` 也处理了。
- `_Meta.xml` 的 UID 用 UUID v5 生成，不带时间戳，重复导出结果稳定。
- 整包模式对输出文件名做了清洗和不区分大小写的重名检测，生成的 XML 会再读一遍校验。
- 断链会在 stderr 警告并写进 `_Index.json`；别的版本的 dump 会被拒绝。
- `oracle_exp.rs` 的解析全程做边界检查，坏数据返回错误而不是崩溃，另有 `tests/parser_panic_safety.rs`。
- SQL 全部参数绑定；临时 `.mdf` 用 `create_new` 创建并在 `Drop` 里删除。
- codelist / stored / unit 三个补值模块都不覆盖已有的值，`stored_values` 还统计了与标签一致和不一致的数量。
- 比对门禁双向比较对象、值、接口、关系和 `<Container>` 属性，并检查重复 UID。

## 8. 核对后排除的（不是问题）

- `--out-dir` 路径穿越：单文件模式的名字来自输入文件名，整包模式有 `output_stem` 清洗和用例。
- bin 无条件使用 `backup-mdf` 的内容：`Cargo.toml` 里它声明了 `required-features = ["backup"]`。
- 管线端点为空串写出 `UID2=""`：`attach_pipe_endpoint_connections` 只写非空值。
- 同一属性写两次：三个补值模块都先查重。
- `IOffpageConnector` 空值：有 `IfValue` 规则和用例。

## 9. 没有覆盖到的

- 只抽查、没有逐行读：`xml_writer.rs` 中段各类型的写手（465–1935 行，只核对了转义）、`oracle_exp.rs` 的各个投影函数、`diff.rs` 门禁核心以外的部分、`catalog.rs` 的接口表、`codelist_values.rs` 的规则表。
- 没有读：`placement.rs`、`catalog_index.rs`、`completeness.rs`、`diff_exclusions.rs`、`model.rs`、`unit_values.rs` 的主体，各文件的测试段，`docs/pid-export-bundle-contract.md`。
- 没有做：大库上的性能实测；全量测试。
