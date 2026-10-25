# map schema 说一个类发哪些接口，和五张参考图、和今天的 `pid_writer` 各差在哪（2026-10-11）

> 计划 `docs/plans/2026-10-11-publish-native-evidence-via-ida-bridge.md` 的 N2c。任务 t-913。
> 只读四份已验收底稿，不改它们：
>
> - `docs/analysis/n2c-map-schema-table.md`（`dlls/sppiddatamap.xml`，`SoftwareVersion="05.02.00.0016"`）
> - `docs/analysis/n2c-reference-measured-ijingchang.md`（I井场参考 `_Data.xml`，`SoftwareVersion="10.00.31.0023"`）
> - `docs/analysis/n2c-reference-measured-other-four.md`（分离器 A / B、低温分离器 A、换热器 CD，同一 `10.00.31.0023`）
> - `docs/analysis/n2c-exporter-interface-inventory.md`（今天 `pid_writer` 会发出的类和接口）
>
> 本页不开 IDA，不读 `EFAdapter.dll`。旧计划里「64 个类 / 2 752 条属性 / 2 条关系」不沿用；类、接口、属性的数以 map 底稿的实数为准：`SPMapClassDef` 31、`SPMapPropertyDef` 1 373、`SPMapRelDef` 1。
> **本轮未改任何 `src`、注释或测试，也没有提交。** 测试 NOT RUN：允许改的只有本页和计划 §7 的追加段。

## 一句话

map 是 SmartPlant P&ID **05.02** 的工具 schema，五张参考图是 **10.00.31** 的发布结果，两边不是同一份名单。
接口只写在 map 类表的 `ClassToMapClass` 上（154 格，其中 18 个发布类这一列是空的）。属性写在「发布类.属性」上（唯一 pair 581 个）。
参考里出现、map 接口列没有的接口 **211** 个；map 接口列有、五张图非空和为 0 的接口 **117** 个。
参考里非空、map 没有对应「发布类.属性」的 **49** 个；map 有、五张图非空和为 0 的属性 **491** 个。两边属性都非空的 **90** 个。
`pid_writer` 今天允许发出的接口，多数不在 map 的接口列里；它实际会写的那几个属性名，有的在 map 属性表里，有的不在。本页只对表，不改常量表的注释出处。

## 对照口径

第三方用那四份文件重算，按下面四条，不要把接口和属性混成一格。

1. **接口格**看 map 底稿「类」表的「接口」列（`ClassToMapClass`，顿号分隔；「—」是空集）。不看属性行。参考侧：两份实测底稿里四级标题（类名）下的接口行，五张图取并集。一个接口只要在任何一张图的表里出现，就算参考有；「非空」是该接口下所有属性的「非空次数」五图相加，大于 0 才算非空。裸接口（属性列是「—」）非空为 0。
2. **属性格**看 map 底稿「属性全表」的「发布类.属性」列。从单元格里抽出每个 `PID类.属性`；`；` 后面不带类名的重复属性名是同一对，只计一次；`（双向）` 去掉。`#612` 一行同时写出 `PIDPipingConnector.ScheduleOrThickness` 和 `PIDPipingConnector.ScheduleThickness2`，所以 580 行有类前缀，唯一 pair 是 581。参考侧按「类 + 属性名」对齐，**不看这个属性坐在哪个接口上**。
3. **全空** = 五张图的非空次数相加等于 0。没出现这一行，记「缺席」；出现了但非空和为 0，记「有行但五图非空和为 0」。两种都算全空。
4. `Rel` 不并进类对象。map 的 `Equipment`、`ControlSysIO` 没有发布类，不进下面两节的发布类格子；它们单独写在口径后面。

属性行核对：1 373 = 760 行发布侧是「—」+ 580 行含 `PID类.属性` + 33 行是裸属性名。这 33 行全部是图侧类 `Equipment`（map 底稿写它没有发布类），名字是：`EquipPrefix`、`EquipSeqNo`、`EquipSuff`、`Name`、`CoatingRequirement`、`DesignResponsibility`、`HTraceRqmt`、`HTraceMediumTemp`、`AvgInsulDens`、`InsulPurpose1`、`InsulSpec`、`InsulTemp`、`InsulThickSrc`、`TotalInsulThick`、`ConstructionStatus`、`CorrosionAllowance`、`RequisitionResponsibility`、`SteamOutPressure`、`SteamoutReq`、`SteamOutTemperature`、`SupplyResponsibility`、`CleaningRqmt`、`InsulCompositeMatl`、`SlopedEquipment_Angle`、`EquipmentTrimSpec`、`HeightRelativeToGrade`、`Description`、`CatalogPartNumber`、`CatalogName`、`LongMaterialDescription2`、`IOTagNumber`、`IOTagSystem`、`IOTagPrefix`。
没有发布类的图侧类还有 `ControlSysIO`（接口只有 `INamedControlSysIO`，map 类表第 48 行）。`Equipment` 自己的接口列在第 41 行。下面只计有发布类的子类，不把父类再计一遍。

## 参考有而 map 没有

### 类

| 参考有的类 | map 类表 | 依据 |
|---|---|---|
| `PIDProcessPoint` | 31 个 `SPMapClassDef` 里没有这个发布类 | 五张图的实测都有这一节（I井场、分离器 A/B、低温分离器 A、换热器 CD）。map 属性表却有挂到 `PIDProcessPoint.*` 的 pair，那些属性五图全空，见下一节 |

map 有、五张参考图一个对象都没有的发布类是 9 个：`PIDDuctBranchPoint`、`PIDDuctComponent`、`PIDDuctConnector`、`PIDDuctNozzle`、`PIDDuctPort`、`PIDInstrumentLoop`、`PIDProcessEquipmentComponent`、`PIDRoom`、`PIDRoomComponent`。它们放在「map 有而参考全空」。

### 接口（211）

参考里出现、该发布类的 `ClassToMapClass` 不含它的接口共 **211**。其中五图非空和大于 0 的 **42**，只是空壳（裸接口或属性全空）的 **169**。
`PIDInstrument`、`PIDDrawing`、`PIDNote`、`PIDRepresentation`、`PIDPipeline`、`PIDBranchPoint`、`PIDSignalConnector`、四个端口 / 分支点类的接口列都是「—」（map 类表对应行）。所以这些类在参考里的接口，全部落在这一节，即使属性表已经把 `MeasuredVariable` 这类名字映射到了同一个发布类。

非空的那些（第三方在实测底稿里搜这个接口，非空次数应大于 0；再看 map 类表同一发布类的接口列，应找不到这个名字）：

| 类 | 接口 | 五图非空和 | map 类表 |
|---|---|---:|---|
| `PIDBranchPoint` | `IObject` | 174 | 第 54 行，接口列不含它 |
| `PIDBranchPoint` | `IOffpageConnector` | 58 | 第 54 行，接口列不含它 |
| `PIDDrawing` | `IDocument` | 10 | 第 32 行，接口列不含它 |
| `PIDDrawing` | `IObject` | 10 | 第 32 行，接口列不含它 |
| `PIDHeatTransferEquipment` | `IPIDTypical` | 1 | 第 51 行，接口列不含它 |
| `PIDInlineInstrument` | `IInlineComponent` | 72 | 第 33 行，接口列不含它 |
| `PIDInlineInstrument` | `IPIDTypical` | 72 | 第 33 行，接口列不含它 |
| `PIDInstrument` | `IInstrumentOcc` | 116 | 第 34 行，接口列不含它 |
| `PIDInstrument` | `INamedInstrument` | 236 | 第 34 行，接口列不含它 |
| `PIDInstrument` | `IObject` | 245 | 第 34 行，接口列不含它 |
| `PIDInstrument` | `IPBSItem` | 254 | 第 34 行，接口列不含它 |
| `PIDInstrument` | `IPIDTypical` | 127 | 第 34 行，接口列不含它 |
| `PIDMechanicalEquipment` | `IPIDTypical` | 4 | 第 52 行，接口列不含它 |
| `PIDNote` | `INote` | 28 | 第 36 行，接口列不含它 |
| `PIDNote` | `IObject` | 56 | 第 36 行，接口列不含它 |
| `PIDNozzle` | `IPIDTypical` | 36 | 第 37 行，接口列不含它 |
| `PIDNozzle` | `IPipingSpecifiedItem` | 21 | 第 37 行，接口列不含它 |
| `PIDPipeline` | `IFluidSystem` | 149 | 第 49 行，接口列不含它 |
| `PIDPipeline` | `IObject` | 344 | 第 49 行，接口列不含它 |
| `PIDPipingBranchPoint` | `IObject` | 183 | 第 46 行，接口列不含它 |
| `PIDPipingComponent` | `IInlineComponent` | 264 | 第 38 行，接口列不含它 |
| `PIDPipingComponent` | `IPIDTypical` | 264 | 第 38 行，接口列不含它 |
| `PIDPipingConnector` | `IConnector` | 444 | 第 39 行，接口列不含它 |
| `PIDPipingConnector` | `IInsulatedItem` | 106 | 第 39 行，接口列不含它 |
| `PIDPipingConnector` | `INamedPipingConnector` | 219 | 第 39 行，接口列不含它 |
| `PIDPipingConnector` | `IPBSItem` | 682 | 第 39 行，接口列不含它 |
| `PIDPipingConnector` | `IPIDTypical` | 341 | 第 39 行，接口列不含它 |
| `PIDPipingConnector` | `IPipeCrossSectionItem` | 200 | 第 39 行，接口列不含它 |
| `PIDPipingConnector` | `IPipingConnector` | 341 | 第 39 行，接口列不含它 |
| `PIDPipingConnector` | `IPipingSpecifiedItem` | 261 | 第 39 行，接口列不含它 |
| `PIDPipingPort` | `IConnection` | 298 | 第 40 行，接口列不含它 |
| `PIDPipingPort` | `IObject` | 1436 | 第 40 行，接口列不含它 |
| `PIDPipingPort` | `IPipeCrossSectionItem` | 444 | 第 40 行，接口列不含它 |
| `PIDProcessEquipment` | `IPIDTypical` | 13 | 第 50 行，接口列不含它 |
| `PIDProcessPoint` | `IObject` | 341 | 类表没有这个发布类 |
| `PIDProcessVessel` | `IPIDTypical` | 5 | 第 53 行，接口列不含它 |
| `PIDRepresentation` | `IDrawingRepresentation` | 1823 | 第 43 行，接口列不含它 |
| `PIDRepresentation` | `IObject` | 1823 | 第 43 行，接口列不含它 |
| `PIDSignalBranchPoint` | `IObject` | 16 | 第 47 行，接口列不含它 |
| `PIDSignalConnector` | `IObject` | 83 | 第 44 行，接口列不含它 |
| `PIDSignalConnector` | `IPIDTypical` | 83 | 第 44 行，接口列不含它 |
| `PIDSignalPort` | `IObject` | 3450 | 第 45 行，接口列不含它 |

