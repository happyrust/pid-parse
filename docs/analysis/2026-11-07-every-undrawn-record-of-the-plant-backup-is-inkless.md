# 53 张备份图里没出图的记录：除 EF井场 一颗实心点外都不带墨（2026-11-07）

> 承接共享文档 `findings/2026-11-07-unreferenced-sheets-audit.md` 的「未解码 PSM type code 普查」一节（基线），在 HEAD
> `398dda4` 上重做，并把它的几处推断换成字节或参考图证据。
> 探针：`examples/probe_which_records_reach_no_page.rs`（普查）、
> `pid-plot/examples/probe_does_the_reference_print_a_retired_record.rs`（`0x8000` 记录对 11 张参考图）。
> 棘轮：`tests/plant_backup_census.rs`。修复：`27015d9`（圆成员边界）、`fa483f5`（普查走记录链）。

## 一句话

53 张图、103 个图内缓存体、5 种实际穿戴的模板里，**带墨迹却没出图的只有一条**：`EF井场` `/Sheet6` oid 52844，
一个填 `igCircle2d` 的 `igBoundary2d`（`0x73` 圆成员），已补解码，页面上多了一颗 r = 0.463 mm 的实心点。其余每个
没出图的类型码都有不出图的证据：关联记录（成员另画）、约束/关系/图层/视图等非图形类、文件自己关掉的图层、原生读取器
跳过的 `0x8000` 旧副本（11 张参考图上 237 条逐条核对，参考图一条都没画），以及**根本不是记录**的普查幻影。
`.sym` 库读取器另有三处缺口，但 53 张图 9225 个符号放置、模板 8 个放置全部取图内缓存体，库体一个没用上，对这 53 页不生效。

## 1. 普查口径

| 位置 | 是什么 | 谁读 | 量 |
|---|---|---|---|
| sheet | 图纸的 `Sheet*` 流（含嵌套 `JSite*/Sheet*`） | 族注册表 `SHEET_RECORD_FAMILIES` | 57 条流 / 140260 条记录 |
| cache | 图纸的 `/JSite*/PSMcluster0` 符号缓存体 | `streams/jsite.rs` 的 11 个嵌套解码器 | 103 个 / 66370 条 |
| igr | `RefData~4~682.zip` 里 10 个 `.igr` 的 `Sheet*` 流 | 同 sheet（模板按图纸解析） | 30 条 / 1244 条 |
| igr-cache | 模板的 `/JSite*/PSMcluster0` | 同 cache | 5 个 / 430 条 |
| sym | `RefData~4~681.zip` 里 644 个 `.sym` | `symbol_library::read_symbol_geometry_from_bytes` | 1187 条流 / 18512 条 |

记录一律按流自己的严格链（`sheet_record_starts`，从 8 字节流头后起、必须恰好走完）认；sheet / igr 的三类普查
（无解码器 / 拒收 / `0x8000` 跳过）与解析器存下的 `SheetGeometry::{undecoded_type_codes, refused_records,
skipped_records}` 逐流比对，修复后 87 条流全部一致。缓存体的严格链 103/103 能走完（走不完的话整个缓存体都会丢，这一项是 0）。

放置去向：53 张图 9225 个 `igSymbol2d` 放置**全部**解析到图内缓存体（库体 0、缺失 0、无路径 0）；压缩包里 10 个模板
的 8 个放置也全是缓存体。所以 `.sym` 库读取器跳过什么，对这 53 页都不出图；它的缺口单列在 §6。

## 2. 普查总表

类名来自 `tools/psm_type_clsid.py`（`radsrvitem.dll` 的类型码→CLSID 表，SHA-256 `C47C0DBE…2139`，与
`sppid-dlls\bin\radsrvitem.dll` 逐字节相同；名字取自 `D:\pid\RADInstallA~\jutil.dll`，`CAD8AFF3…BCBE3`）。
「图形」列 = 原生图形谓词 `radsrvitem.dll!sub_56449950` 是否接受该码（`docs/analysis/2026-08-04-annotation-families-risk.md` §2）。

### 2.1 图纸 Sheet 流（53 张，修复后）

