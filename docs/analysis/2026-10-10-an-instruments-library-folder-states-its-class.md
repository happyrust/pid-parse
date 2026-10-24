# 符号库目录决定仪表的类：287 个对上，错 0（2026-10-10）

> 承接计划 `docs/plans/2026-10-10-publish-from-the-pid-alone.md`。P5 交完之后用户拍板：68 个定不了类的仪表按 Q2 (b) 另开小单，用符号库路径推类。
> 探针 `examples/probe_instruments_without_symbol_attr.rs`（只读，不写文件）。落成规则是 `publish::pid_load::instrument_class_by_library`，只在 `SymbolAttr` 定不了类时用。

## 一句话

`Instrumentation` 紧后面的那一段目录，和参考 `_Data.xml` 里的类，在 5 张图全部 287 个仪表上从不打架：`In-Line` 66 与 `Relief Devices` 6 是 `PIDInlineInstrument`，`Off-Line` 116 与 `Actuators` 11 是 `PIDInstrument`，`System Functions` 88 是 `PIDControlSystemFunction`。
`SymbolAttr` 定不了类的 68 个改按这一段归类之后，仪表从 219 / 287 收到 287 / 287，错类 0、错值 0。

## 口径（打分前写在探针里的，和后来落成的，不是同一条）

探针模块注释里预登记的候选是 `class_by_path`：路径上任意目录段（大小写不敏感）叫 `system functions` → `PIDControlSystemFunction`，叫 `in-line` / `in-line instruments` / `inline instruments` → `PIDInlineInstrument`，叫 `off-line instruments` / `offline instruments` → `PIDInstrument`，否则未决。
这条候选没有 `Actuators`、`Off-Line`、`Relief Devices`。下面的干跑是它的成绩，不是落成规则的成绩。

