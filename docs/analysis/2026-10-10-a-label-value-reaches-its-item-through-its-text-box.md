# 标签值顺「文本框 → 依赖 → 物项属性行」挂到物项上：3 954 / 3 978，错挂 0（2026-10-10）

> 承接计划 `docs/plans/2026-10-10-publish-from-the-pid-alone.md` 的 P2 第一步。用户 2026-10-10 在 G1 选「做 P2，下游只要设备 / 仪表 / 管件清单和图上的表示」，随后：「继续：写探针量标签值顺图形链能不能挂到物项上，顺带量仪表分类」。
> 问题：P1（`2026-10-10-publish-from-the-pid-alone.md`）在标签缓存里找到 1 296 个发布值，但说不出它们属于哪个物项。
> 探针 `examples/probe_label_values_reach_their_item.rs`（只读，`src` 不动，不需要 `backup` 特性）。

## 一句话

标签值所在的属性行描述的是一个 `igTextBox`；文本框载荷 `+4` 指向它所属的 `DependencyObject`，依赖的两端 `+16` / `+22` 分别是标签挂着的图形和**被标注物项自己的 `P&IDAttributes` 行**。
顺这三跳，5 张图 3 978 个非空标签值有 3 954 个（99.4 %）挂到物项上，两端从不打架；没挂上的 24 个是图框里的图号和图名。
凡是物项发布了对应属性的，值**全部相等，错挂 0**。这些文本框在顶层空间映射里一条边都没有，所以链只能从载荷读。

## 结构（打分前在一级闪蒸分离器A上读出，写进探针模块注释）

- 每条顶层 `0x0089` 属性行只装一个属性集，载荷 `+12` 是它描述的记录。
- **物项自己的行** `P&IDAttributes`：`ProjectNumber`（`SQPlant`）、`DrawingNo`（图纸 UID）、`DrawingItemType`（`Symbol` / `Connector`）、`DrawingID`（表示 UID）、`ModelItemType`、`ModelID`（物项 UID）、`Flag`。
  同一图形上还挂着 `SymbolAttr`（`Class` / `Type` / `AABB Code`）、`MultiLinearPattern`，符号还有 `Symbol`（`DefinitionName` = 符号路径）。
- **标签**：自己也有 `P&IDAttributes` 行，但 `DrawingItemType = LabelPersist`、`ModelID = 0`，不指名被标注的物项。P1 里「文件指名的 UID 比发布对象多」，多出来的就是这些标签的表示 UID。标签行描述的记录是一个 `DependencyObject`。
- **值行**（`_TagSequenceNo`、`_SmartText`、`_NominalDiameter` …）描述的是 `igTextBox`（一级闪蒸分离器A 605 / 605），这些文本框在顶层空间映射里没有一条边——一开始按空间映射双向走、四跳以内，一个值都没走到物项。
- **载荷里的链**：文本框 `+4` = 所属 `DependencyObject`（即 sheet 解码器的 `parent_ref`）；`DependencyObject` `+16` = 标签挂着的图形（符号或线串），`+22` = 被标注物项的 `P&IDAttributes` 行，`+34` 起是成员（文本框、引线）。
  例：一级闪蒸分离器A 的线号标签（`\Piping\Labels - Piping Segments\SQ Line  Number 01.sym`，`LabelPersist` 行描述依赖 147）：147 `+16` → 2334 `igLineString2d`，`+22` → 3061 `P&IDAttributes`（`Connector` / `PipeRun` / `ModelID 1460257C…`）。

## 口径（打分前写死在探针注释里）

- **归属**：`+22` 是物项的 `P&IDAttributes` 行（不是 `LabelPersist`，`ModelID` 不是 `0`）就归它；再用 `+16` 图形所属的物项交叉核。两端同一物项、或只有一端认得出，算挂上；两端两个物项算「打架」；链上任一跳的记录族不对算「断」。
- **对错**：物项的发布块里有这个值（任一属性）算对；物项发布了但没有这个值算错；物项不在 `_Data.xml` 算未发布。空白值不计。
- **对照**（只用于打分）：按 P1 看到的「标签属性 → 发布属性」对照，物项发布了对应属性时，值相等 / 不等 / 物项没发布这一属性。

