# `pidobjmgrai` 用一个函数写 `P&IDAttributes` 的四个身份字段，OPC 的 `ModelID` 就是物项自己的 Id（2026-10-11）

> 计划 `docs/plans/2026-10-11-publish-native-evidence-via-ida-bridge.md` N2b 第 1 问（Q1 (b)）。任务 t-1064。
> 结论类型：**native-reader**。IDA 9.2，`ida-bridge`，库只开临时副本。`src` 未改，未提交。测试 NOT RUN：本单只新建这一页。
> 原件与副本 SHA-256 相同，建库前后再算一次原件，没有变：`E7216143BECD41A82F7263210C00ECEE4BF5BF1F3F9646541BCF60BEA6120F75`。
> 模块 `pidobjmgrai.dll`（2,486,317 字节，文件时间 2017-08-30）。副本 `D:\ida_scratch\publish-native\pidobjmgrai\pidobjmgrai.dll`，库 `pidobjmgrai.dll.i64`。镜像基址 `0x57090000`，2,673 个函数。自己开的实例 `idalib-10564` 做完已停。`idalib-29552` / `idalib-40832`（`plant3`）和别人的 `pidobjmgrai-q2` 没停。

## 一句话

`ModelID`、`ModelItemType`、`DrawingID`、`DrawingNo` 不是按类各写一份。`sub_572BCD13` 对每张图上的图形找或建名为 `P&IDAttributes` 的属性集，然后把这四个字段写进去。`DrawingNo` 它自己向图纸对象要；另外三个加上 `ModelItemType` 是调用方递进来的字符串。模块里没有字面量 `OPC`。OPC 行的 `ModelID` 走的是同一条「物项 Id」槽，不是 publish 后来那个分支点对象，所以语料里 53 个里会有 23 个对不上参考。

## 写入器

`sub_572BCD13`（`0x572BCD13`，5,098 字节）反编译后两段是对称的：属性集已经在，就按名取出；不在，就用 late id `2` 建一个名为 `P&IDAttributes` 的集（串 `0x570B55D0`）。然后对下面每个名字再取属性并 `_vbaLateIdSt`：

| 属性 | 串地址 | 值从哪来 |
|---|---|---|
| `ProjectNumber` | （函数内的宽字符串） | 对象虚表 `+180` 再 `+152` |
| `DrawingNo` | `0x570B9470` | 对象虚表 `+188` 拿到图纸，再对图纸虚表 `+92`（接口 `dword_570B9EF8`）取字符串 |
| `DrawingItemType` | `0x570B9640` | 参数 `a7` |
| `DrawingID` | `0x570B5608` | 参数 `a8` |
| `ModelItemType` | `0x570B9BE0` | 参数 `a5` |
| `ModelID` | `0x570B55F4` | 参数 `a6` |
| `Flag` | （函数内的宽字符串） | 整数 `0` |

读回函数是 `sub_572BE0FD`（`0x572BE0FD`，1,268 字节）。它按同样的名字把值读进出参：`a5`=`ProjectNumber`，`a6`=`DrawingNo`，`a7`=`ModelItemType`，`a8`=`ModelID`，`a9`=`DrawingItemType`，`a10`=`DrawingID`。字段集合两边一致。

调用 `sub_572BCD13` 的有 12 个函数、17 处。能从旁边的日志串认出来的：

| 函数 | 地址 | 日志串 | 这一处在写什么 |
|---|---|---|---|
| `sub_572686DC` | `0x572686DC` | `PersistSPItem` / `PidObjMgrAddIn.AddInClass::PersistSPItemHelper` | 两处调用（`0x5726C90E`、`0x5726CEF7`）。落盘一个 SPItem 时写这四个字段 |
| `sub_57276B47` | `0x57276B47` | `PidObjMgrAddIn::DoReplace` | 替换图形时再写一次 |
| `sub_571C3CB6` | `0x571C3CB6` | `CloneGraphicCollection` | 克隆。其中 `0x571C6EA6` 把四个字符串参数都从 `dword_570B4BC4` 拷空串再调用，这一支不填身份 |
| `sub_57117D28` | `0x57117D28` | （函数内有 `Symbol`、`GraphicOID`、`SP_ModelItemID`） | 符号表示 |

其余调用方：`sub_5711EF77`、`sub_57120AE4`、`sub_57138F73`、`sub_571558BC`、`sub_571C3558`、`sub_571D0275`、`sub_571D58B2`、`sub_5721C7AF`。它们递的是运行时字符串，函数体内没有 `Vessel` / `PipeRun` 这种类名。

