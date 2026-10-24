# 一张 84 笔画的符号被滤镜留成 1 条：`Sheet6` 兜底选错了表（2026-10-22）

上一轮把符号库和图纸自带的本体逐条比过之后，`Wellhead Christmas Tree 02.sym` 是最刺眼的一条：
图纸自带的本体有 45 条线、23 个圆、1 条 B 样条、11 个填充，而库本体经
`SymbolGeometry::displayed_primitives()` 只剩 **1 条曲线**。这一轮把原因钉死：不是图层读错、
不是视图集位读反，而是**在「没有正常视图」时兜底挑了 `Sheet6`**，而这份文件的 `Sheet6` 只是
一个一笔的存根。

## 一、这份文件自己的说法

`probe_sym_layer_filter` 把图层表、视图集和每条笔画的去留逐行打出来：

```
84 primitives, 10 layers, 1 views

layer table (oid, number, name):
  oid=8    number=0  name="Default"
  oid=339  number=4  name="Jacket"
  oid=340  number=10 name="HiddenObjects"
  …
view sets:
  Sheet374: 2 bytes, 16 bits set [0..15], supplementary=true,
            listed=[(1,"Label"),(3,"HiddenObjects"),(2,"Heat Trace"),(0,"Default"),(4,"Jacket")]
selected sheets: [6]

per layer:
  Sheet6   layer oid=8   number=0 name="Default": 1 stroke {"bspline":1}, view=no answer, shown=1
  Sheet374 layer oid=372 number=0 name="Default": 83 strokes
           {"boundary":11,"bspline":1,"circle":23,"line":48}, view=on, shown=0

shown 1 of 84: dropped by sheet 83 / view-off 0 / hidden name 0
```

三件事同时成立：

1. 整份文件只有 **一个** 视图集，管的是 `Sheet374`，位图 `FF FF` —— **16 位全开**（连 `Label`、
   `Heat Trace`、`Jacket` 都开）。
2. `shows_supplementary()` 因此把它认成「备用视图」，于是 `normal` 集合为空。
3. `displayed_primitives()` 在 `normal` 为空时的第一兜底是 `Sheet6` —— 而 `Sheet6` 里只有 1 条
   B 样条；真正的主体 83 条笔画全被留下。

`view-off 0 / hidden name 0` 说明图层规则一条都没杀，**全部损失都来自选表**。

## 二、谁对：图纸自带的缓存体和厂商的出图

- 13 张 `井场` 图里都放了这棵树，缓存体一律是 45 线 + 23 圆 + 1 B 样条 + 11 填充。
- `I井场` 是五张参考图之一（对厂商 PDF 覆盖率 **0.9989**），纸上画的就是缓存体。

所以「83 条笔画那面」才是 `SmartPlant` 画的东西；`Sheet6` 在这里是存根，不是主体。

## 三、这不是孤例：1262 个符号里 530 个受影响

`probe_sym_sheet_choice` 扫两个库根（语料 `Ref/Symbols` 644 + `test-file/symbols-full` 618，
去重 1262）：

| 选表分支 | 符号数 |
|---|---:|
| 正常视图（无风险） | 858 |
| **`Sheet6` 兜底** | **314** |
| 备用视图补集 | 41 |
| 全都画 | 38 |

按「选中的表笔刷数 < 另一张表的笔刷数」计，**530 个本体被选少了**，最重的就是
`Wellhead Christmas Tree 01/02`（1/83）、`Opening Action`（1/71）、`Coating Requirements`
（1/55）、`Nominal Diameter Break`（1/35）。

## 四、用缓存体给候选规则打分

`probe_which_sheet_the_cache_matches` 拿 53 张图里 1828 个「两边都有本体」的放置当答案键：
对每个 `.sym` 的每张表按它自己的视图集 + 隐藏图层名算一遍笔画数，看哪张表和缓存体逐类相等。

