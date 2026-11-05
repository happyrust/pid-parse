# 七张参考图 publish 对照面：P0 出数

日期：2026-11-05。只读出数，`src` 未改。

## 怎么跑的

- 整包：`pid_publish_xml D:\work\cad\pid-test-data --out-dir <tmp> --diff-against D:\work\cad\pid-test-data\验收数据`，53 张图，1 张干净（I井场），10 张有发现，42 张无参考。10 张里有 4 张是本计划范围外的旧集注站图（DocUID 不在这份备份里，被同名不同 UID 的参考误配，发现数 1500–4400），不在此处归因。
- 单图：对 6 张新图 + I井场逐张跑 `<图>.pid --diff-against <图>_Data.xml --plant P01 --oracle-exp Export.dmp --datamap RefData~4~804.zip`。必须以长文件名传入：8.3 短名会让 `PIDDrawing IObject.Name` 多出一条差异、并找不到同目录的 `_Meta.xml`（与 I井场基线登记的 alias 现象相同）。

## `_Meta.xml` 门禁

7 张全部 matched（13 个容器属性、6 个块、3 条 rel），只剩已登记的 DocVersionDate / FilePath / 六个派生 UUID v5。

## `_Data.xml` 差异（6 张新图）

| 图 | values different | extra / missing 对象 | rels 不在参考 | FluidSystem 参考为空 |
|---|---|---|---|---|
| J K井场 | 0 | 0 / 12 | 0（参考侧 12 条被 excused） | 34 |
| L M井场 | 0 | 3 / 3 | 3 | 42 |
| 轻烃分离器I气出口改造 | 0 | 5 / 3 | 5 | 0 |
| 烃水露点（一）空冷器A | 0 | 7 / 7 | 7 | 3 |
| 烃水露点（二）空冷器B | 0 | 1 / 1 | 1 | 3 |
| 烃水露点（三）空冷器C | 0 | 1 / 1 | 1 | 3 |

所有 values 层面的差异都已归零；剩下三类根因。

### 根因 B：`PIDPipeline.Name` 的来源（L M / 轻烃I / 空冷器 A·B·C，共 14 条名称不配对）

`pipeline.rs` 把名字拼成 `NOMINALDIAMETER-流体码-TAGSEQUENCE-MATERIALSCLASS-INSULATION`；参考的名字就是 `T_PLANTITEM.ITEMTAG`（`SP_ID` = 管线 UID）。在 7 张范围内图上，629 / 629 条有序列号的管线 ITEMTAG 与参考名逐字相等。按名配对失败的 14 条都是拼法与 ITEMTAG 不一致：

| 图 | 我们 | 参考（= ITEMTAG） | 差在哪 |
|---|---|---|---|
| L M ×3 | `100 mm-PG-1301126-D8-P40` 等 | `40 mm-PG-1301126-D8-P40` 等 | `T_PIPERUN.NOMINALDIAMETER` 与 ITEMTAG 不一致 |
| 轻烃I ×3 | `-PG-1235-` / `-PG-1256-` / `-PG-1257-` | `-PG-1235` 等 | 空的材料等级槽多一个尾 `-` |
| 空冷器A ×6 | `-PC-1127-` / `-WO-1133-` 等 | `-PC-1127` / `-WO-1133` 等 | 同上 |
| 空冷器A/B/C 共用 1 条 | `80 mm-0107202-I2` | `80 mm-AI-0107202-I2` | 流体码 20 在 codelist 19 查不到，槽被省掉 |

空冷器 A/B/C 的那一条是同一条跨图管线（UID `5FC4FECA…` / 参考 `E5EFCC63…`），所以三张图共用一个根因。

### 根因 B'：轻烃I 两条只在我们这边的管线

`--65-AI-010901-I2-`（连接件 `BC395A09…`）与 `--532-`（连接件 `DF07DCAF…`）：`T_PIPERUN` 有序列号，但参考里没有任何 `PIDPipeline` / `PipingConnectors` 收它们；对应 ITEMTAG 分别是 `02534` / `02532`。哪一列决定一条 `T_PIPERUN` 不成为 `PIDPipeline`，目前没有证据。

### 根因 C：J K井场 3 个只在 dump 里的管件

