# 库里画在图上、发布件里没有的表示：53 张整包对库的普查（2026-11-12）

> 提交：导出与普查用的是 `7de1ef1`（`main` `7f71586` 并入 `publish-export-20261110` 的合并提交），探针在 `3411263`。
> 数据：`D:\work\cad\pid-test-data`（53 张图的 Oracle `exp` 备份包）与 `D:\work\cad\pid-test-data\验收数据`（11 份原厂参考）。
> 交付件：`D:\work\cad\pid-test-data\publish-v4`，其中 `vs-database\` 是本页的原始输出。
> 承接：`docs/analysis/2026-11-10-publish-clean-is-nothing-wrong-not-everything-stated.md` §2.3「没有参考可比的」。
> 标「已核实」的是本轮亲自跑到或读到的；标「推断」的没有被数据钉死。

## 一句话

53 张图里 46 张没有同版的原厂参考，`--diff-against` 与完整度的钉都管不到它们。拿包里的数据库来对：51 张有内容的图上，库里画在图上的 42 048 条表示，发布件写出 23 762 条、没有一条是库里没有的；没写出的里 18 263 条属于原厂自己也不发的三类；**剩下 23 条（C井场 21、Q井场 2）是库里画了而 `.pid` 没画的仪表、注释、管嘴、机械设备与容器，现在不发**。同样情形的管件是照库发了的。

## 1. 口径

数据库的 `T_REPRESENTATION` 给每个对象在每张图上的每个图形记一行；发布件把它写成一个 `PIDRepresentation`，UID 就是那一行的 `SP_ID`。所以不用参考也能问：某张图上库里的行，发布件写出了哪些。

- 只数画在图上的行（`INSTOCKPILE = 1`；dump 自己的 73 号代码表：1 = `False`，2 = `True`）。这 53 张图的 stockpile 里一条都没有。
- 行按两样分类：表示类型（dump 自己的 39 号代码表：9 `Connector`、12 `Label`、13 `Symbol`、39 `OPC`、46 `Branch`、200 `Gap`），与这一行背后那个对象的类型（`T_MODELITEM.ITEMTYPENAME`）。
- 有同版参考的图，把参考的 `_Data.xml` 用同样的读法再数一遍，看原厂对同一批行写了哪些。
- 只对表示这一层、只按 UID。属性值、关系、对象本身都没有对。

入库的探针：`examples/probe_which_database_representations_a_publish_leaves_out.rs`，读 dump 用的是它自己的读法，不经 `publish::oracle_exp`。写它之前先用另一份脚本（不入库）数过一遍，数字相同。

## 2. 读数（已核实）

```text
53 drawings in the dump, 53 documents in the publish directory
drawings the database draws nothing on: 2 (D03, D04)
representations the database draws on the published drawings: 42048; in a drawing's stockpile, not counted: 0
the publish states 23762 of them, leaves out 18286, and states 0 the database does not draw

type         item type           publish: stated / left out   reference, 7 drawing(s): same      the class is
Branch       PipeRun                               2582 / 0   584 / 0                            stated everywhere
Branch       SignalRun                              175 / 0   52 / 0                             stated everywhere
Connector    PipeRun                              10487 / 0   2325 / 0                           stated everywhere
Connector    SignalRun                             1270 / 0   323 / 0                            stated everywhere
Gap          PipeRun                                  3 / 0   0 / 0                              stated everywhere
Label        (no model item)                      0 / 12584   0 / 2736                           stated nowhere
OPC          OPC                                    476 / 0   81 / 0                             stated everywhere
Symbol       EquipmentOther                         225 / 0   70 / 0                             stated everywhere
Symbol       Exchanger                               12 / 0   3 / 0                              stated everywhere
Symbol       Instrument                            3705 / 7   888 / 0                            LEFT OUT ON SOME ROWS
Symbol       ItemNote                               793 / 7   167 / 0                            LEFT OUT ON SOME ROWS
Symbol       Mechanical                              91 / 3   15 / 0                             LEFT OUT ON SOME ROWS
Symbol       Nozzle                                 372 / 5   45 / 0                             LEFT OUT ON SOME ROWS
Symbol       PipeRun                               0 / 4678   0 / 1013                           stated nowhere
Symbol       PipingComp                            3503 / 0   739 / 0                            stated everywhere
Symbol       SignalRun                             0 / 1001   0 / 259                            stated nowhere
Symbol       Vessel                                  68 / 1   9 / 0                              LEFT OUT ON SOME ROWS

