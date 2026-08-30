# tag 184 那 306 条不是「只活在表里」：视图过滤集按**名字**写成员

> 日期：2026-08-27
> 范围：`pid-parse`
> 结论类型：**corpus（四图全部 tag-184 / tag-183 边 × 全部记录链的 join，无抽样）**
> ＋ **native-reader（五个类名全部出自 RAD 类注册表）**
> 前置：`2026-08-27-the-spacemap-is-an-incoming-reference-index.md`（本文订正它 §3 的一句话）
> probe：`examples/probe_psmspacemap_tag184_viewfilterset_edges.rs`
> 含**一处订正**：「184 这类找不到 payload 对应的，是只活在这张表里的应用层登记边」
> ——引用者其实把这些成员写下来了，写的是**名字**不是 id。

## 0. 要验的命题

入边那一轮把 13 个 tag 都对上了引用者 payload 里的一个固定偏移，只有 184 对不上：
366 条边里只有 60 条能在引用者的记录里找到条目 id。当时的收尾说法是「应用层登记边，
只活在这张表里」。

这句话很贵。它等于说这张表在这一处**不是索引而是主存储**——删了表就丢数据。真要这么
断，得先把四条更便宜的解释挨个杀掉：换个宽度存的、记录塞不下、只写了个数、写在别的
流里。

## 1. 判决：一条也不缺，写的是名字

`Top ViewFilterSet` 的记录里带着一串 UTF-16 名字，`JSheetLayer` 的记录里带着自己的
名字。两边对起来：

| 命题 | 结果 |
|---|---|
| 一条 `JSheetLayer` 入边，它那个图层的名字在引用者（集合）的记录里 | **208 / 208** |
| 一个集合，它指到的每个图层的名字都在自己记录里 | **49 / 49** |
| 一条 `JSheet` 入边，目标 id 就坐在集合记录的 `+16` | **49 / 49** |
| 一个 `0x0060`，`+12` 就是它持有的集合数 | **11 / 11** |

所以成员关系一直都在记录里，只是**按名字写的**；空间表提供的是「这个名字在本存储里
是哪个对象」。这不是主存储，还是索引——只不过它索引的是一层**名字到对象**的解析，
不是一个可以逐字节比对的 id 字段。

顺带把那四条便宜解释的账也记上（`§2` 报告）：366 条边里 60 条带完整 `u32`（其中 49
条就是 `+16` 那条 `JSheet` 边），76 条能凑出一个 `u16` 段内 index，**289 条在任何宽度
下都不在引用者 payload 里**。所以「换个宽度」确实是死的——错的不是宽度，是「一定得
是个 id」这个前提。

## 2. 五个家族全部有厂商名字：这是图纸 / 图层 / 视图子系统

tag 184 碰到的每一个家族，`tools/psm_type_clsid.py` 都给得出类名：

| type code | 类名 | 模块 |
|---|---|---|
| `0x0057` | `Top ViewFilterSet` | `viewfil.dex` |
| `0x0060` | `Top ViewFilterSet`（同名第二个 CLSID）| `viewfil.dex` |
| `0x0076` | `SheetView` | `sheetvw.dvx` |
| `0x0114` | `JSheet Object` | `docext.dex` |
| `0x0042` | `JSheetLayerManager Object` | `shlyhp.dll` |
| `0x0081` | `JSheetLayer Object` | `shlyhp.dll` |
| `0x0088` | `JSheetLayerGroup Object` | `shlyhp.dll` |

`0x0057` / `0x0060` 正是上一轮查 `TopVFSet` 时注册表回的那两个码（87 和 96）。当时它们
是用来**否掉**「tag 是 type code」的；这一轮它们从另一头回来当了正面证据——tag 184 的
引用者，记录家族恰好就是这两个码。

## 3. 形状是死的，60/60 没有例外

| 引用者 | 数量 | 它的入边指向 |
|---|---|---|
| `0x0060 Top ViewFilterSet` | 11（**每个存储恰好一个**）| 1× `SheetView` ＋ N× `0x0057` |
| `0x0057 Top ViewFilterSet` | 49 | 1× `JSheet` ＋ 1× `JSheetLayerManager` ＋ N× `JSheetLayer` |

