# 端口、工艺点、分支点的 UID 是父 UID 加后缀，关系 UID 是前缀表拼出来的（2026-10-12）

> 计划 `docs/plans/2026-10-11-publish-native-evidence-via-ida-bridge.md` N2a 第 1、2、7 问。任务 t-1270。
> 结论类型：**native-reader**。IDA 9.2，`ida-bridge`，库只开临时副本。`src` 未改，未提交。测试 NOT RUN：本单只新建这一页。
> 原件与副本 SHA-256 相同：`6C77CC79A3CA92256F3C168C6183B03BAA67CF4545D9330E7E854C3F8C487891`。
> 模块 `EFAdapter.dll`（1,519,616 字节，文件时间 2017-08-30）。副本 `D:\ida_scratch\publish-native\efadapter-uid\EFAdapter.dll`，库 `EFAdapter.dll.i64`。镜像基址 `0x5F4F0000`，1,460 个函数。`strlist_unicode.py` 重建后 1,839 条串。自己开的实例 `idalib-34592` 做完已停。`idalib-29552` / `idalib-40832`（`plant3`）和别人的 `efadapter-name` 没停。

## 一句话

端口 UID 是父物项的 `SP_PlantItemID` 加 `.` 再加符号上的点号；工艺点是管段自己的 Id 加 `.PPT`；分支点是表示自己的 Id 加 `.BPT`。关系 UID 是 `FillUpRelDefArray` 里的三字母前缀，再加 `-`、`UID1`、`-`、`UID2`。五张参考图按这两条重算，2,983 个派生 UID 和 8,185 条关系都是错 0。图纸里多出来的被指名串不是这几类对象，主要是 `Relationship.<guid>` 和标签的 `DrawingID`。

## 第 1 问：后缀谁拼、序号哪来

拼接点都在 `AdapterUtil` 的 `ConstructUIDForObject`（`sub_5F5CEF20`，`0x5F5CEF20`）。它先拿物项类型名，再分叉。点号 `.` 是 `0x5F50D600` 上的 BSTR，不是和后缀写在同一个字面量里。

| 物项类型 | 函数 | 地址 | 拼法 |
|---|---|---|---|
| `SignalPoint` | `ConstructUIDForPort` `sub_5F5CFE70` | `0x5F5CFE70` | `SP_PlantItemID` & `.` & `Str(SignalPointNumber)` |
| `PipingPoint` | 同上 | 同上 | `SP_PlantItemID` & `.` & `Str(PipingPointNumber)` |
| `DuctingPoint` | 同上 | 同上 | `SP_PlantItemID` & `.` & `Str(SP_DuctingPointNumber)` |
| `PipeRun` / `DuctRun`，且发布类是 `PIDProcessPoint` | `ConstructUIDForProcessPoint` `sub_5F5CFD40` | `0x5F5CFD40` | 对象自己的 Id & `.` & `PPT` |
| `Representation` / `Symbol`，且发布类是 `PIDPipingBranchPoint` 或 `PIDSignalBranchPoint` | `ConstructUIDForBranchPoint` `sub_5F5CFC10` | `0x5F5CFC10` | 对象自己的 Id & `.` & `BPT` |

`PPT` 在 `0x5F50B618`，`BPT` 在 `0x5F50B60C`。端口函数把第二个参数交给 `__vbaStrI4`，所以序号是整数，不是再从 1 重排的名次。`PipingPointNumber`（`0x5F50932C`）和 `SignalPointNumber`（`0x5F509384`）是从点对象上读出来的属性。`dlls/sppiddatamap.xml` 对 `SPMP_103` 的说明是 “Number field assigned in the definition of the P&ID symbol”。

同一个函数里，其余类型走「原样返回 Id」：

