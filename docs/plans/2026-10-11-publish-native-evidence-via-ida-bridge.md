# 用 ida-bridge 给 `.pid` publish 补原生证据 · 开发计划（2026-10-11 开单；同日 N0–N1 实跑后改成「`EFAdapter.dll` 不在手头」的版本）

> 承接 `docs/plans/2026-10-10-publish-from-the-pid-alone.md`：P0–P5、小单（无 `SymbolAttr` 的仪表）、待定 ② ③ 都已落地（`d1d502e` → `22b3bcb` → `a6f8699` → `685eb33` → `efae71c`），只剩 ④ `_Coverage.json` 口径等交付方。
> 用户 2026-10-11 19:5x（zhimo 会话 fable-5-1-31）：「结合 ida-bridge 制定开发计划」；随后「用 ida-bridge 分析」；再「先把现在这一轮写成分析页和计划 §7，并把计划改成『EFAdapter 不在手头』的版本」。
> 基线：pid-parse `efae71c`（`codex/phase32c-bundle-closeout`）。工作树上别的会话的 5 M + 10 ??（09-20 / 09-21 两份分析页与计划、`probe_jflavorholder_*`、10-11 的 7 份 dash-scale / units-context / line2-offset 分析页和 3 个探针）本单**一个字节不碰**。
> **2026-10-11 N0–N1 的结果改写了本计划**：publish 写出侧的发布器是 `EFAdapter.dll`，本机没有（`docs/analysis/2026-10-11-the-publish-composer-is-efadapter-and-it-is-not-on-hand.md`）。原 Q1 (a) 那一半（N2a）挂起，等文件；能做的是图纸写入侧（N2b）和把 `sppiddatamap.xml` 当出处（N2c，新加）。

## 一句话

只凭 `.pid` 的导出器今天发 2 918 / 6 131 个对象、3 389 / 8 185 条关系、2 822 个值，错 0；剩下的要么「`.pid` 里没有」（172 条管线、28 个 OPC、关系自己的 UID），
要么「可能按规则推」（2 983 个端口 / 工艺点 / 分支点的 UID 是父 UID + 后缀、管线名拼接、公称直径换算、代码表引用），而 Q2 (a) 说每条规则先要有**对照证据**。
到今天为止的证据全是从 5 份参考 XML 反推的样本规律。本计划原想用 ida-bridge 打开 SmartPlant 自己的 publish 一侧和图纸写入一侧，把「规则」换成「写出器就是这么写的」。
N0–N1 实跑后：**publish 一侧的写出器 `EFAdapter.dll` 不在手头**，`dlls/sppiddatamap.xml` 指名了每个发布类归哪个 `EFAdapter.*Publisher`；
所以剩下能做的是 (1) 图纸写入侧 `pidobjmgrai.dll`（天花板、OPC `ModelID`、多出的 UID），(2) 把 `sppiddatamap.xml` 当「按类发哪些接口 × 属性」的出处，(3) 拿到 `EFAdapter.dll` 之后再回头做 N2a。门 **G2** 照旧由你逐条定。

## 1. 现状

### 1.1 导出器还差什么（数字出自 10-10 分析页与 `_Coverage.json`）

| 档 | 项 | 数量 | 今天的依据 | 缺的证据 |
|---|---|---:|---|---|
| 可能按规则推 | 信号端口 / 管道端口 / 工艺点 / 分支点的 UID = 父 UID + `.1` `.2` / `.PPT` / `.BPT` | 2 983 个对象、约 2 500 条 `*PortComposition` / `ProcessPointCollection` / `PipingTapOrFitting` 关系 | PowerShell 在 5 份参考上逐个核过 | 后缀是谁、按什么序号拼的；端口的属性（`Name` = `1` / `2`、`PipingConnectorType`、`ConnectionFlowDirection`）从哪来 |
| 可能按规则推 | 管线名 172、管段名 219（`65 mm-AI-010901-I2[-…]`） | 391 个 `IObject.Name` | 样本看出是 公称直径-介质-流水号-等级 | 拼接是 publish 做的还是库里存好的；分段来自哪些属性 |
| 可能按规则推 | 公称直径 878 个（标签 `2"` / `200 MM` ↔ 发布 `2 in` / `200 mm`） | 878 个值 | 写法不同、数值一致 | publish 用哪张单位表格式化 |
| 可能按规则推 | 代码表属性（`@NewConstruction`、`@EE690`、`@{GUID}`） | 约 4 000 个值 | `.pid` 字段里没有这些串 | `.pid` 里是否以索引 / 别的形式存着；`@` 引用怎么拼 |
| `.pid` 里没有 | 关系自己的 UID（`{prefix}-{UID1}-{UID2}`） | 8 185 | 从参考反推，`pid_writer` 已照拼 | 写出器的拼法与 `prefix` 表，确认不是巧合 |
| `.pid` 里没有 | 172 条管线、28 个 OPC 物项；`_Meta.xml` 的版本 / 修订 / 文件对象 | — | 原始字节搜不到 | 图纸写入器是否真的不写它们（天花板从「没搜到」变成「写出器不写」） |
| 没查 | I井场被指名 UID 3 089 对发布对象 1 460，多出的是什么 | 约 1 600 | — | `P&IDAttributes` 行的写入器写了哪些类的 `ModelID` |
| 没查 | OPC 的 `ModelID` 23 / 53 不是发布的物项 | 23 | 探针 | OPC 行的 `ModelID` 指什么 |

### 1.2 手头的二进制与 ida-bridge

