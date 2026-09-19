# 参数化链在模板上闭合，放置的实例身上没有它（2026-09-18）

> 承接 `2026-08-27-the-recordless-182-referrers-are-symbolinformation.md`（`0x00BD` / `0x00C7` / `0x00EA` /
> `0x006F` 四族解开，Standard Relation 的出参 13/13 是 JDim）、
> `2026-09-07-placement-tail-names-the-cached-definition.md` §3–§4（`Parametric Manifold` 放置指向
> `/JSite396` sheet 113，两条弧 r = 35.59 mm 而库默认 20.32；`/JSite329` sheet 49 是没被点名的模板）、
> `2026-09-14-jdim-is-a-framed-record-whose-blocks-follow-the-dimension-kind.md` 与
> `2026-09-15-tag-188-members-land-in-jdim-reference-slots.md`（`0x0115` 解码器与 `+92` 被量几何）。
> 计划 `OpenCADStudio/docs/plans/2026-09-07-jdim-driving-dimensions-and-layer-panel.md` 的 J3 要求把这三段
> 接成一条链，验「模板 JDim = 20.32、实例 JDim = 35.59 且实例几何落在 JDim 两端点上」。
>
> probe：`examples/probe_parametric_chain_resolves_a_cached_body.rs`（四主图 + A01，逐存储、逐本体，不抽样）。
>
> **等级：corpus。** 没有新的反编译证据；本文做的是把已解开的四族记录与 `0x0115` 按 oid 接起来，
> 拿公式重算一遍和存下来的值比，再把模板本体与放置本体逐坐标比。结论进棘轮
> `the_parametric_chain_closes_on_the_template_not_on_the_placed_instance`，不改任何投影输出。

## 1. 一句话

**链闭合了，但只在模板上**：`SymbolInformation` 的变量 → `Double Value` → `Standard Relation` 公式 →
`JDim` 值，17/17 条关系逐条算得出存下来的尺寸值——**公式里的常数按英寸读**。而每一个**放置出来的参数化
实例**（`Imagineer Document` 存储里的本体）身上**没有一条 JDim、没有关系、没有 Double Value**，只有一份
变量值等于模板的 `SymbolInformation` 副本和算好的几何。计划写的「实例 JDim = 35.59」不存在：35.59 只在
那两条弧的半径里。

## 2. 语料：五对模板 / 实例

| 图 | 符号 | 模板（`Server Document`，无放置点名） | 实例（`Imagineer Document`，放置点名） | 配对依据 |
|---|---|---|---|---|
| D06 | Cone Roof Parametric Tank | `/JSite145` sheet 15：6 线、5 JDim | `/JSite151` sheet 47：6 线、0 JDim | `value_ref` {30,31,32,33} 解到模板存储 |
| 0201 | Parametric Manifold | `/JSite329` sheet 49：8 线、2 弧、3 JDim | `/JSite396` sheet 113：8 线、2 弧、0 JDim | `value_ref` {16,39,50} 解到模板存储 |
| 0201 | ` Line2` | `/JSite329` sheet 501：2 线、2 JDim | `/JSite396` sheet 119：2 线、0 JDim | `value_ref` {516} 解到模板存储 |
| 工艺 | Parametric Black Box | `/JSite7559` sheet 72：4 线、4 JDim | `/JSite6963` sheet 21：4 线、0 JDim | 变量名 + 值全同（{43,44,45,46} 全文件解不到） |
| A01 | Horizontal Drum | `/JSite39` sheet 96：9 线、4 JDim | `/JSite121` sheet 481：9 线、0 JDim | `value_ref` {92,93,94} 解到模板存储 |

语料 22 条 JDim（四主图 18 + A01 4）**全部**在左栏的五张模板 sheet 上，这五张 sheet **没有一张被任何放置点名**；
右栏五张被点名的 sheet 上一条 JDim 也没有。0202 没有参数化符号，两栏皆空。

模板与实例的配对靠实例那份 `SymbolInformation`：它的 `value_ref` 在自己的存储里**一个都解不到**，
在模板存储里 4/5 对全部解到（D06 {30,31,32,33}、0201 {16,39,50} 与 {516}、A01 {92,93,94}），
就是模板那份 `SymbolInformation` 点名的那几条 `Double Value`；工艺那对（{43,44,45,46}）在全文件解不到，
按变量名与值配上（四个 0.0127 全同）。五对里**变量名与值全部与模板一致**——实例副本记的是库默认。

