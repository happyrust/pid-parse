# 放置记录的尾巴点名了它用的缓存定义：`(JSheet, LdcSite)`（2026-09-07）

> 承接同日 `2026-09-07-nested-site-curves-are-embedded-symbol-bodies.md`
> （嵌套 `LdcSite` 是内嵌的符号定义缓存）。那篇留下的问题是"哪个放置用缓存里的哪个本体"。
> 本文回答：**`igSymbol2d` payload 的最后两个 `u32` 就是 `(定义所在 JSheet 的 oid, 缓存
> LdcSite 的 id)`**，四张主图 107/107 个放置全部解析到一个真实本体，曲线与 `.sym` 库逐值一致。
> 复现：`cargo run --quiet --example probe_which_cache_body_a_placement_uses`；
> 棘轮：`every_placement_names_a_body_the_drawing_carries`。

## 一句话

一条 `igSymbol2d` 从矩阵六个 f64 之后的尾巴是
`f64 1.0 | u32 flags | u32 has_membassy | u32 0 | [u32 membassy_oid, u32 0] | u32 JSheet | u32 LdcSite`
——`has_membassy` 为 1 时多出中间那对（payload 121 / 123 字节），为 0 时没有（113 / 115）；
**最后 8 字节永远是 `(JSheet oid, LdcSite id)`**。到那个 `JSite<LdcSite>/PSMcluster0` 里，
该 `JSheet` 的 spacemap 条目带一个 **tag-183 成员 = 它的 `JSheetLayerManager`**，管理器管的
图层上的记录就是这个放置的符号本体（符号本地坐标）；放置自带的 2×2 矩阵 + 插入点就是上页面的变换。
**08-31 那道"LdcSite 到页面的变换"从来不在 `LdcSite` 记录里，因为它本来就在放置记录里。**

## 证据

### 1. 尾巴布局（四图 107 条放置，无一例外）

| payload 长度 | tag 位置 | `t+12`（has_membassy）| 尾部内容 |
|---:|---:|---:|---|
| 113 | 33 | 0 | `t+20 = JSheet`，`t+24 = LdcSite` |
| 115 | 35 | 0 | 同上（子头多 2 字节，整体后移）|
| 121 | 33 | 1 | `t+20 = membassy oid`（根存储 `0x0003` 记录，其 `+4` 反指本放置）、`t+24 = 0`、`t+28 = JSheet`、`t+32 = LdcSite` |
| 123 | 35 | 1 | 同上 |

`t+0..t+8` 恒为 `f64 1.0`；`t+8` 是 `0x01005001` / `03` / `00` 一类标志字，未解。

### 2. 每个 JSheet 只对一个管理器（缓存内 spacemap）

四图 6 个缓存存储的每个 `JSheet` 条目都恰有一个 tag-183 成员指向一个 `0x0042 JSheetLayerManager`
（基底 sheet 6 → 管理器 7；其余各对各的）；管理器再由 `SheetLayer::manager_oid`（08-31 W1 已对账）
带出它管的图层。`link_embedded_definitions`（`streams/psm_tables.rs`）就按这条边分组，两个管理器
以上一律拒收（语料 0 例）。

### 3. 对上 `.sym` 库的地面真值

放置 → 本体 → 圆 / 弧半径，与库本体逐值一致（`every_placement_names_a_body_the_drawing_carries`）：

| 图 | 放置的符号 | 本体位置 | 圆 / 弧半径（mm）|
|---|---|---|---|
| D06 | PT-Pressure Transmitter | `/JSite145` sheet 312 | 6.35、7.57 |
| D06 | Ball Valve Type 1 | `/JSite145` sheet 125 | 1.27 |
| D06 | 2 Way Ball Type 1 | `/JSite145` sheet 220 | 1.59 |
| D06 | Cone Roof Parametric Tank | **`/JSite151`** sheet 47（Imagineer Document）| — |
| 0201 | LG-Magnetic Float Gauge | `/JSite329` sheet 208 | 6.35、7.57 |
| 0201 | Ball Valve Type 2 | `/JSite329` sheet 362 | 1.27 |
| 0201 | **Parametric Manifold** | **`/JSite396`** sheet 113（Imagineer Document）| **35.59、35.59**（库默认 20.32）|
| 0202 | ElecTraceLine（放置 6 次）| `/JSite793` sheet 27852 | 1.62、1.62 |
| 0202 | DCS Field Mounted | `/JSite793` sheet 1968 | 6.35 |

