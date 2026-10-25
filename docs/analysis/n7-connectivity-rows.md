# N7 连通行：I井场六种连通关系对上 T_CONNECTOR（2026-10-12）

> 任务 t-22。只读 `D:\work\cad\pid-test-data\Export.dmp`（`EXPORT:V12.01`，74,489,856 字节）。不跑 `imp`，不连 Oracle。
> 探针 `examples/probe_oracle_exp_connectivity.rs`。`cargo run --example probe_oracle_exp_connectivity` 退出 0。
> 10-12 那一轮只拼两端、不写 `<Rel>`。10-13 补上「定 DefUID」一节后导出器写了（见文末「导出器」）。

## 一句话

`T_CONNECTOR` 的单元格里没有发布 UID，同一行同时含两端原文的是 0。顺着标识列拼：I井场这张图 452 行，物项都是管段或信号线；点号是 `STARTOBJECT` / `ENDOBJECT`；两条 OPC 取 `T_OPC` 一对 Id 里字典序较小的那个。六种关系有序都全中，对调都是 0，两端都可寻址，多出来的对是 0。

六种定义不看单对、看物项：符号端所属的物项（端口的父项；`.BPT` 和 OPC 是自己）在同一条管段 / 信号线的连接行里两侧都出现过，这条物项与该段的每一对都是 `TapOrFitting`；只在起始侧是 `End1Conn`，只在结束侧是 `End2Conn`。I井场 738 / 738，错 0。

## 行格式

解码从 `examples/probe_oracle_exp_rows.rs` 拷来，口径见 `docs/analysis/n5-exp-rows.md`。`APPLICATIONS` 第一行是 `ID=1`、`NAME=SmartPlant PID`、`SHORTNAME=SPPID`，一共 5 行。钉不住探针就退出 1。字符列是字符集 2000、形式 2（AL16UTF16，UTF-16BE）。`0xFFFE` 是 NULL，`0xFFFF` 是表尾。

`T_REPRESENTATION` 有一列字符集 873、形式 1，还有两列类型码 12（`DATE`）。`T_OPC` 的 `MATINGOPCPATH` 也是形式 1。探针把这两类读完以保持行对齐，不拿它们当 UID。

## 图纸

本图用参考根上的 `DocUID` `5633AF4121EE4F4D9FE78E90722CC050`。`T_CONNECTOR` 没有图纸列。图纸在连接行自己的表示上：`T_CONNECTOR.SP_ID` = `T_REPRESENTATION.SP_ID`，再看 `SP_DRAWINGID`。对上这张图的是 452 行，表示都找得到（`skipped_no_rep=0`）。这 452 行的 `SP_MODELITEMID` 全部落在 `T_PIPERUN` 或 `T_SIGNALRUN`。

全库 `T_CONNECTOR` 11,757 行。点号不会同时是管道点和信号点（`point_number_in_both_tables=0`）。

## T_CONNECTOR

| 项 | 数 |
|---|---:|
| 行 | 11,757 |
| 列 | 16 |
| 全空列 | 2 |

全空的是 `ENDRANGEOBJECT`、`STARTRANGEOBJECT`。其余列：

| 列 | 类型 | 非空 |
|---|---|---:|
| `SP_ID` | 字符 | 11,757 |
| `ENDANGLEINDEX` | 数 | 11,757 |
| `ENDOBJECT` | 数 | 11,018 |
| `ENDHORIZVERTFLAG` | 数 | 11,757 |
| `MINIMUMSEGMENTLENGTH` | 数 | 11,757 |
| `RANGECLEARANCE` | 数 | 11,757 |
| `STARTANGLEINDEX` | 数 | 11,757 |
| `STARTOBJECT` | 数 | 10,748 |
| `STARTHORIZVERTFLAG` | 数 | 11,757 |
| `SP_CONNECTITEM1ID` | 字符 | 10,729 |
| `SP_AVOIDITEM1ID` | 字符 | 7,052 |
| `SP_CONNECTITEM2ID` | 字符 | 10,969 |
| `SP_AVOIDITEM2ID` | 字符 | 6,650 |
| `ISZEROLENGTH` | 数 | 11,757 |

