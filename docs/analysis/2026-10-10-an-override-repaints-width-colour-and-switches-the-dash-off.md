# 覆盖不只是指针：`JStyleOverride` 自带的宽 / 色 / 虚线位会压过基样式

> 日期：2026-10-10
> 范围：`pid-parse`（计划 `docs/plans/2026-10-10-an-override-applies-its-own-width-colour-and-dash.md` A0–A4）
> 结论类型：**native-reader + corpus + 屏幕 三证**——`style.dll` 给出位名与合成顺序，语料给出 53 条覆盖的逐字段取值，
> 10-09 的 SmartPlant 截图给出 0201 设备表 14 条格线的实际画法（细实线）。
> 前置：`2026-08-04-jstyleoverride-native-reader-settles-it.md`（v3 版式与成员偏移）、
> `2026-08-07-jstyle-simple-dash-type-linetype.md`（`0x002F` 与 `0x002E +54` 的虚线 join，本文是它的续篇）、
> `2026-08-24-placement-names-the-body-style.md`（放置样式压整个本体）
> 工具：`examples/probe_override_applies_its_own_fields`（本文一切语料数字皆由它产出）；
> headless IDA（`idapro` 开 `dlls/style.dll.i64` 的副本，不存盘）读 `sub_10028130` / `sub_1002A080` / `sub_10027C10` / `sub_10029390`。

## 0. 一句话

`resolve_line_style` 走到 `0x0030 JStyleOverride` 时一直只顺着 `+22` 取**基样式**的宽 / 色 / 虚线。覆盖记录自己 90 字节里 `+30` 是一个**属性掩码**，
置位的属性用覆盖自己的值：bit0 颜色 `+34`、bit1 线宽 `+42`、bit7 虚线类型 `+78`（0 = 不画虚线）。`style.dll` 就是这么合成有效样式的，
0201 设备表 14 条格线因此从「`Dashed` 0.35 mm 虚线」变成 SmartPlant 屏幕上那样的「0.13 / 0.18 mm 黑实线」。
语料 638 条图纸级记录里 52 条换了宽或色、22 条虚线变实、2 条实线变虚；缓存本体里 09-20 当作「可见虚线」的 57 笔全部是这种覆盖，按文件该画实线。

## 1. 现象

10-09 用 OCS `main` `aaedd334` 开 `DWG-0201GP06-01.pid`，与 SmartPlant 参考图 `pid-1.png` 逐实体对比：几何全对，设备表 14 条格线 OCS 画成 `PID-DASH-1`（3.5 / 1.75 mm）虚线，参考图是细实线；
3 条 `i=0.3 %` 坡度标记 OCS 画粉红 `#FE0060`，参考图是黑的。同一张图 LIA–LIT 电信号线两边都是虚线。

分界线在样式链的 hop 上：14 条格线全部 `hop=ViaOverride`（覆盖 86 ×11、101 ×3 → 基样式 85 `Dashed`），电信号线 `hop=Direct`（样式 79 `Electric`）。

## 2. native reader：覆盖怎么合成

### 2.1 v3 loader `sub_10028130`：流字段 → 成员

26 字节基类块（`sub_10003814`，含 `+12` 的虚线索引 `(w & 7) + 10`、`+14` 样式 id、`+22` 基对象引用）之后，逐字段 `jengine_1075(stream, size, this + N)`：

| 流偏移 | 大小 | 成员 | 含义（§2.2） |
|---|---:|---|---|
| `+26` | 4 | `this+88` | 未读；语料上恒为 3 或 4 |
| **`+30`** | 4 | **`this+96`** | **属性掩码** |
| **`+34`** | 4 | **`this+100`** | **颜色 COLORREF**（bit0） |
| `+38` | 4 | `this+152` | 填充色（bit8；填充族覆盖用） |
| **`+42`** | 8 | **`this+104`** | **线宽，米**（bit1） |
| `+50` | 8 | `this+112` | f64（bit2） |
| `+58` | 8 | `this+120` | f64（bit3） |
| `+66` | 8 | `this+136` | f64（bit5） |
| `+74` | 4 | `this+128` | u32（bit4） |
| **`+78`** | 4 | **`this+188`** | **虚线类型的样式 id**（bit7；0 = 无） |
| `+82` | 4 | `this+192` | 另一个引用（§2.3 的 changed 钩子里与 `this+176` 配对） |
| `+86` / `+88` | 2 / 2 | `this+144` / `this+146` | u16 ×2（bit6） |

