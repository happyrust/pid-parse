# `Local Vent` 的可视体是一条弧，不是八条（2026-10-24）

> 承接 `2026-10-23-the-arc-trailing-byte-is-the-direction.md`：那篇用这个符号的顶盖弧
> 证明 `igArc2d` 尾字节是方向位（185.26° vs 174.74°）。验证方向时顺带看到一个
> 不一致——**页面上这个符号只画一条弧，而 `.sym` 的图形体有 8 个图元**——看起来像
> 「取体时选错了 sheet」。本文把这个问题查完：不是选错，那 8 个图元是**三份材料**
> （1 条打印 + 5 条伴热带 + 2 条另一套画法），三方读数（图内缓存体、库侧视图集、
> 参考图墨线）指向同一条弧。前一篇 `2026-10-22-the-normal-view-that-drew-nothing.md`
> 在打分表里记过一句「`Local Vent.sym`（1 对 2）缓存站正常表」，本文补上它当时没有的
> 第三方证据：**参考图只印那一条**。

## 一、图内缓存体：6 个图元，其中 5 个自带「不显示」

`I井场` 这一处放置引用的是 `JSite37311` 的 sheet 760。缓存体 6 个图元、5 个图层：

```text
  drawing's cache: 6 primitives, 1 displayed
    layer oid=763    name=Some("Default") displayed=Some(true)
    layer oid=764    name=Some("Heat Trace") displayed=Some(false)
    layer oid=765    name=Some("HiddenObjects") displayed=Some(false)
    layer oid=766    name=Some("Jacket") displayed=Some(false)
    layer oid=767    name=Some("Label") displayed=Some(false)
    [0] displayed arc       ink 1.00  len    8.2 mm
    [1] hidden    line      ink 0.00  len    1.3 mm
    [2] hidden    line      ink 0.00  len    2.5 mm
    [3] hidden    line      ink 0.00  len    1.3 mm
    [4] hidden    line      ink 0.00  len    2.5 mm
    [5] hidden    line      ink 0.10  len   10.2 mm
```

唯一显示的是那条顶盖弧：`c (0.00119, −0.00254) m`，`r 2.545 mm`，
`1.539039 → 4.772389`，逆时针。5 条线全在 `Heat Trace` 上；`HiddenObjects` / `Jacket` /
`Label` 三个图层一个图元都没有。缓存体里也没有下文 `Sheet71` 那一对。这 6 条与 `.sym`
的 `Sheet6` 体**逐字节相同**（见第二节的坐标）。

换句话说：`SmartPlant` 自己的定义缓存认为这个符号的可视内容就是**一条顶盖弧**。
`PidSymbolDefinition::sheet_layers` 的 `displayed: false` 不是我们猜的，是缓存体所在
storage 的 `0x0057 Top ViewFilterSet` 自己的回答。

## 二、`.sym` 的 8 个图元是三份材料

`Ref/Symbols/Piping/Specialty Components/Misc/Local Vent.sym`（10 个图层，**只有 1 个
视图集**）：

| 所在 sheet | 图层 | 图元 | 视图集怎么说 |
|---|---|---|---|
| `Sheet6` | `Default` (oid 8) | 1 条弧 | 位 0 亮 → 显示 |
| `Sheet6` | `Heat Trace` (oid 65) | 5 条线 | 位 7 **灭**，且图层名属隐藏族 |
| `Sheet71` | `Default` (oid 69) | 1 条线 + 1 条弧 | **没有视图集** |

```text
views (probe_sym_layer_filter):
  Sheet6: 2 bytes, 12 bits set [0,1,2,3,4,5,6,11,12,13,14,15], supplementary=false,
          listed=[(8,"Label"),(9,"HiddenObjects"),(7,"Heat Trace"),(0,"Default"),(10,"Jacket")]
  Sheet71 无视图集
layer table, 整理为 (oid, number, name):
  (8,0,Default) (57,9,HiddenObjects) (60,2,Heat Trace) (61,1,Label) (64,8,Label)
  (65,7,Heat Trace) (69,0,Default) (455,3,HiddenObjects) (1109,10,Jacket) (1179,4,Jacket)
```

- 前 6 个就是缓存体那 6 个（弧 + 5 条伴热带线），差别只在 `.sym` 里多留了一份视图集
  来表态（缓存体把同样的表态抄进了自己的图层表）。
- 真正「多出来」的是 `Sheet71` 的 2 个图元：一条 **6.35 mm 立管**
  （`(−3.51,1.91) → (−3.51,8.26) mm`）+ 一个 **6.36 mm 直径的圆盖**
  （`c (−0.33,8.28)`，`r 3.18 mm`，`3.147902 → 6.276876`，顺时针 = 3.129 rad ≈ 179.3°，
  朝上凸）。这是一根立管加蘑菇盖的**另一套画法**，不是同一个体的另一视图。
- 它没有 `0x0057`，所以「这张表显不显示」在本文件里**没有答案**——不是被关掉，是没表态。