所以「哪些类写这四个字段」的答案是：类不在这个函数里分支。任何走到 `PersistSPItem` 或 `DoReplace` 的图形都用这一个函数写。类的差别在调用方递进来的 `ModelItemType` 字符串。

## `ModelItemType` 取值

写入器内部没有取值表。本模块里成组出现这些名字的是 `sub_572BEABE`（`0x572BEABE`，35,659 字节）。它按名字把关系指到 `SP_PipeRunID`、`SP_EquipmentID`、`SP_PlantItemID`、`SP_SignalRunID`、`SP_RoomID` 等，**不调用** `sub_572BCD13`。它比较的名字是：

`Vessel`、`Exchanger`、`Mechanical`、`EquipmentOther`、`EquipComponent`、`Nozzle`、`PipingComp`、`PipingPoint`、`PipeRun`、`SignalRun`、`SignalPoint`、`Instrument`、`ItemNote`、`Room`、`RoomComponent`、`DuctRun`、`DuctingComp`、`DuctingPoint`。

语料里 `P&IDAttributes.ModelItemType` 的取值（`Vessel`、`EquipmentOther`、`Mechanical`、`Exchanger`、`Nozzle`、`PipingComp`、`PipeRun`、`SignalRun`、`ItemNote`、`Instrument`、`OPC`）除了 `OPC` 都在这张表里。全模块检索没有字面量 `OPC`（有的是 `OPCTag`、`PlacePartnerOPC`、`pairedWithOPC`、`SP_pairedWithID`、`MatingOPCPath`）。`OPC` 这个属性值不是本 DLL 拼出来的，是调用方递进来的物项类型名。`PersistSPItem` 在调用写入器之前读过 `PlantItemType`（`0x57269F41`）。

## OPC 的 `ModelID`

`PersistSPItem` 在 `0x57269981` 对已经 cast 到 `dword_570B9EF8` 的对象调虚表 `+92`，把返回的字符串移进后来作为 `a6` 压给写入器的槽（`var_54`，`0x572699D5`）。写入器自己用同一个槽 `+92` 向图纸对象要 `DrawingNo`。所以 `ModelID` 是这个物项对象的 Id 字符串，原样落进属性，没有再加工。

OPC 专用的两个函数都不改这个字段：

- `sub_57275FCE`（`0x57275FCE`，`UpdateOPCWithPairedwithDrawingAttribute`）只引用 `pairedWithOPC.Representation.Drawing.DrawingNumber`（`0x5727610B`、`0x572761F2`、`0x572762EF`）。它更新的是配对 OPC 所在图纸的图号，不是 `ModelID`。
- `sub_57279AAA`（`0x57279AAA`，`DoUpdateOPCItemAttributes`）把配对 OPC 的属性抄过来，见到名字 `SP_pairedWithID` 就跳过。反编译里没有 `ModelID`。

因此 OPC 行上的 `ModelID` 指的是被落盘的那个物项自己的 Id，不是 SmartPlant publish 另建的 `PIDBranchPoint`。这和 10-10 分析页的 23 / 53 对得上：53 个 OPC 行写出了 `ModelID`，其中 23 个不是参考 `_Data.xml` 里的分支点物项，因为图纸这边写的是物项 Id，publish 那边的分支点是另一套对象。本模块也不拼 `.BPT`。

## 复现

在 `D:\ida_scratch\publish-native` 下。不要 `--stateful`，不要停 `plant3` 那两个实例。

```
ida-bridge exec-idb --input pidobjmgrai\pidobjmgrai.dll --out-idb pidobjmgrai\pidobjmgrai.dll.i64 --save --wait-s 900
ida-bridge supervisor start-idalib --idb pidobjmgrai\pidobjmgrai.dll.i64
ida-bridge exec <cid> -f strlist_unicode.py
ida-bridge exec <cid> --sql "SELECT string_value, func_name, ref_addr FROM string_refs WHERE string_value IN ('P&IDAttributes','ModelID','ModelItemType','DrawingID','DrawingNo','DrawingItemType')"
ida-bridge exec <cid> --timeout-s 180 --sql "SELECT decompile(0x572BCD13) AS t"
ida-bridge supervisor stop <cid>
```

建库前对 `dlls/pidobjmgrai.dll` 和副本各算一次 SHA-256，应都是上面那一串。UTF-16 串表要用 `D:\ida_scratch\publish-native\strlist_unicode.py`（这次重建后 2,858 条）。