与 08-04 文档 §1 的表逐项相同（那篇把 `+26` 记成 `this+88`，本轮核对无误）。

### 2.2 合成例程 `sub_1002A080(this, acc, …)`：逐位把自己叠到有效样式上

`acc+8` 是**已定下来的属性位**（沿链累计，近者先写）。例程逐位看 `this+96`，只有「本覆盖置位、且累计里还没定」的属性才写自己的值，最后 `acc+8 |= this+96`：

```text
bit0 0x01   acc.colour(+12)  = this+100            （还有一条：this+100 == 0x80000000 时无条件压过，见 §2.4）
bit8 0x100  acc.fill(+64)    = this+152，并置 0x40000000
bit1 0x02   acc.width(+16)   = this+104，连同基类块的宽度种类 this+92 → acc+4
bit2 0x04   acc+24 = this+112 ；bit3 0x08  acc+32 = this+120 ；bit4 0x10  acc+40 = this+128
bit5 0x20   acc+48 = this+136 ；bit6 0x40  acc+56/+58 = this+144/+146
bit7 0x80   acc.dash(+60)    = this+148（按 this+188 的 id 懒解析出来的 JStyleSimpleDashType 对象；id 为 0 → NULL = 不画虚线）
```

bit7 清零时例程也会把 `acc.dash` 写成 `this+148`（此时是 NULL），但 `acc+8` 的 bit7 不置位，于是基样式随后仍能写上自己的虚线——**结果就是「置位的用覆盖、没置位的用基样式」**，与本文实现一致。

### 2.3 取虚线类型的 getter `sub_10027C10` 与引用变更钩子 `sub_10029390`

```c
// sub_10027C10(iface, out)：iface - 76 = this
if ((char)this[96] >= 0) { *out = NULL; return S_OK; }            // bit7 清：没有自己的虚线类型
if (this[148] || SUCCEEDED(sub_100016B8(this[188], IID_DashType, &this[148])))   // 按 +78 的 id 解析，缓存在 this+148
    *out = this[148], AddRef;
```

`sub_10029390` 在「某个 id 变了」的通知里：`a2 == this[188] && bit7 置位` → 释放 `this+148`、重新解析——与 `JStyleBase` 处理 `+22` 基引用、`JStyleSimpleLine` 处理 `+54` 虚线引用的「id + 懒解析对象」是同一套机制。

### 2.4 颜色 `0x80000000`

合成例程对 `this+100 == 0x80000000` 无条件压过已定颜色——这是「随层 / by level」字。本 crate 没有层颜色可交，读到它时**保留基样式的颜色**（语料 6 条，全是符号本体内部 0.01 mm 的笔画）。

## 3. 语料：5 张图 53 条覆盖

探针把每张图每个 `StyleCluster`（根 + `JSite*`）的 `0x0030` 记录按 §2.1 逐字段摆开。全部 53 条长度都是 90 字节；`+50/+58/+66/+74/+82/+86/+88` **全为 0**；`+26` 恒为 3 或 4。

| 基样式族 | 掩码 | D06 | 0201 | 0202 | 工艺 | A01 | 说明 |
|---|---|---:|---:|---:|---:|---:|---|
| `0x002E` 实线 | `0x02` bit1 | 3 | 8 | 4 | 3 | 4 | 只改宽：0.13 / 0.18 / 0.50 / 0.70 / 0.01 / 2.0 mm |
| `0x002E` 实线 | `0x03` bit0+bit1 | 1 | 4 | 5 | 2 | – | 改色改宽：黑 0.13 压过 `#00FEA0` / `#00FFFF` / `#FE0060` 0.5；0202 的 109 用 `#800000` 0.35 压过 `As Drawn` `#0000FF` 0.5；6 条颜色是 `0x80000000` |
| `0x002E` 实线 | `0x82` bit1+bit7 | – | 2 | – | – | – | **`+78` = 8，指向本表 `0x002F "Dash"` 3.5 / 1.75**：实线基样式被覆盖成虚线（0201 `/JSite329` 48、`/JSite396` 9，宽 0.01） |
| `0x002E` 虚线 `Dashed` | `0x82` bit1+bit7 | 1 | 3 | 4 | 2 | 1 | `+78` = 0：**虚线关掉**，宽改 0.13 / 0.18 |
| `0x002E` 虚线 `Dashed` | `0x80` bit7 | – | – | 2 | – | – | `+78` = 0：只关虚线，宽仍 0.35 |
| `0x002A` 填充 | `0x40000100` bit8+bit30 | – | 1 | 1 | 2 | – | `+38` = `0x8080`（橄榄）或 0；线几何没有经过它们，本文不碰 |