**2026-09-19（计划 K1）：这张配对进了 DTO。** `PidSymbolDefinition::template`（只在实例上，指模板本体）、
`::variables`（模板与实例都有，实例那份就是库默认副本）、`PidSymbolDimension::name`（出参它的那条关系的入参所对应的
`SymbolInformation` 变量名；入参是别的 JDim 的取不到，`None`）与 `::formula`（关系原文）。配对分两步：先按上表
定模板**存储**（`value_ref` 全部解到、否则名 + 值全同且那个存储有关系），那些值驱动的关系写到哪张 sheet 就是模板
**本体**；实例存储里哪个本体归这份记录，文件没写下来——记录的 `parent_ref` 为 0，也不总写在本体之前（工艺的
记录 27 在 sheet 21 **之后**，D06 / 0201 / A01 的都在之前），所以按**模板本体的线数与弧数**在实例存储里挑，
挑不出唯一一个就不配（同一模板被多份记录认领时按 oid 序对位——语料里没有）。语料 5/5 配上；四主图 18 条 JDim
15 条有名，无名恰三条：0201 JDim 503（无关系，`formula` 也 `None`）、D06 JDim 19 与 A01 JDim 82（派生尺寸，
有 `formula` 无 `name`）。棘轮 `a_placed_parametric_body_names_its_template_and_the_template_names_its_dimensions`；
探针第 4 节改从这几个字段读。

## 3. 关系：17/17 闭合，常数是英寸

`Standard Relation` 的第一个操作数是出参、其余是入参，公式形如 `0E$1`、`0E$1+0.01`、`0E$1+0.1`、
`0E($1+$2)/10`、`0E$1/2`（`$n` = 第 n 个入参）。把入参值代进去算，与出参那条 JDim 存的 `+42` 比：

| 存储 | 关系 | 公式 | 入参 | 出参 JDim | 按米算 | 按英寸算 |
|---|---|---|---|---|---|---|
| D06 `/JSite145` | 44 / 45 | `0E$1+0.01` | Top / Bottom = 0.035306 m（1.39″） | 24 / 18 = 35.56 mm | 45.306 mm ✗ | 1.40″ = **35.56** ✓ |
| D06 | 46 / 47 | `0E$1+0.1` | Left / Right = 0.06096 m（2.4″） | 20 / 21 = 63.5 mm | 160.96 mm ✗ | 2.5″ = **63.5** ✓ |
| D06 | 43 | `0E($1+$2)/10` | JDim 21、JDim 20 | 19 = 12.7 mm | ✓ | ✓ |
| 0201 `/JSite329` | 19 / 20 / 21 | `0E$1` | Top / Left / Right | 36 = 20.32、38 / 40 = 114.3 | ✓ | ✓ |
| 0201 | 512 | `0E$1` | Right = 0.0254 | 499 = 25.4 | ✓ | ✓ |
| 工艺 `/JSite7559` | 86–89 | `0E$1` | Left / Right / Bottom / Top = 0.0127 | 109 / 67 / 94 / 108 = 12.7 | ✓ | ✓ |
| A01 `/JSite39` | 85 / 86 / 87 | `0E$1` | Top / Left / Right | 147 = 20.32、98 / 97 = 114.3 | ✓ | ✓ |
| A01 | 84 | `0E$1/2` | JDim 147 | 82 = 10.16 | ✓ | ✓ |

17 条**全部**在英寸下闭合；13 条无常数的公式与单位无关，D06 带常数的 4 条只在英寸下闭合——
**公式常数的单位是英寸**（符号库的作者单位），而值一律以米持久化。入参可以是另一条 JDim（D06 43、A01 84），
链是可以串的。

0201 sheet 501 的 JDim 503（3.81 mm = 0.15″）没有关系指向它，是语料里唯一没有公式的 JDim。

## 4. 模板上，JDim 的值就是本体的尺寸

- **0201 Manifold 模板（sheet 49）**：JDim 36（Top = 20.32）量的是轴线上那段 20.32 长的短线（line 72，
  x = 0.0635 = `SymbolInformation.extents.0`）；**两条端弧 59 / 61 的半径 = 20.32 = JDim 36**；
  JDim 38 / 40（Left / Right = 114.3）量的是两端那两段 20.32 长的水平短线（line 28 / 32），它们**从弧心出发**
  向外到本体最外端，而 114.3 = 轴线到最外端的距离（0.0635 − (−0.0508) = 0.1778 − 0.0635 = 0.1143）。
  本体外框 228.6 × 40.64 = (Left + Right) × 2·Top。
- **D06 Cone Roof Tank 模板（sheet 15）**：外框 127.0 × 83.82；半宽 63.5 = JDim 20 = JDim 21（量地板线 55 与右墙
  线 56，值 = 轴线 x = 0.1016 到墙的距离）；半高 35.56 = JDim 18 = JDim 24（地板 0.13589 到檐口 0.20701 的一半）；
  顶尖高 12.7 = JDim 19（檐口到 0.21971）= (63.5 + 63.5) / 10，正是关系 43 的公式。
- 工艺 Black Box 模板 25.4 × 25.4 = 2·12.7 两向；A01 Horizontal Drum 模板 228.6 × 40.64，与 Manifold 同一套变量。

所以 tag-188 那条边「JDim → 被约束图元」在模板上落到具体的 `igLine2d`：`+92` 的 oid 就是被量的线
（21/22，A01 计入；唯一的例外 D06 JDim 24 量的是 `Point` 63），`PidSymbolDimension::endpoints` 现在把它给出来。
计划里「实例几何落在 JDim 两端点上」那句不是几何关系的形状：弧**以被量短线的起点为圆心**、半径等于 Top 那条尺寸。

