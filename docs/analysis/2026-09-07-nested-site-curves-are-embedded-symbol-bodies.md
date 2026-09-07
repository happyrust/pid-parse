# 嵌套 `JSite` 里的圆和弧，是图纸随身带的符号定义副本（2026-09-07）

> 承接 `2026-08-31-jsite-geometry-coverage-gap.md`（"LdcSite 到页面的变换未证明，不 emit"）
> 与 `2026-08-31-imagdex-geometry-doio-ida.md`（四族曲线的流布局）。
> 本文回答前者留下的问题："这些几何以什么变换投影到页面"——答案是**它们根本不是页面内容**。
> 复现：`cargo run --quiet --example probe_nested_site_curves_are_symbol_bodies`。

## 一句话

四张主图嵌套 `JSite<N>/PSMcluster0` 里解出的 24 条圆 / 弧，**20 条与同一张图所放置的某个
符号的 `.sym` 本体里的圆 / 弧逐字节相同**（圆心、半径、起止角都在 1 nm 内一致）；
每个对上号的符号，其本体里**全部**圆 / 弧都在该存储里找得到（2/2、1/1……）。
嵌套 `LdcSite` 存储是图纸内嵌的**符号定义缓存**（多个符号的本体合放在一个存储里，各占一个
`JSheetLayer`），坐标是符号本地坐标——所以 08-31 那个"页面变换"从一开始就不该存在于
`LdcSite` 记录里：把它们放到页面上的，是每个 `igSymbol2d` 放置记录自己带的插入点和 2×2 矩阵。

## 证据

| 图 | 嵌套存储 | 曲线数 | 对上的 `.sym` | 未对上 |
|---|---|---:|---|---|
| D06 | `/JSite145` | 4 | Ball Valve Type 1（2/2）、2 Way Ball Type 1（1/1）、PT-Pressure Transmitter（2/2） | 0 |
| 0201 | `/JSite329` | 10 | LG-Magnetic Float Gauge（2/2）、LT-Magnetostrictive Level Gauge（2/2）、Ball Valve Type 2（1/1）、Parametric Manifold（2/2）、DCS Func Access in Prime Loc（2/2）、Cap（1/1） | 0 |
| 0201 | `/JSite396` | 2 | — | 2 条弧 r 35.59 mm、y = 88.9 mm 相对 |
| 0202 | `/JSite793` | 6 | DCS Field Mounted（1/1）、Cap2（1/1）、DCS Func Access in Prime Loc（2/2）、ElecTraceLine（2/2，放置 6 次） | 0 |
| 工艺 | `/JSite7559` | 2 | — | 2 个同样的圆 c=(−6.3, 0) mm r 3.81 mm |
| 合计 | | 24 | **20** | 4 |

- 匹配判据是**原始值逐个相等**（`SAME_MM = 1e-9`），没有任何变换、缩放或容差放宽；
  `.sym` 本体由 `symbol_library.rs` 用同一套 `0x0059` / `0x0061` 布局读出，单位同为米。
- 匹配是**双向完整**的：对上的每个符号，本体里的圆 / 弧数 = 存储里复现的数。
  `ElecTraceLine` 那两条弧（`2.944→0.197`、`6.086→3.339`，同半径 1.62 mm）与
  `symbol_library.rs` 文档注释里描述的库本体一字不差。
- 多个符号共用一个存储：`/JSite329` 一个存储里躺着六个不同符号的本体。这解释了 08-27
  aux_hi 名册里"同一存储的圆各在不同图层"——每个定义各占一层。
- 这些存储的 `JProperties` **不带** `.sym` 路径（表里 `.sym = -`），`igSymbol2d.jsite_ref`
  也不指向它们（08-31 已测）——所以按"被放置引用 + 带 .sym 路径"两个标记去找符号本体，
  它们才一直被判成"不是符号本体"。两个标记都描述的是**实例**站点，不是**定义缓存**。

### 未对上的 4 条

- `0201 /JSite396`：两条 r = 35.59 mm 的半圆在同一 y 上相对——与 `/JSite329` 里
  Parametric Manifold 的两条 r = 20.32 mm 半圆是同一个形状、不同尺寸。**Parametric**
  Manifold 是参数化符号（08-27 已解出 `JSymbolInformation` 变量 → `0x006F` 公式 →
  `JDim` 的驱动链），缓存里存的很可能是**按实例参数重算过的本体**，库里的默认尺寸自然对不上。
  这一条若坐实，意味着缓存副本比 `.sym` 库**更忠实**（库只有默认参数）。待证。
- `工艺 /JSite7559`：该图 7 种放置符号里 3 种（`Xa chu.sym` / `Xa.sym` /
  `Xa Item Note & Label.sym`）不在 `symbols-full` 库里，无从比对；两个同样的圆很可能属于
  其中之一。

没有一条与假设**矛盾**。

## 这对两仓意味着什么

1. **W4 的"页面变换"问题换了形状。** 不再是"`LdcSite` 的 12 字节 payload 里藏着矩阵"
   （08-31 已证没有），而是"**哪个 `igSymbol2d` 放置用的是缓存里的哪个定义**"——定义 ↔
   实例的链接边。候选：缓存存储里按定义分的 `JSheetLayer`；spacemap tag-181/182 的引用边；
   实例站点 `JProperties` 里的某个 id。这条边一旦解出，缓存几何走 `igSymbol2d` 的现有放置
   变换即可上页面，**不需要新的变换解码**。
2. **屏幕上其实不缺这些圆和弧。** OCS 已经用外部 `.sym` 库画符号本体
   （`src/io/pid.rs::place_primitive`），这 20 条在屏幕上早就有了。缓存副本的价值在别处：
   - **没有符号库时**的兜底：现在库缺失退化成 1.5 mm 占位圆点 + 名字标签；缓存里有完整本体。
   - **参数化符号**：若 `/JSite396` 那条坐实，缓存里是按实例参数重算的本体，比库的默认参数
     更接近 SmartPlant 屏幕。
   - **库里没有的站点自定义符号**（工艺图的 `Xa*`）：只有缓存里有。
3. 08-31 "0 repeated segments" 那条证据没有错，但量的是**错的对照组**：它拿嵌套存储与顶层
   `Sheet*` 的线段比，而符号本体从来不在 `Sheet*` 里（它们通过 `igSymbol2d` 放置）。
   正确的对照组是 `.sym` 库——本文就是。

## 还没做的

- 线段族（`igLine2d` / `igLineString2d`）没比：本文只比了曲线。按同样办法比线段，
  能把"缓存 = 完整本体副本"从"曲线全对"推到"全部图元全对"。
- 定义 ↔ 实例的链接边（上文第 1 条）。
- `/JSite396` 的参数化解释需要拿实例的 `JSymbolInformation` 变量值算一遍半径来坐实。
- Rectangle / BspCurve 仍无解码器；按本文结论它们也应是符号本体的一部分（3 + 1 条），
  可在 `.sym` 库里找同形状印证。
