# `aux_hi`（payload `+8`）是图元所在的图层：那两个「常量 12 / 18」是 `Labels` 和 `ConsistencyChecks` 的 id

> 日期：2026-08-27
> 范围：`pid-parse`
> 结论类型：**corpus（五图 × 全部 15 个存储 × 全部记录链，无抽样、无采样窗口）**
> 前置：`2026-08-27-tag-184-is-the-sheet-layer-view-hierarchy.md`（§7 定的方向，本文接着做）、
> `2026-08-11-remaining-header-is-the-psm-aux-field.md`（本文补上它没能给出的语义）
> probe：`examples/probe_sheetlayer_edge_lives_on_the_graphic.rs`
> 棘轮：`tests/parse_real_files.rs::psm_aux_hi_is_the_sheet_layer_every_object_sits_on`
> 含**两处订正**：①guide §5 的「`remaining_header`（常量 12）／（常量 18）」；
> ②`IGBOUNDARY2D_AUX_HI == 12` 那道门——它不是帧装校验，是**只放行一个图层**。

## 0. 上一轮把方向定死了，但问法还是错的

`shlyhp.dll` 的 `AddObjectToSheetLayer` 说得很清楚：图层只持有一个计数（对象 `+8`，
记录 `+12`），成员那条边交给图元自己保管。所以边在图元身上——**可上一轮照着这个
去扫语料，扫了个平手**：图层 id 是 8 / 12 / 34 这种小整数，拿同量级的非图层 oid 做
诱饵重扫，诱饵还赢了几处。

问题不在结论在问法。「某条记录的 payload 里有没有出现某个小整数」这个问题**没有
零假设**——它必然一片命中，怎么加诱饵都只是把噪声量出来而已。

## 1. 换一个自带零假设的问法

图层自己报了数。一个存储里的这些图层，合起来报出的是一个**计数多重集**——`D06`
顶层那 12 个图层报的是 `10 4 0 1 0 0 2 10 0 0 0 0`。

于是问题可以改写成：**存在某个固定偏移，使得把这个存储的对象按那里的值分组，
恰好复原这张计数表吗**——每个图层都对，报 0 的也必须真的一个都没有。

这就不是「找得到吗」而是「分得对吗」。一个偶然落在 id 空间里的小整数进不了这种
考试：它得同时把 37 个非零计数全说对，还得让 54 个空图层保持空。零假设自带在
形状里，不用再造诱饵。

搜索空间：payload 前 256 字节 × `u16`/`u32` 两种宽度 × 两种键空间（图层的持久 id，
以及图层自己的 `+16` 图层号）。

## 2. 判决：`+8`，唯一的一个

| | |
|---|---|
| 15 个存储里精确复原计数表的 | **14** |
| 320 个图层里对上的 | **319** |
| 语料共有对象指向图层 | **1391**，图层声明 **1387** |
| 同一 oid 的多条记录在 `+8` 上打架的 | **0** |
| 除 `+8` 外还有别的偏移在**任何一个**存储里过关的 | **0** |

四张主语料图（不含 `A01`）上是**满分**：290 / 290 个图层、1240 / 1240 个对象，
这就是棘轮锁住的那组数。

唯一对不上的是 `A01/JSite204` 的 `Default`：声明 103，实际 107 个对象指着它
（多出 2 个 `Rectangle`、以及 line/text 若干）。`A01` 是 `export-test/publish-data`
下的导出件，不是主语料；这 4 个差额**本文没有解释**，按未解释记账，别当规律。

一处要说明：一个对象可以在链里有不止一条记录（`工艺` 的 `Labels` 有 314 个对象、
315 条记录）。图层数的是**对象**。按记录数会差 33 个，按对象数一个不差——这不是
调参，是「计数器数的是什么」这个问题本来就有唯一答案。

**「图层号」这条路是死的**：按 `+16` 那个图层号分组，15 个存储里一个都没过。
同名图层每个视图过滤集一份，号是重复的，本来也不足以定位。

