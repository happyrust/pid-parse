# publish 导出的「clean」是「没写错」，不是「写全了」（2026-11-10）

> 基线：`main` = `803984a`。工作在 worktree `D:\work\plant-code\cad\pid-parse-wt\publish-export`、分支 `publish-export-20261110` 上做，主检出只读。
> 方法：构建后跑全部 `tests/publish_*.rs`（`--nocapture --test-threads=1`），再用同一份二进制对每张有原厂参考的图、每种适用的输入方式跑 `--diff-against`，把打印出的每一行与 `docs/baselines/publish-diff-accepted.json` 对上；最后另写一遍「参考里有多少、我方写出多少」的普查，因为 `--diff-against` 在 clean 时不打印被豁免的量。
> 标「已核实」的是本轮亲自跑到或读到的；标「推断」的没有被数据钉死。配套方案：`docs/plans/2026-11-10-publish-export-completion.md`。

## 一句话

13 张有原厂参考的图里，7 张在 `--oracle-exp` 模式下写出的对象、关系与参考 100 % 对上、写出的值 0 错；但这 7 张图参考里的接口元素只写出 **56.5–58.1 %**、非空属性值只写出 **49.6–51.8 %**，其余的被门禁的豁免清单整类放过，所以 exit 0。另外有三件事此前没有任何测试或基线看着：仓库里唯一一个「图纸 + Oracle 备份包 + 参考」三件齐全的夹具 `DWG-0202GP06-01`，备份包根本读不进来（`T_PIPERUN` 一个小数就拒）；它的 `.pid` 单发有 2 个值写错；数据库线（`Export.mdf`）的命令行门禁只数标签个数。

## 1. 「正确」在这个仓库里是怎么定义的

### 1.1 比什么

| 输入方式 | 门禁 | 比的内容 |
|---|---|---|
| `.pid` 单发；`.pid` + `--oracle-exp`（± `--datamap`） | `diff::diff_pid_export`（`src/publish/diff.rs:1532`） | **写出侧**：每个对象的 UID 与类；每个**非空**属性值逐字相等；每条 `<Rel>` 的 `(UID, UID1, UID2, DefUID)` 四元组。`PIDPipeline` 与 `PipingConnectors` 按 `IObject.Name` 配对（管线 UID 是合成的）。**参考侧**（`diff_exclusions.rs`）：参考有、我方没有的对象 / 值 / 关系记 `missing`，除非落在五类豁免里 |
| 同上，且本次写出了 `_Meta.xml` | `diff::diff_meta_xml`（`diff.rs:2211`） | `<Container>` 13 个属性、六个块的次序、3 条 `Rel` 的 `DefUID` 与两端角色、`DocUID` 等于同名 `_Data.xml` |
| 备份包目录（Oracle `exp` 或 SQL Server MTF） | `backup_package::PackageGate` | 先比两侧 `<Container DocUID>`，不同记 `revision mismatch`；相同则逐图走 `diff_pid_export`。参考目录里没有同名文件记 `unpaired` |
| `Export.mdf` / `Export_v2.sqlite` + `--drawing` | `diff_publish_xml` + `diff_rel_defuids`（`pid_publish_xml.rs:974`） | **只比个数**：每种 `<PIDxxx>` 标签的个数、每种 `Rel DefUID` 的个数 |
| 同上，测试层 | `publish_{interface,attribute,rel}_parity.rs`、`publish_a01_raw_residual.rs` | 接口集合、属性名集合「我方 ⊇ 参考」；A01 整份文件在遮掉三类合成值之后逐字节相同 |

退出码：0 = 写出且（带 `--diff-against` 时）clean；1 = 有发现项或读写失败；2 = 用法错误，含 `--oracle-exp` 的 dump 里没有这张图的 `T_DRAWING` 行。

### 1.2 和哪些参考比

本机一共 13 份原厂 `_Data.xml`（各带 `_Meta.xml`）：

| 参考 | 位置 | 对应的库 | `.pid` 与库是否同一版 |
|---|---|---|---|
| A01 | `test-file/export-test/publish-data/A01`（入库） | `test-file/backup-test/TEST02_p`，SQL Server MTF，另有解出的 `Export.mdf` / `Export_v2.sqlite` | 参考的 `GraphicOID` 与连接件 UID 是发布时的值，库里没有（`publish_a01_raw_residual.rs` 的三类残差） |
| DWG-0202GP06-01 | `test-file/export-test/publish-data/DWG-0202GP06-01`（入库） | `test-file/backup-test/DWG-0202GP06-01_p`，Oracle `exp` | **不是**：`.pid` 与参考同版，备份包是后来的状态（§3.3） |
| I井场、J K、L M、轻烃分离器I、空冷器 A / B / C（7 张） | `D:\work\cad\pid-test-data\验收数据` | `D:\work\cad\pid-test-data`，Oracle `exp`，53 张图 | 是（`T_DRAWING` 有行） |
| 凝析油 A / B、换热器 CD、低温分离器 A（4 张） | 同上 | 同一个包里没有这 4 个 `DocUID` | 没有同版的库，只能 `.pid` 单发 |