**`+78` 是虚线槽位的语料旁证**：53 条里它只在 bit7 置位时非零，非零的两条都指向同一表里的 `0x002F`；bit7 置位且 `+78` = 0 的 13 条，基样式都是 `Dashed`，参考图上对应的 0201 格线是实线。

## 4. 影响

### 4.1 图纸级（`style_link_ratchet` 的 638 条）

| 图 | 变化 |
|---|---|
| 0201 | `0.350 #000000` 48 → 27、`0.350 #FE0060` 3 → 0，新增 `0.130 #000000` 21、`0.180 #000000` 3；虚线记录 15 → 1（只剩电信号线）。设备表：横 11 条 0.13、竖 3 条 0.18，全实线 |
| 0202 | `0.350 #000000` 59 → 38、`#FE0060` 3 → 0，新增 0.13 ×22、0.18 ×2；一个放置 `0.5 #0000FF` → `0.35 #800000`；虚线 9 → 1 |
| 工艺 | 6 条 0.35 → 0.13；无虚线变化 |
| D06 | 不变（它的覆盖全在 `/JSite145` 本体里） |
| A01（不在棘轮里） | `0.350 #000000` 79 → 33：`/JSite204/Sheet6` 的 37 条管线 → 0.50、9 条 → 0.70 |

语料调色盘 12 项 → 12 项：`0.130mm #000000` 182 → 231、新项 `0.180mm #000000` 5、`0.350mm #000000` 196 → 148、`0.350mm #800000` 21 → 22、`0.500mm #0000FF` 12 → 11、`0.350mm #FE0060` 6 → 0（删）。
名字与调色盘的交叉随之变多（`Dashed` 名下 0.13 ×14 / 0.18 ×5 / 0.35 ×3，`Normal` 名下 0.13 ×29 / 0.35 ×128）——名字是基样式的，宽是覆盖说的，这正是「名字不是调色盘的标签」那条论断的又一证据。

### 4.2 缓存本体（`parse_real_files::a_cached_body_carries_the_stroke_styles_its_own_storage_states`）

09-20 计划 P-E1 / P-E8 认定的「可见虚线 57 笔 / 11 个放置」——0202 阻火呼吸阀 8 + 污水池 1、工艺 `Xa` / `Xa chu` 45、以及 D06 / 0201 关闭层上的伴热 3 + 6——**全部是 bit7 置位、`+78` = 0 的覆盖**，按文件该画实线。
那次的 GUI 特写（`OpenCADStudio/docs/evidence/2026-09-20-cached-stroke-dash/`）拍的是 OCS 自己的画法，不是 SmartPlant 的，所以没有反证。
现在真正虚的本体笔画是 4 笔：0202 污水池直接点名 `Dashed` 的 3 条，0201 `/JSite396` 本体 119 的那条 0.01 mm 发丝线（覆盖 9 把它变虚）。
本体调色盘也变细：D06 Ball Valve Type 1 全 0.13；0201 Cap `#00FFFF 0.50` → 黑 0.13、Off-Unit → 黑 0.13 ×6 + 0.18 ×1、Parametric Manifold 0.35 → 0.13；工艺 `Xa` → 黑 0.13。
与库 `.sym` 的比对改成「缓存画的每种 (色, 宽, 虚线) 都是 `.sym` 自己 `StyleCluster` 定义得出的」：`Item Note & Label` 的缓存是 `PSMcluster0` 里那两笔 0.01 mm 锚点十字（随层色），库读取器只走 `Sheet*`，拿不到这两笔，但它们的样式（`.sym` 覆盖 8，字段与缓存的覆盖 21 逐字相同）就在表里。

## 5. 实现（`src/style_link.rs`）

