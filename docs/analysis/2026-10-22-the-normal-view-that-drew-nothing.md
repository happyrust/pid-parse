# 正常视图并不是判宽了，而是判空了：库侧选表的第二层（2026-10-22）

上一轮把「没有正常视图」时兜底选 `Sheet6` 改成了「取笔画最多的表」，一致率
58.5 % → 70.1 %。但那次打分里还有一条反向的读数没解释：**在有正常视图的放置上，
「笔画最多的表」命中 88.1 %，按正常视图选只有 65.6 %**。一个已经被验证过的规则
（正常视图 = 视图集不显示隐藏名的表）在自己最该赢的地方输了，只有两种可能：位图
判据把人认错了，或者判据本身没判到「本体在不在这张表上」。这一轮就是去分清这两件事，
结论是后者，而且只用「这张表自己画不画得出笔画」一条就能修好。

## 一、先把输的那 22 处摊开

新探针 `examples/probe_normal_view_too_wide`：对每个「图内缓存体 + 库里有本体」的
放置，先要求某个表在**自己视图集 + 隐藏名规则**下逐类复现缓存体（否则这个放置判不了），
再把「正常视图选」和「笔画最多选」摆在一起看缓存站哪边。

- 230 个放置，27 个没有任何表能复现缓存体（符号修订过，判不了），**203 个可判**；
- 其中 **160 个有正常视图**；
- 两者选择不同的、且缓存站在「笔画最多」那边的：**22 个**。

22 个分三类，而三类的winner都是一样的：

| 类别 | 个数 | 正常视图那张表 |
|---|---:|---|
| 空转（hollow）：视图集把它名下**每一条**笔画都关掉 | 11 | 0 条 |
| 存根（stub）：留着几条，离本体差一个数量级 | 7 | 1–4 条 |
| 本体但在别的表（body） | 4 | 4 条，本体在无视图集的 `Sheet6` 上 6 条 |

赢家那边：**11 个根本没有视图集**，**11 个的视图集是全开那张**（隐藏名全开）。
没有一个是「正确视图集被误判成备用」——所以位图判据没认错人。

## 二、用位和图层表把三例钉死

`--detail <文件名>` 会把一个 `.sym` 的每张表、每个图层号、位值、隐藏名判定和每条
笔画都打出来。三个例子各说明一件事。

### `Insulation Joint.sym`：正常视图是**空转**的

```
Sheet6 set:   16 bits, listed [Label/HiddenObjects/Heat Trace/Default/Jacket], shown [Default]
   -> classified normal (no hidden-named layer is on)
Sheet6:  1 primitives raw
   layer Heat Trace   number   7  bit off  hidden-name yes  1 stroke (bit on: 0, off by name: 1)
   => draws <nothing> (0 strokes)
Sheet182: 16 primitives raw   (no set at all)
   layer Default      number   0  bit on   hidden-name no  16 strokes
   => draws 16 line
```

缓存体是 16 条线 = `Sheet182`。`Sheet6` 上那条线在 `Heat Trace` 层、号 7：位是 0
（该表的位图只有 `Default` 亮），名字又在隐藏名表里 —— **两条规则各自独立地否掉它**。
于是「正常视图选」选出一张一条笔画都画不出的表；`displayed_primitives` 的
「选中集合里什么都没画出来 → 退回整个本体」兜底接手，把 `Sheet182` 的 16 条 **加上**
`Sheet6` 那条被隐藏的线，画出 **17 条**。全量打分里 `shipped` 与缓存的差正是这种多画。

### `Concentric Reducer.sym`：全开的那张不是「备用表示」，是**修过版的本体**

```
Sheet6  set: 16 bits, shown [Default]
Sheet39 set: 16 bits, shown [Default/Heat Trace/HiddenObjects/Jacket/Label]   -> ALTERNATE
Sheet6:  2 raw   1 on Default(0) + 1 on Heat Trace(7, bit off, hidden name)  => draws 1 line
Sheet39: 3 raw   3 on Default(0)                                             => draws 3 line
layer table: [(0,"Default"),(8,"Label"),(7,"Heat Trace"),(0,"Default"),(1,"Label"),(2,"Heat Trace"),
              (4,"Jacket"),(9,"HiddenObjects"),(10,"Jacket"),(3,"HiddenObjects")]
```

