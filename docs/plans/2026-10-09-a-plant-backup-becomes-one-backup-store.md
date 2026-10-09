# 一套 Plant Backup 生成一个 Backup Store · 第一版计划（2026-10-09 开单，同日批准）

> 起因：用户 2026-10-08「使用 grill-with-docs 制定对备份文件的解析计划，可以将相关的数据都存储到 sqlite 里」。三轮问答 Q1–Q21 全部按推荐定案；
> 术语已写进 `CONTEXT.md`（Plant Backup、Database Dump、Option Archive、Backup Store、Schema Role、Ghost Row），难回退的形状写进
> `docs/adr/0004-backup-store-one-file-per-backup-named-by-schema-role.md`，格式事实见 `docs/analysis/2026-10-08-sppid-backup-package-format-cn.md`。
> 开单时没改代码。「实现层决策」P1–P9 是问答没覆盖、动手前必须定的细节，⭕ 为推荐；批了再从 S0 开工。
>
> **2026-10-09 用户批准**：「P1–P9 全部按推荐，Q2 的理解也对，开工 S0」。各步进度见「门禁记录」。
>
> **2026-10-09 补充，同日批准**：审计发现 `D:\work\cad\pid-test-data`（Plant `SQPlant`，Oracle）不在计划里，用户要求补进来，作为第三套样本（第二套 Oracle）。
> 改动：事实表加 SQPlant；新增 P10–P12；S2a 存 zip 条目名原字节并按 GBK 解码；S2d 按 `CONNECT` 标记把表归到 owner；验收第 11 条写明按 owner 计，新增第 13 条。
> 用户：「P10–P12 按推荐，改动稿照落」。

## 一句话

新增 `pid_backup_store <Plant Backup> -o <store.sqlite>`，把一套 Plant Backup 的 Manifest、Database Dump 的 154 张表和包内文件清单写进一个 SQLite。
SQL Server 备份逐行解码，每行带 MDF 的页号和槽号；Oracle 备份（DWG、SQPlant）第一版只建同名空表。为此先修 vendored `oxidized-mdf` 的五处读错
（按 schema 认表、滤掉 Ghost Row、跟着 LOB 指针读值、分开空串和 NULL、给出页和槽），最后让 publish 改读 store，用 A01 的现有测试把关。

## 已定决策（Q1–Q21）

| 题 | 定案 | 落在 |
|---|---|---|
| Q1 | 读者是 publish 管线和人 / 脚本；面向应用的模型以后用视图或二次导出做 | S4 |
| Q2 | 第一版收 Manifest 全部行、154 张表、包内文件清单；文件原始字节做成开关、默认关；`.pid` 解析结果放第二版，参考数据解析放第三版 | S2、S3 |
| Q3 | 第一版只解码 SQL Server 的行；Oracle 只登记 Manifest 和文件清单，表留空并标「未解码」；exp 行解码排第二版 | S2 |
| Q4 | 一套备份一个 SQLite 文件，跨备份用 `ATTACH` | ADR-0004 |
| Q5 | Plant Backup、Database Dump、Option Archive、Backup Store 四个术语 | `CONTEXT.md` |
| Q6 | 沿用 ADR-0001 证据分级：确认了的字段才起名，其余按 key、位置、原值存并标等级；每行可追溯到来源文件 | S2 |
| Q7 | 目录和 zip 都收，每个文件记 SHA-256；目录输入提示「文本文件的换行可能被改过」 | S2 |
| Q8 | 表名 `<角色>__<表名>`，角色 `plant` / `plantd` / `pid` / `pidd` 来自 ConnInfo 类型码 2 / 8 / 4 / 9；另建 `dump_schema` | S2 |
| Q9 | 按源类型存：nvarchar→TEXT（空串和 NULL 分开）、int→INTEGER、float→REAL、datetime→TEXT `YYYY-MM-DD HH:MM:SS.fff`、image→BLOB（LOB 原始字节）；`dump_column` 记源类型、长度、精度、可空 | S1、S2 |
| Q10 | 读取器的修复直接改 vendored `oxidized-mdf` | S1 |
| Q11 | 每张转储表加 `_src_page`、`_src_slot`；LOB 值另记根页和槽 | S1、S2 |
| Q12 | Ghost Row 原样进 `dump_ghost_row`，不解成列 | S1、S2 |
| Q13 | publish 在第一版最后一步改读 store，A01 把关；去掉 Ghost Row 后输出若变，先分析再接受 | S1、S4 |
| Q14 | `manifest_line` + `manifest_field`，语义确认了的做成视图；`backup_file` 记路径、大小、SHA-256、格式、所在容器；zip 只展开一层 | S2 |
| Q15 | 默认脱敏，加 `--keep-secrets` 才原样存 | S2、S3 |
| Q16 | Oracle：把扫 DDL 的能力挪进库，建同名空表，`dump_table` 标「未解码」 | S2 |
| Q17 | `pid_backup_store <输入> -o <输出>`，`-o` 必填，输出已存在时拒绝，除非 `--force` | S3 |
| Q18 | publish 的选择表取 pidd，第一版只换来源、不改连接；连接缺陷单独登记 | S4 |
| Q19 | Oracle 类型映射（见「库的形状」） | S2 |
| Q20 | Oracle 的大写名字原样保留 | S2 |
| Q21 | 验收 13 条（见「验收」；第 13 条 2026-10-09 补） | S1–S5 |
| 自定 | 只收 Manifest 列出的 154 张表，`sys` 下 2 张内部表不收；35 个视图只登记名字 | S2 |

## 事实（2026-10-09，pid-parse `fa4e268`）