落成规则只看 `Instrumentation` 的下一段：`in-line` / `relief devices` → `PIDInlineInstrument`，`off-line` / `actuators` → `PIDInstrument`，`system functions` → `PIDControlSystemFunction`。大小写不敏感，`\` 和 `/` 都认；文件名不算目录段。

## 287 个仪表的库路径

5 张图 287 个 `ModelItemType = Instrument`。67 个图形上没有 `SymbolAttr` 行，这 67 个都有一条 `igSymbol2d` 库路径。

| 图 | 仪表 | 无 `SymbolAttr` 行 | 其中有库路径 |
|---|---:|---:|---:|
| I井场 注采阀组 | 150 | 30 | 30 |
| 一级闪蒸分离器A | 17 | 2 | 2 |
| 一级闪蒸分离器B | 17 | 2 | 2 |
| 低温分离器A | 30 | 5 | 5 |
| 换热器CD及外输阀组 | 73 | 28 | 28 |
| **合计** | **287** | **67** | **67** |

按整条路径归（探针「per library path」；每条路径只出现一个参考类）：

| `Instrumentation` 下一段 | 参考类 | 有 `SymbolAttr` 行 | 无行 | 合计 |
|---|---|---:|---:|---:|
| `In-Line` | `PIDInlineInstrument` | 10 | 56 | 66 |
| `Relief Devices` | `PIDInlineInstrument` | 6 | 0 | 6 |
| `Off-Line` | `PIDInstrument` | 116 | 0 | 116 |
| `Actuators` | `PIDInstrument` | 0 | 11 | 11 |
| `System Functions` | `PIDControlSystemFunction` | 88 | 0 | 88 |
| **合计** | | **220** | **67** | **287** |

没有一条路径对上两个类。再往上的目录不能当规则：无行的那 67 个里，目录段 `instrumentation` 是在线 56 + 离线 11。能分开的是它的下一段——`in-line` 56 个全是在线，`actuators` 11 个全是离线（探针「per directory segment」，只统计无行的仪表）。

## 预登记候选的干跑

`class_by_path` 过全部 287 个，错 0：

| | 对 | 错 | 未决 |
|---|---:|---:|---:|
| 无 `SymbolAttr` 行 | 56 | 0 | 11 |
| 有 `SymbolAttr` 行 | 98 | 0 | 122 |

未决的 11 个全是执行机构，参考类都是 `PIDInstrument`，路径都在 `Instrumentation\Actuators\` 下。候选规则没写这个目录。
有行而未决的 122 个 = `Off-Line` 116 + `Relief Devices` 6。候选找的目录名是 `off-line instruments`，文件里的目录是 `Off-Line`；安全阀那个目录候选里也没有。有行而对上的 98 个 = `System Functions` 88 + `In-Line` 里有行的 10。

## 空的那一行 `SymbolAttr`

装载器的 `instruments_classed_by_library` 五张图是 31 + 2 + 2 + 5 + 28 = 68，比「没有行」多 1，多在 I井场。
多出来的是 `F91061AAA58349919216D6E6E579A3C5`：图形上有 `SymbolAttr` 行，`Type` 和 `Class` 都是空字符串，所以探针把它算进「有行」（路径表里 `PV-Angle Throttle Control Valve.sym` 那一行的 with = 1）。`instrument_class` 不认空行。路径是 `Instrumentation\In-Line\Control Valves\PV-Angle Throttle Control Valve.sym`，参考类 `PIDInlineInstrument`，库目录把它收成在线仪表。
68 = 56 个无行的 `In-Line` + 这 1 个空行 + 11 个 `Actuators`。`Off-Line`、`Relief Devices`、`System Functions` 和其余 9 个有行的 `In-Line` 仍由 `SymbolAttr` 定类，不进这个计数。

## 收回之后导出器实测

`cargo test --test publish_from_pid -- the_pid_alone_export_matches_the_reference_wherever_it_says_anything`：五张图错类 / 错值 / 多发对象 / 多发关系全是 0。括号里是相对收回前钉数的差。

| 图 | 对象 | 值 | 关系 | 按库归类 |
|---|---:|---:|---:|---:|
| I井场 注采阀组 | 1 459（+31） | 1 405（+33） | 1 695（+31） | 31 |
| 一级闪蒸分离器A | 291（+2） | 289（+3） | 335（+2） | 2 |
| 一级闪蒸分离器B | 239（+2） | 230（+3） | 274（+2） | 2 |
| 低温分离器A | 317（+5） | 315（+6） | 358（+5） | 5 |
| 换热器CD及外输阀组 | 612（+28） | 583（+30） | 727（+28） | 28 |
| **合计** | **2 918（+68）** | **2 822（+75）** | **3 389（+68）** | **68** |

按类（发出 / 参考；有名字的发出 / 参考）：

| 类 | 收回前 | 收回后 |
|---|---|---|
| `PIDInlineInstrument` | 15 / 72，名字 4 / 39 | 72 / 72，名字 27 / 39 |
| `PIDInstrument` | 116 / 127，名字 111 / 118 | 127 / 127，名字 113 / 118 |
| `PIDControlSystemFunction` | 88 / 88，名字 79 / 79 | 不动 |

新发出的 68 个对象就是这 68 个仪表；关系 +68 全是 `DrawingItems`（1 498 → 1 566），`DwgRepresentationComposition` 仍是 1 823。值 +75 = 拼出来的名字 +25（在线 +23、离线 +2）+ 标签值 +50（712 → 762）。`GraphicOID` 1 823 与 5 个图名不动。名字合计：设备 13 + 仪表 219 = 232。
五份 `_Coverage.json` 里没定类的物项共 58 个，全是 `OPC`。仪表不在这份清单里。

分离器 B 的 stderr：100 个物项（87 个有类，其中 2 个按库目录），151 个表示，406 个标签值 400 个挂上，63 条属性发出，0 条打架。收回前是 85 个有类、61 条属性。

## 探针全文

debug 构建跑 5 张图约 2 s。预登记干跑的「wrong」一节没有输出（0 条）。

```text
==== I井场 注采阀组工艺及自控流程图: 150 instruments, 30 without SymbolAttr, 30 of those with a library path, 0 graphics without one
  199E7713EC2C41498863395F7C4D6D67 ref=PIDInlineInstrument        graphics=[5868] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Control Valves\PV-Pressure Regulating Valve with Flange.sym
  1B2D0EC65CC74E7BBEAD26698DA51105 ref=PIDInlineInstrument        graphics=[36923] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Valves\2 Way Common\2 Way Generic Body Type 1.sym
  1ED617DAEFA143AD95F34664EBA521F0 ref=PIDInlineInstrument        graphics=[34302] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Valves\2 Way Common\2 Way Generic Body Type 1.sym
  202ED19E522E4C06947AB2A7EA9C3315 ref=PIDInlineInstrument        graphics=[33998] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Valves\2 Way Common\2 Way Generic Body Type 1.sym
  22DABD1E3B45447796E51222618449FC ref=PIDInlineInstrument        graphics=[265] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Valves\2 Way Common\2 Way Generic Body Type 1.sym
  3264C82472CA4C49BA0A4B5AE8807E76 ref=PIDInlineInstrument        graphics=[35184] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Valves\2 Way Common\2 Way Generic Body Type 1.sym
  343CE6E74CE842E2820759F4B5C53301 ref=PIDInlineInstrument        graphics=[5322] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Control Valves\PV-Pressure Regulating Valve with Flange.sym
  35983A1E7E334CB5AECCBBB65F768288 ref=PIDInlineInstrument        graphics=[3226] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Valves\2 Way Common\2 Way Generic Body Type 1.sym
  441F625998E248EBB6FB76D4E48816F7 ref=PIDInstrument              graphics=[2475] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\Actuators\Diaphragm Actuator.sym
  46D8B47328ED4A9DBDAA175CE71CA127 ref=PIDInlineInstrument        graphics=[4823] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Control Valves\PV-Pressure Regulating Valve with Flange.sym
  4E271DD1C60C4A1CA01C701437C56FA8 ref=PIDInlineInstrument        graphics=[36621] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Valves\2 Way Common\2 Way Generic Body Type 1.sym
  50E390AF95F043D7A18A5EA9D325E61A ref=PIDInlineInstrument        graphics=[531] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Valves\2 Way Common\2 Way Generic Body Type 1.sym
  696BFED885D349D79BAB0ED1110F51F2 ref=PIDInlineInstrument        graphics=[6142] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Control Valves\PV-Pressure Regulating Valve with Flange.sym
  739CB74286314FC7B9AA2578A7A8753B ref=PIDInlineInstrument        graphics=[5064] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Control Valves\PV-Pressure Regulating Valve with Flange.sym
  8555AE64E55D4F788752669B557A4B8B ref=PIDInlineInstrument        graphics=[39158] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Valves\2 Way Common\2 Way Generic Body Type 1.sym
  8EF44A3111B24157A53B09C24F2E81F8 ref=PIDInlineInstrument        graphics=[35494] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Valves\2 Way Common\2 Way Generic Body Type 1.sym
  9678564BC24542869AB6D2F8163B49D6 ref=PIDInlineInstrument        graphics=[4328] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Valves\2 Way Common\2 Way Generic Body Type 1.sym
  9A6C9C9BC17045A597423B2523DF9509 ref=PIDInlineInstrument        graphics=[36334] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Valves\2 Way Common\2 Way Generic Body Type 1.sym
  A2DA6EBD74914E87A5F20526DDA973A8 ref=PIDInstrument              graphics=[2746] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\Actuators\Diaphragm Actuator.sym
  A805D51CB84840A9933F78F951B8470F ref=PIDInlineInstrument        graphics=[38889] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Valves\2 Way Common\2 Way Generic Body Type 1.sym
  AAD117E661A34CA39FFC67E8B6B9A0BD ref=PIDInlineInstrument        graphics=[33714] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Valves\2 Way Common\2 Way Generic Body Type 1.sym
  BEFFE2E04FF649FB81448EE57BF99590 ref=PIDInlineInstrument        graphics=[35775] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Valves\2 Way Common\2 Way Generic Body Type 1.sym
  BFCE7992782E4F04B936C2D1F7067C4D ref=PIDInlineInstrument        graphics=[5568] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Control Valves\PV-Pressure Regulating Valve with Flange.sym
  C5238755D93B493CA7E45A33FAF92B39 ref=PIDInlineInstrument        graphics=[2947] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Valves\2 Way Common\2 Way Generic Body Type 1.sym
  C9DC1D18FD884816B60227A5FBCF3424 ref=PIDInlineInstrument        graphics=[34902] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Valves\2 Way Common\2 Way Generic Body Type 1.sym
  D2FC479A922A431CBE9AC4377640C4F6 ref=PIDInlineInstrument        graphics=[34594] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Valves\2 Way Common\2 Way Generic Body Type 1.sym
  D5B98595953D4C21883DDF5998AF7BD3 ref=PIDInlineInstrument        graphics=[36064] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Valves\2 Way Common\2 Way Generic Body Type 1.sym
  E74C16F77997460C905C430020641E3C ref=PIDInlineInstrument        graphics=[7622] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Control Valves\MOV-GLV-Electrically Operated Valve with Flange 02.sym
  E8C2D8BC48F74D6AAEB996F7934F05C3 ref=PIDInlineInstrument        graphics=[37205] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Valves\2 Way Common\2 Way Generic Body Type 1.sym
  F47316A9ED904208A5E2A5A09A20F6C9 ref=PIDInlineInstrument        graphics=[7332] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Control Valves\MOV-GLV-Electrically Operated Valve with Flange 02.sym