对账：`11×1 + 49 = 60` 条来自 `0x0060`，`49 + 49 + 208 = 306` 条来自 `0x0057`，
合计 366。§3.2 里那个反复出现的「60 / 306」正是这两组，不是巧合也不是同一件事——
一个是「按 id 找得到的边数」，一个是「`0x0057` 发出的边数」，两个 306 撞在一起纯属
数字巧合，别当规律。

每个存储一棵树：

```text
0x0060 Top ViewFilterSet          每个存储一个
  ├─ SheetView                    1
  └─ 0x0057 Top ViewFilterSet     N（顶层 1，JSite 里 1..17）
       ├─ JSheet                  1   ← 集合记录 +16 直接写着
       ├─ JSheetLayerManager      1
       └─ JSheetLayer             N   ← 集合记录里按名字列着
```

## 4. 两个记录的布局

**`0x0081 JSheetLayer`**（290 条，长度 42..70）。字段名不是猜的——`shlyhp.dll` 的
`SheetLayer::IJPersistImp::Save` 逐个写出来的，见 §7：

```text
+0  u32 oid                （PSM 框架）
+4  u32 parent_ref         （PSM 框架）
+8  u32 ?                  （PSM 框架，全语料为 0）
+12 u32 图层上的对象数     ← 对象 +8，加入 ++ / 移除 --
+16 u32 图层号             ← 对象 +20，构造时置 -1 表示未分配
+20 u32 字符数 ; UTF-16 图层名     ← 对象 +12
+?  u32 字符数 ; UTF-16 第二个名   ← 对象 +16，全语料为空
+?  u32 ?                  ← 对象 +24
```

按这个读法四种长度全部精确收尾：`Default`(7) 46 字节、`Labels`(6) 44、
`DrawingBorder`(13) 58、`ConsistencyChecks`(17) 66。

语料里出现过的名字：`Default` `Label` `Labels` `HiddenObjects` `Hidden`
`Heat Trace` `HeatTrace` `Jacket` `Dimension` `Construction` `Invisible`
`WaterMark` `DrawingBorder` `Notes` `NotesAG` `ConsistencyChecks` `NotClaimed`
`ClaimedOnlyByOthers` `LinkInfo_1` `LinkInfo_2`。

**同名图层是多份对象，不是一个。** `D06` 的 `JSite145` 里有四个叫 `Default` 的
`JSheetLayer`（id 8 / 34 / 94 / 109），每个视图过滤集一份——**图层状态是按视图存的**，
所以 290 个图层对象只对应二十来个名字。

**`0x0057 Top ViewFilterSet`**（49 条，长度 160..490）：

```text
+0  u32 oid
+12 u32 2          （形式号；0x0060 这一位是它持有的集合数）
+16 u32 JSheet 的 id                     ← 49/49
+20 u32 1 ; +24 u32 1 ; +28 u32 0
+32 u32 ?          （**不是**图层数，见 §7）
…   一段显示状态字节（`FF 02 00 …` 这类）
…   若干 { u32 字符数 ; UTF-16 名 } —— 图层名表
```

**`0x0060`** 恒定 28 字节：`oid ; 0 ; 0 ; 持有的集合数 ; 1 ; 1 ; 0`。它连塞都塞不下自己
的成员——`DWG-0201/JSite329` 那个持有 18 条边，28 字节里没有 72 字节的地方放。这条是
「记录塞不下」这个解释唯一成立的地方，而它成立的方式恰好也支持 §1：`0x0060` 靠 `+12`
的**计数**加空间表的入边活着，一个 id 都不写。

## 5. tag 183 是同一个子系统的另一半

`JSheetLayerManager` 在 `+12` 起的 4 步长表里列它管的东西，语料 357 条：

| 边 | 数量 |
|---|---|
| `JSheetLayerManager` → `JSheetLayer` | 290 |
| `JSheetLayerManager` → `JSheet` | 56 |
| `JSheetLayerManager` → `JSheetLayerGroup` | 11 |

