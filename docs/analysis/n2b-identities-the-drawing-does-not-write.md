# 图纸写出器不写管线、端口、工艺点、分支点的身份（2026-10-11）

> 计划 `docs/plans/2026-10-11-publish-native-evidence-via-ida-bridge.md` 的 N2b 第 2 问。任务 t-1065。Q1 (b)。
> 问题：10-10 分析页说 `.pid` 的原始字节里搜不到管线、端口、工艺点、分支点的身份。这是「写出器没写」，还是「写了但没搜到」？
> 结论类型：**native-reader**。IDA 9.2（`D:\IDA Professional 9.2`），`ida-bridge`，库只开本问自己的副本。`src` 未改。测试 NOT RUN：本单只新建这一页，没有跑 cargo。

## 一句话

写出器不写这四类的发布身份。`pidobjmgrai.dll` 里唯一把 `ModelItemType` / `ModelID` 写进 `P&IDAttributes` 的函数是 `SPItemHelper::CreatePidAttributeSet`（`sub_572BCD13`）。它不拼 `.PPT` / `.BPT` / `.1`，模块里也没有这些字面量。`PipingPoint` / `SignalPoint` 只出现在连接点 SP 项的创建函数里，那两个函数不引用 `P&IDAttributes` 或 `ModelID`。`Pipeline` 有一处 UTF-16 字面量，代码引用是 0。

对照的是 `docs/analysis/2026-10-10-publish-from-the-pid-alone.md`：端口、工艺点、分支点的 UID 是「父 UID + 后缀」，整串不在文件里；管线是普通 GUID，整串也不在。本页把这件事升格为「图纸写出器不写」。publish 侧把它们拼成 `IObject` 的代码在 `EFAdapter.dll`，不在本模块（见 `docs/analysis/2026-10-11-the-publish-composer-is-efadapter-and-it-is-not-on-hand.md`）。

## 库