缓存体 3 条。`Sheet39` 的视图集全开，被归成「备用（保温/伴热）视图」，于是被拒；
但它的 3 条笔画**全在 `Default`（号 0）**，隐藏名层一条都没有 —— 那张全开位图上
多出来的 4 个亮位，底下**没有笔画**。也就是说：这个文件的第二张表不是另一种表示，
而是同一个本体的修订/拷贝，位图全开只是文件自己的写法。判据把「位亮」当成了
「那一层有东西」，这是第二处判宽。

### `Arrestor Type 1.sym`：带干净视图集的是一份**残本**

```
Sheet97 set: 16 bits, shown [Default]                    -> normal, 4 lines on Default
Sheet6 : no set at all                                   -> 6 lines on Default
```

缓存体 6 条。正常视图管着 4 条那份拷贝，本体在**根本没有视图集**的 `Sheet6` 上。
这一例说明干净视图集甚至不代表「这张表是主体」——它可能只是一份更旧/更简的拷贝。

## 三、把判据的失效量出来

只看三个例子会以为是特例。把 1262 个库符号全扫一遍（`--library-census`）：

```
1262 library symbols read, 1120 carry a view set
   868 have a clean (non-supplementary) set
   163 of those govern a sheet with no primitives at all
   431 draw nothing once their own set has had its say
   402 are thinner than the fattest sheet
   418 all-on ("alternate") sheets hold strokes, and only 3 of them keep any
       geometry on a hidden-named layer (41 keep some ink there, but it is
       `Label` lettering, not jacket or tracing)
```

**868 个「有正常视图」的符号里，431 个（49.7 %）那张表在自己位图下一条笔画都画不出**；
163 个连管的那张表都不存在（视图集指向没有笔画的表）。放到放置层面：

```
203 scored placements, 160 with a normal view
   22 of those govern a sheet with no primitives
   41 of those normal sheets draw nothing at all
    0 pick more than one normal sheet
```

所以「判宽了」的准确含义是：判据的**存在性**太便宜——只要文件里有一张表挂着
「只有 `Default` 亮」的位图，就认为正常视图存在，而不管那张表是否真托着本体。
它没有把本体表误标成备用（这一类 0 例）；它只是把**空壳**当成了正常视图。

## 四、规则打分与结论

同一批 203 个可判放置，逐条与缓存体全等比较：

| 规则 | 命中 |
|---|---:|
| 修前 `displayed_primitives`（正常视图 → 全体兜底） | 178（87.7 %） |
| 正常视图选（不计兜底） | 178（87.7 %） |
| 笔画最多的表 | 184（90.6 %） |
| 更瘦就落到最多（不够松） | 184（90.6 %） |
| 存根就落到最多 | 181（89.2 %） |
| **空转才落到最多（采纳）** | **189（93.1 %）** |

只有「**空转**」这一条是可信的判别式：一张连自己名下笔画都关掉的表，不可能是本体
所在。而「更瘦」「存根」都不能用——合法备用视图（保温/伴热）本来就比正常视图更肥，
按薄厚判会把它们抢过来；实测这两条也确实低于只按空转判（184 / 181 对 189）。

## 五、改了什么

`SymbolGeometry::displayed_primitives` 的选表顺序里，正常视图那一支加了「至少画出
一条笔画」的门槛：

```rust
let drawing: BTreeSet<u32> = normal
    .iter()
    .copied()
    .filter(|sheet| self.primitives.iter().any(|s| s.sheet_ref == *sheet && layer_shown(&s)))
    .collect();
let selected = if !drawing.is_empty() { drawing } else if let Some(sheet) = fattest { … };
```

修后 `shipped` 一列从 178 升到 **189**，与 `fattest` 的 184 相比多出的部分正是空转那
11 处；没有一处从对变错（空转那次兜底原本要么等价于「最多」，要么多画一条隐藏笔画）。

