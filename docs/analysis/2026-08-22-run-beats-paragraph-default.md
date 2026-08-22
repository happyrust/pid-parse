# 定谳：run 赢。段落样式的 `CharStyle` 是默认值，不是生效值

> 日期：2026-08-22
> 阶段：run-vs-para 定谳（**只定谳与量化，本轮不接线**）
> 证据等级：**厂商对象模型**（`Interop.RAD2D.dll`，Intergraph 自己的 interop 程序集）
> ＋ **native-reader**（`radsrvitem.dll` 的 run 覆盖不变式）＋ **corpus**（点值吻合）
> 产物：`examples/probe_igtextbox_run_vs_para`、`tools/idalib_igtextbox_run_consumers.py`

## TL;DR

**结论：run 优先。** 一条 `igTextBox` 只要带 run，它的每一个字符都被 run 盖住，
段落样式的字符样式**根本到不了字形**——它是「默认值」，只在没有 run 的地方生效。

代价是实测出来的：`tests/style_link_ratchet.rs` 钉住的那 **155 条**里，

| | 条数 |
|---|---:|
| 带可解析的 run | 145 |
| 无 run，段落默认确实生效 | 10 |
| 两条路一模一样 | 39 |
| **改成 run 优先后屏幕上会变** | **106** |
| ├ 字高变 | 103（其中 99 条肉眼可见） |
| ├ 字体变 | 54 |
| └ 颜色变 | 2 |

两张主图：`DWG-0201GP06-01` **58 条里 30 条会变**（29 字高、12 字体、1 颜色）；
`DWG-0202GP06-01` **52 条里 33 条会变**（31 字高、13 字体、1 颜色）。

## 一、为什么这不是推断

t-42 结束时这条只到「推断」：读序证明 run 被逐区间应用，但没读到渲染端的取舍规则。
本轮补上的是**厂商自己的对象模型**。

`dlls/Interop.RAD2D.dll` 是 Intergraph 为 RAD2D 出的 interop 程序集，反射即可读：

```text
TextStyle                       ← 段落样式，就是 JStyleTextPara
    .CharStyle    : TextCharStyle       ⎫ 这就是我们走的 +38 那一跳
    .CharStyleName: String              ⎭ 名字直说了：段落的「字符样式」
    .Alignment, .LineSpacing, .BeforeSpacing, .AfterSpacing, .Tabs

TextBox
    .Style : TextStyle                  ← 整框一条段落样式
    .Edit  : TextEdit                   ← 逐区间的排字面

TextEdit                        ← 区间级，不是整框级
    SetSelect(start, end, …) / GetSelect(out start, out end)
    .TextSize / .Color / .Font / .Bold / .Italic / .UnderLine
    .SubScript / .SuperScript / .CharacterSpacing / .Language
    IsPropertyShared(TextPropertyConstants, out Boolean)
```

三点定死了语义：

1. **`TextStyle.CharStyle` 自称是段落的字符样式**——即「默认」。我们两跳走的正是它。
2. **逐字符属性挂在 `TextBox.Edit` 上，而且是选区式的**：先 `SetSelect(起, 止)`，
   再设 `TextSize` / `Color` / `Font`。磁盘上的 run `(长度, 选择子, 样式id)` 就是
   这些选区，一一对应。
3. **`IsPropertyShared(属性, out bool)` 这个 API 只有在「选区内可能与默认不一致」
   时才需要存在**。它问的就是「这一段的这个属性是不是统一的」。如果段落默认永远
   赢，这个方法没有意义。

配套的 native-reader 事实（t-42 已坐实，这里只是接上）：
`IGDSFactoryText::GetSize`（`sub_56498AB0`）**拒收选择子 1 的长度之和不等于字符数
的记录**。也就是说，**只要一条记录有 run，它的每个字符都在某条 run 里**，没有
「run 覆盖不到、退回段落默认」的缝隙。两件事合起来就是结论：run 盖住全文 →
段落默认到不了字形。

**外加一条旁证**：`TextCharStyle` 有 `BaseStyleName`——字符样式本身是可以「基于
某个基样式再改」的。派生样式的存在正是「默认 + 覆盖」这套模型的标配。

## 二、语料侧的独立佐证：run 的字高是排版点值

这一条是本轮意外收获，也是最直观的一条。把两侧字高换算成磅（1 pt = 25.4/72 mm）：

```text
   5 x 1.5000mm (   -) -> 1.5875mm (4.5pt)
  20 x 2.5000mm (   -) -> 2.4694mm (  7pt)
   1 x 3.1750mm ( 9pt) -> 1.7639mm (  5pt)
   9 x 3.1750mm ( 9pt) -> 2.2931mm (6.5pt)
  36 x 3.1750mm ( 9pt) -> 2.4694mm (  7pt)
   2 x 3.1750mm ( 9pt) -> 2.6458mm (7.5pt)
  19 x 3.1750mm ( 9pt) -> 2.8222mm (  8pt)
   4 x 3.1750mm ( 9pt) -> 3.1750mm (  9pt)   ← 字高只差浮点噪声，变的是字体
   1 x 3.1750mm ( 9pt) -> 3.7042mm (10.5pt)
   1 x 3.1750mm ( 9pt) -> 4.2333mm ( 12pt)
   5 x 3.5000mm (   -) -> 3.5278mm ( 10pt)
```

**run 侧 11 个取值无一例外落在整半磅上**（4.5 / 5 / 6.5 / 7 / 7.5 / 8 / 9 / 10 /
10.5 / 12 pt）；段落侧的 `1.5mm`、`2.5mm`、`3.5mm` **都不是磅值**，是圆整的公制
制图尺寸（`3.175mm` 恰好同时是 1/8″ 与 9pt，所以两边都能落）。

