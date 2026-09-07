# 矩形是四条边线的父记录，B 样条是叶子：`0x0020` / `0x005D` 的解码器（2026-09-07）

> 承接同日 `2026-09-07-placement-tail-names-the-cached-definition.md`（缓存本体已能经放置矩阵
> 上页面）留下的最后两族：`igRectangle2d`（`0x0020`）与 `igBspCurve2d`（`0x005D`）。
> 布局 08-31 已由 `imagdex.dex` 的 `DoIO` 反编译坐实
> （`2026-08-31-imagdex-geometry-doio-ida.md` §5 / §5bis），本文用语料里仅有的 3 + 1 条记录
> 逐字段对读，把「5 个 f64 的逐个语义」与「变长尾巴的含义」定下来，并据此决定两族**各走哪条路**。
> 复现：`cargo run --quiet --example probe_rectangle_and_bspline_bytes`；
> 棘轮：`rectangles_own_their_edges_and_the_bspline_reaches_its_body`。

## 一句话

**矩形不画，B 样条画。** 一条 `igRectangle2d` 的 5 个 f64 是
`(origin.x, origin.y, width, rotation, height / width)`，尾巴是 `u32 4` + 四个 `u32` oid——
**同一条流里四条 `igLine2d` 的 oid，四条线的端点恰是矩形的四个角**。边线自己已经解码、
已经 emit，矩形只是它们的父记录：再画一次就是把四条边画两遍，所以它有解码器、有字节入账、
有边表，但 `emits_geometry = false`。B 样条相反，是**叶子**：poles / 可选权重 / knots，
没有别的记录替它画，所以它 emit（de Boor 采样成折线），在 `.sym` 读取器与缓存本体里也各占一席。

## 证据

### 1. 语料里的四条记录（+ `.sym` 里的一条）

| 文件 | 流 | 族 | oid | parent | layer | payload |
|---|---|---|---:|---:|---:|---:|
| DWG-0202 | `/Sheet6615` | Rectangle | 1909 | 6615 | 6996 | 78 |
| A01 | `/JSite204/Sheet6` | Rectangle | 114 | 311 | 8 | 78 |
| A01 | `/JSite204/Sheet6` | Rectangle | 295 | 311 | 8 | 78 |
| DWG-0202 | `/JSite793/PSMcluster0` | BspCurve | 2797 | 2817 | 2824 | 194 |
| `arrester breather valve(RD).sym` | `/Sheet6` | BspCurve | 451 | 6 | 8 | 194 |

三条矩形都是 78 字节：18 字节子头 + 5×f64（40）+ `u32` 计数（4）+ 4×`u32`（16）。
B 样条 194 = 18 + `u32 N`（4）+ 5×16 + `u32 weight_flag`（4）+ `u32 M`（4）+ 9×8 + f64（8）+ 4 字节。
两族**没有空位**，每个字节都有名字——与 `igLine2d` 那条「全额入账」的纪律一致。

### 2. 矩形的 5 个 f64

08-31 的 IDA 读到 `u16 + u32 + 5×f64`：那个 `u16 + u32` 就是子头自己的 `+12 sub_type_word` /
`+14 index`（guide §5 此前把它们单列在 `+18`、把 f64 推到 `+24`，本文订正：**f64 从 `+18` 起**）。

| 记录 | `+18` | `+26` | `+34` | `+42` | `+50` | 读法 |
|---|---:|---:|---:|---:|---:|---|
| A01 114 | 0 | 0 | 0.594 | 0 | 0.707071 | 原点 (0, 0)，宽 594 mm，**594 × 0.707071 = 420.0 mm：A2 外框** |
| A01 295 | 0.025 | 0.010 | 0.559 | 0 | 0.715564 | 原点 (25, 10) mm，宽 559，**559 × 0.715564 = 400.0 mm：内框**——左装订边 25、其余三边 10 |
| 0202 1909 | 0.126271 | 0.092379 | 0.127478 | 0 | 0.406863 | 127.478 × 51.866 mm 的框（孤儿存储里的一块） |

第四个 f64 三条都是 0，所以「角」这一读法只靠原生 bbox 子程序用 `ffloor` / `fceil` 求四角这条
旁证，语料没有非零样本；第五个 f64 是**高宽比**而不是高——A2 的 `0.707071` 与 `0.715564 × 0.559 = 0.4`
两个整数毫米把它钉死。

### 3. 尾巴 = 四条边线的 oid

