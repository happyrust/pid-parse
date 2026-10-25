# 整包 53 张图：566 个被滤镜砍半的本体，62 个读 0.0，一个读 3.9 mm（2026-10-24）

> 承接 `2026-10-24-local-vent-prints-one-arc.md` 的第六节与第七节：那一篇在
> `I井场` 一张图上把 15 个「滤镜保留数不到一半」的本体逐条对过参考图，结论是
> 「0 个砍错」；但它的证据只有 5 张有参考出图的图，其余 48 张只能靠「几何全同」外推。
> 本文把同一套量法扩到整包 `backup-plant-data` 的 53 张：**所有**被滤镜砍半的本体都列出来，
> 有参考 PDF 的图才算覆盖率，按缺失墨量排序。
>
> 结论先写：53 张、1806 处放置、**566 个本体被滤镜砍半**。其中落在 5 张有参考图上的
> **63 行全部被计分，62 行缺失墨量 0.0 mm**；唯一非零的是 `换热器CD` 那张上的圆盘符号
> **3.9 mm**，而它的成因不是「被砍掉的那圈该印」——参考图沿那圈只有 0.08–0.26 的墨，
> 那 3.9 mm 是别的笔画从环的走廊里穿过来的。

## 一、口径

`SymbolGeometry::displayed_primitives` 若保留数 `keep * 2 < 图元数`，这个本体算「砍半」。
整包里砍得如此彻底的有两类：仪表气泡（`5 → 1`，丢的是 `Heat Trace` 环与 `Label` 层）
和 `Remarks`（`15 → 3`，丢的是 `Sheet37` 的编辑框）。判据不按「丢了几个」量，只按几何
占比，所以既不挑符号家族，也不看图层名。

缺失墨量沿用上一篇的 `unexplained_ink`，一处改动：走廊从「笔画包围盒」收紧为
**笔画自身 0.6 mm 的管**。包围盒把环内那片区域也算进来，而圆盘符号的标签就印在环里面，
量 `Heat Trace` 环时会把标签的邻墨错算成环的墨。收紧到管之后，只有**沿这条笔画会印出来
的那条带**里的参考墨才计分。

一行的读数定义：

```text
missing = Σ(该本体在图上每一处放置) Σ(参考墨落在该笔画 0.6 mm 管内
                                     且 0.5 mm 内没有任何页面笔画 的长度)
```

三条配套读数保证它可判读：

- 参考图侧只取 `plot.segments`。`read_plot` 把参考出图拆成 `segments` 与 `texts` 两支，
  字形笔画不进 `segments`，所以这个量里**没有字形墨**，不需要字形模型就能对齐；
- 每张有参考图的 sheet 另算一个**整页分母**：全部参考墨里，没有任何页面笔画盖住的那部分
  长度。分母不为 0 才说明这个量是活的；
- 每行还记 `lib` = 这一本体有多少处放置**在图里没有缓存体**。滤镜只有在缓存体缺席时才是
  最后一道话事人。

## 二、整包读数

```text
53 sheets under D:\work\plant-code\cad\pid-test-data\backup-plant-data

1806 placed bodies, 566 halved by their filter; 5 of 53 sheets have a reference plot
```

排在最前的一行（表按 `missing` 降序，其余 565 行见 `--package` 输出）：

```text
sheet                                                    body                                                     str keep inst lib   missing
集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图    Single Function/Disc Single-Func Field Mounted.sym     4    1   11   0     3.9 mm
I井场 注采阀组工艺及自控流程图                               Labels - Equipment/Remarks Chinese_large.sym            15    3    1   0     0.0 mm
I井场 注采阀组工艺及自控流程图                               Labels - General Instrument/Remarks.sym                 15    3   10   0     0.0 mm
I井场 注采阀组工艺及自控流程图                               2 Way Common/Ball Valve Type 1.sym                      15    7    8   0     0.0 mm
I井场 注采阀组工艺及自控流程图                               Misc/Local Vent.sym                                      8    1    1   0     0.0 mm
...
集注站 低压气压缩机入口改造工艺及自控流程图                     Graphics/Leader with Bend & Term.sym                    27   10    1   0         -
```