## 结果（5 图）

| 图 | 非空标签值 | 挂上 | 断 |
|---|---:|---:|---:|
| I井场 注采阀组 | 1 788 | 1 788 | 0 |
| 一级闪蒸分离器A | 556 | 550 | 6 |
| 一级闪蒸分离器B | 406 | 400 | 6 |
| 低温分离器A | 506 | 500 | 6 |
| 换热器CD及外输阀组 | 722 | 716 | 6 |
| **合计** | **3 978** | **3 954（99.4 %）** | **24** |

- 两端打架 0。断的 24 个都是「依赖的两端都不是物项」：图框里的 `DrawingNumber` 16 个、`Name` 8 个。
- 管段（`PipeRun`）的 1 462 个值只认出 `+22`：标签挂在线串上，而管段的物项行描述的是管段的 `DependencyObject`，不是线串。其余类型（管件 1 104、仪表 1 044、OPC 232、注释 56、设备 36）两端一致。

**对照的标签属性**（挂上的值 vs 物项自己发布的那一属性）：

| 标签属性 | 发布属性 | 相等 | 不等 | 物项没发布这一属性 |
|---|---|---:|---:|---:|
| `TagSequenceNo` | `InstrTagSequenceNo` / `PipingConnectorSeqNo` / `EquipSeqNo` | 734 | 0 | 304（管件：只发布拼好的 `Name`） |
| `MeasuredVariableCode` | `MeasuredVariable` | 438 | 0 | 0 |
| `PipingMaterialsClass` | `PipingMaterialsClass` | 366 | 0 | 0 |
| `InsulationSpec` | `InsulSpec` | 158 | 0 | 0 |
| `Description` | `Description` | 60 | 0 | 0 |
| `ToFromText` | `ToFromText` | 60 | 0 | 0 |
| `Note.Body` | `NoteText` | 56 | 0 | 0 |
| `TagPrefix` | `EquipPrefix` | 30 | 0 | 0 |
| `NominalDiameter` | `NominalDiameter` | 0 | 636 | 0 |

- 公称直径 636 个全不等是写法不同：标签是 `2"`、`1"`、`2-1/2"`、`200`、`200 MM`、`25 MM`，发布是 `2 in`、`1 in`、`200 mm`、`25 mm`。
- 不在对照里的标签属性也挂上了，但发布块里没有同值的属性：介质代号 `OperFluidCode` 276、流体系统 `FluidSystem` 100 发布成代码表引用；管件代号 `PipingComponentCode` 304 只出现在拼好的名称里；`OpeningAction` 212、报警值 96、`SP_SetPressure` 8 不发布。
- 「未发布」112 个都是那 28 个不在 `_Data.xml` 里的 OPC 物项的描述和去向（各 56）。

**归属后能给出的发布值**：

| 发布属性 | 由挂上的标签值给出 | 发布非空 |
|---|---:|---:|
| `INamedInstrument.InstrTagSequenceNo` | 219 | 236 |
| `INamedInstrument.MeasuredVariable` | 219 | 236 |
| `IPipingSpecifiedItem.PipingMaterialsClass` | 115 | 282 |
| `INamedPipingConnector.PipingConnectorSeqNo` | 96 | 219 |
| `IInsulatedItem.InsulSpec` | 57 | 201 |
| `IObject.Description` | 30 | 63 |
| `IOffpageConnector.ToFromText` | 30 | 58 |
| `INote.NoteText` | 28 | 28 |
| `INamedEquipment.EquipPrefix` | 15 | 18 |
| `INamedEquipment.EquipSeqNo` | 13 | 13 |
| **全部非空发布值（`GraphicOID` 另算）** | **822** | **11 671** |

P1 的「在标签缓存里」1 296 只要求值在文件某处；按物项归属后是 822。差的主要是相同取值的共享：等级 `B2` 在很多管段上一样，P1 只要有一个标签写着 `B2` 就算，这里要求那条管段自己有标签。没有标签的物项（大多数管段、管口），这些值不在文件里。

## 仪表一对三