| 码 | 类名 | 图形 | 去向 | 条数 / 图数 | 原因 / 证据 |
|---|---|---|---|---|---|
| `0x0013` | Boundary2d | 是 | 解码出图 | 1451 / — | 1449 条 `0x0010` 箭头填充 + 1 条模板语法 + **EF 那条 `0x73` 圆（修复前拒收）** |
| `0x0021` | ComplexString | 是 | 无解码器 | 1 / 1（D04） | 关联记录：11 个成员里 6 个是同流的 `igArc2d`×4 / `igLine2d`×2，各自已出图；另 5 个是 `0x0019` |
| `0x0019` | KeyPoint Constraint | 否 | 无解码器 | 5 / 1（D04） | 上面 ComplexString 的 `0x8000` 标记成员：相邻曲线端点的重合约束（`+18`/`+22` 两个成员 oid） |
| `0x00FA` | Dependency Object | 是 | 拒收 | 1 / 1（A井场） | 36 字节、零成员；该族 55413 条解码记录本来就一条都不出图（审计族） |
| `0x00FA` | Dependency Object | 是 | 解码不出图 | 55413 / 51 | 依赖关系两端的 oid，审计族，无几何 |
| `0x003D` | SmartFrame2d | 是 | 解码不出图 | 51 / 50 | OLE 图框；它链接的 `.igr` 作为模板单独出图 |
| 多码 | — | — | `0x8000` 跳过 | 1549 / 24 | 见 §4：`0x004D`×640、`0x00FA`×370、`0x005E`×163、`0x0084`×157、`0x00CE`×98、`0x003D`×74、`0x0018`×46、`0x0059`×1 |
| `0x2C90` `0x3D00` `0x0E00` `0x0004` `0x0071` `0x0100` `0x050A` `0x1000` `0x3A00` `0x3A3F` | — | 否 | 修复前「无解码器」 | 21 处 / 13 张 | **不是记录**，见 §3；轻烃分离器一的「`0x0020` 拒收」也是其中之一 |

图纸 Sheet 流里没有 `0x0020` / `0x0115` / `0x0063` / `0x007E` / `0x007B` / `0x00FF` / `0x005D` 记录。

### 2.2 图内缓存体（103 个）

| 码 | 类名 | 图形 | 去向 | 条数 / 图数 | 原因 / 证据 |
|---|---|---|---|---|---|
| `0x0115` | JDim（igDimension） | 是 | 拒收 | 40 / 33 | 尺寸种类 4（33 条，`Dimension` 层，值 7.62 mm）和 3（7 条：O/P/Q 井场各 2 条在 `Construction` 层，低压气入口 1 条在 `Dimension` 层）；解码器只认种类 1。**所在图层在缓存体自己的视图过滤集里全部关闭**——SmartPlant 不显示 |
| `0x0115` | JDim | 是 | 解码不出图 | 208 / 43 | 驱动尺寸是约束不是笔画，且同样在关闭的 `Dimension` 层 |
| `0x005E` | Point | 是 | 无嵌套解码器 | 6533 / 51 | 每条点的 `+14` 样式都解析不出线端符号（6387 在显示层、146 在关闭层）——`igPoint2d` 只有终端符号字形能出墨，这里一个都没有 |
| `0x0018` / `0x004D` / `0x0059` / `0x0063` | Line / Text / Circle / Ellipse | 是 | 解码、图层关闭 | 2134 / 1028 / 337 / 4 | 缓存体自己的视图过滤集把这些层（Label、HiddenObjects、Heat Trace…）设为不显示，页面按文件的意思不画 |
| `0x0081` `0x0089` `0x0006` `0x0019` `0x0042` `0x0086` `0x0114` `0x0057` `0x00ED` `0x00BD` `0x0085` `0x0082` `0x00C7` `0x006F` `0x00EA` `0x0077` `0x0002` `0x0004` `0x0036` `0x0037` `0x0075` `0x0076` `0x0079` `0x0088` `0x0060` `0x006A` `0x0058` `0x006E` `0x0015` `0x0017` `0x006B` | JSheetLayer、FreeFormAttrSet、OnElement / KeyPoint / Vertical / Horizontal / Fix / Equal / AlignVertical / Perpendicular / Tangent / Colinear Constraint、JSheetLayerManager、JSheets、JSheet、Top ViewFilterSet、JFlavorHolder、JSymbolInformation、Double Value、Standard Relation、Variables、Persist Preference、OLE sites manager、JViewSet、BackDrop、SheetView、OLE assoc connect、JSheetLayerGroup、JDimGroup | 否 | 无嵌套解码器 | 合计 43796 | 图层/视图/约束/表达式/OLE 基础设施，原生图形谓词不收；`0x0075` 不在 RAD 注册表里，也不在谓词里 |

缓存体里没有「解码了却挂不到任何本体图层」的记录（0 条），也没有 `0x0020` / `0x0013` 拒收（174 条 `0x0013` 全部按
`0x73`×159 / `0x67`×15 成员解码成填充）。

### 2.3 模板（`RefData~4~682.zip`）