## 三、库侧取体落到 `{6}`，与缓存一致

`SymbolGeometry::displayed_primitives` 的链条在这个体上是：

```
sheets_present = {6, 71}
normal         = {6}          // 只有 Sheet6 带视图集，且 supplementary=false
drawing        = {6}          // 它有笔画，所以不会被「空转不选」剔掉
selected       = {6}
Heat Trace 5 条 = 位灭 + 隐藏名，两条规则各自独立地否掉它
Sheet71 2 条   = sheet 不在 selected 里
=> 1 条
```

`probe_library_vs_cached_bodies` 在 `I井场` 上给出的是：这个符号**没有进 MISMATCH**
（两侧的图元种类计数全等、包围盒在 1 µm 内），只在末尾的 `stripped 1/8` 列表里出现
一行。也就是说「被滤镜砍掉 7/8」这件事，**缓存体是同意的**。

同图 15 个「保留数不到一半」的本体，逐条对过缓存：

| 读数 | 值 |
|---|---:|
| 被砍半的本体 | 15 |
| 缓存显示数与滤镜保留数**不等**的 | **0** |

这不是巧合能解释的：图内缓存体就是「滤镜之后的本体」，两边是同一份材料。

## 四、参考图：只印了那一条弧

参考图语料里的 `I井场….pid`、`验收数据/` 下的副本、`backup-plant-data/00/00/` 下的
副本，三者 md5 全为 `150e3142…`（3,997,696 字节），所以
`D:\work\cad\PID自研软件验证\I井场….pdf` 就是这张图纸的参考出图——整包模式下同一张
sheet 的读数也一样。

新探针 `pid-plot/examples/probe_body_against_reference` 把库本体的**全部** 8 个图元按
该放置自己的变换投到纸上，逐点量参考图墨线（0.2 mm 弦长采样，0.5 mm 内算命中）。
这个放置是 `rotation −π/2`、`scale [1, −1]`（**y 镜像**），所以采样在符号自己的空间里
做，每个点再过 `place_metres` —— 镜像会翻转扫掠方向，**只变换圆心和扫掠角是错的读数**。

```text
Local Vent.sym: 8 primitives, filter keeps 1
  placement at (0.76967,0.72425) m rotation -1.570796 rad scale [1.0, -1.0]
  drawing's cache: 6 primitives, 1 displayed
  library body:
    [0] sheet=6 layer=Some("Heat Trace") dropped line      ink 0.10  len   10.2 mm  page 0.00  = cache[5] hidden
    [1] sheet=6 layer=Some("Heat Trace") dropped line      ink 0.00  len    2.5 mm  page 0.00  = cache[4] hidden
    [2] sheet=6 layer=Some("Default") drawn   arc       ink 1.00  len    8.2 mm
    [3] sheet=6 layer=Some("Heat Trace") dropped line      ink 0.00  len    1.3 mm  page 0.00  = cache[3] hidden
    [4] sheet=6 layer=Some("Heat Trace") dropped line      ink 0.00  len    2.5 mm  page 0.00  = cache[2] hidden
    [5] sheet=6 layer=Some("Heat Trace") dropped line      ink 0.00  len    1.3 mm  page 0.00  = cache[1] hidden
    [6] sheet=71 layer=Some("Default") dropped line      ink 0.00  len    6.3 mm  page 0.00
    [7] sheet=71 layer=Some("Default") dropped arc       ink 0.00  len   10.0 mm  page 0.00
```

（`page` 列是这条笔画被缓存**已显示**笔画盖住的比例，见第六节。）
方法自身的对照点也在表里：该画的那条 1.00，其余 0.00–0.10。**参考图印出来的就是这一条弧。**

墨量账也算得清：5 条伴热带线共 17.78 mm，`Sheet71` 立管 6.35 mm + 圆盖 9.95 mm，
合计 **≈ 34 mm**；若把 8 个图元都画上，就是在这处凭空多出 34 mm 墨，而参考图在那儿
一笔都没有。

## 五、为什么不是「选错了表」

上一轮那篇把「库侧选表」的失败分成四类（空转、存根、本体在他表、正常表正确），本符号
属于第四类里最容易被误判的一支：**正常表上只有 1 条，另一张表有 2 条**，而缓存站正常表。

- 判据没有按「肥瘦」走，是因为「更肥就取肥的」那条实测更差（184 vs 189，见前一篇）；
- 这一例正是那条规则会弄坏的地方：按肥瘦取会拿到 `Sheet71` 的立管+圆盖，参考图显示
  **0.00** 覆盖；
- 包围盒比例（1.88）也在前一篇记过是对的。

本次普查的另一条读数：13 处 `Local Vent` 放置的弧参数（半径、两个角、方向）**逐项相同**，
说明它们都是同一体经纯平移落到页面上——所以「这一处的结论适用于其余 12 处」。这也
再次排除「某几处取了 `Sheet71`」的可能：取了的话，画出来的是 `r 3.18 mm`、朝上凸的
圆盖，而不是这条 `r 2.545 mm` 的顶盖。