| 项 | 值 |
|---|---|
| 模块 | `pidobjmgrai.dll`（镜像基址 `0x57090000`） |
| 原件 | `D:\work\plant-code\cad\pid-parse\dlls\pidobjmgrai.dll` |
| 原件 SHA-256 | `E7216143BECD41A82F7263210C00ECEE4BF5BF1F3F9646541BCF60BEA6120F75`（建库前、建库后各算一次，与副本一致；`dlls/` 原件和已有的 `pidobjmgrai\` 目录都没改） |
| 本问副本 | `D:\ida_scratch\publish-native\pidobjmgrai-q2\pidobjmgrai.dll` |
| 库 | `D:\ida_scratch\publish-native\pidobjmgrai-q2\pidobjmgrai.i64` |
| IDA | 9.2 |
| 串表 | `strlist_unicode.py` 重建后 2858 条（8-bit + UTF-16LE，最短 4） |
| 自己开的实例 | `idalib-38296`，问完已 `supervisor stop`。`plant3\core.dll.i64` 与 `Core3D.dll.i64` 没停 |

文件偏移 = 虚拟地址 − `0x57090000`。

## 结论

| 身份 | 写出器 | 函数 | 地址 | 依据 |
|---|---|---|---|---|
| 管线（`PIDPipeline` 的 GUID） | 不写 | 无代码引用。字面量 `Pipeline` 在 `0x570B72AC`（文件 `0x272AC`），`XrefsTo` = 0 | `0x570B72AC` | 串表里没有名为 `Pipeline` / `PipeLine` 的可引用串；字节扫描只此一处，且没有指令引用它 |
| 端口（`PIDPipingPort` / `PIDSignalPort`，后缀 `.1`、`.2`） | 不写 | `ISPItemMgrHelper_CreateConnectionPointSPitems` = `sub_571E9442`；`ISPItemMgrHelper_CreatePointSPItem` = `sub_571EDD2C` | `0x571E9442`，`PipingPoint` 拷贝在 `0x571E9B52`；`0x571EDD2C`，与 `SignalPoint` 比较在 `0x571EDD9B` | 全文件没有 `PipingPort`、`SignalPort`、`.PPT`、`.BPT`。这两个函数的串是 `Type`、`_ConnectPoint`、`ForeignKey`、`PipingPoint`、`SignalPoint`、`DuctingPoint`，没有 `P&IDAttributes` / `ModelID` |
| 工艺点（`PIDProcessPoint`，后缀 `.PPT`） | 不写 | 无此字面量，因此没有函数 | — | 对副本做 UTF-16 / ASCII 扫描，`ProcessPoint` 与 `.PPT` 都是 0 处 |
| 管道/信号分支点（后缀 `.BPT`） | 不写 | `_BranchPoint` 只是后期绑定的成员名，加载点在 `sub_571C3CB6` 的 `0x571C6FD9` | `0x571C6FD9` | 全文件没有 `.BPT`。该函数调用属性集写出器时，四个身份参数都是空串（见下），不是分支点 UID |

OPC（`PIDBranchPoint` 里文件中找得到的那一部分）不在这四类里。10-10 写的是 OPC 的 `ModelID` 有时对不上发布物项；本问不把它升格成「不写」。

`PipeRun` / `SignalRun` / `Instrument` / `Nozzle` 这些**文件里找得到**的 `ModelItemType` 字面量都在串表里，而且有代码引用。缺的是上面四类，不是「这个 DLL 不写任何 `ModelID`」。

## 写出 `ModelID` 的函数

`ModelItemType` 这个字段名只有三个函数引用：

| 函数 | 地址 | 引用的串 |
|---|---|---|
| `sub_572BC552` | `0x572BC552` | 只有 `ModelItemType` |
| `SPItemHelper::CreatePidAttributeSet` = `sub_572BCD13` | `0x572BCD13` | `P&IDAttributes`、`ProjectNumber`、`DrawingNo`、`DrawingItemType`、`DrawingID`、`ModelItemType`、`ModelID`、`Flag`、`AttributeSets`、`Application`、`RADApplication`。错误串 `SPItemHelper::CreatePidAttributeSet` 的引用在本函数内 `0x572BE062` |
| `sub_572BE0FD` | `0x572BE0FD` | 同上这组字段名，没有类型值 |

`CreatePidAttributeSet` 把调用方给的四个 BSTR 写进属性集：`*arg` → `DrawingItemType`，下一个 → `DrawingID`，下一个 → `ModelItemType`（存储点 `0x572BD5CA`），再下一个 → `ModelID`。`Flag` 写 0。函数体里没有 `Pipeline`、`PipingPoint`、`ProcessPoint`、`BranchPoint`、`.PPT`、`.BPT`。

`sub_5721C7AF` 在 `0x57220F1D` 调用它，调用点前面的字面量是 `Drawing`、`SP_DrawingID`、`P&IDAttributes`。这是在写图纸自己的身份，不是管线。

`sub_571C3CB6` 也调用它（`0x571C6EA6`、`0x571C9BDC`）。两次都是把 `dword_570B4BC4` 用 `__vbaStrCopy` 复制四份再传入。该地址开头 8 个字节是 0（419 处引用，VB6 的空串槽），不是管线或分支点的 UID。紧接着 `0x571C6FD9` 才把成员名 `_BranchPoint` 放进后期绑定调用。`_BranchPoint` 是成员名，不是写进 `ModelItemType` 的值。

提到 `PipingPoint` 或 `SignalPoint` 的函数，和提到 `P&IDAttributes` 或 `ModelID` 的函数，集合不相交。连接点创建不走这条属性集写出器。

## 复现

在 `D:\ida_scratch\publish-native` 下。不要打开 `pidobjmgrai\`（那是别的问的副本），不要 `--stateful`，不要 `stop` `idalib-29552` / `idalib-40832`。

```text
ida-bridge exec-idb --input pidobjmgrai-q2\pidobjmgrai.dll --out-idb pidobjmgrai-q2\pidobjmgrai.i64 --save --wait-s 900 --file strlist_unicode.py
ida-bridge supervisor start-idalib --idb pidobjmgrai-q2\pidobjmgrai.i64
ida-bridge exec <cid> --sql "SELECT address, string_value FROM strings WHERE string_value IN ('P&IDAttributes','ModelID','ModelItemType','PipingPoint','SignalPoint','PipeRun')"
ida-bridge exec <cid> --sql "SELECT string_value, func_name, func_addr, ref_addr FROM string_refs WHERE string_value = 'ModelItemType'"
ida-bridge exec <cid> --timeout-s 180 --sql "SELECT decompile(0x572BCD13) AS t"
ida-bridge exec <cid> --sql "SELECT string_value FROM string_refs WHERE func_addr = 0x571E9442"
ida-bridge exec <cid> --sql "SELECT string_value FROM string_refs WHERE func_addr = 0x571EDD2C"
ida-bridge supervisor stop <cid>
```

`<cid>` 用 `start-idalib` 印出的 `client_id`。本轮是 `idalib-38296`，已经停掉。

字节扫描（确认 `.PPT` / `ProcessPoint` / `PipingPort` 不在文件里）对副本做即可，不必入库。原件 SHA-256 用 `Get-FileHash -Algorithm SHA256` 对 `dlls\pidobjmgrai.dll` 再算，应仍是上面那一串。

## N4-2 再量了一次图纸

`examples/probe_n4_ports_and_process_points.rs` 扫了五张 `.pid` 的全部流。`PipingPointNumber`、`SignalPointNumber`、`FlowDirection`、`SignalPoint` 的 ASCII 和 UTF-16 都是 0 处，和本页「连接点创建不写进 `P&IDAttributes`」一致。工艺点的 `.PPT` 也不在文件里：发布 UID 是管段自己的 `ModelID` 加后缀，不是图纸上的一个字段。数和对照见 `docs/analysis/n2a-port-relation-filter.md` 的 N4-2 一节。