| 项 | 数 / 位置 | 出处 |
|---|---|---|
| TEST02 的 schema | nsid 5 = TEST02d（25 张）、6 = TEST02（22）、7 = TEST02pidd（25）、8 = TEST02pid（82），合计 154；另有 `sys` 下 2 张内部表 | `sysschobjs` + `sysclsobjs` class 50 |
| TEST02 行数 | 存活行合计 37,470，逐表等于 `sysrowsets.rcrows`；33 个多页分配单元全是 B 树叶链，没有多页堆表 | 只读解析 `Export.mdf` |
| TEST02 列 | 1,798 列：nvarchar 856、int 650、float 263、datetime 27、image 2 | `syscolpars` |
| Ghost Row | TEST02pid 的 `T_PlantItem`、`T_Equipment`、`T_EquipmentOther`、`T_SmartFrameStorage`、`T_Symbol` 各 1 行；前两张 publish 会读 | 逐页逐槽统计 |
| LOB | `T_DrawingVersion.SP_Storage` 3 个值（18 个 LOB 页）、`T_SmartFrameStorage.SP_Storage` 1 个值，都是 ZIP，第一个条目分别是 `Drawing.xml`、`A01-JSite204.tmp` | 同上 |
| 空串 / NULL | 空串 162 个，分布在 10 张表（`T_Drawing` 4 个，在 `Description` / `Revision` / `Title` / `Version`，publish 不读这四列）；NULL 按列类型计：nvarchar 21,431、int 29,009、float 148、datetime 8，合计 50,596，全是空位图置位，没有「记录没存的列」（154 张表没有短记录）。本文说「NULL 21,431」指 nvarchar 的数。1,798 列里 206 列声明 NOT NULL（`syscolpars.status` 位 1 置位），其中只有 `T_Symbol.SP_ID` 有 1 个 NULL——页 2300 槽 1 那条记录的空位图把它置位了，变长区却存着 32 字符的 `AC9DFB6629974E428402C938E60F4B9C`，定长区是指针模样的字节 | 同上；S1c 复核（2026-10-09）；S2c（2026-10-09） |
| 读取器：按名取表 | `BaseTableData::table` 取第一张同名表；`Sysschobj.nsid` 读了没用；`sysclsobjs` 没读 | `vendor/oxidized-mdf/src/sys.rs:141, 336` |
| 读取器：槽 | `Page::slots` 把槽偏移排序，槽号丢了；记录结尾取下一个偏移，最后一条取页尾 | `pages.rs:795–866` |
| 读取器：Ghost Row | 记录类型 5 / 6 / 7 认出来了，但 `Page::records` 没有过滤 | `pages.rs:41–96, 830` |
| 读取器：空串 | `parse_string` 把零长度当 NULL，留着 TODO | `pages.rs:543–560` |
| 读取器：image | 走 `parse_binary`，拿到的是行内 16 字节文本指针 | `lib.rs:431` |
| 读取器：只能往前读 | `PageReader` 读过的页不能回头，跟 LOB 指针要随机访问；publish 因此每张表重开一次文件 | `lib.rs:490–507`、`src/publish/mdf_load.rs:62` |
| 读取器：datetime | 1/300 秒刻度用 `as i64` 截断成毫秒，刻度除 3 余 2 时少 1 毫秒（SQL Server 显示 .007，这里得 .006） | `pages.rs:294–311` |
| publish 暂存 | 26 张表全存 TEXT；datetime 写成 `2026/4/28 14:37:31`（丢毫秒）；二进制写成大写十六进制；浮点用 Rust 的 `Display` | `src/publish/mdf_load.rs` |
| publish 的选择表 | 现在读到 d（13 个选择表、130 行，属性 80 行）；pidd 有 127 个选择表、3,206 行、798 个属性。代码把 `attribute_codelisted`（实为 `T` / `F`）当选择表编号，在真实数据上从没连上过；真连接是 `attribute_datatype = C<n>` | `src/publish/sqlite_load.rs:357–400` |
| publish 的调用点 | `src/bin/pid_publish_xml.rs:414`、`src/export_bundle.rs:942`、`tests/common/mod.rs`、`tests/publish_mdf_load.rs`、`tests/publish_a01_raw_residual.rs`、`benches/pid_pipeline.rs`、`examples/publish_walkthrough.rs` | `rg "open_mdf_as_sqlite\|load_drawing_graph_from_mdf"` |
| MTF → MDF | 跳过 MSDA 开头那段（默认 0x3F0）的探测写在 bin 里，库里没有 | `src/bin/pid_backup_extract.rs:320` |
| Manifest 解析 | `parse_line` 去掉 `\r`、修剪 key，拼不回原文 | `src/backup/manifest.rs:120` |
| zip | `<Plant>_p.zip` 根下 14 个条目，没有外层目录；本仓至今只读 zip 的中央目录、不解压（`Cargo.toml` 注释） | .NET `ZipFile` 列目录 |
| 样本 | `test-file/backup-test/TEST02_p.zip`（24,368,835 字节）、`DWG-0202GP06-01_p.zip`（12,474,311 字节），都在 git 里；目录副本里的文本文件被 `.gitattributes` 改成了 LF | `git ls-files` |
| Oracle（DWG） | 154 张表 2,039 列（按 owner：plant 22 张 138 列、d 25 / 176、pid 82 / 1,549、pidd 25 / 176），6 种类型；表名列名全大写；空串存成 NULL；比 TEST02 多 241 列 | 按 `CONNECT` 归属扫 `Export.dmp` 的 DDL |
| Oracle DDL 示例 | 开单时 `examples/oracle_exp_schema.rs` 以表名为键存 `BTreeMap`，跨 owner 的同名表互相覆盖（两套字典 schema 共 24 个名字、`MAX_ID` 四个 owner 都有、`SPIDCACHE` 在 plant 和 pid），DWG、SQPlant 都只报 126 张。S2d 后扫描在 `src/backup/oracle_exp.rs`，按 owner 归属，example 调库、按 owner 分组打印，两套都是 154 张 | `examples/oracle_exp_schema.rs`；S2d（2026-10-09） |
| Oracle owner 标记 | 每个 owner 段前有一行 `CONNECT <OWNER>`（SQPlant 36 个），表归属取前面最近的一个 | 扫 `Export.dmp` |
| SQPlant 样本 | `D:\work\cad\pid-test-data`，只有目录、没有 zip 原件，不在 git 里，约 107 MB；16 个顶层文件，文本是原始字节（`RefData~4~680` 331,977、`PlantConfig.xml` 5,085，与 `FileSize` 行 / zip 原件一致） | 目录列表、Manifest |
| SQPlant 库 | Oracle 12.1.0.2 `exp`，`Export.dmp` 74,489,856 字节；154 张表（22 / 25 / 82 / 25）2,024 列（plant 138、d 176、pid 1,534、pidd 176），类型同 DWG（NUMBER 只有 `(10, 0)` / `(11, 0)`，FLOAT 只有 `(126)`）；`Export.log` 365,530 行（pid 324,925，`T_DRAWING` 53）；Manifest 的 schema 名是 `SQPlant` / `SQPlantd` / `SQPlantpid` / `SQPlantpidd`，dump 的 owner 全大写 | 扫 DDL、`Export.log`、Manifest |
| SQPlant Manifest | 322 行，BOM `FF FE`，全 CRLF，末行带换行；`ArchiveFileSize` 168,019,536 等于各 `FileSize` 行原始字节之和；`ExportFileSize` 93,585,407 不等于 dmp 大小（DWG 也不等：14,876,671 对 8,802,304）；`BackupCommand` 带 `system` 明文口令 | Manifest |
| SQPlant zip 条目名 | 711 包 60 个条目都没设 UTF-8 标志，47 个 `.pid` 名含非 ASCII 字节、按 GBK 解码才对（如 `00/00/A井场 注采阀组工艺及自控流程图.pid`）；`zip_index` 取 `entry.name()`，zip crate 对这种名字按 CP437 解；各包条目数 711 60、681 742、682 20、684 10、685 3、804 10、809 4 | 逐条读中央目录、`src/backup/zip_index.rs:95` |
| vendored 测试 | `integration_test.rs:119` 断言 `spg_verein_TST.mdf` 的 `tbl_Mitglied` 第 3 行 `Titel` 为 NULL；`parse_string` 的 TODO 说有一条集成测试依赖「空即 NULL」 | `vendor/oxidized-mdf/tests/integration_test.rs` |
| 工作树 | 分支 `codex/phase32c-bundle-closeout`，几条会话共用；别的会话有未提交改动（4 个文件只差换行，1 个未跟踪的 example） | `git status` |