- `ida-bridge` CLI 已装（`C:\Users\dpc\.agents\tools\ida-bridge\.venv\Scripts\ida-bridge.exe`），bridge server 在跑，`%USERPROFILE%\.idapro\venv` 里 `import ida_bridge, idapro` 通过；IDA 9.2 在 `D:\IDA Professional 9.2`。
  `ida-bridge list` 现有两个 idalib 实例开着 `D:\ida_scratch\plant3\core.dll.i64` / `Core3D.dll.i64`——**别的会话的，不碰、不停**。
- 10-11 别的会话（dash-scale / units-context 两组分析页）的做法是本单的样板：库**只开临时副本**（`D:\ida_scratch\<单名>\…`），原 `dlls/*.i64` SHA-256 前后一致；结论标「native-reader」，每条带模块 + 地址，页尾有「复现」。
- 2026-10-11 对 `dlls/`（151 个文件）与 `D:\pid\RADInstallA~` 逐字节搜 ASCII / UTF-16LE 串（a = ASCII 命中数，w = UTF-16 命中数），publish 相关的落在这几个模块，**全是 VB6（导入 `MSVBVM60`），非托管**：

| 模块 | 大小 | 命中 | 推测角色 |
|---|---:|---|---|
| `drawingmanagerocx.ocx` / `drawingmanagerexe.exe` | 1.5 MB / 1.2 MB | `P&IDAttributes` w1、`DrawingItems` w1、`SPItems` w3、`PublishedVer`、`SchemaVersion` w1 | Drawing Manager——publish 菜单所在 |
| `sppiddwgprocess.dll` | 991 KB | `SPItems` a44 / w31、`DrawingItems` a2 / w2、`EmbeddedSPID` a1 / w2、`P&IDAttributes` w1；06-12 清单里还有 `ISPPidArchive_LoadSPItems[Ex]`、`ISPDrawingISPItems`、`strDrawingXMLFile`、`strSharedItemsXMLFile` | 从图纸 / 库装 SPItems（SmartPlant 物项）并写 Drawing XML |
| `sppidmdadapter.dll` | 124 KB | `ModelItemType` w1、`IObject` w1、`ToFromText` w1 | 元数据适配器——P&ID 类 → SmartPlant `IObject` 接口 |
| `pidobjmgrai.dll` | 2.4 MB | `P&IDAttributes` w1、`ModelItemType` w1、`DrawingItemType` w1、`SPItems` a37 / w83 | P&ID 对象管理器——最可能的 `P&IDAttributes` 写入器 |
| `llama.dll` | 7.1 MB | `P&IDAttributes` w1、`ModelItemType` a6 / w1、`SPItems` a4 / w1、`ToFromText` | 逻辑模型自动化（`LMPipeRuns` / `LMRepresentations` …） |
| `efpublishcmd.dll` / `efcompwpublishedvercmd.dll` | 32 KB / 36 KB | `SPItems` w1、`PublishedVer` a9 / w4 | 发布命令壳 |
| `pidrules.dll` / `pidauto.dll` / `sppidtypical.dll` | 600 KB / 660 KB / 124 KB | `P&IDAttributes`、`ModelItemType`、`DrawingItemType` 各 w1 | 规则 / 自动化 / 典型图，备查 |
| `isppidarchiveinf.tlb` / `sppidautomation.tlb` | — | 类型库 | `ISPPidArchive_*` 的接口签名，不用 IDA 就能读 |

- **一个都没搜到**（ASCII 与 UTF-16 都没有）：`DefUID`、`DwgRepresentationComposition`、`PipingPortComposition`、`PIDPipingPort`、`.PPT`、`.BPT`、`SymbolAttr`（只在 RAD 通用 `igr*412` 里）。
  说明关系定义名与端口后缀**不在这些二进制里以整串出现**。（N1 定位后的答案：它们在 `EFAdapter.dll` 里，见 §1.3。）
- `dlls/` 下现有 `.i64` 11 个（`core.dll` / `imagdex.dex` / `style.dll` / `radsrvitem.dll` / `ugeom2d1.dll` / `j2dsrv.dll` / `viewfil.dex` / `docext.dex` / `shlyhp.dll` / `sppid.dll` / `sppidautomation.dll`），上表的模块开单时一个都没建库；06-12 建过的 `sppiddwgprocess.dll.i64` / `llama.dll.i64` 已不在。

### 1.3 N0–N1 实跑之后（2026-10-11，分析页 `2026-10-11-the-publish-composer-is-efadapter-and-it-is-not-on-hand.md`）

