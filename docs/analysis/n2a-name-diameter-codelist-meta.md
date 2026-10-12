# N2a：名称、公称直径、代码表、Meta（2026-10-12）

计划 `docs/plans/2026-10-11-publish-native-evidence-via-ida-bridge.md` N2a 第 3、4、5、6 问。结论类型 **native-reader**。模块 `EFAdapter.dll`，原件 SHA-256 `6C77CC79A3CA92256F3C168C6183B03BAA67CF4545D9330E7E854C3F8C487891`，IDA 9.2，镜像基址 `0x5F4F0000`。库在 `D:\ida_scratch\publish-native\efadapter-name\EFAdapter.i64`（1460 个函数）。`src` 未改，`dlls` 未改，`n2a-efadapter-on-hand.md` 和 `n2a-port-relation-filter.md` 未改。只停了本会话的 `idalib-46352`，没有动 `efadapter-uid` 和 plant3 的实例。

五份参考 XML 是 `D:\work\cad\PID自研软件验证\` 根目录的 `_Data.xml` / `_Meta.xml`（不含 `publish\`）。`dlls/sppiddatamap.xml` 是计划里写的 SoftwareVersion 05.02；这五份 XML 的 `SoftwareVersion` 是 `10.00.31.0023`。

## 3. `IObject.Name`

### 仪表 / 设备：发布结果等于已发的两条规则，但不是这个 DLL 拼出来的

已发的两条规则是 `{MeasuredVariable}-{InstrTagSequenceNo}` 和 `{EquipPrefix}-{EquipSeqNo}`。

`EFAdapter.dll` 的串表里没有 `MeasuredVariable`、`InstrTagSequenceNo`、`EquipPrefix`、`EquipSeqNo`。`dlls/sppiddatamap.xml` 里仪表 `ItemTag`（`SPMP_Instrument_ItemTag`、`SPMP_InstrumentInline_ItemTag`、`SPMP_InstrumentCtrlSysFunc_ItemTag`）的 `ProcessPropertyCriteria` 是空的，不走 `EFAdapter.PpConnNameConv`。设备组件的 `ItemTag` 同样是空的。所以这两条连字符规则不是 `PpConnNameConv` 的模板，也不是这份 DLL 里的字面量。

五份 `_Data.xml` 里，带 `Name` 且能读到两段属性的对象：

| 规则 | 核对 | 错 |
| --- | --- | --- |
| `Name` = `MeasuredVariable` + `-` + `InstrTagSequenceNo`（`PIDInstrument` / `PIDControlSystemFunction` / `PIDInlineInstrument`） | 236 | 0 |
| `Name` = `EquipPrefix` + `-` + `EquipSeqNo`（容器、工艺设备、机械设备、换热器） | 13 | 0 |

**成立**的只是「发布 XML 里的 `Name` 等于这两段用一个连字符接起来」。**不成立**的是「`EFAdapter.dll` 在写出时现拼这两段」：模块里没有这四个属性名，map 也没有把拼接交给 `PpConnNameConv`。

### 管线 / 管段：05.02 的 map 模板对不上这 10.00 的名字

`SPMP_Pipeline_ItemTag` 的处理串是 `EFAdapter.PpConnNameConv, <OperFluidCode>-<TagSequenceNo>`。`SPMP_PipeRun_ItemTag` 是 `EFAdapter.PpConnNameConv, <OperFluidCode>-<TagSequenceNo>-<NominalDiameter>-<PipingMaterialsClass>-<InsulPurpose>`。分隔符是模板里的 `-`，段序由 map 的这串决定，不是 DLL 里另写死的一张表。

DLL 里和这段有关的字面量：

| 串 | 地址 | 代码引用 |
| --- | --- | --- |
| `PpConnNameConv` | `0x5F505068` | 只有类表 `0x5F503B04`，没有函数内引用 |
| `OperFluidCode` | `0x5F505760`（文件偏移 87904 / `0x15760`） | 0 |
| `TagSequenceNo` | `0x5F505780` | `sub_5F54E530` 在 `0x5F54EF9C` 拿它和属性名做相等比较；同函数还引用 `PipeRun`（`0x5F5099A8`） |
| `NominalDiameter` | `0x5F5057A0` | `sub_5F52B230`（`NominalDiameterConv`） |
| `PipingMaterialsClass` | `0x5F5057C4` | 0 |
| `InsulPurpose` | `0x5F5057F4`（文件偏移 88052 / `0x157F4`） | 0 |

`OperFluidCode` 和 `InsulPurpose` 都在这份 DLL 里，是 UTF-16。`.text` 的 raw 等于 RVA，所以虚拟地址 = `0x5F4F0000` + 文件偏移。`OperFluidCode` 紧挨在 `TagSequenceNo` 前面（相差 `0x20`），`InsulPurpose` 在 `PipingMaterialsClass` 后面。对指针字节 `60 57 50 5F` 和 `F4 57 50 5F` 做全文件扫描是 0 处，IDA 交叉引用也是 0。`InsulSpec` 和 `FluidSystem` 这两个字面量仍然没有。

map 的管线 / 管段模板用的就是这些名字：`OperFluidCode`、`TagSequenceNo`，管段再加 `NominalDiameter`、`PipingMaterialsClass`、`InsulPurpose`。前两个和最后一个在 DLL 的同一簇 UTF-16 里；其中只有 `TagSequenceNo` 有代码引用。整条 `<OperFluidCode>-<TagSequenceNo>-...` 模板还是写在 `sppiddatamap.xml` 里，不是 DLL 里的一整条字面量。名字在 DLL 里，不表示 05.02 的段序就是这五份 10.00 XML 的拼法。

五份 `_Data.xml`：管线 `IObject/@Name` 172 条，其中形如「两段、中间一个 `-`」的是 0 条。map 的管线模板 `<OperFluidCode>-<TagSequenceNo>` 对这 172 条 **错 172，不成立**。管段带 `Name` 的 219 条；用「名字以 `-` + 本对象 `NominalDiameter` + `-` + `PipingMaterialsClass` + `--` 结尾，且去掉这段后等于某条管线名」去核，对上 121，错 98（英寸名在名字里是 `3"` / `3/4"` / `1/2"`，属性上是 `3 in` 这种写法；另有名字里直径那一段是空的）。**不成立**。