## 实现层决策（等批；⭕ = 推荐）

| # | 决策 | 结论 |
|---|---|---|
| P1 | 在哪里干 | ⭕ 新开 worktree `D:\work\plant-code\cad\pid-parse-backup-store`，分支 `backup-store-v1`，从 `fa4e268` 起。第一笔提交是文档：`CONTEXT.md` 的 6 个术语、ADR-0004、格式文档、本计划，从主工作树挪过去（都是本任务链自己写的，主树 `CONTEXT.md` 的 diff 只有这些）。理由：主树有别的会话的未提交改动，`target/` 也共用，并行编译会互相等锁。备选：直接在主树当前分支上提交，只 `git add` 明确路径 |
| P2 | 读取器的修复怎么落 | ⭕ S1 就修进现有的读行路径：`rows` / `try_rows` 不再返回 Ghost Row，空串返回 `""`，image 返回 LOB 字节。A01 输出若变，变化落在 S1，原因只有读取器；到 S4 改读 store 时输出应当逐字节不变。按名取表的旧接口保留（仍取第一张），另加按 schema 的新接口给 store 用。备选：旧路径不动、S4 一次切换——A01 一变，就要同时拆读取器修复和 store 切换两种原因 |
| P3 | 读取器新接口 | ⭕ `MdfDatabase::from_bytes(Vec<u8>)`：整个文件进内存（TEST02 约 19.9 MB），页可随机读；`open(path)` 改成读进内存后走同一条路。`user_tables()` 给出 `TableInfo`（对象 ID、schema ID 和名字、表名、列、`rcrows`）；`scan_table(&TableInfo)` 逐条给出页、槽和「活行或 Ghost Row 原始字节」，活行里的 LOB 值附根页和槽，datetime 附原始的天数和刻度 |
| P4 | store 代码放哪 | ⭕ `src/backup/store/`（`mod.rs`、`input.rs`、`manifest.rs`、`mssql.rs`、`oracle.rs`、`redact.rs`），随 `backup` 特性；对外是 `build_backup_store(input, out, &StoreOptions)`，另有写进内存连接的变体给 publish 用 |
| P5 | 证据等级怎么写 | ⭕ 用 `CONTEXT.md` 的词：`decoded`、`typed_audit`、`identified_only`、`unknown`。Manifest 每个 key 每个位置的等级做成常量表写进库（`manifest_field_meaning`），依据是格式文档第 4 节和第 10 节「未确认项」 |
| P6 | publish 只拿到 `Export.mdf` 时，Schema Role 从哪来 | ⭕ 按 schema 名后缀认：`<p>`、`<p>d`、`<p>pid`、`<p>pidd`，`dump_schema.role_source` 记 `schema-name-suffix`（从 Manifest 来的记 `manifest-conninfo`）；认不出恰好四个角色就报错。`pid_publish_xml` 由此收三种输入：Backup Store、Plant Backup（目录或 zip）、`Export.mdf`；旧 `Export_v2.sqlite` 照旧。备选：不再收 `Export.mdf`——几十个测试和现有用法都要改 |
| P7 | publish 怎么读 store | ⭕ 第一版 `sqlite_load` 不动：从 store 取带类型的值，按今天 `value_to_text` 的规则（datetime `%Y/%-m/%-d %H:%M:%S`、大写十六进制、Rust 写浮点）抄进内存里同名的 26 张 TEXT 表。不用 TEMP VIEW：SQLite 把 REAL 250 转成 TEXT 是 `250.0`，Rust 是 `250`，输出会变 |
| P8 | 确定性 | ⭕ 表按 Manifest `Table` 行的顺序建；行按页链顺序、再按槽号插入；`backup_file` 按容器和条目序号排；库里不写生成时间，`store_info` 只记工具名、版本和输入的 SHA-256；先写 `<out>.tmp` 再改名。比对口径是逐表内容，不比文件字节 |
| P9 | 默认脱敏的 store 怎么验 Manifest 拼回 | ⭕ Q21 第 6 条「逐字节相同」在 `--keep-secrets` 的 store 上验；默认 store 验「原文按 `store_redaction` 记下的位置做同样替换后，与拼回结果逐字节相同」，且每条记下的 SHA-256 等于原值的 SHA-256 |
| P10 | SQPlant 样本放哪 | ⭕ 不进 git。测试读环境变量 `PID_PARSE_SQPLANT_BACKUP`，没设时退到 `D:\work\cad\pid-test-data`，都没有就打印 `skip: … is absent` 后跳过（沿用现有测试写法）。备选 A：只认环境变量、不写本机路径——不设就永远不跑。备选 B：拷进 `test-file/backup-test/`——约 107 MB，比现有两套 zip 原件加起来（36.8 MB）还大 |
| P11 | zip 条目名怎么解 | ⭕ `ZipEntry` 加 `name_raw`；`backup_file` 加 `path_raw`（BLOB）和 `path_encoding`。设了 UTF-8 标志按 UTF-8；没设时纯 ASCII 记 `ascii`，否则先严格按 UTF-8 解，失败再按 GBK（`encoding_rs`，本仓已依赖），还有错字节就按 CP437 并记 `cp437`；`path` 存解出的名字。备选：照 zip crate 现状一律 CP437——SQPlant 47 个名字是乱码 |
| P12 | Oracle 的表归哪个 owner、owner 怎么对角色 | ⭕ 每条 `CREATE TABLE` 归到它前面最近的 `CONNECT <OWNER>`，表以 (owner, 表名) 为键；owner 与 ConnInfo 第 4 字段按 ASCII 大小写不敏感比较；`dump_schema.schema_name` 存 dump 里的大写原样（Q20），Manifest 的写法留在 `manifest_field`；对不上恰好四个角色就报错。备选：按 Manifest `Table` 行归属——`Table` 行没有列，还得再和 DDL 对一次，而且同样要处理大小写 |