**290 个 `JSheetLayer` 每一个都恰好被一个 manager 列着，290/290。** 所以 manager 是
图层的全量登记处，视图过滤集只挑其中一部分（208/290）。两个 tag 分工清楚：

- **183 = 「这个 manager 管着我」**，全量，写在 manager 的 id 表里；
- **184 = 「这个视图过滤集选中了我」**，子集，写在集合的名字表里。

## 6. 一个干净的空缺：`Default` 从来不被指

49 个集合**每一个**都在自己记录里写着 `Default`，而**没有一个**发出指向 `Default`
图层的 184 边（49/49）。其余没有边的名字零散得多：`ConsistencyChecks` 4 次、
`Labels` 4、`Notes` 4、`DrawingBorder` 3、`HeatTrace` 3、`WaterMark` 3、`Invisible` 2、
`LinkInfo_1` / `LinkInfo_2` / `NotesAG` 各 1。

`Default` 那 49/49 太齐了，不像巧合；最省事的读法是「集合只登记状态偏离默认的图层，
`Default` 是那个基线」。但本轮**没有**测到任何一个字节支持「偏离」这个词，所以只把
计数记下来，不把解释写进 guide。

## 7. 两条否定，都得记下来

**（1）`0x0057 +32` 不是图层数。** 36/49 对得上，剩下 13 条是系统性的：三张图的顶层
`id 19` 写 0 而实际有 4~5 个图层，`JSite151/34`、`JSite396/111`、`JSite396/131` 写 1 而
实际 2，另有三个写 5 而实际 6。差值不恒定，所以既不是「少算基线那个」也不是别的
偏移量。这一位没认，别拿它当计数用。

**（2）「图元记录里写着自己的图层」——语料判不了，但 §7 的读器把方向定了。**
这个问题值得问——它是渲染侧真正想要的那条边——但**对照组把它打平了**。图层 id 是
8 / 12 / 34 这种小整数，直接扫「图元 payload 里有没有图层 id」必然一片命中。拿同量级
的非图层 oid 做诱饵重扫：

| 存储 | 命中图层 id 的记录 | 命中诱饵的记录 |
|---|---|---|
| `DWG-0201/JSite329` | 207 | **208** |
| `DWG-0202/JSite793` | 140 | 116 |
| `工艺/JSite7559` | 83 | **92** |
| `D06/JSite151` | 9 | **11** |
| `D06/JSite145` | 86 | 74 |

`JSite` 存储里阴阳性完全不携带信息，好几处诱饵还赢了。顶层存储确实有超出
（`DWG-0201` 354 : 44、工艺 497 : 55，诱饵集更小，粗算每 id 命中率仍高 3~6 倍），但
`D06` 顶层只有 5.8 : 4.9 基本持平。**语料判不了这条。** 方向由 §7 的反汇编给出，
编码待查——下一轮得改成「某个固定家族的某个固定偏移」这种有形状的问法，不能再做整表
扫描，这正是 guide §8.1 记的那个坑。

> ✅ **已在同日结掉：偏移是 payload `+8`。** 换的问法正是本节末尾要的那种「有形状
> 的」——不问「payload 里有没有这个 id」，问「按某个固定偏移分组能不能**精确复原
> `+12` 那张计数表**」。前 256 字节 × `u16`/`u32` 里只有 `+8` 过关：四张主语料图
> **290/290 个图层、1240/1240 个对象**，报 0 的图层一个都不多。
> 那正是本仓一度叫 `remaining_header`、Phase 40 后叫 `aux_hi` 的字段——PSM 信封
> `aux` 的高半段。guide §5 记的「常量 12 / 18」是采样偏差：`12` 是 `Labels` 的
> oid，`18` 是 `ConsistencyChecks` 的。
> 见 `2026-08-27-aux-hi-is-the-sheet-layer.md`。

## 7. `shlyhp.dll`：图层只记个数，边挂在图元身上

