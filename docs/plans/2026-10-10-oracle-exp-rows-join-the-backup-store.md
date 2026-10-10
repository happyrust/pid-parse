# Oracle `exp` 转储的行进 Backup Store · 第二版计划（2026-10-10 开单，同日批准）

> 起因：用户 2026-10-10「给第二版（Oracle exp 行解码）写一份 grill 计划草稿，先不动代码」。
> 第一版（`docs/plans/2026-10-09-a-plant-backup-becomes-one-backup-store.md`，S0–S5 在 `backup-store-v1` 做完，PR [#28](https://github.com/happyrust/pid-parse/pull/28)）
> 按 Q3 / Q16 只把 Oracle 备份的 154 张表按 DDL 登记成空表（`dump_table.decoded = 0`），行解码登记为第二版；
> 同时登记到「第二版」的还有 Q2 的「`.pid` 解析结果入库」和 Q20 的「Oracle 大写列名归一」——本稿只定 **Oracle `exp` 行解码** 一件事，另外两件在 Q1 / Q7 里问清去向。
> 问答 Q1–Q10 和实现层 P1–P8 都带 ⭕ 推荐。开单时没改代码；下面「格式」和「事实」两节的内容是用仓库外的只读探针
> （Python，放在 `%TEMP%`，不进仓）在两套 Oracle 样本上查明的，S1 要用测试把它们钉死。
> 术语沿用 `CONTEXT.md`（Plant Backup、Database Dump、Backup Store、Schema Role）；本稿新增一个：**LOB 片**——`exp` 把一个 BLOB 值切成不超过 8,060 字节的片写出。
>
> **2026-10-10 用户批准**：「Q1–Q10、P1–P8 全部按推荐，把草稿以 docs(plan) 一笔提交到 backup-store-v1」。本计划随 PR #28 走；S1 起按 Q10 在 `backup-store-v2` 上做。各步进度见「门禁记录」。

## 一句话

把 Oracle `exp` 转储里 154 张表的行解出来写进 Backup Store 的 `<角色>__<表>`（DWG 44,023 行、SQPlant 365,530 行），
`dump_table.decoded` 变 1、`expected_rows` 取 `Export.log` 里「N rows exported」，BLOB 原字节进表并登记 `dump_lob`，
每行带它在 `Export.dmp` 里的字节偏移；语法只认两套样本里见过的形状，别的显式报错、不猜。SQL Server 侧一个字节不动，A01 两份 XML 的 SHA-256 不变。

## 格式（本稿查明的部分；证据等级按 `CONTEXT.md`：decoded / identified_only / unknown）

两套样本（DWG `test-file/backup-test/DWG-0202GP06-01_p.zip` 里的 `Export.dmp` 8,802,304 字节；SQPlant `D:\work\cad\pid-test-data\Export.dmp` 74,489,856 字节）都是 Oracle 12.1.0.2 的 `exp`、Conventional Path（`Export.log` 写明），语法逐字节相同。

### 文件层

- 文件头 `03 03 69` + `EXPORT:V12.01.00\n`、`DSYSTEM\n`（导出用户）、`RUSERS\n`（按用户导出），然后四行数字 `2048` / `0` / `64` / `0`，再是几行二进制头（里面有导出时间和 dump 的原路径，如 `Sun Jul 19 20:13:30 2026  \\mm-128\pid_sqproject\Backup\260719201324\Export.dmp`，以及 `+08:00` / `+00:00`、`BYTE`、`UNUSED`、`INTERPRETED`、`DISABLE:ALL` 这类 NLS / 参数串）、一行 `METRICSU`。四行数字的含义 **unknown**；两套文件的大小都是 2048 的整数倍（4,298 × 2048、36,372 × 2048）、文件尾用 0 补齐，`2048` 很可能是 `RECORDLENGTH`（identified_only）。
- 之后是文本语句与二进制行块交错。语句都独占一行、以 `\n` 结尾。两套样本的语句普查完全相同：`CONNECT` 44、`TABLE "…"` 554、`CREATE TABLE` 154、`INSERT INTO` 154、`GRANT` 544、`ALTER TABLE` 588、`CREATE INDEX` 163、`CREATE UNIQUE INDEX` 34、`CREATE VIEW` 35、`ENDTABLE` 251、`EXIT` 2。
- 每张表一段：`TABLE "X"` → `CREATE TABLE "X" (…)  PCTFREE …`（第一版已解析）→ `INSERT INTO "X" ("A", "B", …) VALUES (:1, :2, …)` → **二进制行块** → `\n` → 这张表的 `GRANT …` / `CREATE [UNIQUE] INDEX …` / `ALTER TABLE … ADD …`。`INSERT INTO` 紧跟在自己的 `CREATE TABLE` 后面（154 / 154）；`TABLE "X"` 行在文件后面的授权 / 约束段里还会再出现（554 行里只有 154 行后面跟着 `CREATE TABLE`，109 行后面是 `ALTER TABLE`），所以**行块要按 `CREATE TABLE` → `INSERT INTO` 的相邻关系认，不能按 `TABLE` 行认**。
- 表归 owner 仍按第一版 P12：前面最近的 `CONNECT <OWNER>`。

### 行块（decoded；下面每一条都在两套样本的全部 154 张表上走通，行数逐表等于 `Export.log`）

```
u16  列数 n                         与 INSERT 列表的长度相同
n ×  列描述                         u16 类型码, u16 最大字节长；类型码 1 再跟 u16 字符集 id, u16 字符集形式
u16  0                              描述结束
LOB 名块                            u16 m（没有 LOB 列时 0，有时等于 n）；m 个 u16（语料里全 0，含义 unknown）；
                                    m > 0 时再 u16 0；然后 m 个「u8 长度 + 名字」，LOB 列名排在前面、其余为空（单字节 0）
行 × k                              每列 u16 长度 + 字节；FE FF 表示 NULL
                                    行尾 u16 t = 随后的 LOB 值个数（没有 LOB 列的表恒为 0）
                                    t 个 LOB 值：u16 长度 + 字节；长度字为 FC FF 时改为 u64 总长 + 若干 LOB 片（每片 u16 长度 ≤ 8,060 + 字节）直到凑满总长
FF FF                               表结束，紧跟 \n
```

0 行的表就是 `描述… 00 00 | 00 00 | FF FF \n`（如 `CATEGORY`）。

| 类型码 + 字符集 | 对应 DDL | 最大长 | 值的编码 | 两套样本的列数 |
|---|---|---|---|---|
| 2 | `NUMBER(p, 0)`、`FLOAT(126)` | 22 | Oracle 内部 NUMBER：首字节指数（`0x80` 单字节 = 0；正数 `0xC1` 起、尾数每字节 = 两位十进制 + 1；负数指数取反、尾数 = 101 − 两位数、末尾多一个 `0x66`） | DWG 1,011（663 + 348）；SQPlant 1,003（663 + 340） |
| 1 + 字符集 2000 + 形式 2 | `NVARCHAR2(c)` | 2c | **UTF-16BE**（AL16UTF16）——格式文档 5.2 节写的「UTF-16LE」不对，S3 改 | 987；980 |
| 1 + 字符集 873 + 形式 1 | `VARCHAR2(4000)` | 4000 | UTF-8（AL32UTF8） | 11；11 |
| 12 | `DATE` | 7 | 世纪 + 100、年 + 100、月、日、时 + 1、分 + 1、秒 + 1，如 `78 7E 03 05 11 20 21` = 2026-03-05 16:31:32 | 28；28 |
| 113 | `BLOB` | 64 | 行里是 86 字节的 LOB 定位符（`00 54 00 01 01 0C 00 80 …`，内部结构 unknown），数据在行尾的 LOB 值里 | 2；2（都是 `SP_STORAGE`：`T_DRAWINGVERSION`、`T_SMARTFRAMESTORAGE`） |

单测要钉的 NUMBER 编码（都取自样本）：`80` → 0；`C1 02` → 1；`C2 03 16` → 221；`C2 0B 07` → 1006；`C4 0D 01 01 1D` → 12000028；`3E 64 66` → −1；`C0 02` → 0.01；`C0 06` → 0.05；`BF 04 33` → 0.00035；`C2 03 54 10` → 283.15；`C2 04 5E 10` → 393.15；`40 5A 61 2B 20 0B 2E 64 29 66` → −0.00110458699055016；`3F 43 61 40 1F 38 33 2D 1F 66` → −0.340437704550567。

## 事实（2026-10-10，`backup-store-v1` = `f4fc902`；探针在仓外，S1 的测试要把这些数钉住）

| 项 | 数 / 位置 | 出处 |
|---|---|---|
| 行数 | DWG 44,023（pidd 39,303、d 858、plant 502、pid 3,360）；SQPlant 365,530（pidd 39,197、d 858、plant 550、pid 324,925）；两套都逐表等于 `Export.log` 的「N rows exported」，没有一张对不上 | 探针按上面的语法走完 154 张表；`Export.log` |
| 列 | DWG 2,039、SQPlant 2,024，类型码只有 2 / 1 / 12 / 113 四种，与第一版 S2d 按 DDL 数的 NUMBER + FLOAT / NVARCHAR2 + VARCHAR2 / DATE / BLOB 逐类相同 | 行块描述 vs `dump_column` |
| 非 NULL 值 | DWG：数值 242,909、字符串 145,260、DATE 767、LOB 21；SQPlant：数值 1,612,181、字符串 1,024,059、DATE 74,531、LOB 27 | 探针 |
| NULL | DWG：字符串 46,163、数值 48,324、DATE 460，合计 94,947；SQPlant：字符串 2,048,312、数值 1,583,868、DATE 42,628，合计 3,674,808。Oracle 没有空串（空串即 NULL），所以不会有第一版 Q9「空串与 NULL 分开」的问题 | 探针 |
| NUMBER(p, 0) | DWG `NUMBER(11, 0)` 223,904 个值（最大 12,002,751）、`NUMBER(10, 0)` 17,611（最大 40,352）；SQPlant 223,385（12,002,650）、1,239,347（77,384）；**没有一个带小数、没有一个超出声明精度或 i64**；负数有（如 −1） | 探针按 DDL 类型分组 |
| FLOAT(126) | DWG 1,394 个值（780 带小数、30 负）；SQPlant 149,449（94,658 带小数、1,975 负）；内部尾数最长 9 字节（NUMBER 整体最长 10 字节）、**没有一个值超过 17 位有效数字**，所以按十进制文本转 f64 不丢位 | 探针 |
| DATE | DWG 2003-10-16 00:00:00 … 2026-04-17 17:59:55；SQPlant 1979-11-30 00:00:00 … 2026-07-19 20:09:20；全部 7 字节、没有畸形值、没有公元前 | 探针 |
| 字符串 | 最长 184 字节；NVARCHAR2 全部按 UTF-16BE 解码无错、没有代理对、没有 U+FFFD；VARCHAR2 全部是合法 UTF-8 | 探针 |
| LOB | DWG 21 个值（`T_DRAWINGVERSION` 14 + `T_SMARTFRAMESTORAGE` 7），295,493 字节，最大 141,932，10 个走长格式（最多 18 片）；SQPlant 27 个（15 + 12），1,032,581 字节，最大 449,698，10 个长格式（最多 56 片）；片长只见过 ≤ 8,060；**48 个全部以 `PK\x03\x04` 开头**（和 TEST02 的 4 个 LOB 一样是 ZIP）。DWG 第 1 个 BLOB 7,507 字节 = ZIP 7,506 + 1 个尾字节 `00`（和 S1c 审查见到的「ZIP 之后有残留字节」同类）；SQPlant 第 1 个的 ZIP 尾按 EOCD 算比 BLOB 长 1 字节，S1 要用完整的 ZIP 走查（EOCD → 中央目录 → 本地头）核对而不是只看 EOCD | 探针；格式文档第 5.2 节 |
| LOB 名块 | 12 列的 `T_DRAWINGVERSION`：`0C 00` + 26 个 0 + `0A "SP_STORAGE"` + 11 个 0；3 列的 `T_SMARTFRAMESTORAGE`：`03 00` + 8 个 0 + `0A "SP_STORAGE"` + 2 个 0。两者都符合「u16 n、n 个 u16、u16 0、n 个 u8 长度 + 名字」 | 探针 |
| 现在的库与测试 | `store::oracle::write_empty_table` 建空表、`dump_table` 写 `decoded 0 / expected_rows NULL / rows 0`；`store::mod` 对 Oracle 转储打一条 warning「registered from the DDL and left empty, their rows are not decoded in this version」；`tests/common/backup_store.rs::check_oracle_dump` 钉「every table undecoded and empty」「no ghost rows or LOBs」；`tests/backup_store_cli.rs::sqplant_directory_is_built_with_its_warnings_on_stderr` 钉 stdout `dump tables 154, rows 0, ghost rows 0, LOBs 0 / warnings 2` 和那条 warning 的原文 | 代码 |
| 转储表的形状 | Oracle 的 `<角色>__<表>` 已带 `_src_page` / `_src_slot` 两列、可空，第一版 S2d 写明「第二版再定放什么」；`dump_lob` 的列是 `row_page / row_slot / column_name / root_page / root_slot / length / sha256` | `store::oracle`、`store::mssql` |
| publish 侧 | `publish::store_load::text_of` 只认 SQL Server 的类型名（`datetime` / `real` / `image` …），Oracle 的 `DATE` / `NUMBER` / `FLOAT(126)` 会原样透传；SQLite 的表名、列名对 ASCII 大小写不敏感，所以 `pid__T_Drawing` 的查询能命中 `pid__T_DRAWING`；仓里没有 DWG 的参考 `_Data.xml` / `_Meta.xml`（只有 `test-file/DWG-0202GP06-01.pid` 和 byte-audit 基线） | `src/publish/store_load.rs:305`；`git ls-files` |
| 性能参照 | Python 探针在内存里走完 SQPlant 全部行用 7.9 秒、DWG 0.8 秒 | 探针 |
| 工作树 | `D:\work\plant-code\cad\pid-parse-backup-store` 在 `backup-store-v1` = `f4fc902` = `origin`，干净；PR #28 base `main`，CI 两处红都是 `main` 的旧账（`cargo audit` 的 quick-xml / rkyv、`style_link_ratchet` 两条） | `git status`、`gh pr checks` |

## 已定决策（Q1–Q10；2026-10-10 全部按 ⭕ 推荐定案）

| 题 | 问 | 选项 |
|---|---|---|
| Q1 | 第二版的范围 | ⭕ **a** 只做 Oracle `exp` 行解码；第一版 Q2 放到「第二版」的「`.pid` 解析结果入库」另开一份计划（它和备份格式无关，是 `.pid` 解析器的产出怎么入库）。b 两件一起做——一份计划两条线，验收和提交都要分开写 |
| Q2 | Oracle 行的 `_src_page` / `_src_slot` 放什么 | ⭕ **a** `_src_page` = 这一行在 `Export.dmp` 里的字节偏移（第一列长度字的位置），`_src_slot` = 这一行在本表行块里的序号（0 起）；哪种语义看 `store_info.dump_kind`（`sql-server-mtf` / `sql-server-mdf` 是页和槽，`oracle-exp` 是偏移和序号），README、格式文档、`mssql.rs` 的 SCHEMA 注释写明；两列都还能把行找回来重解（验收第 7 条），表的形状不变。b 两边都加 `_src_offset`（SQL Server 为 NULL）——154 + 154 张表都多一列永远空着的。c 留 NULL——Q6「每行可追溯到来源文件」就落空了 |
| Q3 | 值怎么落（第一版 Q9 的 Oracle 版） | ⭕ `NUMBER(p, 0)` → INTEGER（精确解码；带小数或超出 i64 报错）；`FLOAT(126)` → REAL（先解成十进制文本，再 `f64::from_str` 取最近的 double——Rust 的解析是正确舍入的、确定的）；语料外的 `NUMBER(p, s ≠ 0)` / 裸 `NUMBER` → TEXT 十进制文本（Q19 已定，只有单测）；`DATE` → TEXT `YYYY-MM-DD HH:MM:SS.000`（Q19「毫秒补 000」，与 SQL Server 的 `.fff` 同宽）；`NVARCHAR2` → TEXT，UTF-16BE 严格解码；`VARCHAR2` → TEXT，UTF-8 严格解码；`BLOB` → BLOB，各片按序拼接的原字节；`FE FF` → NULL。备选：FLOAT 存十进制 TEXT 保全部位——语料里没有超过 17 位的值，REAL 不丢位，而且 Q19 已定 REAL |
| Q4 | 行块里的类型码要不要和 DDL 对 | ⭕ 要：`NUMBER` / `FLOAT` ↔ 2、`NVARCHAR2` ↔ 1 + 2000 + 2、`VARCHAR2` ↔ 1 + 873 + 1、`DATE` ↔ 12、`BLOB` ↔ 113，`INSERT` 的列名列表也要和 `CREATE TABLE` 的逐个相同；对不上就报错并写明表、列、两边各是什么。语料外的类型码（`CLOB` 112、`RAW` 23、`LONG` 8、`TIMESTAMP` 180 / 181 / 231 …）按名拒绝，不解。备选：只信行块——那 DDL 和行块不一致时库里就是错的还不知道 |
| Q5 | LOB 怎么登记 | ⭕ `dump_lob` 一个 BLOB 值一行：`row_page` / `row_slot` 同这一行的 `_src_page` / `_src_slot`，`column_name`，`root_page` = 这个 LOB 值第一个长度字的偏移，`root_slot` NULL，`length`，`sha256`（全部字节）；86 字节的定位符**不存**——它是 Oracle 内部句柄，回不了查，而且原字节在 dump 里按偏移随时能找到。备选：定位符存进新表 `dump_lob_locator`——第一版 Q6 的分级是「确认了的字段才起名」，一个 unknown 的 86 字节不值得一张表 |
| Q6 | `expected_rows` 从哪来 | ⭕ 从备份外层的 `Export.log`：`About to export <OWNER>'s objects` 定 owner，`. . exporting table <NAME> <N> rows exported` 定每张表（owner 与 `CONNECT` 的拼法相同、大写）；写进 `dump_table.expected_rows`，和 SQL Server 的 `rcrows` 同一列、同一个意思（「来源自己说有多少行」）。`Export.log` 不在、或没列这张表 → NULL 加 warning；解出的行数 ≠ `expected_rows` → warning（不是错误，和 SQL Server 侧一致），验收在两套样本上钉相等。备选：不读 `Export.log`——那就没有独立的对账数 |
| Q7 | Oracle 大写名字（第一版 Q20 留的「第二版接行时再做归一」） | ⭕ **不归一**：表名、列名、`dump_column.name` 继续存 dump 的大写原样；SQLite 标识符对 ASCII 大小写不敏感，任何按 SmartPlant 拼法写的查询都能命中；Manifest 的拼法在 `manifest_field`。备选：按 Manifest `Table` 行的拼法改写表名——列名 Manifest 里没有，归一只能归一半 |
| Q8 | 第二版要不要让 publish 读 Oracle 的 store | ⭕ **不做，登记不做并另开单**：仓里没有 DWG 的参考 XML 可以把关；`text_of` 要学会 Oracle 的类型名；选择表、符号路径等在 Oracle 库里是否同形没查过。第二版只保证 `open_publish_input` 对 Oracle store 的分类不变（它已是 `backup-store`）。备选：加一条「对 DWG store 跑 publish 不报错」的冒烟测试——没有参考，过了也不说明什么 |
| Q9 | 失败策略 | ⭕ 全部显式报错、带表名、行号（序号）和字节偏移，一条都不静默跳过：未知类型码；描述数 ≠ 列数；LOB 名块的 u16 不是 0 或名字对不上 LOB 列；列值长度超过描述的最大长；行尾 LOB 个数 > LOB 列数；长格式各片之和 ≠ 总长；UTF-16 字节数为奇数或解码失败；UTF-8 解码失败；NUMBER 尾数字节不在 1..=100（负数 1..=101 加结尾 `0x66`）或指数算出小数却落在 INTEGER 列；DATE 长度 ≠ 7 或月日时分秒越界；`FF FF` 之后不是 `\n`。新错误枚举 `ExpRowError`，经 `BackupStoreError` 透出。备选：跳过坏行记 warning——和第一版 S1a「停下报告，不退回」的做法相反 |
| Q10 | 在哪条分支上做 | ⭕ 同一个工作树（已干净）新开分支 `backup-store-v2`，从 `f4fc902` 起；#28 合进 `main` 后再对 `main` 开 PR（#28 之前合不了就先对 `backup-store-v1` 开叠放的 PR）。每步 1 笔提交、只 `git add` 本步路径、`CARGO_TARGET_DIR=D:\Rust\target-backup-store`。备选：继续往 `backup-store-v1` 上提交、让 #28 长大——评审面从 55 个文件再涨 |

## 库的形状（第二版改动；表名、列名都不加不减）

| 表 / 列 | 第一版 | 第二版 |
|---|---|---|
| `<角色>__<表>`（Oracle） | 空表 | 按行块顺序插入全部行；值按 Q3；`_src_page` = 行偏移、`_src_slot` = 行序号（Q2 a） |
| `dump_table`（Oracle） | `decoded 0`、`expected_rows NULL`、`rows 0`、`ghost_rows 0` | `decoded 1`、`expected_rows` = `Export.log` 的数（Q6）、`rows` = 解出的行数、`ghost_rows 0`（`exp` 没有 Ghost Row） |
| `dump_lob`（Oracle） | 无 | 一个 BLOB 值一行（Q5） |
| `dump_column`（Oracle） | 已有 | 不变 |
| `store_info` | `dump_kind = oracle-exp` | 不变；不新增键 |
| warnings | 「154 tables … left empty …」一条 | 删掉；只剩输入是目录时的 Q7 提示；`Export.log` 缺失或对不上时另给 |
| `pid_backup_store` 对 SQPlant 的 stdout | `dump tables 154, rows 0, ghost rows 0, LOBs 0` / `warnings 2` | `dump tables 154, rows 365530, ghost rows 0, LOBs 27` / `warnings 1` |

## 实现层决策（P1–P8；2026-10-10 全部按 ⭕ 推荐定案）

| # | 决策 | 结论 |
|---|---|---|
| P1 | 代码放哪 | ⭕ `src/backup/oracle_exp.rs` 改成目录：`oracle_exp/mod.rs`（现有 DDL 扫描；`ExpTable` 多记 `insert_offset`——紧跟其后的 `INSERT INTO` 行的位置、以及行块起点）、`oracle_exp/rows.rs`（列描述、LOB 名块、行迭代器 `ExpRows`，逐行给出 `ExpRow { offset, ordinal, values, lobs }`，`ExpValue::{Null, Number(ExpNumber), Date(ExpDate), Utf16(String), Utf8(String), LobLocator(Vec<u8>)}`，`ExpLob { offset, bytes }`）、`oracle_exp/number.rs`（`ExpNumber` 的解码与 `to_decimal_string` / `to_i64` / `to_f64`，`ExpDate`）、`oracle_exp/export_log.rs`（Q6）。`store::oracle::write_oracle_dump` 调它们写行，`write_empty_table` 变成 `write_table`。都在 `backup` 特性下 |
| P2 | 内存 | ⭕ 整个 dump 已在内存（`&[u8]`），行迭代器零拷贝切片，只有 LOB 片要拼接成 `Vec<u8>`；SQPlant 74 MB 没问题。备选：流式读——`exp` 的行块里没有可以跳到下一张表的长度，流式也得逐字节走 |
| P3 | NUMBER 的解码 | ⭕ 纯函数、按 Oracle 公开的内部格式：指数字节、以 100 为底的尾数、负数的补码与 `0x66` 结尾；先得到 `(负号, 十进制指数, 两位数数组)`，再按目标类型转：INTEGER 列要求没有小数位、落在 i64；REAL 列走十进制文本 → `f64`；TEXT 列直接存文本。单测用「格式」一节的 13 个编码，再加零、正负边界各一个人造值 |
| P4 | 字符串 | ⭕ `String::from_utf16`（先把 BE 字节两两组成 u16）和 `std::str::from_utf8`，失败即错（Q9），不用 `_lossy`；`encoding_rs` 不需要 |
| P5 | `Export.log` 怎么拿 | ⭕ `BackupInput` 已能按名读外层文件：Manifest 没有 `Export.log` 的 `File` 行也不要紧，直接按名字 `Export.log` 读（两套样本都有、都在 `backup_file` 里），没有就按 Q6 走 NULL + warning；解析只认 `. . exporting table` 和 `About to export … objects` 两种行，别的忽略 |
| P6 | 测试怎么长 | ⭕ 新 `tests/oracle_exp_rows.rs` 直接测解码器（DWG 走 git 里的 zip；SQPlant 按 P10 的路径、缺则跳过）：154 张表逐表行数等于 `Export.log`，按类型的非 NULL / NULL 数、NUMBER / FLOAT / DATE / 字符串 / LOB 的统计全按「事实」表钉；`tests/common/backup_store.rs::check_oracle_dump` 的「undecoded and empty」改成「decoded、rows = expected_rows 逐表、合计」，`OracleDumpExpectation` 加行数 / NULL / LOB 的字段，`backup_store_dwg` 和 `backup_store_sqplant` 填各自的数；抽样回核（验收第 7 条）：`T_DRAWING`、`T_DRAWINGVERSION`（带 LOB）、`CODELISTS`（pidd，行多）三张表的每一行按 `_src_page` 回 dump 重解逐列相同；`…builds_the_same_twice` 已覆盖全部表、不用改；`backup_store_cli` 的 SQPlant 那条改 stdout / stderr 的字面；SQL Server 侧的测试一条不改 |
| P7 | 性能 | ⭕ 行按表一个事务、预编译 INSERT（`mssql.rs` 现成的写法）；不设门槛，S2 记下 `pid_backup_store` 对 SQPlant 在 debug / release 下的耗时作参照 |
| P8 | 文档 | ⭕ 格式文档 5.2 节改写成上面的「格式」（含 UTF-16BE 的更正），第 10 节去掉「行编码尚未实现解码」、补 unknown 项（头部四行、LOB 名块的 u16、定位符）；README 的 Backup Store 一节改 `<角色>__<表>` 那行和 SQPlant 的数；ADR-0004 追加一段「`_src_page` / `_src_slot` 在 Oracle 转储里是偏移和序号」（Q2 a）；AGENTS.md 测试表加 `oracle_exp_rows`；CHANGELOG、`task_plan.md`、本计划门禁记录 |

## 步骤

每一步单独能提交，提交只 `git add` 本步碰过的路径；每笔提交前跑 `cargo test --workspace`、`--no-default-features`、两种特性的 `clippy -D warnings`、`fmt --check`、`check-missing-docs.sh`，数字写进提交说明和 CHANGELOG。

### S1 解码器（P1–P5；1–2 笔提交）

- `oracle_exp/number.rs`：NUMBER、DATE；单测 13 个样本编码 + 边界。
- `oracle_exp/rows.rs`：列描述、LOB 名块、行迭代器、LOB 片拼接、Q4 的 DDL 对账、Q9 的全部错误；单测用手写的小行块（含 0 行表、NULL、长格式 LOB、每一种坏输入）。
- `oracle_exp/export_log.rs`：Q6。
- `tests/oracle_exp_rows.rs`：两套样本全量走查，数按「事实」表钉；每个 BLOB 是内部自洽的 ZIP（走 EOCD → 中央目录 → 本地头，不只看 EOCD）。
- 完成标志：验收第 1、2、4、5、6（解码器部分）、9 条绿；库和 store 还没变。

### S2 入库（P6、P7；1 笔提交）

- `store::oracle` 写行、`_src_page` / `_src_slot`、`dump_table`、`dump_lob`；删掉「left empty」warning，加 `Export.log` 的两条 warning；`check_oracle_dump` 和两套样本的期望值、`backup_store_cli` 的字面跟上；抽样回核；两次构建。
- 顺手确认 `publish_store_parity`、`publish_*`、`backup_store_test02`、`backup_mdf_reader_test02` 一条断言没动、全过（SQL Server 侧零改动）。
- 完成标志：验收第 3、6（入库部分）、7、8、10 条绿。

### S3 文档收口（P8；1 笔提交）

- 格式文档、README、ADR-0004、AGENTS.md、CHANGELOG、`task_plan.md`、本计划的「已定决策」与门禁记录。
- 完成标志：验收第 11 条；第二版做完。

## 验收

| # | 条目 | 钉在 | 步 |
|---|---|---|---|
| 1 | DWG：154 张表逐表行数等于 `Export.log`，合计 44,023（39,303 / 858 / 502 / 3,360） | `oracle_exp_rows`；`backup_store_dwg`（`dump_table.rows = expected_rows` 逐表、表自身 `count(*)` 相同） | S1、S2 |
| 2 | SQPlant（仓外样本，缺则跳过并打印原因）：逐表等于 `Export.log`，合计 365,530（39,197 / 858 / 550 / 324,925） | 同上两处的 SQPlant 版 | S1、S2 |
| 3 | 值按 Q3 落库：`NUMBER(p, 0)` 列全是整数、`FLOAT` 列 REAL、`DATE` 文本 `YYYY-MM-DD HH:MM:SS.000`、`NVARCHAR2` / `VARCHAR2` TEXT、`BLOB` BLOB；抽样值钉住（如 DWG `T_DRAWING` 第 1 行 `NAME` = `001`、`DOCUMENTTYPE` = 631、`DATECREATED` = `2026-03-10 17:31:43.000`、`PATH` = `\zcgc\A3jqz\001.pid`；SQPlant 第 1 行 `NAME` = `D07`、`DOCUMENTTYPE` = 631；DWG `T_PIPERUN` 第 1 行 `NOMINALDIAMETER` 50、`SP_INSULTHICKSI` 0.05） | `backup_store_dwg` / `backup_store_sqplant` | S2 |
| 4 | NULL 按类型：DWG 字符串 46,163、数值 48,324、DATE 460，合计 94,947；SQPlant 2,048,312 / 1,583,868 / 42,628，合计 3,674,808 | `oracle_exp_rows`；`check_oracle_dump` | S1、S2 |
| 5 | 字符串：UTF-16BE / UTF-8 全部严格解码成功，没有 U+FFFD；最长 184 字节；NUMBER 内部最长 10 字节；FLOAT 没有超过 17 位有效数字的值；DATE 范围两套各自钉住 | `oracle_exp_rows` | S1 |
| 6 | LOB：DWG 21 个（14 + 7）、295,493 字节、最大 141,932、10 个长格式；SQPlant 27 个（15 + 12）、1,032,581、最大 449,698、10 个长格式；48 个都是内部自洽的 ZIP；`dump_lob` 一行一个、`sha256` 等于表里 BLOB 的、`length` 等于 BLOB 长 | `oracle_exp_rows`；`check_oracle_dump` | S1、S2 |
| 7 | 来源：`T_DRAWING`、`T_DRAWINGVERSION`、`CODELISTS` 每一行按 `_src_page`（偏移）回 dump 重解，逐列与库里相同；`_src_slot` 连续、从 0 起 | `backup_store_dwg` / `backup_store_sqplant` | S2 |
| 8 | 同一输入生成两次，逐表内容相同（含有行的 Oracle 表） | 现有 `…builds_the_same_twice` 的 Oracle 对应（`backup_store_dwg` 加一条） | S2 |
| 9 | 坏输入确定失败：Q9 列出的每一种各一个构造用例，错误里带表、行、偏移 | `oracle_exp::rows` 单测 | S1 |
| 10 | SQL Server 侧零改动：`cargo test --workspace` 里 S4 / S5 的全部断言不动；A01 两份 XML 的 SHA-256 `6ab41b66…` / `44291a6c…` 不变；`pid_backup_store` 对 SQPlant 的 stdout / stderr 按「库的形状」更新后逐字相同 | `publish_store_parity`；`backup_store_cli` | S2 |
| 11 | `cargo test`、两种特性的 `clippy -D warnings`、`fmt`、文档四处同步 | 每笔提交前 | 全程 |

## 登记不做（第二版）

| 项 | 理由 |
|---|---|
| `.pid` 解析结果入库（第一版 Q2 放在「第二版」的那条） | Q1 a：另开计划 |
| publish 读 Oracle 的 store | Q8：没有参考 XML；另开单 |
| LOB 定位符（86 字节）解码或入库 | Q5；unknown |
| 文件头四行数字、LOB 名块里 n 个 u16 的含义 | unknown；解码器只要求它们是见过的值（u16 为 0），不是就报错 |
| `GRANT` / 索引 / 约束 / 视图定义入库 | 第一版自定项的延续：视图只有名字 |
| Direct Path 导出（`DIRECT=y`）、其他 `exp` 版本、Data Pump（`expdp`） | 语料只有 12.1 的 Conventional Path；语法对不上就按 Q9 报错，不猜 |
| Oracle 大写名字归一 | Q7 |
| 公元前 DATE、`NUMBER(p, s ≠ 0)` 的真实样本 | 语料里没有；只有单测 |

## 风险

- **语法只见过一个版本、一个产品的两套 dump**：`EXPORT:V12.01.00`、Conventional Path。别的版本可能改列描述的宽度或 LOB 的写法；Q9 保证错在明处。
- **`FC FF` 的长格式只在 LOB 上见过**：字符串最大 4,000 字节、NUMBER 最大 22 字节，到不了 `FFFC`，所以普通列不会碰到；解码器仍按「任何列的长度字 ≥ `FFFC` 且不是 `FFFE`」报错。
- **LOB 名块理解不全**：n 个 u16 全 0、名字顺序「LOB 列在前」只在两张表（3 列、12 列，各一个 LOB 列）上见过；多个 LOB 列的表（语料里没有）顺序可能不同——按 Q9 报错，不猜。
- **0 行的 LOB 表没见过**：两套样本里两张 LOB 表都有行；单测用人造行块覆盖。
- **FLOAT → REAL**：语料里没有超过 17 位的值，但 `FLOAT(126)` 允许到 38 位；S1 的测试钉「超过 17 位的值 = 0 个」，以后样本变了会先在这里红。
- **`Export.log` 是日志不是目录**：靠正则认行；格式一变 `expected_rows` 就是 NULL + warning，不会错写。
- **BLOB 与 ZIP 的长度差 1 字节**（DWG 多 1、SQPlant 按 EOCD 少 1）：S1 用完整 ZIP 走查定清楚是 BLOB 带尾字节还是 EOCD 算法的事；库里存 dump 给的全部字节，不裁。
- **工作树共用 `.git`**：主树上别的会话在改别的分支；只 `git add` 明确路径。

## 门禁记录

- 2026-10-10：本草稿写成（fable-5-1-27）。格式用仓外只读探针在 DWG（仓内 zip）和 SQPlant（`D:\work\cad\pid-test-data`）上查明：两套样本 154 张表逐表行数等于 `Export.log`（44,023 / 365,530），NVARCHAR2 是 UTF-16BE。没改代码、没改别的文档。
- 2026-10-10：用户「Q1–Q10、P1–P8 全部按推荐，把草稿以 docs(plan) 一笔提交到 backup-store-v1」；本计划单独一笔提交，只加这一个文件，没改代码。下一步按 Q10 开 `backup-store-v2`、做 S1。
