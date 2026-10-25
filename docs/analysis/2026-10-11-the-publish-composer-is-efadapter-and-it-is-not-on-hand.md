# publish 把端口、工艺点、分支点、管线拼成 `IObject` 的代码在 `EFAdapter.dll` 里，而它不在手头（2026-10-11）

> 承接计划 `docs/plans/2026-10-11-publish-native-evidence-via-ida-bridge.md` N0–N1（publish 写出侧，Q1 (a)）。用户 2026-10-11：「用 ida-bridge 分析」。
> 问题：SmartPlant 自己发布 `_Data.xml` 时，端口 UID 的 `.1` / `.2`、工艺点 `.PPT`、分支点 `.BPT`、管线对象、代码表 `@` 引用、关系 UID `{prefix}-{UID1}-{UID2}`、`IObject.Name` 拼接，是哪个二进制、哪个函数写的？
> 结论类型：**native-reader（定位）+ 配置文件**。IDA 9.2，`ida-bridge`，库只开临时副本；`src` 一字未动。

## 一句话

手头 `dlls/` 里 13 个 VB6 模块都只以 ProgID `EFAdapter.Adapter` **引用**发布器，没有一个实现它；`dlls/sppiddatamap.xml`（SmartPlant P&ID 的 map schema）把 64 个发布类逐个指给 `EFAdapter.PipingPointPublisher` / `PpBranchPtPublisher` / `SignalPointPublisher` / `SigBranchPtPublisher` / `PipeLinePublisher` / `RepresentationPublisher` / `OPCPublisher` …，枚举属性指给 `EFAdapter.EnumConversions`。
也就是说，10-10 分析页里「可能按规则推」那五项的写出器全在 **`EFAdapter.dll`**，而这个文件不在 `dlls/`、不在 `D:\pid`，本机注册表里也没有 SmartPlant P&ID 安装。
原本以为最接近的 `sppidmdadapter.dll` 是 Schema Editor 的元数据适配器（数据字典 → map schema），`sppiddwgprocess.dll` 的 `ISPPidArchive_LoadSPItems` 是从库 / 归档 XML **装载** SPItems，都不是发布。
计划 N2a 的 1–5 在拿到 `EFAdapter.dll` 之前做不了；能继续的是图纸写入侧（`pidobjmgrai.dll`，N2b）和把 `sppiddatamap.xml` 当「按类发哪些接口 / 属性」的出处。

## 库与方法

| 模块 | 原件 SHA-256（`dlls/`，前后一致） | 大小 | 库 | 函数 / 串（8-bit → 加 UTF-16） |
|---|---|---:|---|---|
| `sppidmdadapter.dll` | `B5C088C1…EDB279` | 124 KB | `D:\ida_scratch\publish-native\sppidmdadapter\sppidmdadapter.i64` | 303 / 301 → 542 |
| `sppiddwgprocess.dll` | `B226C341…C499C` | 968 KB | `…\sppiddwgprocess\sppiddwgprocess.i64` | 481 / 408 → （重建后查引用） |
| `efpublishcmd.dll` | `E5BCCD62…4D200` | 32 KB | `…\efpublishcmd\efpublishcmd.i64` | 87 / 140 |
| `efcompwpublishedvercmd.dll` | `0BD47152…6203E2` | 36 KB | `…\efcompwpublishedvercmd\efcompwpublishedvercmd.i64` | 86 / 144 |
| `drawingmanagerocx.ocx` | `A278FA61…F62E61` | 1.5 MB | `…\drawingmanagerocx\drawingmanagerocx.i64` | 1 266 / 1 621 |
| `drawingmanagerexe.exe`、`pidobjmgrai.dll` | `623EFBE6…27737D`、`E7216143…120F75` | 1.2 MB、2.4 MB | 已拷副本，**未建库** | — |