`D:\work\cad\PID自研软件验证` 里的 5 张图（I井场 + 上面 4 张）与 `验收数据` 里的同名文件逐字节相同（15 / 15，已核实），不是另一套参考。

### 1.3 不比什么，为什么

已核实，出处都是 `diff.rs` / `diff_exclusions.rs` / `catalog.rs`：

1. **接口元素本身不比。** `read_publish_document` 只把 `接口.属性 = 值` 收进一张表，`<IDrawingItem/>` 这样没有属性的元素不留痕迹。参考每个对象带 2–26 个接口元素，我方多数对象只有 `IObject` 加一两个带值的接口。
2. **空值属性不比。** 两侧的 `X=""` 在比较前被丢掉（`values.retain(|_, value| !value.is_empty())`）。7 张图的参考共有 16 680 个空值属性，我方一个都不写。
3. **`_Data.xml` 的 `<Container>` 属性不比**（`_Meta.xml` 的比）。实测 13 张图两侧一致，但没有门禁保证。
4. **同一 UID 写两遍不比。** 对象按 UID 进 `HashMap`，后写的覆盖先写的。
5. **块、接口、属性的次序不比。** 这是有意的：比较是语义集合。
6. **41 对 `(接口, 属性)` 整类豁免**（`catalog.rs` 的 `EXCLUDED_ATTRIBUTE_CLASSES`，8 个类）。理由写的是「只有工厂数据库有」。这条理由在 `.pid` 单发时成立；`--oracle-exp` 与整包模式下数据库就在手里，同一张清单照样豁免。
7. **参考侧整类对象豁免**：本次没发出任何该类对象、且目录说该类来自 dump 时不算 `missing`（`.pid` 单发丢掉端口、管线、OPC 即属此类）；`--oracle-exp` 下这条豁免已被收紧为发现项。
8. **厂商拼的对象名**：我方发了对象但没给 `IObject.Name` 时，参考的名字不算 `missing`（管段、管件、在线仪表）。
9. **管线的 `FluidSystem`**：参考为空、dump 有值时不算错，按参考 `DocUID` 登记上界（`FLUID_SYSTEM_REFERENCE_EMPTY_ALLOWANCES`）。
10. **`_Meta.xml`** 的 `DocVersionDate`、`FilePath` 与 6 个 UID 登记为接受的差异（2026-10-14 已批，库与 `.pid` 里都没有这些值）。
11. **数据库线**在命令行上除个数外什么都不比。

第 1、2、6 条合起来就是「clean 而不全」的来源。

## 2. 实测

### 2.1 测试：哪些真跑，哪些软跳过

`803984a` 上 14 个文件全部退出 0：103 条通过、2 条 `#[ignore]`（`publish_a01_raw_residual` 的两条手工探针）。逐条看 `--nocapture` 输出：

| 文件 | 通过 | 其中软跳过 | 说明 |
|---|---:|---:|---|
| `publish_a01_raw_residual` | 4 | 0 | 另有 2 条 ignored |
| `publish_attribute_parity` | 5 | 0 | |
| `publish_backlog_inventory` | 4 | 0 | |
| `publish_diff_accepted_baseline` | 15 | 0 | 语料都在本机，90 s |
| `publish_dwg_generated_parity` | 4 | **4** | 全部等 `DWG-0202GP06-01_p/extracted/Export.mdf` |
| `publish_dwg_mirror` | 5 | **5** | 同上 |
| `publish_from_pid` | 16 | 0 | 含 53 张整包，91 s |
| `publish_interface_parity` | 5 | 0 | |
| `publish_mdf_load` | 3 | 0 | |
| `publish_meta_parity` | 6 | **2** | DWG 两条 |
| `publish_rel_parity` | 9 | **3** | DWG 三条 |
| `publish_sqlite_load` | 4 | 0 | |
| `publish_writer_coverage` | 2 | 0 | |
| `publish_xml_cli` | 21 | 0 | |