## 3. 那两个「常量」的真身

guide §5 把这个字节写成：

```text
igLine2d   +8  u32  remaining_header（常量 12）
igPoint2d  +8  u32  remaining_header（常量 18）
```

**12 是 `Labels` 图层的 oid，18 是 `ConsistencyChecks` 的。** 顶层存储里这套 id
在四张图上是一样的：`Default` = 8、`Labels` = 12、`DrawingBorder` = 13、
`Notes` = 14、`WaterMark` = 15、`Hidden` = 16、`HiddenObjects` = 17、
`ConsistencyChecks` = 18、`HeatTrace` = 21。

之所以当年量出来像常量，是因为**样本恰好全在同一个图层上**：顶层的线大多是
`Labels`，顶层的点几乎全是 `ConsistencyChecks`（246 条点里 138 条）。换个存储、
换个图层，它立刻就变了。全语料 `igLine2d` 在 `+8` 上取到 20 多个不同的值。

`2026-08-11` 那篇没读错——它去原生序列化器把这 4 字节定成了 PSM 信封 8 字节 `aux`
的高半段，并且指出「`== 12` 是本仓自己发明的校验」。它只是没能说出应用层往这个槽
里放的是什么。**放的是图层。**

> ⚠ 一个仍然开着的问题：`PSMSerializeIn` 把 `aux` 读进一个再不引用的局部，说明
> **记录里的这 4 字节是台账字段的副本**，运行时未必从这里取值。本文证明的是
> 「写进去的是哪个对象」，**没有**证明「读的时候从这里读」。对解析来说够用——它
> 是这条边在文件里唯一的落点——但别把它说成运行时的权威来源。

## 4. 一道按图层筛的门，代价第一次量出来了

`decode_igboundaries` 至今要求 `aux_hi == 12`。它的注释很诚实：这条规则留着，是
因为「没人量过这个家族没有它是什么样」。现在量了——它不是帧装校验，
**它只放行 `Labels` 图层上的边界**：

| 家族 | 语料条数 | `aux_hi == 12` 能过的 | 被这道门挡下的 |
|---|---:|---:|---|
| `0x0013` igBoundary2d | 24 | **15** | **9 条**，都在 `JSite` 里的 `Default`（图层 156 / 199 / 465）|
| `0x0018` igLine2d | 646 | 284 | 门已于 Phase 40 撤掉；当年挡的 362 条里就有 `A01` 的整圈页面边框 |
| `0x004D` igTextBox | 235 | 158 | 无此门 |
| `0x0084` igLineString2d | 137 | 50 | 无此门 |
| `0x005E` igPoint2d | 246 | **0** | 无此门（它的「常量」是 18，不是 12）|

`A01` 那 80 条被拒的线，`remaining_header` 全是 `8`——**`8` 就是 `Default`**。
「第二种帧装」这个说法可以退休了：从来没有第二种帧装，只有第二个图层。

**门已在下一轮撤掉**，字段改名 `sheet_layer_ref`。代价比预想的小：那 9 条全在
`JSite*/PSMcluster0`，而 `streams/cluster.rs` 只把**叶名以 `Sheet` 开头**的流交给
几何解码器，所以它们本来就不在管线视野里。撤门后 Phase 34-D 的精确计数**一条没变**
（15 条仍是 15 条，全部 `sheet_layer_ref == 12`），这一点已写进棘轮。真正的缺口是
§8.3 的「没人扫 `JSite`」，不是这道门——但门留着，扫到那天就会静悄悄吃掉 9 条。

## 5. `+8` 顺手给出了一份「什么算图元」的名册

按家族看，这个字段是**干净的二分**——一个家族要么条条指着图层，要么条条为 0，
中间没有骑墙的：