Q2 的「文件原始字节做成开关」按本计划理解为第一版就带 `--embed-files`、默认关（S3）。

## 库的形状

元数据表的名字不含 `__`；转储表一律是 `<角色>__<源表名>`。

| 表 | 主要列 | 说明 |
|---|---|---|
| `store_info` | key, value | 工具名、版本、输入种类（`zip` / `directory` / `mdf`）、输入 SHA-256、是否脱敏、是否收了文件字节 |
| `backup_file` | id, container_id, entry_index, path, size, sha256, format | 外层 zip 条目或目录里的文件，加各 Option Archive 里的条目（只展开一层）；`format` 用 `backup::refdata::classify_format` |
| `backup_file_content` | file_id, bytes | 只在 `--embed-files` 时写 |
| `manifest_line` | line_no, key, raw_text, terminator | 拼回 = BOM + 按 `line_no` 连接 `raw_text ‖ terminator`，编成 UTF-16LE |
| `manifest_field` | line_no, position, value | 原值，不改写 |
| `manifest_field_meaning` | key, position, name, evidence | `name` 为 NULL 表示语义没确认 |
| 视图 `manifest_file`、`manifest_conn_info`、`manifest_table_entry` 等 | — | 只给 `evidence = 'decoded'` 的位置起列名 |
| `store_redaction` | line_no, position, rule, original_sha256 | Q15；`--keep-secrets` 时为空 |
| `dump_schema` | role, schema_name, type_code, db_type, role_source | Q8、P6 |
| `dump_table` | role, source_name, store_name, decoded, expected_rows, rows, ghost_rows | SQL Server 的 `expected_rows` = `rcrows`；Oracle `decoded = 0`、`expected_rows` 为 NULL |
| `dump_column` | role, table_name, ordinal, name, source_type, length, precision, scale, nullable | Q9、Q19；源类型原样 |
| `dump_view` | role, schema_name, name | 只有名字 |
| `<角色>__<表>` | 源列 + `_src_page`、`_src_slot` | SQL Server 按 Q9；Oracle 按 Q19：NVARCHAR2 / VARCHAR2→TEXT，NUMBER(p,0)→INTEGER，FLOAT(126)→REAL，DATE→TEXT（毫秒补 `000`），BLOB→BLOB，`NOT NULL` 只记在 `dump_column` |
| `dump_ghost_row` | role, table_name, page, slot, record_type, bytes | Q12 |
| `dump_lob` | role, table_name, row_page, row_slot, column_name, root_page, root_slot, length, sha256 | Q11 |

## 步骤

每一步单独能提交，提交只 `git add` 本步碰过的路径；每笔提交前跑 `cargo test`、两种特性组合的 `clippy -D warnings`、`fmt --check` 和 `check-missing-docs.sh`。

### S0 开工作树、落文档（批准后）

- 按 P1 开 worktree 和分支；把四份文档挪过去，一笔 `docs(backup)` 提交；主树里这几处随之还原 / 删除。
- 完成标志：新树 `git status` 干净；主树只剩别的会话的改动。
- 本机 `CARGO_TARGET_DIR` 全局指向 `D:\Rust\target`，光开 worktree 分不开编译目录；新树里跑 cargo 一律先设 `CARGO_TARGET_DIR=D:\Rust\target-backup-store`，P1 说的「不抢 `target/` 的锁」才成立。

### S1 修 `oxidized-mdf`（Q10、P2、P3；3 笔提交）

