# 只凭 `.pid` 能给回多少 publish：对象身份一半、表示全部、关系自己的 UID 一个没有（2026-10-10）

> 承接计划 `docs/plans/2026-10-10-publish-from-the-pid-alone.md` P0–P1（用户 2026-10-10：「先做只凭 .pid 的取证探针：5 张图逐张统计能复原参考 _Data.xml 的多少，写一页分析」）。
> 问题：没有工厂库时，`D:\work\cad\PID自研软件验证` 的 5 张图各自能从 `.pid` 里复原参考 `_Data.xml` / `_Meta.xml` 的多少？答案决定 publish 导出器走「只凭 `.pid`」还是「走库」（手头的 SQPlant 备份只有 I井场）。
> 探针 `examples/probe_publish_from_the_pid_alone.rs`（只读，`src` 不动，不需要 `backup` 特性）。

## 一句话

5 张图 6 131 个发布对象里，2 948 个（48.1 %）的 UID 在 `.pid` 里，而且按类非黑即白：表示、管段、管件、仪表、信号线、注释、管口、设备等 14 类全在；
信号端口、管道端口、工艺点、管道分支点、管线、信号分支点 6 类（3 155 个）连原始字节都搜不到。1 823 个表示全部连上，且全落在发布的 `GraphicOID`。
8 185 条关系里两端都在文件里的 3 491 条（42.7 %），**关系自己的 UID 一条都不在**。11 671 个非空属性值只有 1 296 个（11.1 %）出现在标签缓存里；
代码表属性（发布成 `@NewConstruction` / `@EE690` / `@{GUID}` 一类引用）和拼接出来的名称一个都不在。
只凭 `.pid` 的导出器做得出「一半对象 + 全部表示 + 文档身份」，关系和属性还差逐条归属的证据，管线与端口这一层整类缺席。

## 口径（打分前写死在探针的模块注释里）

- **答案键**：每份 `_Data.xml` 的 `<Container>` 下每个顶层块——对象（元素名是类，`IObject/@UID` 是键，其余每个接口属性是一个值，`GraphicOID` 另算）或 `<Rel>`（`IObject/@UID`、`IRel/@UID1`、`@UID2`、`@DefUID`）。`_Meta.xml` 太小，逐项定位，不计数。
- **四种读法**：
  - **Raw**：值的 UTF-8 / UTF-16LE / GBK 字节出现在某条流里。是上界，说不出属于哪个对象；短于 4 字节的值不扫，单列。
  - **Fields**：逐流扫 `u16 tag, u16 len` 框起的属性集字段（`0x8010` 字符串、`0x0003` 整数、`0x000B` 布尔、`0x0000` 集头），长度字须正好框住字段。
    「在标签缓存里」= 等于某个 `ItemValue` 字段或某个 `<F>name=value</F>` 的值（不限长度）；「在字段里」= 至少 4 字节且等于某个 `0x8010` 字段的值。
  - **Named**：`P&IDAttributes` 的 `DrawingNo` / `DrawingID` / `ModelID` 字段持有该 UID。
  - **GraphicOID**：`PidSemanticIndex::load_beside`（10-08 D2 的表示 UID 连）。
- 上一会话写的探针原样接手；本单只加了两处输出（`_Meta.xml` 逐项定位，「类 × 属性」表），并修了 `_Meta.xml` 定位的一处误报（有些 `ModelID` 字段存的是 `0`，短值不再当名字查）。四种读法没动。

## 逐图

| 图 | `.pid` | 流 | 框住的字段 / 集 | 对象：UID 在文件里 | 非空属性值：在标签缓存 | 关系：两端都在 | 表示：连上 / 落在 `GraphicOID` |
|---|---:|---:|---:|---:|---:|---:|---:|
| I井场 注采阀组 | 4.0 MB | 111 | 29 990 / 6 258 | 1 460 / 3 093（47.2 %） | 616 / 5 800（10.6 %） | 1 710 / 4 107（41.6 %） | 902 / 902 |
| 一级闪蒸分离器A | 1.1 MB | 79 | 7 906 / 1 726 | 298 / 570（52.3 %） | 158 / 1 167（13.5 %） | 354 / 767（46.2 %） | 188 / 188 |
| 一级闪蒸分离器B | 0.9 MB | 78 | 6 166 / 1 373 | 247 / 470（52.6 %） | 121 / 939（12.9 %） | 295 / 627（47.0 %） | 151 / 151 |
| 低温分离器A | 1.2 MB | 84 | 7 762 / 1 731 | 325 / 679（47.9 %） | 165 / 1 344（12.3 %） | 382 / 884（43.2 %） | 193 / 193 |
| 换热器CD及外输阀组 | 1.7 MB | 87 | 12 769 / 2 655 | 618 / 1 319（46.9 %） | 236 / 2 421（9.7 %） | 750 / 1 800（41.7 %） | 389 / 389 |
| **合计** | | | | **2 948 / 6 131（48.1 %）** | **1 296 / 11 671（11.1 %）** | **3 491 / 8 185（42.7 %）** | **1 823 / 1 823** |

五张图的比例几乎一样，没有哪张特殊。每一类、每一张图里「UID 原样在」都等于「被指名」：没有哪个发布 UID 只以原始字节出现、却没有 `P&IDAttributes` 字段指名它。

## 对象（5 图合计）

| 类 | 对象 | UID 在文件里 | 文件里记作（`ModelItemType` / `DrawingItemType`） |
|---|---:|---:|---|
| PIDRepresentation | 1 823 | 1 823 | 随所表示的物项：PipeRun 975、Instrument 287、PipingComp 264、SignalRun 124、OPC 58、ItemNote 56、Nozzle 36、EquipmentOther 13、Vessel 5、Mechanical 4、Exchanger 1 |
| PIDSignalPort | 1 725 | **0** | — |
| PIDPipingPort | 718 | **0** | — |
| PIDPipingConnector | 341 | 341 | PipeRun |
| PIDProcessPoint | 341 | **0** | — |
| PIDPipingComponent | 264 | 264 | PipingComp |
| PIDPipingBranchPoint | 183 | **0** | — |
| PIDPipeline | 172 | **0** | — |
| PIDInstrument | 127 | 127 | Instrument |
| PIDControlSystemFunction | 88 | 88 | Instrument |
| PIDSignalConnector | 83 | 83 | SignalRun |
| PIDInlineInstrument | 72 | 72 | Instrument |
| PIDBranchPoint | 58 | 30 | OPC |
| PIDNote | 56 | 56 | ItemNote |
| PIDNozzle | 36 | 36 | Nozzle |
| PIDSignalBranchPoint | 16 | **0** | — |
| PIDProcessEquipment | 13 | 13 | EquipmentOther |
| PIDDrawing | 5 | 5 | Drawing（`DrawingNo`） |
| PIDProcessVessel | 5 | 5 | Vessel |
| PIDMechanicalEquipment | 4 | 4 | Mechanical |
| PIDHeatTransferEquipment | 1 | 1 | Exchanger |

- 类由 `ModelItemType` 一对一定，只有 `Instrument` 一对三（PIDInstrument / PIDControlSystemFunction / PIDInlineInstrument），要另找判据。
- 这批图的 `PIDBranchPoint` 都是 OPC（58 个都带 `ToFromText`，58 个表示都记作 OPC），物项只有 30 个在文件里。
- 文件里被指名的 UID 比发布对象多（I井场 3 089 个，对得上发布对象的 1 460 个），多出来的是什么没查。

## 关系（按 `DefUID`，5 图合计）