| 写图层的家族 | 条数 | 类名（`tools/psm_type_clsid.py`）|
|---|---:|---|
| `0x0018` | 614 | Line Object |
| `0x005E` | 246 | Point Object |
| `0x004D` | 235 | Text Object |
| `0x0084` | 137 | LineString Object |
| `0x00CE` | 109 | JSymbol |
| `0x0013` | 24 | Boundary2d Object |
| `0x0115` | 14 | **JDim Object**（标注）|
| `0x0059` | 12 | **Circle Object** |
| `0x0061` | 12 | **Arc Object** |
| `0x003D` | 10 | SmartFrame2d Object |
| `0x0020` | 3 | **Rectangle Object** |
| `0x005D` | 1 | **BspCurve Object** |

**一个都不写的**：动态属性行 `0x0089`（1447 条全 0）、`0x00FA` DependencyObject
（366）、`0x0030` JStyleOverride（53）、图层子系统自己（`JSheetLayer` 290 条
`+8` 全 0——图层不在图层上）、`JSheet`、`SheetView`、`0x002C`/`0x002D`/`0x002E`
这些样式族，以及其余四十来个家族。

这份名册比它看上去更有用：**它是语料自己给出的「可画对象」清单**，不依赖任何
解码器认不认得这个家族。

### 5.1 两个「不在图层上」的总体，都讲得通

- **28 条 `igLine2d` 的 `+8` 是 0，全部在 `StyleCluster`。** 那正是 guide §5 里
  点符号的**字形线**——它们属于样式库，不在任何图纸上，所以没有图层。五张图无
  例外。
- **`DWG-0202` 的 `/Sheet6615`：4 条 line ＋ 1 条 rectangle，都指着 `6996`，
  而那个存储里根本没有这个图层。** 这个存储此前就被标记过异常（5 条记录一个
  space map 条目都没有）。现在两条异常合成一条：**`/Sheet6615` 整个是孤儿**——
  记录还在，它的图层和索引都没了。

## 6. 为什么空间表里没有这条边

`PSMspacemap` 是入边索引。若图元以「引用」的身份指向图层，每个图层都该挂一大串
入边。实测：落在 `JSheetLayer` 上的边只有 509 条，全部来自 tag 183（管理器登记，
290）、tag 184（视图过滤集选中，208）和 11 条 tag 182。**图元一条都没有。**

这和上一篇的 §6 是同一条规律的两半：**记录头上的两个引用槽都不进表**——`+4`
（`parent_ref`，1401 条非零，0 条入表）和 `+8`（图层，1240 条非零，0 条入表）。
表登记的是应用层的引用，不登记信封里的这两个。

顺带解释了 tag 181/182 那条重建路：`sub_564794D0` 把 `payload+4` 和 `payload+8`
当成这条记录的两个引用槽去登记（写死 181 / 182）。**框架自己就把 `+8` 当对象引用
看待**——这是本文结论在反汇编一侧的独立佐证，只不过那条路在正常保存的文件上从
不执行。

## 7. 语料里其实有圆、有弧、有矩形

`AGENTS.md` 至今写着「SmartPlant fixtures don't use standard IGDS `igCircle2d`
(0x0059), `igRectangle2d` (0x0020), `igArc2d` (0x0061) — zero hits cross-fixture」。
`+8` 这一刀顺手把它们翻出来了：**12 条圆、12 条弧、3 条矩形、1 条 B 样条**，
而且每一条都规规矩矩坐在一个图层上。

它们不在 `Sheet*` 流里，而在**嵌套 `JSite` 存储的 `PSMcluster0`** 里：

```text
0x0059 Circle    D06/JSite145 4、DWG-0201/JSite329 5、DWG-0202/JSite793 1、工艺/JSite7559 2
0x0061 Arc       DWG-0201/JSite329 5、DWG-0201/JSite396 2、DWG-0202/JSite793 5
0x0020 Rectangle A01/JSite204 2（＋DWG-0202/Sheet6615 那条孤儿）
0x005D BspCurve  DWG-0202/JSite793 1
```

