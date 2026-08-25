# 点画不画，由它的线终止符说了算：`0x002E → 0x0033 → 0x0032 → 0x007B → 0x0018`

> 日期：2026-08-25
> 范围：`pid-parse` + `OpenCADStudio`
> 结论类型：**native-reader（类身份与链路语义）+ corpus（引用偏移，五图穷尽）**
> 含**三处推翻**：判别码是颜色、斜杠是一笔 15.24mm、字形只有一种。
> 前置：`2026-08-24-placement-names-the-body-style.md` §4「哪些点画、哪些点不画：
> 判别码还没找到，先按观察落地」——本文关掉这一条。
>
> **§5 是当天晚些时候补的，它比上面的都重要**：样式库把这四个字形分别叫
> `psOk` / `psWarning` / `psError` / `psApproved`——**这些标记是审核状态**。
> 读到这里之前，§3.2 把对勾叫成了「折线」、§4.1 把 X 叉叫成了「模板」，
> 两处都已按 §5 就地更正。§4 的放大倍数也重测过，`~1.7` 站不住。

## 0. 上一轮留下的问题

`DWG-0201` 屏幕上 11 个点带蓝色标记（10 个立管顶 + 1 个容器入口），
另外 64 个点（53 个 `#000000 0.10` 接点 + 11 个 `#000000 0.35` 立管脚）
一个都不画。上一轮逐字节比对过点记录、样式记录、父聚合，
**没找到「画/不画」的判别码**，只能按观察规律作门：非黑色画，黑色不画。

判别码在的。它不在点记录里，也不在颜色里——**在样式往下挂的那条链上**，
上一轮走到 `0x002E` 就停了。

## 1. 链路

```text
igPoint2d          +14  ──▶  JStyleSimpleLine      (0x002E，58 字节那种)
JStyleSimpleLine   +54  ──▶  JStyleLineTerminator  (0x0033)   ← 样式 id
JStyleLineTerminator +46 ──▶ JStylePointSymbol     (0x0032)   ← 样式 id
JStylePointSymbol  +26  ──▶  Group implementation  (0x007B)   ← **oid**
Group              +28  ──▶  两条 Line Object      (0x0018)   ← **oid**
```

那两条 `0x0018` 就是**字形本体**。

### 类身份：native-reader

`style.dll` 的类名串加 `tools/psm_type_clsid.py` 的 CLSID 表，两头对上：

| 类型码 | CLSID | 类 |
|---|---|---|
| `0x0032` | `47FCC33B-2D0F-11D0-A1FF-080036A1CF02` | **JSL PointSymbol Style**（`JStylePointSymbol`）|
| `0x0033` | `47FCC33C-2D0F-11D0-A1FF-080036A1CF02` | **JSL Line Terminator Style**（`JStyleLineTerminator`）|
| `0x007B` | `DA02A6D0-C991-11CD-B02F-08003601BE3A` | Group implementation（`imagdex.dex`）|
| `0x0018` | `2D4E13C0-D3D1-11CD-8AEA-08003601B44A` | Line Object（`igLine2d`）|

关键是接口表里那一条：**`JStylePointSymbol::IJGraphicImp`**。
点符号样式**本身就是个图形对象**——这就是一条样式记录为什么能拥有几何：
它握着一个组，组里装着线。

这同时坐实了 `docs/pid-format-guide.md` §4 在 8-05 写下的猜想——
「`0x0032`/`0x0033` 只出现在 `StyleCluster`、且每张图都成对等量，
**线终止符本质上就是画在线端的点符号**，成对出现符合语义」——
一字不差，而且现在它有了一个真实的渲染后果。

反编译 `JStyleLineTerminator` 的 COM 复制辅助（`style.dll!sub_10039290`）
还能看到它带**两个**引用槽（对象 `+88`/`+92`，各配一个惰性解析的缓存指针）：
起点终止符一个、终点终止符一个。语料里五张图全部只填后一个（`+46`），
前一个置零。

### 字形记录：本仓自己的布局，一字不差

`0x0018` 不是新类型。`symbol_library.rs` 早就有 `TYPE_LINE = 0x0018`，
`sheet_records.rs` 更是把 payload 写死了：

```text
+0   u32  oid          +4  u32 parent_ref     +12 u16 sub_type
+14  u32  index        +18 f64 start.x        +26 f64 start.y
+34  f64  end.x        +42 f64 end.y          共 50 字节
```