按 sheet 分：`I井场` 15 行、`低温分离器A` 13 行、`一级闪蒸分离器A` 12 行、
`一级闪蒸分离器B` 12 行、`换热器CD` 11 行，共 63 行，全部计分；其余 48 张的 503 行
只列本体、不给墨量。

566 行按「图元数 → 保留数」归并，只有九个形状：

| 本体 | 图元 → 保留 | 行数 | 砍掉的是什么 |
|---|---:|---:|---|
| 仪表气泡（`PG`/`PT`/`TG`/`TT`/`LG`/`LT-DP`/`PDT`） | 5 → 1 | 260 | `Heat Trace` 的外环 + `Label` 层文字 |
| `Labels - */Remarks*.sym` | 15 → 3 | 129 | `Sheet37` 的 7×7 mm 圆角框 + 云线 |
| `Disc Single-Func Field Mounted.sym` | 4 → 1 | 45 | 同一圈外环 + 标签 |
| `Globe Valve Type 1.sym` | 13 → 6 | 44 | 另一张 sheet 上的同形副本 |
| `Flange Type 2.sym` | 5 → 2 | 39 | 同上 |
| `Ball Valve Type 1.sym` | 15 → 7 | 36 | 同上 |
| `Local Vent.sym` | 8 → 1 | 10 | 5 条伴热带 + `Sheet71` 的立管/圆盖 |
| `Leader with Bend & Term.sym` | 27 → 10 | 1 | 引线的几段弯折 |
| `Undefined Interlock Logic 02.sym` / `Disc Multi-Func Inaccess …` | 10 → 4 / 3 → 1 | 1 / 1 | 逻辑框的内框 / 外环与标签 |

这九个形状在上一篇的 `I井场` 表里都出现过，只是那时只有一张图，看不出它们占了整包的
多少：**仪表气泡与 `Remarks` 两家就占了 389/566（69%）**。

## 三、为什么 62 行是 0.0：滤镜从来不是最后一道话事人

`lib` 列在全表 566 行里**无一例外是 0**——整包每一处放置，图里都带着自己的定义缓存体。
页面画的是缓存体，不是库本体；库的滤镜只决定「拿哪张表」，而缓存体已经把那句话抄进了
自己的图层表（`2026-10-24-local-vent-prints-one-arc.md` 第三节：`I井场` 15 个砍半本体，
缓存显示数与滤镜保留数不等的 0 个）。

以 `PG-Pressure Gauge.sym` 在 `I井场` 上的一处为例，五行读数是：

```text
PG-Pressure Gauge.sym: 5 primitives, filter keeps 1
  drawing's cache: 4 primitives, 1 displayed
    [0] displayed circle    ink 1.00  tight 1.00  len   39.9 mm   ← r 6.35 气泡
    [1] hidden    circle    ink 0.02  tight 0.00  len   47.6 mm   ← Heat Trace 环
  library body:
    [0] sheet=6 layer=Default   drawn   circle  ink 1.00
    [3] sheet=6 layer=Heat Trace dropped circle  ink 0.02  page 0.01  left 0.0 mm  = cache[1] hidden
```

被丢的那圈 47.6 mm 外环：参考图沿它只有 **0.02** 的墨，页面也只盖 0.01，缺失 **0.0 mm**。
这不是「滤镜砍对了」的修辞——参考图在那儿一笔都没有，画上就是凭空多出 47.6 mm×260 处。

## 四、唯一非零的 3.9 mm：环的走廊里有别人的墨

`Disc Single-Func Field Mounted.sym` 在 `换热器CD` 上有 11 处放置，合计 3.9 mm，分布在
三处（`--package` 的明细给出了每一处的 `ref ink` 与 `page`）：

