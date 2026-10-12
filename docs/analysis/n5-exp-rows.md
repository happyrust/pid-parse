# N5 exp 行：I井场三张表按列从 Oracle exp 读出（2026-10-12）

> 任务 t-1831。只读 `D:\work\cad\pid-test-data\Export.dmp`（`EXPORT:V12.01`，74,489,856 字节）。不跑 `imp`，不连 Oracle，不改 `src/publish`，不重写 `D:\work\cad\PID自研软件验证\publish`。
> 探针 `examples/probe_oracle_exp_rows.rs`。`cargo run --example probe_oracle_exp_rows` 退出 0。
> 既有测试 NOT RUN：本轮没有改 `src`，没有跑 `cargo test`。

## 一句话

`INSERT INTO` 后面的行是按列的长度前缀，NVARCHAR2 的载荷是 UTF-16BE。I井场 362 个管道端口、915 个信号端口都能用 `SP_PLANTITEMID` 加点号再加 `*POINTNUMBER` 对上参考 UID，父项是 183/183 和 127/127。`T_RELATIONSHIP` 在这张图上的 945 行，两端落在已发对象里的 849 行全是表示对表示；设备↔管口、注释、管线↔管段的参考端点对，在这张表的两端列里是 0。

## 行格式

用 `APPLICATIONS` 钉住，再读三张大表。第一行是 `ID=1`、`NAME=SmartPlant PID`、`SHORTNAME=SPPID`，这张小表一共 5 行。钉不住探针就退出 1，不会往下读。

`INSERT` 行以换行结束，后面是：

1. `u16` 小端列数。
2. 每一列：类型 `1` 是字符（再跟最大字节数、字符集 id、字符集形式），类型 `2` 是数（再跟最大长度，NUMBER 和 FLOAT(126) 都是 22）。三张表的字符列都是字符集 2000、形式 2，也就是 AL16UTF16。
3. 四个 `00`。
4. 每一行的每一列：`u16` 小端字节长度；`0xFFFE` 是 NULL；否则随后就是这么多字节。字符字节按 UTF-16BE 解码。从 ASCII 字节看会看成 `S\0m\0a\0r\0t`，那是往右错一字节；长度盖住的载荷本身是大端。
5. 每行之后两个 `00`。
6. 下一行的长度若是 `0xFFFF`，表结束。随后就是换行加 `CREATE` 或 `GRANT`。

数是 Oracle 内部 NUMBER。`80` 是 0，`c1 02` 是 1，`c2 1a 34` 是 2551，`3e 64 66` 是 -1。探针里这些字节有断言。

这份库不是只有 I井场。`T_RELATIONSHIP` 有 51 个不同的 `SP_DRAWINGID`。I井场用参考 XML 根上的 `DocUID`（`5633AF4121EE4F4D9FE78E90722CC050`）过滤，这个 id 在表里，对应 945 行。管道点和信号点没有图纸列，I井场子集是拼出来的 UID 能落进参考端口的那些行。

## 管道点

全表 9,628 行。非空：`SP_ID` 9,628，`SP_PLANTITEMID` 9,628，`PIPINGPOINTNUMBER` 9,628，`FLOWDIRECTION` 4,077，`POINTINDEX` 9,628，`NOMINALDIAMETER` 6,030。`SP_ID` 里带点号的是 0。拼出来的 UID 9,628 个，没有重复。

对 I井场参考 `PIDPipingPort`（362）：

| 项 | 数 |
|---|---:|
| 对上 | 362 |
| 缺 | 0 |
| `Name` 不等于点号 | 0 |
| 父项不等于 `SP_PLANTITEMID` | 0 |
| 参考父项出现在 `SP_PLANTITEMID` | 183/183 |
| `FLOWDIRECTION` 非空 | 152 |
| 流向按下面的表对上 | 152 |
| 两边流向都空 | 210 |
| 流向对不上 | 0 |
| `POINTINDEX` 不等于 `PIPINGPOINTNUMBER` | 13 |

上一轮字符串扫描说参考父项 183 个里 dump 只有 40 个。按列读是 183/183。点号不在 UID 字符串里，在 `PIPINGPOINTNUMBER`。发布用的后缀是这个点号，不是 `POINTINDEX`；那 13 行两个数不同，`Name` 仍等于点号。