只是空壳的那些（参考有这一行，非空和为 0，map 接口列也没有）：

| 类 | 接口 |
|---|---|
| `PIDBranchPoint` | `IConnection`、`IDocumentItem`、`IDrawingItem`、`IDuctConnection`、`IPIDBranchPoint`、`IPipingConnection`、`ISignalConnection` |
| `PIDControlSystemFunction` | `IDocumentItem`、`IDrawingItem`、`IExpandableThing`、`IInstrument`、`ILoopMember`、`INoteCollection`、`IPlannedMatl`、`ISignalPortComposition` |
| `PIDDrawing` | `IDocVersionComposition`、`IDwgRepresentationComposition`、`IPBSItem`、`IPIDDrawing`、`ISchematicDwg` |
| `PIDHeatTransferEquipment` | `IDrawingItem`、`IHeatTransferEquipment`、`IHeatTransferEquipmentOcc`、`IPIDHeatTransferEquipment`、`IPIDHeatTransferEquipmentOcc`、`IPlannedMatl`、`IProcessEquipment`、`IProcessEquipmentOcc` |
| `PIDInlineInstrument` | `IDocumentItem`、`IDrawingItem`、`IDuctPortComposition`、`IElecPowerConsumer`、`IExpandableThing`、`IInlineComponentOcc`、`IInlineInstrument`、`IInlineInstrumentOcc`、`IInstrumentOcc`、`ILoopMember`、`INoteCollection`、`IPart`、`IPartOcc`、`IPipingPortComposition`、`IPlannedMatl`、`IProcessPointCollection`、`ISignalPortComposition` |
| `PIDInstrument` | `IDocumentItem`、`IDrawingItem`、`IElecPowerConsumer`、`IExpandableThing`、`IHeatTracedItem`、`IInlineComponentOcc`、`IInstrument`、`ILoopMember`、`INoteCollection`、`IPart`、`IPartOcc`、`IPipingPortComposition`、`IPlannedMatl`、`IPressureReliefItem`、`IProcessPointCollection`、`ISignalPortComposition` |
| `PIDMechanicalEquipment` | `IDrawingItem`、`IElectricMotor`、`IElectricMotorOcc`、`IFluidTransferMachineOcc`、`IMaterialTransferEquipment`、`IMaterialTransferEquipmentOcc`、`IPIDMechanicalEquipment`、`IPIDMechanicalEquipmentOcc`、`IPlannedMatl`、`IPositiveDisplacementPump`、`IPositiveDisplacementPumpOcc`、`IProcessEquipment`、`IProcessEquipmentDriverOcc`、`IProcessEquipmentOcc` |
| `PIDNote` | `IDocumentItem`、`IDrawingItem`、`IPBSNote` |
| `PIDNozzle` | `IDocumentItem`、`IDrawingItem`、`IElecPowerConsumer`、`IEquipmentComponentOcc`、`IFabricatedItem`、`INoteCollection`、`INozzle`、`INozzleOcc`、`IPBSItemCollection`、`IPart`、`IPartOcc`、`IPipingPortComposition`、`IPlannedMatl`、`IProcessDataCaseComposition`、`IProcessPointCollection`、`ISignalPortComposition` |
| `PIDPipeline` | `IExpandableThing`、`INoteCollection`、`IPBSItem`、`IPBSItemCollection`、`IPIDTypical`、`IPipeline`、`IPipingConnectorComposition`、`IPlannedFacility` |
| `PIDPipingBranchPoint` | `IConnection`、`IDocumentItem`、`IDrawingItem`、`IPipingBranchPoint`、`IPipingConnection` |
| `PIDPipingComponent` | `IDocumentItem`、`IDrawingItem`、`IElecPowerConsumer`、`IFabricatedItem`、`IInlineComponentOcc`、`INoteCollection`、`IPart`、`IPartOcc`、`IPipingComponentOcc`、`IPipingPortComposition`、`IPlannedMatl` |
| `PIDPipingConnector` | `IDocumentItem`、`IDrawingItem`、`IElecPowerConsumer`、`IExpandableThing`、`IFabricatedItem`、`IHeatTracedItem`、`IJacketedItem`、`INoteCollection`、`IPBSItemCollection`、`IPlannedFacility`、`IProcessDataCaseComposition`、`IProcessPointCollection` |
| `PIDPipingPort` | `IPipingConnection`、`IPipingPort`、`IPort` |
| `PIDProcessEquipment` | `IDrawingItem`、`IPBSItemCollection`、`IPlannedMatl` |
| `PIDProcessPoint` | `IBulkBaseProcessPoint`、`IBulkMaxProcessPoint`、`IBulkMinProcessPoint`、`IBulkNormProcessPoint`、`IFacilityPoint`、`IProcessPoint`、`IProcessPointCaseComposition` |
| `PIDProcessVessel` | `IDrawingItem`、`IPBSItemCollection`、`IPIDProcessVessel`、`IPIDProcessVesselOcc`、`IPlannedMatl`、`IProcessEquipment`、`IProcessEquipmentOcc`、`IProcessVesselOcc` |
| `PIDSignalBranchPoint` | `IConnection`、`IDocumentItem`、`IDrawingItem`、`IFacilityPoint`、`ISignalBranchPoint`、`ISignalConnection` |
| `PIDSignalConnector` | `IConnector`、`IDocumentItem`、`IDrawingItem`、`IExpandableThing`、`IPlannedFacility`、`ISignalConnector` |
| `PIDSignalPort` | `IConnection`、`IFacilityPoint`、`IPIDTypical`、`IPipingConnection`、`ISignalConnection`、`ISignalPort` |

### 属性（非空 49，另有只出现空值的 3）

**21** 个是 `UID`。map 属性表没有任何「发布类.UID」。参考每个类的 `IObject.UID` 都非空。类名：`PIDBranchPoint`、`PIDControlSystemFunction`、`PIDDrawing`、`PIDHeatTransferEquipment`、`PIDInlineInstrument`、`PIDInstrument`、`PIDMechanicalEquipment`、`PIDNote`、`PIDNozzle`、`PIDPipeline`、`PIDPipingBranchPoint`、`PIDPipingComponent`、`PIDPipingConnector`、`PIDPipingPort`、`PIDProcessEquipment`、`PIDProcessPoint`、`PIDProcessVessel`、`PIDRepresentation`、`PIDSignalBranchPoint`、`PIDSignalConnector`、`PIDSignalPort`。

其余 **28** 个非空属性，map 的「发布类.属性」里没有这一对。五图非空和是两份实测底稿该属性「非空次数」相加。

| 发布类.属性 | 五图非空和 | 出现于 |
|---|---:|---|
| `PIDControlSystemFunction.ConstructionStatus2` | 88 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDDrawing.DocCategory` | 5 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDDrawing.DocType` | 5 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDHeatTransferEquipment.ConstructionStatus2` | 1 | 换热器CD |
| `PIDHeatTransferEquipment.EqType1` | 1 | 换热器CD |
| `PIDInlineInstrument.ConstructionStatus2` | 72 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDInlineInstrument.InstrumentType` | 46 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDInlineInstrument.InstrumentType3` | 46 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDInstrument.ConstructionStatus2` | 127 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDMechanicalEquipment.ConstructionStatus2` | 4 | I井场 |
| `PIDMechanicalEquipment.EqType1` | 4 | I井场 |
| `PIDMechanicalEquipment.EqType2` | 4 | I井场 |
| `PIDMechanicalEquipment.EqType3` | 2 | I井场 |
| `PIDMechanicalEquipment.EqType4` | 2 | I井场 |
| `PIDMechanicalEquipment.EqType5` | 2 | I井场 |
| `PIDNozzle.ConstructionStatus2` | 36 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDNozzle.ProcessEqCompType2` | 36 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDPipeline.FluidSystem` | 145 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDPipingComponent.ConstructionStatus2` | 264 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDPipingComponent.PipingComponentType2` | 262 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDPipingComponent.PipingComponentType3` | 210 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDPipingConnector.ConstructionStatus2` | 341 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDProcessEquipment.ConstructionStatus2` | 13 | I井场、换热器CD |
| `PIDProcessVessel.ConstructionStatus2` | 5 | I井场、低温分离器A、分离器A、分离器B |
| `PIDProcessVessel.EqType1` | 5 | I井场、低温分离器A、分离器A、分离器B |
| `PIDProcessVessel.EqType2` | 5 | I井场、低温分离器A、分离器A、分离器B |
| `PIDProcessVessel.EqType3` | 5 | I井场、低温分离器A、分离器A、分离器B |
| `PIDProcessVessel.EqType4` | 5 | I井场、低温分离器A、分离器A、分离器B |

参考里出现了、但五图非空和为 0、map 也没有的属性 **3** 个：`PIDDrawing.DocSubtype`、`PIDPipingConnector.PipingConnectorPrefix`、`PIDSignalConnector.FlowDirection`。

