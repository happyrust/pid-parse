# N2c 缺口：仪表和信号 12 个接口

> 任务 t-994。只新建本文件。只读这四份，不改它们：
>
> - `docs/analysis/2026-10-11-the-map-schema-says-which-interfaces-a-class-publishes.md`（t-913 已验收的对表）
> - `docs/analysis/n2c-reference-measured-ijingchang.md`
> - `docs/analysis/n2c-reference-measured-other-four.md`
> - `docs/analysis/n2c-exporter-interface-inventory.md`
>
> 本轮未改 `src`、注释、测试，也没有提交。测试 NOT RUN：本单只新建这一页。

## 口径

这 12 行就是对表「参考有而 map 没有 · 接口」里「非空的那些」中、仪表和信号侧的全部行（对表该表的 `PIDInlineInstrument` 两行、`PIDInstrument` 五行、`PIDProcessPoint` 一行、`PIDSignalBranchPoint` 一行、`PIDSignalConnector` 两行、`PIDSignalPort` 一行）。对表把空壳（裸接口或属性全空，五图非空和为 0）另表列出；那些不进本页。

- **五图非空和**：两份实测底稿里，该类该接口下每个属性的「非空次数」五张图相加。与对表同一格逐格相等，下面把五张图的加数写在格里。
- **参考里非空的属性名**：五张图里该接口「非空次数」大于 0 的属性名，按 I井场表里先出现的顺序。非空次数为 0 的属性名不写。
- **导出器今天发不发**：看导出器底稿「会发出去的类」那张表的「接口」格。类名不在那张表里、底稿写明这条路不发的，记不发。

`PIDProcessPoint` 在 map 类表里没有发布类：对表「参考有而 map 没有 · 类」写 31 个 `SPMapClassDef` 里没有它；`docs/analysis/n2c-map-schema-table.md` 的类表没有 `PIDProcessPoint` 这一行（属性全表里有 `PIDProcessPoint.*` 的 pair，那是另一张表）。

## 12 行

| 类 | 接口 | 五图非空和 | 参考里非空的属性名 | 导出器今天发不发这个接口 |
|---|---|---|---|---|
| `PIDInstrument` | `IInstrumentOcc` | 116（与对表一致。I井场 58 + 分离器A 9 + 分离器B 9 + 低温分离器A 14 + 换热器CD 26） | `InstrumentLocation` | 不发。底稿 `PIDInstrument` 行的接口是 `IObject`、`INamedInstrument`，外加无类门四条 `IPipingSpecifiedItem`、`IInsulatedItem`、`IOffpageConnector`、`INote`，没有 `IInstrumentOcc` |
| `PIDInstrument` | `INamedInstrument` | 236（与对表一致。I井场 116 + 分离器A 18 + 分离器B 18 + 低温分离器A 28 + 换热器CD 56） | `InstrTagSequenceNo`、`MeasuredVariable` | 发。底稿 `PIDInstrument` 行写了 `INamedInstrument`；`published_attribute` 里 `MeasuredVariableCode`、`TagSequenceNo` 的类门是 `INSTRUMENT_CLASSES`，落到这两个属性名 |
| `PIDInstrument` | `IObject` | 245（与对表一致。I井场 118 + 分离器A 19 + 分离器B 19 + 低温分离器A 29 + 换热器CD 60） | `UID`、`Name` | 发。底稿该行写 `IObject` 必发；`Name` 由 `composed_name` 在 `MeasuredVariable` 与 `InstrTagSequenceNo` 都在时拼 |
| `PIDInstrument` | `IPBSItem` | 254（与对表一致。I井场 120 + 分离器A 20 + 分离器B 20 + 低温分离器A 30 + 换热器CD 64） | `ConstructionStatus`、`ConstructionStatus2` | 不发。底稿 `PIDInstrument` 的接口格里没有 `IPBSItem` |
| `PIDInstrument` | `IPIDTypical` | 127（与对表一致。I井场 60 + 分离器A 10 + 分离器B 10 + 低温分离器A 15 + 换热器CD 32） | `IsTypical` | 不发。底稿 `PIDInstrument` 的接口格里没有 `IPIDTypical` |
| `PIDInlineInstrument` | `IInlineComponent` | 72（与对表一致。I井场 40 + 分离器A 2 + 分离器B 2 + 低温分离器A 5 + 换热器CD 23） | `IsFlowDirectional` | 不发。底稿 `PIDInlineInstrument` 行写「同 `PIDInstrument`」，那一格没有 `IInlineComponent` |
| `PIDInlineInstrument` | `IPIDTypical` | 72（与对表一致。I井场 40 + 分离器A 2 + 分离器B 2 + 低温分离器A 5 + 换热器CD 23） | `IsTypical` | 不发。同上，接口格里没有 `IPIDTypical` |
| `PIDProcessPoint` | `IObject` | 341（与对表一致。I井场 169 + 分离器A 39 + 分离器B 29 + 低温分离器A 41 + 换热器CD 63） | `UID` | 不发。map 类表没有发布类 `PIDProcessPoint`（见上）。导出器底稿「这条路不发、也不在本常量表里的类」写工艺点不在 `CLASS_OF_ITEM_TYPE` 里，`pid_writer` 没有对应元素，所以 `IObject` 也不发 |
| `PIDSignalConnector` | `IObject` | 83（与对表一致。I井场 46 + 分离器A 6 + 分离器B 6 + 低温分离器A 9 + 换热器CD 16） | `UID` | 发。底稿该行写「同 `PIDNozzle`」：`IObject` 必发，无拼名 |
| `PIDSignalConnector` | `IPIDTypical` | 83（与对表一致。I井场 46 + 分离器A 6 + 分离器B 6 + 低温分离器A 9 + 换热器CD 16） | `IsTypical` | 不发。`PIDNozzle` 那一格是 `IObject` 加上无类门四条，没有 `IPIDTypical` |
| `PIDSignalBranchPoint` | `IObject` | 16（与对表一致。I井场 10 + 分离器A 1 + 分离器B 1 + 低温分离器A 1 + 换热器CD 3） | `UID` | 不发。底稿「这条路不发、也不在本常量表里的类」写分支点不在 `CLASS_OF_ITEM_TYPE` 里，`pid_writer` 没有对应元素。map 类表有这个发布类，接口列是「—」（对表第 47 行）；不发的是导出器，不是类表缺这个类 |
| `PIDSignalPort` | `IObject` | 3450（与对表一致。I井场 1830 + 分离器A 214 + 分离器B 214 + 低温分离器A 394 + 换热器CD 798） | `UID`、`Name` | 不发。底稿同一节写端口不在 `CLASS_OF_ITEM_TYPE` 里，`pid_writer` 没有对应元素。map 类表有 `PIDSignalPort`，接口列是「—」（对表第 45 行） |

12 个接口里，导出器今天发的是 3 个：`PIDInstrument` 的 `INamedInstrument` 和 `IObject`，`PIDSignalConnector` 的 `IObject`。另外 9 个不发。