**这 14 条 DWG 软跳过永远跑不起来**：它们等的是 `DWG-0202GP06-01_p` 解出来的 `Export.mdf`，而这个包的 `Export.dmp` 是 Oracle `exp`（文件头 `EXPORT:V12.01.00`，`tests/backup_mtf.rs:17` 也把它叫 `ORACLE_EXPORT_DMP`），不是能剥出 MDF 的 MTF。`git log --all` 里这个路径从未出现过。所以 DWG 参考至今没有被任何真跑的断言比过。

在 CI 上：A01 与两个备份包夹具在仓库里，对应的测试真跑；I井场、验收数据、53 张整包都读本机目录，缺了就软跳过。

本阶段之后 `publish_from_pid` 是 19 条（多 3 条 DWG 用例，只读仓库内夹具），其余文件条数不变。

### 2.2 对照矩阵

`803984a` 的二进制，输出在 `target/publish-audit/runs/base`。「已登记 / 未登记」是把打印的每一行与基线逐字比对的结果，没有一张图出现「已登记而不再打印」。

| 图 | 模式 | 退出码 | 写出 对象 / 值 / 关系 | 打印的差异 | 与基线 |
|---|---|---:|---|---|---|
| A01 | `.pid` | 1 | 8 / 10 / 7 | 27 | 27 已登记，0 未登记 |
| A01 | `Export.mdf` | 0 | 标签 8 种 12 个、`Rel` 8 种 15 条全对上 | — | 只比个数 |
| A01 | `Export_v2.sqlite` | 0 | 与 MDF 逐字节相同 | — | 只比个数 |
| A01 | `TEST02_p` 目录 | 1 | 12 / 18 / 15 | 27 | **基线里没有这一项** |
| DWG-0202GP06-01 | `.pid` | 1 | 78 / 71 / 81 | 20（2 值错 + 18 值缺） | **基线里没有这张图** |
| DWG-0202GP06-01 | `--oracle-exp` | 1 | 读 dump 失败 | — | 同上 |
| DWG-0202GP06-01 | `DWG-0202GP06-01_p` 目录 | 1 | 读 dump 失败，0 张写出 | — | 同上 |
| I井场 | `.pid` / `--oracle-exp` | 0 / 0 | 1735 / 1530 / 1971；3093 / 3346 / 4107 | 0 | clean 分支 |
| J K井场 | `.pid` / `--oracle-exp` | 1 / 0 | 2605 / 2296 / 2955；4655 / 5053 / 6175 | 6 / 0 | 6 已登记 |
| L M井场 | `.pid` / `--oracle-exp` | 0 / 0 | 3068 / 2720 / 3474；5493 / 5996 / 7274 | 0 | 一致 |
| 轻烃分离器I | `.pid` / `--oracle-exp` | 0 / 0 | 667 / 578 / 789；1219 / 1292 / 1685 | 0 | 一致 |
| 空冷器 A | `.pid` / `--oracle-exp` | 0 / 0 | 695 / 613 / 780；1260 / 1413 / 1652 | 0 | 一致 |
| 空冷器 B | `.pid` / `--oracle-exp` | 0 / 0 | 699 / 615 / 784；1248 / 1384 / 1636 | 0 | 一致 |
| 空冷器 C | `.pid` / `--oracle-exp` | 0 / 0 | 690 / 607 / 775；1229 / 1358 / 1614 | 0 | 一致 |
| 凝析油 A / B、换热器 CD、低温 A | `.pid` | 0 | 353 / 326 / 397；283 / 256 / 318；713 / 636 / 828；374 / 347 / 415 | 0 | 基线只在 `revision_mismatch_figures` 里记了它们 |
| 同上 4 张 | `--oracle-exp` | 2 | 拒绝，不写文件 | — | 与登记一致 |
| 53 张整包 | 目录 → `验收数据` | 1 | 53 张写出，0 失败 | 7 clean、4 另一版、42 无参考 | 与文档一致 |

MTF 包那 27 条与 `.pid` 单发的 27 条只是个数碰巧相同：4 个多发对象（连接件 `…-CNX` 及其两个端口、工艺点）、4 个缺的、4 处 `GraphicOID`、15 条 `Rel` UID。正是 `publish_a01_raw_residual` 遮掉的三类，但走包门禁时没有遮，也没有登记。

### 2.3 没有参考可比的

- 53 张整包里的 42 张：只有「53 张写出、0 失败、逐图计数与 `_Index.json` 自洽、重复运行逐字节相同」这几条在保证。其中 C井场 的 27 个只在 dump 里的管件是照 J K 的 3 例外推的。
- `DWG-0202GP06-01_p` 里另外 6 张图。
- MTF 整包只有 A01 一张图。