五个字符列都是字符集 2000、形式 2。非空按单元格不是 `0xFFFE` 计。标识列是 `SP_ID` 和四个 `*ITEM*ID`。`SP_AVOIDITEM1ID` / `SP_AVOIDITEM2ID` 有值时与对应的 `SP_CONNECTITEM*ID` 相同，拼的时候不用它们。

跟着打开的表：`T_REPRESENTATION` 42,540 行，`T_PIPINGPOINT` 9,628 行，`T_SIGNALPOINT` 22,057 行，`T_PIPERUN` 4,686 行，`T_SIGNALRUN` 1,001 行，`T_OPC` 952 行。

## 同一行怎么拼

发布 UID 不在 `T_CONNECTOR` 的格子里。同一行的字符列同时等于参考 `UID1` 和 `UID2` 的，738 条里是 0。

一条连接行拼出的是「符号端 + 这条线的管段」。管段端是 `T_CONNECTOR.SP_ID` 那条表示的 `SP_MODELITEMID`：在 `T_PIPERUN.SP_ID` 里就是管道连接器，在 `T_SIGNALRUN.SP_ID` 里就是信号连接器。

符号端是 `SP_CONNECTITEM1ID`（配 `STARTOBJECT`，起始侧）或 `SP_CONNECTITEM2ID`（配 `ENDOBJECT`，结束侧）。这个 Id 是 `T_REPRESENTATION.SP_ID`。它的 `SP_MODELITEMID` 再往下：

- 物项在 `T_PIPERUN` 或 `T_SIGNALRUN`：发布 UID 是表示 Id 加 `.BPT`（分支点）。
- 物项在 `T_OPC`：发布 UID 是 `min(SP_ID, SP_PAIREDWITHID)`。见下一节。
- 否则点号就是这一侧的 `STARTOBJECT` 或 `ENDOBJECT`（大于 0 才算）。对上 `T_PIPINGPOINT` 的 `SP_PLANTITEMID` + `PIPINGPOINTNUMBER`，或 `T_SIGNALPOINT` 的 `SP_PLANTITEMID` + `SIGNALPOINTNUMBER`。发布 UID 是父项 Id、一个点、点号。

有序：符号端 = 参考 `UID1`，管段端 = 参考 `UID2`。对调：反过来。同一对若再出现在另一行，只计一次。`opc_min` 下这种重复是 92 次，集合仍是 738，多出来的对是 0。

`ENDANGLEINDEX` / `STARTANGLEINDEX` 不拿来当点号。

## 六种关系

| 关系 | 参考 | 有序 | 对调 | 两端可寻址 |
|---|---:|---:|---:|---:|
| `PipingTapOrFitting` | 369 | 369 | 0 | 369 |
| `PipingEnd1Conn` | 129 | 129 | 0 | 129 |
| `PipingEnd2Conn` | 143 | 143 | 0 | 143 |
| `SignalEnd1Conn` | 38 | 38 | 0 | 38 |
| `SignalEnd2Conn` | 46 | 46 | 0 | 46 |
| `SignalTapOrFitting` | 13 | 13 | 0 | 13 |
| 合计 | 738 | 738 | 0 | 738 |

两端可寻址不要求同一行：端口是点表里有这个父项和点号，`.BPT` 是表示 Id 在 `T_REPRESENTATION.SP_ID`，管段在 `T_PIPERUN.SP_ID` 或 `T_SIGNALRUN.SP_ID`，OPC 在 `T_OPC.SP_ID`。缺 0。

## 两端落在哪

符号端是参考的 `UID1`，管段端是 `UID2`。下面是有序命中的落点。

