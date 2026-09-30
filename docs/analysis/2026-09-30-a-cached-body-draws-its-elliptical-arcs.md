# 缓存本体画出椭圆弧：`0x007E igEllipticalArc2d` 角在前、顺时针扫，投成精确有理二次 B 样条（2026-09-30）

> 承接 OpenCADStudio 小单 `docs/plans/2026-09-30-a-cached-body-draws-its-elliptical-arcs.md`（E-D1 – E-D6；本文是工作项 E1 的「分析一页」）与 `2026-09-19-igarc2d-sweeps-clockwise-from-start-to-end.md`（扫向怎么裁）。
> 起因：09-30 解析缺口盘点（OCS `docs/plans/2026-09-29-pid-import-next-round.md`「进度 · 2026-09-30 解析缺口盘点」）里唯一看得见的真缺口——A01 的设备 `V 010121A` 画成直角方箱，两端的 2:1 椭圆封头没画。
> 这四条记录 `2026-07-07-phase34e-missing-geometry-fixture-plan.md` 早就数到过，只是一直没有解码器，读缓存本体时被跳过。基线 pid-parse `4e75935`。

## 一句话

A01 缓存本体 `/JSite121`（设备放置 oid 184 画的那一份）里的两条 `0x007E igEllipticalArc2d`（oid 489 / 490）就是卧式容器的两端封头。
布局**不是** `igArc2d` 的：两个参数角放在最前（`+18` / `+26`），然后才是圆心、长半轴向量、短长轴比；从 `igArc2d` 借来的只有扫向——从起角**顺时针**扫到止角，这样读，语料的四条弧都向本体外鼓。
每条弧投成一条**精确的**有理二次 B 样条（`SymbolPrimitive::BSpline`）：`SymbolPrimitive` 不加变体、`PidSymbolDefinition` 不加字段，OCS 的 `src` 不用改。

## 普查

| 存储 | JSheet | oid | 层 | 放置引用 | 上图 |
|---|---:|---|---|---|---|
| `/JSite121/PSMcluster0` | 481 | 489 / 490 | 533 `Default`（显示） | oid 184 = 设备 `V 010121A` | 画 |
| `/JSite39/PSMcluster0` | 96 | 114 / 115 | 149 | 没有 | 不画 |

全语料只有 A01 有 `0x007E`，四条全在嵌套 `JSite*/PSMcluster0` 的定义缓存里；`Sheet*` 流一条没有（phase34e §2.1）。

## 布局

btf 75 = 18 字节子头 + 7 个 f64 + 1 个尾字节，长度单位米，是在这四条真记录上量出来的。对照 `igArc2d`（btf 59）：

| payload | `0x007E igEllipticalArc2d` | `0x0061 igArc2d` |
|---|---|---|
| `+0` – `+17` | 子头 oid / parent_ref / sheet_layer_ref / sub_type_word / index | 同 |
| `+18` / `+26` | **`sweep_start` / `sweep_end`**（参数角） | 圆心 x / y |
| `+34` / `+42` | 圆心 x / y | 半径 / `start_angle` |
| `+50` / `+58` | 长半轴向量 x / y | `end_angle` / 尾字节 |
| `+66` | 短长轴比 `ratio` | — |
| `+74` | 尾字节（意义未知，`flag` 留作审计） | — |

**与小单 E-D2 的出入。** E-D2 写「字段名与扫向照 `igArc2d` 的布局与约定（`+18` 起、`+26` 止、顺时针）」。括号里的偏移与小单事实表一致，是实测；「照 `igArc2d` 的布局」与实测不符：`igArc2d` 先存圆心、半径，两个角在 `+42` / `+50`，`0x007E` 把角放在最前。
字段名也没照搬：`igArc2d` 的是从 +X 量的绝对角 `start_angle` / `end_angle`，这里是从长半轴量的参数角 `sweep_start` / `sweep_end`。实现照实测，从 `igArc2d` 只借扫向约定；E-D2 的措辞由 OCS 的 E3 更正。

## 约定与证据

`P(t) = C + cos t · major + sin t · minor`，`minor = ratio · (−major.y, major.x)`——长半轴转 +90° 再乘比，t 从长半轴量向短半轴。弧从 `t = sweep_start` **顺时针**（t 递减）走到 `t = sweep_end`，
扫角 `Δ = (sweep_start − sweep_end) mod 2π`，两角差整数圈而不相等时算整圈；顺时针 s → e 与逆时针 e → s 是同一段弧，按 DXF 的逆时针约定画要对调两角。两条互相独立的证据：

1. **向外鼓。** 四条弧的长半轴都竖直朝 −y，短半轴于是朝 +x。489 从 2π 递减到 π，途经 `t = 3π/2` 即 `C − minor`，顶点在本体左边线外；490 从 π 递减到 0，途经 `t = π/2` 即 `C + minor`，顶点在右边线外；
   `/JSite39` 的 115（左）/ 114（右）同形。反着读（t 递增）走的是另外半个椭圆，两个顶点都折进本体矩形。