`StyleCluster` 里这些记录**逐字段满足这个布局**，长度正好 50，
`+4` 的 parent 正好指回列出它的那个组。不是新猜的偏移，
是旧结论在新位置上自己对上了。

## 2. 判别码

**黑点那个点符号的组里，两条线是 `(0,0)→(0,0)` 的退化线；蓝点那个是两条真线段。**
没有长度的符号没有东西可画。

本仓 `decode_iglines` 本来就拒绝 `start == end`——「零长度不是线」这条规则
早就写在那儿，跟 SmartPlant 的行为一口径。

组记录还给了两个同向信号：`+20` 的 f64 尺度黑的是 `0.0`、蓝的是 `1.0`；
成员包围盒黑的是 `7FFF/7FFF/8000/8000` 空盒哨兵、蓝的是真盒。
五张图三个信号全一致。取「成员线是否退化」作权威读法，因为它是
**几何本身**，另外两个是它的缓存。

### 五图穷尽验证

| 图 | 画（活字形） | 不画（空字形） | 不画（未点名终止符） |
|---|---|---|---|
| DWG-0201 | **11** | 53 | 11 |
| DWG-0202 | **5** | 22 | 4 |
| 工艺管道及仪表流程-1 | **11** | 23 | 2 |
| D06 | **1** | 9 | 0 |
| A01 | 2 | 1 | 1 |

`DWG-0201` 的 11 与屏幕真值一个不多一个不少。

**它比颜色规则严格更强**：`DWG-0202` 与工艺管道都定义了带活字形的
`#FF0000` 点符号却没有任何点引用它——颜色规则根本看不见这种记录。
而「未点名终止符」这一档颜色规则也表达不出来：立管脚用的是 54 字节的
短 `JStyleSimpleLine`，短到根本放不下 `+54` 这个引用。

## 3. 顺带推翻的两件事

### 3.1 不是一道 15.24mm 的斜杠，是两笔

对目标截图重做连通域聚类，**radius=1、不架桥**：蓝色像素干净地分成
**11 个大簇 + 11 个小簇**。上一轮用 3px 桥把一大一小并成了一条，
才读出「15.24mm 单斜杠」。文件里存的也正是两条线：

- 主笔 `(0,0) → (3, 6)` mm——长 6.708mm，63.43°
- 短杠 `(-1,-2) → (-0.7,-1)` mm——长 1.044mm，在原点左下方

### 3.2 字形不止一种

语料里三种活字形（连空的一共四种，名字见 §5）：

| 字形 | 两条线段（mm） | 出现 |
|---|---|---|
| 斜杠 | `(0,0)→(3,6)`、`(-1,-2)→(-0.7,-1)` | 五张图全有 |
| **X 叉** | `(-2.5,-2.5)→(2.5,2.5)`、`(-2.5,2.5)→(2.5,-2.5)` | 0202、工艺管道（`#FF0000`）|
| **对勾 ✓** | `(-2,2)→(0,0)`、`(0,0)→(5,5)` | 0202、工艺管道（`#008000`）|

工艺管道那 10 个绿点用的是**对勾**，不是斜杠。上一轮 OCS 给所有非黑点
一律画固定斜杠，这 10 个是错的。X 叉与 0202 的对勾没有任何**点**引用，
见 §4.1 与 §5。

> 本节最初把第三个字形写成「折线/勾」，是照着形状起的名。§5 拿到了
> 厂商自己的名字：它是 `psApproved`，一个**对勾**。

## 4. 屏幕上的放大倍数：结账（2026-08-25 重测后改写）

**结论没变，数变了，而且原因清楚了。** 这一节最初写的是「主笔量出
10.8–11.4mm，比值 ~1.7」——那是把**两笔当成一条长斜杠**量的结果。
按 11 个标记分 x/y 两轴做最小二乘标定（2.118 px/mm，残差 ≤0.3mm），
重量一次：

| | 屏幕 | 文件 | 比值 |
|---|---|---|---|
| 一个标记的包围盒 | 6.61 × 11.51 mm | 4.00 × 8.00 mm | x **1.65** / y **1.44** |
| 扣掉描边线宽后 | — | — | 大致 **1.3–1.45** |