| 关系 | 符号端 | 管段端 | 条数 |
|---|---|---|---:|
| `PipingTapOrFitting` | `T_PIPINGPOINT` 父项 + `PIPINGPOINTNUMBER` | `T_PIPERUN.SP_ID` | 258 |
| `PipingTapOrFitting` | `T_REPRESENTATION.SP_ID` + `.BPT` | `T_PIPERUN.SP_ID` | 83 |
| `PipingTapOrFitting` | `T_SIGNALPOINT` 父项 + `SIGNALPOINTNUMBER` | `T_PIPERUN.SP_ID` | 28 |
| `PipingEnd1Conn` | `T_PIPINGPOINT` 父项 + `PIPINGPOINTNUMBER` | `T_PIPERUN.SP_ID` | 42 |
| `PipingEnd1Conn` | `T_REPRESENTATION.SP_ID` + `.BPT` | `T_PIPERUN.SP_ID` | 48 |
| `PipingEnd1Conn` | `T_SIGNALPOINT` 父项 + `SIGNALPOINTNUMBER` | `T_PIPERUN.SP_ID` | 38 |
| `PipingEnd1Conn` | `T_OPC` 一对 Id 里较小的 | `T_PIPERUN.SP_ID` | 1 |
| `PipingEnd2Conn` | `T_PIPINGPOINT` 父项 + `PIPINGPOINTNUMBER` | `T_PIPERUN.SP_ID` | 40 |
| `PipingEnd2Conn` | `T_REPRESENTATION.SP_ID` + `.BPT` | `T_PIPERUN.SP_ID` | 69 |
| `PipingEnd2Conn` | `T_SIGNALPOINT` 父项 + `SIGNALPOINTNUMBER` | `T_PIPERUN.SP_ID` | 33 |
| `PipingEnd2Conn` | `T_OPC` 一对 Id 里较小的 | `T_PIPERUN.SP_ID` | 1 |
| `SignalEnd1Conn` | `T_SIGNALPOINT` 父项 + `SIGNALPOINTNUMBER` | `T_SIGNALRUN.SP_ID` | 33 |
| `SignalEnd1Conn` | `T_REPRESENTATION.SP_ID` + `.BPT` | `T_SIGNALRUN.SP_ID` | 5 |
| `SignalEnd2Conn` | `T_SIGNALPOINT` 父项 + `SIGNALPOINTNUMBER` | `T_SIGNALRUN.SP_ID` | 40 |
| `SignalEnd2Conn` | `T_REPRESENTATION.SP_ID` + `.BPT` | `T_SIGNALRUN.SP_ID` | 6 |
| `SignalTapOrFitting` | `T_REPRESENTATION.SP_ID` + `.BPT` | `T_SIGNALRUN.SP_ID` | 9 |
| `SignalTapOrFitting` | `T_SIGNALPOINT` 父项 + `SIGNALPOINTNUMBER` | `T_SIGNALRUN.SP_ID` | 4 |

管道三种关系的管段端都在 `T_PIPERUN`。信号三种都在 `T_SIGNALRUN`。信号端口也可以接到管段上（`PipingTapOrFitting` 28、`PipingEnd1Conn` 38、`PipingEnd2Conn` 33），所以「信号端口」不等于「信号关系」。

## OPC 这两条

全库只有这两条参考端是 `PIDBranchPoint`，都接到管段。探针把三种取法都打了出来（别的端不变）：

| 取法 | 拼出的对 | 多 | 相对参考 |
|---|---:|---:|---|
| 表示上的物项 Id | 738 | 1 | 737 中、缺 1 |
| `min(SP_ID, SP_PAIREDWITHID)` | 738 | 0 | 738 中 |
| `max(SP_ID, SP_PAIREDWITHID)` | 738 | 2 | 736 中、缺 2 |

缺的那一条，表示的物项 Id 是配对的另一端；参考 UID 是这一对里字典序较小的 `SP_ID`。样本只有 2。上面的有序表用的是较小的那个。较大的那个两条都对不上。

## 侧别分得开 End，分不开 Tap

起始侧是 `SP_CONNECTITEM1ID` + `STARTOBJECT`，结束侧是 `SP_CONNECTITEM2ID` + `ENDOBJECT`。同一对可以在多行里换侧，所以下表按「只在一侧 / 两侧都出现过」计。

| 关系 | 只在起始侧 | 只在结束侧 | 两侧都有 |
|---|---:|---:|---:|
| `PipingTapOrFitting` | 143 | 143 | 83 |
| `PipingEnd1Conn` | 129 | 0 | 0 |
| `PipingEnd2Conn` | 0 | 143 | 0 |
| `SignalEnd1Conn` | 38 | 0 | 0 |
| `SignalEnd2Conn` | 0 | 46 | 0 |
| `SignalTapOrFitting` | 2 | 2 | 9 |

`PipingEnd1Conn` 和 `SignalEnd1Conn` 只在起始侧。两种 `End2Conn` 只在结束侧。两种 Tap 两侧都有。`T_CONNECTOR` 没有 DefUID 列。侧别留得住 End1 / End2 的方向，留不住 Tap。