==== 集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图: 17 instruments, 2 without SymbolAttr, 2 of those with a library path, 0 graphics without one
  35F4406962A747BDA38AC5BE4454AAC3 ref=PIDInlineInstrument        graphics=[2634] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Valves\2 Way Common\2 Way Generic Body Type 1.sym
  B655BE041E564180AFD133884E8F62A8 ref=PIDInstrument              graphics=[3146] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\Actuators\Diaphragm Actuator.sym

==== 集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图: 17 instruments, 2 without SymbolAttr, 2 of those with a library path, 0 graphics without one
  2B049D8542C140E5A4E710D41A563AEE ref=PIDInlineInstrument        graphics=[1384] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Valves\2 Way Common\2 Way Generic Body Type 1.sym
  F45B9E35DC064270805DB1223A5F594F ref=PIDInstrument              graphics=[791] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\Actuators\Diaphragm Actuator.sym

==== 集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图: 30 instruments, 5 without SymbolAttr, 5 of those with a library path, 0 graphics without one
  A58A51783E544EA2818C5CC673CD9FF3 ref=PIDInlineInstrument        graphics=[3524] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Valves\2 Way Common\2 Way Generic Body Type 1.sym
  B3F5A6B68CC14306872A9916FCDA940C ref=PIDInlineInstrument        graphics=[4052] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Control Valves\ESDV-Emergency Stop Valve.sym
  BEB1EF85C28441C38BCABCDC24EB3D81 ref=PIDInstrument              graphics=[2660] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\Actuators\Diaphragm Actuator.sym
  D384C839EFD944B59546781B9E5AE5A6 ref=PIDInlineInstrument        graphics=[1760] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Valves\2 Way Common\2 Way Generic Body Type 1.sym
  DD89C1EA2F934FA1BAC948D7833A2522 ref=PIDInlineInstrument        graphics=[4583] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Control Valves\3-Way Solenoid Valve.sym