| `DefUID` | 关系 | 两端都在 | 一端在 |
|---|---:|---:|---:|
| DrawingItems | 1 823 | 1 596 | 227 |
| DwgRepresentationComposition | 1 823 | 1 823 | 0 |
| SignalPortComposition | 1 725 | 0 | 1 725 |
| PipingTapOrFitting | 738 | 0 | 738 |
| PipingPortComposition | 718 | 0 | 718 |
| ProcessPointCollection | 341 | 0 | 341 |
| PipingEnd1Conn | 283 | 9 | 274 |
| PipingEnd2Conn | 279 | 20 | 259 |
| PipingConnectors | 219 | 0 | 219 |
| SignalEnd2Conn | 83 | 0 | 83 |
| SignalEnd1Conn | 75 | 0 | 75 |
| EquipmentComponentComposition | 36 | 36 | 0 |
| SignalTapOrFitting | 35 | 0 | 35 |
| PBSItemNotes | 7 | 7 | 0 |
| **合计** | **8 185** | **3 491** | **4 694** |

- 关系自己的 UID：8 185 条里 0 条出现在文件里，原始字节也没有。导出时只能新生成，比对只能按（`UID1`、`UID2`、`DefUID`）配对。
- 「两端都在」只说明两端的对象在文件里，不等于文件写着这条关系。`DwgRepresentationComposition` 最可能就是同一个 `P&IDAttributes` 集里的 `DrawingID` + `ModelID`，但本探针没逐集核。
- 没有一条关系两端都不在。一端在的那些，从 `DefUID` 看都连着端口、工艺点、管线或分支点。

## 属性（5 图合计，`GraphicOID` 另算）

17 291 个属性值，空 5 620，非空 11 671：在标签缓存里 1 296（11.1 %）；至少 4 字节的 8 495 个里，在字段里 651（7.7 %），原始字节 851（10.0 %）。

**找得到的**集中在标签会显示的那几项（这十项占 1 296 里的 1 291）：

| 属性 | 非空 | 在标签缓存 | 缓存它的标签属性 |
|---|---:|---:|---|
| `IPipingSpecifiedItem.PipingMaterialsClass` | 282 | 282 | `PipingMaterialsClass` |
| `INamedInstrument.MeasuredVariable` | 236 | 228 | `MeasuredVariableCode` |
| `INamedInstrument.InstrTagSequenceNo` | 236 | 219 | `TagSequenceNo` |
| `INamedPipingConnector.PipingConnectorSeqNo` | 219 | 186 | `TagSequenceNo` |
| `IInsulatedItem.InsulSpec` | 201 | 201 | `InsulationSpec` |
| `IObject.Description` | 63 | 58 | `Description` |
| `IOffpageConnector.ToFromText` | 58 | 58 | `ToFromText` |
| `INote.NoteText` | 28 | 28 | `Note.Body` |
| `INamedEquipment.EquipPrefix` | 18 | 18 | `TagPrefix` |
| `INamedEquipment.EquipSeqNo` | 13 | 13 | `TagSequenceNo` |

**一个都找不到的**（取值分布另用 PowerShell 数过 5 份参考）：

| 属性 | 非空 | 不同值 | 最常见的值 |
|---|---:|---:|---|
| `IObject.Name` | 3 298 | — | 端口 `1`、`2`…（2 501 个短于 4 字节，不扫）；管线 `65 mm-AI-010901-I2`、管段 `65 mm-AI-010901-I2-65 mm-I2--`、仪表 `TT-0112101A` |
| `IPBSItem.ConstructionStatus` / `ConstructionStatus2` | 951 / 951 | 2 / 2 | `@NewConstruction` 943、`@ExistingConstruction` 8 |
| `IPIDTypical.IsTypical` | 946 | 1 | `False` |
| `IPipeCrossSectionItem.NominalDiameter` | 878 | 26 | `1 in` 145、`0.75 in` 127、`2 in` 107、`25 mm` 61 |
| `IConnector.RepresentationsAreAllZeroLength` | 341 | 2 | `False` 324 |
| `IPipingConnector.PipingConnectorType` | 341 | 5 | `@EE690` 168、`@EE68A` 117 |
| `IInlineComponent.IsFlowDirectional` | 336 | 2 | `False` 318 |
| `IConnection.ConnectionFlowDirection` | 298 | 2 | `@{23F9414C-…}` 153、`@{23F94146-…}` 145 |
| `IPipingComponent.CommoditySpecialtyType` / `PipingComponentType1` / `2` / `3` | 264 / 264 / 262 / 210 | 2 / 4 / 10 / 10 | 全是 `@{GUID}` |
| `IFluidSystem.FluidSystem` | 145 | 6 | 全是 `@{GUID}` |
| `IInstrumentOcc.InstrumentLocation` | 116 | 1 | `@EE31D` |
| `IEquipment.EqType0`–`4`、`EqTypeDescription`；`IEquipmentComponent.ProcessEqCompType1` / `2`；`IInstrument.InstrumentType` / `InstrumentType3` / `ProcFunc` | 7–49 | 1–5 | `@EE…`、`@{GUID}`、`Other equipment` |

- 代码表属性发布的是代码表引用（`@` 开头）；`.pid` 的字段里既没有这些串，也没有以这些属性名命名的字段。
- 公称直径：文件里有 318 个 `_NominalDiameter` 标签集，但 878 个发布值一个也对不上，写法不同（标签里的写法没查）。
- 名称：仪表名 = 测量变量 + `-` + 流水号（`TT-0112101A`），两段都在标签缓存里；管线名 = 公称直径-介质-流水号-等级[-保温]，管段名在管线名后再接几段；管件名如 `GLV0112103A`，152 个里 94 个原样在文件里。

## `_Meta.xml`（5 份结果一样）

| 项 | 在 `.pid` 里 |
|---|---|
| `DocUID` | 在（`P&IDAttributes.DrawingNo`） |
| `DocName` | 在（4 张在标签缓存，I井场只有原始字节） |
| `File` 对象的 Name（`<DocName>.pid`） | 在（原始字节） |
| `VersionedDoc` / `RevisedDocument` 两条关系里 DocUID 那一端 | 在 |
| `DocumentVersion` / `DocumentRevision` / `File` 三个对象的 UID，三条 `Rel` 自己的 UID | 不在 |
| `FilePath`（`\\Glpid-7\pidsq\PID`）、`DocVersionDate`、`SoftwareVersion` `10.00.31.0023`、`SchemaVersion`、`ToolID`、`CompSchema` | 不在 |
| `Plant` `P01`、`DocVersion` `1`、`DocRevision` `0` | 短值，不扫 |

## 三档结论

**能精确给**（值在文件里，对象能定位）
- 2 948 个对象的 UID（14 类全部 + 30 个 OPC）；类由 `ModelItemType` 定，`Instrument` 那 287 个除外（见下一档）。
- 1 823 个表示的 UID 与 `GraphicOID`（100 %）。
- `_Meta.xml` 的 `DocUID`、`DocName`、`.pid` 文件名。
- 有证据后可升入本档：3 491 条两端都在的关系（UID 只能新生成），1 296 个标签缓存里的属性值。两者都还差「文件里确实写着这条关系 / 这个值挂在这个对象上」的逐条证据，P2 第一步先量。

**可能按规则推**（只列候选，本单不定）
- `Instrument` 一对三的类：候选判据是符号路径（09-30 规则表里 `system functions` → PIDControlSystemFunction）和在线标志。
- 名称：仪表 236 个（`{测量变量}-{流水号}`）；管线 172 个、管段 219 个（拼接规则在库里，样本能反推，但那是从答案键学规则）。
- 公称直径 878 个：标签写法与发布写法之间的换算。
- 管件类型 1–3、设备类型 0–4、管口类型：候选来源是符号库路径或符号自带的 `SymbolAttr` 集，没查。
- 只有一两种取值的代码表属性（`IsTypical` 全是 `False`、`InstrumentLocation` 全是 `@EE31D` 等）：填默认值多半会对，但没有逐对象的证据，按计划 Q2 (a) 不发。