### 2.4 完整度普查

这是本轮新增的口径：对参考里的每个对象（管线按名配对），数它的接口元素、非空属性值、关系有多少在我方文件里（值要逐字相等，关系按两端与 `DefUID`、不看 `Rel` 自己的 UID）。下表出自当时的脚本 `target/publish-audit/census.py`（不入库）。入库的探针是 `examples/probe_how_much_of_a_reference_does_a_publish_state.rs`（方案 S0），口径相同，只有一处更严：对象要同 UID 且同类才算写出。因此 A01 · `.pid` 那一行探针读作 6 / 12、9 / 105——脚本把那个已登记的错类对象（参考 `PIDPipeline` 的 UID 下我方写的是 `PIDPipingConnector`）算作写出；其余各行两者相同。

| 图 · 模式 | 对象 | 接口元素 | 非空值 | 值错 | 关系 |
|---|---|---|---|---:|---|
| I井场 · exp | 3093 / 3093 | 12944 / 22930（56.5 %） | 3323 / 6702（49.6 %） | 0 | 4107 / 4107 |
| J K · exp | 4655 / 4655 | 19520 / 34527（56.5 %） | 5019 / 10034（50.0 %） | 0 | 6175 / 6175 |
| L M · exp | 5493 / 5493 | 23087 / 40755（56.6 %） | 5954 / 11847（50.3 %） | 0 | 7274 / 7274 |
| 轻烃分离器I · exp | 1219 / 1219 | 5176 / 8906（58.1 %） | 1292 / 2574（50.2 %） | 0 | 1685 / 1685 |
| 空冷器 A · exp | 1260 / 1260 | 5440 / 9446（57.6 %） | 1410 / 2733（51.6 %） | 0 | 1652 / 1652 |
| 空冷器 B · exp | 1248 / 1248 | 5285 / 9293（56.9 %） | 1381 / 2665（51.8 %） | 0 | 1636 / 1636 |
| 空冷器 C · exp | 1229 / 1229 | 5194 / 9126（56.9 %） | 1355 / 2616（51.8 %） | 0 | 1614 / 1614 |
| I井场 · `.pid` | 1735 / 3093（56.1 %） | 3559 / 22930（15.5 %） | 1530 / 6702（22.8 %） | 0 | 1971 / 4107（48.0 %） |
| 另外 10 张 · `.pid` | 54–62 % | 14.5–18.0 % | 22.4–24.1 % | 0 | 46–52 % |
| DWG · `.pid` | 78 / 108（72.2 %） | 163 / 747（21.8 %） | 69 / 236（29.2 %） | **2** | 81 / 133（60.9 %） |
| A01 · `.pid` | 7 / 12 | 10 / 105 | 1 / 29 | 3 | 0 / 15 |
| A01 · `Export.mdf` | 8 / 12 | 63 / 105 | 14 / 29 | 4 | 9 / 15 |

A01 的数据库线看起来低，是因为连接件、两个端口、工艺点 4 个对象的 UID 是参考的合成值，配不上；配上的 8 个对象接口与属性是全的（`publish_a01_raw_residual` 的逐字节断言）。

7 张图 `--oracle-exp` 合计，参考里有 107 种接口，其中 75 种我方从不写。缺得最多的值：

| 参考里的值（7 张合计） | 缺 / 参考 | 库里的来源（推断，列名取自 `T_*` 的 `INSERT` 列表） |
|---|---|---|
| `IPBSItem.ConstructionStatus` / `ConstructionStatus2` | 2782 / 2782，各 | `T_PLANTITEM.CONSTRUCTIONSTATUS` + 枚举映射 |
| `IPIDTypical.IsTypical` | 2754 / 2754 | `T_MODELITEM.SP_ISTYPICAL` |
| `IObject.Name`（厂商拼名） | 1077 / 9846 | `T_PLANTITEM.ITEMTAG` |
| `IConnector.RepresentationsAreAllZeroLength`、`IPipingConnector.PipingConnectorType` | 1013 / 1013，各 | `T_PIPERUN.SP_CONNECTORSZEROLENGTH`、`PIPERUNTYPE` |
| `IInlineComponent.IsFlowDirectional` | 980 / 980 | `T_INLINECOMP` |
| `IPipingComponent.*`（5 个属性） | 3493 / 3493 | `T_PIPINGCOMP` + 层级枚举 |
| `IPipingSpecifiedItem.PipingMaterialsClass` | 514 / 799 | `T_PIPERUN.PIPINGMATERIALSCLASS`（现在只从标签取） |
| `IInsulatedItem.InsulSpec` | 470 / 616 | `T_PIPERUN.INSULATIONSPEC`（同上） |
| `IPipeCrossSectionItem.NominalDiameter` | 438 / 2386 | `T_PIPERUN` / `T_INLINECOMP.NOMINALDIAMETER` |
| `INamedPipingConnector.PipingConnectorSeqNo` | 398 / 631 | `T_PIPERUN.TAGSEQUENCENO` |
| `IInstrumentOcc.InstrumentLocation`、`IInstrument.*`、`IConnector.FlowDirection`、`IEquipment.EqType*`、`IEquipmentComponent.*` | 1526 / 1526 | `T_INSTRUMENT`、`T_EQUIPMENT`、`T_NOZZLE` + 枚举 |

