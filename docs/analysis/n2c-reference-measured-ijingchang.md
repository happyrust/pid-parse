# N2c 参考实测：I井场

> 任务 t-910。从 I井场这一张图的参考 `_Data.xml` 抽出类、接口、属性。只新建本文件。

## 计数

`<Container>` 的每个直接子元素是一个对象，元素名是类。对象的子元素名是接口，接口上的 XML 属性是属性。`Name` 只计 `IObject/@Name` 非空。属性「出现次数」含空字符串，「非空次数」去掉空字符串。`Rel` 是关系，单独列出，不并进类对象总数。

对照的是同名 `_Coverage.json`。这份文件没有 `per_class` 键，按类的计数在 `classes`（`objects` / `with_name` / `attribute_values`）。参考 XML 取 `D:\work\cad\PID自研软件验证\` 根目录、文件时间 2026-06-22 的那一份。`publish\` 里的同名 `_Data.xml` 文件时间 2026-10-11，体积更小（类对象 1459，`Rel` 1695），是导出器产物，不作为本页的实测来源。`_Coverage.json` 只在 `publish\`，文件时间 2026-10-11。

与 `docs/analysis/2026-10-10-publish-from-the-pid-alone.md` 附录「== I井场 注采阀组工艺及自控流程图」的分类对象数逐类相同，类对象合计 3 093，`Rel` 4 107。

这一张找到了，没有用 `publish\` 那份顶上。

## I井场

- 图名：I井场 注采阀组工艺及自控流程图
- 参考 `_Data.xml`：`D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图_Data.xml`
- `_Coverage.json`：`D:\work\cad\PID自研软件验证\publish\I井场 注采阀组工艺及自控流程图_Coverage.json`
- `SoftwareVersion`：`10.00.31.0023`。`DocUID`：`5633AF4121EE4F4D9FE78E90722CC050`。类对象 3093（不含 `Rel`）。`Rel` 4107。
- `classes` 共 11 类，这些类名参考里都有。参考里另有 9 个类名不在 `classes` 中。

### 与 `classes` 对照

共有类的对象数与 `classes.objects` 一致。对不上的是 `with_name`（参考里非空 `IObject/@Name` 的个数）和 `attribute_values`（覆盖报告计导出器写出的属性值条数，小于参考里的非空属性次数）。

| 类 | 参考对象 | `classes.objects` | 参考非空 Name | `classes.with_name` | 参考非空属性 | `classes.attribute_values` | 差异 |
|---|---:|---:|---:|---:|---:|---:|---|
| `PIDControlSystemFunction` | 50 | 50 | 47 | 47 | 291 | 141 | 对象数一致；非空属性 291 ≠ 141 |
| `PIDInlineInstrument` | 40 | 40 | 20 | 12 | 369 | 36 | 对象数一致；Name 20 ≠ 12；非空属性 369 ≠ 36 |
| `PIDInstrument` | 60 | 60 | 58 | 58 | 472 | 174 | 对象数一致；非空属性 472 ≠ 174 |
| `PIDMechanicalEquipment` | 4 | 4 | 2 | 2 | 46 | 6 | 对象数一致；非空属性 46 ≠ 6 |
| `PIDNote` | 32 | 32 | 0 | 0 | 50 | 18 | 对象数一致；非空属性 50 ≠ 18 |
| `PIDNozzle` | 7 | 7 | 0 | 0 | 60 | 0 | 对象数一致；非空属性 60 ≠ 0 |
| `PIDPipingComponent` | 134 | 134 | 73 | 0 | 1494 | 0 | 对象数一致；Name 73 ≠ 0；非空属性 1494 ≠ 0 |
| `PIDPipingConnector` | 169 | 169 | 106 | 0 | 1561 | 107 | 对象数一致；Name 106 ≠ 0；非空属性 1561 ≠ 107 |
| `PIDProcessEquipment` | 12 | 12 | 4 | 4 | 99 | 14 | 对象数一致；非空属性 99 ≠ 14 |
| `PIDProcessVessel` | 2 | 2 | 2 | 2 | 26 | 6 | 对象数一致；非空属性 26 ≠ 6 |
| `PIDSignalConnector` | 46 | 46 | 0 | 0 | 92 | 0 | 对象数一致；非空属性 92 ≠ 0 |

### 不在 `classes` 里的类

| 类 | 参考对象 | 参考非空 Name | 参考非空属性 | 说明 |
|---|---:|---:|---:|---|
| `PIDDrawing` | 1 | 1 | 4 | `classes` 无此项；参考里固定 1 个图纸对象 |
| `PIDProcessPoint` | 169 | 0 | 169 | `classes` 无此项 |
| `PIDPipingPort` | 362 | 362 | 1099 | `classes` 无此项 |
| `PIDRepresentation` | 902 | 0 | 1804 | 与 `objects.representations` 同为 902，但没有 `classes` 项 |
| `PIDSignalPort` | 915 | 915 | 1830 | `classes` 无此项 |
| `PIDPipingBranchPoint` | 97 | 0 | 97 | `classes` 无此项 |
| `PIDSignalBranchPoint` | 10 | 0 | 10 | `classes` 无此项 |
| `PIDPipeline` | 79 | 79 | 214 | `classes` 无此项 |
| `PIDBranchPoint` | 2 | 2 | 8 | 参考 2 个，`left_out` 也是 2 条 OPC，UID 只重合 `17288DB92E9F4A76B97EA8F56E2CFDEC`。参考另有 `6ECB5E61A73C4AA39B6BAD56EB93AECA`，不在 `left_out`。`left_out` 另有 `8B4053A297404826A46FAB595D09F398`，参考 `_Data.xml` 无此 UID |

`Rel` 4107 条也不在 `classes` 里。覆盖报告的 `relationships` 合计 1695（`DwgRepresentationComposition` 902，与参考该 `DefUID` 相同；`DrawingItems` 793，参考该 `DefUID` 902），是导出器写出的子集。

### 类 × 接口 × 属性

#### `PIDDrawing`（1）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 1 | `UID` | 1 | 1 |
| `IObject` | 1 | `Name` | 1 | 1 |
| `IObject` | 1 | `Description` | 1 | 0 |
| `IDocument` | 1 | `DocCategory` | 1 | 1 |
| `IDocument` | 1 | `DocTitle` | 1 | 0 |
| `IDocument` | 1 | `DocType` | 1 | 1 |
| `IDocument` | 1 | `DocSubtype` | 1 | 0 |
| `IDocVersionComposition` | 1 | — | 0 | 0 |
| `IDwgRepresentationComposition` | 1 | — | 0 | 0 |
| `IPIDDrawing` | 1 | — | 0 | 0 |
| `ISchematicDwg` | 1 | — | 0 | 0 |
| `IPBSItem` | 1 | — | 0 | 0 |

#### `PIDControlSystemFunction`（50）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 50 | `UID` | 50 | 50 |
| `IObject` | 50 | `Name` | 47 | 47 |
| `IPBSItem` | 50 | `ConstructionStatus` | 50 | 50 |
| `IPBSItem` | 50 | `ConstructionStatus2` | 50 | 50 |
| `IControlSystemFunction` | 50 | — | 0 | 0 |
| `IDrawingItem` | 50 | — | 0 | 0 |
| `ISignalPortComposition` | 50 | — | 0 | 0 |
| `IInstrument` | 50 | — | 0 | 0 |
| `IPlannedMatl` | 50 | — | 0 | 0 |
| `ILoopMember` | 50 | — | 0 | 0 |
| `IDocumentItem` | 50 | — | 0 | 0 |
| `INoteCollection` | 50 | — | 0 | 0 |
| `IExpandableThing` | 50 | — | 0 | 0 |
| `INamedInstrument` | 50 | `InstrFuncModifier` | 50 | 0 |
| `INamedInstrument` | 50 | `InstrLoopSuffix` | 50 | 0 |
| `INamedInstrument` | 50 | `InstrTagPrefix` | 50 | 0 |
| `INamedInstrument` | 50 | `InstrTagSequenceNo` | 50 | 47 |
| `INamedInstrument` | 50 | `InstrTagSuffix` | 50 | 0 |
| `INamedInstrument` | 50 | `MeasuredVariable` | 50 | 47 |

#### `PIDInlineInstrument`（40）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 40 | `UID` | 40 | 40 |
| `IObject` | 40 | `Name` | 20 | 20 |
| `IInlineInstrument` | 40 | — | 0 | 0 |
| `IInstrument` | 40 | `ProcFunc` | 26 | 26 |
| `IInstrument` | 40 | `InstrumentType3` | 23 | 23 |
| `IInstrument` | 40 | `InstrumentType` | 23 | 23 |
| `IInstrumentOcc` | 40 | — | 0 | 0 |
| `IPBSItem` | 40 | `ConstructionStatus` | 40 | 40 |
| `IPBSItem` | 40 | `ConstructionStatus2` | 40 | 40 |
| `IPipingPortComposition` | 40 | — | 0 | 0 |
| `IPlannedMatl` | 40 | — | 0 | 0 |
| `IDrawingItem` | 40 | — | 0 | 0 |
| `IInlineInstrumentOcc` | 40 | — | 0 | 0 |
| `IExpandableThing` | 40 | — | 0 | 0 |
| `IDuctPortComposition` | 40 | — | 0 | 0 |
| `IInlineComponentOcc` | 40 | — | 0 | 0 |
| `IHeatTracedItem` | 40 | `HTraceRqmt` | 40 | 0 |
| `ILoopMember` | 40 | — | 0 | 0 |
| `IPartOcc` | 40 | — | 0 | 0 |
| `IPressureReliefItem` | 40 | `ReliefPressure` | 1 | 1 |
| `IProcessPointCollection` | 40 | — | 0 | 0 |
| `ISignalPortComposition` | 40 | — | 0 | 0 |
| `IDocumentItem` | 40 | — | 0 | 0 |
| `IElecPowerConsumer` | 40 | — | 0 | 0 |
| `IPart` | 40 | — | 0 | 0 |
| `INoteCollection` | 40 | — | 0 | 0 |
| `INamedInstrument` | 40 | `InstrFuncModifier` | 40 | 0 |
| `INamedInstrument` | 40 | `InstrLoopSuffix` | 40 | 0 |
| `INamedInstrument` | 40 | `InstrTagPrefix` | 40 | 0 |
| `INamedInstrument` | 40 | `InstrTagSequenceNo` | 40 | 20 |
| `INamedInstrument` | 40 | `InstrTagSuffix` | 40 | 0 |
| `INamedInstrument` | 40 | `MeasuredVariable` | 40 | 20 |
| `IPipeCrossSectionItem` | 40 | `NominalDiameter` | 22 | 22 |
| `IInsulatedItem` | 14 | `InsulSpec` | 14 | 14 |
| `IInlineComponent` | 40 | `IsFlowDirectional` | 40 | 40 |
| `IPIDTypical` | 40 | `IsTypical` | 40 | 40 |

#### `PIDInstrument`（60）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 60 | `UID` | 60 | 60 |
| `IObject` | 60 | `Name` | 58 | 58 |
| `IInstrument` | 60 | — | 0 | 0 |
| `IInstrumentOcc` | 60 | `InstrumentLocation` | 58 | 58 |
| `IPBSItem` | 60 | `ConstructionStatus` | 60 | 60 |
| `IPBSItem` | 60 | `ConstructionStatus2` | 60 | 60 |
| `IPlannedMatl` | 60 | — | 0 | 0 |
| `IDrawingItem` | 60 | — | 0 | 0 |
| `IExpandableThing` | 60 | — | 0 | 0 |
| `IInlineComponentOcc` | 60 | — | 0 | 0 |
| `IPipingPortComposition` | 60 | — | 0 | 0 |
| `IHeatTracedItem` | 60 | `HTraceRqmt` | 60 | 0 |
| `ILoopMember` | 60 | — | 0 | 0 |
| `IPartOcc` | 60 | — | 0 | 0 |
| `IPressureReliefItem` | 60 | — | 0 | 0 |
| `IProcessPointCollection` | 60 | — | 0 | 0 |
| `ISignalPortComposition` | 60 | — | 0 | 0 |
| `IDocumentItem` | 60 | — | 0 | 0 |
| `IElecPowerConsumer` | 60 | — | 0 | 0 |
| `IPart` | 60 | — | 0 | 0 |
| `INoteCollection` | 60 | — | 0 | 0 |
| `INamedInstrument` | 60 | `InstrFuncModifier` | 60 | 0 |
| `INamedInstrument` | 60 | `InstrLoopSuffix` | 60 | 0 |
| `INamedInstrument` | 60 | `InstrTagPrefix` | 60 | 0 |
| `INamedInstrument` | 60 | `InstrTagSequenceNo` | 60 | 58 |
| `INamedInstrument` | 60 | `InstrTagSuffix` | 60 | 0 |
| `INamedInstrument` | 60 | `MeasuredVariable` | 60 | 58 |
| `IPIDTypical` | 60 | `IsTypical` | 60 | 60 |

#### `PIDNote`（32）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 32 | `UID` | 32 | 32 |
| `IDrawingItem` | 32 | — | 0 | 0 |
| `IPBSNote` | 32 | — | 0 | 0 |
| `INote` | 32 | `NoteText` | 18 | 18 |
| `IDocumentItem` | 32 | — | 0 | 0 |

#### `PIDNozzle`（7）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 7 | `UID` | 7 | 7 |
| `IPBSItem` | 7 | `ConstructionStatus` | 7 | 7 |
| `IPBSItem` | 7 | `ConstructionStatus2` | 7 | 7 |
| `IPipingPortComposition` | 7 | — | 0 | 0 |
| `IPlannedMatl` | 7 | — | 0 | 0 |
| `IDrawingItem` | 7 | — | 0 | 0 |
| `INozzleOcc` | 7 | — | 0 | 0 |
| `INozzle` | 7 | — | 0 | 0 |
| `IEquipmentComponent` | 7 | `ProcessEqCompType1` | 7 | 7 |
| `IEquipmentComponent` | 7 | `ProcessEqCompType2` | 7 | 7 |
| `IEquipmentComponent` | 7 | `ProcEqpCompTypeDescription` | 7 | 7 |
| `IEquipmentComponentOcc` | 7 | — | 0 | 0 |
| `IFabricatedItem` | 7 | — | 0 | 0 |
| `IHeatTracedItem` | 7 | `HTraceRqmt` | 7 | 0 |
| `IPBSItemCollection` | 7 | — | 0 | 0 |
| `IProcessPointCollection` | 7 | — | 0 | 0 |
| `ISignalPortComposition` | 7 | — | 0 | 0 |
| `IPartOcc` | 7 | — | 0 | 0 |
| `IDocumentItem` | 7 | — | 0 | 0 |
| `IElecPowerConsumer` | 7 | — | 0 | 0 |
| `IPart` | 7 | — | 0 | 0 |
| `INoteCollection` | 7 | — | 0 | 0 |
| `IProcessDataCaseComposition` | 7 | — | 0 | 0 |
| `IPipeCrossSectionItem` | 7 | `NominalDiameter` | 6 | 6 |
| `IPipingSpecifiedItem` | 5 | `PipingMaterialsClass` | 5 | 5 |
| `IPIDTypical` | 7 | `IsTypical` | 7 | 7 |

#### `PIDPipingComponent`（134）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 134 | `UID` | 134 | 134 |
| `IObject` | 134 | `Name` | 73 | 73 |
| `IPBSItem` | 134 | `ConstructionStatus` | 134 | 134 |
| `IPBSItem` | 134 | `ConstructionStatus2` | 134 | 134 |
| `IPipingPortComposition` | 134 | — | 0 | 0 |
| `IPlannedMatl` | 134 | — | 0 | 0 |
| `IDrawingItem` | 134 | — | 0 | 0 |
| `IPipingComponent` | 134 | `PipingComponentType1` | 134 | 134 |
| `IPipingComponent` | 134 | `PipingComponentType3` | 102 | 102 |
| `IPipingComponent` | 134 | `PipingComponentType2` | 132 | 132 |
| `IPipingComponent` | 134 | `PipModelCode` | 134 | 133 |
| `IPipingComponent` | 134 | `CommoditySpecialtyType` | 134 | 134 |
| `IPipingComponentOcc` | 134 | — | 0 | 0 |
| `IInlineComponentOcc` | 134 | — | 0 | 0 |
| `IFabricatedItem` | 134 | — | 0 | 0 |
| `IHeatTracedItem` | 134 | `HTraceRqmt` | 134 | 0 |
| `IPartOcc` | 134 | — | 0 | 0 |
| `IPressureReliefItem` | 134 | — | 0 | 0 |
| `IDocumentItem` | 134 | — | 0 | 0 |
| `IElecPowerConsumer` | 134 | — | 0 | 0 |
| `IPart` | 134 | — | 0 | 0 |
| `INoteCollection` | 134 | — | 0 | 0 |
| `IPipeCrossSectionItem` | 134 | `NominalDiameter` | 84 | 84 |
| `IInlineComponent` | 134 | `IsFlowDirectional` | 134 | 134 |
| `IPIDTypical` | 134 | `IsTypical` | 134 | 134 |
| `IInsulatedItem` | 32 | `InsulSpec` | 32 | 32 |

#### `PIDPipingConnector`（169）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 169 | `UID` | 169 | 169 |
| `IObject` | 169 | `Name` | 106 | 106 |
| `IPBSItem` | 169 | `ConstructionStatus` | 169 | 169 |
| `IPBSItem` | 169 | `ConstructionStatus2` | 169 | 169 |
| `IPlannedFacility` | 169 | — | 0 | 0 |
| `IConnector` | 169 | `FlowDirection` | 169 | 59 |
| `IConnector` | 169 | `RepresentationsAreAllZeroLength` | 169 | 169 |
| `IDrawingItem` | 169 | — | 0 | 0 |
| `IPBSItemCollection` | 169 | — | 0 | 0 |
| `IPipingConnector` | 169 | `PipingConnectorType` | 169 | 169 |
| `IFabricatedItem` | 169 | — | 0 | 0 |
| `IHeatTracedItem` | 169 | `HTraceRqmt` | 169 | 0 |
| `IProcessPointCollection` | 169 | — | 0 | 0 |
| `IDocumentItem` | 169 | — | 0 | 0 |
| `IElecPowerConsumer` | 169 | — | 0 | 0 |
| `INoteCollection` | 169 | — | 0 | 0 |
| `IProcessDataCaseComposition` | 169 | — | 0 | 0 |
| `IExpandableThing` | 169 | — | 0 | 0 |
| `IPipeCrossSectionItem` | 169 | `NominalDiameter` | 90 | 90 |
| `IJacketedItem` | 169 | — | 0 | 0 |
| `IPIDTypical` | 169 | `IsTypical` | 169 | 169 |
| `INamedPipingConnector` | 106 | `PipingConnectorPrefix` | 106 | 0 |
| `INamedPipingConnector` | 106 | `PipingConnectorSeqNo` | 106 | 106 |
| `INamedPipingConnector` | 106 | `PipingConnectorSuff` | 106 | 0 |
| `IPipingSpecifiedItem` | 128 | `PipingMaterialsClass` | 128 | 128 |
| `IInsulatedItem` | 58 | `InsulSpec` | 58 | 58 |

#### `PIDProcessPoint`（169）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 169 | `UID` | 169 | 169 |
| `IProcessPoint` | 169 | — | 0 | 0 |
| `IFacilityPoint` | 169 | — | 0 | 0 |
| `IProcessPointCaseComposition` | 169 | — | 0 | 0 |
| `IBulkMaxProcessPoint` | 169 | `PhaseTemperatureMax` | 169 | 0 |
| `IBulkMaxProcessPoint` | 169 | `PressureMax` | 169 | 0 |
| `IBulkMinProcessPoint` | 169 | `PhaseTemperatureMin` | 169 | 0 |
| `IBulkMinProcessPoint` | 169 | `PressureMin` | 169 | 0 |
| `IBulkNormProcessPoint` | 169 | `MolecularWeightNorm` | 169 | 0 |
| `IBulkNormProcessPoint` | 169 | `CpCvRatioNorm` | 169 | 0 |
| `IBulkBaseProcessPoint` | 169 | `PhaseTemperatureBase` | 169 | 0 |
| `IBulkBaseProcessPoint` | 169 | `PressureBase` | 169 | 0 |

#### `PIDPipingPort`（362）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 362 | `UID` | 362 | 362 |
| `IObject` | 362 | `Name` | 362 | 362 |
| `IConnection` | 362 | `ConnectionFlowDirection` | 152 | 152 |
| `IPipingPort` | 362 | — | 0 | 0 |
| `IPipingConnection` | 362 | — | 0 | 0 |
| `IPort` | 362 | — | 0 | 0 |
| `IPipeCrossSectionItem` | 362 | `NominalDiameter` | 223 | 223 |

#### `PIDRepresentation`（902）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 902 | `UID` | 902 | 902 |
| `IDrawingRepresentation` | 902 | `GraphicOID` | 902 | 902 |

#### `PIDSignalConnector`（46）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 46 | `UID` | 46 | 46 |
| `IPlannedFacility` | 46 | — | 0 | 0 |
| `IConnector` | 46 | `FlowDirection` | 46 | 0 |
| `IDrawingItem` | 46 | — | 0 | 0 |
| `ISignalConnector` | 46 | — | 0 | 0 |
| `IDocumentItem` | 46 | — | 0 | 0 |
| `IExpandableThing` | 46 | — | 0 | 0 |
| `IPIDTypical` | 46 | `IsTypical` | 46 | 46 |

#### `PIDSignalPort`（915）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 915 | `UID` | 915 | 915 |
| `IObject` | 915 | `Name` | 915 | 915 |
| `IConnection` | 915 | — | 0 | 0 |
| `ISignalConnection` | 915 | — | 0 | 0 |
| `ISignalPort` | 915 | — | 0 | 0 |
| `IPipingConnection` | 915 | — | 0 | 0 |
| `IFacilityPoint` | 915 | — | 0 | 0 |
| `IPIDTypical` | 915 | — | 0 | 0 |

#### `PIDPipingBranchPoint`（97）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 97 | `UID` | 97 | 97 |
| `IConnection` | 97 | — | 0 | 0 |
| `IPipingConnection` | 97 | — | 0 | 0 |
| `IDrawingItem` | 97 | — | 0 | 0 |
| `IPipingBranchPoint` | 97 | — | 0 | 0 |
| `IDocumentItem` | 97 | — | 0 | 0 |

#### `PIDSignalBranchPoint`（10）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 10 | `UID` | 10 | 10 |
| `IConnection` | 10 | — | 0 | 0 |
| `IDrawingItem` | 10 | — | 0 | 0 |
| `ISignalConnection` | 10 | — | 0 | 0 |
| `ISignalBranchPoint` | 10 | — | 0 | 0 |
| `IFacilityPoint` | 10 | — | 0 | 0 |
| `IDocumentItem` | 10 | — | 0 | 0 |

#### `PIDPipeline`（79）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 79 | `UID` | 79 | 79 |
| `IObject` | 79 | `Name` | 79 | 79 |
| `IPBSItem` | 79 | — | 0 | 0 |
| `IPlannedFacility` | 79 | — | 0 | 0 |
| `IPBSItemCollection` | 79 | — | 0 | 0 |
| `IPipeline` | 79 | — | 0 | 0 |
| `IPipingConnectorComposition` | 79 | — | 0 | 0 |
| `IFluidSystem` | 79 | `FluidSystem` | 56 | 56 |
| `INoteCollection` | 79 | — | 0 | 0 |
| `IExpandableThing` | 79 | — | 0 | 0 |
| `IPIDTypical` | 79 | — | 0 | 0 |

#### `PIDProcessEquipment`（12）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 12 | `UID` | 12 | 12 |
| `IObject` | 12 | `Name` | 4 | 4 |
| `IObject` | 12 | `Description` | 3 | 3 |
| `IPBSItem` | 12 | `ConstructionStatus` | 12 | 12 |
| `IPBSItem` | 12 | `ConstructionStatus2` | 12 | 12 |
| `IPlannedMatl` | 12 | — | 0 | 0 |
| `IDrawingItem` | 12 | — | 0 | 0 |
| `IEquipmentOcc` | 12 | — | 0 | 0 |
| `IPBSItemCollection` | 12 | — | 0 | 0 |
| `IEquipment` | 12 | `EqType0` | 12 | 12 |
| `IEquipment` | 12 | `EqTypeDescription` | 12 | 12 |
| `IEquipment` | 12 | `EquipmentTrimSpec` | 1 | 1 |
| `INamedEquipment` | 9 | `EquipPrefix` | 9 | 9 |
| `INamedEquipment` | 9 | `EquipSeqNo` | 9 | 4 |
| `INamedEquipment` | 9 | `EquipSuff` | 9 | 0 |
| `ISpecifiedMatlItem` | 6 | `LongMaterialDescription3` | 6 | 6 |
| `IPIDTypical` | 12 | `IsTypical` | 12 | 12 |

#### `PIDMechanicalEquipment`（4）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 4 | `UID` | 4 | 4 |
| `IObject` | 4 | `Name` | 2 | 2 |
| `IObject` | 4 | `Description` | 2 | 2 |
| `IPIDMechanicalEquipmentOcc` | 4 | — | 0 | 0 |
| `IProcessEquipmentOcc` | 4 | — | 0 | 0 |
| `IPIDMechanicalEquipment` | 4 | — | 0 | 0 |
| `IProcessEquipment` | 4 | — | 0 | 0 |
| `IEquipmentOcc` | 4 | — | 0 | 0 |
| `IEquipment` | 4 | `EqType0` | 4 | 4 |
| `IEquipment` | 4 | `EqType5` | 2 | 2 |
| `IEquipment` | 4 | `EqType4` | 2 | 2 |
| `IEquipment` | 4 | `EqType3` | 2 | 2 |
| `IEquipment` | 4 | `EqType2` | 4 | 4 |
| `IEquipment` | 4 | `EqType1` | 4 | 4 |
| `IEquipment` | 4 | `EqTypeDescription` | 4 | 4 |
| `IPlannedMatl` | 4 | — | 0 | 0 |
| `IDrawingItem` | 4 | — | 0 | 0 |
| `IPBSItem` | 4 | `ConstructionStatus` | 4 | 4 |
| `IPBSItem` | 4 | `ConstructionStatus2` | 4 | 4 |
| `IPositiveDisplacementPumpOcc` | 2 | — | 0 | 0 |
| `IPositiveDisplacementPump` | 2 | — | 0 | 0 |
| `IPumpOcc` | 2 | — | 0 | 0 |
| `IPump` | 4 | — | 0 | 0 |
| `IFluidTransferMachineOcc` | 2 | — | 0 | 0 |
| `IFluidTransferMachine` | 2 | — | 0 | 0 |
| `IMaterialTransferEquipment` | 2 | — | 0 | 0 |
| `IMaterialTransferEquipmentOcc` | 2 | — | 0 | 0 |
| `IEquipmentDriver` | 4 | — | 0 | 0 |
| `INamedEquipment` | 2 | `EquipPrefix` | 2 | 2 |
| `INamedEquipment` | 2 | `EquipSeqNo` | 2 | 2 |
| `INamedEquipment` | 2 | `EquipSuff` | 2 | 0 |
| `IPIDTypical` | 4 | `IsTypical` | 4 | 4 |
| `IElectricMotorOcc` | 2 | — | 0 | 0 |
| `IElectricMotor` | 2 | — | 0 | 0 |
| `IProcessEquipmentDriverOcc` | 2 | — | 0 | 0 |

#### `PIDProcessVessel`（2）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 2 | `UID` | 2 | 2 |
| `IObject` | 2 | `Name` | 2 | 2 |
| `IPIDProcessVesselOcc` | 2 | — | 0 | 0 |
| `IProcessVesselOcc` | 2 | — | 0 | 0 |
| `IEquipment` | 2 | `EqType0` | 2 | 2 |
| `IEquipment` | 2 | `EqType4` | 2 | 2 |
| `IEquipment` | 2 | `EqType3` | 2 | 2 |
| `IEquipment` | 2 | `EqType2` | 2 | 2 |
| `IEquipment` | 2 | `EqType1` | 2 | 2 |
| `IEquipment` | 2 | `EqTypeDescription` | 2 | 2 |
| `IEquipmentOcc` | 2 | — | 0 | 0 |
| `IPBSItem` | 2 | `ConstructionStatus` | 2 | 2 |
| `IPBSItem` | 2 | `ConstructionStatus2` | 2 | 2 |
| `IPBSItemCollection` | 2 | — | 0 | 0 |
| `IPlannedMatl` | 2 | — | 0 | 0 |
| `IProcessEquipment` | 2 | — | 0 | 0 |
| `IProcessEquipmentOcc` | 2 | — | 0 | 0 |
| `IProcessVessel` | 2 | — | 0 | 0 |
| `IDrawingItem` | 2 | — | 0 | 0 |
| `IPIDProcessVessel` | 2 | — | 0 | 0 |
| `ILowPressureTank` | 2 | — | 0 | 0 |
| `INamedEquipment` | 2 | `EquipPrefix` | 2 | 2 |
| `INamedEquipment` | 2 | `EquipSeqNo` | 2 | 2 |
| `INamedEquipment` | 2 | `EquipSuff` | 2 | 0 |
| `IPIDTypical` | 2 | `IsTypical` | 2 | 2 |

#### `PIDBranchPoint`（2）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 2 | `UID` | 2 | 2 |
| `IObject` | 2 | `Name` | 2 | 2 |
| `IObject` | 2 | `Description` | 2 | 2 |
| `IPIDBranchPoint` | 2 | — | 0 | 0 |
| `IOffpageConnector` | 2 | `ToFromText` | 2 | 2 |
| `IDuctConnection` | 2 | — | 0 | 0 |
| `IConnection` | 2 | — | 0 | 0 |
| `IDrawingItem` | 2 | — | 0 | 0 |
| `IPipingConnection` | 2 | — | 0 | 0 |
| `ISignalConnection` | 2 | — | 0 | 0 |
| `IDocumentItem` | 2 | — | 0 | 0 |

#### `Rel`（4107，关系，不是 `classes` 的类）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 4107 | `UID` | 4107 | 4107 |
| `IRel` | 4107 | `UID1` | 4107 | 4107 |
| `IRel` | 4107 | `UID2` | 4107 | 4107 |
| `IRel` | 4107 | `DefUID` | 4107 | 4107 |
