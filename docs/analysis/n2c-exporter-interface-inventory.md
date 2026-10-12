# N2c 底稿：`pid_writer` 今天按类发哪些接口

> 任务 t-912。仓库 `pid-parse`。只盘点 `.pid` 导出这条路（`src/publish/pid_load.rs` 定类、定接口，`src/publish/pid_writer.rs` 写出）。不把出处改成 map schema，不开 IDA，不读 `EFAdapter.dll`。
> **本轮未改任何代码、注释或测试，也没有提交。** 只新建本文件。

## 一句话

按类能发哪些接口，不在 `pid_writer.rs` 里有一张「类 → 接口」数组。类名来自 `CLASS_OF_ITEM_TYPE`（仪表再拆），接口来自 `published_attribute` 这张从参考 XML 反推的常量表；`write_item` 只把已经有值的非 `IObject` 接口写出去。`xml_writer.rs` 里按类硬写的那一套接口名单，这条路不用。

## 常量表位置

| 什么 | 符号 | 位置 |
|---|---|---|
| `ModelItemType` → 发布类 | `CLASS_OF_ITEM_TYPE` | `src/publish/pid_load.rs` 52–64 |
| 定了类也不发的类型 | `UNCLASSED_ITEM_TYPES`（只有 `OPC`） | 同文件 68 |
| 仪表三类 | `INSTRUMENT_CLASSES` | 同文件 69–73 |
| 设备四类 | `EQUIPMENT_CLASSES` | 同文件 74–79 |
| 仪表类怎么拆 | `instrument_class` | 同文件 325–335 |
| 没有 `SymbolAttr` 时按库目录拆 | `instrument_class_by_library` | 同文件 345–360 |
| 标签属性 →（接口，属性名），按类设门 | `published_attribute` | 同文件 363–381 |
| 装载时用上面两张表定 `item.class` | `load_publish_from_pid` 里的 `match` | 同文件 513–539 |
| 仪表 / 设备拼 `IObject.Name` | `composed_name` | 同文件 621–649 |
| 物项：必发 `IObject`，其余接口有值才发 | `write_item` | `src/publish/pid_writer.rs` 114–144 |
| 图纸 / 表示 / 两条关系 | `write_document` | 同文件 54–112 |
| 关系上的两个接口 | `write_rel` | 同文件 146–162 |
| `_Meta.xml` 不发任何类 | `write_pid_meta_xml` → `write_meta_envelope` | `pid_writer.rs` 44–51；`src/publish/xml_writer.rs` 161–169 |

`published_attribute`（`pid_load.rs` 366–379）全表：

| 标签属性 | 类门 | 接口 | 属性 |
|---|---|---|---|
| `MeasuredVariableCode` | `INSTRUMENT_CLASSES` | `INamedInstrument` | `MeasuredVariable` |
| `TagSequenceNo` | `INSTRUMENT_CLASSES` | `INamedInstrument` | `InstrTagSequenceNo` |
| `TagSequenceNo` | 恰好 `PIDPipingConnector` | `INamedPipingConnector` | `PipingConnectorSeqNo` |
| `TagSequenceNo` | `EQUIPMENT_CLASSES` | `INamedEquipment` | `EquipSeqNo` |
| `TagPrefix` | `EQUIPMENT_CLASSES` | `INamedEquipment` | `EquipPrefix` |
| `PipingMaterialsClass` | 无 | `IPipingSpecifiedItem` | `PipingMaterialsClass` |
| `InsulationSpec` | 无 | `IInsulatedItem` | `InsulSpec` |
| `Description` | 无 | `IObject` | `Description` |
| `ToFromText` | 无 | `IOffpageConnector` | `ToFromText` |
| `Note.Body` | 无 | `INote` | `NoteText` |

无类门的四条（`IPipingSpecifiedItem`、`IInsulatedItem`、`IOffpageConnector`、`INote`）任何已定类的物项都能带上，只要标签对得上。`IObject` 每个物项都发（`write_item` 126 行）；`Name` / `Description` 只有属性里真有才写进这个元素（117–124 行）。其它接口要该接口下至少有一个值才出现（128–142 行）。

## 会发出去的类

「类从哪来」是定这个类名的源码位置。「接口」是今天这条路会写到 XML 上的接口。物项类的专有接口见上表的类门；四条无类门接口每行都重复一遍，打开 `published_attribute` 就能对上。