```text
Single Function/Disc Single-Func Field Mounted.sym  (1 of 4 kept, 11 placements, 0 library-only)
    circle       3.9 mm
        at (0.697,0.257) m  left 1.8 mm  ref ink 0.08  page 0.02
        at (0.699,0.301) m  left 1.6 mm  ref ink 0.15  page 0.02
        at (0.682,0.390) m  left 0.6 mm  ref ink 0.26  page 0.09
```

`ref ink 0.08 / 0.15 / 0.26` 说明参考图**没有**印那圈外环（印了应在 0.9 以上）；那 3.9 mm
是别的笔画从 0.6 mm 管里穿过去留下的段落。把这三处的管拆开逐条印出来：

```text
disc at (697.0,257.0)
   0.60 mm left of a  6.60 mm stroke (697.48,244.18)->(697.48,250.78)
   1.60 mm left of a  3.59 mm stroke (698.67,247.40)->(696.13,249.94)
   1.19 mm left of a  2.03 mm stroke (697.48,263.48)->(697.48,265.51)
disc at (682.0,390.0)
   3.70 mm left of a 36.41 mm stroke (675.13,419.10)->(675.13,382.69)
```

一根竖向短线（6.6 mm）、一根 45° 斜刺（3.59 mm）、一根本体上方的引线（2.03 mm）、
一条 36.4 mm 竖线的一段。它们各自都只是「页面在这几处以 0.5 mm 之外错过了参考墨」，
与滤镜丢没丢那圈环无关——那圈环本来就不该印。

## 五、分母：5 张有参考图的图，整页只漏 45.8 mm

```text
sheet                                              ref ink    unexpl.   share
I井场 注采阀组工艺及自控流程图                        34849 mm       7.1   0.02%
集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图      9637 mm       2.8   0.03%
集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图     11803 mm       9.9   0.08%
集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图   10149 mm      13.8   0.14%
集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图 14364 mm   12.2   0.09%
```

五张合计 80,802 mm 参考墨，未被页面解释的 45.8 mm（0.057%）。被滤镜砍半的本体在这
45.8 mm 里占 **3.9 mm（8.5%）**，且这 3.9 mm 已证明是过路墨。换句话说：**滤镜这条路上
没有剩下可回收的墨**；整页残余漏墨在别处（弧的折线化、虚线相位、文字之外的零星线段），
不在本体筛选。

## 六、限制

- **48 张图没有参考出图**，503 行只列本体、不计墨。这 48 张上的结论仍然只有第三节那条
  结构证据（缓存体在场、`lib = 0`），不由参考图直接验证。
- **文字与填充不计分**：只量线、弧、圆、多段线、B 样条。`Label` 层被丢的字是这一族的
  大头（仪表气泡、`Remarks` 都是），它们在页面上由缓存体的文字图元负责，本探针看不见。
- **管半径 0.6 mm**：参考图相对页面有 0.2–0.3 mm 系统偏移，0.6 mm 足够容下；但一条被印在
  0.6 mm 之外的笔画不会被算进来，也不会被算成「该印没印」。
- 每张图只有一份参考 PDF，且一份 PDF 只对应一张图；整包没有第二份出图可交叉。

## 复现

```text
cargo run -p pid-plot --example probe_body_against_reference -- --package
cargo run -p pid-plot --example probe_body_against_reference -- \
    "D:/work/cad/PID自研软件验证/集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图.pdf" \
    "D:/work/plant-code/cad/pid-test-data/backup-plant-data/00/00/集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图.pid" \
    "Disc Single-Func Field Mounted.sym"
```

`--package` 模式在 `probe_body_against_reference` 里：它按 `PID_TEST_DATA_DIR` /
语料目录 / 两个 `pid-test-data` 找 `backup-plant-data`，逐张 `parse → build_normalized_geometry`
列放置，只对有参考图的图 `lay_out_drawing` 建页面网格。整包一轮约 2.5 分钟（debug 构建）。