`PIDPipingComponent` `7A46D893…` / `85E62159…` / `C5330847…`，各带 2 个 `PIDPipingPort` 与 1 个 `PIDRepresentation`（GraphicOID 44116 / 50406 / 38704），共 12 个对象，外加 12 条 `DrawingItems` / `DwgRepresentationComposition` / `PipingPortComposition`。这些 UID 不在 `.pid` 里（参考 `.pid` 与包内 `.pid` 字节相同），只在 `T_MODELITEM`（PipingComp, SP_ISTYPICAL=1）、`T_PLANTITEM`、`T_PIPINGCOMP`、`T_INLINECOMP`、`T_PIPINGPOINT`、`T_REPRESENTATION`（`SP_DRAWINGID` = J K，type 13，INSTOCKPILE=1）里。该图 2325 行 `T_REPRESENTATION` 中，type 13 两边都有的 503 行，只在参考的恰好就这 3 行。

### 参考侧版本差（不修，P2 登记）

FluidSystem 参考为空、dump 有值：J K 34、L M 42、空冷器 A/B/C 各 3。现在的上界是一个全局常量 `FLUID_SYSTEM_REFERENCE_EMPTY_ALLOWANCE = 23`（按 I井场定的），J K 与 L M 超界，所以单图跑出 exit 1。

## 早先误判、已排除

- 自写比对脚本没有反转义 `&quot;`，造成大量假的名称差异；CLI 本身正确。
- 连接件 `NominalDiameter` 的 `6"` / `6 in` 拼法差也是脚本假象：CLI 输出里 values different 为 0。

## 决策与修复进度（2026-11-05）

| 根因 | 决定 | 状态 |
|---|---|---|
| B 管线名 | 取 `T_PLANTITEM.ITEMTAG`，没有 plant item 时退回拼名 | 已修：L M、轻烃I 两张归零；空冷器 A/B/C 管线名全部配上 |
| B' 不成管线 | ITEMTAG 为空或不含 `-` 就不成管线（全 dump 13 行；另一候选「OPERFLUIDCODE 为空」会去掉 160 行，未采用） | 随 B 一起修 |
| C J K 3 个 dump-only 管件 | 按 `T_REPRESENTATION` 合成对象与 rel | 已修：J K 只剩 FluidSystem 上界一行 |
| FluidSystem 上界 | 改为按图登记观测值：`FLUID_SYSTEM_REFERENCE_EMPTY_ALLOWANCES` 按参考 `DocUID` 一行，未登记的图上界为 0 | 已修：7 张图 `--oracle-exp` 单图全部 exit 0 |
| D 空冷器 A/B/C 共用管线的 `IFluidSystem.FluidCode` | `OPERFLUIDCODE` 经 `SP_19` 映到标准 `{GUID}` 才写（样本只有 code 20 一个值） | 已修：空冷器 A/B/C、轻烃I exit 0；全包多写 44 个 `FluidCode` |

B 修完之后，L M（只剩 FluidSystem 42）和轻烃I（exit 0）的单图差异只剩已登记类；空冷器 A/B/C 各剩一条
`missing value PIDPipeline E5EFCC63… IFluidSystem.FluidCode: "@{663C8387-…}" not in generated`。

### 根因 D：`IFluidSystem.FluidCode`

7 张参考图里只有这一条管线写了 `FluidCode`。它的 run 是 `OPERFLUIDCODE=20`、`FLUIDSYSTEM=1`；`FluidCode` 的值是数据映射里 `SP_19_20 → {663C8387-FD73-449D-9ABF-BB3ECC4037BE}`。