所以那句话的准确说法是「**顶层 `Sheet*` 流里没有**」。`0x0013 igBoundary2d` 也一样
横跨两处（`Sheet6` 15 条、`JSite/PSMcluster0` 9 条），而后一处正好是被 §4 那道门
挡掉的那 9 条——**同一个盲区的两种表现**。

## 8. 原生一侧：厂商管这条边叫 `Layer`，而且它根本不是一个数

上一版这里列的第 4 条是「去 `imagdex.dex` 拿厂商给这个槽的名字」。库建好了，
拿到了，而且答案比预期干净。

### 8.1 一对同名存取器

`imagdex.dex` 导出 `HGeomGetLayer` / `HGeomPutLayer`——**「一个 Geom 的 Layer」**，
厂商自己给这条边起的名字就是这个。展开后（`HGeomGetLayer_0` @ `0x102AE070`）：

```c
obj->QueryInterface(204D4DD1-B174-11CE-B914-08003601C6EB, &p);  // 「我能待在图层上」
p->vtbl[+56](&layer);                    // 图元交出它所在的图层对象
layer->QueryInterface(C3BE37E0-B703-11CE-9D6C-08003601EB68, &l);
l->vtbl[+40](&bstr);                     // 图层的名字
```

写侧 `HGeomPutLayer_0` 反着走：`HGetTypedParent` → `GetSheetLayerManager`
（`D126A7C0-…`）→ 管理器按名字查出图层 → `AddObjectToSheetLayer(layer, obj)`。

于是 `204D4DD1` 这个接口的 **`+52` / `+56` 是一对 Put/Get Layer**，`+56` 就是
「问图元你在哪个图层」的那个槽。本文测出来的记录 `+8`，是它的持久化形态。

### 8.2 图层上那条边存的是**对象引用**，不是编号

> 「图层把自己的 `IUnknown` 交给图元」这一句
> `2026-08-27-tag-184-is-the-sheet-layer-view-hierarchy.md` §7 已经读出来了
> （那里叫第 13 / 14 槽，就是这里的 `+52` / `+56`）。本节新增的是**字段偏移**
> 和它与 `aux` 引用槽的对账，不是重新发现一遍。

`shlyhp.dll` 有完整 RTTI，类名直接可读：`SheetLayer` / `SheetLayerGroup` /
`SheetLayerManager`，接口 `IJLayer` / `IJLayerManager` / `IJSheetLayerManager`。
`SheetLayer::IJLayer` 的 vtable `+28` 是 AddObject，实现是 `sub_1000B140`：

```c
pGraphic->QueryInterface(204D4DD1-…, &p);
p->vtbl[+52]( *(a1 + 36) );   // 把「图层这个对象」交给图元
++*(a1 + 4);                  // 图层自己的计数 +1
```

`a1` 是 `IJLayerImp` 子对象指针，位于 `SheetLayer` 基址 `+4`（`sub_1000AF80` 在那儿
装 vptr）。所以：

| 表达式 | `SheetLayer` 基址偏移 | 是什么 |
|---|---|---|
| `*(a1 + 4)` | `+8` | 成员计数——**正是上一轮从 `AddObjectToSheetLayer` 读到的那个** |
| `*(a1 + 8)` | `+12` | 图层名（`sub_1000B910` 拿它 `SysAllocString`）|
| `*(a1 + 36)` | `+40` | 构造函数里写的 controlling `IUnknown`——**图层对象自己** |

**交给图元的是图层对象的指针，不是它的 id。** 这条边在运行时是一个 COM 引用；
`+8` 里那个 oid 是它被序列化之后的样子。

### 8.3 这就解释了读侧为什么可以不读

§3 那个警告可以收窄了。`PSMSerializeOut`（`radsrvitem.dll` `0x56491E80`，见
`2026-08-11` 那篇）不是从几何类身上取 `aux`，而是从**对象记录的台账槽** `+32/+38`
或 `+34/+40` 抄过来的；`sub_564794D0` 又把 payload `+4`/`+8` 这两个槽写死登记成
tag 181/182 的**引用**。三处对上：框架把 `aux` 的两半当作这条记录的两个引用槽，
应用层往第二个槽里放的是图层对象，所以 `PSMSerializeIn` 把它读进局部就扔——
**引用是引擎的引用层重建的，不是类的 `Load` 重建的。**