## map 有而参考全空

### 类

这 9 个发布类在两份实测底稿里都没有 `####` 节，五张图的参考 `_Data.xml` 没写出它们：

| 发布类 | map 图侧类 | 类表行 | 接口列条数 | 已映射属性 pair |
|---|---|---:|---:|---:|
| `PIDDuctBranchPoint` | `DuctingBranchPoint` | 61 | 0 | 0 |
| `PIDDuctComponent` | `DuctingComp` | 56 | 0 | 13 |
| `PIDDuctConnector` | `DuctRun` | 55 | 0 | 71 |
| `PIDDuctNozzle` | `DuctNozzle` | 60 | 0 | 17 |
| `PIDDuctPort` | `DuctingPoint` | 57 | 0 | 3 |
| `PIDInstrumentLoop` | `InstrLoop` | 35 | 4 | 8 |
| `PIDProcessEquipmentComponent` | `EquipComponent` | 42 | 13 | 29 |
| `PIDRoom` | `Room` | 58 | 0 | 5 |
| `PIDRoomComponent` | `RoomComponent` | 59 | 0 | 3 |

`PIDDuctBranchPoint` 的接口列是空的，属性表也没有挂到它的 `PID类.属性`（唯一一行是 `Placeholder3`，发布侧「—」）。它仍然是 map 有、参考没有的类。

### 接口（117）

map 类表接口列里的名字，五张图非空和为 0 的共 **117**。接口列有、而且参考非空的是 **37**，列在后面「两边都非空」，不在本节。
「缺席」= 该类在实测里没有这个接口行（类本身没出现，或出现了但没有这个接口）。「有行但五图非空和为 0」= 五张图里至少一张写了这个接口，属性全空或是裸接口。

| 类 | 全空接口 |
|---|---|
| `PIDControlSystemFunction` | `IControlSystemFunction`（有行但五图非空和为 0） |
| `PIDHeatTransferEquipment` | `IAirCooledExchanger`（缺席）、`ICleanedItem`（缺席）、`ICoatedItem`（缺席）、`IDesignedItem`（缺席）、`IElecPowerConsumer`（缺席）、`IEquipmentOcc`（有行但五图非空和为 0）、`IFabricatedItem`（缺席）、`IHeatTracedItem`（缺席）、`IInsulatedItem`（缺席）、`IPartOcc`（缺席）、`IProcessWettedItem`（缺席）、`IRequisitionedItem`（缺席）、`IShellAndTubeHeatExchanger`（有行但五图非空和为 0）、`ISpecifiedMatlItem`（缺席）、`ISteamedItem`（缺席）、`ISuppliedItem`（缺席） |
| `PIDInlineInstrument` | `ICleanedItem`（缺席）、`ICoatedItem`（缺席）、`IHeatTracedItem`（有行但五图非空和为 0）、`IPipingSpecifiedItem`（缺席）、`IPressureDropItem`（缺席）、`IProcessWettedItem`（缺席）、`IReliefDevice`（缺席）、`IRequisitionedItem`（缺席）、`ISuppliedItem`（缺席） |
| `PIDInstrumentLoop` | `IInstrumentLoop`（缺席）、`INamedInstrumentLoop`（缺席）、`IObject`（缺席）、`IPBSItem`（缺席） |
| `PIDMechanicalEquipment` | `ICleanedItem`（缺席）、`ICoatedItem`（缺席）、`IDesignedItem`（缺席）、`IElecPowerConsumer`（缺席）、`IEquipmentDriver`（有行但五图非空和为 0）、`IEquipmentOcc`（有行但五图非空和为 0）、`IFabricatedItem`（缺席）、`IFluidTransferMachine`（有行但五图非空和为 0）、`IHeatTracedItem`（缺席）、`IInsulatedItem`（缺席）、`IPartOcc`（缺席）、`IPressureRatedObject`（缺席）、`IProcessPlantEquipment`（缺席）、`IProcessWettedItem`（缺席）、`IPump`（有行但五图非空和为 0）、`IPumpOcc`（有行但五图非空和为 0）、`IRequisitionedItem`（缺席）、`ISpecifiedMatlItem`（缺席）、`ISteamedItem`（缺席）、`ISuppliedItem`（缺席） |
| `PIDNozzle` | `ICleanedItem`（缺席）、`ICoatedItem`（缺席）、`IDesignedItem`（缺席）、`IHeatTracedItem`（有行但五图非空和为 0）、`IInsulatedItem`（缺席）、`INamedEquipmentComponent`（缺席）、`IProcessWettedItem`（缺席）、`IRequisitionedItem`（缺席）、`ISteamedItem`（缺席）、`ISuppliedItem`（缺席） |
| `PIDPipingComponent` | `ICleanedItem`（缺席）、`ICoatedItem`（缺席）、`IDesignedItem`（缺席）、`IHeatTracedItem`（有行但五图非空和为 0）、`IPressureDropItem`（缺席）、`IPressureRatedObject`（缺席）、`IPressureReliefItem`（有行但五图非空和为 0）、`IProcessWettedItem`（缺席）、`IReliefDevice`（缺席）、`IRequisitionedItem`（缺席）、`ISuppliedItem`（缺席） |
| `PIDPipingConnector` | `IFluid`（缺席）、`IHazardousMaterial`（缺席）、`ILiquid`（缺席）、`ILiquidSolid`（缺席）、`IMovingPhase`（缺席）、`IProcessDataCase`（缺席）、`IProcessDataPhase`（缺席） |
| `PIDProcessEquipment` | `ICleanedItem`（缺席）、`ICoatedItem`（缺席）、`IDesignedItem`（缺席）、`IEquipmentOcc`（有行但五图非空和为 0）、`IFabricatedItem`（缺席）、`IHeatTracedItem`（缺席）、`IInsulatedItem`（缺席）、`IPartOcc`（缺席）、`IProcessWettedItem`（缺席）、`IRequisitionedItem`（缺席）、`ISteamedItem`（缺席）、`ISuppliedItem`（缺席） |
| `PIDProcessEquipmentComponent` | `ICleanedItem`（缺席）、`ICoatedItem`（缺席）、`IDesignedItem`（缺席）、`IEquipmentComponent`（缺席）、`IHeatTracedItem`（缺席）、`IInsulatedItem`（缺席）、`INamedEquipmentComponent`（缺席）、`IObject`（缺席）、`IPBSItem`（缺席）、`IProcessWettedItem`（缺席）、`IRequisitionedItem`（缺席）、`ISteamedItem`（缺席）、`ISuppliedItem`（缺席） |
| `PIDProcessVessel` | `ICleanedItem`（缺席）、`ICoatedItem`（缺席）、`IDesignedItem`（缺席）、`IEquipmentOcc`（有行但五图非空和为 0）、`IHeatTracedItem`（缺席）、`IInsulatedItem`（缺席）、`ILowPressureTank`（有行但五图非空和为 0）、`IPartOcc`（缺席）、`IProcessVessel`（有行但五图非空和为 0）、`IProcessWettedItem`（缺席）、`IRequisitionedItem`（缺席）、`ISpecifiedMatlItem`（缺席）、`ISteamedItem`（缺席）、`ISuppliedItem`（缺席） |

### 属性（491）

581 个「发布类.属性」里，五图非空和为 0 的 **491**，非空的 **90**。
其中 **149** 个落在参考里根本没有对象的 9 个类上。
另外 **342** 个落在参考里有对象的类上，只是这个属性五图没有非空值。这 342 里包含 `PIDProcessPoint` 的 57 个：类表没有这个发布类，属性表有 pair；参考这个类的非空属性只有 `UID`，所以这 57 个记在「参考有该类对象」那张表。

参考没有该类对象的：