## 3. 差距归因

### 3.1 读不进来（已修，S1）

`ExpTables::read` 对 `T_PIPERUN`、`T_PIPINGPOINT`、`T_SIGNALPOINT`、`T_SIGNALRUN`、`CODELISTS` 用的是严格读法：任何 `NUMBER` 不是整数就整表拒绝。`DWG-0202GP06-01_p` 的第一条管段保温 50 mm，`SP_INSULTHICKSI = 0.05`，于是报 `T_PIPERUN: row 0 NUMBER at 0x7a4c07 is not an integer`，`--oracle-exp` 与整包两种模式都退出 1、一个文件不写。53 张那个包没有踩到，是因为它这五张表里没有一个非整数的 `NUMBER`（严格读法读得通）。归类：**读错**（读法过严），与 publish 用不用这一列无关：`T_PIPERUN` 有 293 列，publish 只跟其中 7 列。

更正（2026-11-10，独立审查指出）：上一段原先写的是「第一条管段 `SLOPE = 0.3`」，`80a5f73` 的提交说明、代码注释与 CHANGELOG 也这样写，错了。偏移 `0x7a4c07` 上是第 0 行 `SP_INSULTHICKSI` 的 `c0 06`（0.05）。`SLOPE` 是字符列，值是 `0.3 %`，在第 21、28 行；它的数值列 `SP_SLOPESI` 是 `0.0029999910000486`。这张表一共 10 个非整数的 `NUMBER`：`SP_INSULTHICKSI` 8 个、`SP_SLOPESI` 2 个，另外四张表（`T_PIPINGPOINT`、`T_SIGNALPOINT`、`T_SIGNALRUN`、`CODELISTS`）一个都没有。用自己写的读法复核过（`target/publish-audit/review2/unread_cells.py`）。这两列正是 DWG 那 17 个缺值里 7 个的来源：6 个 `TotalInsulThick "50 mm"` 和 1 个 `SlopedPipingAngle "2.9999910000486E-03 rad"`。

### 3.2 写错（已修，S2 / S3 / S3b）

`DWG-0202GP06-01` 的 `.pid` 单发有两个值与参考不同：

- 容器 `F1081777…`：我方 `IObject.Description = "新建"`，参考是 `"污水池"`，而 `"新建"` 在参考里是 `ISpecifiedMatlItem.LongMaterialDescription`。原因在数据映射：三个备份包里的 `SPPIDDataMap.xml` 是同一份文件（SHA-256 前 12 位 `248c6a3b4bd7`，`SoftwareVersion 10.00.36.0023`），它写着 `SPMP_Vessel_Description → LongMaterialDescription`、`SPMP_Vessel_Name → Description`。换热器同 Vessel，`EquipmentOther` 的 `Description → LongMaterialDescription3`，`Mechanical` 与 `ItemNote` 的 `Description` 没有去向。`pid_load::published_attribute` 把任何类的 `Description` 标签都写到 `IObject.Description`。7 张井场 / 集注站图没暴露，是因为它们的设备上没有 `Description` 标签（我方在设备类上发出的 `Description` 为 0，已核实）。
- 注释 `6A2C6590…`：我方 `NoteText` 的换行是 `\r`，参考是 `\r\n`。dump 的 `T_NOTE.BODY` 是 `\r\n`（已核实，UTF-8 字节 `0D 0A`），标签缓存的是文本框的写法，只剩 `\r`。13 份参考里带换行的注释只有这一条，规则「标签里落单的 `\r` 就是库里的 `\r\n`」是 **1 例 + 机制**，不是多例统计。

归类：**读对了、写到了错的位置 / 写法不同**。