同一改动放到 53 张整包备份的 1828 对本体上看（`pid-plot/examples/probe_library_vs_cached_bodies`）：

| 读数 | 修前 | 修后 |
|---|---:|---:|
| 一致 | 1281（70.1 %） | **1411（77.2 %）** |
| 不一致 | 497 | **367** |
| 过半被滤 | 411 | **343** |
| 只能靠库的放置 | 0 | 0 |
| 被滤镜丢的笔画 | 4206 | 4441 |

最后一列变大不是退步，而是量到了真东西：旧兜底在「空转」时把整份本体（连同隐藏层）
一起交出去，被丢的笔画里那些**根本没被丢**；现在它们按规则被关掉，`Insulation Joint`
从「17 条里 0 条被丢」变成「17 条里 1 条被丢」，画面才是 16 条。
另加两条单测：合成一个「干净视图集空转 + 无视图集体本体」的符号，要求画本体而不是
空壳、也不要兜底把隐藏笔画带进来；以及一个「干净视图集更瘦但确实有笔画」的符号，
要求仍按正常视图选，防止把判据改成按薄厚。

## 六、剩下的 11 处：四种判据都试过了，分不开

修完空转还剩 11 处（7 存根 + 4 本体在他表），逐个实测过四种可以写进代码的判据：

| 判据 | 203 个可判放置命中 | 结论 |
|---|---:|---|
| 现行（空转落最多） | 189（93.1 %） | 基线 |
| 更肥的那张表严格包含正常表的所有笔画 | 190（93.6 %） | 只修好 1 处（`Filter.sym`），0 处被弄坏 |
| 两张表笔画完全相同才算同一本体，否则取肥的 | 184 | 比基线更差 |
| 全开的视图集若隐藏层没笔画，就当干净表参与竞争 | 172（84.7 %） | 明显更差 |
| 只按几何（不含字）比肥瘦 | 184 | 无改进 |

失败的原因在数据里：**当正常表自己画得出笔画、而另一张表更肥时，两种结局都真实存在**。
`Remarks.sym`（正常 `Sheet6` 3 条 1.3 mm 标记；`Sheet37` 11 条 27 mm 标注框）、
`Local Vent.sym`（1 对 2）等处，缓存站正常表；`Concentric Reducer.sym`（1 对 3）、
`Arrestor Type 1.sym`（4 对 6）等处，缓存站肥表。包围盒比例也分不开：
错的那些在 1.00–3.00 之间，而 `Local Vent` 的 1.88 是对的。

唯一站得住的增量是「严格包含」：`Filter.sym` 的 `Sheet6` 那一条线确实在 `Sheet156`
的三条里，缓存是后者。它 0 处弄坏，但只修 1 处，代价是给生产代码加一层按微米取整的
笔画指纹 + 多重集包含；而 `library_only` 恒为 0（每处放置都自带缓存体），这 11 处
一个像素都不上纸。所以没实现，只把判据和读数留在探针里。

还值得记一笔：`418` 张全开表里只有 **3** 张在隐藏名层上真的有几何 —— 代码注释里
那个「保温/伴热备用视图」在语料里几乎不存在（41 张的「隐藏层有东西」是 `Label`
文字）。但剩下那 3 张足够让这条分支继续存在，而且按「全开即干净」处理会把
`Remarks` 这类标注符号的编辑框当成主体（172 那一条就是这么垮的）。

## 七、那些「空表」不是被删空的（同日追查，结案）

上一节留了个问题：163 张「管着一张没有笔画的表」的干净视图集，那些表是不是**曾经有
笔画、后来被删掉**的。先更正计数口径：`163` 是「**每张**干净视图集都指向空表」的文件数，
按视图集数是 **203 条**（1262 次读取 = 两个库根 618 + 644，同名文件各算一次）。

新探针 `examples/probe_empty_sheet_view_sets` 从字节上把 203 条逐条拆开：