==== 集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图: 73 instruments, 28 without SymbolAttr, 28 of those with a library path, 0 graphics without one
  0166D61BB9AA4048A042EDCC41B3D4FA ref=PIDInlineInstrument        graphics=[1789] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Valves\2 Way Common\2 Way Generic Body Type 1.sym
  16558AE44D80404B99B3A587EEFE718B ref=PIDInlineInstrument        graphics=[1140] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Valves\2 Way Common\2 Way Ball Type 2.sym
  1B0995D2DD5341A9A16BB91BDC87A3B1 ref=PIDInlineInstrument        graphics=[1714] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Valves\2 Way Common\2 Way Generic Body Type 1.sym
  203BFEB4C3944D10941631C3FEDB3FA8 ref=PIDInstrument              graphics=[374] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\Actuators\Diaphragm Actuator.sym
  3290230B1B96474C93AB9A4C9A1AADC2 ref=PIDInstrument              graphics=[1168] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\Actuators\Dual Action Cyl Act.sym
  355216909AE149C28FFBC741471727E7 ref=PIDInstrument              graphics=[446] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\Actuators\Diaphragm Actuator.sym
  35C95575CEAD49AAA921CE554FD69757 ref=PIDInstrument              graphics=[1729] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\Actuators\Dual Action Cyl Act.sym
  39C1135C85FE4FA79803BE462DA31784 ref=PIDInlineInstrument        graphics=[1343] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Valves\2 Way Common\2 Way Generic Body Type 1.sym
  3DF055307A044D75A08A7AEB48BF3325 ref=PIDInlineInstrument        graphics=[1528] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Valves\2 Way Common\2 Way Generic Body Type 1.sym
  43783E0D2BEB4D12BDE4150BCC2EFE09 ref=PIDInstrument              graphics=[1249] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\Actuators\Dual Action Cyl Act.sym
  43F7F575FF234F7FAAE993D1AD34A44D ref=PIDInlineInstrument        graphics=[1490] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Control Valves\BDV-Emergency Vent Valve with Flange.sym
  46AA9E8CFF00496C867ACFD229595020 ref=PIDInlineInstrument        graphics=[1945] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Valves\2 Way Common\2 Way Generic Body Type 1.sym
  4F99258B8842477A8DE13D929219B081 ref=PIDInlineInstrument        graphics=[1543] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Control Valves\ESDV-Emergency Stop Valve.sym
  520C24A1372541D18045C18F2C7E290A ref=PIDInlineInstrument        graphics=[1433] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Valves\2 Way Common\2 Way Generic Body Type 1.sym
  52E12BD0E5C54FDAB4E9CCE71E2763B7 ref=PIDInstrument              graphics=[1358] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\Actuators\Dual Action Cyl Act.sym
  5EDF8C78EFE04C66B06D5F82DE15AECA ref=PIDInlineInstrument        graphics=[1187] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Control Valves\MOV-Electrically Operated Valve with Flange.sym
  706FAB339C3E4364B19391511EDF569F ref=PIDInlineInstrument        graphics=[1462] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Valves\2 Way Common\2 Way Generic Body Type 1.sym
  7D437152FCBC4CE6BC6EC2D861CA7405 ref=PIDInlineInstrument        graphics=[169] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Flow\FE-Orifice Flowmeter.sym
  7F6E6AF0560648219FDBC9215AB728F7 ref=PIDInlineInstrument        graphics=[1638] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Control Valves\3-Way Solenoid Valve.sym
  8F477D31F29742C8B1714AC71D50C2CA ref=PIDInlineInstrument        graphics=[1309] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Control Valves\MOV-Electrically Operated Valve with Flange.sym
  9F3DAB6027A34A29BBC4C1C3770A528A ref=PIDInlineInstrument        graphics=[1579] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Valves\2 Way Common\2 Way Generic Body Type 1.sym
  B122B95D8E164CD1981DB2F394894F9C ref=PIDInlineInstrument        graphics=[358] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Valves\2 Way Common\2 Way Generic Body Type 1.sym
  C333C772AAE34911BC20D88A5834C198 ref=PIDInlineInstrument        graphics=[1846] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Valves\2 Way Common\2 Way Generic Body Type 1.sym
  CAC7592421824064B0324606858509B6 ref=PIDInlineInstrument        graphics=[2059] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Valves\2 Way Common\2 Way Generic Body Type 1.sym
  D9CC594A62CD430E96E3D8E2E90F4F16 ref=PIDInlineInstrument        graphics=[2002] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Valves\2 Way Common\2 Way Generic Body Type 1.sym
  E803536F879E421CB269217C0F3DE572 ref=PIDInlineInstrument        graphics=[1623] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Valves\2 Way Common\2 Way Generic Body Type 1.sym
  F4A5D2A914874F44A3E5FC3714F1D8E5 ref=PIDInlineInstrument        graphics=[431] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Valves\2 Way Common\2 Way Generic Body Type 1.sym
  F5A35B9213444B2997B7FE32156A341E ref=PIDInlineInstrument        graphics=[1222] path=\\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Valves\2 Way Common\2 Way Ball Type 2.sym