**`.pid` 里没有**
- 3 155 个对象：信号端口、管道端口、工艺点、管道分支点、管线、信号分支点（6 类整类），外加 28 个 OPC 物项。
- 8 185 条关系自己的 UID；4 694 条至少一端不在文件里的关系。
- 代码表属性的发布值；`_Meta.xml` 里版本 / 修订 / 文件三个对象与三条关系的 UID、发布日期、`\\Glpid-7` 路径、SmartPlant 版本号。

## 对 G1 的含义

Q2 已按推荐选 (a)（只发找得到出处的）。照这条，只凭 `.pid` 的导出器今天能给：48 % 的对象、全部表示及其 `GraphicOID`、`_Meta.xml` 的文档身份；
关系与属性要等 P2 第一步的逐条证据，上限分别是 43 % 和 11 %。管线、端口、工艺点这一层——承载管线号和连接关系的对象——整类缺席。
若「交服务」要的是完整的 publish，这条路给不了，得走库（手头备份里只有 I井场）；若下游只要设备 / 仪表 / 管件清单及其在图上的表示，这条路够用。由你在 G1 定。

## 与 09-30 那页的关系

09-30（`2026-09-30-item-tags-without-data-xml.md`）量的是图形 → 文字的三条关联（`igTextBox.parent_ref`、`DependencyObject` 编组、符号路径），位号 0 / 16；
这页量的是属性集与标签缓存。09-30 末尾「由属性拼 run 的 label」那条未测线索在这里有了一半答案：拼名称的零件（流水号、等级、保温代号、测量变量）在标签缓存里，拼法不在。两页不冲突。

## 没查的

- 文件里多出来的被指名 UID（I井场 3 089 对 1 460）是什么。
- 标签缓存里的值挂在哪个对象上；两端都在的关系在文件里有没有逐条写着（P2 第一步）。
- 公称直径标签的写法；代码表属性在 `.pid` 里是否以别的形式（如索引）存在。

## 复现

```powershell
cargo run --example probe_publish_from_the_pid_alone
```

- 库是 pid-parse `e0cbd0a`（探针随本页一起入库，`src` 不动）；语料 `D:\work\cad\PID自研软件验证`，`PID_PARSE_SELF_CHECK_DIR` 可改，目录不在就打印 `skip:`。
- 2026-10-10 实跑：debug 构建，5 张图约 40 s，输出 524 行，见附录。

<details><summary>附录：探针完整输出（2026-10-10）</summary>

