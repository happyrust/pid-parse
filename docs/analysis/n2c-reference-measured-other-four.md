# N2c 参考实测：其余四张

> 任务 t-911。从分离器A、分离器B、低温分离器A、换热器CD 四张图的参考 `_Data.xml` 抽出类、接口、属性。四张分开写。只新建本文件。

## 计数

`<Container>` 的每个直接子元素是一个对象，元素名是类。对象的子元素名是接口，接口上的 XML 属性是属性。`Name` 只计 `IObject/@Name` 非空。属性「出现次数」含空字符串，「非空次数」去掉空字符串。`Rel` 是关系，单独列出，不并进类对象总数。

对照的是同名 `_Coverage.json`。四份文件都没有 `per_class` 键，按类的计数在 `classes`（`objects` / `with_name` / `attribute_values`）。参考 XML 取 `D:\work\cad\PID自研软件验证\` 根目录、写于 2026-07-06 的那一份。`publish\` 里的同名 `_Data.xml` 写于 2026-10-11，体积更小，是导出器产物，不作为本页的实测来源。`_Coverage.json` 只在 `publish\`。

四张都找到了，没有用另一张顶上。

## 分离器A

- 图名：集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图
- 参考 `_Data.xml`：`D:\work\cad\PID自研软件验证\集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图_Data.xml`
- `_Coverage.json`：`D:\work\cad\PID自研软件验证\publish\集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图_Coverage.json`
- `SoftwareVersion`：`10.00.31.0023`。类对象 570（不含 `Rel`）。`Rel` 767。
- `classes` 共 9 类，这些类名参考里都有。参考里另有 9 个类名不在 `classes` 中。

### 与 `classes` 对照

共有类的对象数与 `classes.objects` 一致。对不上的是 `with_name`（参考里非空 `IObject/@Name` 的个数）和 `attribute_values`（覆盖报告计导出器写出的属性值条数，小于参考里的非空属性次数）。

| 类 | 参考对象 | `classes.objects` | 参考非空 Name | `classes.with_name` | 参考非空属性 | `classes.attribute_values` | 差异 |
|---|---:|---:|---:|---:|---:|---:|---|
| `PIDControlSystemFunction` | 5 | 5 | 4 | 4 | 27 | 12 | 对象数一致；非空属性 27 ≠ 12 |
| `PIDInlineInstrument` | 2 | 2 | 2 | 2 | 26 | 6 | 对象数一致；非空属性 26 ≠ 6 |
| `PIDInstrument` | 10 | 10 | 9 | 8 | 76 | 24 | 对象数一致；Name 9 ≠ 8；非空属性 76 ≠ 24 |
| `PIDNote` | 7 | 7 | 0 | 0 | 10 | 3 | 对象数一致；非空属性 10 ≠ 3 |
| `PIDNozzle` | 5 | 5 | 0 | 0 | 43 | 0 | 对象数一致；非空属性 43 ≠ 0 |
| `PIDPipingComponent` | 27 | 27 | 17 | 0 | 314 | 0 | 对象数一致；Name 17 ≠ 0；非空属性 314 ≠ 0 |
| `PIDPipingConnector` | 39 | 39 | 30 | 0 | 384 | 52 | 对象数一致；Name 30 ≠ 0；非空属性 384 ≠ 52 |
| `PIDProcessVessel` | 1 | 1 | 1 | 1 | 13 | 3 | 对象数一致；非空属性 13 ≠ 3 |
| `PIDSignalConnector` | 6 | 6 | 0 | 0 | 12 | 0 | 对象数一致；非空属性 12 ≠ 0 |

### 不在 `classes` 里的类

| 类 | 参考对象 | 参考非空 Name | 参考非空属性 | 说明 |
|---|---:|---:|---:|---|
| `PIDDrawing` | 1 | 1 | 4 | `classes` 无此项；参考里固定 1 个图纸对象 |
| `PIDProcessPoint` | 39 | 0 | 39 | `classes` 无此项 |
| `PIDPipingPort` | 63 | 63 | 205 | `classes` 无此项 |
| `PIDRepresentation` | 188 | 0 | 376 | 与 `objects.representations` 同为 188，但没有 `classes` 项 |
| `PIDSignalPort` | 107 | 107 | 214 | `classes` 无此项 |
| `PIDPipingBranchPoint` | 22 | 0 | 22 | `classes` 无此项 |
| `PIDSignalBranchPoint` | 1 | 0 | 1 | `classes` 无此项 |
| `PIDPipeline` | 29 | 29 | 87 | `classes` 无此项 |
| `PIDBranchPoint` | 18 | 18 | 72 | 与 `left_out` 条数同为 18（`model_item_type`：OPC），没有 `classes` 项 |

`Rel` 767 条也不在 `classes` 里。覆盖报告的 `relationships` 合计 335（`DwgRepresentationComposition` 188、`DrawingItems` 147），是导出器写出的子集。

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

#### `PIDControlSystemFunction`（5）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 5 | `UID` | 5 | 5 |
| `IObject` | 5 | `Name` | 4 | 4 |
| `IPBSItem` | 5 | `ConstructionStatus` | 5 | 5 |
| `IPBSItem` | 5 | `ConstructionStatus2` | 5 | 5 |
| `IControlSystemFunction` | 5 | — | 0 | 0 |
| `IDrawingItem` | 5 | — | 0 | 0 |
| `ISignalPortComposition` | 5 | — | 0 | 0 |
| `IInstrument` | 5 | — | 0 | 0 |
| `IPlannedMatl` | 5 | — | 0 | 0 |
| `ILoopMember` | 5 | — | 0 | 0 |
| `IDocumentItem` | 5 | — | 0 | 0 |
| `INoteCollection` | 5 | — | 0 | 0 |
| `IExpandableThing` | 5 | — | 0 | 0 |
| `INamedInstrument` | 5 | `InstrFuncModifier` | 5 | 0 |
| `INamedInstrument` | 5 | `InstrLoopSuffix` | 5 | 0 |
| `INamedInstrument` | 5 | `InstrTagPrefix` | 5 | 0 |
| `INamedInstrument` | 5 | `InstrTagSequenceNo` | 5 | 4 |
| `INamedInstrument` | 5 | `InstrTagSuffix` | 5 | 0 |
| `INamedInstrument` | 5 | `MeasuredVariable` | 5 | 4 |

#### `PIDInlineInstrument`（2）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 2 | `UID` | 2 | 2 |
| `IObject` | 2 | `Name` | 2 | 2 |
| `IInlineInstrument` | 2 | — | 0 | 0 |
| `IInstrument` | 2 | `ProcFunc` | 2 | 2 |
| `IInstrument` | 2 | `InstrumentType3` | 2 | 2 |
| `IInstrument` | 2 | `InstrumentType` | 2 | 2 |
| `IInstrumentOcc` | 2 | — | 0 | 0 |
| `IPBSItem` | 2 | `ConstructionStatus` | 2 | 2 |
| `IPBSItem` | 2 | `ConstructionStatus2` | 2 | 2 |
| `IPipingPortComposition` | 2 | — | 0 | 0 |
| `IPlannedMatl` | 2 | — | 0 | 0 |
| `IDrawingItem` | 2 | — | 0 | 0 |
| `IInlineInstrumentOcc` | 2 | — | 0 | 0 |
| `IExpandableThing` | 2 | — | 0 | 0 |
| `IDuctPortComposition` | 2 | — | 0 | 0 |
| `IInlineComponentOcc` | 2 | — | 0 | 0 |
| `IHeatTracedItem` | 2 | `HTraceRqmt` | 2 | 0 |
| `ILoopMember` | 2 | — | 0 | 0 |
| `IPartOcc` | 2 | — | 0 | 0 |
| `IPressureReliefItem` | 2 | `ReliefPressure` | 1 | 1 |
| `IProcessPointCollection` | 2 | — | 0 | 0 |
| `ISignalPortComposition` | 2 | — | 0 | 0 |
| `IDocumentItem` | 2 | — | 0 | 0 |
| `IElecPowerConsumer` | 2 | — | 0 | 0 |
| `IPart` | 2 | — | 0 | 0 |
| `INoteCollection` | 2 | — | 0 | 0 |
| `INamedInstrument` | 2 | `InstrFuncModifier` | 2 | 0 |
| `INamedInstrument` | 2 | `InstrLoopSuffix` | 2 | 0 |
| `INamedInstrument` | 2 | `InstrTagPrefix` | 2 | 0 |
| `INamedInstrument` | 2 | `InstrTagSequenceNo` | 2 | 2 |
| `INamedInstrument` | 2 | `InstrTagSuffix` | 2 | 0 |
| `INamedInstrument` | 2 | `MeasuredVariable` | 2 | 2 |
| `IPipeCrossSectionItem` | 2 | `NominalDiameter` | 2 | 2 |
| `IInsulatedItem` | 1 | `InsulSpec` | 1 | 1 |
| `IInlineComponent` | 2 | `IsFlowDirectional` | 2 | 2 |
| `IPIDTypical` | 2 | `IsTypical` | 2 | 2 |

#### `PIDInstrument`（10）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 10 | `UID` | 10 | 10 |
| `IObject` | 10 | `Name` | 9 | 9 |
| `IInstrument` | 10 | — | 0 | 0 |
| `IInstrumentOcc` | 10 | `InstrumentLocation` | 9 | 9 |
| `IPBSItem` | 10 | `ConstructionStatus` | 10 | 10 |
| `IPBSItem` | 10 | `ConstructionStatus2` | 10 | 10 |
| `IPlannedMatl` | 10 | — | 0 | 0 |
| `IDrawingItem` | 10 | — | 0 | 0 |
| `IExpandableThing` | 10 | — | 0 | 0 |
| `IInlineComponentOcc` | 10 | — | 0 | 0 |
| `IPipingPortComposition` | 10 | — | 0 | 0 |
| `IHeatTracedItem` | 10 | `HTraceRqmt` | 10 | 0 |
| `ILoopMember` | 10 | — | 0 | 0 |
| `IPartOcc` | 10 | — | 0 | 0 |
| `IPressureReliefItem` | 10 | — | 0 | 0 |
| `IProcessPointCollection` | 10 | — | 0 | 0 |
| `ISignalPortComposition` | 10 | — | 0 | 0 |
| `IDocumentItem` | 10 | — | 0 | 0 |
| `IElecPowerConsumer` | 10 | — | 0 | 0 |
| `IPart` | 10 | — | 0 | 0 |
| `INoteCollection` | 10 | — | 0 | 0 |
| `INamedInstrument` | 10 | `InstrFuncModifier` | 10 | 0 |
| `INamedInstrument` | 10 | `InstrLoopSuffix` | 10 | 0 |
| `INamedInstrument` | 10 | `InstrTagPrefix` | 10 | 0 |
| `INamedInstrument` | 10 | `InstrTagSequenceNo` | 10 | 9 |
| `INamedInstrument` | 10 | `InstrTagSuffix` | 10 | 0 |
| `INamedInstrument` | 10 | `MeasuredVariable` | 10 | 9 |
| `IPIDTypical` | 10 | `IsTypical` | 10 | 10 |

#### `PIDNote`（7）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 7 | `UID` | 7 | 7 |
| `IDrawingItem` | 7 | — | 0 | 0 |
| `IPBSNote` | 7 | — | 0 | 0 |
| `INote` | 7 | `NoteText` | 3 | 3 |
| `IDocumentItem` | 7 | — | 0 | 0 |

#### `PIDNozzle`（5）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 5 | `UID` | 5 | 5 |
| `IPBSItem` | 5 | `ConstructionStatus` | 5 | 5 |
| `IPBSItem` | 5 | `ConstructionStatus2` | 5 | 5 |
| `IPipingPortComposition` | 5 | — | 0 | 0 |
| `IPlannedMatl` | 5 | — | 0 | 0 |
| `IDrawingItem` | 5 | — | 0 | 0 |
| `INozzleOcc` | 5 | — | 0 | 0 |
| `INozzle` | 5 | — | 0 | 0 |
| `IEquipmentComponent` | 5 | `ProcessEqCompType1` | 5 | 5 |
| `IEquipmentComponent` | 5 | `ProcessEqCompType2` | 5 | 5 |
| `IEquipmentComponent` | 5 | `ProcEqpCompTypeDescription` | 5 | 5 |
| `IEquipmentComponentOcc` | 5 | — | 0 | 0 |
| `IFabricatedItem` | 5 | — | 0 | 0 |
| `IHeatTracedItem` | 5 | `HTraceRqmt` | 5 | 0 |
| `IPBSItemCollection` | 5 | — | 0 | 0 |
| `IProcessPointCollection` | 5 | — | 0 | 0 |
| `ISignalPortComposition` | 5 | — | 0 | 0 |
| `IPartOcc` | 5 | — | 0 | 0 |
| `IDocumentItem` | 5 | — | 0 | 0 |
| `IElecPowerConsumer` | 5 | — | 0 | 0 |
| `IPart` | 5 | — | 0 | 0 |
| `INoteCollection` | 5 | — | 0 | 0 |
| `IProcessDataCaseComposition` | 5 | — | 0 | 0 |
| `IPipeCrossSectionItem` | 5 | `NominalDiameter` | 4 | 4 |
| `IPipingSpecifiedItem` | 4 | `PipingMaterialsClass` | 4 | 4 |
| `IPIDTypical` | 5 | `IsTypical` | 5 | 5 |

#### `PIDPipingComponent`（27）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 27 | `UID` | 27 | 27 |
| `IObject` | 27 | `Name` | 17 | 17 |
| `IPBSItem` | 27 | `ConstructionStatus` | 27 | 27 |
| `IPBSItem` | 27 | `ConstructionStatus2` | 27 | 27 |
| `IPipingPortComposition` | 27 | — | 0 | 0 |
| `IPlannedMatl` | 27 | — | 0 | 0 |
| `IDrawingItem` | 27 | — | 0 | 0 |
| `IPipingComponent` | 27 | `PipingComponentType1` | 27 | 27 |
| `IPipingComponent` | 27 | `PipingComponentType2` | 27 | 27 |
| `IPipingComponent` | 27 | `PipModelCode` | 27 | 27 |
| `IPipingComponent` | 27 | `CommoditySpecialtyType` | 27 | 27 |
| `IPipingComponent` | 27 | `PipingComponentType3` | 24 | 24 |
| `IPipingComponentOcc` | 27 | — | 0 | 0 |
| `IInlineComponentOcc` | 27 | — | 0 | 0 |
| `IFabricatedItem` | 27 | — | 0 | 0 |
| `IHeatTracedItem` | 27 | `HTraceRqmt` | 27 | 0 |
| `IPartOcc` | 27 | — | 0 | 0 |
| `IPressureReliefItem` | 27 | — | 0 | 0 |
| `IDocumentItem` | 27 | — | 0 | 0 |
| `IElecPowerConsumer` | 27 | — | 0 | 0 |
| `IPart` | 27 | — | 0 | 0 |
| `INoteCollection` | 27 | — | 0 | 0 |
| `IPipeCrossSectionItem` | 27 | `NominalDiameter` | 18 | 18 |
| `IInlineComponent` | 27 | `IsFlowDirectional` | 27 | 27 |
| `IPIDTypical` | 27 | `IsTypical` | 27 | 27 |
| `IInsulatedItem` | 12 | `InsulSpec` | 12 | 12 |

#### `PIDPipingConnector`（39）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 39 | `UID` | 39 | 39 |
| `IObject` | 39 | `Name` | 30 | 30 |
| `IPBSItem` | 39 | `ConstructionStatus` | 39 | 39 |
| `IPBSItem` | 39 | `ConstructionStatus2` | 39 | 39 |
| `IPlannedFacility` | 39 | — | 0 | 0 |
| `IConnector` | 39 | `FlowDirection` | 39 | 12 |
| `IConnector` | 39 | `RepresentationsAreAllZeroLength` | 39 | 39 |
| `IDrawingItem` | 39 | — | 0 | 0 |
| `IPBSItemCollection` | 39 | — | 0 | 0 |
| `IPipingConnector` | 39 | `PipingConnectorType` | 39 | 39 |
| `IFabricatedItem` | 39 | — | 0 | 0 |
| `IHeatTracedItem` | 39 | `HTraceRqmt` | 39 | 0 |
| `IProcessPointCollection` | 39 | — | 0 | 0 |
| `IDocumentItem` | 39 | — | 0 | 0 |
| `IElecPowerConsumer` | 39 | — | 0 | 0 |
| `INoteCollection` | 39 | — | 0 | 0 |
| `IProcessDataCaseComposition` | 39 | — | 0 | 0 |
| `IExpandableThing` | 39 | — | 0 | 0 |
| `INamedPipingConnector` | 30 | `PipingConnectorPrefix` | 30 | 0 |
| `INamedPipingConnector` | 30 | `PipingConnectorSeqNo` | 30 | 30 |
| `INamedPipingConnector` | 30 | `PipingConnectorSuff` | 30 | 0 |
| `IPipeCrossSectionItem` | 39 | `NominalDiameter` | 30 | 30 |
| `IPipingSpecifiedItem` | 31 | `PipingMaterialsClass` | 31 | 31 |
| `IJacketedItem` | 39 | — | 0 | 0 |
| `IPIDTypical` | 39 | `IsTypical` | 39 | 39 |
| `IInsulatedItem` | 17 | `InsulSpec` | 17 | 17 |

#### `PIDProcessPoint`（39）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 39 | `UID` | 39 | 39 |
| `IProcessPoint` | 39 | — | 0 | 0 |
| `IFacilityPoint` | 39 | — | 0 | 0 |
| `IProcessPointCaseComposition` | 39 | — | 0 | 0 |
| `IBulkMaxProcessPoint` | 39 | `PhaseTemperatureMax` | 39 | 0 |
| `IBulkMaxProcessPoint` | 39 | `PressureMax` | 39 | 0 |
| `IBulkMinProcessPoint` | 39 | `PhaseTemperatureMin` | 39 | 0 |
| `IBulkMinProcessPoint` | 39 | `PressureMin` | 39 | 0 |
| `IBulkNormProcessPoint` | 39 | `MolecularWeightNorm` | 39 | 0 |
| `IBulkNormProcessPoint` | 39 | `CpCvRatioNorm` | 39 | 0 |
| `IBulkBaseProcessPoint` | 39 | `PhaseTemperatureBase` | 39 | 0 |
| `IBulkBaseProcessPoint` | 39 | `PressureBase` | 39 | 0 |

#### `PIDPipingPort`（63）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 63 | `UID` | 63 | 63 |
| `IObject` | 63 | `Name` | 63 | 63 |
| `IConnection` | 63 | `ConnectionFlowDirection` | 35 | 35 |
| `IPipingPort` | 63 | — | 0 | 0 |
| `IPipingConnection` | 63 | — | 0 | 0 |
| `IPort` | 63 | — | 0 | 0 |
| `IPipeCrossSectionItem` | 63 | `NominalDiameter` | 44 | 44 |

#### `PIDRepresentation`（188）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 188 | `UID` | 188 | 188 |
| `IDrawingRepresentation` | 188 | `GraphicOID` | 188 | 188 |

#### `PIDSignalConnector`（6）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 6 | `UID` | 6 | 6 |
| `IPlannedFacility` | 6 | — | 0 | 0 |
| `IConnector` | 6 | `FlowDirection` | 6 | 0 |
| `IDrawingItem` | 6 | — | 0 | 0 |
| `ISignalConnector` | 6 | — | 0 | 0 |
| `IDocumentItem` | 6 | — | 0 | 0 |
| `IExpandableThing` | 6 | — | 0 | 0 |
| `IPIDTypical` | 6 | `IsTypical` | 6 | 6 |

#### `PIDSignalPort`（107）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 107 | `UID` | 107 | 107 |
| `IObject` | 107 | `Name` | 107 | 107 |
| `IConnection` | 107 | — | 0 | 0 |
| `ISignalConnection` | 107 | — | 0 | 0 |
| `ISignalPort` | 107 | — | 0 | 0 |
| `IPipingConnection` | 107 | — | 0 | 0 |
| `IFacilityPoint` | 107 | — | 0 | 0 |
| `IPIDTypical` | 107 | — | 0 | 0 |

#### `PIDPipingBranchPoint`（22）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 22 | `UID` | 22 | 22 |
| `IConnection` | 22 | — | 0 | 0 |
| `IPipingConnection` | 22 | — | 0 | 0 |
| `IDrawingItem` | 22 | — | 0 | 0 |
| `IPipingBranchPoint` | 22 | — | 0 | 0 |
| `IDocumentItem` | 22 | — | 0 | 0 |

#### `PIDSignalBranchPoint`（1）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 1 | `UID` | 1 | 1 |
| `IConnection` | 1 | — | 0 | 0 |
| `IDrawingItem` | 1 | — | 0 | 0 |
| `ISignalConnection` | 1 | — | 0 | 0 |
| `ISignalBranchPoint` | 1 | — | 0 | 0 |
| `IFacilityPoint` | 1 | — | 0 | 0 |
| `IDocumentItem` | 1 | — | 0 | 0 |

#### `PIDPipeline`（29）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 29 | `UID` | 29 | 29 |
| `IObject` | 29 | `Name` | 29 | 29 |
| `IPBSItem` | 29 | — | 0 | 0 |
| `IPlannedFacility` | 29 | — | 0 | 0 |
| `IPBSItemCollection` | 29 | — | 0 | 0 |
| `IPipeline` | 29 | — | 0 | 0 |
| `IPipingConnectorComposition` | 29 | — | 0 | 0 |
| `IFluidSystem` | 29 | `FluidCode` | 1 | 1 |
| `IFluidSystem` | 29 | `FluidSystem` | 28 | 28 |
| `INoteCollection` | 29 | — | 0 | 0 |
| `IExpandableThing` | 29 | — | 0 | 0 |
| `IPIDTypical` | 29 | — | 0 | 0 |

#### `PIDProcessVessel`（1）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 1 | `UID` | 1 | 1 |
| `IObject` | 1 | `Name` | 1 | 1 |
| `IPIDProcessVesselOcc` | 1 | — | 0 | 0 |
| `IProcessVesselOcc` | 1 | — | 0 | 0 |
| `IEquipment` | 1 | `EqType0` | 1 | 1 |
| `IEquipment` | 1 | `EqType4` | 1 | 1 |
| `IEquipment` | 1 | `EqType3` | 1 | 1 |
| `IEquipment` | 1 | `EqType2` | 1 | 1 |
| `IEquipment` | 1 | `EqType1` | 1 | 1 |
| `IEquipment` | 1 | `EqTypeDescription` | 1 | 1 |
| `IEquipmentOcc` | 1 | — | 0 | 0 |
| `IPBSItem` | 1 | `ConstructionStatus` | 1 | 1 |
| `IPBSItem` | 1 | `ConstructionStatus2` | 1 | 1 |
| `IPBSItemCollection` | 1 | — | 0 | 0 |
| `IPlannedMatl` | 1 | — | 0 | 0 |
| `IProcessEquipment` | 1 | — | 0 | 0 |
| `IProcessEquipmentOcc` | 1 | — | 0 | 0 |
| `IProcessVessel` | 1 | — | 0 | 0 |
| `IDrawingItem` | 1 | — | 0 | 0 |
| `IPIDProcessVessel` | 1 | — | 0 | 0 |
| `ILowPressureTank` | 1 | — | 0 | 0 |
| `INamedEquipment` | 1 | `EquipPrefix` | 1 | 1 |
| `INamedEquipment` | 1 | `EquipSeqNo` | 1 | 1 |
| `INamedEquipment` | 1 | `EquipSuff` | 1 | 0 |
| `IPIDTypical` | 1 | `IsTypical` | 1 | 1 |

#### `PIDBranchPoint`（18）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 18 | `UID` | 18 | 18 |
| `IObject` | 18 | `Name` | 18 | 18 |
| `IObject` | 18 | `Description` | 18 | 18 |
| `IPIDBranchPoint` | 18 | — | 0 | 0 |
| `IOffpageConnector` | 18 | `ToFromText` | 18 | 18 |
| `IDuctConnection` | 18 | — | 0 | 0 |
| `IConnection` | 18 | — | 0 | 0 |
| `IDrawingItem` | 18 | — | 0 | 0 |
| `IPipingConnection` | 18 | — | 0 | 0 |
| `ISignalConnection` | 18 | — | 0 | 0 |
| `IDocumentItem` | 18 | — | 0 | 0 |

#### `Rel`（767，关系，不是 `classes` 的类）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 767 | `UID` | 767 | 767 |
| `IRel` | 767 | `UID1` | 767 | 767 |
| `IRel` | 767 | `UID2` | 767 | 767 |
| `IRel` | 767 | `DefUID` | 767 | 767 |

## 分离器B

- 图名：集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图
- 参考 `_Data.xml`：`D:\work\cad\PID自研软件验证\集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图_Data.xml`
- `_Coverage.json`：`D:\work\cad\PID自研软件验证\publish\集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图_Coverage.json`
- `SoftwareVersion`：`10.00.31.0023`。类对象 470（不含 `Rel`）。`Rel` 627。
- `classes` 共 9 类，这些类名参考里都有。参考里另有 9 个类名不在 `classes` 中。

### 与 `classes` 对照

共有类的对象数与 `classes.objects` 一致。对不上的是 `with_name`（参考里非空 `IObject/@Name` 的个数）和 `attribute_values`（覆盖报告计导出器写出的属性值条数，小于参考里的非空属性次数）。

| 类 | 参考对象 | `classes.objects` | 参考非空 Name | `classes.with_name` | 参考非空属性 | `classes.attribute_values` | 差异 |
|---|---:|---:|---:|---:|---:|---:|---|
| `PIDControlSystemFunction` | 5 | 5 | 4 | 4 | 27 | 12 | 对象数一致；非空属性 27 ≠ 12 |
| `PIDInlineInstrument` | 2 | 2 | 2 | 2 | 26 | 6 | 对象数一致；非空属性 26 ≠ 6 |
| `PIDInstrument` | 10 | 10 | 9 | 8 | 76 | 24 | 对象数一致；Name 9 ≠ 8；非空属性 76 ≠ 24 |
| `PIDNote` | 7 | 7 | 0 | 0 | 10 | 3 | 对象数一致；非空属性 10 ≠ 3 |
| `PIDNozzle` | 5 | 5 | 0 | 0 | 43 | 0 | 对象数一致；非空属性 43 ≠ 0 |
| `PIDPipingComponent` | 22 | 22 | 14 | 0 | 256 | 0 | 对象数一致；Name 14 ≠ 0；非空属性 256 ≠ 0 |
| `PIDPipingConnector` | 29 | 29 | 15 | 0 | 264 | 30 | 对象数一致；Name 15 ≠ 0；非空属性 264 ≠ 30 |
| `PIDProcessVessel` | 1 | 1 | 1 | 1 | 13 | 3 | 对象数一致；非空属性 13 ≠ 3 |
| `PIDSignalConnector` | 6 | 6 | 0 | 0 | 12 | 0 | 对象数一致；非空属性 12 ≠ 0 |

### 不在 `classes` 里的类

| 类 | 参考对象 | 参考非空 Name | 参考非空属性 | 说明 |
|---|---:|---:|---:|---|
| `PIDDrawing` | 1 | 1 | 4 | `classes` 无此项；参考里固定 1 个图纸对象 |
| `PIDProcessPoint` | 29 | 0 | 29 | `classes` 无此项 |
| `PIDPipingPort` | 53 | 53 | 175 | `classes` 无此项 |
| `PIDRepresentation` | 151 | 0 | 302 | 与 `objects.representations` 同为 151，但没有 `classes` 项 |
| `PIDSignalPort` | 107 | 107 | 214 | `classes` 无此项 |
| `PIDPipingBranchPoint` | 14 | 0 | 14 | `classes` 无此项 |
| `PIDSignalBranchPoint` | 1 | 0 | 1 | `classes` 无此项 |
| `PIDPipeline` | 14 | 14 | 42 | `classes` 无此项 |
| `PIDBranchPoint` | 13 | 13 | 52 | 与 `left_out` 条数同为 13（`model_item_type`：OPC），没有 `classes` 项 |

`Rel` 627 条也不在 `classes` 里。覆盖报告的 `relationships` 合计 274（`DwgRepresentationComposition` 151、`DrawingItems` 123），是导出器写出的子集。

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

#### `PIDControlSystemFunction`（5）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 5 | `UID` | 5 | 5 |
| `IObject` | 5 | `Name` | 4 | 4 |
| `IPBSItem` | 5 | `ConstructionStatus` | 5 | 5 |
| `IPBSItem` | 5 | `ConstructionStatus2` | 5 | 5 |
| `IControlSystemFunction` | 5 | — | 0 | 0 |
| `IDrawingItem` | 5 | — | 0 | 0 |
| `ISignalPortComposition` | 5 | — | 0 | 0 |
| `IInstrument` | 5 | — | 0 | 0 |
| `IPlannedMatl` | 5 | — | 0 | 0 |
| `ILoopMember` | 5 | — | 0 | 0 |
| `IDocumentItem` | 5 | — | 0 | 0 |
| `INoteCollection` | 5 | — | 0 | 0 |
| `IExpandableThing` | 5 | — | 0 | 0 |
| `INamedInstrument` | 5 | `InstrFuncModifier` | 5 | 0 |
| `INamedInstrument` | 5 | `InstrLoopSuffix` | 5 | 0 |
| `INamedInstrument` | 5 | `InstrTagPrefix` | 5 | 0 |
| `INamedInstrument` | 5 | `InstrTagSequenceNo` | 5 | 4 |
| `INamedInstrument` | 5 | `InstrTagSuffix` | 5 | 0 |
| `INamedInstrument` | 5 | `MeasuredVariable` | 5 | 4 |

#### `PIDInlineInstrument`（2）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 2 | `UID` | 2 | 2 |
| `IObject` | 2 | `Name` | 2 | 2 |
| `IInlineInstrument` | 2 | — | 0 | 0 |
| `IInstrument` | 2 | `ProcFunc` | 2 | 2 |
| `IInstrument` | 2 | `InstrumentType3` | 2 | 2 |
| `IInstrument` | 2 | `InstrumentType` | 2 | 2 |
| `IInstrumentOcc` | 2 | — | 0 | 0 |
| `IPBSItem` | 2 | `ConstructionStatus` | 2 | 2 |
| `IPBSItem` | 2 | `ConstructionStatus2` | 2 | 2 |
| `IPipingPortComposition` | 2 | — | 0 | 0 |
| `IPlannedMatl` | 2 | — | 0 | 0 |
| `IDrawingItem` | 2 | — | 0 | 0 |
| `IInlineInstrumentOcc` | 2 | — | 0 | 0 |
| `IExpandableThing` | 2 | — | 0 | 0 |
| `IDuctPortComposition` | 2 | — | 0 | 0 |
| `IInlineComponentOcc` | 2 | — | 0 | 0 |
| `IHeatTracedItem` | 2 | `HTraceRqmt` | 2 | 0 |
| `ILoopMember` | 2 | — | 0 | 0 |
| `IPartOcc` | 2 | — | 0 | 0 |
| `IPressureReliefItem` | 2 | `ReliefPressure` | 1 | 1 |
| `IProcessPointCollection` | 2 | — | 0 | 0 |
| `ISignalPortComposition` | 2 | — | 0 | 0 |
| `IDocumentItem` | 2 | — | 0 | 0 |
| `IElecPowerConsumer` | 2 | — | 0 | 0 |
| `IPart` | 2 | — | 0 | 0 |
| `INoteCollection` | 2 | — | 0 | 0 |
| `INamedInstrument` | 2 | `InstrFuncModifier` | 2 | 0 |
| `INamedInstrument` | 2 | `InstrLoopSuffix` | 2 | 0 |
| `INamedInstrument` | 2 | `InstrTagPrefix` | 2 | 0 |
| `INamedInstrument` | 2 | `InstrTagSequenceNo` | 2 | 2 |
| `INamedInstrument` | 2 | `InstrTagSuffix` | 2 | 0 |
| `INamedInstrument` | 2 | `MeasuredVariable` | 2 | 2 |
| `IPipeCrossSectionItem` | 2 | `NominalDiameter` | 2 | 2 |
| `IInlineComponent` | 2 | `IsFlowDirectional` | 2 | 2 |
| `IPIDTypical` | 2 | `IsTypical` | 2 | 2 |
| `IInsulatedItem` | 1 | `InsulSpec` | 1 | 1 |

#### `PIDInstrument`（10）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 10 | `UID` | 10 | 10 |
| `IObject` | 10 | `Name` | 9 | 9 |
| `IInstrument` | 10 | — | 0 | 0 |
| `IInstrumentOcc` | 10 | `InstrumentLocation` | 9 | 9 |
| `IPBSItem` | 10 | `ConstructionStatus` | 10 | 10 |
| `IPBSItem` | 10 | `ConstructionStatus2` | 10 | 10 |
| `IPlannedMatl` | 10 | — | 0 | 0 |
| `IDrawingItem` | 10 | — | 0 | 0 |
| `IExpandableThing` | 10 | — | 0 | 0 |
| `IInlineComponentOcc` | 10 | — | 0 | 0 |
| `IPipingPortComposition` | 10 | — | 0 | 0 |
| `IHeatTracedItem` | 10 | `HTraceRqmt` | 10 | 0 |
| `ILoopMember` | 10 | — | 0 | 0 |
| `IPartOcc` | 10 | — | 0 | 0 |
| `IPressureReliefItem` | 10 | — | 0 | 0 |
| `IProcessPointCollection` | 10 | — | 0 | 0 |
| `ISignalPortComposition` | 10 | — | 0 | 0 |
| `IDocumentItem` | 10 | — | 0 | 0 |
| `IElecPowerConsumer` | 10 | — | 0 | 0 |
| `IPart` | 10 | — | 0 | 0 |
| `INoteCollection` | 10 | — | 0 | 0 |
| `INamedInstrument` | 10 | `InstrFuncModifier` | 10 | 0 |
| `INamedInstrument` | 10 | `InstrLoopSuffix` | 10 | 0 |
| `INamedInstrument` | 10 | `InstrTagPrefix` | 10 | 0 |
| `INamedInstrument` | 10 | `InstrTagSequenceNo` | 10 | 9 |
| `INamedInstrument` | 10 | `InstrTagSuffix` | 10 | 0 |
| `INamedInstrument` | 10 | `MeasuredVariable` | 10 | 9 |
| `IPIDTypical` | 10 | `IsTypical` | 10 | 10 |

#### `PIDNote`（7）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 7 | `UID` | 7 | 7 |
| `IDrawingItem` | 7 | — | 0 | 0 |
| `IPBSNote` | 7 | — | 0 | 0 |
| `INote` | 7 | `NoteText` | 3 | 3 |
| `IDocumentItem` | 7 | — | 0 | 0 |

#### `PIDNozzle`（5）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 5 | `UID` | 5 | 5 |
| `IPBSItem` | 5 | `ConstructionStatus` | 5 | 5 |
| `IPBSItem` | 5 | `ConstructionStatus2` | 5 | 5 |
| `IPipingPortComposition` | 5 | — | 0 | 0 |
| `IPlannedMatl` | 5 | — | 0 | 0 |
| `IDrawingItem` | 5 | — | 0 | 0 |
| `INozzleOcc` | 5 | — | 0 | 0 |
| `INozzle` | 5 | — | 0 | 0 |
| `IEquipmentComponent` | 5 | `ProcessEqCompType1` | 5 | 5 |
| `IEquipmentComponent` | 5 | `ProcessEqCompType2` | 5 | 5 |
| `IEquipmentComponent` | 5 | `ProcEqpCompTypeDescription` | 5 | 5 |
| `IEquipmentComponentOcc` | 5 | — | 0 | 0 |
| `IFabricatedItem` | 5 | — | 0 | 0 |
| `IHeatTracedItem` | 5 | `HTraceRqmt` | 5 | 0 |
| `IPBSItemCollection` | 5 | — | 0 | 0 |
| `IProcessPointCollection` | 5 | — | 0 | 0 |
| `ISignalPortComposition` | 5 | — | 0 | 0 |
| `IPartOcc` | 5 | — | 0 | 0 |
| `IDocumentItem` | 5 | — | 0 | 0 |
| `IElecPowerConsumer` | 5 | — | 0 | 0 |
| `IPart` | 5 | — | 0 | 0 |
| `INoteCollection` | 5 | — | 0 | 0 |
| `IProcessDataCaseComposition` | 5 | — | 0 | 0 |
| `IPipeCrossSectionItem` | 5 | `NominalDiameter` | 4 | 4 |
| `IPipingSpecifiedItem` | 4 | `PipingMaterialsClass` | 4 | 4 |
| `IPIDTypical` | 5 | `IsTypical` | 5 | 5 |

#### `PIDPipingComponent`（22）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 22 | `UID` | 22 | 22 |
| `IObject` | 22 | `Name` | 14 | 14 |
| `IPBSItem` | 22 | `ConstructionStatus` | 22 | 22 |
| `IPBSItem` | 22 | `ConstructionStatus2` | 22 | 22 |
| `IPipingPortComposition` | 22 | — | 0 | 0 |
| `IPlannedMatl` | 22 | — | 0 | 0 |
| `IDrawingItem` | 22 | — | 0 | 0 |
| `IPipingComponent` | 22 | `PipingComponentType1` | 22 | 22 |
| `IPipingComponent` | 22 | `PipingComponentType3` | 19 | 19 |
| `IPipingComponent` | 22 | `PipingComponentType2` | 22 | 22 |
| `IPipingComponent` | 22 | `PipModelCode` | 22 | 22 |
| `IPipingComponent` | 22 | `CommoditySpecialtyType` | 22 | 22 |
| `IPipingComponentOcc` | 22 | — | 0 | 0 |
| `IInlineComponentOcc` | 22 | — | 0 | 0 |
| `IFabricatedItem` | 22 | — | 0 | 0 |
| `IHeatTracedItem` | 22 | `HTraceRqmt` | 22 | 0 |
| `IPartOcc` | 22 | — | 0 | 0 |
| `IPressureReliefItem` | 22 | — | 0 | 0 |
| `IDocumentItem` | 22 | — | 0 | 0 |
| `IElecPowerConsumer` | 22 | — | 0 | 0 |
| `IPart` | 22 | — | 0 | 0 |
| `INoteCollection` | 22 | — | 0 | 0 |
| `IPipeCrossSectionItem` | 22 | `NominalDiameter` | 14 | 14 |
| `IInsulatedItem` | 11 | `InsulSpec` | 11 | 11 |
| `IInlineComponent` | 22 | `IsFlowDirectional` | 22 | 22 |
| `IPIDTypical` | 22 | `IsTypical` | 22 | 22 |

#### `PIDPipingConnector`（29）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 29 | `UID` | 29 | 29 |
| `IObject` | 29 | `Name` | 15 | 15 |
| `IPBSItem` | 29 | `ConstructionStatus` | 29 | 29 |
| `IPBSItem` | 29 | `ConstructionStatus2` | 29 | 29 |
| `IPlannedFacility` | 29 | — | 0 | 0 |
| `IConnector` | 29 | `FlowDirection` | 29 | 10 |
| `IConnector` | 29 | `RepresentationsAreAllZeroLength` | 29 | 29 |
| `IDrawingItem` | 29 | — | 0 | 0 |
| `IPBSItemCollection` | 29 | — | 0 | 0 |
| `IPipingConnector` | 29 | `PipingConnectorType` | 29 | 29 |
| `IFabricatedItem` | 29 | — | 0 | 0 |
| `IHeatTracedItem` | 29 | `HTraceRqmt` | 29 | 0 |
| `IProcessPointCollection` | 29 | — | 0 | 0 |
| `IDocumentItem` | 29 | — | 0 | 0 |
| `IElecPowerConsumer` | 29 | — | 0 | 0 |
| `INoteCollection` | 29 | — | 0 | 0 |
| `IProcessDataCaseComposition` | 29 | — | 0 | 0 |
| `IExpandableThing` | 29 | — | 0 | 0 |
| `INamedPipingConnector` | 15 | `PipingConnectorPrefix` | 15 | 0 |
| `INamedPipingConnector` | 15 | `PipingConnectorSeqNo` | 15 | 15 |
| `INamedPipingConnector` | 15 | `PipingConnectorSuff` | 15 | 0 |
| `IPipeCrossSectionItem` | 29 | `NominalDiameter` | 20 | 20 |
| `IPipingSpecifiedItem` | 21 | `PipingMaterialsClass` | 21 | 21 |
| `IJacketedItem` | 29 | — | 0 | 0 |
| `IPIDTypical` | 29 | `IsTypical` | 29 | 29 |
| `IInsulatedItem` | 9 | `InsulSpec` | 9 | 9 |

#### `PIDProcessPoint`（29）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 29 | `UID` | 29 | 29 |
| `IProcessPoint` | 29 | — | 0 | 0 |
| `IFacilityPoint` | 29 | — | 0 | 0 |
| `IProcessPointCaseComposition` | 29 | — | 0 | 0 |
| `IBulkMaxProcessPoint` | 29 | `PhaseTemperatureMax` | 29 | 0 |
| `IBulkMaxProcessPoint` | 29 | `PressureMax` | 29 | 0 |
| `IBulkMinProcessPoint` | 29 | `PhaseTemperatureMin` | 29 | 0 |
| `IBulkMinProcessPoint` | 29 | `PressureMin` | 29 | 0 |
| `IBulkNormProcessPoint` | 29 | `MolecularWeightNorm` | 29 | 0 |
| `IBulkNormProcessPoint` | 29 | `CpCvRatioNorm` | 29 | 0 |
| `IBulkBaseProcessPoint` | 29 | `PhaseTemperatureBase` | 29 | 0 |
| `IBulkBaseProcessPoint` | 29 | `PressureBase` | 29 | 0 |

#### `PIDPipingPort`（53）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 53 | `UID` | 53 | 53 |
| `IObject` | 53 | `Name` | 53 | 53 |
| `IConnection` | 53 | `ConnectionFlowDirection` | 33 | 33 |
| `IPipingPort` | 53 | — | 0 | 0 |
| `IPipingConnection` | 53 | — | 0 | 0 |
| `IPort` | 53 | — | 0 | 0 |
| `IPipeCrossSectionItem` | 53 | `NominalDiameter` | 36 | 36 |

#### `PIDRepresentation`（151）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 151 | `UID` | 151 | 151 |
| `IDrawingRepresentation` | 151 | `GraphicOID` | 151 | 151 |

#### `PIDSignalConnector`（6）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 6 | `UID` | 6 | 6 |
| `IPlannedFacility` | 6 | — | 0 | 0 |
| `IConnector` | 6 | `FlowDirection` | 6 | 0 |
| `IDrawingItem` | 6 | — | 0 | 0 |
| `ISignalConnector` | 6 | — | 0 | 0 |
| `IDocumentItem` | 6 | — | 0 | 0 |
| `IExpandableThing` | 6 | — | 0 | 0 |
| `IPIDTypical` | 6 | `IsTypical` | 6 | 6 |

#### `PIDSignalPort`（107）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 107 | `UID` | 107 | 107 |
| `IObject` | 107 | `Name` | 107 | 107 |
| `IConnection` | 107 | — | 0 | 0 |
| `ISignalConnection` | 107 | — | 0 | 0 |
| `ISignalPort` | 107 | — | 0 | 0 |
| `IPipingConnection` | 107 | — | 0 | 0 |
| `IFacilityPoint` | 107 | — | 0 | 0 |
| `IPIDTypical` | 107 | — | 0 | 0 |

#### `PIDPipingBranchPoint`（14）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 14 | `UID` | 14 | 14 |
| `IConnection` | 14 | — | 0 | 0 |
| `IPipingConnection` | 14 | — | 0 | 0 |
| `IDrawingItem` | 14 | — | 0 | 0 |
| `IPipingBranchPoint` | 14 | — | 0 | 0 |
| `IDocumentItem` | 14 | — | 0 | 0 |

#### `PIDSignalBranchPoint`（1）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 1 | `UID` | 1 | 1 |
| `IConnection` | 1 | — | 0 | 0 |
| `IDrawingItem` | 1 | — | 0 | 0 |
| `ISignalConnection` | 1 | — | 0 | 0 |
| `ISignalBranchPoint` | 1 | — | 0 | 0 |
| `IFacilityPoint` | 1 | — | 0 | 0 |
| `IDocumentItem` | 1 | — | 0 | 0 |

#### `PIDPipeline`（14）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 14 | `UID` | 14 | 14 |
| `IObject` | 14 | `Name` | 14 | 14 |
| `IPBSItem` | 14 | — | 0 | 0 |
| `IPlannedFacility` | 14 | — | 0 | 0 |
| `IPBSItemCollection` | 14 | — | 0 | 0 |
| `IPipeline` | 14 | — | 0 | 0 |
| `IPipingConnectorComposition` | 14 | — | 0 | 0 |
| `IFluidSystem` | 14 | `FluidSystem` | 13 | 13 |
| `IFluidSystem` | 14 | `FluidCode` | 1 | 1 |
| `INoteCollection` | 14 | — | 0 | 0 |
| `IExpandableThing` | 14 | — | 0 | 0 |
| `IPIDTypical` | 14 | — | 0 | 0 |

#### `PIDProcessVessel`（1）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 1 | `UID` | 1 | 1 |
| `IObject` | 1 | `Name` | 1 | 1 |
| `IPIDProcessVesselOcc` | 1 | — | 0 | 0 |
| `IProcessVesselOcc` | 1 | — | 0 | 0 |
| `IEquipment` | 1 | `EqType0` | 1 | 1 |
| `IEquipment` | 1 | `EqType4` | 1 | 1 |
| `IEquipment` | 1 | `EqType3` | 1 | 1 |
| `IEquipment` | 1 | `EqType2` | 1 | 1 |
| `IEquipment` | 1 | `EqType1` | 1 | 1 |
| `IEquipment` | 1 | `EqTypeDescription` | 1 | 1 |
| `IEquipmentOcc` | 1 | — | 0 | 0 |
| `IPBSItem` | 1 | `ConstructionStatus` | 1 | 1 |
| `IPBSItem` | 1 | `ConstructionStatus2` | 1 | 1 |
| `IPBSItemCollection` | 1 | — | 0 | 0 |
| `IPlannedMatl` | 1 | — | 0 | 0 |
| `IProcessEquipment` | 1 | — | 0 | 0 |
| `IProcessEquipmentOcc` | 1 | — | 0 | 0 |
| `IProcessVessel` | 1 | — | 0 | 0 |
| `IDrawingItem` | 1 | — | 0 | 0 |
| `IPIDProcessVessel` | 1 | — | 0 | 0 |
| `ILowPressureTank` | 1 | — | 0 | 0 |
| `INamedEquipment` | 1 | `EquipPrefix` | 1 | 1 |
| `INamedEquipment` | 1 | `EquipSeqNo` | 1 | 1 |
| `INamedEquipment` | 1 | `EquipSuff` | 1 | 0 |
| `IPIDTypical` | 1 | `IsTypical` | 1 | 1 |

#### `PIDBranchPoint`（13）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 13 | `UID` | 13 | 13 |
| `IObject` | 13 | `Name` | 13 | 13 |
| `IObject` | 13 | `Description` | 13 | 13 |
| `IPIDBranchPoint` | 13 | — | 0 | 0 |
| `IOffpageConnector` | 13 | `ToFromText` | 13 | 13 |
| `IDuctConnection` | 13 | — | 0 | 0 |
| `IConnection` | 13 | — | 0 | 0 |
| `IDrawingItem` | 13 | — | 0 | 0 |
| `IPipingConnection` | 13 | — | 0 | 0 |
| `ISignalConnection` | 13 | — | 0 | 0 |
| `IDocumentItem` | 13 | — | 0 | 0 |

#### `Rel`（627，关系，不是 `classes` 的类）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 627 | `UID` | 627 | 627 |
| `IRel` | 627 | `UID1` | 627 | 627 |
| `IRel` | 627 | `UID2` | 627 | 627 |
| `IRel` | 627 | `DefUID` | 627 | 627 |

## 低温分离器A

- 图名：集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图
- 参考 `_Data.xml`：`D:\work\cad\PID自研软件验证\集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图_Data.xml`
- `_Coverage.json`：`D:\work\cad\PID自研软件验证\publish\集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图_Coverage.json`
- `SoftwareVersion`：`10.00.31.0023`。类对象 679（不含 `Rel`）。`Rel` 884。
- `classes` 共 9 类，这些类名参考里都有。参考里另有 9 个类名不在 `classes` 中。

### 与 `classes` 对照

共有类的对象数与 `classes.objects` 一致。对不上的是 `with_name`（参考里非空 `IObject/@Name` 的个数）和 `attribute_values`（覆盖报告计导出器写出的属性值条数，小于参考里的非空属性次数）。

| 类 | 参考对象 | `classes.objects` | 参考非空 Name | `classes.with_name` | 参考非空属性 | `classes.attribute_values` | 差异 |
|---|---:|---:|---:|---:|---:|---:|---|
| `PIDControlSystemFunction` | 10 | 10 | 8 | 8 | 54 | 24 | 对象数一致；非空属性 54 ≠ 24 |
| `PIDInlineInstrument` | 5 | 5 | 4 | 3 | 52 | 9 | 对象数一致；Name 4 ≠ 3；非空属性 52 ≠ 9 |
| `PIDInstrument` | 15 | 15 | 14 | 13 | 116 | 39 | 对象数一致；Name 14 ≠ 13；非空属性 116 ≠ 39 |
| `PIDNote` | 8 | 8 | 0 | 0 | 10 | 2 | 对象数一致；非空属性 10 ≠ 2 |
| `PIDNozzle` | 9 | 9 | 0 | 0 | 73 | 0 | 对象数一致；非空属性 73 ≠ 0 |
| `PIDPipingComponent` | 25 | 25 | 17 | 0 | 296 | 0 | 对象数一致；Name 17 ≠ 0；非空属性 296 ≠ 0 |
| `PIDPipingConnector` | 41 | 41 | 30 | 0 | 391 | 44 | 对象数一致；Name 30 ≠ 0；非空属性 391 ≠ 44 |
| `PIDProcessVessel` | 1 | 1 | 1 | 1 | 13 | 3 | 对象数一致；非空属性 13 ≠ 3 |
| `PIDSignalConnector` | 9 | 9 | 0 | 0 | 18 | 0 | 对象数一致；非空属性 18 ≠ 0 |

### 不在 `classes` 里的类

| 类 | 参考对象 | 参考非空 Name | 参考非空属性 | 说明 |
|---|---:|---:|---:|---|
| `PIDDrawing` | 1 | 1 | 4 | `classes` 无此项；参考里固定 1 个图纸对象 |
| `PIDProcessPoint` | 41 | 0 | 41 | `classes` 无此项 |
| `PIDPipingPort` | 68 | 68 | 219 | `classes` 无此项 |
| `PIDRepresentation` | 193 | 0 | 386 | 与 `objects.representations` 同为 193，但没有 `classes` 项 |
| `PIDSignalPort` | 197 | 197 | 394 | `classes` 无此项 |
| `PIDPipingBranchPoint` | 15 | 0 | 15 | `classes` 无此项 |
| `PIDSignalBranchPoint` | 1 | 0 | 1 | `classes` 无此项 |
| `PIDPipeline` | 28 | 28 | 85 | `classes` 无此项 |
| `PIDBranchPoint` | 12 | 12 | 48 | 与 `left_out` 条数同为 12（`model_item_type`：OPC），没有 `classes` 项 |

`Rel` 884 条也不在 `classes` 里。覆盖报告的 `relationships` 合计 358（`DwgRepresentationComposition` 193、`DrawingItems` 165），是导出器写出的子集。

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

#### `PIDControlSystemFunction`（10）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 10 | `UID` | 10 | 10 |
| `IObject` | 10 | `Name` | 8 | 8 |
| `IPBSItem` | 10 | `ConstructionStatus` | 10 | 10 |
| `IPBSItem` | 10 | `ConstructionStatus2` | 10 | 10 |
| `IControlSystemFunction` | 10 | — | 0 | 0 |
| `IDrawingItem` | 10 | — | 0 | 0 |
| `ISignalPortComposition` | 10 | — | 0 | 0 |
| `IInstrument` | 10 | — | 0 | 0 |
| `IPlannedMatl` | 10 | — | 0 | 0 |
| `ILoopMember` | 10 | — | 0 | 0 |
| `IDocumentItem` | 10 | — | 0 | 0 |
| `INoteCollection` | 10 | — | 0 | 0 |
| `IExpandableThing` | 10 | — | 0 | 0 |
| `INamedInstrument` | 10 | `InstrFuncModifier` | 10 | 0 |
| `INamedInstrument` | 10 | `InstrLoopSuffix` | 10 | 0 |
| `INamedInstrument` | 10 | `InstrTagPrefix` | 10 | 0 |
| `INamedInstrument` | 10 | `InstrTagSequenceNo` | 10 | 8 |
| `INamedInstrument` | 10 | `InstrTagSuffix` | 10 | 0 |
| `INamedInstrument` | 10 | `MeasuredVariable` | 10 | 8 |

#### `PIDInlineInstrument`（5）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 5 | `UID` | 5 | 5 |
| `IObject` | 5 | `Name` | 4 | 4 |
| `IInlineInstrument` | 5 | — | 0 | 0 |
| `IInstrument` | 5 | `ProcFunc` | 3 | 3 |
| `IInstrument` | 5 | `InstrumentType3` | 3 | 3 |
| `IInstrument` | 5 | `InstrumentType` | 3 | 3 |
| `IInstrumentOcc` | 5 | — | 0 | 0 |
| `IPBSItem` | 5 | `ConstructionStatus` | 5 | 5 |
| `IPBSItem` | 5 | `ConstructionStatus2` | 5 | 5 |
| `IPipingPortComposition` | 5 | — | 0 | 0 |
| `IPlannedMatl` | 5 | — | 0 | 0 |
| `IDrawingItem` | 5 | — | 0 | 0 |
| `IInlineInstrumentOcc` | 5 | — | 0 | 0 |
| `IExpandableThing` | 5 | — | 0 | 0 |
| `IDuctPortComposition` | 5 | — | 0 | 0 |
| `IInlineComponentOcc` | 5 | — | 0 | 0 |
| `IHeatTracedItem` | 5 | `HTraceRqmt` | 5 | 0 |
| `ILoopMember` | 5 | — | 0 | 0 |
| `IPartOcc` | 5 | — | 0 | 0 |
| `IPressureReliefItem` | 5 | `ReliefPressure` | 1 | 1 |
| `IProcessPointCollection` | 5 | — | 0 | 0 |
| `ISignalPortComposition` | 5 | — | 0 | 0 |
| `IDocumentItem` | 5 | — | 0 | 0 |
| `IElecPowerConsumer` | 5 | — | 0 | 0 |
| `IPart` | 5 | — | 0 | 0 |
| `INoteCollection` | 5 | — | 0 | 0 |
| `INamedInstrument` | 5 | `InstrFuncModifier` | 5 | 0 |
| `INamedInstrument` | 5 | `InstrLoopSuffix` | 5 | 0 |
| `INamedInstrument` | 5 | `InstrTagPrefix` | 5 | 0 |
| `INamedInstrument` | 5 | `InstrTagSequenceNo` | 5 | 4 |
| `INamedInstrument` | 5 | `InstrTagSuffix` | 5 | 0 |
| `INamedInstrument` | 5 | `MeasuredVariable` | 5 | 4 |
| `IPipeCrossSectionItem` | 5 | `NominalDiameter` | 3 | 3 |
| `IInsulatedItem` | 2 | `InsulSpec` | 2 | 2 |
| `IInlineComponent` | 5 | `IsFlowDirectional` | 5 | 5 |
| `IPIDTypical` | 5 | `IsTypical` | 5 | 5 |

#### `PIDInstrument`（15）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 15 | `UID` | 15 | 15 |
| `IObject` | 15 | `Name` | 14 | 14 |
| `IInstrument` | 15 | — | 0 | 0 |
| `IInstrumentOcc` | 15 | `InstrumentLocation` | 14 | 14 |
| `IPBSItem` | 15 | `ConstructionStatus` | 15 | 15 |
| `IPBSItem` | 15 | `ConstructionStatus2` | 15 | 15 |
| `IPlannedMatl` | 15 | — | 0 | 0 |
| `IDrawingItem` | 15 | — | 0 | 0 |
| `IExpandableThing` | 15 | — | 0 | 0 |
| `IInlineComponentOcc` | 15 | — | 0 | 0 |
| `IPipingPortComposition` | 15 | — | 0 | 0 |
| `IHeatTracedItem` | 15 | `HTraceRqmt` | 15 | 0 |
| `ILoopMember` | 15 | — | 0 | 0 |
| `IPartOcc` | 15 | — | 0 | 0 |
| `IPressureReliefItem` | 15 | — | 0 | 0 |
| `IProcessPointCollection` | 15 | — | 0 | 0 |
| `ISignalPortComposition` | 15 | — | 0 | 0 |
| `IDocumentItem` | 15 | — | 0 | 0 |
| `IElecPowerConsumer` | 15 | — | 0 | 0 |
| `IPart` | 15 | — | 0 | 0 |
| `INoteCollection` | 15 | — | 0 | 0 |
| `INamedInstrument` | 15 | `InstrFuncModifier` | 15 | 0 |
| `INamedInstrument` | 15 | `InstrLoopSuffix` | 15 | 0 |
| `INamedInstrument` | 15 | `InstrTagPrefix` | 15 | 0 |
| `INamedInstrument` | 15 | `InstrTagSequenceNo` | 15 | 14 |
| `INamedInstrument` | 15 | `InstrTagSuffix` | 15 | 0 |
| `INamedInstrument` | 15 | `MeasuredVariable` | 15 | 14 |
| `IPIDTypical` | 15 | `IsTypical` | 15 | 15 |

#### `PIDNote`（8）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 8 | `UID` | 8 | 8 |
| `IDrawingItem` | 8 | — | 0 | 0 |
| `IPBSNote` | 8 | — | 0 | 0 |
| `INote` | 8 | `NoteText` | 2 | 2 |
| `IDocumentItem` | 8 | — | 0 | 0 |

#### `PIDNozzle`（9）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 9 | `UID` | 9 | 9 |
| `IPBSItem` | 9 | `ConstructionStatus` | 9 | 9 |
| `IPBSItem` | 9 | `ConstructionStatus2` | 9 | 9 |
| `IPipingPortComposition` | 9 | — | 0 | 0 |
| `IPlannedMatl` | 9 | — | 0 | 0 |
| `IDrawingItem` | 9 | — | 0 | 0 |
| `INozzleOcc` | 9 | — | 0 | 0 |
| `INozzle` | 9 | — | 0 | 0 |
| `IEquipmentComponent` | 9 | `ProcessEqCompType1` | 9 | 9 |
| `IEquipmentComponent` | 9 | `ProcessEqCompType2` | 9 | 9 |
| `IEquipmentComponent` | 9 | `ProcEqpCompTypeDescription` | 9 | 9 |
| `IEquipmentComponentOcc` | 9 | — | 0 | 0 |
| `IFabricatedItem` | 9 | — | 0 | 0 |
| `IHeatTracedItem` | 9 | `HTraceRqmt` | 9 | 0 |
| `IPBSItemCollection` | 9 | — | 0 | 0 |
| `IProcessPointCollection` | 9 | — | 0 | 0 |
| `ISignalPortComposition` | 9 | — | 0 | 0 |
| `IPartOcc` | 9 | — | 0 | 0 |
| `IDocumentItem` | 9 | — | 0 | 0 |
| `IElecPowerConsumer` | 9 | — | 0 | 0 |
| `IPart` | 9 | — | 0 | 0 |
| `INoteCollection` | 9 | — | 0 | 0 |
| `IProcessDataCaseComposition` | 9 | — | 0 | 0 |
| `IPipeCrossSectionItem` | 9 | `NominalDiameter` | 5 | 5 |
| `IPipingSpecifiedItem` | 5 | `PipingMaterialsClass` | 5 | 5 |
| `IPIDTypical` | 9 | `IsTypical` | 9 | 9 |

#### `PIDPipingComponent`（25）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 25 | `UID` | 25 | 25 |
| `IObject` | 25 | `Name` | 17 | 17 |
| `IPBSItem` | 25 | `ConstructionStatus` | 25 | 25 |
| `IPBSItem` | 25 | `ConstructionStatus2` | 25 | 25 |
| `IPipingPortComposition` | 25 | — | 0 | 0 |
| `IPlannedMatl` | 25 | — | 0 | 0 |
| `IDrawingItem` | 25 | — | 0 | 0 |
| `IPipingComponent` | 25 | `PipingComponentType1` | 25 | 25 |
| `IPipingComponent` | 25 | `PipingComponentType3` | 22 | 22 |
| `IPipingComponent` | 25 | `PipingComponentType2` | 25 | 25 |
| `IPipingComponent` | 25 | `PipModelCode` | 25 | 25 |
| `IPipingComponent` | 25 | `CommoditySpecialtyType` | 25 | 25 |
| `IPipingComponentOcc` | 25 | — | 0 | 0 |
| `IInlineComponentOcc` | 25 | — | 0 | 0 |
| `IFabricatedItem` | 25 | — | 0 | 0 |
| `IHeatTracedItem` | 25 | `HTraceRqmt` | 25 | 0 |
| `IPartOcc` | 25 | — | 0 | 0 |
| `IPressureReliefItem` | 25 | — | 0 | 0 |
| `IDocumentItem` | 25 | — | 0 | 0 |
| `IElecPowerConsumer` | 25 | — | 0 | 0 |
| `IPart` | 25 | — | 0 | 0 |
| `INoteCollection` | 25 | — | 0 | 0 |
| `IPipeCrossSectionItem` | 25 | `NominalDiameter` | 17 | 17 |
| `IInlineComponent` | 25 | `IsFlowDirectional` | 25 | 25 |
| `IPIDTypical` | 25 | `IsTypical` | 25 | 25 |
| `IInsulatedItem` | 15 | `InsulSpec` | 15 | 15 |

#### `PIDPipingConnector`（41）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 41 | `UID` | 41 | 41 |
| `IObject` | 41 | `Name` | 30 | 30 |
| `IPBSItem` | 41 | `ConstructionStatus` | 41 | 41 |
| `IPBSItem` | 41 | `ConstructionStatus2` | 41 | 41 |
| `IPlannedFacility` | 41 | — | 0 | 0 |
| `IConnector` | 41 | `FlowDirection` | 41 | 15 |
| `IConnector` | 41 | `RepresentationsAreAllZeroLength` | 41 | 41 |
| `IDrawingItem` | 41 | — | 0 | 0 |
| `IPBSItemCollection` | 41 | — | 0 | 0 |
| `IPipingConnector` | 41 | `PipingConnectorType` | 41 | 41 |
| `IFabricatedItem` | 41 | — | 0 | 0 |
| `IHeatTracedItem` | 41 | `HTraceRqmt` | 41 | 0 |
| `IProcessPointCollection` | 41 | — | 0 | 0 |
| `IDocumentItem` | 41 | — | 0 | 0 |
| `IElecPowerConsumer` | 41 | — | 0 | 0 |
| `INoteCollection` | 41 | — | 0 | 0 |
| `IProcessDataCaseComposition` | 41 | — | 0 | 0 |
| `IExpandableThing` | 41 | — | 0 | 0 |
| `INamedPipingConnector` | 30 | `PipingConnectorPrefix` | 30 | 0 |
| `INamedPipingConnector` | 30 | `PipingConnectorSeqNo` | 30 | 30 |
| `INamedPipingConnector` | 30 | `PipingConnectorSuff` | 30 | 0 |
| `IPipeCrossSectionItem` | 41 | `NominalDiameter` | 25 | 25 |
| `IPipingSpecifiedItem` | 29 | `PipingMaterialsClass` | 29 | 29 |
| `IJacketedItem` | 41 | — | 0 | 0 |
| `IPIDTypical` | 41 | `IsTypical` | 41 | 41 |
| `IInsulatedItem` | 16 | `InsulSpec` | 16 | 16 |

#### `PIDProcessPoint`（41）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 41 | `UID` | 41 | 41 |
| `IProcessPoint` | 41 | — | 0 | 0 |
| `IFacilityPoint` | 41 | — | 0 | 0 |
| `IProcessPointCaseComposition` | 41 | — | 0 | 0 |
| `IBulkMaxProcessPoint` | 41 | `PhaseTemperatureMax` | 41 | 0 |
| `IBulkMaxProcessPoint` | 41 | `PressureMax` | 41 | 0 |
| `IBulkMinProcessPoint` | 41 | `PhaseTemperatureMin` | 41 | 0 |
| `IBulkMinProcessPoint` | 41 | `PressureMin` | 41 | 0 |
| `IBulkNormProcessPoint` | 41 | `MolecularWeightNorm` | 41 | 0 |
| `IBulkNormProcessPoint` | 41 | `CpCvRatioNorm` | 41 | 0 |
| `IBulkBaseProcessPoint` | 41 | `PhaseTemperatureBase` | 41 | 0 |
| `IBulkBaseProcessPoint` | 41 | `PressureBase` | 41 | 0 |

#### `PIDPipingPort`（68）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 68 | `UID` | 68 | 68 |
| `IObject` | 68 | `Name` | 68 | 68 |
| `IConnection` | 68 | `ConnectionFlowDirection` | 38 | 38 |
| `IPipingPort` | 68 | — | 0 | 0 |
| `IPipingConnection` | 68 | — | 0 | 0 |
| `IPort` | 68 | — | 0 | 0 |
| `IPipeCrossSectionItem` | 68 | `NominalDiameter` | 45 | 45 |

#### `PIDRepresentation`（193）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 193 | `UID` | 193 | 193 |
| `IDrawingRepresentation` | 193 | `GraphicOID` | 193 | 193 |

#### `PIDSignalConnector`（9）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 9 | `UID` | 9 | 9 |
| `IPlannedFacility` | 9 | — | 0 | 0 |
| `IConnector` | 9 | `FlowDirection` | 9 | 0 |
| `IDrawingItem` | 9 | — | 0 | 0 |
| `ISignalConnector` | 9 | — | 0 | 0 |
| `IDocumentItem` | 9 | — | 0 | 0 |
| `IExpandableThing` | 9 | — | 0 | 0 |
| `IPIDTypical` | 9 | `IsTypical` | 9 | 9 |

#### `PIDSignalPort`（197）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 197 | `UID` | 197 | 197 |
| `IObject` | 197 | `Name` | 197 | 197 |
| `IConnection` | 197 | — | 0 | 0 |
| `ISignalConnection` | 197 | — | 0 | 0 |
| `ISignalPort` | 197 | — | 0 | 0 |
| `IPipingConnection` | 197 | — | 0 | 0 |
| `IFacilityPoint` | 197 | — | 0 | 0 |
| `IPIDTypical` | 197 | — | 0 | 0 |

#### `PIDPipingBranchPoint`（15）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 15 | `UID` | 15 | 15 |
| `IConnection` | 15 | — | 0 | 0 |
| `IPipingConnection` | 15 | — | 0 | 0 |
| `IDrawingItem` | 15 | — | 0 | 0 |
| `IPipingBranchPoint` | 15 | — | 0 | 0 |
| `IDocumentItem` | 15 | — | 0 | 0 |

#### `PIDSignalBranchPoint`（1）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 1 | `UID` | 1 | 1 |
| `IConnection` | 1 | — | 0 | 0 |
| `IDrawingItem` | 1 | — | 0 | 0 |
| `ISignalConnection` | 1 | — | 0 | 0 |
| `ISignalBranchPoint` | 1 | — | 0 | 0 |
| `IFacilityPoint` | 1 | — | 0 | 0 |
| `IDocumentItem` | 1 | — | 0 | 0 |

#### `PIDPipeline`（28）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 28 | `UID` | 28 | 28 |
| `IObject` | 28 | `Name` | 28 | 28 |
| `IPBSItem` | 28 | — | 0 | 0 |
| `IPlannedFacility` | 28 | — | 0 | 0 |
| `IPBSItemCollection` | 28 | — | 0 | 0 |
| `IPipeline` | 28 | — | 0 | 0 |
| `IPipingConnectorComposition` | 28 | — | 0 | 0 |
| `IFluidSystem` | 28 | `FluidSystem` | 28 | 28 |
| `IFluidSystem` | 28 | `FluidCode` | 1 | 1 |
| `INoteCollection` | 28 | — | 0 | 0 |
| `IExpandableThing` | 28 | — | 0 | 0 |
| `IPIDTypical` | 28 | — | 0 | 0 |

#### `PIDProcessVessel`（1）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 1 | `UID` | 1 | 1 |
| `IObject` | 1 | `Name` | 1 | 1 |
| `IPIDProcessVesselOcc` | 1 | — | 0 | 0 |
| `IProcessVesselOcc` | 1 | — | 0 | 0 |
| `IEquipment` | 1 | `EqType0` | 1 | 1 |
| `IEquipment` | 1 | `EqType4` | 1 | 1 |
| `IEquipment` | 1 | `EqType3` | 1 | 1 |
| `IEquipment` | 1 | `EqType2` | 1 | 1 |
| `IEquipment` | 1 | `EqType1` | 1 | 1 |
| `IEquipment` | 1 | `EqTypeDescription` | 1 | 1 |
| `IEquipmentOcc` | 1 | — | 0 | 0 |
| `IPBSItem` | 1 | `ConstructionStatus` | 1 | 1 |
| `IPBSItem` | 1 | `ConstructionStatus2` | 1 | 1 |
| `IPBSItemCollection` | 1 | — | 0 | 0 |
| `IPlannedMatl` | 1 | — | 0 | 0 |
| `IProcessEquipment` | 1 | — | 0 | 0 |
| `IProcessEquipmentOcc` | 1 | — | 0 | 0 |
| `IProcessVessel` | 1 | — | 0 | 0 |
| `IDrawingItem` | 1 | — | 0 | 0 |
| `IPIDProcessVessel` | 1 | — | 0 | 0 |
| `ILowPressureTank` | 1 | — | 0 | 0 |
| `INamedEquipment` | 1 | `EquipPrefix` | 1 | 1 |
| `INamedEquipment` | 1 | `EquipSeqNo` | 1 | 1 |
| `INamedEquipment` | 1 | `EquipSuff` | 1 | 0 |
| `IPIDTypical` | 1 | `IsTypical` | 1 | 1 |

#### `PIDBranchPoint`（12）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 12 | `UID` | 12 | 12 |
| `IObject` | 12 | `Name` | 12 | 12 |
| `IObject` | 12 | `Description` | 12 | 12 |
| `IPIDBranchPoint` | 12 | — | 0 | 0 |
| `IOffpageConnector` | 12 | `ToFromText` | 12 | 12 |
| `IDuctConnection` | 12 | — | 0 | 0 |
| `IConnection` | 12 | — | 0 | 0 |
| `IDrawingItem` | 12 | — | 0 | 0 |
| `IPipingConnection` | 12 | — | 0 | 0 |
| `ISignalConnection` | 12 | — | 0 | 0 |
| `IDocumentItem` | 12 | — | 0 | 0 |

#### `Rel`（884，关系，不是 `classes` 的类）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 884 | `UID` | 884 | 884 |
| `IRel` | 884 | `UID1` | 884 | 884 |
| `IRel` | 884 | `UID2` | 884 | 884 |
| `IRel` | 884 | `DefUID` | 884 | 884 |

## 换热器CD

- 图名：集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图
- 参考 `_Data.xml`：`D:\work\cad\PID自研软件验证\集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图_Data.xml`
- `_Coverage.json`：`D:\work\cad\PID自研软件验证\publish\集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图_Coverage.json`
- `SoftwareVersion`：`10.00.31.0023`。类对象 1319（不含 `Rel`）。`Rel` 1800。
- `classes` 共 10 类，这些类名参考里都有。参考里另有 9 个类名不在 `classes` 中。

### 与 `classes` 对照

共有类的对象数与 `classes.objects` 一致。对不上的是 `with_name`（参考里非空 `IObject/@Name` 的个数）和 `attribute_values`（覆盖报告计导出器写出的属性值条数，小于参考里的非空属性次数）。

| 类 | 参考对象 | `classes.objects` | 参考非空 Name | `classes.with_name` | 参考非空属性 | `classes.attribute_values` | 差异 |
|---|---:|---:|---:|---:|---:|---:|---|
| `PIDControlSystemFunction` | 18 | 18 | 16 | 16 | 102 | 48 | 对象数一致；非空属性 102 ≠ 48 |
| `PIDHeatTransferEquipment` | 1 | 1 | 1 | 1 | 10 | 3 | 对象数一致；非空属性 10 ≠ 3 |
| `PIDInlineInstrument` | 23 | 23 | 11 | 8 | 209 | 24 | 对象数一致；Name 11 ≠ 8；非空属性 209 ≠ 24 |
| `PIDInstrument` | 32 | 32 | 28 | 26 | 238 | 78 | 对象数一致；Name 28 ≠ 26；非空属性 238 ≠ 78 |
| `PIDNote` | 2 | 2 | 0 | 0 | 4 | 2 | 对象数一致；非空属性 4 ≠ 2 |
| `PIDNozzle` | 10 | 10 | 0 | 0 | 83 | 0 | 对象数一致；非空属性 83 ≠ 0 |
| `PIDPipingComponent` | 56 | 56 | 31 | 0 | 611 | 0 | 对象数一致；Name 31 ≠ 0；非空属性 611 ≠ 0 |
| `PIDPipingConnector` | 63 | 63 | 38 | 0 | 554 | 35 | 对象数一致；Name 38 ≠ 0；非空属性 554 ≠ 35 |
| `PIDProcessEquipment` | 1 | 1 | 1 | 1 | 8 | 3 | 对象数一致；非空属性 8 ≠ 3 |
| `PIDSignalConnector` | 16 | 16 | 0 | 0 | 32 | 0 | 对象数一致；非空属性 32 ≠ 0 |

### 不在 `classes` 里的类

| 类 | 参考对象 | 参考非空 Name | 参考非空属性 | 说明 |
|---|---:|---:|---:|---|
| `PIDDrawing` | 1 | 1 | 4 | `classes` 无此项；参考里固定 1 个图纸对象 |
| `PIDProcessPoint` | 63 | 0 | 63 | `classes` 无此项 |
| `PIDPipingPort` | 172 | 172 | 480 | `classes` 无此项 |
| `PIDRepresentation` | 389 | 0 | 778 | 与 `objects.representations` 同为 389，但没有 `classes` 项 |
| `PIDSignalPort` | 399 | 399 | 798 | `classes` 无此项 |
| `PIDPipingBranchPoint` | 35 | 0 | 35 | `classes` 无此项 |
| `PIDSignalBranchPoint` | 3 | 0 | 3 | `classes` 无此项 |
| `PIDPipeline` | 22 | 22 | 65 | `classes` 无此项 |
| `PIDBranchPoint` | 13 | 13 | 52 | 与 `left_out` 条数同为 13（`model_item_type`：OPC），没有 `classes` 项 |

`Rel` 1800 条也不在 `classes` 里。覆盖报告的 `relationships` 合计 727（`DwgRepresentationComposition` 389、`DrawingItems` 338），是导出器写出的子集。

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

#### `PIDControlSystemFunction`（18）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 18 | `UID` | 18 | 18 |
| `IObject` | 18 | `Name` | 16 | 16 |
| `IPBSItem` | 18 | `ConstructionStatus` | 18 | 18 |
| `IPBSItem` | 18 | `ConstructionStatus2` | 18 | 18 |
| `IControlSystemFunction` | 18 | — | 0 | 0 |
| `IDrawingItem` | 18 | — | 0 | 0 |
| `ISignalPortComposition` | 18 | — | 0 | 0 |
| `IInstrument` | 18 | — | 0 | 0 |
| `IPlannedMatl` | 18 | — | 0 | 0 |
| `ILoopMember` | 18 | — | 0 | 0 |
| `IDocumentItem` | 18 | — | 0 | 0 |
| `INoteCollection` | 18 | — | 0 | 0 |
| `IExpandableThing` | 18 | — | 0 | 0 |
| `INamedInstrument` | 18 | `InstrFuncModifier` | 18 | 0 |
| `INamedInstrument` | 18 | `InstrLoopSuffix` | 18 | 0 |
| `INamedInstrument` | 18 | `InstrTagPrefix` | 18 | 0 |
| `INamedInstrument` | 18 | `InstrTagSequenceNo` | 18 | 16 |
| `INamedInstrument` | 18 | `InstrTagSuffix` | 18 | 0 |
| `INamedInstrument` | 18 | `MeasuredVariable` | 18 | 16 |

#### `PIDInlineInstrument`（23）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 23 | `UID` | 23 | 23 |
| `IObject` | 23 | `Name` | 11 | 11 |
| `IInlineInstrument` | 23 | — | 0 | 0 |
| `IInstrument` | 23 | `ProcFunc` | 16 | 16 |
| `IInstrument` | 23 | `InstrumentType3` | 16 | 16 |
| `IInstrument` | 23 | `InstrumentType` | 16 | 16 |
| `IInstrumentOcc` | 23 | — | 0 | 0 |
| `IPBSItem` | 23 | `ConstructionStatus` | 23 | 23 |
| `IPBSItem` | 23 | `ConstructionStatus2` | 23 | 23 |
| `IPipingPortComposition` | 23 | — | 0 | 0 |
| `IPlannedMatl` | 23 | — | 0 | 0 |
| `IDrawingItem` | 23 | — | 0 | 0 |
| `IInlineInstrumentOcc` | 23 | — | 0 | 0 |
| `IExpandableThing` | 23 | — | 0 | 0 |
| `IDuctPortComposition` | 23 | — | 0 | 0 |
| `IInlineComponentOcc` | 23 | — | 0 | 0 |
| `IHeatTracedItem` | 23 | `HTraceRqmt` | 23 | 0 |
| `ILoopMember` | 23 | — | 0 | 0 |
| `IPartOcc` | 23 | — | 0 | 0 |
| `IPressureReliefItem` | 23 | — | 0 | 0 |
| `IProcessPointCollection` | 23 | — | 0 | 0 |
| `ISignalPortComposition` | 23 | — | 0 | 0 |
| `IDocumentItem` | 23 | — | 0 | 0 |
| `IElecPowerConsumer` | 23 | — | 0 | 0 |
| `IPart` | 23 | — | 0 | 0 |
| `INoteCollection` | 23 | — | 0 | 0 |
| `INamedInstrument` | 23 | `InstrFuncModifier` | 23 | 0 |
| `INamedInstrument` | 23 | `InstrLoopSuffix` | 23 | 0 |
| `INamedInstrument` | 23 | `InstrTagPrefix` | 23 | 0 |
| `INamedInstrument` | 23 | `InstrTagSequenceNo` | 23 | 11 |
| `INamedInstrument` | 23 | `InstrTagSuffix` | 23 | 0 |
| `INamedInstrument` | 23 | `MeasuredVariable` | 23 | 11 |
| `IPipeCrossSectionItem` | 23 | `NominalDiameter` | 12 | 12 |
| `IInlineComponent` | 23 | `IsFlowDirectional` | 23 | 23 |
| `IPIDTypical` | 23 | `IsTypical` | 23 | 23 |
| `IInsulatedItem` | 1 | `InsulSpec` | 1 | 1 |

#### `PIDInstrument`（32）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 32 | `UID` | 32 | 32 |
| `IObject` | 32 | `Name` | 28 | 28 |
| `IInstrument` | 32 | — | 0 | 0 |
| `IInstrumentOcc` | 32 | `InstrumentLocation` | 26 | 26 |
| `IPBSItem` | 32 | `ConstructionStatus` | 32 | 32 |
| `IPBSItem` | 32 | `ConstructionStatus2` | 32 | 32 |
| `IPlannedMatl` | 32 | — | 0 | 0 |
| `IDrawingItem` | 32 | — | 0 | 0 |
| `IExpandableThing` | 32 | — | 0 | 0 |
| `IInlineComponentOcc` | 32 | — | 0 | 0 |
| `IPipingPortComposition` | 32 | — | 0 | 0 |
| `IHeatTracedItem` | 32 | `HTraceRqmt` | 32 | 0 |
| `ILoopMember` | 32 | — | 0 | 0 |
| `IPartOcc` | 32 | — | 0 | 0 |
| `IPressureReliefItem` | 32 | — | 0 | 0 |
| `IProcessPointCollection` | 32 | — | 0 | 0 |
| `ISignalPortComposition` | 32 | — | 0 | 0 |
| `IDocumentItem` | 32 | — | 0 | 0 |
| `IElecPowerConsumer` | 32 | — | 0 | 0 |
| `IPart` | 32 | — | 0 | 0 |
| `INoteCollection` | 32 | — | 0 | 0 |
| `INamedInstrument` | 32 | `InstrFuncModifier` | 32 | 0 |
| `INamedInstrument` | 32 | `InstrLoopSuffix` | 32 | 0 |
| `INamedInstrument` | 32 | `InstrTagPrefix` | 32 | 0 |
| `INamedInstrument` | 32 | `InstrTagSequenceNo` | 32 | 28 |
| `INamedInstrument` | 32 | `InstrTagSuffix` | 32 | 0 |
| `INamedInstrument` | 32 | `MeasuredVariable` | 32 | 28 |
| `IPIDTypical` | 32 | `IsTypical` | 32 | 32 |

#### `PIDNote`（2）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 2 | `UID` | 2 | 2 |
| `IDrawingItem` | 2 | — | 0 | 0 |
| `IPBSNote` | 2 | — | 0 | 0 |
| `INote` | 2 | `NoteText` | 2 | 2 |
| `IDocumentItem` | 2 | — | 0 | 0 |

#### `PIDNozzle`（10）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 10 | `UID` | 10 | 10 |
| `IPBSItem` | 10 | `ConstructionStatus` | 10 | 10 |
| `IPBSItem` | 10 | `ConstructionStatus2` | 10 | 10 |
| `IPipingPortComposition` | 10 | — | 0 | 0 |
| `IPlannedMatl` | 10 | — | 0 | 0 |
| `IDrawingItem` | 10 | — | 0 | 0 |
| `INozzleOcc` | 10 | — | 0 | 0 |
| `INozzle` | 10 | — | 0 | 0 |
| `IEquipmentComponent` | 10 | `ProcessEqCompType1` | 10 | 10 |
| `IEquipmentComponent` | 10 | `ProcessEqCompType2` | 10 | 10 |
| `IEquipmentComponent` | 10 | `ProcEqpCompTypeDescription` | 10 | 10 |
| `IEquipmentComponentOcc` | 10 | — | 0 | 0 |
| `IFabricatedItem` | 10 | — | 0 | 0 |
| `IHeatTracedItem` | 10 | `HTraceRqmt` | 10 | 0 |
| `IPBSItemCollection` | 10 | — | 0 | 0 |
| `IProcessPointCollection` | 10 | — | 0 | 0 |
| `ISignalPortComposition` | 10 | — | 0 | 0 |
| `IPartOcc` | 10 | — | 0 | 0 |
| `IDocumentItem` | 10 | — | 0 | 0 |
| `IElecPowerConsumer` | 10 | — | 0 | 0 |
| `IPart` | 10 | — | 0 | 0 |
| `INoteCollection` | 10 | — | 0 | 0 |
| `IProcessDataCaseComposition` | 10 | — | 0 | 0 |
| `IPipeCrossSectionItem` | 10 | `NominalDiameter` | 10 | 10 |
| `IPIDTypical` | 10 | `IsTypical` | 10 | 10 |
| `IPipingSpecifiedItem` | 3 | `PipingMaterialsClass` | 3 | 3 |

#### `PIDPipingComponent`（56）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 56 | `UID` | 56 | 56 |
| `IObject` | 56 | `Name` | 31 | 31 |
| `IPBSItem` | 56 | `ConstructionStatus` | 56 | 56 |
| `IPBSItem` | 56 | `ConstructionStatus2` | 56 | 56 |
| `IPipingPortComposition` | 56 | — | 0 | 0 |
| `IPlannedMatl` | 56 | — | 0 | 0 |
| `IDrawingItem` | 56 | — | 0 | 0 |
| `IPipingComponent` | 56 | `PipingComponentType1` | 56 | 56 |
| `IPipingComponent` | 56 | `PipingComponentType3` | 43 | 43 |
| `IPipingComponent` | 56 | `PipingComponentType2` | 56 | 56 |
| `IPipingComponent` | 56 | `PipModelCode` | 56 | 52 |
| `IPipingComponent` | 56 | `CommoditySpecialtyType` | 56 | 56 |
| `IPipingComponentOcc` | 56 | — | 0 | 0 |
| `IInlineComponentOcc` | 56 | — | 0 | 0 |
| `IFabricatedItem` | 56 | — | 0 | 0 |
| `IHeatTracedItem` | 56 | `HTraceRqmt` | 56 | 0 |
| `IPartOcc` | 56 | — | 0 | 0 |
| `IPressureReliefItem` | 56 | — | 0 | 0 |
| `IDocumentItem` | 56 | — | 0 | 0 |
| `IElecPowerConsumer` | 56 | — | 0 | 0 |
| `IPart` | 56 | — | 0 | 0 |
| `INoteCollection` | 56 | — | 0 | 0 |
| `IPipeCrossSectionItem` | 56 | `NominalDiameter` | 31 | 31 |
| `IInlineComponent` | 56 | `IsFlowDirectional` | 56 | 56 |
| `IPIDTypical` | 56 | `IsTypical` | 56 | 56 |
| `IInsulatedItem` | 6 | `InsulSpec` | 6 | 6 |

#### `PIDPipingConnector`（63）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 63 | `UID` | 63 | 63 |
| `IObject` | 63 | `Name` | 38 | 38 |
| `IPBSItem` | 63 | `ConstructionStatus` | 63 | 63 |
| `IPBSItem` | 63 | `ConstructionStatus2` | 63 | 63 |
| `IPlannedFacility` | 63 | — | 0 | 0 |
| `IConnector` | 63 | `FlowDirection` | 63 | 7 |
| `IConnector` | 63 | `RepresentationsAreAllZeroLength` | 63 | 63 |
| `IDrawingItem` | 63 | — | 0 | 0 |
| `IPBSItemCollection` | 63 | — | 0 | 0 |
| `IPipingConnector` | 63 | `PipingConnectorType` | 63 | 63 |
| `IFabricatedItem` | 63 | — | 0 | 0 |
| `IHeatTracedItem` | 63 | `HTraceRqmt` | 63 | 0 |
| `IProcessPointCollection` | 63 | — | 0 | 0 |
| `IDocumentItem` | 63 | — | 0 | 0 |
| `IElecPowerConsumer` | 63 | — | 0 | 0 |
| `INoteCollection` | 63 | — | 0 | 0 |
| `IProcessDataCaseComposition` | 63 | — | 0 | 0 |
| `IExpandableThing` | 63 | — | 0 | 0 |
| `INamedPipingConnector` | 38 | `PipingConnectorPrefix` | 38 | 0 |
| `INamedPipingConnector` | 38 | `PipingConnectorSeqNo` | 38 | 38 |
| `INamedPipingConnector` | 38 | `PipingConnectorSuff` | 38 | 0 |
| `IPipeCrossSectionItem` | 63 | `NominalDiameter` | 35 | 35 |
| `IPipingSpecifiedItem` | 52 | `PipingMaterialsClass` | 52 | 52 |
| `IJacketedItem` | 63 | — | 0 | 0 |
| `IPIDTypical` | 63 | `IsTypical` | 63 | 63 |
| `IInsulatedItem` | 6 | `InsulSpec` | 6 | 6 |

#### `PIDProcessPoint`（63）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 63 | `UID` | 63 | 63 |
| `IProcessPoint` | 63 | — | 0 | 0 |
| `IFacilityPoint` | 63 | — | 0 | 0 |
| `IProcessPointCaseComposition` | 63 | — | 0 | 0 |
| `IBulkMaxProcessPoint` | 63 | `PhaseTemperatureMax` | 63 | 0 |
| `IBulkMaxProcessPoint` | 63 | `PressureMax` | 63 | 0 |
| `IBulkMinProcessPoint` | 63 | `PhaseTemperatureMin` | 63 | 0 |
| `IBulkMinProcessPoint` | 63 | `PressureMin` | 63 | 0 |
| `IBulkNormProcessPoint` | 63 | `MolecularWeightNorm` | 63 | 0 |
| `IBulkNormProcessPoint` | 63 | `CpCvRatioNorm` | 63 | 0 |
| `IBulkBaseProcessPoint` | 63 | `PhaseTemperatureBase` | 63 | 0 |
| `IBulkBaseProcessPoint` | 63 | `PressureBase` | 63 | 0 |

#### `PIDPipingPort`（172）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 172 | `UID` | 172 | 172 |
| `IObject` | 172 | `Name` | 172 | 172 |
| `IConnection` | 172 | `ConnectionFlowDirection` | 40 | 40 |
| `IPipingPort` | 172 | — | 0 | 0 |
| `IPipingConnection` | 172 | — | 0 | 0 |
| `IPort` | 172 | — | 0 | 0 |
| `IPipeCrossSectionItem` | 172 | `NominalDiameter` | 96 | 96 |

#### `PIDRepresentation`（389）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 389 | `UID` | 389 | 389 |
| `IDrawingRepresentation` | 389 | `GraphicOID` | 389 | 389 |

#### `PIDSignalConnector`（16）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 16 | `UID` | 16 | 16 |
| `IPlannedFacility` | 16 | — | 0 | 0 |
| `IConnector` | 16 | `FlowDirection` | 16 | 0 |
| `IDrawingItem` | 16 | — | 0 | 0 |
| `ISignalConnector` | 16 | — | 0 | 0 |
| `IDocumentItem` | 16 | — | 0 | 0 |
| `IExpandableThing` | 16 | — | 0 | 0 |
| `IPIDTypical` | 16 | `IsTypical` | 16 | 16 |

#### `PIDSignalPort`（399）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 399 | `UID` | 399 | 399 |
| `IObject` | 399 | `Name` | 399 | 399 |
| `IConnection` | 399 | — | 0 | 0 |
| `ISignalConnection` | 399 | — | 0 | 0 |
| `ISignalPort` | 399 | — | 0 | 0 |
| `IPipingConnection` | 399 | — | 0 | 0 |
| `IFacilityPoint` | 399 | — | 0 | 0 |
| `IPIDTypical` | 399 | — | 0 | 0 |

#### `PIDPipingBranchPoint`（35）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 35 | `UID` | 35 | 35 |
| `IConnection` | 35 | — | 0 | 0 |
| `IPipingConnection` | 35 | — | 0 | 0 |
| `IDrawingItem` | 35 | — | 0 | 0 |
| `IPipingBranchPoint` | 35 | — | 0 | 0 |
| `IDocumentItem` | 35 | — | 0 | 0 |

#### `PIDSignalBranchPoint`（3）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 3 | `UID` | 3 | 3 |
| `IConnection` | 3 | — | 0 | 0 |
| `IDrawingItem` | 3 | — | 0 | 0 |
| `ISignalConnection` | 3 | — | 0 | 0 |
| `ISignalBranchPoint` | 3 | — | 0 | 0 |
| `IFacilityPoint` | 3 | — | 0 | 0 |
| `IDocumentItem` | 3 | — | 0 | 0 |

#### `PIDPipeline`（22）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 22 | `UID` | 22 | 22 |
| `IObject` | 22 | `Name` | 22 | 22 |
| `IPBSItem` | 22 | — | 0 | 0 |
| `IPlannedFacility` | 22 | — | 0 | 0 |
| `IPBSItemCollection` | 22 | — | 0 | 0 |
| `IPipeline` | 22 | — | 0 | 0 |
| `IPipingConnectorComposition` | 22 | — | 0 | 0 |
| `IFluidSystem` | 22 | `FluidCode` | 1 | 1 |
| `IFluidSystem` | 22 | `FluidSystem` | 20 | 20 |
| `INoteCollection` | 22 | — | 0 | 0 |
| `IExpandableThing` | 22 | — | 0 | 0 |
| `IPIDTypical` | 22 | — | 0 | 0 |

#### `PIDProcessEquipment`（1）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 1 | `UID` | 1 | 1 |
| `IObject` | 1 | `Name` | 1 | 1 |
| `IPBSItem` | 1 | `ConstructionStatus` | 1 | 1 |
| `IPBSItem` | 1 | `ConstructionStatus2` | 1 | 1 |
| `IPlannedMatl` | 1 | — | 0 | 0 |
| `IDrawingItem` | 1 | — | 0 | 0 |
| `IEquipmentOcc` | 1 | — | 0 | 0 |
| `IPBSItemCollection` | 1 | — | 0 | 0 |
| `IEquipment` | 1 | `EqType0` | 1 | 1 |
| `INamedEquipment` | 1 | `EquipPrefix` | 1 | 1 |
| `INamedEquipment` | 1 | `EquipSeqNo` | 1 | 1 |
| `INamedEquipment` | 1 | `EquipSuff` | 1 | 0 |
| `IPIDTypical` | 1 | `IsTypical` | 1 | 1 |

#### `PIDHeatTransferEquipment`（1）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 1 | `UID` | 1 | 1 |
| `IObject` | 1 | `Name` | 1 | 1 |
| `IPIDHeatTransferEquipmentOcc` | 1 | — | 0 | 0 |
| `IHeatTransferEquipmentOcc` | 1 | — | 0 | 0 |
| `IPIDHeatTransferEquipment` | 1 | — | 0 | 0 |
| `IHeatTransferEquipment` | 1 | — | 0 | 0 |
| `IProcessEquipmentOcc` | 1 | — | 0 | 0 |
| `IEquipmentOcc` | 1 | — | 0 | 0 |
| `IPlannedMatl` | 1 | — | 0 | 0 |
| `IProcessEquipment` | 1 | — | 0 | 0 |
| `IEquipment` | 1 | `EqType0` | 1 | 1 |
| `IEquipment` | 1 | `EqType1` | 1 | 1 |
| `IEquipment` | 1 | `EqTypeDescription` | 1 | 1 |
| `IDrawingItem` | 1 | — | 0 | 0 |
| `IPBSItem` | 1 | `ConstructionStatus` | 1 | 1 |
| `IPBSItem` | 1 | `ConstructionStatus2` | 1 | 1 |
| `IShellAndTubeHeatExchanger` | 1 | — | 0 | 0 |
| `INamedEquipment` | 1 | `EquipPrefix` | 1 | 1 |
| `INamedEquipment` | 1 | `EquipSeqNo` | 1 | 1 |
| `INamedEquipment` | 1 | `EquipSuff` | 1 | 0 |
| `IPIDTypical` | 1 | `IsTypical` | 1 | 1 |

#### `PIDBranchPoint`（13）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 13 | `UID` | 13 | 13 |
| `IObject` | 13 | `Name` | 13 | 13 |
| `IObject` | 13 | `Description` | 13 | 13 |
| `IPIDBranchPoint` | 13 | — | 0 | 0 |
| `IOffpageConnector` | 13 | `ToFromText` | 13 | 13 |
| `IDuctConnection` | 13 | — | 0 | 0 |
| `IConnection` | 13 | — | 0 | 0 |
| `IDrawingItem` | 13 | — | 0 | 0 |
| `IPipingConnection` | 13 | — | 0 | 0 |
| `ISignalConnection` | 13 | — | 0 | 0 |
| `IDocumentItem` | 13 | — | 0 | 0 |

#### `Rel`（1800，关系，不是 `classes` 的类）

| 接口 | 有该接口的对象 | 属性 | 出现次数 | 非空次数 |
|---|---:|---|---:|---:|
| `IObject` | 1800 | `UID` | 1800 | 1800 |
| `IRel` | 1800 | `UID1` | 1800 | 1800 |
| `IRel` | 1800 | `UID2` | 1800 | 1800 |
| `IRel` | 1800 | `DefUID` | 1800 | 1800 |