| 类 | 类从哪来 | 接口 |
|---|---|---|
| `PIDDrawing` | `write_document`，`pid_writer.rs` 67–74。每份文件固定一块，不走 `CLASS_OF_ITEM_TYPE` | `IObject`（`UID`、`Name`）。没有第二个接口 |
| `PIDRepresentation` | `write_document`，`pid_writer.rs` 82–94。每个表示一块 | `IObject`（只有 `UID`）、`IDrawingRepresentation`（`GraphicOID`） |
| `Rel` | `write_rel`，`pid_writer.rs` 146–162。`write_document` 96–109 行发两种 `DefUID`：`DwgRepresentationComposition`（每条表示一条）、`DrawingItems`（表示所指物项已发布才发，103–108 行）。UID 前缀 `DRC` / `DRI` 在 `xml_writer.rs` `defuid_prefix` 2631–2632 行 | `IObject`（`UID`）、`IRel`（`UID1`、`UID2`、`DefUID`） |
| `PIDProcessVessel` | `CLASS_OF_ITEM_TYPE`：`Vessel`，`pid_load.rs` 53 行。设备门：`EQUIPMENT_CLASSES` 75 行 | `IObject`（必发；`Name` 由 `composed_name` 621–649 行在 `EquipPrefix` 与 `EquipSeqNo` 都在时拼）；`INamedEquipment`；另加无类门的 `IPipingSpecifiedItem`、`IInsulatedItem`、`IOffpageConnector`、`INote` |
| `PIDProcessEquipment` | `EquipmentOther`，`pid_load.rs` 54 行；设备门 76 行 | 同上 |
| `PIDMechanicalEquipment` | `Mechanical`，`pid_load.rs` 55 行；设备门 77 行 | 同上 |
| `PIDHeatTransferEquipment` | `Exchanger`，`pid_load.rs` 56 行；设备门 78 行 | 同上 |
| `PIDNozzle` | `Nozzle`，`pid_load.rs` 57 行。不在仪表门也不在设备门 | `IObject`（必发；无拼名规则，`composed_name` 640–641 行直接返回）；无类门的 `IPipingSpecifiedItem`、`IInsulatedItem`、`IOffpageConnector`、`INote` |
| `PIDPipingComponent` | `PipingComp`，`pid_load.rs` 58 行 | 同 `PIDNozzle` |
| `PIDPipingConnector` | `PipeRun`，`pid_load.rs` 59 行。`TagSequenceNo` 的门是类名相等，`published_attribute` 369–370 行 | `IObject`（必发；无拼名）；`INamedPipingConnector`；无类门四条同上 |
| `PIDSignalConnector` | `SignalRun`，`pid_load.rs` 60 行 | 同 `PIDNozzle` |
| `PIDNote` | `ItemNote`，`pid_load.rs` 61 行 | 同 `PIDNozzle`（`Note.Body` → `INote` 没有「必须是注释类」的门） |
| `PIDInstrument` | `CLASS_OF_ITEM_TYPE` 把 `Instrument` 留空（`pid_load.rs` 63 行），再由 `instrument_class`：`Type == "Off-line instruments"`（332 行），或 `instrument_class_by_library`：目录 `Off-Line` / `Actuators`（356 行）。装载分支 520–534 行 | `IObject`（必发；`Name` 由 `composed_name` 在 `MeasuredVariable` 与 `InstrTagSequenceNo` 都在时拼）；`INamedInstrument`；无类门四条同上 |
| `PIDControlSystemFunction` | `instrument_class`：`Type == "System functions"`（331 行）；库目录 `System Functions`（357 行） | 同 `PIDInstrument` |
| `PIDInlineInstrument` | `instrument_class`：`Class == "Instrument In-Line Component"`（327–328 行）；库目录 `In-Line` / `Relief Devices`（355 行） | 同 `PIDInstrument` |

`write_item` 的类名就是上表这些字符串，原样当 XML 元素名（`pid_writer.rs` 115 行）。`class == None` 的物项整段跳过（77–79 行）。

## 表里有名字、今天不发的类

| 类 | 为什么不发 |
|---|---|
| `PIDBranchPoint` | `CLASS_OF_ITEM_TYPE` 62 行把 `OPC` 映成这个类，但 `UNCLASSED_ITEM_TYPES`（68 行）在 `load_publish_from_pid` 519 行优先判成 `None`。物项不进 `write_item`。它的表示仍作 `PIDRepresentation` 发出；`DrawingItems` 因为物项未发布而被 103–108 行滤掉 |

`_Meta.xml`（`write_meta_envelope`，`xml_writer.rs` 161–169）只有 `<Container>`，不发 `DocumentVersion` / `DocumentRevision` / `File` / `Rel`。库那条路的 `write_meta_xml` 才会发那些类；`.pid` 这条路不调用它。

## 这条路不发、也不在本常量表里的类

端口、工艺点、分支点、管线等不在 `CLASS_OF_ITEM_TYPE` 里，`pid_writer` 没有对应元素。`xml_writer.rs` 的 `write_process_vessel` / `write_nozzle` / `write_pipeline` 等按类硬写的接口（例如 `IPIDProcessVesselOcc`、`IPipingPortComposition`）不属于本表；`pid_writer` 不调用那些函数。

## 核对

打开上表「位置」列的文件和行号即可。类名字符串、接口字符串都在那几行的字面量里。本轮没有跑测试（任务边界是只读 `src`、只新建本文件）。
