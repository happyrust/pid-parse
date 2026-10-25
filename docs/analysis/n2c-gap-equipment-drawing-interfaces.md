# N2c 缺口：设备和图纸 14 个接口

> 任务 t-995。只读四份已有底稿，不改它们：
>
> - 对表 `docs/analysis/2026-10-11-the-map-schema-says-which-interfaces-a-class-publishes.md`（t-913）
> - `docs/analysis/n2c-reference-measured-ijingchang.md`
> - `docs/analysis/n2c-reference-measured-other-four.md`
> - `docs/analysis/n2c-exporter-interface-inventory.md`
>
> 范围是对表「参考有而 map 没有 / 接口（211）」里、设备和图纸侧那 14 个非空接口。空壳接口不收。设备类只取 `IPIDTypical`；管嘴另取 `IPipingSpecifiedItem`。
> **本轮未改任何 `src`、注释、测试，也没有改那四份底稿。没有提交。** 测试 NOT RUN：允许新建的只有本文件。

## 口径

五图非空和取对表「非空的那些」那一表的数字。复核是把两份实测里该接口下每个属性的「非空次数」相加。14 行都与对表相等，所以下面不另写重算数。

「参考里非空的属性名」只列非空次数大于 0 的属性。同一接口上非空次数为 0 的属性不算，例如 `PIDDrawing` / `IDocument` 还有 `DocTitle`、`DocSubtype`，`PIDDrawing` / `IObject` 还有 `Description`。括号里的数是该属性五图非空次数，加起来等于这一行的五图非空和。

「导出器今天发不发」只看导出器底稿「会发出去的类」和「表里有名字、今天不发的类」，不看 map。

## 14 行

| 类 | 接口 | 五图非空和 | 参考里非空的属性名 | 导出器今天发不发 |
|---|---|---:|---|---|
| `PIDHeatTransferEquipment` | `IPIDTypical` | 1，与对表一致 | `IsTypical`（1） | 不发 |
| `PIDMechanicalEquipment` | `IPIDTypical` | 4，与对表一致 | `IsTypical`（4） | 不发 |
| `PIDProcessEquipment` | `IPIDTypical` | 13，与对表一致 | `IsTypical`（13） | 不发 |
| `PIDProcessVessel` | `IPIDTypical` | 5，与对表一致 | `IsTypical`（5） | 不发 |
| `PIDNozzle` | `IPIDTypical` | 36，与对表一致 | `IsTypical`（36） | 不发 |
| `PIDNozzle` | `IPipingSpecifiedItem` | 21，与对表一致 | `PipingMaterialsClass`（21） | 发 |
| `PIDDrawing` | `IDocument` | 10，与对表一致 | `DocCategory`（5）、`DocType`（5） | 不发 |
| `PIDDrawing` | `IObject` | 10，与对表一致 | `UID`（5）、`Name`（5） | 发 |
| `PIDNote` | `INote` | 28，与对表一致 | `NoteText`（28） | 发 |
| `PIDNote` | `IObject` | 56，与对表一致 | `UID`（56） | 发 |
| `PIDRepresentation` | `IDrawingRepresentation` | 1823，与对表一致 | `GraphicOID`（1823） | 发 |
| `PIDRepresentation` | `IObject` | 1823，与对表一致 | `UID`（1823） | 发 |
| `PIDBranchPoint` | `IObject` | 174，与对表一致 | `UID`（58）、`Name`（58）、`Description`（58） | 不发 |
| `PIDBranchPoint` | `IOffpageConnector` | 58，与对表一致 | `ToFromText`（58） | 不发 |

14 行里导出器已发 6 行、未发 8 行。已发：管嘴的 `IPipingSpecifiedItem`，图纸的 `IObject`，注释的 `INote` 和 `IObject`，表示的 `IDrawingRepresentation` 和 `IObject`。未发：四个设备类的 `IPIDTypical`，管嘴的 `IPIDTypical`，图纸的 `IDocument`，分支点的 `IObject` 和 `IOffpageConnector`。

角色是工程，任务书只许新建本文件、不改 `src`。按任务书做，没有改代码。

## 导出器底稿怎么对上「发 / 不发」

四个设备类在底稿里写成同一套接口：`IObject`、`INamedEquipment`，外加无类门的 `IPipingSpecifiedItem`、`IInsulatedItem`、`IOffpageConnector`、`INote`。`PIDHeatTransferEquipment`、`PIDMechanicalEquipment`、`PIDProcessEquipment` 都写「同 `PIDProcessVessel`」。这套名单里没有 `IPIDTypical`，所以那四行是不发。

`PIDNozzle` 的专有接口只有必发的 `IObject`，另加上面那四条无类门接口。`IPIDTypical` 不在里面，不发。`IPipingSpecifiedItem` 在无类门四条里，发。

`PIDDrawing` 底稿写明只有 `IObject`（`UID`、`Name`），「没有第二个接口」。所以 `IDocument` 不发，`IObject` 发。

`PIDNote` 写「同 `PIDNozzle`」，并写明 `Note.Body` → `INote` 没有「必须是注释类」的门。所以 `INote` 发，`IObject` 发。

`PIDRepresentation` 底稿写 `IObject`（只有 `UID`）和 `IDrawingRepresentation`（`GraphicOID`）。两行都发。

`PIDBranchPoint` 在「表里有名字、今天不发的类」：`OPC` 被判成无类，物项不进 `write_item`。无类门的 `IOffpageConnector` 只加在已定类的物项上，套不到这个类。所以 `IObject` 和 `IOffpageConnector` 都不发。

## 核对

在对表「非空的那些」里搜上表 14 个「类 + 接口」，五图非空和应与本表相同。在两份实测里搜同一个接口，把非空次数大于 0 的属性名拼起来，应与「参考里非空的属性名」相同，次数相加应等于五图非空和。发或不发按上一节的底稿句子，不打开 `src`。