- **S1a 按槽读、分出 Ghost Row**：槽表按槽号读，不再排序；记录长度按记录自身结构算（定长段、空位图、变长列的尾偏移），不再取下一个偏移；类型 5 / 6 / 7 的记录不进活行，单独给出原始字节；槽表项为 0 的跳过。单测：乱序槽、ghost 数据记录、变长列尾偏移定长度。
- **S1b 按 schema 认表**：读 `sysclsobjs`（class 50）得 schema 名，用上 `nsid`；`TableInfo` 带 `rcrows`；加 `from_bytes`、`user_tables`、`scan_table`。
- **S1c 随机读、LOB、空串、datetime**：页读取改成随机访问；`image` / `text` / `ntext` 跟着 16 字节指针读 LOB 根和数据片段，只解语料里见过的 LOB 结构，别的显式报「未支持」，不猜；零长度且空位图没置位的变长列返回 `""`；datetime 毫秒改成整数换算 `(刻度 × 10 + 1) / 3`，单测钉 .000 / .003 / .007。改空串之前先查 `tbl_Mitglied` 第 3 行 `Titel` 的空位图：置位，那条断言不用动；没置位，说明断言建在旧假设上，改断言并在提交说明里写清。
- 每个改过的 vendored 文件更新顶部的 GPL §5(a) 修改说明。
- 本仓新测试 `tests/backup_mdf_reader_test02.rs`：154 张表逐表活行数等于 `rcrows`、合计 37,470；Ghost Row 5 张表各 1 行；LOB 4 个值都是 ZIP、首条目名对；空串 162、nvarchar NULL 21,431（全部类型 50,596）。
- **publish 的影响（Q13）**：跑 `publish_*` 12 个集成测试。钉了 `T_PlantItem` / `T_Equipment` 行数的断言会各少 1；XML 若有变化，逐处查明是 Ghost Row 还是空串引起，写进提交说明和 CHANGELOG 再接受。

### S2 写 store（P4–P9；4 笔提交）

- **S2a 输入和文件清单**：收目录或 zip；MTF → MDF 的探测从 bin 挪进库，`pid_backup_extract` 改调库；`backup_file` 收外层和各 Option Archive 的条目，逐条 SHA-256；目录输入写进 `store_info` 并打警告（Q7）；`Cargo.toml` 里 zip「只读中央目录」那段注释改写。测试：TEST02 zip 外层 14 个条目；与格式文档第 8 节列出的 `.pid` SHA-256 前缀对上。条目名按 P11 解。SQPlant（P10，目录输入）：顶层 16 个文件；711 包 60 条，47 条按 GBK 解出、名字里没有 U+FFFD，钉两条（`00/00/A井场 注采阀组工艺及自控流程图.pid`、`test/U01/D06.pid`）；其余各包条目数按事实表。
- **S2b Manifest**：保原文的拆行（保留行尾和 BOM）；`manifest_line`、`manifest_field`、`manifest_field_meaning` 和视图。脱敏（Q15）：`DBUids`、`DBPwds` 和 ConnInfo 第 1、5 个字段只留 SHA-256；`BackupCommand` 里的口令段换成 `***`，认不出口令段的格式就整段换掉。测试：两套样本按 P9 的两种口径拼回；默认 store 里按 UTF-8 和 UTF-16LE 都搜不到任何一条原值。SQPlant 的 Manifest 同样按 P9 两种口径拼回；默认库按 UTF-8 和 UTF-16LE 都搜不到 `BackupCommand` 里的口令——口令在测试运行时从原 Manifest 现取，不写进测试源码。
- **S2c SQL Server 转储**：`dump_schema`（ConnInfo 类型码 → 角色，再和 MDF 里的 schema 名对上）、`dump_table`、`dump_column`、`dump_view`、154 张数据表、`dump_ghost_row`、`dump_lob`；值按 Q9 落库。
- **S2d Oracle 转储**：`examples/oracle_exp_schema.rs` 的扫描挪进 `src/backup/oracle_exp.rs`，同时改成按 owner 归属（P12），不再按表名去重；example 改成调库、按 owner 分组打印。按 Q19 / Q20 建空表，`decoded = 0`。测试：DWG 154 张 / 2,039 列（138 / 176 / 1,549 / 176），SQPlant 154 张 / 2,024 列（138 / 176 / 1,534 / 176）；两套都是 126 个不同表名，重名的表各在自己的角色下。
- 完成标志：「验收」第 1–9、11、13 条在 S2 内各自变绿。

### S3 命令行 `pid_backup_store`（Q17；1 笔提交）

- `pid_backup_store <输入> -o <输出.sqlite> [--force] [--keep-secrets] [--embed-files]`。缺 `-o` 或参数错退出 2，运行错误退出 1；输出已存在且没加 `--force` 时拒绝；先写临时文件再改名；结束时打印表数、行数、Ghost Row、LOB、文件数、脱敏条数和警告。`Cargo.toml` 加 `[[bin]]`，`required-features = ["backup"]`。
- 测试 `tests/backup_store_cli.rs`：`--help`、缺 `-o`、拒绝覆盖、`--force` 覆盖、TEST02 zip 成功；SQPlant 目录输入跑通（P10 没样本则跳过）；`--embed-files` 收进的文件字节与 zip 里的一致。README 加一节。

### S4 publish 改读 store（Q13、Q18、P6、P7；1–2 笔提交）

- 新 `src/publish/store_load.rs` 做 P7 的抄写：`codelists` / `attributes` 取 `pidd__`，其余 24 张取 `pid__`。
- `open_mdf_as_sqlite` / `load_drawing_graph_from_mdf` 改成「MDF → 内存 store → 抄写」；`pid_publish_xml` 和 `export_bundle` 认 Backup Store、Plant Backup 两种新输入；`mdf_load.rs` 里的旧暂存删掉。
- 测试：`publish_*` 不改断言就过；A01 的 `_Data.xml` / `_Meta.xml` 与 S1 之后的输出逐字节相同；新加一条：从 `TEST02_p.zip` 建的 store 出的 XML，与从 `Export.mdf` 出的逐字节相同。
- 连接缺陷（Q18）按 `docs/agents/issue-tracker.md` 开 GitHub issue，写进「把 `attribute_codelisted` 当编号、真连接是 `C<n>`」和「连上后 A01 的 `EqTypeDescription` 会从 `Horizontal Drum` 变成 `1D 1C 2:1 H Drum`」两件事；开之前把草稿给你看。

