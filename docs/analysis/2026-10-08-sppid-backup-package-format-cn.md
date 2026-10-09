# SmartPlant P&ID 备份包格式说明与 PID 文件清单（2026-10-08）

> 范围：SmartPlant Engineering Manager 生成的 plant 备份包（`<Plant>_p/`），以及包内的 `.pid` 文件清单。
> 样本：`test-file/backup-test/TEST02_p`（SQL Server 后端）与 `test-file/backup-test/DWG-0202GP06-01_p`（Oracle 后端），两者都有同级的 `<Plant>_p.zip` 原件。
> 口径：基于这两套样本的实测，不是 SmartPlant 官方规范。标“推断”的条目只有间接证据。
> 相关代码：`src/backup/{manifest,refdata,mtf,msci,mdf_page,zip_index,oracle_exp}.rs`，`src/backup/store/`（Backup Store），`src/bin/{pid_backup_probe,pid_backup_extract,pid_backup_store,pid_publish_xml}.rs`，`src/publish/store_load.rs`，`examples/oracle_exp_schema.rs`。
> 2026-10-09 起本文的数字由 Backup Store 的测试钉住（第 11 节）；第三套样本 SQPlant（Oracle，仓外）的事实见计划 `docs/plans/2026-10-09-a-plant-backup-becomes-one-backup-store.md` 的事实表。

## 一句话

备份包由一份 UTF-16LE 的 `Manifest.txt`、一份数据库 dump（`Export.dmp`）和若干按“选项 ID”命名的 `PlantData~2~*` / `RefData~4~*` 文件组成。
数据库 dump 随后端不同而格式完全不同：SQL Server 后端是 MTF 磁带格式，pid-parse 一路解到 MDF、逐行入库、再出 Publish XML；Oracle 后端是传统 `exp` 导出，pid-parse 读它的 DDL 把 154 张表按 `CONNECT` owner 登记下来（留空、标未解码），行解码排第二版。
`pid_backup_store` 把一套备份——Manifest 全部行、Database Dump 的表、包内文件清单——写成一个 SQLite（Backup Store，ADR-0004），publish 管线从它读。
图纸 `.pid` 在 `PlantData~2~711.zip` 里按 PlantGroup 层级存放，与库中 `T_Drawing.Path` 一一对应；模板 `.pid` 在 `RefData~4~682.zip`。

## 1. 样本概况

| 项 | `TEST02_p` | `DWG-0202GP06-01_p` |
|---|---|---|
| Plant 名（`Name`） | TEST02 | QSMCQTAZ13_PLANT |
| 描述（`Rootitem` 第 3 字段） | 空 | 沁水煤层气田安泽区块安13井区开发 |
| 数据库后端 | SQL Server 2008 R2（MSSQL10_50） | Oracle 12.1.0.2.0 |
| `Export.dmp` 格式 | MTF（`TAPE`） | Oracle `exp`（`EXPORT:V12.01.00`） |
| 备份时间（`DateCreated`） | 04/20/2026 12:06:10 | 04/25/2026 19:11:20 |
| 图纸数（`T_Drawing` 行数） | 1 | 7 |

pid-parse 另有两个 worktree（`pid-parse-m4-m5`、`pid-parse-pid-real-geometry-dto`）带着同一批样本副本，内容相同。

## 2. 包结构

一套备份是一个 `<Plant>_p/` 目录。`Manifest.txt` 里 `BackupType=2`、`Version=7.02`。

| 文件 | 识别方式 | 内容 |
|---|---|---|
| `Manifest.txt` | `FF FE`，UTF-16LE | 备份清单，见第 4 节 |
| `Export.dmp` | `TAPE` 或 `03 03 69` + `EXPORT:V12.01.00` | 4 个 schema 的数据库备份，见第 5 节 |
| `Export.log` | 文本 | 仅 Oracle 后端：`exp` 日志，含每张表的导出行数 |
| `PlantConfig.xml` | UTF-8 BOM XML | `<PlantConfiguration><ISO15926><Symbology>`：样式到线型的映射 |
| `PlantData~2~<id>[.zip]` | ZIP / XML | Plant 侧文件，含图纸 `.pid` |
| `RefData~4~<id>[.zip]` | ZIP / CFB / XML / ASCII | 参考数据：符号、模板、规则、报表、映射等 |