2. **构造线。** 同存储关着的 `Construction` 层上两条中心线（oid 491 / 492）从两个圆心向外伸 12.979 mm——正好是短半轴，终点就是顺时针读出的封头顶点。`igArc2d` 那篇裁 Manifold 端帽用的也是这一条：参数化本体的轴线指向端帽顶点。

## 数值

存储自己的坐标（米）。四条都是半个椭圆（Δ = π），`index` 都是 7；圆心是本体矩形左右两条竖边的中点，长半轴长是矩形半高，比 0.5 是标准 2:1 椭圆封头。

| oid | JSheet / 层 | `sweep_start` → `sweep_end` | 圆心 C | 长半轴 | 比 | 顶点 x = C ∓ minor |
|---|---|---|---|---|---:|---|
| 489 | 481 / 533 | 2π → π | (−0.01029614841, 0.0889) | (≈ 0, −0.02595843658) | 0.5 | −0.0232753667（左） |
| 490 | 481 / 533 | π → 0 | (0.1232044064, 0.0889) | (≈ 0, −0.02595843658) | 0.5 | 0.1361836247（右） |
| 115 | 96 / 149 | 2π → π | (−0.04064, 0.0889) | (≈ 0, −0.02032) | 0.5 | −0.0508（左） |
| 114 | 96 / 149 | π → 0 | (0.16764, 0.0889) | (≈ 0, −0.02032) | 0.5 | 0.1778（右） |

长半轴的 x 分量是 1e-18 量级的舍入残差（489 / 490 / 115 / 114 依次 4.768e-18 / 7.947e-18 / 3.733e-18 / 6.221e-18）。

上图：放置 184 的矩阵是单位阵、插入点 (223.846, 140.573) mm；左封头圆心 (213.550, 229.473)、顶点 x = 200.571，右封头圆心 (347.051, 229.473)、顶点 x = 360.030，本体矩形 (213.550, 203.515)–(347.051, 255.432)；长半轴 25.958 mm、短半轴 12.979 mm。

## 解码门槛（E-D5）

`decode_igellipticalarcs`（链门控：只在 `sheet_record_starts` 给出的记录起点上试）与 `decode_igellipticalarc_at` 都走 `IgEllipticalArc2dDecoder::decode_at`：

- 从 `parse_live_psm_header` 起步：类型字带 `0x8000` 的记录是原生读取器跳过的，不收（P-D12）；
- 类型码 `0x007E`（`PSM_TYPE_CODE_IGELLIPTICALARC2D`），btf = 75（`IGELLIPTICALARC2D_PAYLOAD_LEN`）；
- 七个 f64（`curve_doubles::<7>`）全有限、绝对值不超过 `GLINE2D_COORDINATE_DOMAIN_LIMIT`（1e9，与 `igCircle2d` / `igArc2d` 共用）；E-D5 写作 `COORDINATE_LIMIT`，本仓同名常量是 `.sym` 读取器的 10 m 上限，不是这一道；
- 长半轴非零，`0 < ratio ≤ 1`（比恰为 1 是圆弧，照收）。

角只要有限，不归一化，同 `igArc2d`。不合就返回 `None`、这一条拒收，任意字节不 panic。`streams::jsite::decode_nested_geometry` 在 `igArc2d` 之后、`igLine2d` 之前试它；`model::sheet_families` 故意不登记——它描述 `Sheet*` 流能装什么，这个族只读缓存本体。

## 投影（E-D1 / E-D3）

**不加 `SymbolPrimitive` 变体、不改 `PidSymbolDefinition`。** OCS 主工作树与工作树都以 `path = "../pid-parse"` 编同一个检出；主工作树的 `src/io/pid/symbols.rs::shape_primitive` 对 `SymbolPrimitive` 穷举匹配，`src/io/pid/tests.rs` 按字面构造 `PidSymbolDefinition`，
加变体或字段，另一会话正在合并的主工作树立刻编不过。所以记录解进 `JSiteNestedGeometry::elliptical_arcs`（`DecodedIgEllipticalArc2dRecord`；`serde(default, skip_serializing_if = "Vec::is_empty")`，没有这个族的图 JSON 不变），投缓存本体时每条弧化成一条 `SymbolPrimitive::BSpline`：

- **`bspline::elliptical_arc`**：扫角切成 `n = ceil(Δ / 45°)` 等段（`1 ≤ n ≤ 8`，`δ = Δ / n`），每段一个有理二次 Bézier：两端控制点在椭圆上、权 1；中间控制点是段中参数的椭圆点、从圆心往外推到 `1 / cos(δ/2)` 倍，权 `cos(δ/2)`。
  节点 `[0,0,0, 1,1, 2,2, …, n−1,n−1, n,n,n]`，`2n + 4` 个配 `2n + 1` 个控制点，`bspline::sample` 按「节点数 − 控制点数 − 1」读成二次。椭圆是圆的仿射像，圆弧的这套有理二次表示经仿射原样成立，**没有逼近误差**。