仍然**没有**追到的：引擎把「持有的引用」换算成 oid 的那一步，以及反过来的重建路。
本文能说的是「这个槽是引用槽，装的是图层对象」，不能说「运行时从这 4 字节取值」。

### 8.4 未能确定的

`204D4DD1` 和 `C3BE37E0` 的**接口名**没拿到：`tools/clsid_registry.py` 只收
coclass，这两个 IID 在 `D:\pid` 下也没有任何文本形态（`.reg` / `.idl` / `.tlb` 都
没有）。按 `shlyhp.dll` 的命名习惯，`C3BE37E0` 极可能就是 `IJLayer`——但这是推测，
没有证据，本文不采用。

## 9. 已落地与下一步

**已落地（第一轮）**：

- probe `examples/probe_sheetlayer_edge_lives_on_the_graphic.rs`（9 项）；
- 棘轮 `tests/parse_real_files.rs::psm_aux_hi_is_the_sheet_layer_every_object_sits_on`
  （290 / 290 图层、1240 / 1240 对象、图层自身 `+8` 全 0、写图层的家族集合）；
- guide §5 与 §4 订正，CHANGELOG 同步。解码行为零变化。

**已落地（第二轮，本文 §8 与撤门）**：

- 撤掉 `IGBOUNDARY2D_AUX_HI == 12`；`SheetIgBoundary2dDecoded` /
  `DecodedIgBoundary2dRecord` 新增 `sheet_layer_ref`（payload `+8`）；
- 单测 `igboundary2d_accepts_every_sheet_layer_the_corpus_carries` 取代
  `igboundary2d_rejects_wrong_remaining_header`；
- Phase 34-D 棘轮加一条 `sheet_layer_ref == 12`，把「撤门后计数不变」钉住。

**下一步（按收益排序）**：

1. **`JSite` 存储里的几何没人解码**：圆 12、弧 12、矩形 3、B 样条 1，外加那 9 条
   边界，全在 `JSite*/PSMcluster0`。`streams/cluster.rs` 只收叶名 `Sheet*` 的流，
   这块是和「被拒记录」并列的独立缺口，现在是最大的一块。
2. **把 `sheet_layer_ref` 提到其余家族的 DTO 上**（line / point / text /
   linestring / symbol）。渲染侧真正要的是图层名，以及「计数为 0 的图层画不出任何
   东西」——`D06` 顶层十二个图层的计数是 `10/4/0/1/0/0/2/10/0/0/0/0`，八个是空的。
3. **接口名**：找一个带 RTTI 且实现了 `204D4DD1` 的模块，把 §8.4 那个空补上。

## 10. 复现

```powershell
cd pid-parse
cargo run --example probe_sheetlayer_edge_lives_on_the_graphic
cargo test --test parse_real_files psm_aux_hi
cargo test --lib igboundary2d_accepts_every_sheet_layer
python tools/psm_type_clsid.py 0x20 0x3D 0x59 0x5D 0x61 0x115
```

原生一侧（`ida-bridge`，库已建在 `dlls/imagdex.dex.i64` / `dlls/shlyhp.dll.i64`）：

```powershell
ida-bridge exec <imagdex-client> --sql "SELECT decompile(0x102AE070) AS t"   # HGeomGetLayer_0
ida-bridge exec <imagdex-client> --sql "SELECT decompile(0x102AE230) AS t"   # HGeomPutLayer_0
ida-bridge exec <shlyhp-client>  --sql "SELECT decompile(0x1000B140) AS t"   # IJLayer::AddObject
ida-bridge exec <shlyhp-client>  --sql "SELECT decompile(0x1000AFC0) AS t"   # SheetLayer ctor
```