53 张图实际穿戴 7 种：CPE-SQ-A0/A1/A2、CPECCHB-SQ-A1/A2，以及在读备份自身压缩包时 `A井场` 的 `A0-W - New` 与
`稳定塔(600td)` 的 `A1-W - New`（`export_plant_backup.rs::TEMPLATE_PLACEMENTS`）。

| 码 | 位置 | 条数 / 文件 | 原因 / 证据 |
|---|---|---|---|
| `0x0021` ComplexString | CCHB-SQ-A1/A2 Sheet6 | 7 / 2 | 关联记录：成员是同流 `igLine2d`（各自出图）+ `0x0019` KeyPoint 约束 |
| `0x0021` ComplexString | CPE-SQ-A0/A1/A2 `/JSite16` 缓存 | 3 / 3 | 关联记录：成员 2 条 `igArc2d`（`比例` 格那道 5 mm 波纹，已出图）+ 1 条 `0x0019` |
| `0x0019` KeyPoint Constraint | CCHB-SQ Sheet6 / CPE-SQ 缓存 | 18 + 3 | 上面两类 ComplexString 的约束成员 |
| `0x0020` Rectangle | A0/A1/A2/A3-W Sheet6 | 8 / 4 | 审计族：8/8 的四条边都是同流活的 `igLine2d`、端点都落在矩形四角（1 nm），边线已各自出图 |
| `0x007B` Group | A0/A1/A2/A3-W Sheet6 | 4 / 4 | 每条把同一模板的两只矩形（外框 + 内框）编成一组，没有自己的几何 |
| `0x0018` Line（`0x8000`） | A0/A1/A2/A3-W Sheet6 | 16 / 4 | 原生跳过，见 §4 |
| `0x003D` SmartFrame2d | A0/A1/A2-W | 3 / 3 | 图框审计族 |
| `0x0006` / `0x0085` | A1/A2/A3-W | 3 + 3 | OnElement / Vertical 约束 |

模板 Sheet 流的 `igBoundary2d` 70 条全部按模板语法解码（`0x68` 点列 12、`0x67`/`0x74` 混合 58）。

### 2.4 其他流

图纸根 `/PSMcluster0` 里没有图形码。`/StyleCluster` 里有 `0x0018`×418、`0x007B`×222、`0x0059`×13：这是点符号的
字形（`JStylePointSymbol → Group → 成员`），只经线端符号 / 复杂线型那条样式链出图。13 个圆全是 13 张图里
`Data Link` 点符号的字形，没有任何线样式、覆盖样式或复杂线型引用它（引用它的 `0x0034` 部件无人引用），页面上
也就没有该用它的笔画。

## 3. 无类名码：普查自己读出来的幻影

修复前的普查（`undecoded_census::unclaimed_counts`）从**字节 0** 起贪心滑动，接受「终点也像记录头」的候选。
Sheet 流的前 8 字节是流头（魔数 `0x6C90F544` + 记录数），于是：

- `+2`：魔数高半 `90 6C` 读成类型字 `0x6C90`（14 位码 `0x2C90`、`0x4000` 位），记录数读成长度；C井场的记录数 5676
  让它恰好落到 `+5684` 的真记录头上，于是一条「`0x2C90`」吞掉了 `+8..+5684` 的真记录。8 张图都是这个形状。
- `+7`：记录数的高字节 `00` + 首条记录类型字的低字节 `3D`（图框）读成 `0x3D00`（4 张），P井场首条是 `0xCE`
  （符号）读成 `0xCE00`（`0x0E00` + `0x8000` 位，记进了「跳过」）。
- 失步后在真记录**内部**接受巧合的头：`0x0004`（轻烃I +67518，在 `+67498` 的 `0x00FA` 里）、`0x0071`（轻烃I +1976）、
  `0x0100`（G H1 H2 +35812）、`0x050A`（J井场收球 +951）、`0x1000`（L M +9226）、`0x3A00`×2 / `0x3A3F`（`0x00FA` 成员表里的
  `00 FA` 字节对），以及轻烃分离器一 +1653 的「`0x0020` 拒收」（声明长度 65536，在 `+1625` 的 `0x00FA` 里）。

这 21 处没有一处是严格链记录，严格链在这 13 条流上都能从头走到尾。被幻影吞掉的字节里有 7324 条真记录：
7301 条已被解码器认领（照常出图），另 23 条带 `0x8000`（`0x003D`×8、`0x004D`×6、`0x00CE`×4、`0x00FA`×4、
`0x0084`×1）从没进过跳过普查；无解码器 / 拒收的真缺口被藏住的为 0。基线说它们是「空闲区或旧版本残片」，
不对——它们不是记录。`fa483f5` 让普查先走严格链、走不通才退回贪心法，读数见 §5。