这正是「谁是人设的、谁是标称的」的分水岭：**磅是文本编辑器里设字号的单位**
（`TextEdit.TextSize`），毫米整数是制图标准里的标称字高。run 侧带的是这张图**实际
排出来的字号**，段落侧带的是**样式表上的默认字号**。

## 三、一条需要说清楚的旧账

`tests/style_link_ratchet.rs` 现在的注释写着：「3.175mm 是 1/8 英寸，也是最常见的
尺寸，所以渲染器原来固定的 2.5mm 对一张图的大部分字来说小了 27%」。

按本轮的结论，这句话对其中一大块是**反的**：那 36 条 `3.1750mm → 2.4694mm` 的记录，
真实字号是 7pt = 2.4694mm。

- 老的固定 `2.5mm`：偏大 **1.2%**；
- 08-13 换成两跳读到的 `3.175mm`：偏大 **28.6%**。

也就是说，对这批记录而言 08-13 那次改动**让字高离真值更远了**。这不推翻 08-13
的工作——在那之前根本没有任何字高来源，而且它对没有 run 的那 10 条、以及两条路
一致的那 39 条都是对的——但「解出真字高」这个说法要收窄成「解出了段落默认字高」。
接线那一轮应当同时改掉那条注释与它的断言。

## 四、量化：改成 run 优先，屏幕上变多少

`examples/probe_igtextbox_run_vs_para` 用**库自己的 `resolve_text_height`** 算两边
——它本来就支持两种形状：传段落 id 就走 `+38` 那一跳，传字符样式 id 就直接读那条
记录——所以对照的是解码器的真实行为，不是我另写一遍。索引也按
`text_heights_for_file` 的 `(stream, oid)` 键去重，**条目数正好复现 155**，这既是
口径对齐的验证，也让「155 里有多少条」这个问句有分母。

| 口径 | 条目 | 带 run | 无 run | 一致 | **会变** | 字高 | 字体 | 颜色 | run 内部打架 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| ratchet 四图（155） | 155 | 145 | 10 | 39 | **106** | 103 | 54 | 2 | 11 |
| 全六个 fixture | 209 | 195 | 14 | 54 | **141** | 136 | 68 | 3 | 12 |
| `DWG-0201GP06-01` | 58 | 52 | 6 | 22 | **30** | 29 | 12 | 1 | 0 |
| `DWG-0202GP06-01` | 52 | 48 | 4 | 15 | **33** | 31 | 13 | 1 | 1 |
| `工艺管道及仪表流程-1` | 41 | 41 | 0 | 2 | **39** | 39 | 27 | 0 | 10 |
| `D06` | 4 | 4 | 0 | 0 | **4** | 4 | 2 | 0 | 0 |

字体的迁移集中在三对：

```text
  40 x "Arial"       -> "Arial Narrow"
  11 x "Braggadocio" -> "Arial Narrow"
   3 x "Arial"       -> "仿宋_GB2312"
```

`Braggadocio` 是一款极粗的展示体——11 条标签现在按它渲染，而记录自己说的是
`Arial Narrow`。这类错比字号错更显眼。

**`run 名了一条本文档没定义的样式`：0 条。**所以改口径不会新增拒收。

## 五、接线时的三个坑（下一轮的输入，本轮不做）

1. **对齐与行距必须继续走段落那一跳。** 它们是段落属性，`ResolvedTextHeight` 的
   `alignment` / `line_spacing` 在字符样式那条路上恒为 `None`。**145 条**会因为
   「只是把 resolver 换个入参」而丢掉已经接好的对齐——那是 08-13 刚修好的位置错，
   会原地退回去。正确做法是**两边都取**：字高／颜色／字体取 run，对齐／行距取段落。
2. **11 条标签的 run 彼此不一致**（`工艺管道及仪表流程-1` 占 10 条），单一
   `ResolvedTextHeight` 表示不了。建议取**覆盖字符数最多**的那条 run，并把这次
   压平显式登记，而不是悄悄取第一条。
3. **无 run 的 10 条继续用段落默认**——这不是回退，这正是厂商模型里
   `TextStyle.CharStyle` 该生效的地方。两跳链不能删。

## 六、没能做到的

**没有读到真正的字形栅格化代码。** `tools/idalib_igtextbox_run_consumers.py` 把
文本 sink 的 22 个槽全部反编译了一遍：只有**写入方**
（`SetText` / 追加 run / 复位 / 取放置对象）以及序列化用的 `GetSize` / `Save`，
**`radsrvitem.dll` 里没有任何一个槽把 run 数组读出去**。这个 DLL 只负责持久化，
渲染在别处（我们手上这批 DLL 里没有明确的入口）。

所以本轮的定谳建立在**厂商对象模型 + 原生覆盖不变式 + 语料点值**三条互相独立的
证据上，**不是**建立在读到了渲染器。按本仓的分级，这条记 **厂商对象模型级**，比
corpus 强、比 native-reader 弱一档——但三条证据指向同一个结论，且没有任何一条
反向证据。

## 七、复现

```powershell
cargo run --example probe_igtextbox_run_vs_para
python tools/idalib_igtextbox_run_consumers.py
```

厂商对象模型可以这样自己看一遍（不需要 IDA）：

```powershell
$asm = [Reflection.Assembly]::LoadFrom("dlls\Interop.RAD2D.dll")
$asm.GetTypes() | ? { $_.Name -in 'TextStyle','TextEdit','TextCharStyle' } |
  % { $_.GetMembers() | % { $_.ToString() } }
```

探针沿用 t-42 的规矩：**fixture 打不开就报 UNREAD、整轮标 PARTIAL**，不把短缺的
语料当完整的报。