- 常量：`OVERRIDE_MASK_OFFSET` 30、`OVERRIDE_COLOUR_OFFSET` 34、`OVERRIDE_WIDTH_OFFSET` 42、`OVERRIDE_DASH_REFERENCE_OFFSET` 78、`OVERRIDE_STATES_COLOUR` / `_WIDTH` / `_DASH` = `0x01` / `0x02` / `0x80`、私有 `OVERRIDE_COLOUR_BY_LAYER` = `0x8000_0000`。
- `StyleRecord` 新增 `override_mask` / `override_colour` / `override_width_m` / `override_dash_reference`（只在 `0x0030` 上填；颜色拒收高字节非零、`-2` 哨兵、随层字；宽度与 `read_symbology` 同一道合理性闸，且 0 不算宽）。
- `resolve_line_style` 的覆盖分支：以基样式为底，bit1 且宽可读 → 替宽；bit0 且色可读 → 替色；bit7 → `dash_named(+78)`（0 / 悬空 / 非 `0x002F` 都是实线），否则沿用基样式的虚线；`marker` 仍取基样式的。**`ResolvedLineStyle` 形状不变**，`style_id` 仍是基样式的，`hop` 仍是 `ViaOverride { override_id }`——OCS 不用改码。
- 单测 +8（宽 + 关虚线、色 + 宽、开虚线、掩码没点名的字段不动、随层色保基色、不合理值保基值、悬空虚线 id 实线、只有覆盖读这些字段）；棘轮调色盘重钉、交叉 / 跨度重钉、新增一条按 oid 钉 0201 14 条格线 + 电信号线仍虚；`parse_real_files` 缓存本体一条重钉。

## 6. 对 OpenCADStudio 的影响（计划 O1–O3）

- `tests/pid_import.rs`：0201 调色盘（`" 35 #000000"` 43 → 22、`" 35 #FE0060"` 删、新增 `" 13 #000000"` 21、`" 18 #000000"` 3）；`dashed_line_work_…`（3.5 / 1.75 不再出现在 0201 图纸线上，只剩 1.905 / 1.27）；`a_cached_strokes_dash_…`（工艺 45 / 0202 12 → 0 / 3，0201 多一笔）；0202 的放置调色盘若有钉。
- 批量基线：预计 5 行 `dxf_sha256` 变（0201、0202、publish-0202、工艺、A01），D06 不变；DXF 逐实体比只该变 `370` / `62` / `420` / `6`。
- 渲染策略提醒：0201 `/JSite396` 本体 119 那条 0.01 mm 虚线，OCS 按 P-E1 用放置线宽重涂后会成为一条**可见的**虚线；SmartPlant 里它是发丝线。P-E1「放置线宽压过逐笔线宽」的备选项（逐笔更细时取逐笔）该重新摆上台面——本文只登记。

## 7. 等级与未做

| 结论 | 等级 |
|---|---|
| `+30` 掩码、bit0 → `+34` 色、bit1 → `+42` 宽、bit7 → `+78` 虚线 id、0 = 无 | **native-reader**（§2）+ corpus（§3）+ 屏幕（0201 格线） |
| 「置位用覆盖、没置位用基样式」的合成顺序 | native-reader（§2.2） |
| `0x80000000` = 随层色 | native-reader（无条件压过）；本 crate 以「保基色」近似 |
| bit2 / 3 / 4 / 5 / 6 的字段含义 | 未读；语料全 0 |
| 填充族覆盖（bit8 `+38`、bit30） | 未应用到 `resolve_fill`；语料里线几何没经过它们 |

## 8. 复现

```powershell
cargo run --example probe_override_applies_its_own_fields          # §3 / §4 的全部数字
cargo test --test style_link_ratchet                                # 调色盘、交叉、0201 格线
cargo test --test parse_real_files a_cached_body                    # 本体笔画
```

IDA：`idapro.open_database("style.dll.i64", False)`，反编译 `0x10028130`（loader）、`0x1002A080`（合成）、`0x10027C10`（虚线 getter）、`0x10029390`（引用变更钩子）；
全库搜 `test byte ptr [reg+60h], 80h` 只命中这几处与 `sub_10027750` / `sub_10028CB0` / `sub_100294D0` / `sub_1002A310` / `sub_1002A3A0`，均是同一对象上的 bit7 判断。