```text
corpus: D:\work\cad\PID自研软件验证 (5 drawings)
== I井场 注采阀组工艺及自控流程图
  .pid 3997696 B, 111 streams; 29990 framed fields in 6258 sets (6258 with a header); 3089 UIDs named
  objects 3093: named 1460 (47.2 %; DrawingID 902 / DrawingNo 1 / ModelID 557), UID raw 1460
  attribute values 8602 (GraphicOID apart): empty 2802, non-empty 5800: in a label cache 616 (10.6 %); of the 4188 at least 4 bytes long, in the fields 296 (7.1 %), raw 414 (9.9 %)
  rels 4107: both ends named 1710 (41.6 %), one end 2397, own UID raw 0
  representations 902: UID-joined 902, on their published GraphicOID 902
  _Meta.xml Container: CompSchema=DocVersioningComponent [-], Scope=Data [raw], SoftwareVersion=10.00.31.0023 [-], IsValidated=False [-], SchemaVersion=04.02.17.01 [-], Plant=P01 [short], DocUID=5633AF4121EE4F4D9FE78E90722CC050 [named (DrawingNo)], DocName=I井场 注采阀组工艺及自控流程图 [raw], ToolID=SMARTPLANTPID [-], ToolSignature=AAAD [-], SDECIMAL=. [short]
  _Meta.xml DocumentVersion UID [-]: IObject.Name=I井场 注采阀组工艺及自控流程图 Version [-], IDocumentVersion.DocRevision=0 [short], IDocumentVersion.DocVersionDate=2026/06/22 [-], IDocumentVersion.DocVersion=1 [short]
  _Meta.xml DocumentRevision UID [-]: IObject.Name=I井场 注采阀组工艺及自控流程图 Revision [-], IDocumentRevision.MajorRev_ForRevise=0 [short]
  _Meta.xml File UID [-]: IObject.Name=I井场 注采阀组工艺及自控流程图.pid [raw], IFile.FilePath=\\Glpid-7\pidsq\PID [-]
  _Meta.xml Rel VersionedDoc: own UID [-], UID1 [named (DrawingNo)], UID2 [-]
  _Meta.xml Rel RevisedDocument: own UID [-], UID1 [-], UID2 [named (DrawingNo)]
  _Meta.xml Rel FileComposition: own UID [-], UID1 [-], UID2 [-]
  class                        objects  named    raw  named as
  PIDBranchPoint                     2      1      1  OPC 1
  PIDControlSystemFunction          50     50     50  Instrument 50
  PIDDrawing                         1      1      1  Drawing 1
  PIDInlineInstrument               40     40     40  Instrument 40
  PIDInstrument                     60     60     60  Instrument 60
  PIDMechanicalEquipment             4      4      4  Mechanical 4
  PIDNote                           32     32     32  ItemNote 32
  PIDNozzle                          7      7      7  Nozzle 7
  PIDPipeline                       79      0      0  
  PIDPipingBranchPoint              97      0      0  
  PIDPipingComponent               134    134    134  PipingComp 134
  PIDPipingConnector               169    169    169  PipeRun 169
  PIDPipingPort                    362      0      0  
  PIDProcessEquipment               12     12     12  EquipmentOther 12
  PIDProcessPoint                  169      0      0  
  PIDProcessVessel                   2      2      2  Vessel 2
  PIDRepresentation                902    902    902  EquipmentOther 12, Instrument 150, ItemNote 32, Mechanical 4, Nozzle 7, OPC 2, PipeRun 492, PipingComp 134, SignalRun 67, Vessel 2
  PIDSignalBranchPoint              10      0      0  
  PIDSignalConnector                46     46     46  SignalRun 46
  PIDSignalPort                    915      0      0  
  rel DefUID                          rels   both   one own raw
  DrawingItems                         902    794   108       0
  DwgRepresentationComposition         902    902     0       0
  EquipmentComponentComposition          7      7     0       0
  PBSItemNotes                           6      6     0       0
  PipingConnectors                     106      0   106       0
  PipingEnd1Conn                       129      1   128       0
  PipingEnd2Conn                       143      0   143       0
  PipingPortComposition                362      0   362       0
  PipingTapOrFitting                   369      0   369       0
  ProcessPointCollection               169      0   169       0
  SignalEnd1Conn                        38      0    38       0
  SignalEnd2Conn                        46      0    46       0
  SignalPortComposition                915      0   915       0
  SignalTapOrFitting                    13      0    13       0
== 集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图
  .pid 1146880 B, 79 streams; 7906 framed fields in 1726 sets (1726 with a header); 681 UIDs named
  objects 570: named 298 (52.3 %; DrawingID 188 / DrawingNo 1 / ModelID 109), UID raw 298
  attribute values 1731 (GraphicOID apart): empty 564, non-empty 1167: in a label cache 158 (13.5 %); of the 903 at least 4 bytes long, in the fields 84 (9.3 %), raw 98 (10.9 %)
  rels 767: both ends named 354 (46.2 %), one end 413, own UID raw 0
  representations 188: UID-joined 188, on their published GraphicOID 188
  _Meta.xml Container: CompSchema=DocVersioningComponent [-], Scope=Data [raw], SoftwareVersion=10.00.31.0023 [-], IsValidated=False [-], SchemaVersion=04.02.17.01 [-], Plant=P01 [short], DocUID=D710241072D54ECCB8F5961357166E5D [named (DrawingNo)], DocName=集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图 [label], ToolID=SMARTPLANTPID [-], ToolSignature=AAAD [-], SDECIMAL=. [short]
  _Meta.xml DocumentVersion UID [-]: IObject.Name=集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图 Version [-], IDocumentVersion.DocRevision=0 [short], IDocumentVersion.DocVersionDate=2026/07/06 [-], IDocumentVersion.DocVersion=1 [short]
  _Meta.xml DocumentRevision UID [-]: IObject.Name=集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图 Revision [-], IDocumentRevision.MajorRev_ForRevise=0 [short]
  _Meta.xml File UID [-]: IObject.Name=集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图.pid [raw], IFile.FilePath=\\Glpid-7\pidsq\PID [-]
  _Meta.xml Rel VersionedDoc: own UID [-], UID1 [named (DrawingNo)], UID2 [-]
  _Meta.xml Rel RevisedDocument: own UID [-], UID1 [-], UID2 [named (DrawingNo)]
  _Meta.xml Rel FileComposition: own UID [-], UID1 [-], UID2 [-]
  class                        objects  named    raw  named as
  PIDBranchPoint                    18      7      7  OPC 7
  PIDControlSystemFunction           5      5      5  Instrument 5
  PIDDrawing                         1      1      1  Drawing 1
  PIDInlineInstrument                2      2      2  Instrument 2
  PIDInstrument                     10     10     10  Instrument 10
  PIDNote                            7      7      7  ItemNote 7
  PIDNozzle                          5      5      5  Nozzle 5
  PIDPipeline                       29      0      0  
  PIDPipingBranchPoint              22      0      0  
  PIDPipingComponent                27     27     27  PipingComp 27
  PIDPipingConnector                39     39     39  PipeRun 39
  PIDPipingPort                     63      0      0  
  PIDProcessPoint                   39      0      0  
  PIDProcessVessel                   1      1      1  Vessel 1
  PIDRepresentation                188    188    188  Instrument 17, ItemNote 7, Nozzle 5, OPC 18, PipeRun 105, PipingComp 27, SignalRun 8, Vessel 1
  PIDSignalBranchPoint               1      0      0  
  PIDSignalConnector                 6      6      6  SignalRun 6
  PIDSignalPort                    107      0      0  
  rel DefUID                          rels   both   one own raw
  DrawingItems                         188    154    34       0
  DwgRepresentationComposition         188    188     0       0
  EquipmentComponentComposition          5      5     0       0
  PipingConnectors                      30      0    30       0
  PipingEnd1Conn                        34      2    32       0
  PipingEnd2Conn                        31      5    26       0
  PipingPortComposition                 63      0    63       0
  PipingTapOrFitting                    69      0    69       0
  ProcessPointCollection                39      0    39       0
  SignalEnd1Conn                         6      0     6       0
  SignalEnd2Conn                         6      0     6       0
  SignalPortComposition                107      0   107       0
  SignalTapOrFitting                     1      0     1       0
== 集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图
  .pid 937984 B, 78 streams; 6166 framed fields in 1373 sets (1373 with a header); 552 UIDs named
  objects 470: named 247 (52.6 %; DrawingID 151 / DrawingNo 1 / ModelID 95), UID raw 247
  attribute values 1370 (GraphicOID apart): empty 431, non-empty 939: in a label cache 121 (12.9 %); of the 709 at least 4 bytes long, in the fields 66 (9.3 %), raw 75 (10.6 %)
  rels 627: both ends named 295 (47.0 %), one end 332, own UID raw 0
  representations 151: UID-joined 151, on their published GraphicOID 151
  _Meta.xml Container: CompSchema=DocVersioningComponent [-], Scope=Data [raw], SoftwareVersion=10.00.31.0023 [-], IsValidated=False [-], SchemaVersion=04.02.17.01 [-], Plant=P01 [short], DocUID=AAFCDC31FA3C4177BECB02EBBB6F776F [named (DrawingNo)], DocName=集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图 [label], ToolID=SMARTPLANTPID [-], ToolSignature=AAAD [-], SDECIMAL=. [short]
  _Meta.xml DocumentVersion UID [-]: IObject.Name=集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图 Version [-], IDocumentVersion.DocRevision=0 [short], IDocumentVersion.DocVersionDate=2026/07/06 [-], IDocumentVersion.DocVersion=1 [short]
  _Meta.xml DocumentRevision UID [-]: IObject.Name=集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图 Revision [-], IDocumentRevision.MajorRev_ForRevise=0 [short]
  _Meta.xml File UID [-]: IObject.Name=集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图.pid [raw], IFile.FilePath=\\Glpid-7\pidsq\PID [-]
  _Meta.xml Rel VersionedDoc: own UID [-], UID1 [named (DrawingNo)], UID2 [-]
  _Meta.xml Rel RevisedDocument: own UID [-], UID1 [-], UID2 [named (DrawingNo)]
  _Meta.xml Rel FileComposition: own UID [-], UID1 [-], UID2 [-]
  class                        objects  named    raw  named as
  PIDBranchPoint                    13      8      8  OPC 8
  PIDControlSystemFunction           5      5      5  Instrument 5
  PIDDrawing                         1      1      1  Drawing 1
  PIDInlineInstrument                2      2      2  Instrument 2
  PIDInstrument                     10     10     10  Instrument 10
  PIDNote                            7      7      7  ItemNote 7
  PIDNozzle                          5      5      5  Nozzle 5
  PIDPipeline                       14      0      0  
  PIDPipingBranchPoint              14      0      0  
  PIDPipingComponent                22     22     22  PipingComp 22
  PIDPipingConnector                29     29     29  PipeRun 29
  PIDPipingPort                     53      0      0  
  PIDProcessPoint                   29      0      0  
  PIDProcessVessel                   1      1      1  Vessel 1
  PIDRepresentation                151    151    151  Instrument 17, ItemNote 7, Nozzle 5, OPC 13, PipeRun 78, PipingComp 22, SignalRun 8, Vessel 1
  PIDSignalBranchPoint               1      0      0  
  PIDSignalConnector                 6      6      6  SignalRun 6
  PIDSignalPort                    107      0      0  
  rel DefUID                          rels   both   one own raw
  DrawingItems                         151    131    20       0
  DwgRepresentationComposition         151    151     0       0
  EquipmentComponentComposition          5      5     0       0
  PipingConnectors                      15      0    15       0
  PipingEnd1Conn                        24      3    21       0
  PipingEnd2Conn                        22      5    17       0
  PipingPortComposition                 53      0    53       0
  PipingTapOrFitting                    57      0    57       0
  ProcessPointCollection                29      0    29       0
  SignalEnd1Conn                         6      0     6       0
  SignalEnd2Conn                         6      0     6       0
  SignalPortComposition                107      0   107       0
  SignalTapOrFitting                     1      0     1       0
== 集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图
  .pid 1224704 B, 84 streams; 7762 framed fields in 1731 sets (1731 with a header); 720 UIDs named
  objects 679: named 325 (47.9 %; DrawingID 193 / DrawingNo 1 / ModelID 131), UID raw 325
  attribute values 1994 (GraphicOID apart): empty 650, non-empty 1344: in a label cache 165 (12.3 %); of the 983 at least 4 bytes long, in the fields 85 (8.6 %), raw 102 (10.4 %)
  rels 884: both ends named 382 (43.2 %), one end 502, own UID raw 0
  representations 193: UID-joined 193, on their published GraphicOID 193
  _Meta.xml Container: CompSchema=DocVersioningComponent [-], Scope=Data [raw], SoftwareVersion=10.00.31.0023 [-], IsValidated=False [-], SchemaVersion=04.02.17.01 [-], Plant=P01 [short], DocUID=DABA92D1739740EAB431DAC6DF5F6419 [named (DrawingNo)], DocName=集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图 [label], ToolID=SMARTPLANTPID [-], ToolSignature=AAAD [raw], SDECIMAL=. [short]
  _Meta.xml DocumentVersion UID [-]: IObject.Name=集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图 Version [-], IDocumentVersion.DocRevision=0 [short], IDocumentVersion.DocVersionDate=2026/07/06 [-], IDocumentVersion.DocVersion=1 [short]
  _Meta.xml DocumentRevision UID [-]: IObject.Name=集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图 Revision [-], IDocumentRevision.MajorRev_ForRevise=0 [short]
  _Meta.xml File UID [-]: IObject.Name=集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图.pid [raw], IFile.FilePath=\\Glpid-7\pidsq\PID [-]
  _Meta.xml Rel VersionedDoc: own UID [-], UID1 [named (DrawingNo)], UID2 [-]
  _Meta.xml Rel RevisedDocument: own UID [-], UID1 [-], UID2 [named (DrawingNo)]
  _Meta.xml Rel FileComposition: own UID [-], UID1 [-], UID2 [-]
  class                        objects  named    raw  named as
  PIDBranchPoint                    12      8      8  OPC 8
  PIDControlSystemFunction          10     10     10  Instrument 10
  PIDDrawing                         1      1      1  Drawing 1
  PIDInlineInstrument                5      5      5  Instrument 5
  PIDInstrument                     15     15     15  Instrument 15
  PIDNote                            8      8      8  ItemNote 8
  PIDNozzle                          9      9      9  Nozzle 9
  PIDPipeline                       28      0      0  
  PIDPipingBranchPoint              15      0      0  
  PIDPipingComponent                25     25     25  PipingComp 25
  PIDPipingConnector                41     41     41  PipeRun 41
  PIDPipingPort                     68      0      0  
  PIDProcessPoint                   41      0      0  
  PIDProcessVessel                   1      1      1  Vessel 1
  PIDRepresentation                193    193    193  Instrument 30, ItemNote 8, Nozzle 9, OPC 12, PipeRun 96, PipingComp 25, SignalRun 12, Vessel 1
  PIDSignalBranchPoint               1      0      0  
  PIDSignalConnector                 9      9      9  SignalRun 9
  PIDSignalPort                    197      0      0  
  rel DefUID                          rels   both   one own raw
  DrawingItems                         193    173    20       0
  DwgRepresentationComposition         193    193     0       0
  EquipmentComponentComposition          9      9     0       0
  PipingConnectors                      30      0    30       0
  PipingEnd1Conn                        36      2    34       0
  PipingEnd2Conn                        30      5    25       0
  PipingPortComposition                 68      0    68       0
  PipingTapOrFitting                    66      0    66       0
  ProcessPointCollection                41      0    41       0
  SignalEnd1Conn                         9      0     9       0
  SignalEnd2Conn                         9      0     9       0
  SignalPortComposition                197      0   197       0
  SignalTapOrFitting                     3      0     3       0
== 集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图
  .pid 1699840 B, 87 streams; 12769 framed fields in 2655 sets (2655 with a header); 1307 UIDs named
  objects 1319: named 618 (46.9 %; DrawingID 389 / DrawingNo 1 / ModelID 228), UID raw 618
  attribute values 3594 (GraphicOID apart): empty 1173, non-empty 2421: in a label cache 236 (9.7 %); of the 1712 at least 4 bytes long, in the fields 120 (7.0 %), raw 162 (9.5 %)
  rels 1800: both ends named 750 (41.7 %), one end 1050, own UID raw 0
  representations 389: UID-joined 389, on their published GraphicOID 389
  _Meta.xml Container: CompSchema=DocVersioningComponent [-], Scope=Data [raw], SoftwareVersion=10.00.31.0023 [-], IsValidated=False [-], SchemaVersion=04.02.17.01 [-], Plant=P01 [short], DocUID=5A552026D9144FF9BA325D9C8160064C [named (DrawingNo)], DocName=集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图 [label], ToolID=SMARTPLANTPID [-], ToolSignature=AAAD [raw], SDECIMAL=. [short]
  _Meta.xml DocumentVersion UID [-]: IObject.Name=集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图 Version [-], IDocumentVersion.DocRevision=0 [short], IDocumentVersion.DocVersionDate=2026/07/06 [-], IDocumentVersion.DocVersion=1 [short]
  _Meta.xml DocumentRevision UID [-]: IObject.Name=集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图 Revision [-], IDocumentRevision.MajorRev_ForRevise=0 [short]
  _Meta.xml File UID [-]: IObject.Name=集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图.pid [raw], IFile.FilePath=\\Glpid-7\pidsq\PID [-]
  _Meta.xml Rel VersionedDoc: own UID [-], UID1 [named (DrawingNo)], UID2 [-]
  _Meta.xml Rel RevisedDocument: own UID [-], UID1 [-], UID2 [named (DrawingNo)]
  _Meta.xml Rel FileComposition: own UID [-], UID1 [-], UID2 [-]
  class                        objects  named    raw  named as
  PIDBranchPoint                    13      6      6  OPC 6
  PIDControlSystemFunction          18     18     18  Instrument 18
  PIDDrawing                         1      1      1  Drawing 1
  PIDHeatTransferEquipment           1      1      1  Exchanger 1
  PIDInlineInstrument               23     23     23  Instrument 23
  PIDInstrument                     32     32     32  Instrument 32
  PIDNote                            2      2      2  ItemNote 2
  PIDNozzle                         10     10     10  Nozzle 10
  PIDPipeline                       22      0      0  
  PIDPipingBranchPoint              35      0      0  
  PIDPipingComponent                56     56     56  PipingComp 56
  PIDPipingConnector                63     63     63  PipeRun 63
  PIDPipingPort                    172      0      0  
  PIDProcessEquipment                1      1      1  EquipmentOther 1
  PIDProcessPoint                   63      0      0  
  PIDRepresentation                389    389    389  EquipmentOther 1, Exchanger 1, Instrument 73, ItemNote 2, Nozzle 10, OPC 13, PipeRun 204, PipingComp 56, SignalRun 29
  PIDSignalBranchPoint               3      0      0  
  PIDSignalConnector                16     16     16  SignalRun 16
  PIDSignalPort                    399      0      0  
  rel DefUID                          rels   both   one own raw
  DrawingItems                         389    344    45       0
  DwgRepresentationComposition         389    389     0       0
  EquipmentComponentComposition         10     10     0       0
  PBSItemNotes                           1      1     0       0
  PipingConnectors                      38      0    38       0
  PipingEnd1Conn                        60      1    59       0
  PipingEnd2Conn                        53      5    48       0
  PipingPortComposition                172      0   172       0
  PipingTapOrFitting                   177      0   177       0
  ProcessPointCollection                63      0    63       0
  SignalEnd1Conn                        16      0    16       0
  SignalEnd2Conn                        16      0    16       0
  SignalPortComposition                399      0   399       0
  SignalTapOrFitting                    17      0    17       0
== all 5 drawings
  objects 6131: named 2948 (48.1 %)
  class                        objects  named    raw  named as
  PIDRepresentation               1823   1823   1823  EquipmentOther 13, Exchanger 1, Instrument 287, ItemNote 56, Mechanical 4, Nozzle 36, OPC 58, PipeRun 975, PipingComp 264, SignalRun 124, Vessel 5
  PIDSignalPort                   1725      0      0  
  PIDPipingPort                    718      0      0  
  PIDPipingConnector               341    341    341  PipeRun 341
  PIDProcessPoint                  341      0      0  
  PIDPipingComponent               264    264    264  PipingComp 264
  PIDPipingBranchPoint             183      0      0  
  PIDPipeline                      172      0      0  
  PIDInstrument                    127    127    127  Instrument 127
  PIDControlSystemFunction          88     88     88  Instrument 88
  PIDSignalConnector                83     83     83  SignalRun 83
  PIDInlineInstrument               72     72     72  Instrument 72
  PIDBranchPoint                    58     30     30  OPC 30
  PIDNote                           56     56     56  ItemNote 56
  PIDNozzle                         36     36     36  Nozzle 36
  PIDSignalBranchPoint              16      0      0  
  PIDProcessEquipment               13     13     13  EquipmentOther 13
  PIDDrawing                         5      5      5  Drawing 5
  PIDProcessVessel                   5      5      5  Vessel 5
  PIDMechanicalEquipment             4      4      4  Mechanical 4
  PIDHeatTransferEquipment           1      1      1  Exchanger 1
  attribute values 17291 (GraphicOID apart): empty 5620, non-empty 11671: in a label cache 1296 (11.1 %); of the 8495 at least 4 bytes long, in the fields 651 (7.7 %), raw 851 (10.0 %)
  interface.attribute (non-empty >= 5)                 values labels  short fields    raw  label caches carrying them
  IObject.Name                                           3298      4   2501      4    137  <F>Name 4, Name 4
  IPBSItem.ConstructionStatus                             951      0      0      0      0  
  IPBSItem.ConstructionStatus2                            951      0      0      0      0  
  IPIDTypical.IsTypical                                   946      0      0      0      0  
  IPipeCrossSectionItem.NominalDiameter                   878      0      0      0      0  
  IConnector.RepresentationsAreAllZeroLength              341      0      0      0      0  
  IPipingConnector.PipingConnectorType                    341      0      0      0      0  
  IInlineComponent.IsFlowDirectional                      336      0      0      0      0  
  IConnection.ConnectionFlowDirection                     298      0      0      0      0  
  IPipingSpecifiedItem.PipingMaterialsClass               282    282    282      0      0  <F>PipingMaterialsClass 282, PipingMaterialsClass 282
  IPipingComponent.CommoditySpecialtyType                 264      0      0      0      0  
  IPipingComponent.PipingComponentType1                   264      0      0      0      0  
  IPipingComponent.PipingComponentType2                   262      0      0      0      0  
  IPipingComponent.PipModelCode                           259      0      5      0     35  
  INamedInstrument.InstrTagSequenceNo                     236    219     17    219    219  <F>TagSequenceNo 219, TagSequenceNo 219
  INamedInstrument.MeasuredVariable                       236    228    219     17     17  <F>MeasuredVariableCode 228, MeasuredVariableCode 228, <F>OperFluidCode 21
  INamedPipingConnector.PipingConnectorSeqNo              219    186      0    186    208  <F>TagSequenceNo 186, TagSequenceNo 186
  IPipingComponent.PipingComponentType3                   210      0      0      0      0  
  IInsulatedItem.InsulSpec                                201    201    127     74     74  <F>InsulationSpec 201, InsulationSpec 201
  IFluidSystem.FluidSystem                                145      0      0      0      0  
  IInstrumentOcc.InstrumentLocation                       116      0      0      0      0  
  IConnector.FlowDirection                                103      0      0      0      0  
  IObject.Description                                      63     58      0     58     58  <F>Description 58, Description 58
  IOffpageConnector.ToFromText                             58     58      0     58     58  <F>ToFromText 58, ToFromText 58
  IInstrument.ProcFunc                                     49      0      0      0      0  
  IInstrument.InstrumentType                               46      0      0      0      0  
  IInstrument.InstrumentType3                              46      0      0      0      0  
  IEquipmentComponent.ProcEqpCompTypeDescription           36      0      0      0      5  
  IEquipmentComponent.ProcessEqCompType1                   36      0      0      0      0  
  IEquipmentComponent.ProcessEqCompType2                   36      0      0      0      0  
  INote.NoteText                                           28     28      6     22     22  <F>Note.Body 28, Note.Body 28
  IEquipment.EqType0                                       23      0      0      0      0  
  IEquipment.EqTypeDescription                             22      0      0      0      0  
  INamedEquipment.EquipPrefix                              18     18     18      0      0  <F>TagPrefix 18, TagPrefix 18
  INamedEquipment.EquipSeqNo                               13     13      0     13     13  <F>TagSequenceNo 13, TagSequenceNo 13
  IEquipment.EqType1                                       10      0      0      0      0  
  IEquipment.EqType2                                        9      0      0      0      0  
  IEquipment.EqType3                                        7      0      0      0      0  
  IEquipment.EqType4                                        7      0      0      0      0  
  ISpecifiedMatlItem.LongMaterialDescription3               6      0      0      0      0  
  IDocument.DocCategory                                     5      0      0      0      0  
  IDocument.DocType                                         5      0      0      0      5  
  class                        interface.attribute (non-empty >= 5)                 values labels  short fields    raw
  PIDSignalPort                IObject.Name                                           1725      0   1725      0      0
  PIDPipingPort                IObject.Name                                            718      0    718      0      0
  PIDPipingPort                IPipeCrossSectionItem.NominalDiameter                   444      0      0      0      0
  PIDPipingPort                IConnection.ConnectionFlowDirection                     298      0      0      0      0
  PIDPipingConnector           IConnector.RepresentationsAreAllZeroLength              341      0      0      0      0
  PIDPipingConnector           IPBSItem.ConstructionStatus                             341      0      0      0      0
  PIDPipingConnector           IPBSItem.ConstructionStatus2                            341      0      0      0      0
  PIDPipingConnector           IPIDTypical.IsTypical                                   341      0      0      0      0
  PIDPipingConnector           IPipingConnector.PipingConnectorType                    341      0      0      0      0
  PIDPipingConnector           IPipingSpecifiedItem.PipingMaterialsClass               261    261    261      0      0
  PIDPipingConnector           INamedPipingConnector.PipingConnectorSeqNo              219    186      0    186    208
  PIDPipingConnector           IObject.Name                                            219      0      0      0      0
  PIDPipingConnector           IPipeCrossSectionItem.NominalDiameter                   200      0      0      0      0
  PIDPipingConnector           IInsulatedItem.InsulSpec                                106    106     72     34     34
  PIDPipingConnector           IConnector.FlowDirection                                103      0      0      0      0
  PIDPipingComponent           IInlineComponent.IsFlowDirectional                      264      0      0      0      0
  PIDPipingComponent           IPBSItem.ConstructionStatus                             264      0      0      0      0
  PIDPipingComponent           IPBSItem.ConstructionStatus2                            264      0      0      0      0
  PIDPipingComponent           IPIDTypical.IsTypical                                   264      0      0      0      0
  PIDPipingComponent           IPipingComponent.CommoditySpecialtyType                 264      0      0      0      0
  PIDPipingComponent           IPipingComponent.PipingComponentType1                   264      0      0      0      0
  PIDPipingComponent           IPipingComponent.PipingComponentType2                   262      0      0      0      0
  PIDPipingComponent           IPipingComponent.PipModelCode                           259      0      5      0     35
  PIDPipingComponent           IPipingComponent.PipingComponentType3                   210      0      0      0      0
  PIDPipingComponent           IPipeCrossSectionItem.NominalDiameter                   164      0      0      0      0
  PIDPipingComponent           IObject.Name                                            152      0      0      0     94
  PIDPipingComponent           IInsulatedItem.InsulSpec                                 76     76     40     36     36
  PIDPipeline                  IObject.Name                                            172      0      0      0     27
  PIDPipeline                  IFluidSystem.FluidSystem                                145      0      0      0      0
  PIDInstrument                IPBSItem.ConstructionStatus                             127      0      0      0      0
  PIDInstrument                IPBSItem.ConstructionStatus2                            127      0      0      0      0
  PIDInstrument                IPIDTypical.IsTypical                                   127      0      0      0      0
  PIDInstrument                INamedInstrument.InstrTagSequenceNo                     118    113      5    113    113
  PIDInstrument                INamedInstrument.MeasuredVariable                       118    113    116      2      2
  PIDInstrument                IObject.Name                                            118      0      0      0      2
  PIDInstrument                IInstrumentOcc.InstrumentLocation                       116      0      0      0      0
  PIDControlSystemFunction     IPBSItem.ConstructionStatus                              88      0      0      0      0
  PIDControlSystemFunction     IPBSItem.ConstructionStatus2                             88      0      0      0      0
  PIDControlSystemFunction     INamedInstrument.InstrTagSequenceNo                      79     79      0     79     79
  PIDControlSystemFunction     INamedInstrument.MeasuredVariable                        79     79     66     13     13
  PIDControlSystemFunction     IObject.Name                                             79      0      0      0      0
  PIDSignalConnector           IPIDTypical.IsTypical                                    83      0      0      0      0
  PIDInlineInstrument          IInlineComponent.IsFlowDirectional                       72      0      0      0      0
  PIDInlineInstrument          IPBSItem.ConstructionStatus                              72      0      0      0      0
  PIDInlineInstrument          IPBSItem.ConstructionStatus2                             72      0      0      0      0
  PIDInlineInstrument          IPIDTypical.IsTypical                                    72      0      0      0      0
  PIDInlineInstrument          IInstrument.ProcFunc                                     49      0      0      0      0
  PIDInlineInstrument          IInstrument.InstrumentType                               46      0      0      0      0
  PIDInlineInstrument          IInstrument.InstrumentType3                              46      0      0      0      0
  PIDInlineInstrument          IPipeCrossSectionItem.NominalDiameter                    41      0      0      0      0
  PIDInlineInstrument          INamedInstrument.InstrTagSequenceNo                      39     27     12     27     27
  PIDInlineInstrument          INamedInstrument.MeasuredVariable                        39     36     37      2      2
  PIDInlineInstrument          IObject.Name                                             39      0      0      0      0
  PIDInlineInstrument          IInsulatedItem.InsulSpec                                 19     19     15      4      4
  PIDBranchPoint               IObject.Description                                      58     58      0     58     58
  PIDBranchPoint               IObject.Name                                             58      0     58      0      0
  PIDBranchPoint               IOffpageConnector.ToFromText                             58     58      0     58     58
  PIDNote                      INote.NoteText                                           28     28      6     22     22
  PIDNozzle                    IEquipmentComponent.ProcEqpCompTypeDescription           36      0      0      0      5
  PIDNozzle                    IEquipmentComponent.ProcessEqCompType1                   36      0      0      0      0
  PIDNozzle                    IEquipmentComponent.ProcessEqCompType2                   36      0      0      0      0
  PIDNozzle                    IPBSItem.ConstructionStatus                              36      0      0      0      0
  PIDNozzle                    IPBSItem.ConstructionStatus2                             36      0      0      0      0
  PIDNozzle                    IPIDTypical.IsTypical                                    36      0      0      0      0
  PIDNozzle                    IPipeCrossSectionItem.NominalDiameter                    29      0      0      0      0
  PIDNozzle                    IPipingSpecifiedItem.PipingMaterialsClass                21     21     21      0      0
  PIDProcessEquipment          IEquipment.EqType0                                       13      0      0      0      0
  PIDProcessEquipment          IPBSItem.ConstructionStatus                              13      0      0      0      0
  PIDProcessEquipment          IPBSItem.ConstructionStatus2                             13      0      0      0      0
  PIDProcessEquipment          IPIDTypical.IsTypical                                    13      0      0      0      0
  PIDProcessEquipment          IEquipment.EqTypeDescription                             12      0      0      0      0
  PIDProcessEquipment          INamedEquipment.EquipPrefix                              10     10     10      0      0
  PIDProcessEquipment          ISpecifiedMatlItem.LongMaterialDescription3               6      0      0      0      0
  PIDProcessEquipment          INamedEquipment.EquipSeqNo                                5      5      0      5      5
  PIDProcessEquipment          IObject.Name                                              5      0      0      0      5
  PIDDrawing                   IDocument.DocCategory                                     5      0      0      0      0
  PIDDrawing                   IDocument.DocType                                         5      0      0      0      5
  PIDDrawing                   IObject.Name                                              5      4      0      4      5
  PIDProcessVessel             IEquipment.EqType0                                        5      0      0      0      0
  PIDProcessVessel             IEquipment.EqType1                                        5      0      0      0      0
  PIDProcessVessel             IEquipment.EqType2                                        5      0      0      0      0
  PIDProcessVessel             IEquipment.EqType3                                        5      0      0      0      0
  PIDProcessVessel             IEquipment.EqType4                                        5      0      0      0      0
  PIDProcessVessel             IEquipment.EqTypeDescription                              5      0      0      0      0
  PIDProcessVessel             INamedEquipment.EquipPrefix                               5      5      5      0      0
  PIDProcessVessel             INamedEquipment.EquipSeqNo                                5      5      0      5      5
  PIDProcessVessel             IObject.Name                                              5      0      0      0      1
  PIDProcessVessel             IPBSItem.ConstructionStatus                               5      0      0      0      0
  PIDProcessVessel             IPBSItem.ConstructionStatus2                              5      0      0      0      0
  PIDProcessVessel             IPIDTypical.IsTypical                                     5      0      0      0      0
  rels 8185: both ends named 3491 (42.7 %), own UID raw 0
  rel DefUID                          rels   both   one own raw
  DrawingItems                        1823   1596   227       0
  DwgRepresentationComposition        1823   1823     0       0
  SignalPortComposition               1725      0  1725       0
  PipingTapOrFitting                   738      0   738       0
  PipingPortComposition                718      0   718       0
  ProcessPointCollection               341      0   341       0
  PipingEnd1Conn                       283      9   274       0
  PipingEnd2Conn                       279     20   259       0
  PipingConnectors                     219      0   219       0
  SignalEnd2Conn                        83      0    83       0
  SignalEnd1Conn                        75      0    75       0
  EquipmentComponentComposition         36     36     0       0
  SignalTapOrFitting                    35      0    35       0
  PBSItemNotes                           7      7     0       0
  representations 1823: UID-joined 1823, on their published GraphicOID 1823
  attribute sets by storage / record family / class (all drawings)
         4  /            0x0089 FreeFormAttrSet               DrawingBorder
        11  /            0x0089 FreeFormAttrSet               FromTemplate
      1569  /            0x0089 FreeFormAttrSet               MultiLinearPattern
         7  /            0x0089 FreeFormAttrSet               P&ID
      5189  /            0x0089 FreeFormAttrSet               P&IDAttributes
         5  /            0x0089 FreeFormAttrSet               PIDTemplateVersion
       479  /            0x0089 FreeFormAttrSet               Symbol
       398  /            0x0089 FreeFormAttrSet               SymbolAttr
         5  /            0x0089 FreeFormAttrSet               TypicalMode
        20  /            0x0089 FreeFormAttrSet               _AlarmHigh
         6  /            0x0089 FreeFormAttrSet               _AlarmHighHigh
        20  /            0x0089 FreeFormAttrSet               _AlarmLow
         2  /            0x0089 FreeFormAttrSet               _AlarmLowLow
       206  /            0x0089 FreeFormAttrSet               _BranchPoint
        50  /            0x0089 FreeFormAttrSet               _CleaningReqmts
        50  /            0x0089 FreeFormAttrSet               _CoatingReqmts
        58  /            0x0089 FreeFormAttrSet               _Description
         8  /            0x0089 FreeFormAttrSet               _DrawingNumber
        50  /            0x0089 FreeFormAttrSet               _FluidSystem
        50  /            0x0089 FreeFormAttrSet               _HTraceMedium
        50  /            0x0089 FreeFormAttrSet               _HTraceMediumTemp
        50  /            0x0089 FreeFormAttrSet               _HTraceReqmt
        50  /            0x0089 FreeFormAttrSet               _InsulPurpose
        50  /            0x0089 FreeFormAttrSet               _InsulTemp
        50  /            0x0089 FreeFormAttrSet               _InsulType
       140  /            0x0089 FreeFormAttrSet               _InsulationSpec
       219  /            0x0089 FreeFormAttrSet               _MeasuredVariableCode
         4  /            0x0089 FreeFormAttrSet               _Name
       318  /            0x0089 FreeFormAttrSet               _NominalDiameter
        28  /            0x0089 FreeFormAttrSet               _Note.Body
       106  /            0x0089 FreeFormAttrSet               _OpeningAction
       140  /            0x0089 FreeFormAttrSet               _OperFluidCode
       152  /            0x0089 FreeFormAttrSet               _PipingComponentCode
       185  /            0x0089 FreeFormAttrSet               _PipingMaterialsClass
         1  /            0x0089 FreeFormAttrSet               _PipingPoint1.NominalDiameter
         1  /            0x0089 FreeFormAttrSet               _PipingPoint2.NominalDiameter
        50  /            0x0089 FreeFormAttrSet               _ProcessDesign.Max.Pressure
        50  /            0x0089 FreeFormAttrSet               _ProcessDesign.Max.SpecificGravity
        50  /            0x0089 FreeFormAttrSet               _ProcessDesign.Max.Temperature
        50  /            0x0089 FreeFormAttrSet               _ProcessOperating.Max.Pressure
        50  /            0x0089 FreeFormAttrSet               _ProcessOperating.Max.SpecificGravity
        50  /            0x0089 FreeFormAttrSet               _ProcessOperating.Max.Temperature
        50  /            0x0089 FreeFormAttrSet               _ProcessTest.Max.Pressure
         4  /            0x0089 FreeFormAttrSet               _SP_SetPressure
      1279  /            0x0089 FreeFormAttrSet               _SmartText
        50  /            0x0089 FreeFormAttrSet               _StreamNo
        50  /            0x0089 FreeFormAttrSet               _StressAnalysisReqmts
       254  /            0x0089 FreeFormAttrSet               _SubFunctionArea
        15  /            0x0089 FreeFormAttrSet               _TagPrefix
       526  /            0x0089 FreeFormAttrSet               _TagSequenceNo
        58  /            0x0089 FreeFormAttrSet               _ToFromText
        28  /JSite2895/  0x0089 FreeFormAttrSet               P&ID Symbol
        14  /JSite2895/  0x0089 FreeFormAttrSet               SymbolAttr
        28  /JSite2895/  0x0089 FreeFormAttrSet               SymbolIcon
       101  /JSite2895/  0x0089 FreeFormAttrSet               _ConnectPoint
         1  /JSite2895/  0x0089 FreeFormAttrSet               _Description
         8  /JSite2895/  0x0089 FreeFormAttrSet               _MeasuredVariableCode
         3  /JSite2895/  0x0089 FreeFormAttrSet               _Note.Body
        21  /JSite2895/  0x0089 FreeFormAttrSet               _SmartText
         8  /JSite2895/  0x0089 FreeFormAttrSet               _SubFunctionArea
         2  /JSite2895/  0x0089 FreeFormAttrSet               _SymbolBehavior
         8  /JSite2895/  0x0089 FreeFormAttrSet               _TagSequenceNo
         1  /JSite2895/  0x0089 FreeFormAttrSet               _ToFromText
        60  /JSite37311/ 0x0089 FreeFormAttrSet               P&ID Symbol
        26  /JSite37311/ 0x0089 FreeFormAttrSet               SymbolAttr
        60  /JSite37311/ 0x0089 FreeFormAttrSet               SymbolIcon
       179  /JSite37311/ 0x0089 FreeFormAttrSet               _ConnectPoint
         1  /JSite37311/ 0x0089 FreeFormAttrSet               _Description
        10  /JSite37311/ 0x0089 FreeFormAttrSet               _MeasuredVariableCode
         5  /JSite37311/ 0x0089 FreeFormAttrSet               _Note.Body
        27  /JSite37311/ 0x0089 FreeFormAttrSet               _SmartText
        10  /JSite37311/ 0x0089 FreeFormAttrSet               _SubFunctionArea
        10  /JSite37311/ 0x0089 FreeFormAttrSet               _SymbolBehavior
        10  /JSite37311/ 0x0089 FreeFormAttrSet               _TagSequenceNo
         1  /JSite37311/ 0x0089 FreeFormAttrSet               _ToFromText
        29  /JSite3773/  0x0089 FreeFormAttrSet               P&ID Symbol
        14  /JSite3773/  0x0089 FreeFormAttrSet               SymbolAttr
        29  /JSite3773/  0x0089 FreeFormAttrSet               SymbolIcon
       103  /JSite3773/  0x0089 FreeFormAttrSet               _ConnectPoint
         1  /JSite3773/  0x0089 FreeFormAttrSet               _Description
         8  /JSite3773/  0x0089 FreeFormAttrSet               _MeasuredVariableCode
         3  /JSite3773/  0x0089 FreeFormAttrSet               _Note.Body
        21  /JSite3773/  0x0089 FreeFormAttrSet               _SmartText
         8  /JSite3773/  0x0089 FreeFormAttrSet               _SubFunctionArea
         2  /JSite3773/  0x0089 FreeFormAttrSet               _SymbolBehavior
         8  /JSite3773/  0x0089 FreeFormAttrSet               _TagSequenceNo
         1  /JSite3773/  0x0089 FreeFormAttrSet               _ToFromText
        33  /JSite381/   0x0089 FreeFormAttrSet               P&ID Symbol
        17  /JSite381/   0x0089 FreeFormAttrSet               SymbolAttr
        33  /JSite381/   0x0089 FreeFormAttrSet               SymbolIcon
       135  /JSite381/   0x0089 FreeFormAttrSet               _ConnectPoint
         1  /JSite381/   0x0089 FreeFormAttrSet               _Description
         9  /JSite381/   0x0089 FreeFormAttrSet               _MeasuredVariableCode
         2  /JSite381/   0x0089 FreeFormAttrSet               _Note.Body
        22  /JSite381/   0x0089 FreeFormAttrSet               _SmartText
         9  /JSite381/   0x0089 FreeFormAttrSet               _SubFunctionArea
         3  /JSite381/   0x0089 FreeFormAttrSet               _SymbolBehavior
         9  /JSite381/   0x0089 FreeFormAttrSet               _TagSequenceNo
         1  /JSite381/   0x0089 FreeFormAttrSet               _ToFromText
        38  /JSite81/    0x0089 FreeFormAttrSet               P&ID Symbol
        16  /JSite81/    0x0089 FreeFormAttrSet               SymbolAttr
        38  /JSite81/    0x0089 FreeFormAttrSet               SymbolIcon
       163  /JSite81/    0x0089 FreeFormAttrSet               _ConnectPoint
         1  /JSite81/    0x0089 FreeFormAttrSet               _Description
         7  /JSite81/    0x0089 FreeFormAttrSet               _MeasuredVariableCode
         2  /JSite81/    0x0089 FreeFormAttrSet               _Note.Body
        18  /JSite81/    0x0089 FreeFormAttrSet               _SmartText
         7  /JSite81/    0x0089 FreeFormAttrSet               _SubFunctionArea
         5  /JSite81/    0x0089 FreeFormAttrSet               _SymbolBehavior
         7  /JSite81/    0x0089 FreeFormAttrSet               _TagSequenceNo
         1  /JSite81/    0x0089 FreeFormAttrSet               _ToFromText
```

</details>