侧别也分不出「这一对是 End 还是 Tap」。只在起始侧的，管道是 `PipingEnd1Conn` 129 加上 `PipingTapOrFitting` 143，信号是 `SignalEnd1Conn` 38 加上 `SignalTapOrFitting` 2。只在结束侧的，管道是 `PipingEnd2Conn` 143 加上 `PipingTapOrFitting` 143，信号是 `SignalEnd2Conn` 46 加上 `SignalTapOrFitting` 2。两侧都出现过的 92 条全是 Tap（管道 83、信号 9），End 是 0。那是唯一能单靠侧别定死的集合。

因此 10-12 这轮不写 `<Rel>`。Tap 先留着，End1 / End2 又没有一条不把 Tap 卷进来的规则。`ISZEROLENGTH`、角度、表示类型、符号种类都试过，没有一列能把 End 从 Tap 里挑干净。按单对的侧别分不开；按物项分得开，见下一节。

## 定 DefUID：看物项是不是在段的两侧（2026-10-13）

上一节按「这一对」数侧别。换成按「这一对的物项」数：端口 `PARENT.N` 的物项是 `PARENT`，`.BPT` 和 OPC 是自己。同一物项、同一段，把它名下所有对的侧别并起来：

- 两侧都出现过 → 这条物项与该段的**每一对**都是 `TapOrFitting`。段从物项中间穿过去：在线阀门的 `.1` 是上一条连接行的 `ENDOBJECT`、`.2` 是下一条的 `STARTOBJECT`，两个点各只在一侧，但物项在两侧；分支点一个点就在两侧。
- 只在起始侧 → `End1Conn`；只在结束侧 → `End2Conn`。
- 管道 / 信号按段落在 `T_PIPERUN` 还是 `T_SIGNALRUN`。

I井场对参考（`cargo run --example probe_oracle_exp_connectivity` 的 `rule=item_both_sides_is_tap` 段）：

| 关系 | 参考 | 对 | 错 |
|---|---:|---:|---:|
| `PipingTapOrFitting` | 369 | 369 | 0 |
| `PipingEnd1Conn` | 129 | 129 | 0 |
| `PipingEnd2Conn` | 143 | 143 | 0 |
| `SignalEnd1Conn` | 38 | 38 | 0 |
| `SignalEnd2Conn` | 46 | 46 | 0 |
| `SignalTapOrFitting` | 13 | 13 | 0 |
| 合计 | 738 | 738 | 0 |

物项组的形状也闭合：只在结束侧、1 个点的组 189 = `PE2` 143 + `SE2` 46；只在起始侧、1 个点的组 167 = `PE1` 129 + `SE1` 38；两侧、1 个点的组 92 = 分支点 Tap（管道 83 + 信号 9）；两侧、2 个点的组 145 = 在线物项，290 对 = `PTF` 286 + `STF` 4。上一节「只在起始侧 143 / 只在结束侧 143」的 Tap 正是这 145 个在线物项各自的两个点（其中 2 个是信号）。

这和 IDA 那条岔路对得上：`sub_5F5E7A80` 问连接对象（虚表 `+50h`）要一个 16 位端序号，`0xFFFF` 取 Tap 名、否则取 End 名。段穿过的物项不在段的端点上，端序号就是 `0xFFFF`；在端点上的物项，起始侧是 End1、结束侧是 End2。库里没有一列直接存这个端序号，但「物项是否在该段两侧都出现」就是它。

样本只有 I井场一张图；另外 4 张图不在这份 dump 里。规则里没有用到 `ISZEROLENGTH`、角度和表示类型。

## IDA

库是 n2a 那份 `EFAdapter.dll`（`D:\ida_scratch\publish-native\efadapter-uid\EFAdapter.dll.i64`）。这次用 `ida-bridge exec-idb`，`--skip-initial-auto-analysis`，没有 `--save`。

`ConstructUIDForRelationship`（`sub_5F5D0280`，`0x5F5D0280`）只拼关系 UID。定义名是参数，它不选 End 还是 Tap。六个名字的登记在 `FillUpRelDefArray`（`sub_5F5D0B50`），和 n2a 一样。

选名字的是 `sub_5F5E7A80`（入口 `0x5F5E7A80`，IDA 里还没标成函数）：