| 规则 | 1636 个「至少有一张表对得上」的放置里命中 |
|---|---:|
| 现行（正常视图 → `Sheet6` → …） | 933（57.0 %） |
| 无正常视图时先取「被视图集管的表」 | 994（60.8 %） |
| 一直取笔画最多的表 | 1484（90.7 %） |
| **正常视图优先，没有时取笔画最多的表** | **1172（71.6 %）** |

再按形状拆开看，`Sheet6` 兜底这一支的问题最清楚：

| 形状 | 现行 | 取笔画最多的表 |
|---|---:|---:|
| 没有正常视图（372 个放置） | 133（35.8 %） | **372（100 %）** |
| 有正常视图（1264 个放置） | 800 | 800（不变） |

「一直取最多的表」在有正常视图的那支把命中率抬到 88 %，但那等于废掉「备用视图」这套已经验证过
的规则（绝缘/伴热视图就是靠它躲开的），证据不够，**没有采纳**。采纳的是最窄的一刀：正常视图
规则原样保留；**只有 `normal` 为空时**，改取「按自己的视图集算下来笔画最多的那张表」。

## 五、改了什么

`SymbolGeometry::displayed_primitives` 的选表：

```rust
let selected = if !normal.is_empty() {
    normal
} else if let Some(sheet) = fattest {   // 新增：按各自的视图集算笔画数，最多者胜
    [sheet].into_iter().collect()
} else if sheets_present.contains(&6) { // 原来的兜底，保留给「一张表都没有笔画」的文件
    …
```

平局取表号小的，确定性不变；一张有笔画的表都没有时仍走原来的兜底链，`visible.is_empty()`
时整份返回的护栏也原样保留。

单测两条：

- `a_sole_all_on_view_does_not_shrink_a_body_to_its_sheet6_stub` —— 合成一份
  `Sheet6` 1 条 + `Sheet374` 2 条、只有 `Sheet374` 有一个全开视图集的本体，断言画出 2 条。
- `a_wellhead_tree_keeps_its_body_off_the_single_stroke_stub` —— 真文件（语料软跳过），
  断言 `displayed_primitives().len() >= 40`。

## 六、改前改后的数

同一批 1828 个放置、同一套比法（`probe_library_vs_cached_bodies`）：

| | 改前 | 改后 |
|---|---:|---:|
| 库本体与缓存体完全一致 | 1069（58.5 %） | **1281（70.1 %）** |
| 不一致 | 709（38.8 %） | **497（27.2 %）** |
| 被滤镜丢掉的笔画 | 5356 / 15316（35 %） | **4206（27.5 %）** |
| 丢掉一半以上的本体 | 411 | **343** |
| `Wellhead Christmas Tree 02` 的库本体 | 1 条曲线 | **48 线 + 23 圆 + 1 B 样条 + 11 填充** |

**页面一个像素没动**：53 张图上每个放置都自带缓存体，`library_only_placements` 始终是 0，
`lay_out_drawing` 走的一直是缓存那条路。这一刀修的是「库当后备时会画成什么样」，不是今天的纸面；
五张参考图的覆盖率 / 多画 / 文字仍是 0.9989 / 0.9998 / 0.9979 / 0.9986 / 0.9993、
41.8 / 7.6 / 1.5 / 22.9 / 24.6 mm、544/546 + 151/151 + 123/123 + 167/167 + 249/249。

## 七、仍未闭合

- **还剩 497 处不一致**（27 %）：多数差 1–3 条线、或尺寸差 1.27 mm 一类，是库与图内本体的
  **修订差**（同一符号不同版本的 `.sym`），不是解码差。要分清得拿到符号的修订记录。
- **有正常视图时「最多笔画」比「正常视图」更贴缓存**（88 % 对 63 %）：这条线索指向「正常视图」
  的判据（是否显示任何隐藏家族的层）可能过宽，但今天只有一条证据链，没动。
- 三个探针都只在有语料/备份的机器上才有意义：`probe_sym_layer_filter`、
  `probe_sym_sheet_choice`、`probe_which_sheet_the_cache_matches`，仓库里没有 fixture 版。