- 五个库都是原生编译的 VB6（函数数与镜像大小成比例，Hex-Rays 能反），不是 p-code。
- VB6 的字符串常量是 UTF-16，IDA 默认串表只收 8-bit。`D:\ida_scratch\publish-native\strlist_unicode.py` 用 `ida_strlist` 把 `STRTYPE_C_16` 加进串表（`minlen = 4`），`sppidmdadapter` 的串从 301 升到 542 之后 `ModelItemType` / `ToFromText` / `SPPIDDataMap.xml` 才查得到。
- 不开 `--stateful`；bridge server 与 `D:\ida_scratch\plant3\` 下的两个 idalib 实例是别的会话的，没碰。自己开的实例做完即 `supervisor stop`。
- 二进制整盘串扫描（ASCII + UTF-16LE，`dlls/` 151 个文件 + `D:\pid\RADInstallA~`）用的是一次性 PowerShell，不入库；数字见下文。

## 1. `sppidmdadapter.dll`：Schema Editor 的元数据适配器，不是发布器

串表（UTF-16，重建后）直接说明用途：

| 地址 | 串 | 含义 |
|---|---|---|
| `0x545C1CCB` | `SmartPlant P&ID Metadata Adapter` | 模块自述 |
| `0x545C3F10` / `0x545C3FB8` | `C:\PROGRA~2\COMMON~1\INTERG~1\SCHEMA~1\SCHEMA~2.TLB` / `SCHEMA~1.TLB`（`SchemaCompInterfaces` / `SchemaGUI`） | 引用 SmartPlant Schema Component 的类型库 |
| `0x545C4078` | `r:\rad2d\bin\ISPMDAdapter.tlb`（`MDAdapterLib`） | 实现 `IMetadataAdapter` / `IMetadataAdapterGUI[2]` / `IToolMDAdapter` / `IMetadataAdapterIdentity` / `IMetadataAdapterForeign`（`0x545C415C`–`0x545C46FC` 共 30 个方法名） |
| `0x545C5280` / `0x545C5300` / `0x545C55C0` | `igrMetaSchemaExtension.xml` / `SPPIDMetaSchemaExtension.xml` / `SPPIDDataMap.xml` | 它读写的三份文件 |
| `0x545C538C` | `PIDComponent` | `IMetadataAdapter_GetCompSchemaName` 的返回 |
| `0x545C5408` / `0x545C5458` / `0x545C56C4` | `Could not get DataPile from DDBroker` / `igrCLientDa412.SPFactory` / `DataDictionary Broker failed to connect:` | 从 P&ID 数据字典取表 |
| `0x545C574C`–`0x545C5DB8` | `AreaBreak` … `TaskItemProperty`（约 50 个表名）；`0x545C5DF0`–`0x545C6D0C` `SP_DesignBasisUID` … `TextLabelJustification`（列名、枚举名） | 数据字典里被排除 / 特判的表与列 |
| `0x545C6D88`–`0x545C6DD8` | `SPMC_` / `SPMP_` / `SPEN_` / `SPUOM_` / `SPMU_` | map schema 里 ToolOID 的前缀，与 `sppiddatamap.xml` 一致 |
| `0x545C3CB8` / `0x545C3CD4` / `0x545C55E8` | `Instrument` / `Nozzle` / `InstrumentInline` | 代码里特判的三个类 |

它的工作是把 P&ID 数据字典（表、列、枚举、单位）生成 / 同步成 `SPPIDDataMap.xml`——**定义**映射，不执行发布。

## 2. `dlls/sppiddatamap.xml`：64 个发布类，每个指名自己的发布器

`Container CompSchema="PIDComponent" Scope="ToolSchema" SoftwareVersion="05.02.00.0016" SchemaVersion="07.01.00.00"`（文件时间 2017-07-23；旁边还有 `sppiddatamap_04_03.xml` … `_07_00_00.xml` 共 14 个历史版本，2014-04-24）。

| 元素 | 个数 |
|---|---:|
| `SPMapClassDef` | 64 |
| `SPMapPropertyDef` | 2 752 |
| `SPMapEnumListDef` | 194 |
| `SPMapUoMListDef` | 30 |
| `SPMapRelDef` | 2（`OwnsComponentOcc` 等） |

每个 `SPMapClassDef` 的 `IMapClassDef/@ProcessClassCriteria` 指名处理它的 COM 类（`ProgID,flag[,n]`）。去重后 22 条：

```text
EFAdapter.CtrlSysFuncExchanger,False,8      EFAdapter.EqDataExchanger,False,1..5
EFAdapter.EquipCompPublisher,False,6        EFAdapter.InstrInlDataExchanger,False,9
EFAdapter.InstrLoopDataExchanger,False,7    EFAdapter.InstrPublisher,False
EFAdapter.NotePublisher,False               EFAdapter.NozzlePublisher,False,10
EFAdapter.OPCPublisher,False                EFAdapter.PipeLinePublisher
EFAdapter.PipeRunDataExchanger,True,11      EFAdapter.PipingPointPublisher,False
EFAdapter.PpBranchPtPublisher,False         EFAdapter.PpCompDataExchanger,False,12
EFAdapter.RepresentationPublisher,False     EFAdapter.SigBranchPtPublisher,False
EFAdapter.SigConnPublisher,False            EFAdapter.SignalPointPublisher,False
```

对照 10-10 分析页「可能按规则推」与「`.pid` 里没有」两档：

| 10-10 的项 | 发布类（`ToolOID`） | 发布器 |
|---|---|---|
| 管道端口 718（`UID.1` …）、工艺点 341（`.PPT`） | `SPMC_PipingPoint` | `EFAdapter.PipingPointPublisher` |
| 信号端口 1 725 | `SPMC_SignalPoint` | `EFAdapter.SignalPointPublisher` |
| 管道分支点 183、信号分支点 16（`.BPT`） | `SPMC_PipingBranchPoint` / `SPMC_SignalBranchPoint` | `EFAdapter.PpBranchPtPublisher` / `SigBranchPtPublisher` |
| 管线 172（名称拼接） | `SPMC_Pipeline` | `EFAdapter.PipeLinePublisher` |
| 表示 1 823（`GraphicOID`，已发） | `SPMC_Representation` | `EFAdapter.RepresentationPublisher` |
| OPC 58（23 / 53 `ModelID` 不对） | `SPMC_OPC` | `EFAdapter.OPCPublisher` |
| 代码表属性（`@NewConstruction`、`@EE690`、`@{GUID}`） | 各类的 `SPMapPropertyDef`，`ProcessPropertyCriteria="EFAdapter.EnumConversions"` | `EFAdapter.EnumConversions` |

两点限制：`SPMapEnumListDef` / `SPMapUoMListDef` 在这份文件里都是空壳（`<IMapEnumListDef/>`、`<IMapUoMListDef/>`），枚举值与单位写法不在这里；
文件的另一半（SmartPlant 侧的 `IPipeCrossSectionItem` / `INamedInstrument` 等接口定义）只以名字出现，接口的属性表在 SmartPlant Foundation 的 CMF 里，手头没有。

## 3. `EFAdapter.dll` 不在手头

- `dlls/`（151 个文件）与 `D:\pid\RADInstallA~`、`RADInstallPatchA~` 里没有任何 `efadapter*`；只有 `efpublishcmd.dll` / `efcompwpublishedvercmd.dll` 两个命令壳。
- 注册表：`HKLM\SOFTWARE\WOW6432Node\Intergraph\Applications\SmartPlantPID.Application\Setup\InstallPath` 不存在，`HKCR\EFAdapter.*` 不存在——本机没装 SmartPlant P&ID，`dlls/` 是从别处拷的部分文件。
- 13 个 VB6 模块（`drawingmanagerexe.exe`、`drawingmanagerocx.ocx`、`efcompwpublishedvercmd.dll`、`efpublishcmd.dll`、`llama.dll`、`pidauto.dll`、`pidobjmgrai.dll`、`pidoptions.dll`、`pidrules.dll`、`sparchivecomps.dll`、`sppiddwgprocess.dll`、`sppidmdadapter.dll`、`sppidtypical.dll`）里 `EFAdapter` 各恰好出现 1 次（UTF-16），上下文都是 ProgID 表：
  `… SmartPlant2DCatalogManager.Application · EFAdapter.Adapter · Operation · CheckIN · CheckOUT · NewVersion · NewRevision · UpdateLastRevision …`（`sppiddwgprocess.dll` 文件偏移 `0xD5F0`、`drawingmanagerocx.ocx` `0x1B6E0`、`efpublishcmd.dll` `0x2B14`）——是 `CreateObject("EFAdapter.Adapter")` 的参数，不是实现。
- 这解释了 10-11 整盘零命中：`DefUID`、`DwgRepresentationComposition`、`PipingPortComposition`、`PIDPipingPort`、`.PPT`、`.BPT` 都应在 `EFAdapter.dll` 和 SPF 的 CMF 里。

## 4. `sppiddwgprocess.dll` 的 `LoadSPItems` 是装载，不是发布

`string_refs` 里与 SPItems / 发布相关的引用只有 20 条，集中在 7 个函数：

| 函数 | 引用的串 |
|---|---|
| `sub_547B8B70` | `.ISPPidArchive_LoadSPItems()`、`Start LoadSPItems with Bind ON during loading PIGs`、`Load Drawing ISPItems`、`Done Load Drawing ISPItems` |
| `sub_547B9870` | `.ISPPidArchive_LoadSPItemsEx()` |
| `sub_5472E350` | `::ISPProcessModule_WSPublish()`、`Start WSPublish -`、`End WSPublish`、`LastPublishTime`、`WSPublish BLOB ERROR` |
| `sub_54740BB0` | `(SP_ID, SP_WSSITEID, LASTPUBLISHTIME,`、`SET LASTPUBLISHTIME =` |
| `sub_5473B170` / `sub_5473B3D0` | `::GetDrawingXMLFile()` / `::GetSharedItemsXMLFile()` |
| `sub_547CA0F0` | `AddSourceAppToISPItem` |

`sub_547B8B70`（491 行）按一个模式参数分三路：
1 / 2 —— 走库：`rtcCreateObject2("igrCLientDa412.SPFactory")`，`Load Drawing ISPItems` → `Load Plant Item Group Data`；
3 —— 走文件：`Load XML Items`（`GetDrawingXMLFile`）→ 可选 `Load XML Shared Items`（`GetSharedItemsXMLFile`）。
`WSPublish` 是 Workshare 的站点间发布（写 `LASTPUBLISHTIME` 列），与 SmartPlant Foundation 的 publish 无关。`EmbeddedSPID` 两处引用是归档包内嵌 `.pid` 的路径，不是我们的 5 张图。
这个模块是 Drawing Manager 的归档 / Workshare 层：SPItems 是**读进来**的，之后交给 `EFAdapter.Adapter`。

## 5. 对计划的含义

- N2a 的 1–5（后缀、关系 UID、名称拼接、公称直径、代码表引用）在拿到 `EFAdapter.dll` 之前做不了；按计划 Q4 (a)，本页记为「不在原生侧」。
- 要继续 N2a，需要从装了 SmartPlant P&ID 的机器（参考 XML 由 `\\Glpid-7\pidsq\PID` 发布，那台机器上应有）拷 `…\SmartPlant\P&ID Workstation\Program\EFAdapter.dll`（及其依赖的 `EFCommon*` / Schema Component 的 CMF）。有了它，N0 的建库命令照用。
- 现在就能做的：
  - **N2b 图纸写入侧**：`pidobjmgrai.dll`（副本已拷，未建库）——`P&IDAttributes` 写什么、OPC `ModelID`、多出的被指名 UID、天花板。
  - **`sppiddatamap.xml` 当出处**：按类列出 `SPMapPropertyDef` → 发布接口 × 属性，与 5 份参考 XML 的「类 × 属性」表对一遍；导出器今天「按类发哪些接口」的依据可以从「参考 XML 反推」换成这份 map schema。它是 XML，不用 IDA。

## 复现

```powershell
# 库（临时副本；原件哈希见 D:\ida_scratch\publish-native\sha256-before.txt / sha256-after.txt）
cd D:\ida_scratch\publish-native
ida-bridge exec-idb --input sppidmdadapter\sppidmdadapter.dll --out-idb sppidmdadapter\sppidmdadapter.i64 --save
ida-bridge exec-idb --input sppiddwgprocess\sppiddwgprocess.dll --out-idb sppiddwgprocess\sppiddwgprocess.i64 --save --wait-s 900
# 同样建 efpublishcmd / efcompwpublishedvercmd / drawingmanagerocx

# 查串（先把 UTF-16 加进串表）
ida-bridge supervisor start-idalib --idb sppidmdadapter\sppidmdadapter.i64        # 记下 client_id
ida-bridge exec <cid> -f strlist_unicode.py
ida-bridge exec <cid> --sql "SELECT address, string_value FROM strings WHERE string_value LIKE '%DataMap%' OR string_value LIKE 'SPM%'"
ida-bridge supervisor stop <cid>

ida-bridge supervisor start-idalib --idb sppiddwgprocess\sppiddwgprocess.i64
ida-bridge exec <cid> -f strlist_unicode.py
ida-bridge exec <cid> --sql "SELECT string_value, func_name, ref_addr FROM string_refs WHERE string_value LIKE '%SPItem%' OR string_value LIKE '%Publish%' OR string_value LIKE '%XMLFile%'"
ida-bridge exec <cid> --timeout-s 180 --sql "SELECT decompile(0x547B8B70) AS t"   # ISPPidArchive_LoadSPItems
ida-bridge supervisor stop <cid>

# map schema
Select-String -Path D:\work\plant-code\cad\pid-parse\dlls\sppiddatamap.xml -Pattern 'ProcessClassCriteria="([^"]*)"' -AllMatches |
  ForEach-Object { $_.Matches } | ForEach-Object { $_.Groups[1].Value } | Sort-Object -Unique
```

- IDA 9.2（`D:\IDA Professional 9.2`），ida-bridge CLI `C:\Users\dpc\.agents\tools\ida-bridge`。
- 2026-10-11 实跑：五个库建完共约 2 分钟（`drawingmanagerocx` 70 s）；`LoadSPItems` 反编译 1.3 s。
