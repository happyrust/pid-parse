# `igArc2d` 从 `startAngle` 到 `endAngle` 是**顺时针**扫过的（2026-09-19）

> 承接 `2026-07-27-ugeom2d1-curve-readers-ida.md`（IDA：两个角是绝对起止角，不是「起始角 + 扫掠角」）
> 与 `2026-09-07-placement-tail-names-the-cached-definition.md`（缓存本体就是放置画的那个）。
> 那两篇定了字段是什么，没有定弧**往哪边走**——`symbol_library.rs` 的注释写成了「逆时针」，
> 消费方照着画。本文用本体自己的线段裁决：**顺时针**。反着读，语料里每一条能裁的弧都是它自己的补弧，
> 折回本体内部。棘轮：`a_cached_arc_sweeps_clockwise_from_its_start_angle_to_its_end_angle`。

## 一句话

`igArc2d`（PSM `0x0061`）payload `+42` / `+50` 的两个 f64 是标准的绝对角（弧度，从 +X 逆时针量），
两个端点都严丝合缝落在本体线段的角点上；但**弧本身是从 `startAngle` 顺时针走到 `endAngle`**。
按 DXF 的逆时针约定画，要把两个角对调。尾字节 `+58` 不是方向位：Manifold 两条弧为 0、Cap 与
DCS 盒为 1，走向全一样。

## 证据

语料的弧全在符号本体里（缓存 0201 7 条、0202 5 条，D06 / 工艺 / A01 无；图纸自己的记录里一条没有），
能由线段裁出朝向的有三处，三处一致：

| 本体 | 弧 | 逆时针读 | 顺时针读 | 裁决依据 |
|---|---|---|---|---|
| 0201 `Parametric Manifold` 实例（`/JSite396` sheet 113） | 左端帽 c=(0.04118, 0.08890) r=0.03559 `270° → 90°`；右端帽 c=(0.14221, …) `90° → 270°` | 顶点落在壳体矩形**内**（x = c ± r 朝内），外框宽 **101.03** mm | 顶点在壳体**外**，外框宽 **172.21** mm | 本体自带的 4 条 `Construction[OFF]` 线里有两条**从弧心画到顺时针顶点**（(0.04118, 0.0889) → (0.00559, 0.0889)、(0.14221, 0.0889) → (0.17780, 0.0889)）——参数化本体的轴线只可能指向端帽的顶点 |
| 0201 `Parametric Manifold` 模板（`/JSite329` sheet 49） | 同形，c.x = −0.03048 / 0.15748，r = 0.02032 | 内 | 外，宽 **228.6** mm = 库默认 `Left` + `Right` | 同上 |
| 库 `Remarks.sym`（工艺放置 35 次的那张云线） | 上下两条大弧：c=(0.10939, 0.13339) r=0.00214 `357° → 183°`、c=(0.10939, 0.14040) r=0.00251 `183° → 357°` | 顶点在矩形框**内** | 顶点在框**外** | 云线的凸弧朝外；其余六条小弧的端点不在框上，两种读法顶点都在框外，不裁 |

Cap / Cap2 的半圆（弦在 x = 0.0019）与 DCS 盒的两个半圆（0° → 180°、180° → 360°）没有可裁的线段：
前者的短横线在关闭层上、弦本身不分内外；后者两半无论朝向都拼成整圆。ElecTraceLine 的两条波弧
相切成 S 形，两种读法都是 S。

## 后果

- **`SymbolPrimitive::Arc` / `PidGraphicKind::Arc` / `SheetIgArc2dDecoded` / `DecodedIgArc2dRecord`
  的注释改为「顺时针」**；字段值不动（`startAngle` / `endAngle` 是 IDA 坐实的字面名，原样保留），
  换约定是消费方的事。
- OpenCADStudio 此前按逆时针画的每一条弧都是补弧：库画的 Manifold（228.6 × 40.64）两端帽是向内的
  凹口，Cap 的半圆朝反面。之前没人看出来，因为语料里带弧的放置只有这几处，且 Manifold 一直走库本体、
  没和任何一个逐数钉住的外框对过。计划 C2 把缓存本体画上屏幕、`extent=` 改量可见笔画之后才露出来——
  没有构造线补位，Manifold 的 `extent=` 会从 172.21 掉到 101.03。C2 在 `shape_primitive` 里对调两角
  （镜像放置同样对调），见该仓 `docs/plans/2026-09-19-draw-the-cached-body-first-and-the-library-only-when-the-drawing-carries-none.md`。
- 图纸自己的 `igArc2d`（`PidGraphicKind::Arc`）语料为零，按同一条记录、同一种读法处理，未经实测。

## 复现

`tests/parse_real_files.rs::a_cached_arc_sweeps_clockwise_from_its_start_angle_to_its_end_angle`：
两张 Manifold 本体每条弧的两个端点在壳体矩形上、顺时针中点在矩形外、逆时针中点在矩形内、有一条构造线
从弧心到顺时针中点，壳体加两端帽宽 0.17221 / 0.22860；`Remarks.sym` 端点在框上的两条弧同样内外分明
（库不在时跳过这一段）；各图缓存弧总数 7 / 5 / 0 / 0。
