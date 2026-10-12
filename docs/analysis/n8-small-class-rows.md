# N8 小类：管口、注释、OPC 在 Oracle exp 里的列（2026-10-12）

> 任务 t-1986。只读 `D:\work\cad\pid-test-data\Export.dmp`（`EXPORT:V12.01`，74,489,856 字节）。不跑 `imp`，不连 Oracle，不改 `src/publish`，不重写 `D:\work\cad\PID自研软件验证\publish`。没读管线表，也没读 `T_CONNECTOR`。
> 探针 `examples/probe_oracle_exp_small_classes.rs`。`cargo run --example probe_oracle_exp_small_classes` 退出 0。
> 既有测试 NOT RUN：本轮没有改 `src`，没有跑 `cargo test`。

## 一句话

I井场 7 条设备↔管口在 `T_NOZZLE` 上是同一行：`SP_ID` 是管口，`SP_EQUIPMENTID` 是容器，对调是 0。6 条注释的两端不在 `T_ITEMNOTE`（那张表只有注释自己的 `SP_ID`），在 `T_NOTE` 上是同一行：`SP_ID` 与 `SP_ITEMNOTEID` 都是注释，`SP_MODELITEMID` 是被注释的物项。2 个 `PIDBranchPoint` 都在 `T_OPC.SP_ID`；少发的 2 条 `DrawingItems` 是表示→这两个分支点，表示端在 `T_OPC` 里是 0，关系本身也对不上 `SP_ID` / `SP_PAIREDWITHID`。

参考是 `D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图_Data.xml`（`DocUID` `5633AF4121EE4F4D9FE78E90722CC050`，对象 3,093，关系 4,107）。已发是 `publish\` 下同名文件（对象 1,735，关系 1,971）。字符列字符集 2000、形式 2 的按 UTF-16BE 读；`VARCHAR2`（字符集 873、形式 1）按字节读。`DATE` 的类型码是 12，头上的最大长度是 7。标识列只拿形式 2、列名里带 `ID` 的列来对 UID。

## 行数和非空列

`T_NOZZLE` 377 行、52 列。非空：`SP_ID` 377，`NOZZLETYPE` 377，`SP_EQUIPMENTID` 377，`NOMINALDIAMETER` 274，`OPERFLUIDCODE` 267，`FLUIDSYSTEM` 267，`PIPINGMATERIALSCLASS` 251，`INSULPURPOSE` 24。`SP_PIPERUNID` 和 `FLOWDIRECTION` 都是 0。其余列非空也是 0。

`T_ITEMNOTE` 800 行、2 列，两列都满：`SP_ID` 800，`ITEMNOTETYPE` 800。没有父项列。

`T_NOTE` 800 行、10 列。非空：`SP_ID` 800，`SP_ITEMNOTEID` 800，`NOTETYPE` 800，`UPDATECOUNT` 800，`BODY` 473，`SP_MODELITEMID` 239。`AUTHOR`、`TIMESTAMP`、`DESCRIPTION`、`SP_DRAWINGID` 都是 0。`SP_DRAWINGID` 没有一张图的 id，I井场 `DocUID` 对不上，空行 800。

`T_OPC` 952 行、10 列。非空：`SP_ID` 952，`TAGREQDFLAG` 952，`OPCTYPE` 952，`SP_PAIREDWITHID` 952，`OPCTAG` 952，`SP_WSSITEID` 952，`MATINGOPCPATH` 939，`TOFROMTEXT` 456。`TODRAWINGNAME` 和 `TODRAWINGSPID` 都是 0。

## 设备↔管口

参考 `EquipmentComponentComposition` 7 条，全是 `PIDNozzle` → `PIDProcessVessel`。

| 对法 | 命中 |
|---|---:|
| 同一行 `SP_ID` = 管口、`SP_EQUIPMENTID` = 容器 | 7 |
| 对调 | 0 |
| 两端都在标识列里（不要求同一行） | 7 |

6 条挂在容器 `E7FD246A423149B28966D9DC5DD4C017`，1 条挂在 `F2AE6E4A4C2541AAA084459B29A82911`。标识列里没有更好的另一对。

## 注释

参考 `PBSItemNotes` 6 条：`PIDNote` → `PIDControlSystemFunction` 4，`PIDNote` → `PIDInlineInstrument` 1，`PIDNote` → `PIDPipingComponent` 1。

`T_ITEMNOTE` 只有 `SP_ID`。6 个注释 UID 都在 `SP_ID`，6 个父项都是 0。同一行配不成对，所以有序和对调都是 0。

`T_NOTE`：

| 对法 | 命中 |
|---|---:|
| 同一行 `SP_ITEMNOTEID` = 注释、`SP_MODELITEMID` = 父项 | 6 |
| 对调 | 0 |
| 两端都在标识列里 | 6 |

这 6 行上 `SP_ID` 等于 `SP_ITEMNOTEID`。父项是两个控制系统功能（各 2 条）、一个直连仪表、一个管件。

## OPC

参考 `PIDBranchPoint` 2 个，已发 0 个。两个 UID 都在 `T_OPC` 的标识列里（`SP_ID` 和 `SP_PAIREDWITHID` 都出现过）。以 `SP_ID` 取行，配对对象不是这两个分支点自己，也不是下面两条表示：

| `SP_ID` | `SP_PAIREDWITHID` |
|---|---|
| `17288DB92E9F4A76B97EA8F56E2CFDEC` | `D3F117B81BE94135A0C286B869384290` |
| `6ECB5E61A73C4AA39B6BAD56EB93AECA` | `8B4053A297404826A46FAB595D09F398` |

`DrawingItems` 参考 902、已发 900，参考有而已发没有的端点对是 2。两条都是 `PIDRepresentation` → `PIDBranchPoint`，分支点就是上面那两个。表示端 `A851C83D57AD4DFDB43220C2659CA004` 和 `8C3B7F64E34347F29AEAB617353C16A0` 在 `T_OPC` 标识列里是 0。这两条关系按 `SP_ID` → `SP_PAIREDWITHID` 同一行是 0，对调是 0。

## 复现

```powershell
cargo run --example probe_oracle_exp_small_classes
```

可选三个参数：dump、参考 `_Data.xml`、已发 `_Data.xml`。缺省就是上面这三个路径。