| 记录 | 计数 | 边 oid | 四条线在同一流里？ | 端点 = 四角？ |
|---|---:|---|---|---|
| 0202 1909 | 4 | 4732, 5238, 6099, 6530 | 是（`/Sheet6615` 的 `igLine2d`） | 是 |
| A01 114 | 4 | 369, 297, 294, 296 | 是 | 是——端点就是 A2 的四角 (0, 0) / (594, 420) mm |
| A01 295 | 4 | 181, 237, 303, 302 | 是 | 是 |

「每条边 oid 都是同流的 `igLine2d`、其两个端点都落在矩形四角（1 nm）」写进了棘轮。
这就是 08-31 IDA 说的「版本化的 SmartSketch 关系数据」：矩形与它四条边的约束关系，
不是绘制几何。**投影里没有任何实体带矩形的 oid**——四条边各自 emit，矩形一条不多。

顺带把 DWG-0202 `/Sheet6615` 那个孤儿存储说清了：它此前唯一的「missing decoder」丢弃就是这条
矩形；现在 `dropped_graphic_records` 为空，存储里解码出的线段恰是这 4 条边。A01
`/JSite204/Sheet6` 剩下的丢弃只有 1 条 `0x007B` Group implementation。

### 4. B 样条：五个控制点、九个结点、三次

```text
N = 5   poles = (5.041, 5.076) (5.379, 4.890) (6.078, 4.280) (5.379, 3.669) (5.041, 3.483)  mm
weight_flag = 0（多项式，非有理）
M = 9   knots = [0, 0, 0, 0, 0.5, 1, 1, 1, 1]      → degree = M − N − 1 = 3，两端 clamped，两节
f64 = −1.0；尾 4 字节 04 01 01 00
```

它坐在 `/JSite793/PSMcluster0`，`parent_ref = 2817` = 该缓存里 `arrester breather valve(RD)`
的定义 sheet（放置 oid 4508 的尾巴点名 `(2817, 793)`），`layer 2824` 归那张 sheet 的管理器管——
所以它经 `link_embedded_definitions` 进了该定义的本体，成为 `SymbolPrimitive::BSpline`。
`.sym` 库里同一符号的 `/Sheet6` 有同一条记录（oid 451），**五个控制点、九个结点逐值相同**——
到 2 ulp（一个 y 差 2e-18 m）：缓存是经过一次算术的再序列化副本，不是字节拷贝，所以棘轮用
1e-15 容差比而不是 `==`。

一条 2 mm 的弧形唇，`bspline::sample` 每节 8 段（两节 → 17 点）弦误差在 0.01 mm 以内。

## 决定

| 族 | 解码 | 投影 | 缓存本体 | `.sym` 读取器 |
|---|---|---|---|---|
| `igRectangle2d` | `decode_igrectangles`，注册为 `Decoded`，字节入账 | **不 emit**（`IgRectangle2dEmitter` no-op；四边已由 `igLine2d` 各自 emit） | 进 `JSiteNestedGeometry::rectangles` 作证据，不进 `primitives` | 不读（`.sym` 语料 0 条） |
| `igBspCurve2d` | `decode_igbspcurves` | `IgBspCurve2dEmitter` → `Polyline`（de Boor，每节 8 段） | `SymbolPrimitive::BSpline { poles, weights, knots }` | `decode_primitive` 新增 `0x005D` |

- 两族与其它五族走同一道记录链门（`sheet_record_starts`），族间不可能互读；边表与 poles / weights /
  knots 三个计数都要求**恰好**填满 payload，多一字节少一字节都拒收。
- 顶层 `Sheet*` 流里两族语料 0 条（矩形那 3 条在孤儿存储与 A01 的 OLE 站点里），golden 快照没有变化。
- OpenCADStudio `shape_primitive` 加 `BSpline` 一臂：在符号坐标系采样再逐点 `apply`，旋转 / 缩放 /
  镜像与折线同一条路；段数用 `pid_parse::bspline::SEGMENTS_PER_SPAN`，页面直画与经放置画出来的曲线一致。
  DWG-0202 上那只 arrester breather valve 的弧形唇，有库（`.sym` 读取器）与无库（缓存本体）两条路都画，
  17 个顶点逐点重合（`a_symbols_bspline_lip_reaches_the_drawing_from_either_body`）。

## 未解 / 不做

- 矩形 `rotation` 非零的样本为 0；`corners()` 按 08-31 bbox 子程序的读法实现了旋转，未经数据验证。
- B 样条尾巴的 `f64 −1.0` 与 `04 01 01 00` 四个 flag 语义未定（原生读取器只是逐个读进 `obj+48` /
  `obj+72..75`），解码器原样带出，不解释。
- `imagdex.dex` 里还有 Ellipse（`0x0063`）/ Elliptical Arc（`0x007E`）/ ComplexString（`0x0021`）
  三族有 DoIO，语料 0 条，不写。