**关键的旁证：线宽也被放大了。** 同一张图的立管文件写 0.70mm，屏幕上
稳定是 2px ≈ 0.94mm，**×1.35**。所以这不是点符号的毛病，而是
**整个视图对「样式声明的尺寸」统一乘了一个数**。
（`2px` 的测量精度只有 ±0.47mm，单拿它定不了数，只能说方向一致。）

### 为什么这个数不可能在文件里

上一轮的理由是「扫了全部 f64 没找到」。现在有更硬的：
`Interop.RAD2D.dll` 是 RAD2D 的 .NET 互操作程序集，带完整类型元数据，
把厂商自己的对象模型摊开看——**这条链上没有任何一个属性能装尺寸**：

| 对象 | 与尺寸有关的成员 |
|---|---|
| `PointStyle` | 只有 `StyleName` / `Notes` / **`Units`** / `Key` / `IsEditable` / `IsInternal` / **`Graphic`** / `Type`。**没有 size，没有 scale** |
| `Point2d` | `x` / `y` / `Style` / `LinearStyle` / `Layer` / `Visible` / `ZOrder`…… **没有 scale，也没有角度** |
| `LinearStyle` | 有 `Width`、`Units`、**`StartTerminator`/`EndTerminator`（类型正是 `PointStyle`）**，没有终止符尺寸 |
| `DimStyle` / `DimensionStyle` | `TerminatorSize`、`ViewTerminatorRatio` **只在这里** |
| `SheetSetup` | `ScaleOption` 是个枚举，1:1、1:2、1:50 这类整比，**没有 1.3–1.7 这种数** |

最后一行把上一轮那条「`rad2d.dat` 里的 `ViewTerminatorRatio` 挂在
`igDimStyleTerm*` 下、不能当证据」从**字符串位置的推测**升级成了
**元数据实证**：这两个属性确实只出现在尺寸标注样式上。

文件那一侧也对得上：`0x0032` 记录一共 **30 字节**，
`+0` oid、`+12` 一个标志、`+14` 样式 id、`+26` 组 oid，其余全零，
**物理上没有地方放尺寸**。

顺带一提，`PointStyle.Units` 的取值来自 `StyleUnitsConstants`
（`igPaperStyleUnits=11` / `igWorld=igDesign=12` / `igView=13` / `igDrawing=14`），
这是一个「样式尺寸按哪套单位解释」的真实机制，很可能就是那个倍数的出处——
但它**没有被序列化进这条 30 字节记录**，所以对我们不可读。

### 还想接着查的话

`style.dll` 把实际绘制交给 `render.dll`
（串 `RealRender2d` / `BuildRenderSolidLine` / `render.dll`），
而 `render.dll` 不在 `dlls/` 这批材料里，这台机器上也没装 SmartPlant。
`j2dsrv.dll` 已排除（那几个名字全是 `jmp ds:__imp_…` 转发桩）。
拿到 `render.dll` 才谈得上继续。

**落地取舍（不变）**：`pid-parse` 交出文件写的字形，`OpenCADStudio` 按 1:1 画。
知道了这些标记是**审核批注**（§5）之后，「查看器按自己的尺寸画」
就更说得通了——批注本来就不是图纸内容。

## 4.1 那个没人用的 X 叉：不是给线端的（2026-08-25 补）

`0x0033` 叫「**线**终止符」，类本身也确实带起点/终点两个槽，所以很自然要问：
那两个没被点引用的字形（`#FF0000` 的 X 叉、0202 的绿勾）是不是给线两端用的？
如果是，我们就漏画了线端符号。

**不是。** 把五张图每一条 `Sheet*` 记录、**每一个类型码**的 `+14` 与 `+25`
都拿去撞「能到活字形的样式 id」集合（含经 `JStyleOverride` 中转的），结果：

```
DWG-0201/Sheet6   活字形 [74]          ← 0x005E 引用 11 次
DWG-0202/Sheet6   活字形 [17,33,165]   ← 0x005E 引用  5 次
工艺管道/Sheet6    活字形 [58,62,67]    ← 0x005E 引用 11 次
D06/Sheet6        活字形 [26]          ← 0x005E 引用  1 次
A01/Sheet6        活字形 [28]          ← 0x005E 引用  2 次
```

**每一次引用都来自 `0x005E` `igPoint2d` 的 `+14`，没有第二个来源。**
没有任何 `igLine2d` / `igLineString2d` / `igSymbol2d` / `igBoundary2d` /
`igTextBox` 够到过活字形，**也没有任何 `JStyleOverride` 指向过活字形**
（五张图 via-override 全空）。