### S5 验收收口（1 笔提交）

- 把散在各步的测试补齐成 Q21 的 12 条；`AGENTS.md` 测试表、README、格式文档第 11 节「复现方法」、CHANGELOG、`task_plan.md` 跟上。

## 验收（Q21 a）

| # | 条目 | 测试 | 步 |
|---|---|---|---|
| 1 | 行数：154 张表逐表等于 `rcrows`，合计 37,470 | `backup_mdf_reader_test02`、`backup_store_test02` | S1、S2 |
| 2 | Ghost Row：5 张表各 1 行进 `dump_ghost_row`，字节与 MDF 相同 | 同上 | S1、S2 |
| 3 | LOB：4 个值都是 ZIP，首条目 `Drawing.xml` / `A01-JSite204.tmp` | 同上 | S1、S2 |
| 4 | 空串 162；NULL 按列类型计，nvarchar 21,431、全部类型 50,596 | 同上 | S1、S2 |
| 5 | 来源：抽样行按 `_src_page` / `_src_slot` 回 MDF 重解，结果一致 | `backup_store_test02` | S2 |
| 6 | Manifest 拼回逐字节相同（口径见 P9） | `backup_store_test02`、`backup_store_dwg` | S2 |
| 7 | `backup_file` 条目数和每条 SHA-256 与 zip 原件一致 | 同上 | S2 |
| 8 | 默认库里搜不到口令原文和加密串 | 同上 | S2 |
| 9 | 同一输入生成两次，逐表内容相同 | `backup_store_test02` | S2 |
| 10 | publish 改读 store 后 A01 测试通过；输出变化先说明原因 | `publish_*` + 新 parity 测试 | S1、S4 |
| 11 | DWG：154 张空表、2,039 列，按 owner 计，重名表各在自己的角色下；类型按 Q19，标「未解码」，Manifest 和文件清单照常入库 | `backup_store_dwg` | S2 |
| 12 | `cargo test`、两种特性组合的 `clippy -D warnings`、`fmt` | 每笔提交前 | 全程 |
| 13 | SQPlant（外部样本，缺则跳过）：154 张空表、2,024 列，四个角色按 P12 对上；Manifest 拼回；16 个顶层文件和各包条目数、SHA-256 对上，47 个 GBK 名字没有 U+FFFD；默认库搜不到口令原文 | `backup_store_sqplant` | S2、S3 |

## 登记不做（第一版）

| 项 | 理由 |
|---|---|
| Oracle exp 行解码 | Q3，第二版；SQPlant 是它的主样本（365,530 行） |
| `.pid` 解析结果、参考数据解析结果入库 | Q2，第二、三版 |
| SQPlant 的 `.pid` 解析结果入库 | Q2，第二版；现有 `pid_inspect` 53/53 能解析，留作那时的基线 |
| 选择表连接改成 `attribute_datatype = C<n>`，连同 writer 的优先级 | Q18，单独登记的缺陷 |
| d 与 pidd 合并 | Q18 没选 c |
| 视图定义、`sys` 下内部表 | 自定项 |
| Ghost Row 解成列 | Q12 |
| xlsx / xlsm / CFB 等容器展开 | Q14 |
| Oracle 大写列名归一 | Q20，第二版接 Oracle 行时再做 |
| publish 直接按类型查 store、去掉抄写 | P7；要做另开单，以 A01 为基线 |

## 风险

- **LOB 结构**：语料只有 4 个值、18 页，解码器只认见过的结构；DWG 是 Oracle，帮不上。
- **记录长度**：按结构算若在某类记录上算不出，Ghost Row「字节与 MDF 相同」那条会失败；那时停下报告，不退回取下一个偏移。
- **空串改动**会碰 vendored 集成测试，S1c 先查证再改。
- **S1 后 A01 输出可能变**：Ghost Row 在 `T_PlantItem` / `T_Equipment`，空串在 `T_Drawing`；按 Q13 先分析再接受。
- **共用工作树**：并行编译抢 `target/` 锁、误把别人的改动卷进提交；P1 规避。
- **外部样本**：SQPlant 不在 git 里，换机器或 CI 上第 13 条只会跳过，以本机结果为准；跳过时测试要打印原因。
- **条目名编码**：GBK 是按字节试解的推断，别的语言环境的备份可能是别的代码页；`path_raw` 保证原字节不丢。

## 门禁记录