| 类 | 全空属性 |
|---|---|
| `PIDDuctComponent` | `AvgInsulDens`（缺席）、`ConstructionStatus`（缺席）、`Description`（缺席）、`InsulCompositeMatl`（缺席）、`InsulPurpose1`（缺席）、`InsulSpec`（缺席）、`InsulTemp`（缺席）、`InsulThickSrc`（缺席）、`IsTypical`（缺席）、`Name`（缺席）、`RequisitionResponsibility`（缺席）、`SupplyResponsibility`（缺席）、`TotalInsulThick`（缺席） |
| `PIDDuctConnector` | `AltDesignPressMax`（缺席）、`AltDesignPressMin`（缺席）、`AltDesignTempMax`（缺席）、`AltDesignTempMin`（缺席）、`AvgInsulDens`（缺席）、`CoatingRequirement`（缺席）、`CompressibilityBase`（缺席）、`CompressibilityMax`（缺席）、`CompressibilityMin`（缺席）、`CompressibilityNorm`（缺席）、`ConstructionStatus`（缺席）、`Corrosive`（缺席）、`CpCvRatioMax`（缺席）、`CpCvRatioMin`（缺席）、`CpCvRatioNorm`（缺席）、`CriticalPressureMax`（缺席）、`CriticalPressureMin`（缺席）、`CriticalPressureNorm`（缺席）、`Description`（缺席）、`Erosive`（缺席）、`EstimatedLength`（缺席）、`FlowDirection`（缺席）、`InsulCompositeMatl`（缺席）、`InsulPurpose1`（缺席）、`InsulSpec`（缺席）、`InsulTemp`（缺席）、`InsulThickSrc`（缺席）、`IsTypical`（缺席）、`MassDensityBase`（缺席）、`MassDensityMax`（缺席）、`MassDensityMin`（缺席）、`MassDensityNorm`（缺席）、`MassFlowRateMax`（缺席）、`MassFlowRateMin`（缺席）、`MassFlowRateNorm`（缺席）、`MassSpecificGravityBase`（缺席）、`MassSpecificGravityMax`（缺席）、`MassSpecificGravityMin`（缺席）、`MassSpecificGravityNorm`（缺席）、`MaterialBulkPhaseMax`（缺席）、`MaterialBulkPhaseMin`（缺席）、`MaterialBulkPhaseNorm`（缺席）、`MolecularWeightBase`（缺席）、`MolecularWeightMax`（缺席）、`MolecularWeightMin`（缺席）、`MolecularWeightNorm`（缺席）、`Name`（缺席）、`NormDesignPressMax`（缺席）、`NormDesignPressMin`（缺席）、`NormDesignTempMax`（缺席）、`NormDesignTempMin`（缺席）、`PhaseTemperatureBase`（缺席）、`PhaseTemperatureMax`（缺席）、`PhaseTemperatureMin`（缺席）、`PhaseTemperatureNorm`（缺席）、`PourPointTMax`（缺席）、`PourPointTMin`（缺席）、`PourPointTNorm`（缺席）、`PressureBase`（缺席）、`PressureMax`（缺席）、`PressureMin`（缺席）、`PressureNorm`（缺席）、`RepresentationsAreAllZeroLength`（缺席）、`TotalInsulThick`（缺席）、`Toxic`（缺席）、`VaporPressureMax`（缺席）、`VaporPressureMin`（缺席）、`VaporPressureNorm`（缺席）、`ViscosityMax`（缺席）、`ViscosityMin`（缺席）、`ViscosityNorm`（缺席） |
| `PIDDuctNozzle` | `AvgInsulDens`（缺席）、`ConstructionStatus`（缺席）、`Description`（缺席）、`DesignResponsibility`（缺席）、`EquipComponentPrefix`（缺席）、`EquipComponentSeqNo`（缺席）、`EquipComponentSuff`（缺席）、`InsulCompositeMatl`（缺席）、`InsulPurpose1`（缺席）、`InsulSpec`（缺席）、`InsulTemp`（缺席）、`InsulThickSrc`（缺席）、`IsTypical`（缺席）、`Name`（缺席）、`ProcEqpCompTypeDescription`（缺席）、`ProcessEqCompType1`（缺席）、`TotalInsulThick`（缺席） |
| `PIDDuctPort` | `ConnectionFlowDirection`（缺席）、`Description`（缺席）、`Name`（缺席） |
| `PIDInstrumentLoop` | `ConstructionStatus`（缺席）、`Description`（缺席）、`ExpansionType`（缺席）、`LoopIdentifier`（缺席）、`LoopPrefix`（缺席）、`LoopSequenceNo`（缺席）、`LoopSuff`（缺席）、`Name`（缺席） |
| `PIDProcessEquipmentComponent` | `AverageActiveLoad`（缺席）、`AvgInsulDens`（缺席）、`CleaningRqmt`（缺席）、`CoatingRequirement`（缺席）、`ConstructionStatus`（缺席）、`CorrosionAllowance`（缺席）、`Description`（缺席）、`EquipComponentPrefix`（缺席）、`EquipComponentSeqNo`（缺席）、`EquipComponentSuff`（缺席）、`HTraceMediumTemp`（缺席）、`HTraceRqmt`（缺席）、`HeightRelativeToGrade`（缺席）、`InsulCompositeMatl`（缺席）、`InsulPurpose1`（缺席）、`InsulSpec`（缺席）、`InsulTemp`（缺席）、`InsulThickSrc`（缺席）、`IsTypical`（缺席）、`LongMaterialDescription`（缺席）、`Name`（缺席）、`ProcEqpCompTypeDescription`（缺席）、`ProcessEqCompType1`（缺席）、`RequisitionResponsibility`（缺席）、`SteamOutPressure`（缺席）、`SteamOutTemperature`（缺席）、`SteamoutReq`（缺席）、`SupplyResponsibility`（缺席）、`TotalInsulThick`（缺席） |
| `PIDRoom` | `Description`（缺席）、`ExplosionCategory`（缺席）、`IsTypical`（缺席）、`Name`（缺席）、`WindowSize`（缺席） |
| `PIDRoomComponent` | `Description`（缺席）、`IsTypical`（缺席）、`Name`（缺席） |

参考有该类对象、该属性仍全空的：