- `SPPIDDataMap.xml`（`RefData~4~804.zip`）：`SPMP_Pipeline_FluidSystem` 的 `MapPropertyToProperty` 指向 `FluidCode`，`ProcessPropertyCriteria="EFAdapter.EnumConversions"`；`SPMP_Pipeline_OperFluidCode` 没有 `MapPropertyToProperty`，处理串为空。也就是说映射里能写进 `PIDPipeline.FluidCode` 的只有 FluidSystem 这条属性。
- 这条管线的 FluidSystem 值 `{FD96270A…}` 是 `SP_20_1` 的目标；`FluidCode` 的 `{663C8387…}` 在映射里只出现一次，是 `SP_19_20` 的目标。所以 `FluidCode` 的值来自 OperFluidCode（`SP_19`），不是 FluidSystem 那条属性自己的枚举。
- `EFAdapter.dll`（IDA 9.2，ida-bridge，库副本在 `D:\ida_scratch\publish-native\efadapter-fluid\`）：`EnumConversions` 是 `sub_5F5F4D60`。它是通用的枚举翻译：沿 `MapPropertyToProperty` / `MapEnumToEnum` / `MapEnumListMapEnumList` 查目标，查不到时记 `Warning!!! Found an unmapped SPMapProperty`。反汇编里没有针对某个代码或某条管线的特判，看不出离线可复现的「写 / 不写」判据。一个可能的解释是 FluidSystem 与 OperFluidCode 是同一棵层级枚举的上下两层、经 `MapEnumListMapEnumList` 取下层值，但这一点没有在反汇编里坐实。
- 数据侧：7 张参考图的 629 条 `PipingConnectors`，按管段 `OPERFLUIDCODE` 的 `SP_19` 映射目标分：无映射 503 条、目标是 `EE_*` 工程扩展码 123 条、目标是 `{GUID}` 标准枚举 3 条。写了 `FluidCode` 的恰好是这 3 条（同一条管线出现在 3 张图上），其余 626 条都没写。所以这批样本里「`SP_19` 目标是标准 `{GUID}` 才写」与观测完全一致，但正例只有一个值（code 20）。

采用的规则：`OPERFLUIDCODE` 的 `SP_19_{code}` 目标是 8-4-4-4-12 的标准 GUID 时写 `FluidCode`，是 `EE_*` 或没有映射时不写（数据映射把每个目标都拼成 `@{…}`，所以要看括号里的形状）。全 dump 有 184 个 `SP_19` 代码映到标准 GUID，但 7 张参考图里实际出现、能检验这条规则的只有 code 20。别的标准代码一旦在参考图里出现、却没写 `FluidCode`，这条规则就要改。

### 根因 C 的规则与外推面

规则：`T_REPRESENTATION` 中 `SP_DRAWINGID` 是本图、`REPRESENTATIONTYPE` = 13、模型项 `ITEMTYPENAME` = `PipingComp`，且这个 representation 不在 `.pid` 里，就发布 `PIDPipingComponent`（带 `T_INLINECOMP.NOMINALDIAMETER`）、它的 `PIDRepresentation`，以及 `T_PIPINGPOINT` 端口。端口和 `PipingPortComposition` / `DrawingItems` / `DwgRepresentationComposition` 都走已有的路径。

7 张参考图里命中这条规则的 representation 只有 J K 的 3 行，3 行都在参考里（3/3，没有反例）。另外 6 张图没有命中。全包里还有 C井场命中 27 行（同样是 `INSTOCKPILE=1`、`SP_ISTYPICAL=1` 的阀门、过滤器、异径管），所以 C井场多了 108 个对象、108 条 rel、140 个值。C井场没有参考图，这部分是外推，没有验证过。

参考图里 `IPipingComponent.PipingComponentType*`、`PipModelCode` 和 `IPBSItem` 这类接口值，本仓库的 `.pid` 路径对已画出的管件也不写（对比时按 vendor composed 豁免）。合成出来的管件同样不写，只写 `NominalDiameter`。

## P2 登记与 P3 重发

P2：`docs/baselines/publish-diff-accepted.json` 的 `acceptance_figures` 登记 6 张图，每张分 `.pid` 单发与 `--oracle-exp` 两种模式，记下退出码、摘要行和打印出来的每一条差异。12 次运行里 11 次 exit 0。另一次是 J K 的 `.pid` 单发：没有 dump 就没有那 3 个管件，所以登记了 6 条 `missing`（3 个 `PIDPipingComponent` 和 3 个 `PIDRepresentation`）。`tests/publish_diff_accepted_baseline.rs` 每张图一条测试。图纸和参考从 `PID_ACCEPTANCE_CORPUS` 读，dump 从 `PID_TEST_DATA_DIR` 读，缺了就 soft-skip。

P3：整包重发到 `D:\work\cad\pid-test-data\publish-v3\`（160 件），`_ByteCompare.json` 逐件对 `publish-v2`：

- `_Meta.xml` 53 件逐字节不变。
- `_Data.xml` 43 件变化，10 件不变。每一件都能归到上面的根因：管线块 142 个改名（B，其中 44 个同时写了 `FluidCode`，D），26 个不再成管线（B'），新增 30 个 `PIDPipingComponent`（C，C井场 27 个、J K 3 个），每个带 2 个端口和 1 个 representation。
- `_Coverage.json` 51 件变化：每张图的 `pipeline_synthesis` 多了 `item_tag_not_a_name` 计数，管线有变化的图同时更新了计数。
- `_Index.json` 合计：对象 80 085 → 80 179，值 87 419 → 87 598，关系 106 705 → 106 812，管线 2 395 → 2 369，`PipingConnectors` 3 128 → 3 115，管道端口 9 557 → 9 617。
- 同一命令再跑一次，160/160 逐字节相同。

## 补记：4 张旧集注站图的 `--oracle-exp` 是配对错误，不是干净运行（2026-11-05 晚）

上一节把 10 张有发现的图里 4 张旧集注站图（凝析油 A/B、换热器 CD、低温分离器 A）记为
「DocUID 不在这份备份里，被同名不同 UID 的参考误配」。这句只说了参考误配，漏了更严重的一半：
**这 4 张图的 `.pid` 与 `Export.dmp` 也不是同一版本**，而 `--oracle-exp` 当时不检查这一点。

### 证据

`T_DRAWING` 53 行，每行 `SP_ID` 是图纸的 `DocUID`。把 4 张图的 `DocUID` 按 UTF-16BE
（dump 的字符列编码）在 `Export.dmp` 的 74,489,856 字节里数命中：

| 图 | DocUID | dump 内命中 |
|---|---|---|
| 凝析油A | `D710241072D54ECCB8F5961357166E5D` | 0 |
| 凝析油B | `AAFCDC31FA3C4177BECB02EBBB6F776F` | 0 |
| 换热器CD | `5A552026D9144FF9BA325D9C8160064C` | 0 |
| 低温分离器A | `DABA92D1739740EAB431DAC6DF5F6419` | 0 |
| 对照：J K井场 | `C80930B9697B4F2190640D8141C56FF7` | 3 743 |
| 对照：轻烃分离器I | `A77BEFB93BFA4DFAB366BAB069D90B1A` | 1 015 |

4 张图的 `DocUID` 同时也是它们自己 `_Data.xml` 的 `<Container DocUID>`。dump 里根本没有这 4 张图，
所以 `T_PIPINGPOINT` / `T_CONNECTOR` / `T_PIPERUN` / `T_REPRESENTATION` 一行都不属于它们。
运行输出也印证：`oracle-exp pipelines: 0 PIDPipeline`、`connectivity: 0 pairs`、`opc: 0 PIDBranchPoint`，
而 `compositions: 377 nozzle pairs` 是全 dump 的共享值，不是本图的。

结论：这 4 次 `--oracle-exp` 实际只是 `.pid` 单发，却报 exit 0。参考图恰好也是这 4 张图自己的
`_Data.xml`，`.pid` 单发本来就能对上，于是「干净」是两次错误互相抵消的结果。

### 修复

1. **CLI 守卫（主修复）**：`src/publish/oracle_exp.rs` 新增
   `drawing_in_dump(&ExpTables, uid) -> Result<bool, ExpError>`（折叠 `DocUID` 后与
   `T_DRAWING.SP_ID` 比对，带 `///` 与命中/未命中两条单元测试）。`run_pid` 读完 `ExpTables` 后
   立刻调用，未命中就打印
   `argument error: <dump> has no T_DRAWING row for DocUID <uid>; the .pid is not this dump's revision`
   并退出 2，不写出任何文件。