- A01 的封头：4 段、9 个控制点、节点 `[0,0,0,1,1,2,2,3,3,4,4,4]`，按 `SEGMENTS_PER_SPAN` = 8 采样得 33 点。每段不超过 45° 是为采样定的（E-D3）：OCS 每个节点跨度采 8 段，约 5.6° 一段，A01 封头的弦高误差约 0.03 mm。
- 接在本体自己的 B 样条之后（没有椭圆弧的本体，图元顺序不动），带弧的 `sheet_layer_ref` 与 `Some(index)`；`resolve_stroke_styles` 把椭圆弧的 `index` 一并查进本存储的样式表，封头有笔画样式。过了门槛的记录只有两角相等（扫角为零）时得 `None`，这一条不进本体。
- `JSiteNestedGeometry::len()` 计入椭圆弧；缓存本体的告警只在 N > 0 时多一项 `N elliptical arcs`（夹在 `B-splines` 与 `dimensions` 之间），别的存储一字不变。

OCS 本来就把 B 样条图元按跨度采样成 LWPOLYLINE，所以它的 `src` 一行不改。备选登记：`SymbolPrimitive::EllipticalArc` + DXF `ELLIPSE`（选中是椭圆、DXF 里是精确曲线），等阶段 B 两棵树合流后另开单。

## 钉住它的

- **golden 不动。** `tests/geometry_golden_snapshot.rs`（`normalized_geometry_matches_golden_snapshot`）的 `render_snapshot` 只序列化 `geometry.entities`，封头在 `symbol_definitions` 里；小单 E1 预计的「golden 只有 A01 变、重签」没有发生，六份 golden 一个字节不动。
- `tests/elliptical_arcs.rs`（新；fixture 不在时软跳过）：`a01s_cached_bodies_read_their_four_elliptical_arcs` 钉四条真记录的层、`index`、两角、圆心、长半轴与比（容差 1e-9）；`a01s_heads_bulge_outward_from_their_bodies` 对本体 481 与 96 各断言：
  B 样条比本体自己的多 2 条、排在最后、在封头的层上；左封头整条在左边线外侧、右封头整条在右边线外侧，最小 / 最大 x 落在顶点上（容差 1e-6），y 不出矩形高度。
- `sheet_records` 单测 3 条：`an_elliptical_arc_reads_its_angles_first_then_centre_axis_and_ratio`（合成记录用 489 的七个 f64 逐字段比对；`decode_igarcs` / `decode_igcircles` 不认它；比 = 1 照收）、
  `an_elliptical_arc_off_the_gate_is_refused`（btf 74 / 76、比 0 / 1.5、零长半轴、NaN 角、NaN 圆心、`0x8000` 位，各自拒收而链照走）、`an_elliptical_arc_survives_truncation_at_every_length`。
- `bspline` 单测 5 条：`an_elliptical_arc_samples_onto_its_ellipse_clockwise_from_start_to_end`（Property 1，定种 2000 例：采样点在椭圆上 1e-9 内、首尾点是起角 / 止角处的椭圆点、每一步顺时针、点数 = 段数 × 8 + 1，含四种整圈对）、
  `a01s_left_head_is_half_an_ellipse_bulging_left`、`a_pair_a_whole_turn_apart_is_a_full_ellipse`、`a_degenerate_or_non_finite_arc_is_none`、`random_garbage_never_panics`（两万组随机输入）。
- `tests/parser_panic_safety.rs` 的 `exercise_all_parsers` 加上两个新入口。
- `tests/parse_real_files.rs` 只重签 A01：`a_cached_body_says_which_layer_each_stroke_is_on_and_which_are_hidden` 的 `strokes_over_placements` (12, 6) → (14, 8)（全部 / 显示的笔画，两条封头在显示着的层 533 上）；
  `a_cached_body_carries_the_stroke_styles_its_own_storage_states` 的 `visible_over_placements` 6 → 8（每条显示笔画都带样式）。两张表里别的图不动。

## 登记不做（E-D4）

- `Sheet*` 流上的 `0x007E`：语料没有；出现时作为「无解码器」计进 not drawn，不会悄悄丢。
- `.sym` 库的 `0x007E`：备份库里有 50 条（phase34e），布局没核。
- `igEllipse2d 0x0063`：`.pid` 语料没有。
- `SymbolPrimitive::EllipticalArc` + DXF `ELLIPSE`：E-D1 的备选，阶段 B 两棵树合流后另开单。

## 复现与参考

- 复现：`cargo test --test elliptical_arcs`（A01 不在时软跳过）；单测在 `bspline::tests` 与 `parsers::sheet_records::nested_curve_family_tests`。
- 小单：OpenCADStudio `docs/plans/2026-09-30-a-cached-body-draws-its-elliptical-arcs.md`（事实表、E-D1 – E-D6、工作项 E1 – E3）。
- 扫向：`docs/analysis/2026-09-19-igarc2d-sweeps-clockwise-from-start-to-end.md`；四条记录最早的计数：`docs/analysis/2026-07-07-phase34e-missing-geometry-fixture-plan.md`。