- 已建库 5 个（临时副本 `D:\ida_scratch\publish-native\`，原件 SHA-256 前后一致）：`sppidmdadapter` 303 函数、`sppiddwgprocess` 481、`efpublishcmd` 87、`efcompwpublishedvercmd` 86、`drawingmanagerocx` 1 266；都是原生编译的 VB6，Hex-Rays 能反。VB6 的 UTF-16 常量要先用 `strlist_unicode.py` 重建串表。`pidobjmgrai.dll` / `drawingmanagerexe.exe` 已拷副本、未建库。
- `sppidmdadapter.dll` 是 Schema Editor 的元数据适配器（数据字典 → `SPPIDDataMap.xml`），不是发布器。
- **`dlls/sppiddatamap.xml`**（3.3 MB，2017-07；旁边 14 个历史版本）是 SmartPlant P&ID 的 map schema：64 个 `SPMapClassDef`、2 752 个 `SPMapPropertyDef`、194 个枚举表（空壳）、30 个单位表（空壳）、2 个 `SPMapRelDef`。每个类的 `ProcessClassCriteria` 指名发布器：`EFAdapter.PipingPointPublisher`（管道端口 / 工艺点）、`SignalPointPublisher`（信号端口）、`PpBranchPtPublisher` / `SigBranchPtPublisher`（分支点）、`PipeLinePublisher`（管线）、`RepresentationPublisher`、`OPCPublisher`、`InstrPublisher`、`NozzlePublisher`、`NotePublisher`、`*DataExchanger`；枚举属性 `ProcessPropertyCriteria="EFAdapter.EnumConversions"`。
- **`EFAdapter.dll` 不在手头**：`dlls/`、`D:\pid` 没有，注册表里没装 SmartPlant P&ID；13 个 VB6 模块里 `EFAdapter` 各出现 1 次，都是 `CreateObject("EFAdapter.Adapter")` 的 ProgID。
- `sppiddwgprocess.dll` 的 `ISPPidArchive_LoadSPItems`（`sub_547B8B70`）是从库（`igrCLientDa412.SPFactory`）或归档 XML **装载** SPItems；`WSPublish` 是 Workshare 站点间发布。都不是 SPF publish 的写出器。

## 2. 范围

**做**：N0 建库（临时副本，已做 5 个）；N1 定位（已做，publish 侧定位到 `EFAdapter.dll`）；N2b 图纸写入侧一页分析；N2c `sppiddatamap.xml` 一页分析（新加）；N2a publish 写出侧**挂起**，拿到 `EFAdapter.dll` 再做；G2 门；G2 选中的规则逐条开小单（N4），每条照「小单：无 `SymbolAttr` 的仪表」的套路：探针 → 装载 / 写出 → 重钉 → 全量门 → 重新交付 → 按明确路径提交。
**不做**：往 `.pid` 写回；语料以外的图；SmartPlant Foundation 那一侧（SPF 服务、EF 框架、CMF）；库路（Backup Store）；`_Coverage.json` 口径（④ 另等交付方）；给 `dlls/` 新增 `.i64`（见 Q3）；去别的机器找 `EFAdapter.dll`（那是你的事，见 Q5）。

## 3. 要你拍板的

| # | 问题 | 选项 | 推荐 | 状态 |
|---|---|---|---|---|
| Q1 | 先摸哪一侧 | (a) **publish 写出侧**；(b) **图纸写入侧**：`pidobjmgrai.dll`（必要时 `llama.dll`），回答 §1.1 后三行（天花板、多出的 UID、OPC `ModelID`）；(c) 两侧并行 | 原推荐 (a) 再 (b)。**(a) 已做到头**：写出器不在手头。现在只剩 (b) 和新加的 N2c | (a) 做完定位；(b) 待开 |
| Q2 | 有了原生证据后，哪些规则**准许**升格为小单（G2 时再逐条确认，这里先定原则） | (a) 只要写出器的拼法**完全由 `.pid` 里有的值决定**（如端口 UID = 父 UID + 后缀、`Name` = 序号）就准发；(b) 再加「值在库里但 5 图取值唯一」的默认值（`IsTypical` = `False`、`InstrumentLocation` = `@EE31D`）；(c) 一条都不发，只写分析页 | **(a)**：和 10-10 的 Q2 (a) 一脉相承，「出处」从「字节在文件里」放宽到「写出器从文件里的值确定地算出来」；(b) 不要——那是猜 | 待答（N2a 挂起期间不急） |
| Q3 | 库文件放哪 | (a) `D:\ida_scratch\publish-native\<模块>\<模块>.i64`，仓库只记原二进制 SHA-256、IDA 版本、地址；(b) 新 `.i64` 入 `dlls/` | **(a)** | 已按 (a) 做 |
| Q4 | 关键串若最终不在这些二进制里 | (a) 分析页记「不在原生侧」，给出去哪找，本单到此；(b) 继续：I井场 SQPlant 备份里查元数据表，或 dnSpy 开托管程序集 | **(a)** | **已发生**，按 (a) 记在分析页 §3 / §5 |
| Q5 | `EFAdapter.dll` 要不要去找 | (a) 从装了 SmartPlant P&ID 的机器（参考 XML 来自 `\\Glpid-7\pidsq\PID`）拷 `…\SmartPlant\P&ID Workstation\Program\EFAdapter.dll`（及 `EFCommon*`、Schema Component 的 `*.cmf`）到 `dlls/`，N2a 照计划做；(b) 不找，publish 侧到此，导出器维持 Q2 (a) 的口径 | **(a)**：2 983 个对象、约 2 500 条关系都压在它上面；只是拷文件 | 待你办 |
| Q6 | N2c 要不要做 | (a) 做：把 `sppiddatamap.xml` 的「类 → 接口 × 属性」表导出，与 5 份参考 XML 的「类 × 属性」对一遍，导出器「按类发哪些接口」的依据换成它；(b) 不做，参考 XML 反推够用 | **(a)**：半天的活，不用 IDA，把今天「从答案键学的」那部分换成厂商的 schema | 待答 |

## 4. 步骤

### N0 建库（只读；`src` 不动）

- 新建 `D:\ida_scratch\publish-native\`，把 `dlls/drawingmanagerocx.ocx`、`drawingmanagerexe.exe`、`sppiddwgprocess.dll`、`sppidmdadapter.dll`、`efpublishcmd.dll`、`efcompwpublishedvercmd.dll`、`pidobjmgrai.dll`（Q1 (b) 时再加 `llama.dll`）各拷一份到各自子目录；拷贝前后对 `dlls/` 原件算 SHA-256，记入分析页。
- 一次性建库并保存：`ida-bridge exec-idb --input <副本> --out-idb <副本>.i64 --save`；`llama.dll`（9 286 个函数）与 `pidobjmgrai.dll` 用 `--wait-s 900`。
- 开持久实例做迭代：`ida-bridge supervisor start-idalib --idb <...>.i64`，记下 `client_id`；全程**无状态** `exec`（不开 `--stateful`，bridge 与别的会话共用）；做完 `ida-bridge supervisor stop <client_id>`，只停自己开的。
- 类型库不走 IDA：用 Python `comtypes`（或 `oleview`）把 `isppidarchiveinf.tlb` 的接口与方法签名导出成文本，放分析页附录——`ISPPidArchive_LoadSPItems[Ex]` 的参数名往往直接说明输入是图纸还是库。
- VB6 分流：每个库先 `SELECT COUNT(*) FROM funcs` 与 `SELECT COUNT(*) FROM strings`；函数数与镜像大小不成比例（如 1 MB 镜像只有几十个函数）即为 **p-code**，Hex-Rays 不能反编译，该模块降级为「只做串 / 引用 / 类型库」，在分析页标明。06-12 的数据：`sppiddwgprocess.dll` 481 个函数、`llama.dll` 9 286 个，是原生编译。

### N1 定位（每个库一张表：串 → 引用它的函数 → 角色）

- 串：`SELECT address, length, string_value FROM strings WHERE string_value LIKE '%SPItems%'`，同样查 `DrawingItems`、`EmbeddedSPID`、`strDrawingXMLFile`、`SharedItems`、`ModelItemType`、`IObject`、`ToFromText`、`P&IDAttributes`、`PublishedVer`、`SchemaVersion`、`UID`、`Rel`、`Interface`、`"-"`、`"."`、`" in"` / `" mm"`、`"@"`。VB6 的串是 UTF-16，若 `strings` 表只给 ASCII，用 `-c` 调 `ida_strlist` 把 Unicode 类型加进串表后再查。
- 引用：`SELECT string_value, func_name, ref_addr FROM string_refs WHERE string_value LIKE '%…%'`；VB6 的 `__vbaStrCat` / `__vbaStrCopy` / `__vbaVarCat` 调用点就是拼接发生的地方——`SELECT from_ea, name_at(from_func) FROM xrefs WHERE to_ea = <__vbaStrCat 导入项>`，按函数聚合，看哪些函数既引用 `"."` / `"-"` 又引用 UID 一类的串。
- 调用链：`callers` / `callees` 视图从 Drawing Manager 的 publish 命令往下走到 `sppiddwgprocess.dll` 的 `LoadSPItems`，再到 `sppidmdadapter.dll` 的接口映射；跨模块的 COM 调用（`CoCreateInstance` / `IDispatch::Invoke`）在 IDA 里断链，用 CLSID / ProgID 串接上（`SELECT … FROM strings WHERE string_value LIKE '%{%-%-%-%-%}%'`）。
- 产出：每模块「候选函数 ≤ 15 条」的表，列地址、名字（VB6 导出名或 `sub_`）、引用的串、初判角色。**这一步不下结论。**

### N2 两页分析

**N2a `docs/analysis/2026-10-1x-how-smartplant-publishes-a-drawing-from-its-items.md`（publish 写出侧，Q1 (a)）——挂起，等 `EFAdapter.dll`（Q5）**

写出器是 `EFAdapter.dll` 里的 `EFAdapter.*Publisher` / `EnumConversions`（§1.3）。文件到手后：`D:\ida_scratch\publish-native\efadapter\` 建库，`strlist_unicode.py` 重建串表，`string_refs` 找 `PipingPointPublisher` / `PpBranchPtPublisher` / `SignalPointPublisher` / `PipeLinePublisher` / `EnumConversions` 的类工厂与方法，再逐条反编译（`SELECT decompile(0x…) AS t`，或 `pseudocode` 表分段读）并给出：
1. **端口 / 工艺点 / 分支点 UID**：后缀 `.1` `.2` / `.PPT` / `.BPT` 由谁拼、序号按什么排（端口在物项上的顺序？`PortIndex`？）；端口的 `Name`、`PipingConnectorType`、`ConnectionFlowDirection` 从哪取。用 5 图逐个核：PowerShell 按得出的规则重算 2 983 个 UID 与参考比，**错 0** 才算成立。
2. **关系 UID** `{prefix}-{UID1}-{UID2}`：`prefix` 表与拼法；核 8 185 条。
3. **`IObject.Name`**：仪表 / 设备的两条规则（已发）是不是写出器拼的；管线 / 管段名的分段来自哪些属性、分隔符规则；哪些分段在 `.pid` 里（公称直径标签、介质 = `FluidSystem` 代码表、流水号、等级 = `PipingMaterialsClass`、保温 = `InsulSpec`）、哪些不在。
4. **公称直径**：publish 用的单位表与格式（`2 in` / `200 mm`），对照 `_NominalDiameter` 标签集 318 个里的写法。
5. **代码表引用** `@Name` / `@EE690` / `@{GUID}`：拼法与来源表；`.pid` 的字段里有没有对应的索引值。
6. **`_Meta.xml`**：`DocVersionDate` / `FilePath` / 版本 / 修订 / 文件对象的来源（库表 / 当前时间 / 配置），确认 ③ 的「不发」没有漏掉文件里其实有的东西。
7. **过滤**：publish 发哪些类、按什么条件漏掉（对照 I井场多出的 1 600 个被指名 UID）。

**N2b `docs/analysis/2026-10-1x-what-the-drawing-writes-into-p-and-id-attributes.md`（图纸写入侧，Q1 (b)）**

1. `P&IDAttributes` 行的写入器：哪些类写 `ModelID` / `ModelItemType` / `DrawingID` / `DrawingNo`，`ModelItemType` 的取值表；OPC 行的 `ModelID` 指什么（解释 23 / 53）。
2. 图纸是否写管线、端口、工艺点、分支点的任何身份（结论要能把「原始字节搜不到」升格为「写出器不写」）。
3. 代码表属性在图纸里的形态（索引 / GUID / 不写）；`_NominalDiameter` 标签集的写法规则。
4. ~~`EmbeddedSPID` 是什么~~（已答：归档包内嵌 `.pid` 的路径，`sppiddwgprocess.dll`，与 5 张图无关）。

**N2c `docs/analysis/2026-10-1x-the-map-schema-says-which-interfaces-a-class-publishes.md`（`sppiddatamap.xml`，Q6；不用 IDA）**

1. 解析 `dlls/sppiddatamap.xml`：64 个 `SPMapClassDef` × 其 `SPMapPropertyDef`（`ToolOID` = `SPMP_<类>_<列>`、映射到的 SmartPlant 接口 / 属性、`ProcessPropertyCriteria`、`UnmappedMapProperties` / `IgnoredMapProperties`），出一张「发布类 → 接口 × 属性 → 来源列 → 处理器」表；`SPMapRelDef` 两条照列。
2. 与 5 份参考 `_Data.xml` 的「类 × 接口 × 属性」实测表（10-10 分析页 + `_Coverage.json` 的 `per_class`）逐格对：参考里有而 map 里没有的（版本差：map 是 `SoftwareVersion 05.02`，参考是 `10.00.31`）、map 里有而参考里全空的，各列一节。
3. 对 `pid_writer` 的影响：今天「按类发哪些接口」是从参考 XML 反推的常量表；若 map schema 与参考一致，把常量表的注释出处改成 map schema 的 `ToolOID`（代码不动或只动注释，另开小单）。
4. 枚举 / 单位两张表是空壳，明确写「`@` 引用与 `2 in` 的格式不在此文件」。

三页都按 10-11 的样板：结论类型 **native-reader**；每条结论带模块、地址、原件 SHA-256、IDA 9.2；「复现」一节列 `ida-bridge` 命令；探针 / PowerShell 核对的脚本与输出进附录。**`src` 不动**；需要的探针放 `examples/probe_*.rs`，只读。

### G2 门（你拍板）

看 N2a 的 1–5 逐条答：按 Q2 的原则，哪几条升格为小单。每条小单的「对照证据」就是 N2a 的那一节 + 5 图核对错 0。

### N4 小单（G2 选中的，每条一单、一个提交）

照 `a6f8699` 的套路：
- 装载：`src/publish/pid_load.rs` 加对应对象 / 值 / 关系，每个值带出处（`ValueSource` 多一种：`DerivedByNativeRule { analysis_page, address }` 一类，让 `_Coverage.json` 能把「文件里的」和「按原生规则算的」分开数）。
- 写出：`pid_writer` 的「从不发」口径随之改；`_Coverage.json` 加一项计数。
- 测试：`pid_load` / `pid_writer` 单测；`tests/publish_from_pid.rs` 五组钉数重钉，逐块比仍是错类 / 错值 / 多发 = 0；`.pid` 的 `--diff-against` 5 图退出 0。
- 全量门五项 + `--no-default-features` + wasm32 lib 检查；5 张图重新写入 `D:\work\cad\PID自研软件验证\publish\`。
- 文档：本页 §7、分析页「导出器实测」、`CHANGELOG.md`、`task_plan.md`；只 `git add` 明确路径。

### N5 收尾

- 停掉自己开的 idalib 实例；`D:\ida_scratch\publish-native\` 留着（别的会话的样板也留着），分析页记路径。
- 本页 §7 记每步的日期、会话、提交号。

## 5. 验收

- N0–N2：每条结论能按「复现」一节在 `D:\ida_scratch\publish-native\` 的库上重现到同一地址；5 图核对脚本的输出在附录里，规则类结论**错 0**；`dlls/` 原件 SHA-256 前后一致；`src` 无改动，工作树上别的会话的文件无改动。
- N4 每单：5 图 `--diff-against` 退出 0；钉数只升；`cargo test --locked --workspace --all-targets` 0 败；clippy `-D warnings` 两种特性；fmt；missing-docs 棘轮不升；wasm32 lib 过。

## 6. 风险

- **VB6**：原生编译的 VB6 在 Hex-Rays 里是 `__vba*` 运行时调用堆成的面条，COM 调用经 `IDispatch` 断链；p-code 模块根本不能反编译。N0 先分流，分析页对每个模块写明是哪种。
- **关键串不在原生侧**（§1.2 零命中那一行）：端口后缀与关系定义名可能在库元数据表或 .NET 适配器里。N1 定位不到就按 Q4 (a) 收，不硬找。
- **publish 真正的写 XML 代码不在手头（已证实）**：`dlls/` 是部分拷贝，`EFAdapter.dll` 不在；拿到之前 N2a 做不了，端口 / 分支点 / 管线那 2 983 个对象维持不发。
- **bridge 共用**：server 和两个 idalib 实例是别的会话的；本单只用自己开的实例、不开 `--stateful`、不 `stop` 别人的。`exec` 默认 60 s 超时，反编译大函数加 `--timeout-s`；超时不重试同一条，先 `ida-bridge list`。
- **规则「成立」的标准**：5 图错 0 是必要条件，不是充分条件——5 图同一工厂、同一套命名配置。分析页要写明哪些分段是工厂配置（如管线名格式），导出器实现时把它们做成显式参数 / 配置，而不是写死。
- **共享工作树**：只碰新文件、`src/publish/`、`src/bin/pid_publish_xml.rs`、`CHANGELOG.md` / `task_plan.md` 的追加段；提交只 `git add` 明确路径；cargo 目录锁被占就等。

## 7. 执行记录

### 开单（2026-10-11，会话 fable-5-1-31）

- 读完三份交接记录与仓库现状，确认 10-10 计划只剩 ④；对 `dlls/` 与 `D:\pid\RADInstallA~` 做了一遍 ASCII / UTF-16 串扫描（§1.2 的表），用的是临时 PowerShell 脚本，不入库。
- 没建库、没开 IDA、没改 `src`、没提交。

### N0–N1 publish 写出侧（2026-10-11，同一会话；用户「用 ida-bridge 分析」）

- 建库 5 个（`D:\ida_scratch\publish-native\`，`sha256-before.txt` / `sha256-after.txt` 一致）：`sppidmdadapter` 303 函数 / 542 串、`sppiddwgprocess` 481、`efpublishcmd` 87、`efcompwpublishedvercmd` 86、`drawingmanagerocx` 1 266。`strlist_unicode.py` 放同目录。
- 定位结果与证据在 `docs/analysis/2026-10-11-the-publish-composer-is-efadapter-and-it-is-not-on-hand.md`：publish 写出器 = `EFAdapter.dll`，不在手头；`sppidmdadapter.dll` 是元数据适配器；`sppiddwgprocess.dll` 的 `LoadSPItems` 是装载；`dlls/sppiddatamap.xml` 是 map schema，指名每个类的发布器。
- Q3 按 (a) 做了；Q4 (a) 发生了；新加 Q5（找 `EFAdapter.dll`）、Q6（N2c）。本计划据此改成现在这版。`src` 未动、未提交；自己开的 idalib 实例已全停。
- 下一步候选：N2b（`pidobjmgrai.dll`，副本已拷）、N2c（`sppiddatamap.xml`）；N2a 等 Q5。

### N2c 对表（2026-10-11，会话 grok-4.7-10，任务 t-913）

- 分析页 `docs/analysis/2026-10-11-the-map-schema-says-which-interfaces-a-class-publishes.md`。四份上游底稿只读。map `SoftwareVersion` 05.02.00.0016，五张参考图 10.00.31.0023。实数 `SPMapClassDef` 31、`SPMapPropertyDef` 1 373、`SPMapRelDef` 1，不沿用旧的 64 / 2 752 / 2。
- 差异：参考有而 map 类表没有的类 1（`PIDProcessPoint`）；map 有而五图无对象的类 9。接口：参考有、map 接口列没有 211（非空 42，空壳 169）；map 有、五图非空和为 0 的 117；两边非空 37。属性：参考非空而 map 没有 49（`UID` 21，其余 28），参考只出现空值而 map 没有 3；map 有、五图非空和为 0 的 491（缺席类上 149，有对象的类上 342）；两边非空 90。
- 导出器允许发出的接口 73 格，在 map 接口列里 23，不在 50。允许写的属性 102 格，map 属性表有 50，其中参考也非空 32。不把常量表注释改成 `ToolOID`。
- `src` 未改、未提交。测试 NOT RUN：本步只写分析页并追加本节。

### N2b 图纸写入侧（2026-10-11，任务 t-1064 / t-1065 / t-1066）

三页都只读 `pidobjmgrai.dll`。原件 SHA-256 `E7216143BECD41A82F7263210C00ECEE4BF5BF1F3F9646541BCF60BEA6120F75`，IDA 9.2，镜像基址 `0x57090000`，2 673 个函数。`src` 未改、未提交。测试 NOT RUN：三页都只新建分析页。Q1 (b) 的第 4 问 `EmbeddedSPID` 此前已答，没有重做。N2a 仍等 `EFAdapter.dll`，G2 未开。

- **第 1 问** `docs/analysis/n2b-pid-attributes-writer.md`（t-1064）。`ModelID`、`ModelItemType`、`DrawingID`、`DrawingNo` 由同一个函数 `sub_572BCD13`（`0x572BCD13`）写入名为 `P&IDAttributes` 的属性集，不按类分叉。`DrawingNo` 向图纸对象要，另外三个由调用方传入。`ModelItemType` 的名字表在 `sub_572BEABE`，语料里的类名除 `OPC` 都在；模块里没有字面量 `OPC`。OPC 的 `ModelID` 是 `PersistSPItem` 用虚表 `+92` 取的物项 Id，OPC 专用函数只改配对图纸的图号，对得上 23 / 53。
- **第 2 问** `docs/analysis/n2b-identities-the-drawing-does-not-write.md`（t-1065）。图纸写出器不写管线、端口、工艺点、管道/信号分支点的发布身份。`ProcessPoint`、`PipingPort`、`SignalPort`、`.PPT`、`.BPT` 在模块里是 0 处。`Pipeline` 只有一处 UTF-16 字面量（`0x570B72AC`），代码引用是 0。`PipingPoint` / `SignalPoint` 只在连接点创建函数 `sub_571E9442`、`sub_571EDD2C`，这两个函数不写 `P&IDAttributes` / `ModelID`。
- **第 3 问** `docs/analysis/n2b-codelist-and-nominal-diameter.md`（t-1066）。图纸不写 `@Name` / `@{GUID}`。模块里有一张长度前缀的名字表（`0x570B6640`），含 `NominalDiameter`、`Class`、`EquipmentType` 等名字，没有 `@EE`，也没有 `2 in`。五份 `.pid` 共 639 处 `_NominalDiameter`：绑 `ItemProperty.NominalDiameter`，过滤是管件 305、管段 294、仪表 40；`PropertyFormat` 525 处是 `vbNullString`；值是 `1"`、`3/4"`、`25 MM`、`200`，不是发布 XML 的 `2 in`。

### N2a publish 写出侧（2026-10-12，任务 t-1269 / t-1270 / t-1271）

三页：`docs/analysis/n2a-efadapter-on-hand.md`、`docs/analysis/n2a-port-relation-filter.md`（t-1270）、`docs/analysis/n2a-name-diameter-codelist-meta.md`。`EFAdapter.dll` SHA-256 `6C77CC79A3CA92256F3C168C6183B03BAA67CF4545D9330E7E854C3F8C487891`，IDA 9.2，镜像基址 `0x5F4F0000`。三页 `src` 未改。

- **第 1 问** 端口 / 工艺点 / 分支点 UID 2,983 错 0。
- **第 2 问** 关系 UID 8,185 错 0。
- **第 3 问** 名字 236 / 13 错 0，但写出器不拼；管线模板 172 错 172，不成立。
- **第 4 问** 公称直径格式 878 错 0，成立；整数码对 `.pid` NOT RUN。
- **第 5 问** 代码表走 `codelists` 表。
- **第 6 问** `_Meta` 不是当前时间。
- **第 7 问** 表外类型报 `Unhandled itemtype name`，不是静默丢掉；取数滤掉 `SP_Ignored` 等。派生 UID 不在图纸 `ModelID` 里。

G2：用户 10-12 选 2，开 N4-0 … N4-4。

### N4-0 出处变体（2026-10-12，任务 t-1489）

`PidValueSource` 增加 `DerivedByNativeRule { analysis_page, address }`：由 `.pid` 里的值按 `EFAdapter.dll` 的原生规则确定地算出。`_Coverage.json` 的 `attribute_values.derived_by_native_rule` 与标签值、`composed_names` 分开数，stderr 同步这一计数。本单没有值改挂这个变体，5 图新计数为 0。仪表 / 设备名仍是 `NameRule`，规则和发出的值都不改；出处改口为 5 图实测规则（236 / 13 错 0），写出器侧无拼接证据，见 n2a 页。

- **验证**：`cargo test --locked --workspace --lib --bins --tests` 全过（`--lib` 1 175、`publish_from_pid` 4 / 4），`--no-default-features` 全过（`--lib` 850）；clippy `-D warnings` 两种特性零告警（lib / bins / tests）；rustfmt；missing-docs 0；wasm32 lib 过。`--all-targets` NOT RUN：别的会话未提交的 `examples/probe_ports_from_connect_points.rs` 此刻编不过、`probe_dash_scale_field_candidates.rs` clippy 不过。5 图 `--diff-against` 退出 0，五组钉数不变；`publish\` 15 个文件重写，`derived_by_native_rule` 都是 0。
- 会话：grok-4.7-27 写了代码与文档后掉线，fable-5-1-14（指挥官）核对、补 rustdoc 英文、跑门、提交。

### N4-2 工艺点（2026-10-12，提交 `fa16824`）

已发布的 `PIDPipingConnector` 各发一条 `PIDProcessPoint` 和一条 `ProcessPointCollection`。UID 是管段 `ModelID` 加 `.PPT`，关系 UID 是 `PRP-{工艺点}-{管段}`，`UID2` 是管段。规则地址 `ConstructUIDForProcessPoint`（`0x5F5CFD40`）、`ConstructUIDForRelationship`（`0x5F5D0280`，前缀 `PRP`）。探针 `examples/probe_n4_ports_and_process_points.rs`，数在 `docs/analysis/n2a-port-relation-filter.md`「N4-2 探针」。

- 五图工艺点 341 对 341，多 0、缺 0；关系 341 对 341。参考里没有 `Name`，也没有 `ConnectionFlowDirection`。提交说明：no extra values。
- 端口不发。`PipingPointNumber`、`SignalPointNumber`、`FlowDirection`、`SignalPoint` 在五张 `.pid` 的 ASCII 和 UTF-16 都是 0 处。`PipingPoint` 只在分离器 A 出现 6 次，是标签类名，不是点对象。`_ConnectPoint`：I井场 179、分离器 A 103、分离器 B 101、低温分离器 A 135、换热器 CD 163；`index` 15 / 12 / 12 / 7 / 13，`sub_type` 全是 `0x4`。
- 管道端口 / 发出 / 信号端口 / 发出 / 参考里有流向的管道端口：I井场 362 / 0 / 915 / 0 / 152，分离器 A 63 / 0 / 107 / 0 / 35，分离器 B 53 / 0 / 107 / 0 / 33，低温分离器 A 68 / 0 / 197 / 0 / 38，换热器 CD 172 / 0 / 399 / 0 / 40。合计 718 / 0 / 1 725 / 0 / 298。`PPC` / `SPC` 不发。
- `tests/publish_from_pid.rs` 的 `EXPECTED` 值列不动。对象 / 关系：I井场 1 459 / 1 695 → 1 628 / 1 864，分离器A 291 / 335 → 330 / 374，分离器B 239 / 274 → 268 / 303，低温分离器A 317 / 358 → 358 / 399，换热器CD 612 / 727 → 675 / 790。cargo、clippy、`--diff-against`：未记录。

### N4-1 公称直径（2026-10-12，任务 t-1490）

`_NominalDiameter` 的 `ItemValue` 换成发布格式后，以 `PidValueSource::DerivedByNativeRule { analysis_page: docs/analysis/n2a-name-diameter-codelist-meta.md, address: 0x5F52BDE0 }` 写上 `IPipeCrossSectionItem.NominalDiameter`。`N"` / `a/b"` / `N-a/b"` → `N in`，`N MM` → `N mm`，裸数字 → `N mm`（5 图对照错 0 才当毫米发）。换不出来的不发，计入 `_Coverage.json` 的 `attribute_values.nominal_diameter_not_emitted`。本单发出 273 条（管件 152、管段 101、直连仪表 20），错 0；参考里另外 605 处在本模式还不发的对象上。值钉数 2 822 → 3 095。对象和关系钉数不动：`b39ff7e` 的 `EXPECTED` 只改了值列。