所以：

1. **X 叉（0202 id 33、工艺管道 id 67）与 0202 的对勾（id 165）
   定义了但零引用。**（这一条原本写的是「模板」，`§5` 给出了更准的读法：
   它们是四态审核状态里**这张图没有触发的那一档**，不是画库里剩下的颜料。）
2. **线端不画符号，我们没漏东西。** 这条渲染缺口不存在。

后来又补了一次同类普查（这次在 `StyleCluster` **内部**）：除了
`0x0033 +46` 之外，**没有任何记录引用点符号的样式 id**。
特别是 RAD2D 的 `Stroke` 对象能挂 `PointStyle`（带
`PointStyleAngle` / `XOffset` / `YOffset` / `PointStylePosition` /
`PointStyleOrientation`），也就是「沿线画符号」——语料里一条都没有。
唯一遍历到每个点符号 oid 的是 `0x005A` 样式库，那是目录，见 §5。

一个顺带说得通的读法（**是读法，不是证明**）：
`JStyleLineTerminator` 的第一个槽（起点终止符）在语料里恒为零，
只有第二个槽被填——点只有一个「端」，没有起点终点之分。
点大概就是走这套线端机制的退化情形，这同时解释了两件事：
为什么点会去查一个名字里带「线」的样式，以及为什么只有一半的槽在用。

## 5. 这些标记是审核状态：`psOk` / `psWarning` / `psError` / `psApproved`（2026-08-25 补）

前面几节把「画什么形状」讲完了，但没回答「这形状**是什么意思**」。
答案一直躺在每个 `StyleCluster` 的第一条记录里。

### `0x005A` 是样式库，它存着每个样式的作者命名

`tools/psm_type_clsid.py` 认得它：`0x005A` = `{9196D9D1-E94A-11CF-8094-080036CE6C02}`
= **JSL Style Librarian**（`style.dll`）。本仓一直把它列在
`undecoded_census.rs` 里，也在 `style_link.rs` 的
`STYLE_FAMILY_TYPE_CODES` 注释里点名排除过（它的 `+14` 不是这个空间的 id）。

**绑定规则：名字是 UTF-16、既无长度前缀也无结束符，所以按「文字在哪断」定位——
文字末尾往后 8 字节处的那个 `u32` 就是被命名对象的 `oid`。**

规则的证据不是「能出名字」，而是**能分得开**：五张图一共绑中 210 条，
每一条都落在该流真实定义的记录上，而且 `ps…` **无一例外**落在
`0x0032 JStylePointSymbol` 上、`ls…` **无一例外**落在
`0x002E JStyleSimpleLine` 上。一个只是「看着合理」的间距不会把两族分开。

### 四个名字

| 名字 | 字形 | 0201 | 0202 | 工艺管道 | D06 | A01 |
|---|---|---|---|---|---|---|
| `psOk` | **空**（两条零长线） | 53 | 22 | 23 | 9 | 1 |
| `psWarning` | 斜杠 | **11** | **5** | **1** | **1** | **2** |
| `psError` | X 叉 | — | 定义，0 引用 | 定义，0 引用 | — | — |
| `psApproved` | 对勾 ✓ | — | 定义，0 引用 | **10** | — | — |

一口气解释了三件之前各自挂着的事：

1. **`psOk` 的字形为什么是两条退化线。** 不是坏数据也不是哨兵——
   **通过的东西本来就不该画任何东西**。厂商用一个空字形表达「没问题」，
   而 `decode_iglines` 早就拒绝零长度线，两边天然一口径。
2. **那个「没人引用的 X 叉」不是模板。** 它是 `psError`，四态里的第三态，
   **定义着但这张图没有任何东西出错**。
3. **工艺管道那 10 个绿色的不是「折线」，是对勾。** `psApproved`。

### 状态还有一张「线」的脸

每个 `ps*` 都配一个同名的 `ls*` **线**样式（`lsOk` / `lsWarning` /
`lsError` / `lsApproved`，全部落在 `0x002E` 上）。所以
**颜色和字形是同一个状态枚举的两张脸**：线用颜色表达状态，点用符号表达。
这正是为什么上一任那条「按颜色判断画不画」的规则能蒙对大半却做不到全对。

而且整条链就是厂商 API 的原样，不是我们凑出来的偏移巧合：

