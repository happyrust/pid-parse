# N2c 缺口：管路侧 16 个参考非空、map 接口列没有的接口

> 任务 t-993。只读已验收的对表页、两份参考实测和导出器底稿，不改它们。
> 名单只取 `docs/analysis/2026-10-11-the-map-schema-says-which-interfaces-a-class-publishes.md`「参考有而 map 没有」里「非空的那些」表的管路行。空壳接口不算。
> 五图非空和与那张表相同，没有重算后改数。属性名是两份实测里该接口「非空次数」大于 0 的属性，五图相加写在名字后面；加总应等于五图非空和。
> 导出器只看 `docs/analysis/n2c-exporter-interface-inventory.md`「会发出去的类」和「这条路不发」。
> **本轮未改任何代码、注释、测试、对表、实测、导出器底稿、map 全表或计划，也没有提交。** 测试 NOT RUN：只新建本文件。

对表「非空的那些」里，管路这五个类一共 16 行。`PIDPipingConnector` 的 `IObject` 在 map 接口列里（对表类表第 39 行 `PipeRun`），不在这 16 行里。

| 类 | 接口 | 五图非空和 | 参考里非空的属性 | 导出器今天发不发 |
|---|---|---:|---|---|
| `PIDPipingConnector` | `IConnector` | 444 | `FlowDirection`（103）、`RepresentationsAreAllZeroLength`（341） | 不发。底稿该类行只有 `IObject`、`INamedPipingConnector` 和无类门的 `IPipingSpecifiedItem`、`IInsulatedItem`、`IOffpageConnector`、`INote` |
| `PIDPipingConnector` | `IInsulatedItem` | 106 | `InsulSpec`（106） | 发。无类门，`InsulationSpec` → `IInsulatedItem` |
| `PIDPipingConnector` | `INamedPipingConnector` | 219 | `PipingConnectorSeqNo`（219） | 发。`TagSequenceNo` 在该类上的门 |
| `PIDPipingConnector` | `IPBSItem` | 682 | `ConstructionStatus`（341）、`ConstructionStatus2`（341） | 不发。该类行没有这个接口 |
| `PIDPipingConnector` | `IPIDTypical` | 341 | `IsTypical`（341） | 不发。该类行没有这个接口 |
| `PIDPipingConnector` | `IPipeCrossSectionItem` | 200 | `NominalDiameter`（200） | 不发。该类行没有这个接口 |
| `PIDPipingConnector` | `IPipingConnector` | 341 | `PipingConnectorType`（341） | 不发。该类行没有这个接口 |
| `PIDPipingConnector` | `IPipingSpecifiedItem` | 261 | `PipingMaterialsClass`（261） | 发。无类门，`PipingMaterialsClass` → `IPipingSpecifiedItem` |
| `PIDPipingComponent` | `IInlineComponent` | 264 | `IsFlowDirectional`（264） | 不发。该类与 `PIDNozzle` 同一名单：`IObject` 加无类门四条，没有 `IInlineComponent` |
| `PIDPipingComponent` | `IPIDTypical` | 264 | `IsTypical`（264） | 不发。同上，名单里没有 `IPIDTypical` |
| `PIDPipingPort` | `IConnection` | 298 | `ConnectionFlowDirection`（298） | 不发。底稿「这条路不发」写明端口不在常量表里，没有这个类 |
| `PIDPipingPort` | `IObject` | 1436 | `Name`（718）、`UID`（718） | 不发。同上，类本身不发 |
| `PIDPipingPort` | `IPipeCrossSectionItem` | 444 | `NominalDiameter`（444） | 不发。同上，类本身不发 |
| `PIDPipeline` | `IFluidSystem` | 149 | `FluidCode`（4）、`FluidSystem`（145） | 不发。底稿「这条路不发」写明管线不在常量表里，没有这个类 |
| `PIDPipeline` | `IObject` | 344 | `Name`（172）、`UID`（172） | 不发。同上，类本身不发 |
| `PIDPipingBranchPoint` | `IObject` | 183 | `UID`（183） | 不发。底稿「这条路不发」写明分支点不在常量表里，没有这个类 |

16 行里导出器会发的是 3 个，都在 `PIDPipingConnector`：`IInsulatedItem`、`INamedPipingConnector`、`IPipingSpecifiedItem`。其余 13 个参考里非空，今天这条路不发。

## 核对

- 五图非空和：打开对表页「非空的那些」，搜本表的类和接口，数字应相同。
- 属性名：打开 `docs/analysis/n2c-reference-measured-ijingchang.md` 和 `docs/analysis/n2c-reference-measured-other-four.md` 里该类的节，把该接口非空次数大于 0 的属性五图相加。
- 发不发：打开 `docs/analysis/n2c-exporter-interface-inventory.md`「会发出去的类」。管段、管件在表里；端口、管线、分支点在「这条路不发」。