- 2026-10-08：用户「使用 grill-with-docs 制定对备份文件的解析计划，可以将相关的数据都存储到 sqlite 里」；Q1–Q7 全部按推荐（会话 opus-5-5-71）。
- 2026-10-09：Q8–Q17 全部按推荐（opus-5-5-11）；`CONTEXT.md` 加 Schema Role、Ghost Row，ADR-0004 写成，Q18–Q21 全部按推荐（opus-5-5-13）。
- 2026-10-09：本计划写成（opus-5-5-18），P1–P9 等批，没改代码。
- 2026-10-09：用户「P1–P9 全部按推荐，Q2 的理解也对，开工 S0」（opus-5-5-18 收到后断开）→ S0（opus-5-5-17）：从 `fa4e268` 开 worktree `D:\work\plant-code\cad\pid-parse-backup-store`、分支 `backup-store-v1`；四份文档原样挪进一笔 `docs(backup)` 提交，只给本计划补了批准记录和上面那条 `CARGO_TARGET_DIR`；提交核对无误后，主树的 `CONTEXT.md` 还原、三份新文档删除。
- 2026-10-09：审计（opus-5-5-22）发现 SQPlant（`D:\work\cad\pid-test-data`）不在计划里；用户要求补进来作为第三套样本（第二套 Oracle）。改动稿（P10–P12、S2a / S2b / S2d / S3、验收第 11 条改、第 13 条加）经用户「P10–P12 按推荐，改动稿照落」批准后落进本计划，单独一笔 `docs(plan)` 提交，没改代码。
- 2026-10-09：S1a（opus-5-5-22）：槽表按槽号读、记录长度按结构算、Ghost Row 分出并由 `MdfDatabase::ghost_rows` 原样给出；TEST02 Ghost Row 正好 5 行，A01 的 `_Data.xml` / `_Meta.xml` 逐字节不变，`publish_mdf_load` 的 `T_PlantItem` 4 → 3。vendored 集成测试 `rows::case_5` 改动前就失败，不属本步，另记。
- 2026-10-09：S1b（opus-5-5-22）：`sysclsobjs` 认 schema、`user_tables` / `scan_table` / `from_bytes`；TEST02 的 154 张表逐表活行等于 `rcrows`、合计 37,470，列 1,798，四个 schema 与事实表一致（验收第 1 条的 S1 部分绿）；另加页链出分配单元即报错、多页堆显式拒读、超范围 decimal 不再 panic。`from_bytes` 仍走顺序读页器，随机读在 S1c。
- 2026-10-09：S1c（fable-5-1-9，接着一份未提交的 `pages.rs` 草稿做完）：整个文件进内存、按页号随机读；`image` / `text` / `ntext` 跟 16 字节文本指针读 `LARGE_ROOT_YUKON` 根、`INTERNAL` 节点和 `DATA` 片段，别的形状显式报不支持；零长度且空位未置位的变长列是 `""`；datetime 毫秒按 `(刻度 × 10 + 1) / 3`。TEST02：LOB 4 个值都是 ZIP、首条目 `A01-JSite204.tmp` / `Drawing.xml`；空串 162（10 张表，`T_Drawing` 4）；NULL 的 21,431 是 nvarchar 的数，全部类型合计 50,596 等于空位图置位数；A01 两份 XML 与 S1b 逐字节相同（验收第 3、4 条的 S1 部分绿，第 10 条 S1 部分绿）。
  查证：`tbl_Mitglied` 第 3 行 `Titel` 空位未置位，vendored 断言改为 `""`。顺手堵上：空分配单元的 `0:0` 指针以前被当第 0 页读、`T_EquipComponent` 多出一行假行，现跳过。P3 里「datetime 附原始的天数和刻度」没有做进公开接口：store 要的 `.fff` 从整数换算的 `DateTime` 直接格式化就对，换算可逆；要原始刻度另加。
- 2026-10-09：S1c 审查（fable-5-1-2，只读复核 `eda765b`，另在临时副本里跑 vendored 单测 66 / 集成 22 / 23、clippy、fmt，自写探针扫 TEST02）：数全部对上；另验 4 个 LOB 都是内部自洽的 ZIP（EOCD → 中央目录 → 每个本地头都在它说的偏移上），65,536 字节那个的 ZIP 在 38,535 字节处结束、其后 27,001 字节不是全零（同一存储块里上一版的残留，另外三个尾部全零）；`dump_lob` 的 SHA-256 按全部字节算。
  用户采纳两条意见落成一笔小提交：`text` LOB 不再按 UTF-8 猜解、改为原字节 `Value::Binary`（`text` 是排序规则代码页的单字节文本，读取器不知道代码页；TEST02 没有 `text` / `ntext` 列）；本计划事实表和验收第 4 条的「NULL 21,431」写明是 nvarchar 的数、全部类型 50,596。记下的两条限制留给 S2c：定长列超出记录存的列数仍走「omitted trailing column」的 break（行里缺键，写库时当 NULL），空位图按 leaf offset 顺序配、不是 `leaf_null_bit`，表结构改过的库会错，TEST02 没有短记录所以不受影响；INTERNAL 链接的尾偏移按全局偏移校验，语料只有 1 个 INTERNAL 节点，多节点时假设错了会报错、不会静默拼错。
- 2026-10-09：S2a（fable-5-1-9）：新模块 `backup::store`（`mod.rs`、`input.rs`），`build_backup_store` / `build_backup_store_in_memory`；收 zip 或目录，`store_info`、`backup_file`（外层 + 各 Option Archive 一层，逐条 SHA-256，`path_raw` / `path_encoding` 按 P11）、`backup_file_content`（`embed_files`）；先写 `.tmp` 再改名。MTF → MDF 的探测挪进 `backup::mtf`（`locate_sql_server_streams`、`detect_backup_stream_header_len`、`mdf_bytes_of_dump`），`pid_backup_extract` 改调库、输出不变。
  数：TEST02 外层 14、共 1,554 行，`.pid` 的 SHA-256 与格式文档第 8 节一致；DWG 外层 16、共 802 行；SQPlant 顶层 16、共 865 行，711 包 47 个 GBK 名字没有 U+FFFD（验收第 7 条 TEST02 / DWG 部分、第 13 条文件清单部分绿；第 9 条「两次构建逐表相同」在 `backup_file` / `store_info` 上先绿）。两处自定：目录输入的 `input_sha256` 取「`<sha256>  <名字>\n`」清单的 SHA-256（目录没有自己的字节）；`backup_file` 加了 `is_dir` 列，目录条目不记哈希和格式。Option Archive 的判据是 `(PlantData|RefData)~<n>~<id>.zip` 的名字（格式文档第 3 节：带 `.zip` 的是目录打包），不是 magic——`RefData~4~703` 是 xlsx，Q14 说不展开。
- 2026-10-09：S2b（fable-5-1-9）：`store::manifest`（`manifest_line` / `manifest_field` / `manifest_field_meaning` / `store_redaction`，视图从常量表生成，`reassemble_manifest`）、`store::redact`（Q15 规则）。三套样本 P9 两种口径都过，默认库搜不到 `DBPwds`、ConnInfo 密文和两个口令（验收第 6、8 条绿；SQPlant 的拼回与口令部分也绿）。
  查证：格式文档第 4 节的「加密串」有两处不对——`PlantConnInfo` 第 1 字段是 Plant 名（只有 `SiteConnInfo` 的是 64 字符密文），`DBUids` 是四个 Plant schema 名的逗号串（Oracle 命令行 `OWNER=(…)` 里原样还有）。两者按 Q15 仍换成 SHA-256，「搜不到原值」对它们不成立、测试排除；要不要把这两处从脱敏名单里去掉，等用户定。一处自定：`BackupCommand` 认得 `user/password@service` 就只换口令，SQL Server 的 `BACKUP DATABASE … TO …` 没有口令、原样保留，别的形状才整段换。