```text
Point2d.LinearStyle  →  LinearStyle.EndTerminator（类型就是 PointStyle）
                     →  PointStyle.Graphic  →  Group  →  两条 Line2d
```

`LinearStyle` 上那两个槽的官名就叫 `StartTerminator` / `EndTerminator`，
跟 §1 里反编译看到的对象 `+88`/`+92` 两个引用槽一一对应。

### 顺带出来的：全部线样式的专业名

同一张名表还把每条线样式叫什么给了出来——
`Primary Piping - New`、`Secondary Piping - New`、`Equipment - New`、
`Nozzle - New`、`Piping Component - New`、`In-Line Instrument - New`、
`Off-Line Instrument`、`Electric Signal`、`Electric`、`Piping OPC`、
`Connect To Process`、`Construction Status`、`As Drawn`……
这是现成的语义层输入，价值可能比点符号本身还大，另开一轮处理。

## 6. 已落地

`pid-parse`：

- `MarkerStroke` / `PointMarker`，`ResolvedLineStyle` 增加 `marker`。
- `DocumentStyleTable` 增加 **oid 键**的组与线对象索引。必须按 oid：
  这些记录的 `+14` 带的是**所属线样式的 id**（0201 里两条 `igLine2d`
  加另两条记录同读 id 71），进样式索引会互相遮蔽。
- `+54` 认了第二种落点：命中 `0x002F` 是虚线型，命中 `0x0033` 是线终止符。
  同一个槽、按落到哪一类分流，正是本模块一贯的判据方式；语料里没有
  任何一条线样式同时够到两边。
- 链路断在任何一环都返回 `None`，不交半个字形——缺一笔的字形是另一个形状。
- 测试：`parse_real_files` 两条（四图三态计数、字形读数），
  `style_link` 单测五条（活字形、空字形、`+54` 分流、断链、id 遮蔽）。

`OpenCADStudio`：

- `build_entities` 的 Point 分支改由字形驱动，删掉
  `POINT_TICK_LENGTH_MM` / `POINT_TICK_ANGLE_DEG` 两个屏幕标定常数。
- `a_point_draws_the_symbol_its_terminator_names` 钉住 11 个点各画两笔、
  长度分别 6.708mm 与 1.044mm；退回颜色门或固定斜杠都会红。

### 6.1 §5 那一轮追加的（2026-08-25 晚）

`pid-parse`：

- 解码 `0x005A JStyleLibrarian`。`StyleRecord` 增加 `oid` 与 `name`，
  `DocumentStyleTable` 增加 `name_of_style()`。样式库排在链首、
  名字比被命名的记录先到，所以先暂存、走完链再绑。
- **唯一的过滤是「oid 必须是这个流真的定义过的」**。一段碎文本想变成样式名，
  得恰好在 +8 处撞上一个真实存在的 oid——全语料一次都没撞上。
- `MarkerStatus { Ok, Warning, Error, Approved }`，`PointMarker` 增加
  `status`。**不设 `Other(String)` 兜底变体**：不在四态里的名字返回 `None`，
  名字本人从 `name_of_style()` 取——这样 `PointMarker` 保持 `Copy`。
- 测试：`parse_real_files` 增一条（四图按状态计数，外加不变式
  **「只有 `psOk` 是空的」**），`style_link_ratchet` 增一条
  （全语料 92 个名字、`ps…`/`ls…` 不串家族、八个状态名与七个专业名钉死），
  `style_link` 单测增四条。

`OpenCADStudio`：

- 标记按状态分层：`PID-POINT-WARNING` / `PID-POINT-ERROR` /
  `PID-POINT-APPROVED`。`psOk` 什么都不画所以没有对应层；
  状态未命名的标记留在 `PID-POINT`——替图纸定一个状态不是导入器的事。
- `PID-POINT-ERROR` 全语料为空，**这正是声明它的理由**：
  定义了这一态的图纸都定义全了而没有东西在这一态里，
  一个存在且为空的层能说出这件事，缺席的层不能。
- `apply_symbology` 的层门从三个等值判断改成前缀判断，
  否则标记搬层之后就丢了颜色和线宽。

## 7. 复现

```powershell
cd pid-parse
cargo test --test parse_real_files a_point
cargo test --lib style_link
cargo test --test style_link_ratchet

cd ..\OpenCADStudio
cargo test --test pid_import a_point
```