| 类 | 全空属性 |
|---|---|
| `PIDControlSystemFunction` | `Description`（缺席）、`ExpansionType`（缺席）、`InstrFuncModifier`（有行但五图非空和为 0）、`InstrLoopSuffix`（有行但五图非空和为 0）、`InstrTagPrefix`（有行但五图非空和为 0）、`InstrTagSuffix`（有行但五图非空和为 0） |
| `PIDDrawing` | `Description`（有行但五图非空和为 0）、`DocTitle`（有行但五图非空和为 0） |
| `PIDHeatTransferEquipment` | `AirCooledExchanger_BundlePerBay`（缺席）、`AirCooledExchanger_FanNumberPerBay`（缺席）、`AirCooledExchanger_NumberOfBays`（缺席）、`AirCooledExchanger_PowerPerFanAtDesignTemperature`（缺席）、`AirCooledExchanger_TypeDraft`（缺席）、`AverageActiveLoad`（缺席）、`AvgInsulDens`（缺席）、`CatalogName`（缺席）、`CatalogPartNumber`（缺席）、`CleaningRqmt`（缺席）、`CoatingRequirement`（缺席）、`CorrosionAllowance`（缺席）、`Description`（缺席）、`DesignResponsibility`（缺席）、`EquipSuff`（有行但五图非空和为 0）、`EquipmentTrimSpec`（缺席）、`HTraceMediumTemp`（缺席）、`HTraceRqmt`（缺席）、`HeightRelativeToGrade`（缺席）、`InsulCompositeMatl`（缺席）、`InsulPurpose1`（缺席）、`InsulSpec`（缺席）、`InsulTemp`（缺席）、`InsulThickSrc`（缺席）、`LongMaterialDescription`（缺席）、`RequisitionResponsibility`（缺席）、`ShellAndTubeHeatExchanger_NumberOfTubes`（缺席）、`ShellAndTubeHeatExchanger_ShellDiameterInner`（缺席）、`ShellAndTubeHeatExchanger_TubeCorrosionAllowance`（缺席）、`ShellAndTubeHeatExchanger_TubeLength`（缺席）、`ShellAndTubeHeatExchanger_TubeMaterialClass`（缺席）、`SlopedEquipment_Angle`（缺席）、`SteamOutPressure`（缺席）、`SteamOutTemperature`（缺席）、`SteamoutReq`（缺席）、`SupplyResponsibility`（缺席）、`TotalInsulThick`（缺席） |
| `PIDInlineInstrument` | `AvgInsulDens`（缺席）、`CatalogName`（缺席）、`CatalogPartNumber`（缺席）、`CleaningRqmt`（缺席）、`CoatingRequirement`（缺席）、`CorrosionAllowance`（缺席）、`Description`（缺席）、`ExpansionType`（缺席）、`HTraceMediumTemp`（缺席）、`HTraceRqmt`（有行但五图非空和为 0）、`InstrFuncModifier`（有行但五图非空和为 0）、`InstrLoopSuffix`（有行但五图非空和为 0）、`InstrTagPrefix`（有行但五图非空和为 0）、`InstrTagSuffix`（有行但五图非空和为 0）、`InstrumentLocation`（缺席）、`InsulCompositeMatl`（缺席）、`InsulPurpose1`（缺席）、`InsulTemp`（缺席）、`InsulThickSrc`（缺席）、`OrificeLetter`（缺席）、`PipingMaterialsClass`（缺席）、`PressureDrop`（缺席）、`PressureRating`（缺席）、`RequisitionResponsibility`（缺席）、`ScheduleOrThickness`（缺席）、`SupplyResponsibility`（缺席）、`TotalInsulThick`（缺席） |
| `PIDInstrument` | `AvgInsulDens`（缺席）、`CatalogName`（缺席）、`CleaningRqmt`（缺席）、`Description`（缺席）、`ExpansionType`（缺席）、`HTraceMediumTemp`（缺席）、`HTraceRqmt`（有行但五图非空和为 0）、`InstrFuncModifier`（有行但五图非空和为 0）、`InstrLoopSuffix`（有行但五图非空和为 0）、`InstrTagPrefix`（有行但五图非空和为 0）、`InstrTagSuffix`（有行但五图非空和为 0）、`InsulCompositeMatl`（缺席）、`InsulPurpose1`（缺席）、`InsulSpec`（缺席）、`InsulTemp`（缺席）、`InsulThickSrc`（缺席）、`NominalDiameter`（缺席）、`OrificeLetter`（缺席）、`PipingMaterialsClass`（缺席）、`PressureDrop`（缺席）、`ProcFunc`（缺席）、`ReliefPressure`（缺席）、`RequisitionResponsibility`（缺席）、`SupplyResponsibility`（缺席）、`TotalInsulThick`（缺席） |
| `PIDMechanicalEquipment` | `AverageActiveLoad`（缺席）、`AvgInsulDens`（缺席）、`CatalogName`（缺席）、`CatalogPartNumber`（缺席）、`CleaningRqmt`（缺席）、`CoatingRequirement`（缺席）、`CorrosionAllowance`（缺席）、`DesignResponsibility`（缺席）、`EquipSuff`（有行但五图非空和为 0）、`EquipmentTrimSpec`（缺席）、`FluidTransferMachine_Capacity`（缺席）、`FluidTransferMachine_RatedPower`（缺席）、`HTraceMediumTemp`（缺席）、`HTraceRqmt`（缺席）、`HeightRelativeToGrade`（缺席）、`InsulCompositeMatl`（缺席）、`InsulPurpose1`（缺席）、`InsulSpec`（缺席）、`InsulTemp`（缺席）、`InsulThickSrc`（缺席）、`PressureRating`（缺席）、`ProcessEquipmentDriverPower`（缺席）、`ProcessPlantEquipment_DischargePressureRated`（缺席）、`Pump_CoolingWaterPlan`（缺席）、`Pump_SealFlushPipingPlanDescription`（缺席）、`RequisitionResponsibility`（缺席）、`SlopedEquipment_Angle`（缺席）、`SteamOutPressure`（缺席）、`SteamOutTemperature`（缺席）、`SteamoutReq`（缺席）、`SupplyResponsibility`（缺席）、`TotalInsulThick`（缺席） |
| `PIDNote` | `Author`（缺席）、`Description`（缺席） |
| `PIDNozzle` | `AvgInsulDens`（缺席）、`CleaningRqmt`（缺席）、`CoatingRequirement`（缺席）、`CorrosionAllowance`（缺席）、`Description`（缺席）、`DesignResponsibility`（缺席）、`EquipComponentPrefix`（缺席）、`EquipComponentSeqNo`（缺席）、`EquipComponentSuff`（缺席）、`HTraceMediumTemp`（缺席）、`HTraceRqmt`（有行但五图非空和为 0）、`InsulCompositeMatl`（缺席）、`InsulPurpose1`（缺席）、`InsulSpec`（缺席）、`InsulTemp`（缺席）、`InsulThickSrc`（缺席）、`Name`（缺席）、`PressureRating`（缺席）、`RequisitionResponsibility`（缺席）、`ScheduleOrThickness`（缺席）、`SteamOutPressure`（缺席）、`SteamOutTemperature`（缺席）、`SteamoutReq`（缺席）、`SupplyResponsibility`（缺席）、`TotalInsulThick`（缺席） |
| `PIDPipeline` | `ConstructionStatus`（缺席）、`Description`（缺席）、`IsTypical`（缺席） |
| `PIDPipingComponent` | `AvgInsulDens`（缺席）、`CatalogName`（缺席）、`CatalogPartNumber`（缺席）、`CleaningRqmt`（缺席）、`CoatingRequirement`（缺席）、`CorrosionAllowance`（缺席）、`Description`（缺席）、`DesignResponsibility`（缺席）、`HTraceMediumTemp`（缺席）、`HTraceRqmt`（有行但五图非空和为 0）、`InsulCompositeMatl`（缺席）、`InsulPurpose1`（缺席）、`InsulTemp`（缺席）、`InsulThickSrc`（缺席）、`OrificeLetter`（缺席）、`PipCommodOption1`（缺席）、`PressureDrop`（缺席）、`PressureRating`（缺席）、`ReliefPressure`（缺席）、`RequisitionResponsibility`（缺席）、`ScheduleOrThickness`（缺席）、`SupplyResponsibility`（缺席）、`TotalInsulThick`（缺席） |
| `PIDPipingConnector` | `AvgInsulDens`（缺席）、`CleaningRqmt`（缺席）、`CoatingRequirement`（缺席）、`CorrosionAllowance`（缺席）、`Description`（缺席）、`DesignResponsibility`（缺席）、`EstimatedLength`（缺席）、`FluidServiceCategory`（缺席）、`HTraceMediumTemp`（缺席）、`HTraceRqmt`（有行但五图非空和为 0）、`InsulCompositeMatl`（缺席）、`InsulPurpose1`（缺席）、`InsulTemp`（缺席）、`InsulThickSrc`（缺席）、`Jacket_CleaningRqmt`（缺席）、`Jacket_CorrosionAllowance`（缺席）、`Jacket_FluidSystem`（缺席）、`Jacket_InsulPurpose1`（缺席）、`Jacket_InsulSpec`（缺席）、`Jacket_InsulTemp`（缺席）、`Jacket_ItemTag`（缺席）、`Jacket_MaterialOfConstructionClass`（缺席）、`Jacket_NominalDiameter`（缺席）、`Jacket_OperatingFluidCode`（缺席）、`Jacket_SteamOutPressure`（缺席）、`Jacket_SteamoutReq`（缺席）、`Jacket_SteamoutTemperature`（缺席）、`Jacket_TotalInsulThick`（缺席）、`PipingConnectorSuff`（有行但五图非空和为 0）、`ScheduleOrThickness`（缺席）、`ScheduleThickness2`（缺席）、`SlopedPipeDirection`（缺席）、`SlopedPipingAngle`（缺席）、`SteamOutPressure`（缺席）、`SteamOutTemperature`（缺席）、`SteamoutReq`（缺席）、`SupplyResponsibility`（缺席）、`TotalInsulThick`（缺席）、`isStressAnalysisReqd`（缺席） |
| `PIDPipingPort` | `Description`（缺席）、`PressureRating`（缺席）、`ScheduleOrThickness`（缺席） |
| `PIDProcessEquipment` | `AverageActiveLoad`（缺席）、`AvgInsulDens`（缺席）、`CatalogName`（缺席）、`CatalogPartNumber`（缺席）、`CleaningRqmt`（缺席）、`CoatingRequirement`（缺席）、`CorrosionAllowance`（缺席）、`DesignResponsibility`（缺席）、`EquipSuff`（有行但五图非空和为 0）、`HTraceMediumTemp`（缺席）、`HTraceRqmt`（缺席）、`HeightRelativeToGrade`（缺席）、`InsulCompositeMatl`（缺席）、`InsulPurpose1`（缺席）、`InsulSpec`（缺席）、`InsulTemp`（缺席）、`InsulThickSrc`（缺席）、`RequisitionResponsibility`（缺席）、`SlopedEquipment_Angle`（缺席）、`SteamOutPressure`（缺席）、`SteamOutTemperature`（缺席）、`SteamoutReq`（缺席）、`SupplyResponsibility`（缺席）、`TotalInsulThick`（缺席） |
| `PIDProcessPoint` | `AltDesignPressMax`（缺席）、`AltDesignPressMin`（缺席）、`AltDesignTempMax`（缺席）、`AltDesignTempMin`（缺席）、`CompressibilityBase`（缺席）、`CompressibilityMax`（缺席）、`CompressibilityMin`（缺席）、`CompressibilityNorm`（缺席）、`Corrosive`（缺席）、`CpCvRatioMax`（缺席）、`CpCvRatioMin`（缺席）、`CpCvRatioNorm`（有行但五图非空和为 0）、`CriticalPressureMax`（缺席）、`CriticalPressureMin`（缺席）、`CriticalPressureNorm`（缺席）、`Erosive`（缺席）、`MassDensityBase`（缺席）、`MassDensityMax`（缺席）、`MassDensityMin`（缺席）、`MassDensityNorm`（缺席）、`MassFlowRateMax`（缺席）、`MassFlowRateMin`（缺席）、`MassFlowRateNorm`（缺席）、`MassSpecificGravityBase`（缺席）、`MassSpecificGravityMax`（缺席）、`MassSpecificGravityMin`（缺席）、`MassSpecificGravityNorm`（缺席）、`MaterialBulkPhaseMax`（缺席）、`MaterialBulkPhaseMin`（缺席）、`MaterialBulkPhaseNorm`（缺席）、`MolecularWeightBase`（缺席）、`MolecularWeightMax`（缺席）、`MolecularWeightMin`（缺席）、`MolecularWeightNorm`（有行但五图非空和为 0）、`Name`（缺席）、`NormDesignPressMax`（缺席）、`NormDesignPressMin`（缺席）、`NormDesignTempMax`（缺席）、`NormDesignTempMin`（缺席）、`PhaseTemperatureBase`（有行但五图非空和为 0）、`PhaseTemperatureMax`（有行但五图非空和为 0）、`PhaseTemperatureMin`（有行但五图非空和为 0）、`PhaseTemperatureNorm`（缺席）、`PourPointTMax`（缺席）、`PourPointTMin`（缺席）、`PourPointTNorm`（缺席）、`PressureBase`（有行但五图非空和为 0）、`PressureMax`（有行但五图非空和为 0）、`PressureMin`（有行但五图非空和为 0）、`PressureNorm`（缺席）、`Toxic`（缺席）、`VaporPressureMax`（缺席）、`VaporPressureMin`（缺席）、`VaporPressureNorm`（缺席）、`ViscosityMax`（缺席）、`ViscosityMin`（缺席）、`ViscosityNorm`（缺席） |
| `PIDProcessVessel` | `AverageActiveLoad`（缺席）、`AvgInsulDens`（缺席）、`CatalogName`（缺席）、`CatalogPartNumber`（缺席）、`CleaningRqmt`（缺席）、`CoatingRequirement`（缺席）、`CorrosionAllowance`（缺席）、`Description`（缺席）、`DesignResponsibility`（缺席）、`EquipSuff`（有行但五图非空和为 0）、`EquipmentTrimSpec`（缺席）、`HTraceMediumTemp`（缺席）、`HTraceRqmt`（缺席）、`HeightRelativeToGrade`（缺席）、`InsulCompositeMatl`（缺席）、`InsulPurpose1`（缺席）、`InsulSpec`（缺席）、`InsulTemp`（缺席）、`InsulThickSrc`（缺席）、`LongMaterialDescription`（缺席）、`LowPressureTank_OverfillProtectionLevel`（缺席）、`ProcessVessel_InternalDiameter`（缺席）、`ProcessVessel_TangentTangentLength`（缺席）、`ProcessVessel_VesselMaximumLiquidLevel`（缺席）、`ProcessVessel_VesselNormalLiquidLevel`（缺席）、`ProcessVessel_VesselVolumetricCapacity`（缺席）、`RequisitionResponsibility`（缺席）、`SlopedEquipment_Angle`（缺席）、`SteamOutPressure`（缺席）、`SteamOutTemperature`（缺席）、`SteamoutReq`（缺席）、`SupplyResponsibility`（缺席）、`TotalInsulThick`（缺席） |
| `PIDSignalConnector` | `Description`（缺席）、`Name`（缺席） |
| `PIDSignalPort` | `Description`（缺席）、`IsTypical`（缺席） |

## 两边都非空的格子

接口 **37**。这些名字同时在 map 类表的接口列和参考的非空属性里。