- **验证**：`cargo test --locked --workspace --lib --bins --tests` 0 败（`--lib` 1 176，`publish_from_pid` 4 / 4）；`--no-default-features` 0 败（`--lib` 850）；clippy `-D warnings` 两种特性零告警；rustfmt；missing-docs 0。`--all-targets` NOT RUN：别的会话未提交的 example 编不过。5 图 `--diff-against` 退出 0，`derived_by_native_rule` 37 / 26 / 53 / 32 / 125，未换算 0；`publish\` 15 个文件重写。`b39ff7e` 的 `EXPECTED` 只改值列：分离器A 330 / 326 / 374，分离器B 268 / 256 / 303，换热器CD 675 / 636 / 790，低温分离器A 358 / 347 / 399，I井场 1 628 / 1 530 / 1 864。值合计 2 822 → 3 095；对象 2 918、关系 3 389 是 N4-2 之前的合计，不是这一笔。

### N4-3 分支点（2026-10-12，提交 `cec947e`）

点符号发分支点。`DrawingItemType = Symbol` 且描述记录是 `igPoint2d`（`0x005E`）：`PipeRun` → `PIDPipingBranchPoint`，`SignalRun` → `PIDSignalBranchPoint`。UID = `DrawingID` & `.` & `BPT`（`ConstructUIDForBranchPoint`，`0x5F5CFC10`）。`DrawingItems` 是 `DRI-{DrawingID}-{DrawingID.BPT}`，表示在 `UID1`，分支点在 `UID2`。探针 `examples/probe_branch_points_from_representations.rs`，数在 `docs/analysis/n2a-port-relation-filter.md`「N4-3 探针」。`PIDBranchPoint`（OPC）仍不发。Name 全空。

- 五图 199 对 199，错 0、多 0、缺 0。管道 183（I井场 97、分离器 A 22、分离器 B 14、低温分离器 A 15、换热器 CD 35），信号 16（10 / 1 / 1 / 1 / 3）。关系 199 对 199。提交说明：对象和关系各加 199，值不动。
- 管道接口：`IConnection`、`IPipingConnection`、`IDrawingItem`、`IPipingBranchPoint`、`IDocumentItem`。信号接口：`IConnection`、`IDrawingItem`、`ISignalConnection`、`ISignalBranchPoint`、`IFacilityPoint`、`IDocumentItem`。
- `EXPECTED`（对象 / 值 / 关系）：I井场 1 628 / 1 530 / 1 864 → 1 735 / 1 530 / 1 971，分离器A 330 / 326 / 374 → 353 / 326 / 397，分离器B 268 / 256 / 303 → 283 / 256 / 318，低温分离器A 358 / 347 / 399 → 374 / 347 / 415，换热器CD 675 / 636 / 790 → 713 / 636 / 828。cargo、clippy、`--diff-against`：未记录。本单提交说明写明 CHANGELOG、计划和 `task_plan` 当时没进暂存，因为工作树里还有 N4-1 的未提交文字。

### N4-4 连通关系（2026-10-12，提交 `7fdeae8`）

连通关系不发。探针 `examples/probe_connectivity_relationships.rs`，数在 `docs/analysis/n2a-port-relation-filter.md`「N4-4 探针」。关系行只存 `Relationship.<guid>`，管口行和注释行没有父项。Sheet 端点对在五张图上解不出图纸 id，所以那八种定义和 `PipingConnectors` 都不进装载。

- `ModelItemType = Relationship` 的行：I井场 946、分离器 A 191、分离器 B 153、低温分离器 A 200、换热器 CD 411。字段只有 `ProjectNumber`、`DrawingNo`、空的 `DrawingItemType`、空的 `DrawingID`、`ModelItemType`、`ModelID`、`Flag`。
- Sheet 端点对：I井场 26、分离器 A 36、分离器 B 2、低温分离器 A 78、换热器 CD 412，`both_drawing_ids = 0`。
- 发出都是 0。参考 / 两端都已发出：`PipingEnd1Conn` 283 / 93，`PipingEnd2Conn` 279 / 117，`SignalEnd1Conn` 75 / 5，`SignalEnd2Conn` 83 / 12，`PipingTapOrFitting` 738 / 164，`SignalTapOrFitting` 35 / 15，`EquipmentComponentComposition` 36 / 36，`PBSItemNotes` 7 / 7，`PipingConnectors` 219 / 0。从图纸配上的都是 0。`EquipmentComponentComposition` 实测是管口在 `UID1`、设备在 `UID2`。
- `tests/publish_from_pid.rs` 不在这笔提交里，钉数不改。cargo、clippy、`--diff-against`：未记录。