==== per library path (all 287 instruments): with SymbolAttr / without -> reference class
  \\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\Actuators\Diaphragm Actuator.sym
      with: 
      without: PIDInstrument 7
  \\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\Actuators\Dual Action Cyl Act.sym
      with: 
      without: PIDInstrument 4
  \\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Control Valves\3-Way Solenoid Valve.sym
      with: 
      without: PIDInlineInstrument 2
  \\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Control Valves\BDV-Emergency Vent Valve with Flange.sym
      with: 
      without: PIDInlineInstrument 1
  \\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Control Valves\ESDV-Emergency Stop Valve.sym
      with: 
      without: PIDInlineInstrument 2
  \\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Control Valves\MOV-Electrically Operated Valve with Flange.sym
      with: 
      without: PIDInlineInstrument 2
  \\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Control Valves\MOV-GLV-Electrically Operated Valve with Flange 02.sym
      with: 
      without: PIDInlineInstrument 2
  \\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Control Valves\PV-Angle Throttle Control Valve.sym
      with: PIDInlineInstrument 1
      without: 
  \\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Control Valves\PV-Pressure Regulating Valve with Flange.sym
      with: 
      without: PIDInlineInstrument 6
  \\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Flow\FE-Orifice Flowmeter.sym
      with: 
      without: PIDInlineInstrument 1
  \\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Flow\FT-Electromagnetic Flowmeter.sym
      with: PIDInlineInstrument 1
      without: 
  \\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Flow\FT-Rotor Flowmeter.sym
      with: PIDInlineInstrument 4
      without: 
  \\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Flow\FT-Ultrasonic Flowmeter.sym
      with: PIDInlineInstrument 4
      without: 
  \\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Valves\2 Way Common\2 Way Ball Type 2.sym
      with: 
      without: PIDInlineInstrument 2
  \\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\In-Line\Valves\2 Way Common\2 Way Generic Body Type 1.sym
      with: 
      without: PIDInlineInstrument 38
  \\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\Off-Line\Without Implied Components\General\I-P Converter.sym
      with: PIDInstrument 3
      without: 
  \\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\Off-Line\Without Implied Components\General\P-I Converter.sym
      with: PIDInstrument 2
      without: 
  \\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\Off-Line\Without Implied Components\Level\LG-Magnetic Float Gauge.sym
      with: PIDInstrument 6
      without: 
  \\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\Off-Line\Without Implied Components\Level\LT-DP Level Transmitter.sym
      with: PIDInstrument 10
      without: 
  \\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\Off-Line\Without Implied Components\Pressure\PDT-D-P Transmitter.sym
      with: PIDInstrument 1
      without: 
  \\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\Off-Line\Without Implied Components\Pressure\PG-Pressure Gauge.sym
      with: PIDInstrument 21
      without: 
  \\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\Off-Line\Without Implied Components\Pressure\PT-Pressure Transmitter.sym
      with: PIDInstrument 19
      without: 
  \\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\Off-Line\Without Implied Components\Single Function\Disc Single-Func Field Mounted.sym
      with: PIDInstrument 32
      without: 
  \\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\Off-Line\Without Implied Components\Temperature\TG-Temperature Gauge.sym
      with: PIDInstrument 12
      without: 
  \\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\Off-Line\Without Implied Components\Temperature\TT-Temperature Transmitter.sym
      with: PIDInstrument 10
      without: 
  \\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\Relief Devices\Safety Valve Type 2.sym
      with: PIDInlineInstrument 6
      without: 
  \\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\System Functions\D C S\DCS Func Access in Prime Loc.sym
      with: PIDControlSystemFunction 74
      without: 
  \\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\System Functions\Interlocks\Turn off the system signal.sym
      with: PIDControlSystemFunction 7
      without: 
  \\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\System Functions\P L C\ESD Interlock.sym
      with: PIDControlSystemFunction 2
      without: 
  \\MM-128\PID_SQProject\SQPlant\Ref\Symbols\Instrumentation\System Functions\P L C\PLC Func Access in Prime Loc.sym
      with: PIDControlSystemFunction 5
      without: 

