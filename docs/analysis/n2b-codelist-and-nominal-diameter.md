# N2b：代码表在图纸里的形态，以及 `_NominalDiameter` 怎么写（2026-10-11）

> 计划 N2b 第 3 问（Q1 (b)）。第 4 问 `EmbeddedSPID` 已在 `docs/analysis/2026-10-11-the-publish-composer-is-efadapter-and-it-is-not-on-hand.md` 答过，这里不重做。
> 模块 `pidobjmgrai.dll`。原件 `D:\work\plant-code\cad\pid-parse\dlls\pidobjmgrai.dll`，SHA-256 `E7216143BECD41A82F7263210C00ECEE4BF5BF1F3F9646541BCF60BEA6120F75`。副本 `D:\ida_scratch\publish-native\pidobjmgrai-q3\pidobjmgrai.dll`，同一哈希。IDA 9.2，`idalib`，映像基址 `0x57090000`，函数 2673（不是 p-code）。库 `D:\ida_scratch\publish-native\pidobjmgrai-q3\pidobjmgrai.i64`。
> **本轮未改 `src`、`dlls` 原件、计划或已有分析页，也没有提交。** 测试 NOT RUN：只新建本文件，没有跑 `cargo test`。
> 上一手 grok-4.7-12 掉线时目录和本文件都不存在，这份从零做。本会话是指挥官，任务书要求写这一页，所以按任务书写了。

## 一句话

图纸写入侧不写发布 XML 里的代码表引用（`@Name` / `@EE690` / `@{GUID}`），也不写 `2 in` 这种公称直径格式。`pidobjmgrai.dll` 里只有属性**名字**；`.pid` 里 `_NominalDiameter` 标签存的是 `1"`、`25 MM`、`200` 这种原值，`PropertyFormat` 大多是 `vbNullString`。

## 代码表：不写 `@`，也不写 GUID

`pidobjmgrai.dll` 全文没有这些 UTF-16 或 ASCII 字面量：`@NewConstruction`、`@EE`、`EnumConversions`、`InstrumentLocation`、`_NominalDiameter`、`2 in`。UTF-16 的 ` in` 有 165 处，抽看都是英文 `in`（`Error in Update…`），不是单位。

有一张 VB6 长度前缀的 UTF-16 名字表，文件偏移 `0x26640`，IDA 地址 `0x570B6640`。`get_bytes` 开头是 `1e000000` 再加 `NominalDiameter`（15 个字符 = 30 字节，与长度字一致）。IDA 的 `strings` 表没收这 11 个名字，因为它们是长度前缀而不是 C 串；对 `0x570B6640` 和 `0x570B6644` 做 `XrefsTo` 都是空的，没有函数直接引用。11 个名字是：

| 名字 |
|---|
| `NominalDiameter` |
| `PointIndex` |
| `Class` |
| `EquipmentSubclass` |
| `EquipmentType` |
| `OPCType` |
| `PipingCompClass` |
| `PipingCompSubclass` |
| `PipingCompType` |
| `SP_DuctingCompClass` |
| `SP_DisableFlipText` |

这是分类属性和公称直径的**名字**，不是枚举值，不是序号，不是 GUID。

另一张名字表在文件偏移 `0x25910` 起，第一个是 `ConstructionStatus`，后面是 `GraphicOID`、`SP_DrawingID`、`XCoordinate`、`SP_PipeRunID` 这类图纸持久化字段，不是代码表的值。`IsTypical` 只作为 `SP_IsTypical` 的子串出现（文件偏移 `0x28742`），旁边是 `TypicalMode`。

ASCII 串 `SPenums` 在 `0x570B476C`，唯一交叉引用是数据 `0x570B2948`，不在函数里。这是接口/类型名，不是往属性里写 `@` 的函数。

五份 `.pid` 里的 `@` 字节也不是代码表引用。分离器 B 的 UTF-16 `@` 共 188 处，每一处后面都不是可打印的名字（没有 `@EE…`）。所以图纸文件里没有发布侧那种 `@` 引用。GUID 形式的代码表值本模块的字符串里没有。

结论：代码表属性在图纸写入侧**不是**以 `@` 引用或 GUID 写下去的。发布 XML 里的 `@` 是后来的 `EFAdapter.EnumConversions` 做的，那个 DLL 不在手头，本问不追。

## 公称直径：标签集绑属性名，值不带 `2 in`

`pidobjmgrai.dll` 里 `NominalDiameter` 只出现在上面那张名字表，没有 `_NominalDiameter`，没有 `2 in` / `200 mm` 格式串。

五份参考 `.pid`（`D:\work\cad\PID自研软件验证\` 根目录，不含 `publish\`）里，ASCII 标记 `_NominalDiameter` 共 639 处。每处后面的可打印字段是：

| 字段 | 取值 |
|---|---|
| `ItemFilter` | `PipingComp` 305、`PipeRun` 294、`Instrument` 40 |
| `ItemProperty` | `NominalDiameter` |
| `PropertyFormat` | `vbNullString` 525、`Upper Case - Variable Length` 114 |
| `ItemValue` | 36 种。最多的是 `1"` 80、`3/4"` 66、`2"` 56、`25` 36、`25 MM` 34、`100` 32。没有 `2 in` |

写法规则：标签集名叫 `_NominalDiameter`，用 `ItemFilter` 限定管件、管段或仪表，绑定 `ItemProperty.NominalDiameter`。格式串大多为空（`vbNullString`）。存下来的是图纸自己的写法（英寸记号 `1"`、`3/4"`，或毫米数字 / `25 MM`），不是发布 XML 的 `1 in` / `2 in` / `200 mm`。这和 `docs/analysis/2026-10-10-publish-from-the-pid-alone.md` 说的「标签集对不上 878 个发布值」是同一件事；那一页没打开标签，这里把字段读出来了。

N4-1（2026-10-12）在写出侧把这些 `ItemValue` 换成发布格式，规则和 5 图核对记在 `docs/analysis/n2a-name-diameter-codelist-meta.md`「标签文本换算」一节。本页仍只描述图纸写入侧，不改这里的结论。

## 复现

不碰另外两个目录（`pidobjmgrai\`、`pidobjmgrai-q2\`），不 `--stateful`，不 `stop` 别人的 `idalib`。下面命令用完会自己停掉本次实例。

```text
ida-bridge exec-idb --input D:\ida_scratch\publish-native\pidobjmgrai-q3\pidobjmgrai.dll --out-idb D:\ida_scratch\publish-native\pidobjmgrai-q3\pidobjmgrai.i64 --save --wait-s 900 --sql "SELECT COUNT(*) AS funcs FROM funcs"
```

本次结果：`funcs = 2673`，然后 `saved` / `stopped`。

```text
ida-bridge exec-idb --idb D:\ida_scratch\publish-native\pidobjmgrai-q3\pidobjmgrai.i64 --timeout-s 120 -f D:\ida_scratch\publish-native\pidobjmgrai-q3\xrefs_n2b_q3.py
```

`0x570B6640` 的 `get_bytes` 应为 `1e0000004e006f006d0069006e0061006c00440069006100`，`xrefs` 为空。IDA 版本在同一次执行里是 `9.2`。

原件哈希：

```text
Get-FileHash -Algorithm SHA256 D:\work\plant-code\cad\pid-parse\dlls\pidobjmgrai.dll
```

`.pid` 计数是读五份文件里的 ASCII `_NominalDiameter`，把不可打印字节当分隔符，取后面的 `ItemFilter` / `PropertyFormat` / `ItemValue`。没有改这些 `.pid`。