- `Drawing` → `ConstructUIDForDrawing`（`sub_5F5CF910`，`0x5F5CF910`），把参数抄回去。
- `ModelItem`、`ItemNote`、`PlantItem`、`Equipment`、`EquipmentOther`、`Exchanger`、`Mechanical`、`Vessel`、`EquipComponent`、`Nozzle`、`PipingComp`、`Instrument`、`Pipeline`、`SignalRun`、`RoomComponent`、`PlantItemGroup`、`InstrLoop`、`Room`、`DuctingComp`，以及不是工艺点的 `PipeRun` / `DuctRun` → `ConstructUIDForModelItem`（`sub_5F5CFA10`，`0x5F5CFA10`），同样原样返回。
- `Connector`，以及不是分支点的 `Representation` / `Symbol` → `ConstructUIDForRepItem`（`sub_5F5CFB10`，`0x5F5CFB10`），原样返回。
- `OPC` → `ConstructUIDForOPC`（`sub_5F5CFFD0`，`0x5F5CFFD0`）。两个参数是对象自己的 Id 和 `SP_pairedWithID`（`0x5F5CF2D9`）。`__vbaStrCmp` 之后取字典序较大的那一个。
- 上面都不是 → 报 `Unhandled itemtype name:`（`0x5F5CF558`），不发。

`Name`、`ConnectionFlowDirection`、`PipingConnectorType` 不在这几个拼 UID 的函数里。

- **`Name`**：map 把 `PipingPointNumber`（`SPMP_103`）映到 `PIDPipingPort.Name`（`SPMP_103.PIDPipingPort.Name`），把 `SignalPointNumber`（`SPMP_182`）映到 `PIDSignalPort.Name`。`ProcessPropertyCriteria` 是空的，所以号按文本抄出去。工艺点和分支点没有这条映射，参考 XML 里它们的 `Name` 也是空的。
- **`ConnectionFlowDirection`**：map 把管道点的 `FlowDirection`（`SPMP`，`ToolOID` `SPMP_PipingPoint_FlowDirection`）交给 `EFAdapter.PortFlowDirectionConv`，目标属性是 `PIDPipingPort.ConnectionFlowDirection`。转换在 `sub_5F583230`（`0x5F583230`）。值是枚举序号，`dec` 之后按 0..4 跳表：序号 1 和 5 → `{23F94152-B87D-11D6-80D4-00A0C92487AF}`，2 → `{23F94146-B87D-11D6-80D4-00A0C92487AF}`，3 → `{23F9414C-B87D-11D6-80D4-00A0C92487AF}`，4 → `{BCB3F097-66CA-487B-9AF4-D8FCF18624A3}`（字面量从 `0x5F583AE5` 起）。序号是空就走到 `0x5F583BF0`，不写这个属性。信号端口没有对应转换。
- **`PipingConnectorType`**：五张图的端口、工艺点、分支点上都是 0 次。map 把它映到 `PIDPipingConnector`（`SPMP_93.PIDPipingConnector.PipingConnectorType`），不映到端口。本 DLL 的串表里没有 `PipingConnectorType`。

### 五图重算