## 5. 实例：几何各不相同，参数都不在文件里

| 实例 | 模板外框 | 实例外框 | 与模板的关系 |
|---|---|---|---|
| 0201 ` Line2` sheet 119 | 25.4 × 3.81 | 25.4 × 3.81 | **逐坐标相同**（公式 `0E$1` 无常数） |
| D06 Cone Roof Tank sheet 47 | 127.0 × 83.82 | 122.12 × 82.844 | **同一套公式、常数按毫米读**：半宽 61.06 = 60.96 + **0.1 mm**，半高 35.316 = 35.306 + **0.01 mm**，顶尖 12.212 = 122.12 / 10——三个数精确到 1e-9 m |
| 0201 Manifold sheet 113 | 228.6 × 40.64，弧 r 20.32 | 172.209 × 71.18，弧 **r 35.590035** | 被拉过：Right 仍 114.3，Left′ = 57.909、Top′ = r′ = 35.590035；构造不变（短线 = r、轴线 x = 0.0635） |
| 工艺 Black Box sheet 21 | 25.4 × 25.4 | 126.627 × 90.767 | 被拉过 |
| A01 Horizontal Drum sheet 481 | 228.6 × 40.64 | 159.459 × 51.917 | 被拉过 |

两点结论：

1. **实例那份 `SymbolInformation` 不是实例的参数。** 五对里它的值全等于模板（= 库默认），而实例几何有四对不同；
   Manifold 的 35.59、Black Box 的 126.6、Drum 的 159.5 在文件的任何 `Double Value` / `JDim` 里都找不到。
   放置实例真正的参数存在哪里（放置记录？`_Data.xml` 的项属性？）本文没有找到，登记为开口。
2. **D06 那一对是同一条链跑了两遍、两种单位。** 模板的 JDim 值是公式按**英寸**算的（库作者单位），
   放置实例的本体是同一公式按**毫米**算的（图纸单位）——`+0.1` 一次是 2.54 mm、一次是 0.1 mm。
   这解释了 08-31 起「缓存本体对不上库」里的一类：不是拉过，是单位。

## 6. 对计划 J3 验收的结算

- 「模板 JDim = 20.32」✓（JDim 36 = Top，且 = 两弧半径）。「实例 JDim = 35.59」✗——**实例没有 JDim**，
  35.59 只在弧里，且是 35.590035，不是任何变量。
- 「实例几何（两条弧）落在 JDim 两端点上」✗——关系是**弧心在被量短线的起点、半径 = Top 尺寸**（模板上成立）。
- 「tag-188 边 = JDim → 被约束图元，按 oid 对上 `igLine2d`」✓（17/18 线、1 点；弧从不被量）。
- 棘轮按事实钉：`the_parametric_chain_closes_on_the_template_not_on_the_placed_instance`
  （17 关系全在英寸下闭合、D06 4 条只在英寸下闭合；带 JDim 的 sheet 皆未被放置；实例零 JDim 且变量等于模板；
  0201 模板弧 = Top、弧心在短线起点、实例弧 35.59 无尺寸对应、副本 Top 仍 20.32、` Line2` 两本体相同；
  D06 实例 = 公式按毫米算，三个数 1e-9）。计划原名 `a_parametric_instance_carries_its_own_dimension_values`
  说的是被否掉的那个前提，不用。
- 不改投影：本族不发实体，golden 不变。

## 7. 对 OCS 消费的含义（D2 的后半句）

D2 说「解码结果进 OCS 特性面板 / 导入摘要」。按本文：放置实例身上**没有**属于它的尺寸值，能挂到放置上的只有
模板（库默认）的 JDim——在面板上把 20.32 标给一个画着 35.59 弧的 Manifold 是误导。要给用户看，能给的是
（a）模板本体的驱动尺寸（明说是库默认），或（b）实例本体的实际外框。**不建议**把 `PidSymbolDefinition::dimensions`
当作放置实例的尺寸展示；导入摘要计数（「n 条驱动尺寸，全在模板上」）无害。

## 8. 还开着的

- 放置实例的实际参数在文件的哪一处：五个实例四个被拉过，值不在缓存里。
- `SymbolInformation.extents` 两个数：在 Manifold / Drum 上是 (0.0635, 0.0889) = 轴线 x 与弧心 y，
  在 Tank 上是 (0.1016, 0.17145) = 轴线 x 与本体中高——像是符号原点，未坐实。
- JDim 503（0.15″）没有公式；`+22` = 49 只出现在 0201 两条上，仍 raw。
- 那些非参数化符号的 `SymbolInformation`（`(0,0)`、无变量的十几条）是什么，本文没碰。

## 9. 复现

```powershell
cd pid-parse
cargo run --example probe_parametric_chain_resolves_a_cached_body
cargo test --test parse_real_files the_parametric_chain_closes
```

输出第 2 节「does the formula reproduce the output?」是 §3 那张表，第 3 节是 §4 的弧对尺寸，第 4 节是 §5 的模板对实例。