| 类 | 接口 | 五图非空和 |
|---|---|---:|
| `PIDControlSystemFunction` | `INamedInstrument` | 158 |
| `PIDControlSystemFunction` | `IObject` | 167 |
| `PIDControlSystemFunction` | `IPBSItem` | 176 |
| `PIDHeatTransferEquipment` | `IEquipment` | 3 |
| `PIDHeatTransferEquipment` | `INamedEquipment` | 2 |
| `PIDHeatTransferEquipment` | `IObject` | 2 |
| `PIDHeatTransferEquipment` | `IPBSItem` | 2 |
| `PIDInlineInstrument` | `IInstrument` | 141 |
| `PIDInlineInstrument` | `IInsulatedItem` | 19 |
| `PIDInlineInstrument` | `INamedInstrument` | 78 |
| `PIDInlineInstrument` | `IObject` | 111 |
| `PIDInlineInstrument` | `IPBSItem` | 144 |
| `PIDInlineInstrument` | `IPipeCrossSectionItem` | 41 |
| `PIDInlineInstrument` | `IPressureReliefItem` | 4 |
| `PIDMechanicalEquipment` | `IEquipment` | 22 |
| `PIDMechanicalEquipment` | `INamedEquipment` | 4 |
| `PIDMechanicalEquipment` | `IObject` | 8 |
| `PIDMechanicalEquipment` | `IPBSItem` | 8 |
| `PIDNozzle` | `IEquipmentComponent` | 108 |
| `PIDNozzle` | `IObject` | 36 |
| `PIDNozzle` | `IPBSItem` | 72 |
| `PIDNozzle` | `IPipeCrossSectionItem` | 29 |
| `PIDPipingComponent` | `IInsulatedItem` | 76 |
| `PIDPipingComponent` | `IObject` | 416 |
| `PIDPipingComponent` | `IPBSItem` | 528 |
| `PIDPipingComponent` | `IPipeCrossSectionItem` | 164 |
| `PIDPipingComponent` | `IPipingComponent` | 1259 |
| `PIDPipingConnector` | `IObject` | 560 |
| `PIDProcessEquipment` | `IEquipment` | 26 |
| `PIDProcessEquipment` | `INamedEquipment` | 15 |
| `PIDProcessEquipment` | `IObject` | 21 |
| `PIDProcessEquipment` | `IPBSItem` | 26 |
| `PIDProcessEquipment` | `ISpecifiedMatlItem` | 6 |
| `PIDProcessVessel` | `IEquipment` | 30 |
| `PIDProcessVessel` | `INamedEquipment` | 10 |
| `PIDProcessVessel` | `IObject` | 10 |
| `PIDProcessVessel` | `IPBSItem` | 10 |

属性 **90**。map 有「发布类.属性」，五张图非空和大于 0。

| 发布类.属性 | 五图非空和 | 出现于 |
|---|---:|---|
| `PIDBranchPoint.Description` | 58 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDBranchPoint.Name` | 58 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDBranchPoint.ToFromText` | 58 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDControlSystemFunction.ConstructionStatus` | 88 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDControlSystemFunction.InstrTagSequenceNo` | 79 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDControlSystemFunction.MeasuredVariable` | 79 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDControlSystemFunction.Name` | 79 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDDrawing.Name` | 5 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDHeatTransferEquipment.ConstructionStatus` | 1 | 换热器CD |
| `PIDHeatTransferEquipment.EqType0` | 1 | 换热器CD |
| `PIDHeatTransferEquipment.EqTypeDescription` | 1 | 换热器CD |
| `PIDHeatTransferEquipment.EquipPrefix` | 1 | 换热器CD |
| `PIDHeatTransferEquipment.EquipSeqNo` | 1 | 换热器CD |
| `PIDHeatTransferEquipment.IsTypical` | 1 | 换热器CD |
| `PIDHeatTransferEquipment.Name` | 1 | 换热器CD |
| `PIDInlineInstrument.ConstructionStatus` | 72 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDInlineInstrument.InstrTagSequenceNo` | 39 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDInlineInstrument.InsulSpec` | 19 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDInlineInstrument.IsFlowDirectional` | 72 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDInlineInstrument.IsTypical` | 72 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDInlineInstrument.MeasuredVariable` | 39 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDInlineInstrument.Name` | 39 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDInlineInstrument.NominalDiameter` | 41 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDInlineInstrument.ProcFunc` | 49 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDInlineInstrument.ReliefPressure` | 4 | I井场、低温分离器A、分离器A、分离器B |
| `PIDInstrument.ConstructionStatus` | 127 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDInstrument.InstrTagSequenceNo` | 118 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDInstrument.InstrumentLocation` | 116 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDInstrument.IsTypical` | 127 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDInstrument.MeasuredVariable` | 118 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDInstrument.Name` | 118 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDMechanicalEquipment.ConstructionStatus` | 4 | I井场 |
| `PIDMechanicalEquipment.Description` | 2 | I井场 |
| `PIDMechanicalEquipment.EqType0` | 4 | I井场 |
| `PIDMechanicalEquipment.EqTypeDescription` | 4 | I井场 |
| `PIDMechanicalEquipment.EquipPrefix` | 2 | I井场 |
| `PIDMechanicalEquipment.EquipSeqNo` | 2 | I井场 |
| `PIDMechanicalEquipment.IsTypical` | 4 | I井场 |
| `PIDMechanicalEquipment.Name` | 2 | I井场 |
| `PIDNote.NoteText` | 28 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDNozzle.ConstructionStatus` | 36 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDNozzle.IsTypical` | 36 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDNozzle.NominalDiameter` | 29 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDNozzle.PipingMaterialsClass` | 21 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDNozzle.ProcEqpCompTypeDescription` | 36 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDNozzle.ProcessEqCompType1` | 36 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDPipeline.FluidCode` | 4 | 低温分离器A、分离器A、分离器B、换热器CD |
| `PIDPipeline.Name` | 172 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDPipingComponent.CommoditySpecialtyType` | 264 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDPipingComponent.ConstructionStatus` | 264 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDPipingComponent.InsulSpec` | 76 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDPipingComponent.IsFlowDirectional` | 264 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDPipingComponent.IsTypical` | 264 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDPipingComponent.Name` | 152 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDPipingComponent.NominalDiameter` | 164 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDPipingComponent.PipModelCode` | 259 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDPipingComponent.PipingComponentType1` | 264 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDPipingConnector.ConstructionStatus` | 341 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDPipingConnector.FlowDirection` | 103 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDPipingConnector.InsulSpec` | 106 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDPipingConnector.IsTypical` | 341 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDPipingConnector.Name` | 219 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDPipingConnector.NominalDiameter` | 200 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDPipingConnector.PipingConnectorSeqNo` | 219 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDPipingConnector.PipingConnectorType` | 341 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDPipingConnector.PipingMaterialsClass` | 261 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDPipingConnector.RepresentationsAreAllZeroLength` | 341 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDPipingPort.ConnectionFlowDirection` | 298 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDPipingPort.Name` | 718 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDPipingPort.NominalDiameter` | 444 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDProcessEquipment.ConstructionStatus` | 13 | I井场、换热器CD |
| `PIDProcessEquipment.Description` | 3 | I井场 |
| `PIDProcessEquipment.EqType0` | 13 | I井场、换热器CD |
| `PIDProcessEquipment.EqTypeDescription` | 12 | I井场 |
| `PIDProcessEquipment.EquipPrefix` | 10 | I井场、换热器CD |
| `PIDProcessEquipment.EquipSeqNo` | 5 | I井场、换热器CD |
| `PIDProcessEquipment.EquipmentTrimSpec` | 1 | I井场 |
| `PIDProcessEquipment.IsTypical` | 13 | I井场、换热器CD |
| `PIDProcessEquipment.LongMaterialDescription3` | 6 | I井场 |
| `PIDProcessEquipment.Name` | 5 | I井场、换热器CD |
| `PIDProcessVessel.ConstructionStatus` | 5 | I井场、低温分离器A、分离器A、分离器B |
| `PIDProcessVessel.EqType0` | 5 | I井场、低温分离器A、分离器A、分离器B |
| `PIDProcessVessel.EqTypeDescription` | 5 | I井场、低温分离器A、分离器A、分离器B |
| `PIDProcessVessel.EquipPrefix` | 5 | I井场、低温分离器A、分离器A、分离器B |
| `PIDProcessVessel.EquipSeqNo` | 5 | I井场、低温分离器A、分离器A、分离器B |
| `PIDProcessVessel.IsTypical` | 5 | I井场、低温分离器A、分离器A、分离器B |
| `PIDProcessVessel.Name` | 5 | I井场、低温分离器A、分离器A、分离器B |
| `PIDRepresentation.GraphicOID` | 1823 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDSignalConnector.IsTypical` | 83 | I井场、低温分离器A、分离器A、分离器B、换热器CD |
| `PIDSignalPort.Name` | 1725 | I井场、低温分离器A、分离器A、分离器B、换热器CD |

## 导出器今天发的接口，和 map 一致与否

导出器名单只取 `docs/analysis/n2c-exporter-interface-inventory.md` 的「会发出去的类」和 `published_attribute` 全表，不另读 `src`。物项上除 `IObject` 外，接口是「有值才发」；下表比的是底稿写明**允许发出**的接口，不是某一张图实际写出的子集。
`PIDBranchPoint` 在 map 类表有（`OPC`，接口列「—」），底稿写明今天不发。端口、工艺点、分支点、管线不在导出器常量表里，即使 map 和参考都有。

### 接口

允许发出的接口格 **73**。在 map 接口列里的 **23**，不在的 **50**。

在 map 接口列里：