- 正在发的列是 `SP_ConnectItem1ID`：End 名是 `*End1Conn`，点号列名抄成 `StartObject`。否则 End 名是 `*End2Conn`，点号列名抄成 `EndObject`。
- 管道还是信号看对象偏移 `+34h`：`1` 是管道，`2` 是信号，其余走风管。Tap 名按同一个值写成 `PipingTapOrFitting` 或 `SignalTapOrFitting`。
- 表示类型 `13`（`0x0D`，普通符号）和 `46`（`0x2E`，分支点）两条路最后都做同一次比较：某个 16 位值等于 `0xFFFF` 就用 Tap 名，否则用 End 名。比较在 `0x5F5E8E2E` 和 `0x5F5E8925`。这个字是连接对象虚表 `+50h` 的返回值低 16 位（`0x5F5E8CA3`、`0x5F5E8772`），不是 `ConstructUIDForRelationship` 算出来的。

这和 dump 对得上的部分：起始侧对 End1、结束侧对 End2；管段表对管道、信号线表对信号。对不上的部分：`0xFFFF` 还没有落成某一列。本图 `STARTOBJECT` 空的只有 48 侧、`ENDOBJECT` 空的只有 26 侧，而 Tap 有 382 条，空值盖不住。点号列本身又被用来拼端口 UID，Tap 上大多不是空。所以 IDA 把岔路口指出来了，对应的库字段还没钉死，10-12 这轮仍然不写 `<Rel>`。（10-13：这个端序号不在某一列里，是「物项在该段两侧是否都出现」，见「定 DefUID」一节。）

n2a 的 `ConstructUIDForOPC`（`0x5F5CFFD0`）写的是取字典序较大的 Id。本页这两条连通按较大的缺 2，按较小的缺 0。两端仍按较小的拼，不把 n2a 那句改过来。

## 缺的是什么

738 条的两端都不缺，对调也不缺，没有多出来的对。10-12 缺的是类型规则——不能把起始侧一律写成 `End1Conn`，因为 Tap 也在起始侧；10-13 按物项定了（上一节），不再缺。

还没发的是两条 OPC 端的关系（`PE1` 1、`PE2` 1）：符号端是 OPC 的 `PIDBranchPoint`，本导出不发这个对象（N4-3 起就不发），两端不全在图里的关系不写。

## 导出器

`connectivity_ends_for_drawing` 按上面的拼法把两端挂到 `PidPublishDrawing::connectivity_ends`（`symbol_uid`、`run_uid`、这对出现在起始侧还是结束侧，以及按「定 DefUID」一节定下的 `def_uid`）。`--oracle-exp` 会读。

`pid_writer` 把两端都是本图已发出对象的写成 `<Rel>`：`UID1` 是符号端，`UID2` 是段，UID 是 `PE1-` / `PE2-` / `PTF-` / `SE1-` / `SE2-` / `STF-` 加两端（`FillUpRelDefArray` 的表，n2a 页第 2 问；`SE1` / `SE2` / `STF` / `ITN` 这次补进 `defuid_prefix`）。覆盖报告：`relationships` 下按六种定义计数；`oracle_exp.connectivity_ends` 是拼出的对数，`connectivity_rels` 是写出的条数，`connectivity_ends_left_out` 是一端没发出而留下的。

### 导出器实测（2026-10-13）

`pid_publish_xml <I井场.pid> --oracle-exp Export.dmp --diff-against <参考 _Data.xml> --plant P01` 退出 0，clean 行 `3012 objects, 3182 values, 3984 rels`，extra / wrong class / different / not in reference / rels not in reference 都是 0。

- 拼出 738 对，写出 736 条，留下 2 条（OPC 端）。写出：`PTF` 369、`PE1` 128、`PE2` 142、`SE1` 38、`SE2` 46、`STF` 13。
- I井场 exp 模式关系 3 248 → 3 984；对象 3 012、值 3 182 不动。`.pid` 模式五组钉数不动。
- 库测 `ijingchang_connectivity_ends_match_the_reference_pairs`：738 对的 `def_uid` 逐条等于参考，六种计数 369 / 129 / 143 / 38 / 46 / 13。

## 复现

```powershell
cargo run --example probe_oracle_exp_connectivity
```

可选两个参数：dump、参考 `_Data.xml`。缺省就是上面这两个路径。