五份 XML 里属性出现次数：`OperFluidCode` 0，`InsulPurpose` 0，`InsulSpec` 201，`FluidSystem` 145，`PipingMaterialsClass` 282，`PipingConnectorSeqNo` 219。名字里的介质短码（`AI`、`PC`）和保温段（`H40`）没有同名的 `OperFluidCode` / `InsulPurpose` 属性。`FluidSystem` 出现时是 `@{GUID}`，不是名字里的短码。

哪些段在 `.pid` 里：公称直径的图纸标签不是这套名字写法，见 N2b `docs/analysis/n2b-codelist-and-nominal-diameter.md`（本页没有重做那 639 处）。介质短码、流水号、等级、保温没有在本页再扫 `.pid` 字段。

## 4. 公称直径

`NominalDiameterConv` 的转换在 `sub_5F52BDE0`。它按数据类型分两支，错误串写在：

- `0x5F50D7FC`：属性在 Engineering Framework 上是 `UoMListType` 时做转换
- `0x5F50D8F8`：属性是 ENUM 时做转换

UoM 这一支在 `0x5F52C623` 调用 `sub_5F5D35E0`（错误串 `:ConvertNominalDiameterToUOM::`，`0x5F51A134`）。反编译出来的编码：

- 整数码 `< 5000`：数字串是这个整数，单位宽串是 `mm`（`0x5F51A11C`），米制值是该整数 × 0.001。
- 整数码 `≥ 5000`：英寸数是 `(码 - 5000) / 32`，单位宽串是 `in`（`0x5F51A128`），米制值是英寸数 × 0.0254。

函数把数字和单位分开放进两个出参，反编译里看不到它自己写上空格。发布 XML 里的空格是结果里有的，见下表。

五份 `_Data.xml` 的 `NominalDiameter="..."` 共 878 处（98 + 76 + 184 + 95 + 425）。每一个都是「数字 + 空格 + `in` 或 `mm`」（`0.5 in` … `14 in`，`15 mm` … `600 mm`）。**错 0。这个写法成立。**

`< 5000` / `/ 32` 这套整数码没有对回五份 `.pid`（发布 XML 里只有已经格式化的字符串）。这项 **NOT RUN**，不把编码写成成立。图纸标签集是 `1"` / `25 MM` / `200`，不是 `2 in` / `200 mm`，出处是 N2b 那一页，这里不重做。

## 5. 代码表引用

`EnumToStringConv` 是 `sub_5F564800`。它引用 `Name`（`0x5F5079E0`）和 `Description`（`0x5F508C8C`），对应 map 里的 `EFAdapter.EnumToStringConv,<Name>` 和 `<Description>`。同函数拼的 SQL 字面量：

- `0x5F512AA0` `select codelist_short_text from `
- `0x5F512AE8` `codelists where codelist_number=`
- `0x5F512B30` ` and codelist_index=`

旁边还有 `Data_Dictionary`（`0x5F50DB68`）和 `SPPIDDATA_DICTIONARY`（`0x5F50DBE4`）。短文本来自库表 `codelists`，键是 `codelist_number` 和 `codelist_index`，不是 `.pid` 里的 `@` 串。