==== per directory segment (instruments without SymbolAttr) -> reference class
  2 way common                             PIDInlineInstrument 40
  actuators                                PIDInstrument 11
  control valves                           PIDInlineInstrument 15
  flow                                     PIDInlineInstrument 1
  in-line                                  PIDInlineInstrument 56
  instrumentation                          PIDInlineInstrument 56, PIDInstrument 11
  mm-128                                   PIDInlineInstrument 56, PIDInstrument 11
  pid_sqproject                            PIDInlineInstrument 56, PIDInstrument 11
  ref                                      PIDInlineInstrument 56, PIDInstrument 11
  sqplant                                  PIDInlineInstrument 56, PIDInstrument 11
  symbols                                  PIDInlineInstrument 56, PIDInstrument 11
  valves                                   PIDInlineInstrument 40

==== dry run of class_by_path over all 287: (has SymbolAttr, outcome) -> count
  without            right      56
  without            undecided  11
  with SymbolAttr    right      98
  with SymbolAttr    undecided  122
  undecided (without SymbolAttr):
  I井场 注采阀组工艺及自控流程图: 441F625998E248EBB6FB76D4E48816F7 ref=PIDInstrument paths=["\\\\MM-128\\PID_SQProject\\SQPlant\\Ref\\Symbols\\Instrumentation\\Actuators\\Diaphragm Actuator.sym"]
  I井场 注采阀组工艺及自控流程图: A2DA6EBD74914E87A5F20526DDA973A8 ref=PIDInstrument paths=["\\\\MM-128\\PID_SQProject\\SQPlant\\Ref\\Symbols\\Instrumentation\\Actuators\\Diaphragm Actuator.sym"]
  集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图: B655BE041E564180AFD133884E8F62A8 ref=PIDInstrument paths=["\\\\MM-128\\PID_SQProject\\SQPlant\\Ref\\Symbols\\Instrumentation\\Actuators\\Diaphragm Actuator.sym"]
  集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图: F45B9E35DC064270805DB1223A5F594F ref=PIDInstrument paths=["\\\\MM-128\\PID_SQProject\\SQPlant\\Ref\\Symbols\\Instrumentation\\Actuators\\Diaphragm Actuator.sym"]
  集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图: BEB1EF85C28441C38BCABCDC24EB3D81 ref=PIDInstrument paths=["\\\\MM-128\\PID_SQProject\\SQPlant\\Ref\\Symbols\\Instrumentation\\Actuators\\Diaphragm Actuator.sym"]
  集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图: 203BFEB4C3944D10941631C3FEDB3FA8 ref=PIDInstrument paths=["\\\\MM-128\\PID_SQProject\\SQPlant\\Ref\\Symbols\\Instrumentation\\Actuators\\Diaphragm Actuator.sym"]
  集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图: 3290230B1B96474C93AB9A4C9A1AADC2 ref=PIDInstrument paths=["\\\\MM-128\\PID_SQProject\\SQPlant\\Ref\\Symbols\\Instrumentation\\Actuators\\Dual Action Cyl Act.sym"]
  集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图: 355216909AE149C28FFBC741471727E7 ref=PIDInstrument paths=["\\\\MM-128\\PID_SQProject\\SQPlant\\Ref\\Symbols\\Instrumentation\\Actuators\\Diaphragm Actuator.sym"]
  集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图: 35C95575CEAD49AAA921CE554FD69757 ref=PIDInstrument paths=["\\\\MM-128\\PID_SQProject\\SQPlant\\Ref\\Symbols\\Instrumentation\\Actuators\\Dual Action Cyl Act.sym"]
  集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图: 43783E0D2BEB4D12BDE4150BCC2EFE09 ref=PIDInstrument paths=["\\\\MM-128\\PID_SQProject\\SQPlant\\Ref\\Symbols\\Instrumentation\\Actuators\\Dual Action Cyl Act.sym"]
  集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图: 52E12BD0E5C54FDAB4E9CCE71E2763B7 ref=PIDInstrument paths=["\\\\MM-128\\PID_SQProject\\SQPlant\\Ref\\Symbols\\Instrumentation\\Actuators\\Dual Action Cyl Act.sym"]
```

## 复现

```powershell
cargo run --example probe_instruments_without_symbol_attr
cargo test --locked --test publish_from_pid -- --nocapture the_pid_alone_export_matches_the_reference_wherever_it_says_anything
cargo run --bin pid_publish_xml -- "D:\work\cad\PID自研软件验证\<图名>.pid" --out-dir "D:\work\cad\PID自研软件验证\publish" --plant P01
```