统计（放置数 / 解析到本体 / 点名的不同本体 / 缓存里的本体数）：D06 6/6/6/9、0201 20/20/17/21、
0202 23/23/11/12、工艺 58/58/7/10。"点名的不同本体"= 该图放置的不同符号数，无一漏；"缓存里的本体数"
多出的部分是各存储的基底 sheet（空）和**没被任何放置点名**的定义（如 `/JSite329` 的 sheet 49：
库默认尺寸的 Parametric Manifold 模板，实例用的是 `/JSite396` 里重算过的那份）。

### 4. 两个缓存各司其职

`PSMroots` 给两个 `LdcSite` 起的名字说明了分工：**`Server Document`** 存静态定义（放置指向它的
占绝大多数）；**`Imagineer Document`**（Imagineer Technical 即 SmartSketch 的旧名）存**参数化
实例**——Cone Roof Parametric Tank、Parametric Manifold、Parametric Black Box、` Line2` 全指向它。
08-31 那两条 r = 35.59 mm "对不上库"的弧，正是 Manifold 按实例参数重算后的本体。
工艺图 `/JSite7559` 那两个"对不上库"的圆（r 3.81 mm，圆心 (−6.3, 0)）则各属 `Xa.sym`
（sheet 155）与 `Xa chu.sym`（sheet 219）一只——库里没有这两个站点自定义符号，缓存里有。

## 一处副产品：库读取器会多画一张 sheet

D06 的 Ball Valve Type 1 放置：缓存本体 12 条记录（9 线 + 1 圆 r 1.27 + 1 文字 + 1 伴热线 + 2 夹套线，
恰等于 `Ball Valve Type 1.sym` 的 `/Sheet6` 全部内容）；而 `.sym` 文件还有一张 `/Sheet245`
（6 线 + 1 圆 r 1.59 + 1 文字）。`symbol_library.rs` 把一个 `.sym` 的**所有** `Sheet*` 合成一个本体，
于是库路径给这只球阀多画了一圈同心 r 1.59 的圆和六条线；OCS 用库时 D06 的 `PID-SYMBOL` 是 46 个实体，
用缓存是 38 个，差的 8 个正是这张 sheet。**缓存里的是 SmartPlant 实际放置的那一个 flavor**
（`0x00EC JFlavorManager` 一个符号一个，它就是 `JSite<jsite_ref>` 本人）。哪条路更该上屏幕，
要拿 SmartPlant 截图裁；本轮 OCS 只在**库缺失**时用缓存，不改优先级。

## 落地

- `SheetIgSymbol2dDecoded` / `DecodedIgSymbol2dRecord` 新增 `definition_sheet_ref` /
  `definition_site_ref`（从 payload 末尾读）。
- `JSiteNestedGeometry` 扩成完整本体：`circles / arcs / lines / polylines / texts / sheets /
  definitions`，全部走记录链门；`link_embedded_definitions` 用 spacemap tag-183 把 sheet 归到管理器。
- `PidGraphicKind::SymbolInstance.definition: Option<PidSymbolDefinitionRef>`，
  `NormalizedPidGeometry::symbol_definitions` + `symbol_definition(ref)`，本体用 `.sym` 读取器
  同一套 `SymbolPrimitive` 词汇。
- OCS `src/io/pid.rs::build_entities`：库里没有本体时用缓存本体走同一个 `shape_primitive` 放置，
  `apply_symbology` 照旧按放置样式着色；`report_import` 记一行 info。D06 无库打开：6 个占位圆点
  → 38 个真实笔画。

## 还没做的

- 优先级：缓存 vs 库（见上）。需要一张 SmartPlant 截图对 Ball Valve Type 1 裁决。
- 缓存存储自己的 `StyleCluster`（如 `/JSite145/StyleCluster` 29 条）未接：缓存本体现在不带
  自身笔画样式，只靠放置样式重涂；放置样式解析不到的那几条会落 `ByLayer`。
- `t+8` 标志字与 membassy 的语义；`Site LdcSite Relation`（`0x004F`）两条各指向什么。
- Rectangle / BspCurve 仍未解码，按本文它们也是某个本体的一部分。