`<Plant>_p.zip` 是 SmartPlant 产出的原件，文件名与目录里的一致：TEST02 14 个，DWG 16 个（多出 `Export.log` 和 `PlantData~2~722`）。目录里额外的 `extracted/` 是 pid-parse 生成的，见第 9 节。

## 3. 文件命名与选项 ID

文件名规则：`<PlantData|RefData>~<schema 码>~<选项 ID>[.zip]`。

- schema 码：`2` = Plant，`4` = Reference Data。
- 选项 ID：对应 SmartPlant 选项表里的一条路径设置（推断：TEST02 的 722 在 Manifest 里的报错文字是 “Value was not set in option tables.”）。
- 带 `.zip`：源是目录，整目录打包。
- 不带 `.zip`：源是单文件，原样拷贝，扩展名丢失，只能按 magic 判断格式。例如 `RefData~4~703` 没有后缀，实际是 xlsx（ZIP）。`src/backup/refdata.rs` 的 `classify_format` 就是按前 4 字节分类的。

两套样本的选项 ID 对照（一致）：

| ID | 源路径 | 落盘形式 |
|---|---|---|
| 711 | Plant 根目录 | `PlantData~2~711.zip` |
| 714 | 备份目录 | 不打包（`File` 行的归档名为空） |
| 722 | `SmartPlant Resources\SPEMDataMap.xml` | Oracle 样本为 `PlantData~2~722`（XML）；TEST02 未设置 |
| 680 | `rules.rul` | 单文件，ASCII，开头 `"Begin Rules",120,…` |
| 681 | `Symbols\` | zip，TEST02 605 个 `.sym`，DWG 616 个 |
| 682 | `Template Files\` | zip，模板 `.pid` 与 `.igr` |
| 683 | `ProjectStyles.spp` | 单文件，CFB |
| 684 | `Report Files\` | zip，`.xlsm` 报表 |
| 685 | `Symbols\Assemblies\` | zip |
| 703 | `exportlayer.xlsx` | 单文件，OOXML |
| 709 | `InsulationSpec.isl` | 单文件，XML `<ProjectInsulationSpecifications>` |
| 804 | `SmartPlant Resources\` | zip，`SPPIDDataMap*.xml` 与 `CatalogIndex.mdb` |
| 809 | `Import Map Files\` | zip，映射 XML |
| 824 | `JacketNDCoreND.XML` | 两套都是 “Failed to access file.”，未打包 |

## 4. `Manifest.txt`

### 4.1 编码与行格式

- UTF-16LE，带 `FF FE` BOM。`backup::manifest::parse_manifest_bytes` 按 BOM 嗅探后解码。
- 每行 `Key<<|>>字段1<<|>>字段2…`，分隔符是 `<<|>>`（`FIELD_SEP`）。key 可以重复。
- TEST02 共 319 行，DWG 共 402 行；差额主要来自多一个角色的 78 条 `Right`。

### 4.2 单例 key

| Key | 示例 / 说明 |
|---|---|
| `BackupType` | `2` |
| `Version` | `7.02` |
| `DateCreated` | `MM/dd/yyyy HH:mm:ss` |
| `Name` | Plant 名 |
| `Spid` | 32 位 hex UID，等于 `Rootitem` 第 1 字段 |
| `BackupRefData` | `2` |
| `ProjectType` | `0` |
| `Serial_ID` | `yyMMddHHmmss`，同时是备份子目录名（DWG 的 `BackupCommand` 路径里可见） |
| `Rootitem` | 10 个字段：UID、名称、描述、UNC 路径、`Plant`、`1`、`1`、空、空、创建时间 |
| `PidIsAssociated` / `SpelIsAssociated` / `SPIIsAssociated` | `2` / `1` / `1` |
| `DbaExport` | `2` |
| `ExportFileSize` / `ArchiveFileSize` | 字节数 |
| `SlotCount` | `0<<|>>0`，含义未确认 |
| `DBUids` / `DBPwds` | `DBUids` 是四个 Plant schema 名的逗号串（Oracle 的 `BackupCommand` 里 `OWNER=(…)` 原样还有），`DBPwds` 是加密串——本文初稿把两者都写成「加密串」，2026-10-09 S2b 在三套样本上查证后改正。Backup Store 默认把两者都换成 SHA-256（Q15） |
| `BackupCommand` | SQL Server：`BACKUP DATABASE <库> TO <Plant>`；Oracle：完整的 `Exp.exe` 命令行 |
| `Characterset` / `OracleVersion` | 仅 Oracle：`AL32UTF8` / `12.1.0.2.0` |
| `TableSpace` | 仅 Oracle，两条（`Permanent` / `Temporary`），值是混淆串 |

### 4.3 连接信息：`SiteConnInfo`（2 条）与 `PlantConnInfo`（4 条）

每条 13 个字段：

| # | 含义 | TEST02 | DWG |
|---|---|---|---|
| 1 | `SiteConnInfo` 行：64 字符加密串；`PlantConnInfo` 行：Plant 名（TEST02 / DWG / SQPlant 分别 6 / 16 / 7 个字符；初稿写成「加密串」，S2b 查证后改正） | `TEST02` | `QSMCQTAZ13_PLANT` |
| 2 | schema 类型码 | 见下 | 见下 |
| 3 | 服务器名 / Oracle 服务名 | `MM-128` | `SPIDDB` |
| 4 | schema 名 / 用户名 | `TEST02`、`TEST02d`… | `QSMCQTAZ13_PLANT`… |
| 5 | 64 字符加密口令 | — | — |
| 6 | 登录名 | `sa` / `SEMSYS` | `SYSTEM` |
| 7 | 空 | | |
| 8 | 数据库类型 | `1` = SQL Server | `2` = Oracle |
| 9 | SQL Server 库名 | `SPIDSite` / `SP3DTrain_RDB_SCHEMA` | 空 |
| 10 | Oracle 默认表空间 | 空 | `QSMCQTAZ13_SITE_NEW` |
| 11 | Oracle 临时表空间 | 空 | `TEMP` |
| 12 | dump 文件名 | `Export.dmp`（Site 行为 `None`） | 同左 |
| 13 | log 文件名 | `Export.log`（Site 行为 `None`） | 同左 |

schema 类型码：`1` = Site，`7` = Site 字典，`2` = Plant，`8` = Plant 字典（`…d`），`4` = PID（`…pid`），`9` = PID 字典（`…pidd`）。

### 4.4 其余重复 key

- `DatabaseFiles`（仅 SQL Server）：逻辑名、文件号、物理路径、文件组、大小、上限、增长方式、`data only` / `log only`。
- `Table` / `View`：`Table<<|>>schema<<|>>表名`。两套完全一致：154 张表（plant 22、d 25、pid 82、pidd 25），35 个视图（d 25、pid 10）。
- `Role`：角色 UID、`0`、Windows 组、空、空。
- `Right`：角色 UID、`0`、`3`、权限 ID。每个角色 78 条。
- `File`：schema 码、类型（`1` = 目录打 zip，`2` = 单文件）、选项 ID、选项键（RefData 一律是 20000 + 选项 ID；PlantData 是 13087 / 13089 / 13296，含义未确认）、源路径、状态（`1` 成功，`2` 失败）、归档文件名或错误信息。
- `FileSize`：schema 码、选项 ID、文件数、目录数、原始总字节。TEST02 的 711 是 676 个文件加 106 个目录，正好等于 zip 的 782 个条目；DWG 的 711 是 8 + 4 = 12 个条目，也对得上。

## 5. `Export.dmp`

### 5.1 SQL Server 后端：MTF

`pid_backup_probe` 的实测结果（TEST02）：逻辑块 1024 字节，TAPE attributes `0x00030000`，os_id `0x010E`，共 9 个描述块。

| 偏移 | 描述块 | 大小 | 内含流 |
|---|---|---|---|
| `0x00000000` | `TAPE` | 1,024 | SPAD |
| `0x00000400` | `SFMB` | 512 | |
| `0x00000600` | `SSET` | 1,024 | SPAD |
| `0x00000A00` | `VOLB` | 19,929,088 | SPAD、`MSCI`、SPAD、`MSDA` |
| `0x01302200` | `MQDA` | 76,800 | SPAD × 7 |
| `0x01314E00` | `SFMB` | 512 | |
| `0x01315000` | `ESET` | 2,048 | `TSMP`、SPAD |
| `0x01315800` | `ESET` | 1,024 | SPAD |
| `0x01315C00` | `SFMB` | 512 | |

- `MSCI`（body 在 `0x0E10`，3,304 字节）：依次是 `MQCI`、`SCIN`、`SFGI`、`SFIN`（`+0x2F0`，MDF）、`SFIN`（`+0x7EC`，LDF）。文件组 `PRIMARY`，逻辑名 `SP3DTrain_RDB_SCHEMA_dat` / `SP3DTrain_RDB_SCHEMA_log`。
- `MSDA`（body 在 `0x1E10`，19,923,952 字节）：SQL Server 备份流。跳过开头 1,008 字节（`0x3F0`，`pid_backup_extract` 自动探测）后是 8 KB 页序列，即 `Export.mdf`（19,922,944 字节）。
- 4 个 schema（`TEST02`、`TEST02d`、`TEST02pid`、`TEST02pidd`）都在同一个库 `SP3DTrain_RDB_SCHEMA` 里。

pid-parse 的处理链：`pid_backup_extract --as-mdf` 导出 MDF → vendored `oxidized-mdf` 读表 → SQLite 暂存 → `pid_publish_xml` 生成 `_Data.xml` / `_Meta.xml`。

### 5.2 Oracle 后端：`exp` 导出

- 文件头 `03 03 69`，随后是 `EXPORT:V12.01.00\n`、`DSYSTEM\n`（导出用户）、`RUSERS\n`（按用户导出）等文本行。
- 导出参数来自 `BackupCommand`：`CONSISTENT=y OWNER=(…pidd, …d, …, …pid) GRANTS=y ROWS=y INDEXES=y CONSTRAINTS=y COMPRESS=y`。
- 字符集 AL32UTF8，NCHAR 字符集 AL16UTF16，因此 NVARCHAR 列的值在 dump 里是 UTF-16LE。
- 每张表依次是：`TABLE "X"`、`CREATE TABLE` DDL、`INSERT INTO "X" (列…) VALUES (:1, …, :n)`，然后是带长度前缀的二进制行。
- pid-parse 的 `pid_backup_extract` 识别到这个文件头就报错退出；`examples/oracle_exp_schema.rs` 只能扫 DDL。要拿行数据，要么用 Oracle 的 `imp` / `impdp` 还原，要么另写行解码器。本文的 DWG 图纸清单是直接按 UTF-16LE 扫 `T_DRAWING` 行区得到的。

`Export.log` 汇总：

| Owner | 表数 | 行数 |
|---|---|---|
| QSMCQTAZ13_PLANT | 22 | 502 |
| QSMCQTAZ13_PLANTD | 25 | 858 |
| QSMCQTAZ13_PLANTPID | 82 | 3,360 |
| QSMCQTAZ13_PLANTPIDD | 25 | 39,303 |

### 5.3 关键表行数

TEST02 的数字是 `Export.mdf` 里各表的存活行数（不含 ghost 记录，与 `sysrowsets.rcrows` 一致），DWG 的取自 `Export.log`。legacy 镜像 `extracted/Export_full.sqlite`（127 张表）把已删除、尚未清理的 ghost 记录也算成了行，所以它给出的 `T_PlantItem` 是 4、`T_Equipment` 是 2。

| 表 | TEST02 | DWG |
|---|---|---|
| `T_PlantGroup` | 3 | 4 |
| `T_Drawing` | 1 | 7 |
| `T_DrawingVersion` | 3 | 14 |
| `T_ModelItem` | 4 | 422 |
| `T_PlantItem` | 3 | 146 |
| `T_Representation` | 6 | 403 |
| `T_Relationship` | 3 | 284 |
| `T_PipeRun` | 1 | 50 |
| `T_Equipment` / `T_Vessel` / `T_Nozzle` | 1 / 1 / 1 | 4 / 1 / 10 |
| `T_Instrument` / `T_InlineComp` / `T_PipingComp` | 0 / 0 / 0 | 22 / 51 / 43 |
| `T_PipeLine` / `T_Note` / `T_SignalRun` | 0 / 0 / 0 | 15 / 16 / 2 |

## 6. `PlantData` / `RefData` 载荷

- `PlantData~2~711.zip` 就是 Plant 根目录。图纸按 PlantGroup 层级存成 `<区域>/<单元>/<图名>.pid`，路径与 `T_Drawing.Path` 一致（`\01\01\A01.pid`、`\zcgc\A3jqz\001.pid`）。另有 `SmartPlant Resources/SPEMDataMap.xml`。
- TEST02 的参考数据目录恰好放在 Plant 目录下面，所以它的 711 包还重复带了一整份 `ZGSY P&ID Reference Data`（782 个条目，605 个 `.sym`，11.5 MB）；DWG 的 711 包只有 12 个条目，283 KB。
- 各类载荷的格式：

| 类型 | 格式 | 备注 |
|---|---|---|
| `.pid`、`.sym`、`.igr`、`.spp` | OLE CFB（`D0 CF 11 E0`） | `.igr` 与同名模板 `.pid` 配对出现 |
| `rules.rul` | ASCII CSV | `"Begin Rules",120,…` |
| `.isl`、`SPPIDDataMap*.xml`、Import Map、`ItemTag*.xml`、`CatalogIndex.xml` | XML | |
| `exportlayer.xlsx`、报表 `.xlsm` | OOXML（ZIP） | |
| `CatalogIndex.mdb` | Access / Jet（`00 01 00 00 Standard Jet DB`） | `.ldb` 是它的锁文件 |
| `*.sav` | 同名文件的备份副本 | TEST02 的 711 包里 5 个 `.sav` 与原文件逐字节相同 |
| `PIDPipingMaterialCfg.ini`、`*.txt` | 文本 | `PipeChina_EquipmentType.txt` 是 GBK |

## 7. `.pid` 单文件格式

`.pid` 是 OLE CFB 复合文档。顶层 stream / storage 的布局、magic 和证据等级以 `2026-06-03-pid-file-format-analysis-cn.md` 为准，权威分级见 `2026-06-19-authoritative-pid-format-atlas.md`。与备份包相关的要点：

- `TaggedTxtData/Drawing` 里的图号、模板与库中 `T_Drawing.DrawingNumber` / `Template` 对应，但两边不一定一致（见第 8.1 节 DWG-0202GP06-01）。
- 模板 `.pid` 与图纸 `.pid` 是同一种容器格式，图纸由模板派生。

## 8. PID 文件清单

哈希列是 SHA-256 的前 12 位，用来区分同名不同版本。

### 8.1 图纸（与库中 `T_Drawing` 一一对应）

| Plant | 图名 | 包内路径（`PlantData~2~711.zip`） | 字节 | 模板 | SHA-256 前缀 |
|---|---|---|---|---|---|
| TEST02 | A01 | `01/01/A01.pid` | 106,496 | A2-W-New.pid | `8cccc7342c74` |
| QSMCQTAZ13 | 001 | `zcgc/A3jqz/001.pid` | 28,672 | CPECCHBA2-new.pid | `565c8293a4ba` |
| QSMCQTAZ13 | 002 | `zcgc/A3jqz/002.pid` | 131,072 | qsmcqtaz13A2.pid | `c8e61273fd62` |
| QSMCQTAZ13 | 222 | `zcgc/A3jqz/222.pid` | 98,304 | qsmcqtaz13A2.pid | `888f038c56e8` |
| QSMCQTAZ13 | DWG-0201GP01-01 | `zcgc/A3jqz/DWG-0201GP01-01.pid` | 479,232 | CPECCHBA2-new.pid | `75d59c75fcb3` |
| QSMCQTAZ13 | DWG-0201GP06-01 | `zcgc/A3jqz/DWG-0201GP06-01.pid` | 278,528 | XIONGANA2.pid | `c3903be43806` |
| QSMCQTAZ13 | DWG-0202GP06-01 | `zcgc/A3jqz/DWG-0202GP06-01.pid` | 360,448 | CPECCHBA2-new.pid | `eafe73905f4e` |
| QSMCQTAZ13 | DWG-0104GP01-08 | `zcgc/A4jqz/DWG-0104GP01-08.pid` | 81,920 | CPECCHBA2-new.pid | `1bd4c336c7b1` |

- TEST02 的 A01 SP_ID 为 `D9635C3C898840D1990B7E8BEE1D55DA`，`DocumentCategory=6`，`DocumentType=631`。
- DWG-0202GP06-01 在库里的 `DRAWINGNUMBER` 是 `DWG-0202GP06-02`，与图名不一致，是源数据本身的问题。

### 8.2 模板（`RefData~4~682.zip`，Template Files）

| 模板 | 字节 | SHA-256 前缀 | TEST02 | DWG |
|---|---|---|---|---|
| A0-W.pid | 65,536 | `5bcd2485cf68` | 有 | 有 |
| A0-W-New.pid | 118,784 | `bf68bc98b929` | 有 | 有 |
| A1-W.pid | 114,688 | `cf025e584c0d` | 有 | 有 |
| A1-W-New.pid | 57,344 | `299c5934c37b` | 有 | 有 |
| A2-W.pid | 61,440 | `a5ae6833a4a9` | 有 | 有 |
| A2-W-New.pid | 57,344 | `ca07fb6e45c0` | 有 | 有 |
| A3-W.pid | 57,344 | `7ac7a98878d2` | 有 | 有 |
| A3-W-New.pid | 102,400 | `586cce0139df` | 有 | 有 |
| CPECCHBA2.pid | 28,672 | `0fb6aa3e4861` | 有 | 有 |
| CPECCHBA2-new.pid | 28,672 | `70356068b7d2` | 有 | 有 |
| XIONGANA2（旧版） | 49,152 | `af33d7a22264` | `XIONGANA2.pid` | `XIONGANA2old.pid` |
| qsmcqtaz13A2 | 61,440 | `a7566c361665` | 无 | `qsmcqtaz13A2.pid`，以及被覆盖后的 `XIONGANA2.pid` |

- TEST02 有 11 个模板 `.pid` 和 10 个 `.igr`；DWG 有 13 个模板 `.pid` 和 11 个 `.igr`（多一个 `qsmcqtaz13A2.igr`）。去重后共 12 个模板内容。
- DWG 的 `XIONGANA2.pid` 已被换成与 `qsmcqtaz13A2.pid` 相同的内容，旧版另存为 `XIONGANA2old.pid`。所以库里 DWG-0201GP06-01 记录的模板名 `XIONGANA2.pid` 现在指向的是新内容。
- TEST02 的 711 包里还有同一套 11 个模板的副本，哈希与 682 包一致。

### 8.3 装配（`Symbols\Assemblies`）

只有 DWG 有：`Equipment/wuyouchi.pid`，208,896 字节，SHA-256 前缀 `1473297b3ef6`。`RefData~4~681.zip`（`Assemblies/Equipment/wuyouchi.pid`）和 `RefData~4~685.zip` 里各一份，内容相同。TEST02 的 685 包只有 3 个目录条目。

### 8.4 备份之外的独立 `.pid`（`test-file/` 下）

| 路径 | 字节 | SHA-256 前缀 | 与备份的关系 |
|---|---|---|---|
| `工艺管道及仪表流程-1.pid` | 450,560 | `a3cceb23bc50` | 不在任何备份里 |
| `D06.pid` | 118,784 | `eb0043a28e1b` | 不在任何备份里 |
| `DWG-0201GP06-01.pid` | 389,120 | `3e544acd94c6` | 与备份里的同名图（278,528）不是同一版本 |
| `DWG-0202GP06-01.pid` | 249,856 | `bd0900562b47` | 与备份里的同名图（360,448）不是同一版本 |
| `export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid` | 249,856 | `18ce4808cb57` | 与上一行同尺寸但哈希不同；连同备份版，这张图共 3 个版本 |
| `export-test/publish-data/A01/A01.pid` | 106,496 | `8cccc7342c74` | 与备份里的 A01.pid 是同一文件 |

合计：两套备份内去重后 21 个 `.pid` 内容（8 张图纸、12 个模板、1 个装配），`test-file/` 下另有 5 个不在备份里的内容，总计 26 个不同的 `.pid`。

## 9. 注意事项

- **字节级比对请用 `<Plant>_p.zip` 原件。** 仓库 `.gitattributes` 是 `* text=auto eol=lf`，目录里的文本文件在检出时被转成了 LF：

  | 文件 | zip 原件 | 目录副本 |
  |---|---|---|
  | `PlantConfig.xml` | 5,085 | 4,995 |
  | `RefData~4~680` | 332,275 | 317,903 |
  | `RefData~4~709` | 27,958 | 27,957 |
  | `Export.log` | 15,715 | 15,459 |
  | `PlantData~2~722` | 12,903 | 12,618 |

  `Manifest.txt`（UTF-16）和二进制文件不受影响。
- `TEST02_p/extracted/` 是 pid-parse 生成的派生文件（`Export.msci.*`、`Export.msda.bin`、`Export.mdf`、`Export*.sqlite`、生成的 XML），不是 SmartPlant 原始内容。
- DWG 样本没有 `extracted/`，Oracle 后端也走不通 MTF 这条链，所以 README 里提到的 `DWG-0202GP06-01_p/extracted/Export.mdf` 实际缺失，相关测试会 soft-skip。
- **敏感信息**：Oracle 样本的 `BackupCommand` 含 `SYSTEM` 账号的明文口令；`DBPwds`、`SiteConnInfo` 的第 1 字段和全部连接信息的第 5 字段是加密串（`DBUids` 和 `PlantConnInfo` 的第 1 字段不是，见 4.2 / 4.3）。这类备份外发前需要脱敏：`pid_backup_store` 默认把 `DBUids`、`DBPwds`、连接信息第 1、5 字段换成 SHA-256、`BackupCommand` 的口令换成 `***`（Q15，每处记进 `store_redaction`），`--keep-secrets` 才原样存。本文没有转录任何口令或加密串。

## 10. 未确认项

- `File` 行 PlantData 侧第 4 字段（13087 / 13089 / 13296）的含义。
- `SlotCount` 两个字段的含义。
- `TableSpace` 值的编码方式。
- `Right` 行第 3 字段（恒为 `3`）以及权限 ID 与功能的对应关系。
- `SiteConnInfo` 第 1 字段（64 字符加密串）承载的内容（`PlantConnInfo` 的第 1 字段已查证是 Plant 名，见 4.3）。
- 选项 ID 与 SmartPlant 选项表的对应关系目前只有间接证据。
- Oracle `exp` 行编码的完整规则（列长度前缀、NULL 标记、NUMBER / DATE 编码）尚未实现解码；Backup Store 第一版只按 DDL 登记空表（`dump_table.decoded = 0`）。
- SQL Server 侧：`TEST02pid.T_Symbol.SP_ID` 声明 NOT NULL，页 2300 槽 1 那条记录的空位图却把它置位（变长区存着 32 字符的 UID，定长区是指针模样的字节）；SQL Server 自己读这列会不会跳过空位图没法验，Backup Store 按空位图写 NULL（S2c）。

## 11. 复现方法

本文的数字（文件数与 SHA-256、Manifest 行数、154 张表、37,470 行、Ghost Row 5、LOB 4、空串 162、NULL 50,596、Oracle 的 2,039 / 2,024 列……）由 Backup Store 的测试钉住，跑一遍就是复核：

```bash
# SQL Server 样本 TEST02：读取器（S1）与 store（S2）
cargo test --test backup_mdf_reader_test02 --test backup_store_test02
# Oracle 样本 DWG（仓内）与 SQPlant（仓外，PID_PARSE_SQPLANT_BACKUP 或 D:\work\cad\pid-test-data，缺则跳过）
cargo test --test backup_store_dwg --test backup_store_sqplant
# 命令行与 publish：pid_backup_store 的汇总、A01 从 MDF / zip / 目录 / store 文件出同样的字节
cargo test --test backup_store_cli --test publish_store_parity --test publish_xml_cli
```

手工看一套备份：

```bash
# 整套备份 → 一个 SQLite（Backup Store；输出已存在加 --force；--keep-secrets 不脱敏；--embed-files 连文件字节也存）
cargo run --bin pid_backup_store -- test-file/backup-test/TEST02_p.zip -o TEST02.sqlite
cargo run --bin pid_backup_store -- test-file/backup-test/DWG-0202GP06-01_p.zip -o DWG.sqlite
# 库里：store_info / backup_file / manifest_* 与视图 / dump_schema / dump_table / dump_column / dump_view /
# <角色>__<表>（SQL Server 逐行带 _src_page、_src_slot；Oracle 空表 decoded = 0）/ dump_ghost_row / dump_lob

