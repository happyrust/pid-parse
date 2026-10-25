# N2c 总表：导出器今天不发的 30 个接口

> 任务 t-1050。只读三份已验收的缺口表，数字原样拷，不改它们：
>
> - `docs/analysis/n2c-gap-piping-interfaces.md`（管路 16 行，抽出不发的 13 行）
> - `docs/analysis/n2c-gap-equipment-drawing-interfaces.md`（设备和图纸 14 行，抽出不发的 8 行）
> - `docs/analysis/n2c-gap-instrument-signal-interfaces.md`（仪表和信号 12 行，抽出不发的 9 行）
>
> 三份里标成「发」的 12 行不收：管路 3（`PIDPipingConnector` 的 `IInsulatedItem`、`INamedPipingConnector`、`IPipingSpecifiedItem`），设备和图纸 6（`PIDNozzle` 的 `IPipingSpecifiedItem`，`PIDDrawing` 的 `IObject`，`PIDNote` 的 `INote` 和 `IObject`，`PIDRepresentation` 的 `IDrawingRepresentation` 和 `IObject`），仪表和信号 3（`PIDInstrument` 的 `INamedInstrument` 和 `IObject`，`PIDSignalConnector` 的 `IObject`）。
> **本轮未改 `src`、注释、测试，也没有改这三份缺口表、对表、实测或导出器底稿。没有提交。** 测试 NOT RUN：只新建本文件。

五图非空和、非空属性名与来源表的单元格相同。

| 类 | 接口 | 五图非空和 | 参考里非空的属性名 | 出处 |
|---|---|---|---|---|
| `PIDPipingConnector` | `IConnector` | 444 | `FlowDirection`（103）、`RepresentationsAreAllZeroLength`（341） | 管路表 |
| `PIDPipingConnector` | `IPBSItem` | 682 | `ConstructionStatus`（341）、`ConstructionStatus2`（341） | 管路表 |
| `PIDPipingConnector` | `IPIDTypical` | 341 | `IsTypical`（341） | 管路表 |
| `PIDPipingConnector` | `IPipeCrossSectionItem` | 200 | `NominalDiameter`（200） | 管路表 |
| `PIDPipingConnector` | `IPipingConnector` | 341 | `PipingConnectorType`（341） | 管路表 |
| `PIDPipingComponent` | `IInlineComponent` | 264 | `IsFlowDirectional`（264） | 管路表 |
| `PIDPipingComponent` | `IPIDTypical` | 264 | `IsTypical`（264） | 管路表 |
| `PIDPipingPort` | `IConnection` | 298 | `ConnectionFlowDirection`（298） | 管路表 |
| `PIDPipingPort` | `IObject` | 1436 | `Name`（718）、`UID`（718） | 管路表 |
| `PIDPipingPort` | `IPipeCrossSectionItem` | 444 | `NominalDiameter`（444） | 管路表 |
| `PIDPipeline` | `IFluidSystem` | 149 | `FluidCode`（4）、`FluidSystem`（145） | 管路表 |
| `PIDPipeline` | `IObject` | 344 | `Name`（172）、`UID`（172） | 管路表 |
| `PIDPipingBranchPoint` | `IObject` | 183 | `UID`（183） | 管路表 |
| `PIDHeatTransferEquipment` | `IPIDTypical` | 1，与对表一致 | `IsTypical`（1） | 设备和图纸表 |
| `PIDMechanicalEquipment` | `IPIDTypical` | 4，与对表一致 | `IsTypical`（4） | 设备和图纸表 |
| `PIDProcessEquipment` | `IPIDTypical` | 13，与对表一致 | `IsTypical`（13） | 设备和图纸表 |
| `PIDProcessVessel` | `IPIDTypical` | 5，与对表一致 | `IsTypical`（5） | 设备和图纸表 |
| `PIDNozzle` | `IPIDTypical` | 36，与对表一致 | `IsTypical`（36） | 设备和图纸表 |
| `PIDDrawing` | `IDocument` | 10，与对表一致 | `DocCategory`（5）、`DocType`（5） | 设备和图纸表 |
| `PIDBranchPoint` | `IObject` | 174，与对表一致 | `UID`（58）、`Name`（58）、`Description`（58） | 设备和图纸表 |
| `PIDBranchPoint` | `IOffpageConnector` | 58，与对表一致 | `ToFromText`（58） | 设备和图纸表 |
| `PIDInstrument` | `IInstrumentOcc` | 116（与对表一致。I井场 58 + 分离器A 9 + 分离器B 9 + 低温分离器A 14 + 换热器CD 26） | `InstrumentLocation` | 仪表和信号表 |
| `PIDInstrument` | `IPBSItem` | 254（与对表一致。I井场 120 + 分离器A 20 + 分离器B 20 + 低温分离器A 30 + 换热器CD 64） | `ConstructionStatus`、`ConstructionStatus2` | 仪表和信号表 |
| `PIDInstrument` | `IPIDTypical` | 127（与对表一致。I井场 60 + 分离器A 10 + 分离器B 10 + 低温分离器A 15 + 换热器CD 32） | `IsTypical` | 仪表和信号表 |
| `PIDInlineInstrument` | `IInlineComponent` | 72（与对表一致。I井场 40 + 分离器A 2 + 分离器B 2 + 低温分离器A 5 + 换热器CD 23） | `IsFlowDirectional` | 仪表和信号表 |
| `PIDInlineInstrument` | `IPIDTypical` | 72（与对表一致。I井场 40 + 分离器A 2 + 分离器B 2 + 低温分离器A 5 + 换热器CD 23） | `IsTypical` | 仪表和信号表 |
| `PIDProcessPoint` | `IObject` | 341（与对表一致。I井场 169 + 分离器A 39 + 分离器B 29 + 低温分离器A 41 + 换热器CD 63） | `UID` | 仪表和信号表 |
| `PIDSignalConnector` | `IPIDTypical` | 83（与对表一致。I井场 46 + 分离器A 6 + 分离器B 6 + 低温分离器A 9 + 换热器CD 16） | `IsTypical` | 仪表和信号表 |
| `PIDSignalBranchPoint` | `IObject` | 16（与对表一致。I井场 10 + 分离器A 1 + 分离器B 1 + 低温分离器A 1 + 换热器CD 3） | `UID` | 仪表和信号表 |
| `PIDSignalPort` | `IObject` | 3450（与对表一致。I井场 1830 + 分离器A 214 + 分离器B 214 + 低温分离器A 394 + 换热器CD 798） | `UID`、`Name` | 仪表和信号表 |

30 行都是来源表里写「不发」的。来源表里写「发」的 12 行没有收进来。

## 核对

在三份缺口表里搜上表的「类 + 接口」。五图非空和、属性名应与该行相同，并且该行的「导出器今天发不发」是不发。已发的 12 个不应出现在本表。