**等级：native-reader。** `shlyhp.dll`（102 KB，`D:\pid\RADInstallA~\`）**带完整
RTTI**，类名和接口名直接读得出：`SheetLayer` / `SheetLayerGroup` /
`SheetLayerManager`，接口 `IJLayer` / `IJLayerGroup` / `IJLayerManager` /
`IJSheetLayerManager` / `IJPersist` / `IJCopy` / `IJDelete` / `IJCompute`。
IDB 在 `dlls/shlyhp.dll.i64`（`dlls/` 已 gitignore）。

**先对一次表**：`SheetLayer::IJPersistImp::GetClassID` 返回的 CLSID 是
`ED78D960-D33E-11CE-9D79-08003601EB68`——正是 §2 那张表里 `0x0081` 的 CLSID。
所以「`0x0081` 就是 `shlyhp.dll` 的 `SheetLayer`」有了模块自己的签名，不只是查表。

**对象布局**（构造函数 `sub_1000AFC0` 逐个写出）：`IJLayerImp` 子对象在 `+4`、
`IJPersistImp` 在 `+28`（正是 persist 方法里那个 `a1 - 28`）、`IJCopyImp` `+32`、
`IJDeleteImp` `+36`。`IJLayerImp` 的构造把 `+8` 置 0、`+20` 置 **-1**，而
`Save` 写的顺序就是 `+8`、`+20`、`+12` 的名字、`+16` 的名字、`+24`——§4 那张布局表
由此而来。取图层号的方法先判 `< 0` 再返回 `u16`，**-1 = 未分配**，和构造对得上。

**关键的那条：`AddObjectToSheetLayer(layer, object)`。** 导出表直接给了名字
（还有 `AddObjectToActiveSheetLayer` / `GetSheetLayerManager` /
`CreateSheetLayerManager`）。它做四件事：

1. 把 `object` QI 到 `204D4DD1-B174-11CE-B914-08003601C6EB`——「我能待在图层上」这个接口；
2. **问 object 它现在在哪个图层**（该接口第 14 槽，出参是个图层）；
3. 不一样就先从旧图层摘掉（第 8 槽）；
4. 挂到新图层（第 7 槽）。

而图层这一侧的加/减（`IJLayer` 第 7/8 槽）只做两件事：把图层的 outer `IUnknown`
交给图元（该接口第 13 槽，和第 2 步的第 14 槽是一对存取器），然后 `+8` 那个计数
`++` / `--`。

**所以：图层不持有成员表，只持有一个计数；那条边存在图元身上。** 这也解释了空间表
里为什么没有「图层 → 图元」的边——根本没有这种边可存。§6.2 的语料扫描问对了问题，
只是小整数噪声把它淹了；接下来该问的是**每个图元家族自己的 `Save` 把这个引用写在哪
一个固定偏移**，那要去图元所在的模块看，不在 `shlyhp.dll` 里。

**顺带**：`+12` 那个数从此有了名字——**这个图层上有多少个对象**。`D06` 顶层十二个
图层的计数是 10/4/0/1/0/0/2/10/0/0/0/0。计数为 0 的图层画不出任何东西。

## 8. 对 §3.2 的订正

原文：

> 184 这类「payload 里找不到」的边（视图过滤集成员关系等）是**只活在这张表里的应用层
> 登记边**。

改成：引用者**写了**这些成员，写的是名字（208/208）；空间表提供名字到对象的解析。
这张表在这一处仍然是索引，不是主存储。原文那句在字面上（「payload 里找不到 id」）
没错，错在从它跳到了「这条关系只存在于表里」。

## 9. 复现

```powershell
cd pid-parse
cargo run --example probe_psmspacemap_tag184_viewfilterset_edges
cargo test --test parse_real_files psm_space_map_184
python tools/psm_type_clsid.py 0x42 0x57 0x60 0x76 0x81 0x88 0x114
```

§7 的 IDB（`dlls/` 已 gitignore，从 `D:\pid\RADInstallA~\` 复制后现建）：

```powershell
ida-bridge exec-idb --input dlls\shlyhp.dll --out-idb dlls\shlyhp.dll.i64 --save
ida-bridge supervisor start-idalib --idb dlls\shlyhp.dll.i64
# SheetLayer::IJPersistImp::Save = sub_1000C3F0；构造 = sub_1000AFC0 / sub_1000B070
# AddObjectToSheetLayer = sub_10005030
```