| 类 | 接口 | map 类表 |
|---|---|---|
| `PIDProcessVessel` | `IObject` | 第 53 行 `Vessel` |
| `PIDProcessVessel` | `INamedEquipment` | 第 53 行 `Vessel` |
| `PIDProcessVessel` | `IInsulatedItem` | 第 53 行 `Vessel` |
| `PIDProcessEquipment` | `IObject` | 第 50 行 `EquipmentOther` |
| `PIDProcessEquipment` | `INamedEquipment` | 第 50 行 `EquipmentOther` |
| `PIDProcessEquipment` | `IInsulatedItem` | 第 50 行 `EquipmentOther` |
| `PIDMechanicalEquipment` | `IObject` | 第 52 行 `Mechanical` |
| `PIDMechanicalEquipment` | `INamedEquipment` | 第 52 行 `Mechanical` |
| `PIDMechanicalEquipment` | `IInsulatedItem` | 第 52 行 `Mechanical` |
| `PIDHeatTransferEquipment` | `IObject` | 第 51 行 `Exchanger` |
| `PIDHeatTransferEquipment` | `INamedEquipment` | 第 51 行 `Exchanger` |
| `PIDHeatTransferEquipment` | `IInsulatedItem` | 第 51 行 `Exchanger` |
| `PIDNozzle` | `IObject` | 第 37 行 `Nozzle` |
| `PIDNozzle` | `IInsulatedItem` | 第 37 行 `Nozzle` |
| `PIDPipingComponent` | `IObject` | 第 38 行 `PipingComp` |
| `PIDPipingComponent` | `IInsulatedItem` | 第 38 行 `PipingComp` |
| `PIDPipingConnector` | `IObject` | 第 39 行 `PipeRun` |
| `PIDControlSystemFunction` | `IObject` | 第 31 行 `InstrumentCtrlSysFunc` |
| `PIDControlSystemFunction` | `INamedInstrument` | 第 31 行 `InstrumentCtrlSysFunc` |
| `PIDInlineInstrument` | `IObject` | 第 33 行 `InstrumentInline` |
| `PIDInlineInstrument` | `INamedInstrument` | 第 33 行 `InstrumentInline` |
| `PIDInlineInstrument` | `IPipingSpecifiedItem` | 第 33 行 `InstrumentInline` |
| `PIDInlineInstrument` | `IInsulatedItem` | 第 33 行 `InstrumentInline` |

不在 map 接口列里：

| 类 | 接口 | 对照 |
|---|---|---|
| `PIDDrawing` | `IObject` | 类表第 32 行接口列不含它 |
| `PIDRepresentation` | `IObject` | 类表第 43 行接口列不含它 |
| `PIDRepresentation` | `IDrawingRepresentation` | 类表第 43 行接口列不含它 |
| `Rel` | `IObject` | map 没有 Rel 这个发布类 |
| `Rel` | `IRel` | map 没有 Rel 这个发布类 |
| `PIDProcessVessel` | `IPipingSpecifiedItem` | 类表第 53 行接口列不含它 |
| `PIDProcessVessel` | `IOffpageConnector` | 类表第 53 行接口列不含它 |
| `PIDProcessVessel` | `INote` | 类表第 53 行接口列不含它 |
| `PIDProcessEquipment` | `IPipingSpecifiedItem` | 类表第 50 行接口列不含它 |
| `PIDProcessEquipment` | `IOffpageConnector` | 类表第 50 行接口列不含它 |
| `PIDProcessEquipment` | `INote` | 类表第 50 行接口列不含它 |
| `PIDMechanicalEquipment` | `IPipingSpecifiedItem` | 类表第 52 行接口列不含它 |
| `PIDMechanicalEquipment` | `IOffpageConnector` | 类表第 52 行接口列不含它 |
| `PIDMechanicalEquipment` | `INote` | 类表第 52 行接口列不含它 |
| `PIDHeatTransferEquipment` | `IPipingSpecifiedItem` | 类表第 51 行接口列不含它 |
| `PIDHeatTransferEquipment` | `IOffpageConnector` | 类表第 51 行接口列不含它 |
| `PIDHeatTransferEquipment` | `INote` | 类表第 51 行接口列不含它 |
| `PIDNozzle` | `IPipingSpecifiedItem` | 类表第 37 行接口列不含它 |
| `PIDNozzle` | `IOffpageConnector` | 类表第 37 行接口列不含它 |
| `PIDNozzle` | `INote` | 类表第 37 行接口列不含它 |
| `PIDPipingComponent` | `IPipingSpecifiedItem` | 类表第 38 行接口列不含它 |
| `PIDPipingComponent` | `IOffpageConnector` | 类表第 38 行接口列不含它 |
| `PIDPipingComponent` | `INote` | 类表第 38 行接口列不含它 |
| `PIDPipingConnector` | `INamedPipingConnector` | 类表第 39 行接口列不含它 |
| `PIDPipingConnector` | `IPipingSpecifiedItem` | 类表第 39 行接口列不含它 |
| `PIDPipingConnector` | `IInsulatedItem` | 类表第 39 行接口列不含它 |
| `PIDPipingConnector` | `IOffpageConnector` | 类表第 39 行接口列不含它 |
| `PIDPipingConnector` | `INote` | 类表第 39 行接口列不含它 |
| `PIDSignalConnector` | `IObject` | 类表第 44 行接口列不含它 |
| `PIDSignalConnector` | `IPipingSpecifiedItem` | 类表第 44 行接口列不含它 |
| `PIDSignalConnector` | `IInsulatedItem` | 类表第 44 行接口列不含它 |
| `PIDSignalConnector` | `IOffpageConnector` | 类表第 44 行接口列不含它 |
| `PIDSignalConnector` | `INote` | 类表第 44 行接口列不含它 |
| `PIDNote` | `IObject` | 类表第 36 行接口列不含它 |
| `PIDNote` | `IPipingSpecifiedItem` | 类表第 36 行接口列不含它 |
| `PIDNote` | `IInsulatedItem` | 类表第 36 行接口列不含它 |
| `PIDNote` | `IOffpageConnector` | 类表第 36 行接口列不含它 |
| `PIDNote` | `INote` | 类表第 36 行接口列不含它 |
| `PIDInstrument` | `IObject` | 类表第 34 行接口列不含它 |
| `PIDInstrument` | `INamedInstrument` | 类表第 34 行接口列不含它 |
| `PIDInstrument` | `IPipingSpecifiedItem` | 类表第 34 行接口列不含它 |
| `PIDInstrument` | `IInsulatedItem` | 类表第 34 行接口列不含它 |
| `PIDInstrument` | `IOffpageConnector` | 类表第 34 行接口列不含它 |
| `PIDInstrument` | `INote` | 类表第 34 行接口列不含它 |
| `PIDControlSystemFunction` | `IPipingSpecifiedItem` | 类表第 31 行接口列不含它 |
| `PIDControlSystemFunction` | `IInsulatedItem` | 类表第 31 行接口列不含它 |
| `PIDControlSystemFunction` | `IOffpageConnector` | 类表第 31 行接口列不含它 |
| `PIDControlSystemFunction` | `INote` | 类表第 31 行接口列不含它 |
| `PIDInlineInstrument` | `IOffpageConnector` | 类表第 33 行接口列不含它 |
| `PIDInlineInstrument` | `INote` | 类表第 33 行接口列不含它 |

### 属性

底稿允许写到 XML 上的属性名（`UID` 每个对象都写；`Name` 只在图纸、表示以外的拼名规则或标签给出时写；无类门的五条任何已定类物项都能带）。对照 map 的「发布类.属性」，不对照接口列。