| 发布类 | `SymbolAttr` `Type` | `SymbolAttr` `Class` | 个 |
|---|---|---|---:|
| PIDControlSystemFunction | `System functions` | `Instrument off-line component` | 88 |
| PIDInstrument | `Off-line instruments` | `Instrument off-line component` | 116 |
| PIDInstrument | （没有 `SymbolAttr`） | | 11 |
| PIDInlineInstrument | `Other In-Line Component` / `Other In-Line Instrument(s)` / `Relief Devices` | `Instrument In-Line Component` | 15 |
| PIDInlineInstrument | （空，或没有 `SymbolAttr`） | | 57 |

有 `SymbolAttr` 的 219 个按 `Type` / `Class` 一一对应：`System functions` → 控制功能，`Off-line instruments` → 仪表，`Instrument In-Line Component` → 在线仪表。没有的 68 个（在线 57、仪表 11）要另找判据（候选是 `Symbol` 行的符号路径），没查。

## 名称（只对参考 XML 核了规则，未从 `.pid` 拼）

- 仪表：发布了 `Name` 的 236 个全部等于 `{MeasuredVariable}-{InstrTagSequenceNo}`（`TT-0112101A`），两段都能按上面的链精确归属。
- 设备：发布了 `Name` 的 13 个（容器 5、工艺设备 5、机械 2、换热 1）全部等于 `{EquipPrefix}-{EquipSeqNo}`（`V-01121A`）。
- 管件：`GLV0112103A` 像是 `{PipingComponentCode}{TagSequenceNo}`，两段都挂得上，但代号不发布，规则没法从参考 XML 单独核。
- 这些规则是从答案键看出来的，按计划 Q2 (a) 不能直接用，要你拍板。线号标签的 `FormatString` 自己写着拼法和分隔符（`<F>NominalDiameter=50</F>-<F>OperFluidCode=CI</F>-<F>TagSequenceNo=081001</F>-<F>PipingMaterialsClass=B5</F>-<F>InsulationSpec= </F>`），是管线名规则的文件内证据，但公称直径写法、介质代号都与发布不同，不在本单范围。

## 对 P2 的含义

**有逐条出处、可直接导出**：对象 UID 与类（`ModelItemType`；仪表另按 `SymbolAttr` 分出 219 / 287）；表示 UID 与 `GraphicOID`；
`DwgRepresentationComposition`（图纸 → 表示，`DrawingNo` + `DrawingID`）与 `DrawingItems`（表示 → 物项，`DrawingID` + `ModelID`）两类关系逐条写在物项行里；上表 822 个属性值。

**要拍板才能导出**：名称（仪表、设备、管件三条拼法）；公称直径的写法换算；没有 `SymbolAttr` 的 68 个仪表的类。

## 没查的

- 没有 `SymbolAttr` 的 68 个仪表怎么分类。
- 管件名的拼法；线号标签 `FormatString` 能否当管线名规则的证据。
- `DependencyObject` 的 `+16` / `+22` 在标签以外的依赖上是不是同样的含义（本页只用到标签）。

## 复现

```powershell
cargo run --example probe_label_values_reach_their_item
```

- 库是 pid-parse `054fc4e`（探针随本页一起入库，`src` 不动）；语料 `D:\work\cad\PID自研软件验证`，`PID_PARSE_SELF_CHECK_DIR` 可改，目录不在就打印 `skip:`。
- 2026-10-10 实跑：debug 构建，5 张图约 1.4 s，输出 80 行，见附录。名称规则那一节是用 PowerShell 直接在 5 份参考 XML 上核的，不在探针输出里。

<details><summary>附录：探针完整输出（2026-10-10）</summary>