2. **diff 兜底**：`report_pid_diff` 在 `--oracle-exp` 模式下，把「参考侧整类被
   `this run published no <class>` 豁免、而目录说该类是 `PidTagRole::Port` /
   `PidTagRole::Enrichment`」算作发现项，每类打印
   `oracle-exp run excused <class> ×N the dump should supply` 并 exit 1。`.pid` 单发模式不动，
   `diff_pid_export` 签名不动（新增 `PidExportDiff::dump_supplied_excused` 字段）。
   **先查了有没有合理豁免**：6 张验收图 + I井场 的 `--oracle-exp` 运行里这类行 0 条，所以没有
   收窄到「参考里有 ≥1、生成侧为 0」的必要；规则按「该类在目录里是 dump 角色且本次豁免了」
   直接生效。

### 对上一轮「重建基线」建议的更正

上一节把 4 张图归到「范围外」，隐含的处理是以后补参考、重建基线。这次更正为：**这 4 张图
无法用这份 `Export.dmp` 建 `--oracle-exp` 基线**——dump 里没有它们的任何内容，`--oracle-exp`
对它们只能失败。它们被记进 `docs/baselines/publish-diff-accepted.json` 的新段
`revision_mismatch_figures`（4 张图 + 两边 `DocUID` + 期望 exit 2），由
`tests/publish_diff_accepted_baseline.rs::a_drawing_absent_from_the_dump_is_refused_not_silently_published`
逐张断言。它们的 `.pid` 单发行为不变，仍 exit 0。

轻烃分离器I 与空冷器A 的现有基线不动：`DocUID` 在 dump 里，配对正确。

### 未受影响

- 整包模式（`--out-dir` 指向备份目录）按 `T_DRAWING` 找 `.pid`，天然一致，本改动不经过它。
  抽查重发一张图（J K井场），`_Data.xml` / `_Meta.xml` 与 `publish-v3` 逐字节相同。
- 7 张参考图的 `--oracle-exp` 单图运行全部仍 exit 0。