| 类.属性 | map 属性表 | 参考五图 |
|---|---|---|
| `PIDDrawing.UID` | 没有 | 非空和 5 |
| `PIDDrawing.Name` | 有 | 非空和 5 |
| `PIDRepresentation.UID` | 没有 | 非空和 1823 |
| `PIDRepresentation.GraphicOID` | 有 | 非空和 1823 |
| `Rel.UID` | 没有「Rel.属性」 | Rel 上非空和 8185 |
| `Rel.UID1` | 没有「Rel.属性」 | Rel 上非空和 8185 |
| `Rel.UID2` | 没有「Rel.属性」 | Rel 上非空和 8185 |
| `Rel.DefUID` | 没有「Rel.属性」 | Rel 上非空和 8185 |
| `PIDProcessVessel.UID` | 没有 | 非空和 5 |
| `PIDProcessVessel.Name` | 有 | 非空和 5 |
| `PIDProcessVessel.EquipPrefix` | 有 | 非空和 5 |
| `PIDProcessVessel.EquipSeqNo` | 有 | 非空和 5 |
| `PIDProcessVessel.PipingMaterialsClass` | 没有 | 缺席 |
| `PIDProcessVessel.InsulSpec` | 有 | 缺席 |
| `PIDProcessVessel.Description` | 有 | 缺席 |
| `PIDProcessVessel.ToFromText` | 没有 | 缺席 |
| `PIDProcessVessel.NoteText` | 没有 | 缺席 |
| `PIDProcessEquipment.UID` | 没有 | 非空和 13 |
| `PIDProcessEquipment.Name` | 有 | 非空和 5 |
| `PIDProcessEquipment.EquipPrefix` | 有 | 非空和 10 |
| `PIDProcessEquipment.EquipSeqNo` | 有 | 非空和 5 |
| `PIDProcessEquipment.PipingMaterialsClass` | 没有 | 缺席 |
| `PIDProcessEquipment.InsulSpec` | 有 | 缺席 |
| `PIDProcessEquipment.Description` | 有 | 非空和 3 |
| `PIDProcessEquipment.ToFromText` | 没有 | 缺席 |
| `PIDProcessEquipment.NoteText` | 没有 | 缺席 |
| `PIDMechanicalEquipment.UID` | 没有 | 非空和 4 |
| `PIDMechanicalEquipment.Name` | 有 | 非空和 2 |
| `PIDMechanicalEquipment.EquipPrefix` | 有 | 非空和 2 |
| `PIDMechanicalEquipment.EquipSeqNo` | 有 | 非空和 2 |
| `PIDMechanicalEquipment.PipingMaterialsClass` | 没有 | 缺席 |
| `PIDMechanicalEquipment.InsulSpec` | 有 | 缺席 |
| `PIDMechanicalEquipment.Description` | 有 | 非空和 2 |
| `PIDMechanicalEquipment.ToFromText` | 没有 | 缺席 |
| `PIDMechanicalEquipment.NoteText` | 没有 | 缺席 |
| `PIDHeatTransferEquipment.UID` | 没有 | 非空和 1 |
| `PIDHeatTransferEquipment.Name` | 有 | 非空和 1 |
| `PIDHeatTransferEquipment.EquipPrefix` | 有 | 非空和 1 |
| `PIDHeatTransferEquipment.EquipSeqNo` | 有 | 非空和 1 |
| `PIDHeatTransferEquipment.PipingMaterialsClass` | 没有 | 缺席 |
| `PIDHeatTransferEquipment.InsulSpec` | 有 | 缺席 |
| `PIDHeatTransferEquipment.Description` | 有 | 缺席 |
| `PIDHeatTransferEquipment.ToFromText` | 没有 | 缺席 |
| `PIDHeatTransferEquipment.NoteText` | 没有 | 缺席 |
| `PIDInstrument.UID` | 没有 | 非空和 127 |
| `PIDInstrument.Name` | 有 | 非空和 118 |
| `PIDInstrument.MeasuredVariable` | 有 | 非空和 118 |
| `PIDInstrument.InstrTagSequenceNo` | 有 | 非空和 118 |
| `PIDInstrument.PipingMaterialsClass` | 有 | 缺席 |
| `PIDInstrument.InsulSpec` | 有 | 缺席 |
| `PIDInstrument.Description` | 有 | 缺席 |
| `PIDInstrument.ToFromText` | 没有 | 缺席 |
| `PIDInstrument.NoteText` | 没有 | 缺席 |
| `PIDControlSystemFunction.UID` | 没有 | 非空和 88 |
| `PIDControlSystemFunction.Name` | 有 | 非空和 79 |
| `PIDControlSystemFunction.MeasuredVariable` | 有 | 非空和 79 |
| `PIDControlSystemFunction.InstrTagSequenceNo` | 有 | 非空和 79 |
| `PIDControlSystemFunction.PipingMaterialsClass` | 没有 | 缺席 |
| `PIDControlSystemFunction.InsulSpec` | 没有 | 缺席 |
| `PIDControlSystemFunction.Description` | 有 | 缺席 |
| `PIDControlSystemFunction.ToFromText` | 没有 | 缺席 |
| `PIDControlSystemFunction.NoteText` | 没有 | 缺席 |
| `PIDInlineInstrument.UID` | 没有 | 非空和 72 |
| `PIDInlineInstrument.Name` | 有 | 非空和 39 |
| `PIDInlineInstrument.MeasuredVariable` | 有 | 非空和 39 |
| `PIDInlineInstrument.InstrTagSequenceNo` | 有 | 非空和 39 |
| `PIDInlineInstrument.PipingMaterialsClass` | 有 | 缺席 |
| `PIDInlineInstrument.InsulSpec` | 有 | 非空和 19 |
| `PIDInlineInstrument.Description` | 有 | 缺席 |
| `PIDInlineInstrument.ToFromText` | 没有 | 缺席 |
| `PIDInlineInstrument.NoteText` | 没有 | 缺席 |
| `PIDPipingConnector.UID` | 没有 | 非空和 341 |
| `PIDPipingConnector.PipingConnectorSeqNo` | 有 | 非空和 219 |
| `PIDPipingConnector.PipingMaterialsClass` | 有 | 非空和 261 |
| `PIDPipingConnector.InsulSpec` | 有 | 非空和 106 |
| `PIDPipingConnector.Description` | 有 | 缺席 |
| `PIDPipingConnector.ToFromText` | 没有 | 缺席 |
| `PIDPipingConnector.NoteText` | 没有 | 缺席 |
| `PIDNozzle.UID` | 没有 | 非空和 36 |
| `PIDNozzle.PipingMaterialsClass` | 有 | 非空和 21 |
| `PIDNozzle.InsulSpec` | 有 | 缺席 |
| `PIDNozzle.Description` | 有 | 缺席 |
| `PIDNozzle.ToFromText` | 没有 | 缺席 |
| `PIDNozzle.NoteText` | 没有 | 缺席 |
| `PIDPipingComponent.UID` | 没有 | 非空和 264 |
| `PIDPipingComponent.PipingMaterialsClass` | 没有 | 缺席 |
| `PIDPipingComponent.InsulSpec` | 有 | 非空和 76 |
| `PIDPipingComponent.Description` | 有 | 缺席 |
| `PIDPipingComponent.ToFromText` | 没有 | 缺席 |
| `PIDPipingComponent.NoteText` | 没有 | 缺席 |
| `PIDSignalConnector.UID` | 没有 | 非空和 83 |
| `PIDSignalConnector.PipingMaterialsClass` | 没有 | 缺席 |
| `PIDSignalConnector.InsulSpec` | 没有 | 缺席 |
| `PIDSignalConnector.Description` | 有 | 缺席 |
| `PIDSignalConnector.ToFromText` | 没有 | 缺席 |
| `PIDSignalConnector.NoteText` | 没有 | 缺席 |
| `PIDNote.UID` | 没有 | 非空和 56 |
| `PIDNote.PipingMaterialsClass` | 没有 | 缺席 |
| `PIDNote.InsulSpec` | 没有 | 缺席 |
| `PIDNote.Description` | 有 | 缺席 |
| `PIDNote.ToFromText` | 没有 | 缺席 |
| `PIDNote.NoteText` | 有 | 非空和 28 |

上表 102 格里，map 属性表有的 50 格，其中参考也非空的 32 格。`Rel` 的四格 map 没有发布类，不计入这 50。

和 map 不一致、而且导出器真会靠它发出接口的，集中在这几条：

- `INamedPipingConnector` / `PipingConnectorSeqNo`：导出器对 `PIDPipingConnector` 允许发。map 接口列（`PipeRun`，类表第 39 行）没有 `INamedPipingConnector`；属性表**有** `PIDPipingConnector.PipingConnectorSeqNo`，参考也非空。接口列和属性表结论不同，改注释时不能只看接口列。
- `IPipingSpecifiedItem`、`IOffpageConnector`、`INote`：导出器对每个已定类物项都允许发。map 接口列里，`IPipingSpecifiedItem` 只挂在 `PIDInlineInstrument`；`IOffpageConnector`、`INote` 不在任何发布类的接口列。属性 `PipingMaterialsClass` 在若干类上 map 有、参考有的类并不都非空；`ToFromText` 只在 `PIDBranchPoint` 上参考非空，而导出器今天不发这个类；`NoteText` 在 `PIDNote` 上 map 有、参考非空。
- `IInsulatedItem`：map 接口列挂在在线仪表、管件和四个设备类上，不挂在 `PIDPipingConnector`。导出器对管段也允许发，参考里 `PIDPipingConnector` 的 `IInsulatedItem` / `InsulSpec` 非空。
- `PIDInstrument` 的 `INamedInstrument`：导出器允许发，参考非空，但 map 的 `Instrument` 行接口列是「—」。属性 `MeasuredVariable`、`InstrTagSequenceNo`、`Name` 在 map 属性表里有，参考也非空。

所以不能把导出器常量表的出处直接改成 map 的 `ToolOID`：05.02 的接口列盖不住 10.00.31 参考里非空、且导出器已经在发的那些接口。

## 关系、枚举、单位

map 底稿「关系」只有 1 条 `SPMapRelDef`：UID `SP_OwnsComponentOcc`，图侧 `PipeRun` → `PipingComp`，发布关系名 `OwnsComponentOcc`。
两份实测的 `Rel` 节只记了接口 `IObject`、`IRel` 和属性 `UID` / `UID1` / `UID2` / `DefUID`（五图 `DefUID` 非空和 8 185），**没有把 DefUID 的取值拆开**。因此这四份底稿对不上「参考里的 `DrawingItems` 是否等于 map 的 `OwnsComponentOcc`」这一格。导出器底稿写明今天只发 `DwgRepresentationComposition` 和 `DrawingItems`，这两个名字不是 `OwnsComponentOcc`。

枚举和单位沿用 map 底稿「枚举表和单位表」：2 971 个 `SPMapEnumDef` 的条件体是空字符串，212 个 `SPMapUoMDef` 没有换算。全文 `@` 出现 0 次，字面量 `2 in` 不出现。公称直径只作为处理器名 `EFAdapter.NominalDiameterConv` 出现。`@` 引用和 `2 in` 的格式不在这份 map 里。实测底稿没有单列属性值是不是 `@` 引用，本页不另数。

## 核对

抽三格即可确认本页不是另算的一套：

1. map 类表 `Instrument` 行（`n2c-map-schema-table.md` 第 34 行）接口列是「—」，发布类 `PIDInstrument`。I井场实测 `PIDInstrument` 节有 `INamedInstrument` / `MeasuredVariable`，非空次数 58。所以该接口在「参考有而 map 没有」，该属性在「两边都非空」（五图和 118，含另外四张）。
2. 换热器 CD 实测 `PIDHeatTransferEquipment` 节有 `IShellAndTubeHeatExchanger`，非空次数 0；没有 `IAirCooledExchanger`。map 类表 `Exchanger` 行（第 51 行）接口列含这两个名字。两格都在「map 有而参考全空」，一格是有行，一格是缺席。
3. map 属性全表 `#23` 是 `PIDDrawing.Name`。五张图 `PIDDrawing` 的 `IObject` / `Name` 各非空 1 次，和为 5，在「两边都非空」。

重算：接口用类表「接口」列对两份实测的接口列取差；属性用「发布类.属性」对实测的「类 + 属性名」取差，非空次数五图相加。`Rel` 不加进类。