```text
corpus: D:\work\cad\PID自研软件验证 (5 drawings)
== I井场 注采阀组工艺及自控流程图
  5859 attribute rows, 2064 item graphics, 1465 value rows; values 1788: reached 1788 (right 884, wrong 900, unpublished 4), disagree 0, broken 0
== 集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图
  1498 attribute rows, 425 item graphics, 437 value rows; values 556: reached 550 (right 238, wrong 268, unpublished 44), disagree 0, broken 6
== 集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图
  1150 attribute rows, 340 item graphics, 333 value rows; values 406: reached 400 (right 186, wrong 194, unpublished 20), disagree 0, broken 6
== 集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图
  1456 attribute rows, 444 item graphics, 423 value rows; values 506: reached 500 (right 254, wrong 230, unpublished 16), disagree 0, broken 6
== 集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图
  2349 attribute rows, 880 item graphics, 610 value rows; values 722: reached 716 (right 340, wrong 348, unpublished 28), disagree 0, broken 6
== all 5 drawings
  label property                   values reached disagree broken  right  wrong  unpub
  AlarmHigh                            40      40        0      0      0     40      0
  AlarmHighHigh                        12      12        0      0      0     12      0
  AlarmLow                             40      40        0      0      0     40      0
  AlarmLowLow                           4       4        0      0      0      4      0
  Description                         116     116        0      0     60      0     56
  DrawingNumber                        16       0        0     16      0      0      0
  FluidSystem                         100     100        0      0      0    100      0
  InsulationSpec                      158     158        0      0    158      0      0
  MeasuredVariableCode                438     438        0      0    438      0      0
  Name                                  8       0        0      8      0      0      0
  NominalDiameter                     636     636        0      0      0    636      0
  Note.Body                            56      56        0      0     56      0      0
  OpeningAction                       212     212        0      0      0    212      0
  OperFluidCode                       276     276        0      0      0    276      0
  PipingComponentCode                 304     304        0      0      0    304      0
  PipingMaterialsClass                366     366        0      0    366      0      0
  PipingPoint1.NominalDiameter          2       2        0      0      0      2      0
  PipingPoint2.NominalDiameter          2       2        0      0      0      2      0
  SP_SetPressure                        8       8        0      0      0      8      0
  TagPrefix                            30      30        0      0     30      0      0
  TagSequenceNo                      1038    1038        0      0    734    304      0
  ToFromText                          116     116        0      0     60      0     56
  (all)                              3978    3954        0     24   1902   1940    112
  attributed values by the ends that named the item (values, right)
        24     24  +22 and +16 agree    EquipmentOther
         4      4  +22 and +16 agree    Exchanger
      1044    876  +22 and +16 agree    Instrument
        56     56  +22 and +16 agree    ItemNote
         8      8  +22 and +16 agree    Mechanical
       232    120  +22 and +16 agree    OPC
      1104      0  +22 and +16 agree    PipingComp
        20     20  +22 and +16 agree    Vessel
      1462    794  +22 only             PipeRun
  mapped label properties on the item reached (equal, different, item has none)
    Description                          60      0      0
    InsulationSpec                      158      0      0
    MeasuredVariableCode                438      0      0
    NominalDiameter                       0    636      0
    Note.Body                            56      0      0
    PipingMaterialsClass                366      0      0
    TagPrefix                            30      0      0
    TagSequenceNo                       734      0    304
    ToFromText                           60      0      0
  broken chains (values)
        24  neither end of the dependency is an item
  published values a label gives (attributes with any)
    INamedInstrument.InstrTagSequenceNo                     219 / 236   
    INamedInstrument.MeasuredVariable                       219 / 236   
    IPipingSpecifiedItem.PipingMaterialsClass               115 / 282   
    INamedPipingConnector.PipingConnectorSeqNo               96 / 219   
    IInsulatedItem.InsulSpec                                 57 / 201   
    IObject.Description                                      30 / 63    
    IOffpageConnector.ToFromText                             30 / 58    
    INote.NoteText                                           28 / 28    
    INamedEquipment.EquipPrefix                              15 / 18    
    INamedEquipment.EquipSeqNo                               13 / 13    
    (all non-empty published values, GraphicOID apart)      822 / 11671 
  instruments: published class x SymbolAttr Type / Class
       88  PIDControlSystemFunction   System functions                 Instrument off-line component
        1  PIDInlineInstrument                                         
       56  PIDInlineInstrument        (no SymbolAttr)                  
        1  PIDInlineInstrument        Other In-Line Component          Instrument In-Line Component
        4  PIDInlineInstrument        Other In-Line Instrument         Instrument In-Line Component
        4  PIDInlineInstrument        Other In-Line Instruments        Instrument In-Line Component
        6  PIDInlineInstrument        Relief Devices                   Instrument In-Line Component
       11  PIDInstrument              (no SymbolAttr)                  
      116  PIDInstrument              Off-line instruments             Instrument off-line component
```

</details>