# Publish：输入可以是 zip、目录、store 文件或 Export.mdf，出的 XML 相同
cargo run --bin pid_publish_xml -- test-file/backup-test/TEST02_p.zip --list-drawings
cargo run --bin pid_publish_xml -- TEST02.sqlite --drawing D9635C3C898840D1990B7E8BEE1D55DA --plant TEST02 --out A01_Data.xml --meta-out A01_Meta.xml

# 更底层的探针
cargo run --bin pid_backup_probe -- test-file/backup-test/TEST02_p/Export.dmp          # MTF 描述块、流和 MSCI 摘要
cargo run --bin pid_backup_extract -- test-file/backup-test/TEST02_p/Export.dmp --out <dir> --as-mdf --dry-run   # 剥离 MTF 得到 MDF
cargo run --bin pid_backup_extract -- test-file/backup-test/DWG-0202GP06-01_p/Export.dmp --out <dir> --dry-run   # Oracle：识别为 exp 导出并报错
cargo run --example oracle_exp_schema -- test-file/backup-test/DWG-0202GP06-01_p/Export.dmp   # Oracle DDL 按 owner 分组打印
```

库里的入口：`backup::build_backup_store` / `build_backup_store_in_memory`（备份）、`build_backup_store_from_mdf_in_memory`（单独的 MDF）；`backup::store::reassemble_manifest` 从库拼回 `Manifest.txt`；`backup::manifest::parse_manifest_bytes` 读 Manifest；`backup::zip_index::list_zip_entries` 列 zip 条目；`backup::refdata::scan_refdata_dir` 分类 `RefData~*`；`backup::oracle_exp::scan_create_tables` 扫 Oracle DDL；`publish::open_publish_input` 把任一输入开成 publish 读的表。