还有一处是门禁看不见、普查看得见的：没有 `ToFromText` 的 OPC，我方写了一个空的 `<IOffpageConnector/>`，参考对象没有这个元素。13 份参考的 142 个 `PIDBranchPoint` 里 123 个带 `ToFromText`、19 个没有这个元素、0 个写空的；我方在轻烃分离器I 上多写 14 个、DWG 上 4 个，是 7 + 1 张图里唯一「我方有、参考对象没有」的接口元素（已修，S3b）。

**这三条规则各自站在多少样本上**（独立审查提出；本轮用自己的脚本重数，`target/publish-audit/review2/item9_samples.py`，结论相同）：

| 规则 | 有的样本 | 没有样本的部分 |
|---|---|---|
| `Description` 标签按数据映射发 | 13 张参考图上挂到对象的 `Description` 标签一共 3 个：DWG 的容器 1 个（参考放在 `LongMaterialDescription`），轻烃分离器I 的管件 2 个（参考在 `IObject.Description`）。库侧的旁证：7 张同版图里 34 个 `PIDProcessEquipment` 的 `T_MODELITEM.DESCRIPTION` 全部（34 / 34）等于参考的 `LongMaterialDescription3`；设备的 `T_PLANTITEM.NAME` 等于参考的 `IObject.Description`（`PIDProcessEquipment` 17 / 17、`PIDMechanicalEquipment` 6 / 6、DWG 的容器 1 / 1） | 换热器、机械设备、注释三条分支是 0 例：这三类参考对象在库里的 `DESCRIPTION` 全空（3、15、178 个对象），53 张整包的 `.pid` 上这几类也没有 `Description` 标签。只有数据映射作证，没有反例 |
| 落单的 `\r` 写成 `\r\n` | 1 例，DWG 的注释 `6A2C6590…`。13 份参考里带换行的属性值一共 2 个，另一个是同一张图的 `IDocument.DocTitle`（也是 `\r\n`，我方不写这个属性）。两个 dump 的 `T_NOTE.BODY` 非空 481 条（8 + 473），带换行的只有这一条 | 落单的 `\n` 原样保留，没有样本；别的文本属性不套这条规则，也没有样本 |
| 没有 `ToFromText` 的 OPC 不写 `IOffpageConnector` | 142 个 `PIDBranchPoint`：123 个带 `ToFromText`，19 个没有这个元素，0 个写空的 | 那 19 个只来自 2 张图（轻烃分离器I 14 个、DWG 5 个） |

### 3.3 版本差（修不了，要数据）

- **DWG 的备份包比参考新。** 参考里 `C7C3E092…` 的 `PipingConnectorSeqNo` 是 `0301`、名字 `A3jqz0301-OD-50 mm-1.6AR12-WE-50mm`；包里 `T_PIPERUN.TAGSEQUENCENO` 是 `565`、`T_PLANTITEM.ITEMTAG` 是 `OD-50 mm-03-565-1.6AR12-WE50mm`。包内 `.pid` 的 `GraphicOID` 也与参考不同（整包门禁报 100 条）。所以 DWG 的 `--oracle-exp` 对照里，6 条多发管线、5 条缺的管线、6 条 `PipingConnectors` 是版本差。
  同一份数据还说明了一件事：这个库的 `T_PIPELINE` 有 15 行，参考的 5 条 `PIDPipeline` 的 UID 与名字就是其中 5 行的 `SP_ID` 与 `T_PLANTITEM.ITEMTAG`（5 / 5，已核实）。53 张那个库 `T_PIPELINE` 是 0 行，管线 UID 只能合成。所以「有 `T_PIPELINE` 行时用它的 UID」是对的方向，但哪条管段属于哪条管线在这份 dump 里对不出参考那一版（`T_PLANTITEM.SP_PARTOFID` 50 条管段全空），要同版的数据才能验证。
- **4 张旧集注站图**的库不在手里（2026-11-05 已定性）。
- A01 的 `GraphicOID`、连接件 UID、`Rel` UID 是发布时的值，库里搜不到（`publish_a01_raw_residual` 的两条 ignored 探针）。

### 3.4 没写（能修，是主要工作量）

接口元素缺 43 %、非空值缺 50 %，全部被 §1.3 的 1、2、6 条放过。两类：