| 那张「空表」的实际情况 | 条数 |
|---|---:|
| 表在，链就是空的：流正好 8 字节（`magic` + 计数 0） | 71 |
| 表里**整份本体都在**，被跨表去重丢掉 | 114 |
| 表里只有一条读不出来的文字记录 | 14 |
| 边界/多段线的图元已被孪生表先认领 | 4 |

**删除的三条证据全部为负：**

- **没有一条是悬挂引用。** 203 条视图集点名的 sheet，在 `PSMcluster0` 里都有活着的
  `0x0114 JSheet` 对象（203/203），并且都有对应的 `Sheet<N>` 流（0 条「连流都没有」，
  流名也都在 `PSMclustertable` 里登记过）。这个格式里的删除是看得见的——绘图侧那条
  「被删掉的 `0x00FA` 记录留下的悬挂 spacemap 边」就是样子（见 `docs/pid-format-guide.md`），
  这批一条也不像。
- **簇头从不比链上多。** 流头 `+4` 的记录数与链上真实条数逐条对过：**0 例**头里多算，
  13 例头里少算（文件自己的计数口径）。没有「索引说 5 条、字节只剩 0 条」这种截断痕迹。
- **132/203 的表里还放着整份本体。** 「空」不是读取这一侧漏了，而是
  `read_symbol_geometry` 把同一份本体只留一份：`package.streams` 是 `BTreeMap`，先读到
  的那份留下，字面序靠后的整张丢掉。114 条严格落在这个机制上，且 **114/114** 保留的都
  是路径排序在前的那一份（`Sheet12` < `Sheet28` < `Sheet6` < `Sheet92`），去重本身无损——
  留下来的并集就是所有表笔画的全集。

剩下那 71 张真正零对象的表也有具体身份：`Line.sym`、`Circle.sym`、`Block.sym`、
`Ellipse.sym`、`Annotation Point.sym` 这类标注图元，本体在 `Sheet12`，`Sheet6` 只有 8 字节
空链。更有说服力的是其中 5 个符号（`Annotation Point`、`Ellipse`、`Text with Border`、
`Revision Cloud`、`Cone Roof Tank`）**每一张表都是 8 字节空链**，整份文件一条笔画都没有——
按「被删空」解释，得先有笔画被删；按「编辑器留下的空白表」解释则一致。所以这 71 张是
空白画布，不是清空后的残骸。

另外 14 条是单个 `0x004D` 文字记录，读者两个计数偏移都对不上它的长度（`Nozzle NPD.sym`、
`Nozzle Rating.sym`、`Fail Action.sym` 之类，本体都在 `Sheet43`/`Sheet37`），属于文本解码
的一个小缺口，不是删除。4 条边界/多段线情形（`MOV-GLV-…02.sym`、`Self-Operated Safety
Block Valve.sym`）是边界成员被孪生表先认领后整条被丢，同样是被去重，不是被删。

**结论**：「正常视图空转」在字节层面就是空——没有墓碑字段可读，也没有任何删除留下的
悬挂引用。那条线索到此为止，`displayed_primitives` 里「空转不选」的门槛仍然是正确的
修法：它不依赖这张表空的原因，只要求它画得出东西。

## 八、仍未闭合

- 22 个「正常但空转/存根」的放置只是本语料里被缓存体钉住的部分；库整体 431 个空转
  符号没有参考图可对，它们的修后行为只由「空转不选」这条原则保证，不由出图验证。
- 剩下 11 处的正确答案在图内缓存里（`PidSymbolDefinition` 只记图形自己的
  `JSheet` oid，不记它抄自库的哪张表），库里没有第二条线索引到它。
- 14 条读不出来的 `0x004D`（上节）如果哪天真要修，得先弄清它们用的是第三种头宽度；
  它们全在隐藏名层上，纸面不受影响。

工具：`examples/probe_normal_view_too_wide`（放置打分、库普查、逐符号位/层明细）；
`examples/probe_empty_sheet_view_sets`（203 条空表逐条的流/链/簇头/JSheet 读数，
`--inventory dump:<文件名>` 可把一张符号每张表的形状逐条打出来）。