rows left out of a class the publish states elsewhere: 23
```

读法：

1. **`D03`、`D04` 在库里没有任何表示。** 整包发布把这两张图写成只有 `PIDDrawing` 的壳（`_Index.json` 的 `shells: 2`，stderr 的 `2 without a DrawingNo`），与库一致：库里这两张图上什么都没有。
2. **三类谁都不发**：标签、管段上的符号、信号线上的符号，共 18 263 条。7 张参考图上这三类有 4 008 条，原厂一条都没写，我方也没写。所以这三类不算缺。
3. **其余 14 类在 7 张参考图上两边都写全了**（每一类的每一行）。这是完整度那条「对象 100 %」在表示这一层的另一种说法，只是这回分母取自库而不是参考。
4. **没有一条写出的表示是库里没有的**（53 张图合计 0）。
5. **23 条没写出，而它们的类在别处都写出**：C井场 21 条，Q井场 2 条。

## 3. 那 23 条

| 图 | 仪表 | 注释 | 管嘴 | 机械设备 | 容器 | 合计 |
|---|---:|---:|---:|---:|---:|---:|
| C井场 | 5 | 7 | 5 | 3 | 1 | 21 |
| Q井场 | 2 | 0 | 0 | 0 | 0 | 2 |

逐条的对象 UID、表示 UID 与 `GraphicOID` 在探针输出里（交付目录 `publish-v4\vs-database\`）。

已核实的：

- **库里它们与写出的行没有区别。** 23 条都是 `INSTOCKPILE = 1`、带 `GraphicOID`；表示与对象的 `ITEMSTATUS` 都是 1，`REPRESENTATIONCLASS`、`MODELITEMTYPE`、`SP_ISTYPICAL` 的取值都落在写出的 23 762 条的取值之内。库里没有任何一列说它们被删了。
- **包里的 `.pid` 没有这些图元。** 把 C井场 与 Q井场 的 `.pid` 从 `PlantData~2~711.zip` 里取出来，`pid_inspect --geometry-json` 给出的全部图形（C井场 3 450 个不同的 `graphic_oid`，Q井场 3 395 个）里没有这 23 个 `GraphicOID`。发布从 `.pid` 认对象，所以读不到它们。
- **同样情形的管件是发了的。** `ExpTables::dump_only_piping_components` 把「库里在本图上有表示、`.pid` 没画」的管件照库发出，连端口与表示：C井场 27 个、J K井场 3 个。依据是 J K井场 的参考里那 3 个都在（3 / 3）。上表里 `Symbol · PipingComp` 因此是 3 503 / 0。
- **7 张有同版参考的图上没有这种情形**（管件以外的类 0 条），所以没有一份参考能说明原厂怎么处理它们。

推断：原厂的发布读的是库。J K 那 3 个管件说明「库里画着、`.pid` 没画」的对象它照发；按同一个道理，这 23 个它多半也发。C井场 同时有 27 个管件和 21 个别的对象是这种情形，像是这张图在库里的状态比包里的 `.pid` 多出一块，但从时间戳看不出来（`T_DRAWING` 没有文件修改时间，`T_REPRESENTATION.SP_FILELASTMODIFIEDTIME` 记的是符号文件的时间）。

## 4. 为什么这一轮不改

管件那条规则有 3 个样本。别的类一个样本都没有，而且各有各的连带内容：管嘴要挂到设备上（组成关系），注释的正文在 `T_NOTE.BODY`，仪表有位号与信号端口，设备有名字与描述。照管件的写法外推，写出来的东西没有任何参考可对。按这条线一直的做法（规则先有样本、再进代码），这一轮只把它量出来、记下来。

要定下来需要的数据：**C井场 的一份原厂发布件**（对现在这个库发布）。它同时能验证那 27 个外推的管件（`docs/plans/2026-11-10-publish-export-completion.md` Q6 ②，现在多了一条理由）。Q井场 的也行，但它只有 2 个仪表。

在那之前，这 23 条由探针看着：规则若改，`rows left out of a class the publish states elsewhere` 应当从 23 降到 0，而「states 0 the database does not draw」不能变。

## 5. 复现

```powershell
$wt = 'D:\work\plant-code\cad\pid-parse-wt\publish-export'
$env:CARGO_TARGET_DIR = "$wt\target"
rustup run stable cargo run --locked --example probe_which_database_representations_a_publish_leaves_out -- `
  D:\work\cad\pid-test-data\Export.dmp D:\work\cad\pid-test-data\publish-v4 `
  --reference D:\work\cad\pid-test-data\验收数据
```

`publish-v4` 是 `pid_publish_xml D:\work\cad\pid-test-data --out-dir …` 的输出；对任何一次整包发布的输出目录都可以跑。