## 六、同图另外 14 个被砍半的本体：没有一个是砍错的

`I井场` 上「滤镜保留数不到一半」的本体共 15 个（本篇第一节那个是其中之一）。不给符号
名跑同一个命令就能拿到全部，逐条与参考图对完：

| 读数 | 值 |
|---|---:|
| 本体（不同符号路径） | 15 |
| 缓存**显示**、被计分的笔画 | 35 条 / 358 mm，**最低墨覆盖 1.00** |
| 缓存**隐藏**、被计分的笔画 | 19 条 |
| 库本体被滤镜丢掉、被计分的笔画 | 76 条 |
| 其中落在参考墨上的（≥ 0.5） | **15 条** |

15 条「丢了但有墨」的笔画分三类，没有一类是漏印：

| 类别 | 条数 | 长度 | 判据 |
|---|---:|---:|---|
| 缓存里**已显示**的同形笔画的副本 | 9 | 39 mm | `= cache[k] displayed`，且 `page 1.00`：库本体在另一张表上留了第二份拷贝（球阀 `Sheet245` 与 `Sheet6`、截止阀 `Sheet6` 与 `Sheet83`、法兰 `Sheet6` 与 `Sheet30`）|
| 不是副本，但页面已经盖住 | 1 | 6.4 mm | `Globe Valve` 的一条 6.4 mm 线，`page 1.00` |
| `Remarks` 的编辑框 | 5 | 18.6 mm | 见下 |

反方向的读数同样干净：真不该印的笔画几乎没有。仪表类的伴热带圆圈（`LT-DP`
那圈 47.6 mm，r 7.57 mm，比气泡大 1.9 mm）墨覆盖 **0.06**；第一节那个 `Sheet71`
立管 + 蘑菇盖是 0.00。

`Remarks` 那 5 条是标注符号的**编辑框**（`Sheet37` 的 15 个图元：7×7 mm 圆角框 + 云线），
而缓存体与滤镜都只用 `Sheet6` 的 3 条线（0.0 / 0.6 / 1.3 mm 的锚点十字）：

```text
Remarks.sym placement (0.23604,0.28501) m
    [4] sheet=37 Default dropped arc   ink 0.35  len  6.5 mm  page 0.00
    [5] sheet=37 Default dropped arc   ink 0.29  len  8.1 mm  page 0.00
    [6] sheet=37 Default dropped line  ink 0.00  len  7.0 mm  page 0.00   ← 框的对边
    [7] sheet=37 Default dropped line  ink 1.00  len  7.0 mm  page 0.00   ← 一条边压在别的墨上
```

同一个框的对边是 0.00、两条邻边只有 0.29/0.35，同一段圆角弧在另外两处放置读数
0.29 / 0.55——框本身没有印，命中是它刚好压在管线与引线上的段落。

**结论：15 个里 0 个砍错。** 滤镜丢掉的笔画要么是同一形状的第二份拷贝（页面已画），
要么根本没被印。

## 七、仍未闭合

- **`Sheet71` 的身份没有定**：已知它不打印（参考图 0.00、缓存体不含它、无视图集），
  但不知道它记录的是什么状态——旧版本体？另一种表示？无视图集就没有文件自己的答案，
  语料里也没有第二处放置用到它。
- 13 处放置分布在 10 张图上，其中**只有 `I井场` 有参考出图**；其余 9 张（整包 53 张里
  48 张无参考 PDF）的结论只由「几何与这一处全同」推出，不由参考图直接验证。
- 每个符号只量一个放置：同一个符号在图上放几十次（`Ball Valve Type 1` 有 40+ 处），
  不同放置的命中度会不同（`Remarks` 那一段圆角弧在三处分别读 0.29 / 0.55 / 0.00）。
  这足够回答「这条笔画到底印不印」，但不足以回答「这一处贴得准不准」。
- 文字与填充不计分：「被丢掉的是不是一句该印的字」不在这个探针的能力里（只量线、弧、
  圆、多段线、B 样条）。`Remarks.sym`、仪表类本体的 `Label` 层文字都属于这一类。

## 复现

```text
cargo run --example probe_sym_layer_filter -- \
    "D:/work/cad/PID自研软件验证/Ref/Symbols/Piping/Specialty Components/Misc/Local Vent.sym"
cargo run -p pid-plot --example probe_body_against_reference -- \
    "D:/work/cad/PID自研软件验证/I井场 注采阀组工艺及自控流程图.pdf" \
    "D:/work/cad/PID自研软件验证/I井场 注采阀组工艺及自控流程图.pid" "Local Vent.sym"
cargo run -p pid-plot --example probe_library_vs_cached_bodies -- \
    "D:/work/cad/PID自研软件验证/I井场 注采阀组工艺及自控流程图.pid"
```

`probe_body_against_reference` 不给符号名时，只报「滤镜保留数不到一半」的本体，用来
找下一个值得量参考图的符号。