## 4. `0x8000` 跳过：旧副本，也有已删除对象，参考图都不画

`PSMSerializeIn` 读到类型字 `0x8000` 置位就 seek 过整条记录、连 oid 都不读
（`docs/analysis/2026-09-28-the-native-reader-skips-a-flagged-record.md`）。53 张图 Sheet 流 1549 条：

| 码 | 图层 | 同流有同 oid 活记录（在后） | 没有同 oid 活记录 |
|---|---|---:|---:|
| `0x004D` Text | Labels（显示） / HiddenObjects（关闭） | 509 / 93 | 31 / 7 |
| `0x00FA` Dependency | — | 74 | 296 |
| `0x005E` Point | ConsistencyChecks / Default | 0 | 149 / 14 |
| `0x0084` LineString | Default / HiddenObjects / Labels | 31 | 104 / 15 / 7 |
| `0x00CE` JSymbol | Default / `0` | 60 / 1 | 37 |
| `0x003D` SmartFrame2d | DrawingBorder / Notes | 46 / 28 | 0 |
| `0x0018` Line | Labels | 0 | 46 |
| `0x0059` Circle | Labels | 0 | 1 |

另有模板 A0/A1/A2/A3-W 的 16 条 `0x0018`（`Default` 层），同样没有同 oid 活记录。基线「每一条都是同 oid 活记录的旧副本」
在五图 fixture 上成立，在这里只对 842 条成立；707 条找不到活记录——是被删掉的对象。
文件本身因此说明不了它们「已退役」，改问参考图：`probe_does_the_reference_print_a_retired_record` 把 11 张参考图对应的
`.pid` 里 237 条带位记录逐条当作活记录解码（副本里清掉该位），按同流活线算出的偏移放上页、经比对拟合映射到参考页，
每 0.25 mm 取样，问参考图和我方各自有没有墨（0.3 mm 内）：

| 码 | 结论 | 条数 | 长度 |
|---|---|---:|---:|
| `0x0018` | 两边都不画 | 34 | 40.83 mm |
| `0x0059` | 两边都不画 | 1 | 8.77 mm |
| `0x0084` | 两边都不画 | 12 | 266.33 mm |
| `0x0084` | 压在我方画的活笔画下 | 36 | 1446.90 mm |
| `0x0084` | 部分有墨，参考图与我方比例完全相同（0.32/0.32、0.79/0.79、0.68/0.68×2）——有墨的那段是活记录 | 4 | 475.60 mm |
| `0x0084` | 短于 0.2 mm | 7 | 0.07 mm |
| `0x004D` | 两边都不写（同字串、3 mm 内） | 117 | — |
| `0x004D` | 空字串 | 4 | — |
| `0x00CE` | 同点有同本体的活放置 | 8 | — |
| `0x00CE` | 无活孪生；插入点 6 mm 内参考图的墨全部被我方覆盖（5 处参考图 6 mm 内一笔都没有） | 14 | — |

「参考图画了、我方没画」= 0 条。带位记录不出图与 SmartPlant 一致。

## 5. 两处修复与前后读数

| 修复 | 前 | 后 |
|---|---|---|
| `27015d9` 图纸侧 `igBoundary2d` 接受 `0x73` 圆成员 | 53 张图 `igBoundary2d` 拒收 1；EF井场 (851.37, 131.12) mm 只有红圈 | 拒收 0；同处 26 点填充环（r = 0.463 mm，样式 113 → `Normal` 填充，与 1449 条箭头同路） |
| `fa483f5` 普查先走严格链 | 无解码器 `0x2C90`×8、`0x3D00`×4、`0x0004`、`0x0071`、`0x0100`、`0x050A`、`0x1000`、`0x0019`×5、`0x0021`；拒收 `0x0020`、`0x00FA`；跳过 1526 + `0x0E00`/`0x3A00`×2/`0x3A3F` | 无解码器 `0x0019`×5、`0x0021`×1；拒收 `0x00FA`×1；跳过 1549；87/87 条流的存档普查与严格链一致 |