`EnumConversions` 是 `sub_5F5F4D60`。它引用 `MapEnumToEnum`（`0x5F50F3AC`）和 `GetEnumHierarchyValue` 的失败串（`0x5F51BB24`）。map 里大量属性的 `ProcessPropertyCriteria` 就是 `EFAdapter.EnumConversions`。DLL 里没有 `@EE` 这个字面量；有一处 `@Gauge`（`0x5F50E774`，`sub_5F5376B0`）。

五份 `_Data.xml` 里以 `@` 开头的属性值：`@` + 名字 997，`@EE` + 十六进制 711，`@{GUID}` 2472，其他形状 0。这是发布结果的形状统计，不是「DLL 用某个地址拼出 `@EE`」的核对，所以不写成立。

`.pid` 里有没有对应索引：这份 DLL 的 SQL 读的是库表 `codelist_index`。N2b 已经写明图纸不写 `@Name` / `@{GUID}`。本页没有把 `codelist_index` 对回 `.pid` 的整数字段。**NOT RUN**。

## 6. `_Meta.xml`

五份根目录 `_Meta.xml`：

| 项 | 五份里的值 |
| --- | --- |
| `FilePath` | 全是 `\\Glpid-7\pidsq\PID` |
| `DocVersion` | 全是 `1` |
| `DocRevision` | 全是 `0` |
| `SoftwareVersion` | 全是 `10.00.31.0023` |
| `DocVersionDate` | I井场 `2026/06/22`，其余四份 `2026/07/06` |

`DocVersionDate` 这个字面量不在 `EFAdapter.dll` 里。日期 API `rtcGetPresentDate`（`0x5F4F9AA6`）和 `__vbaStrDate`（`0x5F4F9AAC`）没有交叉引用。日期格式串 `MM/dd/yyyy hh:mm:ss`（`0x5F517A38`）也没有交叉引用。这份 DLL 不是用「当前时间」格式化 `DocVersionDate`。

`FilePath`（`0x5F520744`）的代码引用在 `0x5F633E74`：先把接口名 `IFile` 抄进字符串，再把属性名 `FilePath` 抄进去做 COM 取值。UNC 路径本身不是 DLL 里的字面量，是运行时对象上的值。`File`（`0x5F5211A8`）和 `FileComposition`（`0x5F521094`）也在附近的数据里。

版本 / 修订走的是 `sub_5F60D800`（错误串 `:IEFAdapter3_ReviseDocuments::`，`0x5F51DFE8`）。它引用 `SP_ID`（`0x5F506AB8`）、`Revision`（`0x5F507A5C`）、`RevisionNumber`（`0x5F51DF80`）、`DocRevision`（`0x5F51DFA4`）、`DocumentVersion`（`0x5F51DF44`）、`VersionedDoc`（`0x5F50BB10`）、`_REV_`（`0x5F51DE04`）、` Revision`（`0x5F51DF68`，前面有空格）。这是库里的修订字段，不是时钟，也不是写死的 `1` / `0`。

和「只凭 `.pid` 不发这三项」的关系：`DocVersionDate`、`\\Glpid-7\pidsq\PID`、修订号都不在这份 DLL 的常量里，修订函数读的是库字段 `SP_ID` / `RevisionNumber`。没有漏掉一份「其实在 `.pid` 里、只是这条写出器没写」的文件内常量。本页没有新扫 `.pid` 字节。

## 复现

```
ida-bridge exec-idb --input D:\ida_scratch\publish-native\efadapter-name\EFAdapter.dll --out-idb D:\ida_scratch\publish-native\efadapter-name\EFAdapter.i64 --save --wait-s 900 --sql "SELECT COUNT(*) AS funcs FROM funcs"
ida-bridge supervisor start-idalib --idb D:\ida_scratch\publish-native\efadapter-name\EFAdapter.i64
ida-bridge exec <cid> --timeout-s 180 -f D:\ida_scratch\publish-native\strlist_unicode.py
ida-bridge exec <cid> --timeout-s 180 --sql "SELECT decompile(0x5f5d35e0) AS t"
ida-bridge exec <cid> --sql "SELECT string_value, func_addr, func_name FROM string_refs WHERE func_addr IN (0x5f52bde0, 0x5f54e530, 0x5f564800, 0x5f5f4d60, 0x5f60d800)"
ida-bridge supervisor stop <cid>
```

`OperFluidCode` / `InsulPurpose` 不在 IDA 串表里。UTF-16 在文件偏移 87904 和 88052；虚拟地址 `0x5F505760`、`0x5F5057F4`。全文件搜指针 `60 57 50 5F` 和 `F4 57 50 5F` 是 0 处。补看时开过 `idalib-18572`，查完已停。

本会话的 `<cid>` 是 `idalib-46352`，分析结束后已停。不开 `--stateful`。

测试 **NOT RUN**：没有跑 cargo，没有把公称直径整数码或 `codelist_index` 对回 `.pid`。上面的 236 / 13 / 172 / 219 / 878 是对五份参考 XML 的字符串计数。