参考 `_Data.xml` 在 `D:\work\cad\PID自研软件验证`（不含 `publish\`）。规则：

- `PIDPipingPort`：`PipingPortComposition` 的 `UID2` & `.` & `IObject/@Name`
- `PIDSignalPort`：`SignalPortComposition` 的 `UID2` & `.` & `Name`
- `PIDProcessPoint`：`ProcessPointCollection` 的 `UID2` & `.PPT`（`Name` 为空）
- `PIDPipingBranchPoint` / `PIDSignalBranchPoint`：去掉 `.BPT` 之后的 UID 必须是同图的 `PIDRepresentation`

| 类 | 对象 | 错 | 父类 |
|---|---:|---:|---|
| `PIDSignalPort` | 1,725 | 0 | `PIDInstrument` 963，`PIDControlSystemFunction` 662，`PIDInlineInstrument` 100 |
| `PIDPipingPort` | 718 | 0 | `PIDPipingComponent` 537，`PIDInlineInstrument` 144，`PIDNozzle` 31，`PIDProcessEquipment` 6 |
| `PIDProcessPoint` | 341 | 0 | `PIDPipingConnector` 341 |
| `PIDPipingBranchPoint` | 183 | 0 | `PIDRepresentation` 183 |
| `PIDSignalBranchPoint` | 16 | 0 | `PIDRepresentation` 16 |
| **合计** | **2,983** | **0** | |

端口的 `Name` 与后缀逐条相同。管道端口的号只出现 `1`–`4`，信号端口 `1`–`8`。611 个父对象里有 36 个的号不是从 1 排到最大号（例如只有 `2`，或 `3,4,5,6`）。这和「号是符号上的 `*PointNumber`，不是发布时重排」一致，不算规则失败。

`ConnectionFlowDirection`：718 个管道端口里 298 个有值，`{23F9414C-…}` 153 个、`{23F94146-…}` 145 个，就是转换表的序号 3 和 2。另外 420 个为空。1,725 个信号端口、341 个工艺点、199 个分支点都没有这个属性，也没有 `PipingConnectorType`。

这条规则成立。

## 第 2 问：关系 UID

`ConstructUIDForRelationship`（`sub_5F5D0280`，`0x5F5D0280`）三个参数是关系定义名、`UID1`、`UID2`。结果是：

`Lookup(定义名)` & `-` & `UID1` & `-` & `UID2`

`-` 是 `0x5F50D9D0`。`Lookup` 是 `sub_5F5D0710`（`0x5F5D0710`）。表是模块加载后由 `FillUpRelDefArray`（`sub_5F5D0B50`，`0x5F5D0B50`，日志串在 `0x5F5D20AC`）填进 `dword_5F637084` 的。每一项 12 字节，第一项是定义名，偏移 +4 是前缀。对不上、或前缀是空串（`dword_5F5068A8`）就报 `Unknown RelDef UID:`（`0x5F519C8C`）。

五张图里出现的 14 个 `DefUID` 都在这张表里，而且一个定义只有一个前缀：

| `DefUID` | 前缀 | 条数 |
|---|---|---:|
| `DrawingItems` | `DRI` | 1,823 |
| `DwgRepresentationComposition` | `DRC` | 1,823 |
| `SignalPortComposition` | `SPC` | 1,725 |
| `PipingTapOrFitting` | `PTF` | 738 |
| `PipingPortComposition` | `PPC` | 718 |
| `ProcessPointCollection` | `PRP` | 341 |
| `PipingEnd1Conn` | `PE1` | 283 |
| `PipingEnd2Conn` | `PE2` | 279 |
| `PipingConnectors` | `PCN` | 219 |
| `SignalEnd2Conn` | `SE2` | 83 |
| `SignalEnd1Conn` | `SE1` | 75 |
| `EquipmentComponentComposition` | `EQC` | 36 |
| `SignalTapOrFitting` | `STF` | 35 |
| `PBSItemNotes` | `ITN` | 7 |
| **合计** | | **8,185** |

8,185 条的 `IObject/@UID` 都等于 `前缀-UID1-UID2`，顺序就是 `IRel` 的 `UID1` 然后 `UID2`，没有对调。错 0。

表里还有这五张图没用到的定义：`InstrumentLoopAssembly`=`ILA`，`PipingSysConnectorImplementation`=`PSI`，`PBSItemCollection`=`PBS`，`EquipmentOcc_EquipmentOcc`=`E2E`，`OwnsComponentOcc`=`OCO`，`EquipmentComponentComponents`=`ECC`，以及风管和房间的 `DE1` / `DE2` / `DTF` / `DPC` / `RDC` / `VCV` / `RRC`。本页不把它们算进「错 0」。

## 第 7 问：发哪些类、漏掉什么

发不发，先看物项类型能不能进 `ConstructUIDForObject` 的那张表。进表的类见第 1 问。表外的类型直接报 `Unhandled itemtype name`，不是静默丢掉。

派生对象（端口、工艺点、分支点）的 UID 不在图纸的 `ModelID` 里，它们是发布时算出来的。管线走 `Pipeline` → `ConstructUIDForModelItem`，UID 是库里的 Id，不是图纸上的某个字段。所以 I井场「被指名 3,089、对得上发布对象 1,460」里，对不上的那一侧不是这 2,983 个派生 UID。

多出来的是图纸 `P&IDAttributes` 的 `DrawingNo` / `DrawingID` / `ModelID` 里，那些不等于任何发布对象 UID 的串。10-10 探针的数是 I井场 3,089 个被指名、1,460 个对上发布对象，多 1,629。本页用同一份 `.pid` 做了一次原始扫描（属性集在扇区边界被切开时会少一个字节）：3,090 个串、对上 1,459 个对象，和探针差 1。多出来的 1,631 个串分成：

| 字段 | 条数 | 是什么 |
|---|---:|---|
| `ModelID` | 947 | 944 条是 `Relationship.<32 位十六进制>`，`ModelItemType` 为 `Relationship`。另外 `SP_DrawingID` 1 条、标签 1 条、`OPC` 1 条 |
| `DrawingID` | 681 | 标签图形（`LabelPersist`）以及 `PipeRun` / `SignalRun` 行上、本身不是发布对象 UID 的图形 Id |
| `DrawingNo` | 3 | 不是 32 位 UID |

这三样都不是 `ConstructUIDForObject` 会当成物项发出去的类：

- `Relationship.<guid>` 是图纸上的关系行。发布出去的关系用第 2 问的前缀 UID，不用这个 `ModelID`。
- `LabelPersist` 不在物项类型表里，标签图形不发成对象。
- 那 1 条对不上的 `OPC` `ModelID` 对得上 `ConstructUIDForOPC`：发布 UID 是本项 Id 和 `SP_pairedWithID` 里字典序较大的一个，图纸上的 `ModelID` 可以是落选的那一个。10-10 附录里 I井场 `PIDBranchPoint` 2 个对象只有 1 个被指名，是同一件事。

表示这一侧还有明确的跳过。`RepresentationPublisher` 的 `IClassPublisher_Publish` 是 `sub_5F52FE20`（`0x5F52FE20`）：

- `0x5F531EB3` 记录 `Skip Publishing Representation for SP_ID =`，然后离开这条表示，不去建 `DrawingItems`。
- `0x5F5331D2` 记录 `Skip Publishing Representation for Implied Item whose SP_ID =`，同样离开。
- `GraphicOID` 为空（`0x5F531F74`）或 `SP_ModelItemID` 为空（`0x5F530ABA`）走错误日志，不把这条表示发出去。
- `0x5F531FDC` 把 `RepresentationType` 和字面量 `39` 比较，不相等就跳到 `0x5F5332E4`，不走后面的臂。另一处查询 `sub_5F57DF50`（`0x5F57E0E6`）只留 `ItemStatus = 1` 且 `RepresentationType IN (13, 39, 45)` 的表示。

取数过滤器在 `RetrieveUtil` 的 `sub_5F53DAF0` / `sub_5F53EC50`。`0x5F53E6E1` 把条件属性设成 `SP_Ignored`，紧接着 `__vbaStrI2(0)` 把比较值设成 0。同一个函数还把 `SP_IsDeleted`、`SP_TaskStatus`、`SP_TaskType` 放进条件。被标成忽略的物项不会进入发布集合。

## 复现

在 `D:\ida_scratch\publish-native` 下。不要 `--stateful`，不要停 `plant3` 或 `efadapter-name`。

```
ida-bridge exec-idb --input efadapter-uid\EFAdapter.dll --out-idb efadapter-uid\EFAdapter.dll.i64 --save --wait-s 900
ida-bridge supervisor start-idalib --idb efadapter-uid\EFAdapter.dll.i64
ida-bridge exec <cid> -f strlist_unicode.py
ida-bridge exec <cid> --sql "SELECT string_value, func_name, ref_addr FROM string_refs WHERE string_value LIKE ':ConstructUID%'"
ida-bridge supervisor stop <cid>
```

建库前对 `dlls\EFAdapter.dll` 和副本各算一次 SHA-256，应都是上面那一串。做完再算一次原件，应仍相同。UTF-16 串表用 `D:\ida_scratch\publish-native\strlist_unicode.py`。

五图核对是只读 PowerShell / Python，读 `D:\work\cad\PID自研软件验证\*_Data.xml`，不写回。派生 UID 按上一节的四条规则重算，关系 UID 按前缀表重算。

## N4-2 探针：工艺点可以从已发布的管段发出，端口的点号不在图纸里

探针 `examples/probe_n4_ports_and_process_points.rs`（`cargo run --example probe_n4_ports_and_process_points`）。只读五张 `.pid` 和旁边的 `_Data.xml`，不写 `publish\`。

关系两端按第 2 问的前缀表重算：718 条 `PipingPortComposition`、1,725 条 `SignalPortComposition`、341 条 `ProcessPointCollection` 全部是 `UID2` 为父、`UID1` 为端口或工艺点，关系 UID 等于 `前缀-UID1-UID2`。错 0。

工艺点不需要图纸上另有一条记录。每个已经发布的 `PIDPipingConnector`（管段 `ModelID`）拼 `UID.PPT`，关系是 `PRP-{工艺点 UID}-{管段 UID}`。五图 341 对 341，多 0、缺 0，关系同样 341 对 341。规则地址是 `ConstructUIDForProcessPoint`（`0x5F5CFD40`）和 `ConstructUIDForRelationship`（`0x5F5D0280`，前缀 `PRP`）。参考里工艺点没有 `Name`，也没有 `ConnectionFlowDirection`。这一类按「五图错 0 才发」可以发。

端口发不出来。五张 `.pid` 的全部流里，ASCII 和 UTF-16 的 `PipingPointNumber`、`SignalPointNumber`、`FlowDirection`、`SignalPoint` 都是 0 处。`PipingPoint` 只在分离器 A 出现 6 次，是标签类名 `_PipingPoint1.NominalDiameter` 和 `_PipingPoint2.NominalDiameter`，不是点对象。符号缓存 `JSite*/PSMcluster0` 里的 `_ConnectPoint`（I井场 179、分离器 A 103、分离器 B 101、低温分离器 A 135、换热器 CD 163）只有 `MaxConnectedLines`，有的还有 `AngleCount`，没有点号，也没有流向。这些点记录的 `index` 几乎是常数（15 / 12 / 12 / 7 / 13），不是参考端口名里的 `1`–`8`；`sub_type` 全是 `0x4`，分不出管道点和信号点。

所以 `PIDPipingPort`、`PIDSignalPort`、`PPC`、`SPC` 和 `ConnectionFlowDirection` 都不发，只计数。差就是参考里的全部，不是少数对不上：

| 图 | 管道端口 | 发出 | 信号端口 | 发出 | 参考里有流向的管道端口 |
|---|---:|---:|---:|---:|---:|
| I井场 | 362 | 0 | 915 | 0 | 152 |
| 分离器 A | 63 | 0 | 107 | 0 | 35 |
| 分离器 B | 53 | 0 | 107 | 0 | 33 |
| 低温分离器 A | 68 | 0 | 197 | 0 | 38 |
| 换热器 CD | 172 | 0 | 399 | 0 | 40 |
| **合计** | **718** | **0** | **1,725** | **0** | **298** |

## N4-3 探针：分支点按管线类型分，UID 是表示自己的 Id 加 `.BPT`

探针 `examples/probe_branch_points_from_representations.rs`（`cargo run --example probe_branch_points_from_representations`）。只读五张 `.pid` 和旁边的 `_Data.xml`。

候选在计分前写死：`DrawingItemType = Symbol` 且描述记录是 `igPoint2d`（`0x005E`）的行是分支点表示。`ModelItemType = PipeRun` 发 `PIDPipingBranchPoint`，`SignalRun` 发 `PIDSignalBranchPoint`，UID = `DrawingID` & `.` & `BPT`（`ConstructUIDForBranchPoint`，`0x5F5CFC10`）。别的类型不发。`DrawingItems` 是 `DRI-{DrawingID}-{DrawingID.BPT}`，表示在 `UID1`，分支点在 `UID2`。行上没有符号路径，字段只有 `DrawingID`、`DrawingItemType`、`DrawingNo`、`Flag`、`ModelID`、`ModelItemType`、`ProjectNumber`。

五图 199 对 199，错 0、多 0、缺 0。管道 183（I井场 97、分离器 A 22、分离器 B 14、低温分离器 A 15、换热器 CD 35），信号 16（10 / 1 / 1 / 1 / 3）。和装载里 `item_uid = None` 的表示是同一集合。参考里 Name 全空。关系 199 对 199。

管道接口：`IConnection`、`IPipingConnection`、`IDrawingItem`、`IPipingBranchPoint`、`IDocumentItem`。信号接口：`IConnection`、`IDrawingItem`、`ISignalConnection`、`ISignalBranchPoint`、`IFacilityPoint`、`IDocumentItem`。`PIDBranchPoint`（OPC）仍不发。

## N4-4 探针：连通关系的两端不在图纸属性里

探针 `examples/probe_connectivity_relationships.rs`（`cargo run --example probe_connectivity_relationships`）。只读五张 `.pid` 和旁边的 `_Data.xml`。关系 UID 仍按第 2 问的前缀表核过，错 0。本页要问的是两端能不能从 `.pid` 读出来。

`P&IDAttributes` 里 `ModelItemType = Relationship` 的行（I井场 946、分离器 A 191、分离器 B 153、低温分离器 A 200、换热器 CD 411）只有 `ProjectNumber`、`DrawingNo`、空的 `DrawingItemType`、空的 `DrawingID`、`ModelItemType`、`ModelID = Relationship.<guid>`、`Flag`。没有两端。管口行和注释行也只有这七个身份字段，没有设备 Id，没有宿主物项。

读出器已经会把 Sheet 端点对解成 `source_drawing_id` / `target_drawing_id`。这五张图上解出来的两端都是空：I井场 26 条、分离器 A 36、分离器 B 2、低温分离器 A 78、换热器 CD 412，`both_drawing_ids = 0`。

所以这八种都不进装载。两端即使都是已经发出的对象，也配不成对。`PipingConnectors` 另有原因：一端是管线，图纸不发管线。

| `DefUID` | 参考 | 两端都已发出 | 从图纸配上 | 发出 |
|---|---:|---:|---:|---:|
| `PipingEnd1Conn` | 283 | 93 | 0 | 0 |
| `PipingEnd2Conn` | 279 | 117 | 0 | 0 |
| `SignalEnd1Conn` | 75 | 5 | 0 | 0 |
| `SignalEnd2Conn` | 83 | 12 | 0 | 0 |
| `PipingTapOrFitting` | 738 | 164 | 0 | 0 |
| `SignalTapOrFitting` | 35 | 15 | 0 | 0 |
| `EquipmentComponentComposition` | 36 | 36 | 0 | 0 |
| `PBSItemNotes` | 7 | 7 | 0 | 0 |
| `PipingConnectors` | 219 | 0 | 0 | 0 |

「两端都已发出」只说明那两个 UID 本模式已经会发（分支点、管段、设备、管口、注释），不是说图纸写了它们的配对。端连接里没发出的那一端主要是端口。`EquipmentComponentComposition` 实测是管口在 `UID1`、设备在 `UID2`，和「设备在 UID1」的口头方向相反；配不上，所以这个方向不影响发出。