EF 那条记录：子类型 `0x0000`，成员组 `0x73 + (圆心 x, 圆心 y, 半径, 0, 1) + 方向字节`（42 字节，与模板语法 58 字节的
`0x74` 弧同构、少两个角度），其后是图纸侧语法的锚点（= 圆心）、标志 1、成员数 1、成员引用（oid 52772、类字
`0x00D1`、子字 5）——52772 是同流同圆心同半径的 `igCircle2d`。模板语法不认 `0x73`、严格语法只认 `0x0010` + `0x67`，
两头都拒。现在 `0x0000` 先走模板语法、拒收再落到图纸侧语法，后者的成员组认 `0x67` 线与 `0x73` 圆，要求尾部恰好收尾
（全是线成员时就是原来的 `49 + 41n`）。test-file 语料的 golden 快照、parse_real_files、render_gap_census 读数不变。
EF井场没有参考图；判定它出墨的依据是同族同写法：1449 条图纸侧边界都经覆盖样式指向 `JStyleSimpleFill` 并在参考图上
印成实心，缓存体里 159 条 `0x73` 圆成员边界（法兰实心点）同样印成实心。

## 6. `.sym` 库读取器的缺口（对 53 页不生效）

库体只在图纸缺缓存体时才用；53 张图一次都没用上。以下按「若改用库体会怎样」记录，供以后修：

1. **会画原生跳过的记录**：`read_sheet_chain` 用 `raw_type & 0x0FFF` 取码、不看 `0x8000`。280 条带位的
   `0x0018`×102（23 个文件）、`0x004D`×169（93 个）、`0x0059`×4、`0x0084`×4、`0x0013`×1 会被当活记录画出：146 条与同 oid
   活记录内容不同（如 `Drawing Revision.sym` 的旧文字），133 条根本没有活记录（如 `Fuel Gas Heater.sym` `/Sheet190` 的
   四条线），1 条与活记录逐字节相同。改法：跳过 `raw_type & 0x8000`，与 `parse_live_psm_header` 一致。
2. **不读根 `/PSMcluster0`**：只开 `Sheet*` 流，而 PSM 把部分图元放在根簇里——`0x0018`×157（36 个）、`0x0061`×22（6 个，
   如 `Revision Cloud`、`Parametric Manifold`、`1to1Parametric H Drum`）、`0x004D`×10、`0x007E`×6（4 个，含
   `2to1Parametric V Drum`）、`0x0059`×3、`0x0063`×1。`Wellhead Control Panel.sym` 矩形的四条边就在根簇里。
3. **`igBoundary2d` 的 `0x68` 点列 / `0x74` 弧成员读不了**：17 条 / 11 个（`Self-Operated Safety Block Valve`、
   `Foam Monitor Elevated`、`Angle Check Valve`、各 `Remarks` 云线框等），填充会丢；图纸侧模板语法已经认这两种成员。
4. `0x0020` 矩形 11 条 / 10 个不读：8 条的四条边全是同 sheet 的活 `igLine2d`（库读取器会画），两个 `Title Block Label`
   各是 3 条活边加一个空槽，`Wellhead Control Panel` 那条的边在根簇（见 2）；另有 8 条宽或高宽比为负，图纸侧解码器也会拒。
   `0x0115` 240 + 90（带位）条在 `Dimension` 层；`0x005E` 2648 条点：2632 条样式无线端符号，16 条样式解析不到。

## 7. 未做 / 开口

- 测试语料 `工艺管道及仪表流程-1` 的缓存体里有 8 条 `0x0067`/`0x0074` 混合成员的 `igBoundary2d` 被缓存解码器拒（
  `decode_igsym_boundary_payload` 不认 `0x74`），不在 53 张图里；与 §6.3 同源。
- `0x00FA` 那条零成员 DependencyObject 不出墨，没去放宽它的解码器。
- `0x0075`（CLSID `C5EA66F0-11DF-11CE-8B57-080036181E02`）不在 RAD 注册表里，类名未解；它不在原生图形谓词里。
- AGENTS.md 里「`0x0013 igBoundary2d` … deliberately emits no `PidGraphicKind`」一段早于填充发射器，已过时，本次未改。

## 8. 复现

```powershell
$env:CARGO_TARGET_DIR='<独立目录>'
$env:PID_TEST_DATA_DIR='D:\work\plant-code\cad\pid-test-data'
$env:PID_PLOT_FONTS='D:\work\cad\PID自研软件验证\fonts'
cargo run --example probe_which_records_reach_no_page                       # 53 张备份图 + 旁边的 RefData~*.zip
cargo run --example probe_which_records_reach_no_page -- "D:\work\cad\pid-test-data\验收数据"
cargo run -p pid-plot --example probe_does_the_reference_print_a_retired_record
cargo test --test plant_backup_census
python tools/psm_type_clsid.py 0x02 0x04 0x19 0x36 0x42 0x57 0x75 0x81 0x86 0x88 0xBD 0xC7 0xEA 0xED
```