- **类的固定接口表。** 13 份参考、21 355 个对象、21 个类里，12 个类的接口序列只有一种写法，其余 9 个类只在 1–11 个可选接口上有出入（`IInsulatedItem`、`INamedPipingConnector`、`IPipingSpecifiedItem`、`ISlopedPipingItem`、`IOffpageConnector`、`INamedEquipment`、`ISpecifiedMatlItem` 与设备子类接口）。也就是说「这个类带哪些接口」基本是常量，可以照参考量成一张表。`SPPIDDataMap.xml` 的 `ClassToMapClass` 只说得出 11 张验收图参考里 157 773 个接口元素中的 10 898 个（6.9 %），不能当出处。
- **库里的属性。** 同一份 `SPPIDDataMap.xml` 的 `MapPropertyToProperty` 对 11 张验收图参考里 45 963 个非空值中的 40 208 个（87.5 %）给得出「哪个库属性发成哪个属性名」。剩下的是层级枚举的上层（`ConstructionStatus2`、`PipingComponentType2 / 3`、`InstrumentType / 3`、`EqType1–5`）、管线的 `FluidSystem` 与 `DocCategory / DocType`。值的写法要靠枚举映射（`MapEnumToEnum`，`src/backup/datamap.rs` 已读）和单位规则（`50 mm`、`3 m`、`27 m^3`、`2.9999910000486E-03 rad`，未定）。

数据库线的写手（`xml_writer.rs`）已经按对象写全接口与属性，A01 上逐字节验证过；但它只接 MDF / SQLite，Oracle `exp` 的行进不去，而且比 `.pid` 线少 6 个类（`PIDInstrument`、`PIDInlineInstrument`、3 个设备类、`PIDSignalBranchPoint`）。这是方案里要用户定的方向。

### 3.5 门禁本身的缺口

- 数据库线的命令行只比个数；MTF 包走 `diff_pid_export` 时的 27 条没有登记。
- DWG 参考没有任何真跑的门（§2.1）。本轮在 `tests/publish_from_pid.rs` 加了 3 条用仓库内夹具真跑的用例。
- §1.3 的 1–4 条。主检出的 `stash@{0}` 里有一份补 1（写出侧）、3、4 的未合入改动，见 §4。

### 3.6 这一阶段没有归因完的

- DWG `--oracle-exp` 少一个 OPC（`799B9008…`，`Name = 278`）。已核实的部分：参考里这个 `PIDBranchPoint` 没有任何表示、也没有任何 `<Rel>` 提到它，是个孤立对象；dump 里它的配对 `9B50CC42…` 有一条在本图上的表示 `3A36F254…`，而这条表示不在参考、也不在参考那一版的 `.pid` 里，所以被「表示必须在 `.pid` 里」这条过滤掉。看起来是版本差（参考那时这对 OPC 还没画到图上），但「没画到图上的 OPC 原厂照样发一个孤立对象」这条规则只有这 1 例，没有定论。
- DWG 的 18 个 `missing value`（`InsulThickSrc`、`TotalInsulThick`、`SlopedPipeDirection`、`SlopedPipingAngle`、`CatalogPartNumber`、`HeightRelativeToGrade`、`ProcessVessel_VesselVolumetricCapacity`、`LongMaterialDescription`）不在豁免清单里，所以是发现项；除最后一个外都属于 §3.4。

## 4. 别处没合进来的 publish 工作

| 位置 | 是什么 | 做到哪（已核实） |
|---|---|---|
| 主检出 `stash@{0}`（`9b4d60b`，2026-11-08 15:55，「publish line WIP - out of plot mission scope」） | 18 个文件 +1233 / −328：门禁新增三种发现项（接口不在参考、`<Container>` 属性不同、UID 重复）与「标签属性整张图一个都没写出」；包模式只给 dump 里有行的图挂 dump 内容并清理输出文件名；8.3 短名解析成长名；`ExpTables` 五张表改宽松读（与本轮 S1 同一处）；`StagedMdf` 临时文件；MDF 读取跳过 ghost 记录（`T_PlantItem` 4 → 3 行）；`IOffpageConnector` 无值时不写元素（与本轮 S3b 同一处）；`--style dwg` 的关系接线；`escape_attr` 处理控制字符；三处整数溢出与一处切片 panic | 基于 `e5fa361`，其后 `main` 没动过这些文件，`git apply --check` 通过。在本 worktree 临时套上后：`cargo test --lib publish::` 348 过；14 个 `publish_*` 与 `parser_panic_safety` 全部退出 0；对照矩阵除 DWG 能读之外与 `803984a` 相同。它**改了基线**（A01 27 → 28 项，I井场 别名 1 → 2 项）与对应测试的计数。没跑 clippy / fmt / 全量。验完已还原，stash 未动 |
| `pid-parse-backup-store`（`backup-store-v2`，领先 16、落后 302） | 「一个备份包变成一个 Backup Store」第一版（S0–S5，含 `publish` 从 store 读、A01 与 MDF 逐字节相同），以及第二版计划「Oracle exp 的行进 store」 | 分支上 16 个提交未合；工作树里还有未提交的 Oracle 行解码器（`src/backup/oracle_exp/{rows,number,export_log}.rs`、`tests/oracle_exp_rows.rs`）。与 §3.4 的方向 B 直接相关。没动 |
| `pid-parse-m4-m5`（`agent/m4-i3-inspect-cli`，领先 1、落后 479） | `f4ac842`：把 `xml_writer.rs` 拆成目录、`catalog.rs` 初版、inspect 命令接口 | 目录与 inspect 的做法 `main` 上已另行落地；工作树未提交的 5 个文件都是 inspect 的。与导出正确性无关。没动 |
| `pid-parse-mission`（`mission/publish-hardening`） | 已全部在 `main` 里（领先 0），工作树干净 | — |
| 主检出未提交 | `rusqlite` 0.39 → 0.41，`sqlite_load.rs`、`publish_a01_raw_residual.rs` 各一行 `column_names()` 的适配 | 不改行为。本轮没有碰这两个文件 |

