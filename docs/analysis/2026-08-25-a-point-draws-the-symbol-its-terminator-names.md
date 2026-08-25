# 点画不画，由它的线终止符说了算：`0x002E → 0x0033 → 0x0032 → 0x007B → 0x0018`

> 日期：2026-08-25
> 范围：`pid-parse` + `OpenCADStudio`
> 结论类型：**native-reader（类身份与链路语义）+ corpus（引用偏移，五图穷尽）**
> 含**三处推翻**：判别码是颜色、斜杠是一笔 15.24mm、字形只有一种。
> 前置：`2026-08-24-placement-names-the-body-style.md` §4「哪些点画、哪些点不画：
> 判别码还没找到，先按观察落地」——本文关掉这一条。

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

语料里三种，按专业分：

| 字形 | 两条线段（mm） | 出现 |
|---|---|---|
| 斜杠 | `(0,0)→(3,6)`、`(-1,-2)→(-0.7,-1)` | 五张图全有 |
| **X 叉** | `(-2.5,-2.5)→(2.5,2.5)`、`(-2.5,2.5)→(2.5,-2.5)` | 0202、工艺管道（`#FF0000`）|
| **折线/勾** | `(-2,2)→(0,0)`、`(0,0)→(5,5)` | 0202、工艺管道（`#008000`）|

工艺管道那 10 个绿点用的是**勾**，不是斜杠。上一轮 OCS 给所有非黑点
一律画固定斜杠，这 10 个是错的。

## 4. 仍然开放：屏幕上的放大倍数不在文件里

拿 11 个点（跨度 212mm）重拟比例尺，残差 ≤0.6mm。在这个尺度下：

- 主笔量出来 **10.8–11.4mm**，文件写的是 **6.708mm**，比值 **~1.7**；
- 点落在主笔**中间偏下**，不在它的原点。

把该 `.pid` 全部流的每一个 f64 扫了一遍（`1.40..1.80` ∪ `0.55..0.72`），
**没有这个数**（命中的 `1.5708` 全是 π/2 旋转角）。所以
**放大倍数与锚点约定都是查看器的，不在文件里**。

往下追也到头了：`style.dll` 把实际绘制交给 `render.dll`
（串 `RealRender2d` / `BuildRenderSolidLine` / `render.dll`），
而 `render.dll` 不在 `dlls/` 这批逆向材料里。`rad2d.dat` 里确实有
`ViewTerminatorRatio`（「视图终止符比率」）和 `TerminatorSize`，
终止符形状枚举里也确实有 `Terminators Slash` / `Terminators Back Slash` /
`Terminators Blank`——**但那一套挂在 `igDimStyleTerm*` 尺寸标注样式下**，
是标注箭头那条线，不是 `JStyleLineTerminator`。词汇同源，不能当证据用。

**落地取舍**：`pid-parse` 交出文件写的字形，`OpenCADStudio` 按 1:1 画。
一张截图定不了这个倍数，就不编一个看着像真的数写进代码。

## 5. 已落地

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

## 6. 复现

```powershell
cd pid-parse
cargo test --test parse_real_files a_point
cargo test --lib style_link
cargo test --test style_link_ratchet

cd ..\OpenCADStudio
cargo test --test pid_import a_point_draws
```