流向沿用 `docs/analysis/n2a-port-relation-filter.md` 的序号表，本页只核对、不另造：1 和 5 → `{23F94152-B87D-11D6-80D4-00A0C92487AF}`，2 → `{23F94146-B87D-11D6-80D4-00A0C92487AF}`，3 → `{23F9414C-B87D-11D6-80D4-00A0C92487AF}`，4 → `{BCB3F097-66CA-487B-9AF4-D8FCF18624A3}`，空就不写。I井场对上的 362 个里，152 个序号 2 或 3，210 个两边都空。

公称直径：223 行两边都有值，139 行两边都空，没有「参考有字、dump 是空」或反过来。223 行分成 16 个代码，每个代码只对应一种参考文本（`ambiguous_codes=0`）。探针对这 223 行检查了一条规则，命中 223、落空 0：

- 代码小于 5000：文本是 `{代码} mm`（25、50、80、100、150）。
- 代码大于 5000：英寸 = `(代码 - 5000) / 32`，写成参考里的样子（`5032` → `1 in`，`5024` → `0.75 in`，`5384` → `12 in`）。

## 信号点

全表 22,057 行。`SP_ID`、`SP_PLANTITEMID`、`SIGNALPOINTNUMBER`、`POINTINDEX` 都是 22,057 非空。带点号的 `SP_ID` 是 0。

对 I井场参考 `PIDSignalPort`（915）：对上 915，缺 0，`Name` 错 0，父项错 0。参考父项 127/127 都在 `SP_PLANTITEMID` 里。`POINTINDEX` 不等于 `SIGNALPOINTNUMBER` 的有 59 行；发布后缀仍是点号。这张表没有流向列。

## 关系

全表 24,664 行。非空：`SP_ID` 24,664，`SP_DRAWINGID` 24,664，`SP_ITEM1ID` 24,653，`SP_ITEM2ID` 22,364。

I井场 `DocUID` 上 945 行：`SP_ITEM1ID` 945，`SP_ITEM2ID` 849，`SP_ITEM2ID` 空 96。两端都在已发对象里的是 849，类对全是 `PIDRepresentation` → `PIDRepresentation`。另外 96 行只有一端在已发对象里，就是 `SP_ITEM2ID` 为空的那些。已发对象取 `publish\I井场 注采阀组工艺及自控流程图_Data.xml`，1,735 个对象，管道端口和信号端口都是 0。

三类参考关系（端点对是否出现在 `T_RELATIONSHIP` 的 `SP_ITEM1ID`/`SP_ITEM2ID`，全表都找过，含对调）：

| 类 | DefUID | 参考 | dump 有序对 | dump 对调 | 两端都在参考对象 | 两端都在已发对象 |
|---|---|---:|---:|---:|---:|---:|
| 设备↔管口 | `EquipmentComponentComposition` | 7 | 0 | 0 | 7 | 7 |
| 注释 | `PBSItemNotes` | 6 | 0 | 0 | 6 | 6 |
| 管线↔管段 | `PipingConnectors` | 106 | 0 | 0 | 106 | 0 |

参考端的类：

- 设备↔管口 7 条全是 `PIDNozzle` → `PIDProcessVessel`。管口和容器都在已发对象里，所以「两端都已发」是 7。这两个 UID 都不在 `T_RELATIONSHIP` 的两端列里。
- 注释 6 条：`PIDNote` → `PIDControlSystemFunction` 4，`PIDNote` → `PIDInlineInstrument` 1，`PIDNote` → `PIDPipingComponent` 1。两端都已发是 6。两端列里是 0。
- 管线↔管段 106 条全是 `PIDPipingConnector` → `PIDPipeline`。已发对象里没有 `PIDPipeline`，所以两端都已发是 0。两端列里也是 0。

上一轮字符串扫描说注释 6 条里 5 条两端都在 dump。本探针只读这三张表的列，注释端点 UID 在 `SP_ITEM1ID` / `SP_ITEM2ID` 里是 0。没有再去扫别的表。

## 复现

```powershell
cargo run --example probe_oracle_exp_rows
```

可选三个参数：dump、参考 `_Data.xml`、已发 `_Data.xml`。缺省就是上面这三个路径。

## N5-1 读进库（2026-10-12，t-1966）

行格式没改，搬进 `src/publish/oracle_exp.rs`。`APPLICATIONS` 钉不住就不读 `T_PIPINGPOINT` / `T_SIGNALPOINT`。对外是管道点四列（流向和公称直径可空）和信号点两列，再按已发物项的 UID 集合挑一张图。不写 XML。

库侧实跑（`ijingchang_ports_match_the_reference`）：全表 9 628 / 22 057；I井场管道 362、信号 915；UID 错 0；多发 0。上面各节的探针数字没有重跑。