## 5. `publish`、`publish-v2`、`publish-v3` 是什么

三个都是**我方输出**，不是原厂参考：里面有只有本工具才写的 `_Coverage.json` 与 `_Index.json`。

| 目录 | 生成时间 | 内容 | 与 `803984a` 今天的输出 |
|---|---|---|---|
| `D:\work\cad\pid-test-data\publish` | 2026-10-14 02:48 | 53 张 × 三件 + `_Index.json` = 160 | `_Data.xml` / `_Meta.xml` 106 件里 63 同、43 不同 |
| `…\publish-v2` | 2026-10-24 – 25 | 同上 + `_ByteCompare.json` 与几个核对用子目录 | 同上 |
| `…\publish-v3` | 2026-11-05 16:15 – 16:19 | 160 + `_ByteCompare.json`（记着命令行与每件变化对应的提交） | **106 / 106 逐字节相同** |
| `D:\work\plant-code\cad\pid-test-data\publish` | 2026-10-14 | 第一个目录的逐字节拷贝（160 / 160） | — |
| `D:\work\cad\PID自研软件验证\publish` | 2026-10-12 / 14 | 5 张图的早期输出，其中 4 份 `_Meta.xml` 是 418–434 字节的空壳 | 已过时 |

## 6. 本阶段修掉的

见方案 §5 的提交表。修完之后 DWG 的两个值错归零、备份包读得进来（`.pid` 单发 78 / 71 / 81、值错 0、缺 17 个值；`--oracle-exp` 108 / 99 / 133、值错 0，剩 6 多发 + 6 缺的对象、6 条关系与同样 17 个值，前三项是 §3.3 的版本差）。前三片之后其余输出逐字节不变：整张矩阵 253 个文件里只有 DWG 的 26 个不同，其中 25 个是原来写不出来的（`target/publish-audit/compare_runs.py base s3`）。第四片（S3b）去掉空的 `<IOffpageConnector/>`，再变 6 个 `_Data.xml`：轻烃分离器I 的单图与整包各一、整包里的低压气压缩机入口改造、DWG 的三个；7 张图的退出码与摘要行都没动，与基线逐行比对的结果也没动。

## 7. 复现

```powershell
$wt = 'D:\work\plant-code\cad\pid-parse-wt\publish-export'
$env:CARGO_TARGET_DIR = "$wt\target"
rustup run nightly cargo test --locked -p pid-parse --test publish_from_pid -- --nocapture

# 一张图、一种模式
& "$wt\target\debug\pid_publish_xml.exe" `
  "$wt\test-file\export-test\publish-data\DWG-0202GP06-01\DWG-0202GP06-01.pid" `
  --diff-against "$wt\test-file\export-test\publish-data\DWG-0202GP06-01\DWG-0202GP06-01_Data.xml" `
  --plant P01 `
  --oracle-exp "$wt\test-file\backup-test\DWG-0202GP06-01_p\Export.dmp" `
  --datamap "$wt\test-file\backup-test\DWG-0202GP06-01_p\RefData~4~804.zip"

# 一对文件的完整度（入库的探针，方案 S0）
rustup run nightly cargo run --locked --example probe_how_much_of_a_reference_does_a_publish_state -- `
  <我方的 _Data.xml> <参考的 _Data.xml>

# 整张矩阵与普查（脚本在 target 下，不入库）
python "$wt\target\publish-audit\audit.py" "$wt\target\debug\pid_publish_xml.exe" <run-set>
python "$wt\target\publish-audit\census.py" <run-set> --agg
```