- 2026-10-09：S2c（fable-5-1-9 起草，fable-5-1-5 接着做完）：`store::mssql`——`PlantConnInfo` 的类型码 2 / 8 / 4 / 9 → 角色（`SchemaRole`，恰好四个否则报错），`dump_schema`（`role_source` = `manifest-conninfo`）；Database Dump 按 Manifest 第 12 字段的名字从外层文件里取，`mtf::mdf_bytes_of_dump` → `MdfDatabase::from_bytes`；按 Manifest `Table` 行顺序建 154 张 `<角色>__<表>`，行按页链和槽号插入，值按 Q9，`dump_table` / `dump_column` / `dump_view` / `dump_ghost_row` / `dump_lob` 齐。Oracle 转储这一步只认出来：`store_info.dump_kind` = `oracle-exp`（SQL Server 为 `sql-server-mtf`），warnings 写明未解码，表等 S2d。
  数：TEST02 154 张逐表等于 `rcrows`、合计 37,470；Ghost Row 5、字节与 MDF 槽上相同；LOB 4、`sha256` 等于表里 BLOB 的；NULL 按类型 21,431 / 29,009 / 148 / 8，空串 162；`pid__T_Drawing`、`pid__T_PlantItem`、`pidd__codelists` 共 3,210 行按 `_src_page` / `_src_slot` 回 MDF 重解逐列相同；两次构建全部表逐行相同（验收第 1、2、3、4、5、9 条绿）。
  查证：vendored `ColumnInfo` 加 `nullable`——`syscolpars.status` 位 1 是 `CPM_NOTNULL`（`sys.columns.is_nullable` = `1 - (status & 1)`），草稿里方向反了；TEST02 上置位的 206 列全是键列、205 列没有一个 NULL，由此坐实。顺带查出 `T_Symbol.SP_ID`（NOT NULL）在 2300:1 有一个空位图置位的 NULL（见事实表）；库按空位图写 NULL、与 Q9 和 50,596 的口径一致，`dump_column.nullable` 让人能查到。要不要改成「NOT NULL 列不看空位图」，等用户定。一处自定：`store_info` 多了 `dump_kind` 一键。`check-missing-docs.sh` 里的 `cargo rustdoc -W missing-docs` 报 4 处错，全在 `src/lib.rs` 和 `src/parsers/sheet_records.rs`（指向私有项的文档链接），本步没碰这两个文件；本 crate `deny(missing_docs)`，公开项的文档由编译把关。
- 2026-10-09：S2d（fable-5-1-5）：新 `backup::oracle_exp`——`scan_create_tables` 按 `\n` 走整个 dump，`CONNECT <OWNER>` 行换 owner、`CREATE TABLE "…" (…)` 行归到它前面最近的 owner（P12），表以 (owner, 名) 为键、不再按名去重；每列存 `type_spec`（`NUMBER(11, 0)` 原样）和 `modifiers`（`NOT NULL ENABLE`），`CREATE TABLE` 在任何 `CONNECT` 之前、列项不以引号开头等形状显式报错；example 改成调库、按 owner 分组打印。新 `store::oracle`——owner 与 Manifest `PlantConnInfo` 第 4 字段按 ASCII 大小写不敏感配、恰好一个否则报错，`dump_schema.schema_name` 存 dump 的大写原样（Q20；`DumpSchema` 多了 `dump_name`，SQL Server 两者相同），`dump_view.schema_name` 同；按 Manifest `Table` 行顺序建 154 张空表，列按 Q19 定类型（`sqlite_type_of_oracle`：NVARCHAR2 / VARCHAR2 TEXT、NUMBER(p, 0) 与 NUMBER(p) INTEGER、FLOAT REAL、DATE TEXT、BLOB BLOB；语料外的 NUMBER(p, s≠0) / 裸 NUMBER 记 TEXT 保数字原样），`dump_column` 的 length / precision / scale 取类型参数（NVARCHAR2(32) → 32，NUMBER(11, 0) → 11 / 0，FLOAT(126) → 126），`nullable` = 没写 NOT NULL；`dump_table.decoded = 0`、`expected_rows` NULL、rows / ghost_rows 0；表尾同样带 `_src_page` / `_src_slot`（Oracle 没有页，这两列可空，第二版再定放什么）。`store_info.dump_kind` = `oracle-exp`，warnings 一条写明 154 张表从 DDL 登记、留空、行未解码；dump 的四个 owner 下有 Manifest 没列的表时另给一条警告（语料里没有）。
  数：DWG 4 个 owner `QSMCQTAZ13_PLANT` / `…PLANTD` / `…PLANTPID` / `…PLANTPIDD`，154 张（22 / 25 / 82 / 25）、2,039 列（138 / 176 / 1,549 / 176），按类型 NUMBER 663、NVARCHAR2 987、VARCHAR2 11、FLOAT 348、DATE 28、BLOB 2，NOT NULL 150；SQPlant 4 个 owner `SQPLANT` / `SQPLANTD` / `SQPLANTPID` / `SQPLANTPIDD`（Manifest 写 `SQPlant…`），154 张、2,024 列（138 / 176 / 1,534 / 176），FLOAT 340、NVARCHAR2 980、其余同 DWG，NOT NULL 150；两套都是 126 个不同表名，`MAX_ID` 四个角色各一张、`SPIDCACHE` plant / pid 各一张；NUMBER 只有 (10, 0) / (11, 0)、FLOAT 只有 (126)；视图 35；每张表 0 行，每列的 SQLite 类型按 Q19 对上（验收第 11 条绿，第 13 条的转储部分绿）。按类型的列数另用 PowerShell 正则逐条数过，相同。`Cargo.toml` 给 example 加了 `required-features = ["backup"]`（它现在调库）。
