# 验收门禁读的是「画了什么」，不是「纸上看得见什么」：11 张参考图的差异逐项定性

日期：2026-11-10

对象：`D:\work\cad\pid-test-data\验收数据` 里有原厂参考图的 11 张图。起点是
`target\export-pdf-fix\next\status.md` 里被归为「参考图与 .pid 不同版」「原因不明」「语料缺文件」的
那几项。基线是 `dc86a26`，工具链 stable 1.99.1。本文每个数字都由文末列出的探针或脚本重新量出，
没有沿用旧记录。

## 结论

1. **A2 的仪表簇偏移和两条文字不同（原定性「参考图与 .pid 不同版」）成立，属于 (b)。** 手上四份互相独立的
   数据——验收语料的 `.pid`（rev 53，6/25）、备份包的 `.pid`（rev 61，7/2）、厂家发布的 `_Data.xml`（6/29）、
   备份包的数据库（7/19）——逐项一致，都是我方画出来的样子；参考图（10/14）是它们之后的一版。差异全部
   落在四个矩形里，矩形之外整页读数是 1.000000 / 0 / 0 / 232 对全配。
2. **两页「多出的图框」不是我方缺陷，但 (b) 还是 (c) 手上的数据分不出来。** 我方画的图框逐笔就是原厂给同模板
   姊妹图画的那一个（286 笔、34 条标签全配、笔宽 0 不符）；`.pid` 29 个字段、数据库的 `T_DRAWING` /
   `T_SMARTFRAME` / `T_REPRESENTATION` 都不区分带框与不带框。
3. **「三张 A0 缺 `pulsation damper.sym`、参考图在这些位置同样没有墨」这条定性被推翻。** 这个符号在三张图上
   各有 2 处、每处 22 个图元 30.438 mm，是从 `.pid` 自带的缓存体画出来的；参考图也画了它；6 处的读数都是
   0 未覆盖、0 多余。缺的是符号库里的文件，不是页面上的符号。
4. **掩膜本身藏的东西很少，但门禁的读法藏着两类真实的、看得见的缺陷。** 读图器不分颜色：白色描的线和黑色
   的线一样算「墨」。原厂图把标签原点记号（325 mm）描成白色，我方印成黑色；原厂图把模板的线印成黑色，
   我方把它们描成白色（每张 `CPE-SQ` 图 3323 mm，A2 5274 mm）。**交付到 `验收数据\plot\` 的 PDF 因此
   没有标题栏和会签栏的格线，A2 那张连图框都没有**，标签旁边多出 `+` / `⊢` 记号。按「只读墨」重读，
   11 张图没有一张两种读法相同；整页 coverage 最低 0.688636。
5. 另有三处门禁读不到：贯穿整页的线被读图器当作图纸边缘丢掉（A2 内框原厂印 1.355 mm，我方 1.017 mm，
   从来没被比过）；页幅容差 1 mm（A2 我方 594.33 × 420.57，原厂 594 × 420）；线帽与拐角（原厂圆头，
   我方平头）。
6. **`SP:` 不再是「原因不明」**：原厂把冒号单独作为一个文字对象，落在「未取整的起点 + 两个字进」取整之后的
   设备单位上；这条规则在 9 张图上 9/9 预测对（6 张 22 个单位、3 张空冷器 21 个单位）。
   **`仪表风管线` 收窄为两层**：A2 整页没有上设备网格（映射已经量出来）；按量出的映射，全页只剩 4 条
   带盒文字记录对不上——原厂把它们画得比规则高整 1 个设备单位，为什么还不知道。

一句话：按门禁现在的读法 8 张图已经是 1.000000；按纸上看得见的墨，**11 张没有一张完全相同**——其中 8 张只差
代码（数据齐全），闪蒸A、低温A 两张的图面只差同样的代码、图框要等现场答复，A2 既要新数据也要代码。

## 一、重量的读数

`reference_plots_acceptance` 在本分支重跑（`PID_ACCEPTANCE_DELIVER=0`，输出在
`target\accept-evidence\gate-baseline\`），2 个用例通过，读数与 `status.md` 逐项相同。

| 图 | 掩膜：coverage / 未覆盖 / 多余 / 标签 / 超限标签 / 笔宽 一致-不符-未配 | 整页：coverage / 未覆盖 / 多余 / 标签 / 超限标签 |
|---|---|---|
| I井场 | 1.000000 / 0 / 0 / 546/546 / 2 / 9370-0-0 | 1.000000 / 0 / 0 / 583/583 / 8 |
| J K井场 | 1.000000 / 0 / 0 / 775/775 / 1 / 14709-0-0 | 1.000000 / 0 / 0 / 813/813 / 7 |
| L M井场 | 1.000000 / 0 / 0 / 921/921 / 2 / 17866-0-0 | 1.000000 / 0 / 0 / 959/959 / 8 |
| 闪蒸A | 1.000000 / 0 / 0 / 151/151 / 0 / 1278-0-286 | 1.000000 / 0 / **3478.925** / 155/155 / 0 |
| 闪蒸B | 1.000000 / 0 / 0 / 123/123 / 0 / 1425-0-0 | 1.000000 / 0 / 0 / 161/161 / 6 |
| 低温A | 1.000000 / 0 / 0 / 167/167 / 2 / 1695-0-286 | 1.000000 / 0 / **3478.683** / 171/171 / 2 |
| 换热器CD | 1.000000 / 0 / 0 / 249/249 / 2 / 3196-0-0 | 1.000000 / 0 / 0 / 253/253 / 2 |
| 空冷器A | 1.000000 / 0 / 0 / 288/288 / 2 / 3404-0-0 | 1.000000 / 0 / 0 / 326/326 / 8 |
| 空冷器B | 1.000000 / 0 / 0 / 290/290 / 2 / 3411-0-0 | 1.000000 / 0 / 0 / 328/328 / 8 |
| 空冷器C | 1.000000 / 0 / 0 / 283/283 / 2 / 3278-0-0 | 1.000000 / 0 / 0 / 321/321 / 8 |
| A2 | 0.982943 / 208.671 / 187.091 / 214/225 / 5 / 2806-0-102 | 0.988161 / 208.840 / 187.091 / 250/261 / 7 |

长度单位 mm。「超限标签」是锚点偏离参考超过 0.15 mm 或 run 宽差超过 0.05 mm 的已配标签，11 页合计整页
64、掩膜 20。这张表是门禁的读法：**把描出来的每一笔都当墨**。5.2 节说明它和纸面的差别。

比对的松紧：coverage / 未覆盖 / 多余按 0.5 mm 采样和容差算（`compare.rs` 的 `COVER_MM`）；真正把几何卡紧的是
笔宽配对，我方每一笔（≥ 0.2 mm）要在参考那一笔自己的线上 0.08 mm 以内（同宽的弦 0.20 mm），未配数逐页钉死。

## 二、手上有哪些版本

| 图 | 验收语料的 `.pid` | 备份包里的 `.pid` | 参考 PDF 生成时间 |
|---|---|---|---|
| I井场 | rev 30，6/22 14:27 | 同一文件 | 10/12 11:59:16 |
| J K井场 | rev 41，6/29 14:56 | 同一文件 | 10/14 16:57:21 |
| L M井场 | rev 90，6/29 14:57 | 同一文件 | 10/14 16:57:33 |
| 闪蒸A | rev 23，7/6 17:18（`liuzongshuai_hb`，SPPID 09.00.00.0155） | rev 22，7/6 02:01 | 10/12 11:56:18 |
| 闪蒸B | rev 18，7/6 17:18（同上） | rev 19，7/13 00:31 | 10/12 11:53:13 |
| 低温A | rev 30，7/6 17:18（同上） | rev 31，7/9 17:00 | 10/12 11:59:26 |
| 换热器CD | rev 16，7/6 17:19（同上） | rev 15，7/6 01:45 | 10/12 11:59:22 |
| 空冷器A | rev 17，6/25 22:39 | rev 21，7/1 22:23 | 10/14 16:57:45 |
| 空冷器B | rev 14，6/29 14:55 | 同一文件 | 10/14 16:57:38 |
| 空冷器C | rev 11，6/29 14:57 | 同一文件 | 10/14 16:57:41 |
| A2 | rev 53，6/25 23:55（909 312 B） | rev 61，7/2 00:11（1 372 160 B） | 10/14 16:58:31 |

- 备份包（`PlantData~2~711.zip`，清单时间 2026-07-19 20:13:24）在本机有两份
  （`D:\work\cad\pid-test-data` 与 `D:\work\plant-code\cad\pid-test-data`），逐字节相同；A2 的 `.pid` 在本机没有
  第三个版本。
- 6 张两份拷贝不同的图，两份导出的页面相同：闪蒸A、闪蒸B、A2、换热器CD 的 PDF 逐字节相同；低温A、空冷器A
  读回的线段和文字相同。**换版本不改变任何一页的读数。**
- 11 张参考 PDF 的 Producer 都是 Amyuni PDF Converter 4.5.2.9，Title 都是 `Document1`，颜色只有
  DeviceGray 的 0 和 1。
- 数据库与 `.pid` 的一致性对 11 张图都成立（`probe_does_the_database_place_a_sheet_where_the_pid_does`，
  验收语料和备份包两套拷贝都退出 0）：有图元的 2395 行 `T_SYMBOL` 里 1772 行与 `.pid` 的放置点相差不到
  1 µm，其余每个符号文件各差一个常量（符号自己的原点；`Skid Mounted Equipment.sym` 要先除以该行的
  `SCALEFACTOR`，单位比例下都是 89.616 mm）；137 条 OPC 的 `DESCRIPTION` 全部照样写在页面上。所以手上的
  `.pid` 相对手上的数据库没有一张是旧的。

## 三、逐项定性

### 1、2　A2 的仪表簇偏移与两条文字——(b) 源数据确实与参考图不同

`集注站 轻烃分离器I气出口接口位置改造工艺及自控流程图`。

**差异全部落在四个矩形里**（`probe_what_a_page_reads_inside_a_window`，退出 0）：

| 矩形（参考图页面 mm） | 参考墨 | 未覆盖 | 多余 | 标签 |
|---|---:|---:|---:|---|
| TT / TICA 回路与它的信号线 x 436..520，y 196..346 | 959.812 | 208.671 | 187.091 | 18/27 |
| `天然气自轻烃分…器I来` x 50..96，y 291.5..298.5 | 0 | 0 | 0 | 0/1 |
| `…-PC-0114301-A2-C40` x 100..150，y 312..319 | 0 | 0 | 0 | 0/1 |
| 左下角图纸边缘的一小段 x 0..0.4，y 0.2..0.7 | 0.169 | 0.169 | 0 | 0/0 |
| **四个矩形之外的整页** | 16679.649 | **0.000** | **0.000** | **232/232**，coverage 1.000000 |

第四个矩形不是图面差异：参考图的图纸边缘是两条折线，左边那条是
`m(0.169,420.116) l(0.169,0.508) l(0.169,0.339)`——驱动在每条折线末尾多画一个设备单位（0.169 mm）。
边缘本身被读图器当图纸边缘丢掉（5.3 节），只剩末尾这一小段。

**回路的位置**。参考图与我方的标签起点（mm）：`TT` (496.99, 221.83) 对 (498.94, 238.63)；`TICA`
(495.47, 238.25) 对 (497.48, 255.09)；`SP=-8℃` (506.14, 236.56) 对 (508.09, 253.35)——相差约 (+2.0, +16.8)。
渲染图（`a2-cluster-reference.png`、`a2-cluster-ours-rev53.png`）上看得清楚：参考图里 TT 的工艺接头
（`Flange Type 2.sym`）在下面那根 6" 管线上，仪表圆压在上面那根 4" 管线上；我方（也就是手上的 `.pid`）里接头在
上面那根 4" 管线上。测温点从一根线挪到了另一根线，连带把整个回路往下带了 16.8 mm。`分流器` 变成 `分离器`、
`50-PC` 变成 `100-PC` 是参考图上另外两处与手上数据不同的地方；三处是不是同一次修改，手上的数据说不了。

**四份数据都是我方画的样子，而且都早于参考图至少三个月**：

| 数据 | 时间 | 回路位置 | OPC 文字 | 管线号 |
|---|---|---|---|---|
| 验收语料 `.pid` rev 53 | 6/25 | TT (500.836, 237.212) mm，TICA (500.836, 253.665) | `天然气自轻烃分流器I来` | `50-PC-0114301-A2-C40` |
| 备份包 `.pid` rev 61 | 7/2 | 同上（导出的 PDF 与 rev 53 逐字节相同） | 同上 | 同上 |
| 厂家 `_Data.xml` | 6/29 | — | `Description="天然气自轻烃分流器I来"` | `50 mm-PC-0114301-A2-C40` |
| 备份数据库 `Export.dmp` | 7/19 | `T_SYMBOL` TT (0.500836, 0.237212) m、TICA (0.500836, 0.253665) m，与 `.pid` 相差 0.000 mm | OPC `95810B3A…` 的 `DESCRIPTION` `天然气自轻烃分流器I来` | `ITEMTAG` `50 mm-PC-0114301-A2-C40`，`NOMINALDIAMETER` 50（2 条 run） |

- 两个 `.pid` 版本按原始字节搜（空闲扇区也搜）：`分流器I来`、`50-PC-0114301` 命中，`分离器I来`、`100-PC` 零命中。
- 数据库里 `H=-15℃ ` (0.507793, 0.260766)、`L=-5℃ ` (0.508307, 0.251551) 两条标签的点正好落在 `.pid` 的
  文字框上；这张图 185 行 `T_SYMBOL` 里 142 行有图元，112 行在 1 µm 内，其余 30 行是 8 个符号文件各自的原点
  常量；14 条 OPC 文字全部写在页面上。
- 数据库对这张图最晚的 `SP_MODELITEMTIMESTAMP` 是 2026-06-25 23:53:57，`T_HISTORY` 最晚 2026-07-01 23:23:58。

所以参考图对应的是 **2026-07-19 之后、2026-10-14 当天或之前**的一版图纸，手上没有。拿到那一版以后这一页
应当读到什么：四个矩形之外已经是 1.000000 / 0 / 0，矩形之内取决于新数据。

### 3　两页多出的整套图框——不是我方缺陷；(b) 还是 (c) 未定

`一级闪蒸分离器A`、`低温分离器A`。整页多余 3478.925 / 3478.683 mm，各 286 笔未配、34 条模板标签无对。

**我方画的就是原厂的图框**（`probe_is_the_border_a_reference_lacks_the_one_its_sister_prints`，退出 0）：把这两页的
图框单独取出来（141 个线条图元、34 条标签）写成一页，去和姊妹图闪蒸B 的参考图比——

| 比对对象 | 多余 | 我方标签配上 | 笔宽 |
|---|---:|---|---|
| 它自己的参考图 | 3479.367 mm | 0/34 | 0 配 / 286 未配 |
| 闪蒸B 的参考图 | **0.000 mm** | **34/34** | 286 配，286 一致，0 不符，0 未配 |

**手上没有任何数据说这两张图不该带框**：

- `.pid`：`probe_does_the_pid_say_the_border_prints` 重跑，验收语料「29 fields over 2 bare and 7 framed sheets: 0
  separate the groups」，备份包「…2 bare and 4 framed…: 0」。细目见
  `2026-11-07-the-two-bare-a1-plots-are-not-in-the-pid.md`。
- 数据库：`T_DRAWING` 没有一列区分两组（6 张 A1 的 `TEMPLATE` 都是 `CPE-SQ-A1.pid`）；6 张 A1 都没有
  `T_SMARTFRAME` 行；`T_REPRESENTATION` 没有图层信息。
- 出图时间：10/12 同一批里 11:53:13 闪蒸B 带框、11:56:18 闪蒸A 不带、11:59:16 I井场 带框（A0 模板）、
  11:59:22 换热器CD 不带、11:59:26 低温A 不带。带框和不带框交错出现，不是「那天模板目录整个不可用」。

**有一个先例值得注意**：换热器CD。数据库说它的模板也是 `CPE-SQ-A1.pid`，但它的 `.pid`（两个版本都是）里没有
图框、没有模板链接，参考图不带框，我方也不画。也就是说，这个工厂里「数据库记着模板、文件里图框已经没了」的
状态是存在的。三张不带框的参考图内容流结构相同：0 个裁剪组（带框的 A0/A1 是 6 个，A2 是 2 个）。闪蒸A、低温A
如果在 7/19 之后也变成了换热器CD 那样，参考图就是现在这个样子——这是 (b) 的读法；出图那一刻模板链接没解析
出来是 (c) 的读法。两种读法手上的数据都不能排除，也都不能证实。

**这两页纸面上的差别不是 3478.9 mm 这个数**：模板的线我方大多描成了白色（5.2 节），读图器又把贯穿整页的
8 条框线丢掉（5.3 节）。纸面上我方比参考图多的是每页 8 条框线（5630 mm，黑色）、155.9 mm 黑色的模板内容和
34 条模板文字。

### 4　三张 A0「缺 `pulsation damper.sym`」——原定性推翻

`status.md` 写的是「我们不画这个符号…参考图在这些位置同样没有墨」。实测
（`probe_is_a_library_gap_drawn_from_the_file`，退出 0）：

| 图 | 放置 | 我方画出 | 参考图在放置框外扩 1 mm 内的墨 | 未覆盖 | 多余 |
|---|---|---|---:|---:|---:|
| I井场 | x 304.063..307.238，y 341.628..347.957 | 22 图元 30.438 mm | 32.470 | 0.000 | 0.000 |
| I井场 | x 303.823..306.998，y 405.559..411.888 | 22 图元 30.438 mm | 30.607 | 0.000 | 0.000 |
| J K井场 | x 259.889..263.064，y 470.345..476.673 | 22 图元 30.438 mm | 30.607 | 0.000 | 0.000 |
| J K井场 | x 259.648..262.823，y 534.275..540.604 | 22 图元 30.438 mm | 32.131 | 0.000 | 0.000 |
| L M井场 | x 239.212..242.387，y 376.066..382.395 | 22 图元 30.438 mm | 30.311 | 0.000 | 0.000 |
| L M井场 | x 238.971..242.146，y 439.996..446.325 | 22 图元 30.438 mm | 32.470 | 0.000 | 0.000 |

两边都按「只读墨」读。三张图的 `missing_boxes` 都是 0。`PaperModel::missing_symbols` 记的是「图纸点了名、符号库
答不上来的文件」，是符号库的账；放置先从 `.pid` 自带的缓存体画，画不出来才进 `missing_boxes`，只有后者进掩膜。
渲染对照：`pulsation-damper-I-reference.png`、`pulsation-damper-I-ours.png`。

这一项不是差异，不需要数据，也不需要改画法。需要改的只是说法：门禁里把它钉成「缺符号」的那一条读起来像
「页面上少了东西」，建议改成「符号库缺文件、页面由缓存体画出」（在 `reference_plots_acceptance.rs` 里，提案）。
向现场要 `Pulsation Damper.sym` 只对「符号库齐全」有意义，对这三页的读数没有影响。

### 5　掩膜藏了什么，以及门禁的读法藏了什么

#### 5.1　模板掩膜（`assets/template-masks.json`）

`probe_what_the_template_masks_hide`。掩膜里的参考墨与参考标签：

| 图 | 掩膜内参考墨 mm | 掩膜内标签：参考 / 我方 | 整页 − 掩膜：未覆盖 / 多余 | 掩膜内超限标签 |
|---|---:|---|---|---:|
| I井场 | 3515.440 | 37 / 37 | 0 / 0 | 6 |
| J K井场、L M井场 | 3499.184 | 38 / 38 | 0 / 0 | 6 |
| 闪蒸A | 0 | 4 / 38 | 0 / **+3478.925** | 0 |
| 闪蒸B、空冷器A、空冷器C | 3499.970 | 38 / 38 | 0 / 0 | 6 |
| 空冷器B | 3505.865 | 38 / 38 | 0 / 0 | 6 |
| 低温A | 0 | 4 / 37 | 0 / **+3478.683** | 0 |
| 换热器CD | 0 | 4 / 4 | 0 / 0 | 0 |
| A2 | 5405.667 | 36 / 36 | **+0.169** / 0 | 2 |

掩膜里被藏起来的差异逐条是：

- **6957.607 mm 多余**：闪蒸A、低温A 的整套图框（第 3 项）。闪蒸A：左侧会签栏 1559.989 mm / 31 笔，右下标题栏
  1918.936 mm / 255 笔；低温A：1559.881 mm / 31 笔与 1918.802 mm / 255 笔。两页各有 34 条模板标签无对
  （会签栏 12 条、标题栏 22 条）；参考图在掩膜里只有图纸自己的 4 条标题栏标签，都配上了。
- **0.169 mm 未覆盖**：A2 左下角那一小段（第 1 项的第四个矩形）。
- **44 条超限标签**：7 张 `CPE-SQ` 图各 6 条模板字——`010065-kj` +0.0679、`华北油田苏桥储气库群` +0.0532、
  `中国石油集团工程设计有限责任公司华北分公司` −0.1030、`A111001641` +0.0741、`设计证书号` / `勘察证书号`
  +0.0756（都是 run 宽差，mm）；A2 两条——`中国石油工程建设有限公司` +0.4257、`华北分公司` +0.1772。这些是
  文字排版的事，另有人在做。

除此之外掩膜里没有别的差异：其余 8 页掩膜内未覆盖、多余都是 0，标签全配。

#### 5.2　颜色：描了不等于印了——两类 (a) 缺陷

读图器（`read.rs`）读回的线段只有起点、终点、宽度，没有颜色；`draw_path` 不看 `Paint`。于是白色描的线和黑色的线
读出来一样。两边恰好在相反的地方用了白色，互相抵消，门禁读到 1.000000。

`probe_which_paint_leaves_no_ink`（退出 1，这就是结论）量的是：

- **原厂图**共有 325.289 mm 描成白色（`1 G`）：I 58.928、J K 91.101、L M 106.680、闪蒸A/B 7.281、低温A 4.741、
  换热器CD 4.572、空冷器 A/B/C 各 12.361、A2 7.620。它们是标签符号的原点记号：`Remarks.sym`、
  `Remarks Chinese_large.sym`、`Item Note & Label.sym`。**我方把其中 322.862 mm 印成了黑色**——标签旁边
  多出一个 `⊢` 或 `+`（`item-note-I-reference.png` 对 `item-note-I-ours.png`，`remarks-tick-I-*.png`）。
- **我方**共有 35185.357 mm 描成白色：每张 `CPE-SQ` 图 3323.443 mm，A2 5274.366 mm，都是图框模板的线。
  **原厂图把这些线印成黑色**；我方描成白色的线里有 28536.439 mm 在参考图上是墨——标题栏和会签栏没有格线，
  A2 整张没有图框（`title-block-flashB-reference.png` 对 `title-block-flashB-delivered.png`；
  `whole-A2-reference.png` 对 `whole-A2-delivered.png`；`delivered` 是从 `验收数据\plot\` 里已交付的 PDF 裁的）。

两边都按「只读墨」重读（`read::Painted::Inked`，本分支 `0d94da1` 加的读法；`read_plot` 本身行为不变）：

| 图 | 整页（只读墨）coverage / 未覆盖 / 多余 | 掩膜（只读墨）多余 |
|---|---|---:|
| I井场 | 0.908242 / 3192.063 / 44.285 | 29.046 |
| J K井场 | 0.940290 / 3192.063 / 74.763 | 74.763 |
| L M井场 | 0.947372 / 3192.063 / 89.998 | 89.998 |
| 闪蒸A | 1.000000 / 0 / 161.727 | 5.824 |
| 闪蒸B | 0.729280 / 3192.571 / 5.824 | 5.824 |
| 低温A | 1.000000 / 0 / 159.775 | 3.882 |
| 换热器CD | 1.000000 / 0 / 3.883 | 3.883 |
| 空冷器A | 0.844485 / 3192.571 / 7.802 | 7.802 |
| 空冷器B | 0.845622 / 3192.571 / 7.802 | 7.802 |
| 空冷器C | 0.843227 / 3192.571 / 7.802 | 7.802 |
| A2 | 0.688636 / 5489.966 / 192.409 | 192.409 |

11 页合计：按墨读比按描读，整页未覆盖多 27627.600 mm，多余少 6388.628 mm。

**源数据里的规则**（`probe_which_strokes_state_the_paper_colour`）：

- 标签原点记号的样式是一条 `JStyleOverride`（类型 `0x0030`），掩码有「声明颜色」位（`OVERRIDE_STATES_COLOUR`），
  颜色字是 **`0x8000_0000`**。`src/style_link.rs` 把这个字叫 `OVERRIDE_COLOUR_BY_LAYER`，解析时保留基样式的
  颜色，于是这一笔解析成黑色。符号库（618 个 `.sym` + 5 个 `.igr`）里声明这个字的覆盖记录正好出现在 8 个
  文件里：`Item Note & Label.sym`、`Item Note & Label line1.5.sym`、`Drawing Description.sym`、
  `Remarks Chinese_large.sym`、三个目录下的 `Remarks.sym`、`Up Remarks.sym`；11 张图各自的定义存储里各有
  1 条（宽 0.010 mm）。原厂图上声明这个字的每一笔都是白色，没有别的笔是白色。
- 模板的线声明的是字面白色 `0x00FF_FFFF`：`CPE-SQ-A0/A1.igr` 125 笔里 122 笔，`CPECCHB-SQ-A2.igr` 43 笔里
  42 笔。`write.rs::plot_ink` 对白色有一条例外，原样描成白色（2026-10-20 加的，当时还没有画模板）。原厂图上
  这些线全是黑色。

也就是：**`0x8000_0000` = 用纸的颜色画（什么都不印）；字面白色 = 在白纸上换成对比色印出来**。我方两条都做反了。

还有一处与此相连的细节：原厂的白色记号是**后画的**，压在别的墨上，会在框线和字上留下一个小缺口
（`white-wipe-I-reference.png`：`STOP` 左边的框线和 `S` 的左缘各被咬掉一点）。要逐像素相同，白色的笔得画在
同一层的墨之后。

#### 5.3　贯穿整页的线：读图器丢掉，两边都不比

`read::sheet_edge_mm` 把长度超过页宽或页高 90 % 的线当作图纸边缘丢掉，参考图和我方都丢。所以框线在不在、在
哪里、多宽、印没印，门禁的任何读数都不反映。`probe_which_strokes_the_reader_drops_as_sheet_edges`（退出 1）：
参考图一共被丢掉 49891.357 mm，我方 62172.626 mm；两边对不上的有 24 条——

- 闪蒸A、低温A 各 8 条（外边缘 4 条 + 内框 4 条，每页 5630 mm，我方黑色，参考图没有）：第 3 项的图框。
- A2 的 8 条：我方 8 条全部描成白色（5.2）；**内框 4 条原厂印 1.355 mm（`3.84 w`，8 个设备单位），我方
  1.017 mm**——模板声明 1.4141 mm，`write.rs::pen_width_mm` 把 0.85 mm 以上都归到 1.017 mm。这是第五支笔，
  笔宽配对从来没见过它；另外原厂图没有上边缘那一条（纸比图纸矮，见 5.4）。

#### 5.4　页幅：容差 1 mm

门禁对页幅的容差是 1 mm。10 张标准幅面两边相同；A2 我方 594.33 × 420.57 mm，原厂 594.00 × 420.00 mm。
这张图的图框模板是 593.939 × 420.000 mm，页框把它放在离页面原点 (0.388, 0.566) mm 的地方，我方把页面撑大到
装得下；原厂把它印在 594 × 420 的纸上，放不下的部分（上边缘、右边缘）落在纸外。6.2 节量出的映射说明了原厂是
怎么放的。

#### 5.5　线帽与拐角

11 张参考图每页声明一次 `1 j 1 J`（圆角拐、圆头线帽），我方是 `0 J 0 j`。读图器只读中心线，读数不反映；
纸面上粗线的端头和框角看得出来（`white-wipe-I-reference.png` 的框角是圆的，我方是方的、缺一个小角）。

### 6　`SP:` 与 `仪表风管线`

这两项属于文字排版，只调查、不修。

#### 6.1　空冷器 A/B/C 的 `SP:` 比参考宽 0.1693 mm——规则找到了

`2026-11-08-the-eight-text-residuals-are-the-drivers-own-placements.md` 已经查明：原厂把 `SP` 和 `:` 写成两个文字
对象，冒号在三张空冷器图上离起点 21 个设备单位、在另外 6 张图上 22 个；我方每张图都是 11 + 11 = 22。当时的结论
是「驱动自己的落点，不修」，为什么偏偏是这三张没有解释。

解释是取整的先后（`probe_does_the_unrounded_start_place_the_colon_of_sp`，退出 0）。标签起点换成设备单位、取整
之前的值是 `x0 = 插入点 x × (⌊页宽单位⌋ / 页宽单位) / 单位`；`SP:` 每个字的字进是 10.9756 个单位。如果第一个
对象落在 `round(x0)`、冒号落在 `round(x0 + 2 × 10.9756)`，那么当且仅当 `x0` 的小数部分在 [0.5, 0.5488) 时冒号
离起点 21 个单位，否则 22 个：

| 图 | `x0`（单位） | 小数部分 | 预测 | 参考图 |
|---|---:|---:|---:|---:|
| I井场 | 4668.299 | 0.299 | 22 | 22 |
| J K井场 | 5017.359 | 0.359 | 22 | 22 |
| L M井场 | 4366.591 | 0.591 | 22 | 22 |
| 闪蒸A、闪蒸B | 2008.960 | 0.960 | 22 | 22 |
| 低温A | 1913.635 | 0.635 | 22 | 22 |
| 空冷器A、B、C | 2281.523 | **0.523** | **21** | **21** |

9/9。三张空冷器图是 9 张里唯一起点小数落在那个窗口里的。离取整边界最近的是空冷器的 2303.474（差 0.026 个
单位）和 L M 的 4388.542（差 0.042），都不是擦边。

所以这不是「原因不明」，是 (a)：我方先把起点取整、再从取整后的起点按整单位字进排下去；原厂是从**未取整**的
起点算每个文字对象的落点再各自取整，起点取整丢掉的那部分小数被带进了后面的对象。修法在 `write.rs`
（提案 5）。

#### 6.2　A2 的 `仪表风管线`（锚点偏 0.2122 / 0.2006 mm）——两层

**第一层：A2 整页没有上设备网格，映射量得出来。** `write.rs::device_scale` 只认标准幅面；A2 的页面是
594.33 × 420.57，所以它的文字起点没有取整到 1/150 in 网格，每条标签在两个方向上各带着最多半个单位
（0.085 mm）的零头。`probe_which_device_grid_a_page_is_lettered_on`（退出 1，因为 A2）把参考图每个 run 的起点
（全部在整单位上）和我方同文字的起点配对，搜索 `参考单位 = round(比例 × 我方 mm / 单位 + 偏移)`：

| 图 | 配对 run | 标准幅面规则（页宽向下取整、零偏移）复现 x / y | 最优映射 |
|---|---:|---|---|
| 10 张标准幅面 | 149…955 | 每页全部复现，只差被切开的 `℃` 那一段（I、L M、低温A、换热器CD 各 2 条，空冷器 A/B/C 各 1 条） | 就是这条规则 |
| A2 | 250 | 119 / 168 | 比例 1.000029…1.000037（两轴公共），偏移 x −1.002…−0.986、y −0.010…+0.013 个单位 |

A2 的最优映射可以写成：**比例 3508 / 3507.874 = 1.0000359（594 mm 的纸取 3508 个单位），x 偏移 −1 个单位，
y 偏移 0**。它复现 250 个起点里 x 的 249 个、y 的 246 个。标准幅面的规则用在 A2 上只复现 119 / 168 个。
（标准幅面上 7021.654 → 7021、4966.535 → 4966 是向下取整；A2 上 3507.874 → 3508。三个数用同一条规则说得通
的一种写法是「纸宽按百分之一英寸的标准表 46.81 / 33.11 / 23.39 in × 150 再截断」，得 7021 / 4966 / 3508——
这只是一个与三个数都相符的猜测，没有别的证据。）

**第二层：按这个映射，全页只剩 4 条标签在 y 上对不上，它们正好比映射给的位置高整 1 个单位。**

| 标签 | 参考（单位） | 映射给出 | 取整后 | 差 |
|---|---:|---:|---:|---:|
| `低压放空总管` ×2 | 2131 | 2130.34 | 2130 | +1 |
| `仪表风管线` ×2 | 2206 | 2204.89 | 2205 | +1 |

（x 上唯一对不上的是被切开的 `℃`，已知的例外。）

`2026-11-08-…` 那篇（探针 `probe_a2_device_grid`，量的是这 4 条相对模型的偏差）记的是「四条落点分成
−0.1978 / −0.1221 mm 两组，不共享一个常量，所以按框高取中线修不了」。放到设备网格上看，两组其实是同一件事：
两条都是「比规则高整 1 个单位」，−0.1978 和 −0.1221 的差别只是各自取整前的小数不同（.89 与 .34）。

这 4 条有什么特别（`probe_which_text_records_state_a_box`）：11 张图里带盒（kind-2 尾部）的文字记录共 141 条，
盒高 3.689 mm 的 133 条（123 条有字）、5.966 mm 的 4 条（都是空白行）、**4.034 mm 的 4 条——就是这 4 条**，
而且它们是 `/Sheet6` 最前面的记录（字节 4164…5092），是这张图上较早的一批 OPC 标签（两行间距 6.000 mm，
别的 OPC 是 5.5123 mm）。同名标签在 A1 图上是不带盒的记录（84 / 86 字节），落点与参考图相同。

两条候选规则，用 127 条有字的带盒记录检验（`target\scratch\boxed_label_rule.py`）：

| 规则 | 盒高 3.689 mm（123 条） | 盒高 4.034 mm（4 条） |
|---|---|---|
| 现行：按字高自己的中线（不看盒高） | 123 条复现 | 0 条 |
| 把字格挂在盒顶（盒以插入点为中心） | 118 条，5 条可区分的全部不符 | 4 条 |

所以「按盒顶」被 3.689 mm 的记录否定，「不看盒高」被这 4 条否定。一条与 127 条都相符的写法是「盒高和字格都
先取整到设备单位再取半差」：4.034 mm = 23.8 → 24 个单位，字格 22 个单位，半差 1 个单位；3.689 mm = 21.8 →
22 个单位，半差 0。**但支持它的独立观测只有 2 个（两个不同的 y），不能算证实。** 定性：这 4 条为什么高 1 个
单位，还不知道；已经确定的是它只涉及这 4 条、大小正好 1 个单位、与盒高有关。

注意：把 A2 放上设备网格（提案 6）以后，`低压放空总管` 那两条现在碰巧在 0.15 mm 以内（0.148 mm）的标签会变成
整 1 个单位（0.169 mm）而超限，除非同时把这 4 条的规则找到。

## 四、提案（要动别人正在改的文件，或要重写金样的）

**提案 1　颜色（5.2）。** 改 `src/style_link.rs`、`src/symbol_library.rs`、`pid-plot/src/model.rs`，不碰禁区
文件；但它改变 7 张夹具的页面模型（模板的线从白变黑），`paper_model_golden` 要用 `PID_PLOT_BLESS=1` 重写，
所以没有在本分支落地，以补丁交回：`target\accept-evidence\proposal\0001-paper-word-and-white-on-the-sheet.patch`
（带两条先红后绿的单元测试）。做法：

- `LineSymbology` 加 `paper: bool`；`StyleRecord` 加 `override_states_paper`；`resolve_line_style` 在覆盖记录
  声明颜色、颜色字是 `0x8000_0000` 时置 `paper = true`（颜色仍是基样式的）。`PrimitiveStyle` 同样带上 `paper`
  （`false` 时不序列化）。
- `pid-plot/src/model.rs` 加 `sheet_colour(rgb, paper)`：`paper` → `[255,255,255]`；字面白色 → `[0,0,0]`；其余
  不变。`stroke_of` 与 `body_stroke` 过这个函数；符号体的 `paper` 只在放置不声明笔（`As Drawn`）时生效——参考图
  上只见过这一种情形。

实测（补丁应用后，随后还原）：

| | 结果 |
|---|---|
| 只读墨的读数 | I、J K、L M、闪蒸B、换热器CD、空冷器 A/B/C **8 页整页与掩膜都是 1.000000 / 0 / 0**；闪蒸A、低温A 掩膜 1.000000 / 0 / 0，整页多余仍是 3478.925 / 3478.683（图框）；A2 整页 0.988264 / 206.935 / 186.595，掩膜 0.983088 / 206.766 / 186.595 |
| 门禁现有读数（描 = 墨） | 不变，`reference_plots_acceptance` 2/2 通过，钉值不动 |
| `pid-parse` 库测试（`--no-default-features --features refdata-archive`） | 945 通过 |
| `pid-parse` 集成测试 `geometry_golden_snapshot`、`style_link_ratchet`、`symbol_library_text`、`symbol_library_sheet_pick`、`sym_elliptical_arc_matches_cached_body`、`elliptical_arcs`、`parse_real_files` | 全部通过 |
| `pid-plot` 库测试 | 270 通过 |
| `pid-plot` 集成测试 `border_template`、`cli_pid_plot`、`export_every_fixture`、`export_ledger_fixtures`、`refdata_archive_corpus`、`reference_plots_acceptance`、`style_witness` | 全部通过 |
| `export_plant_backup`（53 页，默认选中备份包） | 通过：`export-ledger: 53 页全部通过（可疑项 48 个，未归因 0）`，钉值与账本基线不动 |
| `paper_model_golden` | **失败**：`D06.pid` 的 `primitives[0].stroke.rgb[0]` 期望 255、实际 0——要重写金样 |
| clippy（`pid-plot --all-targets`、`pid-parse --lib --tests`，`-D warnings`） | 干净 |
| 没跑 | `bash .github/scripts/check-missing-docs.sh`（新增的 3 个公开字段都写了文档）；`paper_model`、`template_symbol_gap`、`read_reference_plots` 和会往共享语料写文件的四个测试 |

另一种放法：模型如实保留白色、`PaperStroke` 加 `paper` 标志，由 `write.rs::plot_ink` 把白色印成黑色、把 `paper`
描成白色。读数相同，金样同样要动，而且要改 `write.rs`。

**提案 2　门禁改读墨。** `pid-plot/tests/reference_plots_acceptance.rs` 里两边都用
`read_plot_painted(…, Painted::Inked)`（`0d94da1` 已提供）。提案 1 落地之前这样改，10 页会失败——那正是 5.2 的
发现；提案 1 落地之后，`min_pen_agreed` 的钉值变成只读墨的数：I 9334、J K 14661、L M 17812、闪蒸A 1272、
闪蒸B 1419、低温A 1691、换热器CD 3192、空冷器A 3394、B 3401、C 3268、A2 2802（未配 100）。

**提案 3　白色的笔后画。** `write.rs` 里同一层的 `paper` 笔画在该层的墨和文字之后，复现原厂的小缺口。读数
不变，纸面可见。没有试。

**提案 4　线帽与拐角（5.5）。** `write.rs::push_layer` 的 `0 J 0 j` 改成 `1 J 1 j`。读数不变。补丁
`target\accept-evidence\proposal\0002-round-caps-and-the-eight-unit-pen.patch` 的后半。

**提案 4b　第五支笔（5.3）。** `write.rs::pen_width_mm`：声明宽度大于 1.27 mm 的印 1.355 mm（8 个设备单位）。
同一个补丁的前半。实测（与提案 1 一起应用）A2 内框变成黑色的 1.355 mm，图纸边缘对不上的从 24 条降到 17 条
（剩下闪蒸A、低温A 的 16 条和 A2 的上边缘）。`write.rs` 里两条钉着旧笔表的单元测试要跟着改：`declared_widths_snap_onto_the_four_plot_pens`、
`a_validation_marks_width_is_not_a_pen`。分界 1.27 是 (1.1313, 1.4141) 之间选的一个值，证据只有 A2 模板这
4 条线。

**提案 5　`SP:` 的冒号（6.1）。** `write.rs` 的字进取整：把起点取整丢掉的小数带进后面的落点，即后面的文字对象
离取整后的起点 `round(x0 + 前面各字的字进之和) − round(x0)` 个单位。预期：三张空冷器图的 `SP:` 宽差 +0.1693 mm 归零，
`text_off` 各少 1。这条规则对别的标签有没有副作用没有验证——要在 `write.rs` 里试过、把 11 页的超限标签重数一遍
才知道。

**提案 6　A2 上设备网格（6.2）。** `write.rs::device_scale` / `on_the_device_grid`：非标准页面按量出的映射
（比例 3508 / 3507.874，x −1 个单位，y 0）取整。预期：A2 上 250 个起点里 x 的 249 个、y 的 246 个落到参考图的
单位上；副作用见 6.2 末尾（`低压放空总管` 两条会从 0.148 mm 变成 0.169 mm 而超限）。证据只有这一张图：另外
8 张 `CPECCHB-SQ-A2` 图没有参考图，映射里的「−1 个单位」来源不明。

**提案 7　页幅（5.4）。** 非标准图纸按纸张大小出页，放不下的裁掉。要动 `model.rs` 的页幅和
`export_plant_backup.rs` 里的页幅与 `FRAME_OFFSET_SHEETS` 钉值。只有 A2 一张图的证据，没有试。

**提案 8　`pulsation damper` 的说法（第 4 项）。** 见该节。

## 五、向现场要的数据（可直接转发）

1. **`集注站 轻烃分离器I气出口接口位置改造工艺及自控流程图`（A2）**：请提供 2026-10-14 出这张 PDF 时的图纸
   文件（`.pid`），最好连同当时的工厂备份（`PlantData` 压缩包 + `Export.dmp`）。原因：我们手上这张图最新的
   版本是 2026-07-02 存的，数据库备份是 2026-07-19 的，两者一致；而 10/14 的 PDF 上测温回路
   TT-0114301 / TICA-0114301 接在另一根管线上（整体低 16.8 mm），进口 OPC 的文字是「天然气自轻烃**分离器**I来」
   （我们的是「分流器」），管线号是 `100-PC-0114301-A2-C40`（我们的是 `50-PC-…`）。这三处在我们手上的数据里
   （最晚到 7/19）都还不是这样。如果拿不到当时的文件，退一步：用我们手上这一版（6/25 或 7/2 存的）重新出一张
   PDF 也可以。
2. **`集注站 凝析油闪蒸单元 一级闪蒸分离器A…` 与 `集注站 烃水露点控制单元（一） 低温分离器A…`（A1）**：
   这两张 2026-10-12 出的 PDF 没有图框、标题栏和会签栏，而同一天、同一模板（`CPE-SQ-A1`）的
   `一级闪蒸分离器B` 有。请确认两件事：(1) 现在在 SmartPlant P&ID 里打开这两张图，图框还在不在；(2) 10/12 出图
   时这两张是不是就没显示图框。如果图里已经没有图框，请提供当前的 `.pid`；如果图里有图框而 PDF 没有，请在
   模板路径（`…\Ref\Template Files\CPE-SQ-A1.igr`）能打开的环境里重新出这两张 PDF。
3. （可选）符号库文件 `Piping\Specialty Components\Misc\Pulsation Damper.sym`。三张井场图用到它，现在是靠
   图纸自带的缓存画出来的，画得对；补上只是让符号库齐全。

## 六、复现

环境（PowerShell）：

```powershell
Set-Location D:\work\plant-code\cad\pid-parse-wt\accept-evidence
$env:CARGO_TARGET_DIR = 'D:\work\plant-code\cad\pid-parse-wt\accept-evidence\target'
Remove-Item Env:PID_PARSE_SELF_CHECK_DIR,Env:PID_PLOT_FONTS,Env:PID_TEST_DATA_DIR,Env:PID_ACCEPTANCE_CORPUS -ErrorAction SilentlyContinue
```

`pid-plot` 的探针读 `PID_ACCEPTANCE_CORPUS`（默认 `D:\work\cad\pid-test-data\验收数据`），只读，不往语料里写：

```powershell
rustup run stable cargo run --release -p pid-plot --example probe_what_the_template_masks_hide
rustup run stable cargo run --release -p pid-plot --example probe_which_paint_leaves_no_ink            # 退出 1：11 页两种读法都不同
rustup run stable cargo run --release -p pid-plot --example probe_which_strokes_the_reader_drops_as_sheet_edges   # 退出 1：24 条对不上
rustup run stable cargo run --release -p pid-plot --example probe_what_a_page_reads_inside_a_window    # A2 的四个矩形，退出 0
rustup run stable cargo run --release -p pid-plot --example probe_is_a_library_gap_drawn_from_the_file  # 退出 0
rustup run stable cargo run --release -p pid-plot --example probe_is_the_border_a_reference_lacks_the_one_its_sister_prints  # 退出 0
rustup run stable cargo run --release -p pid-plot --example probe_which_device_grid_a_page_is_lettered_on   # 退出 1：A2
rustup run stable cargo run --release -p pid-plot --example probe_does_the_unrounded_start_place_the_colon_of_sp  # 退出 0
```

根 crate 的探针（先构建，再直接运行可执行文件）：

```powershell
rustup run stable cargo build --release -p pid-parse --no-default-features --features refdata-archive `
    --example probe_does_the_database_place_a_sheet_where_the_pid_does `
    --example probe_which_strokes_state_the_paper_colour `
    --example probe_which_text_records_state_a_box `
    --example probe_does_the_pid_say_the_border_prints
$corpus = 'D:\work\cad\pid-test-data\验收数据'
$dmp = 'D:\work\plant-code\cad\pid-test-data\Export.dmp'
$all = Get-ChildItem "$corpus\*.pid" | ForEach-Object { $_.FullName }
$bin = 'target\release\examples'
# 数据库与 .pid：11 张，退出 0
& "$bin\probe_does_the_database_place_a_sheet_where_the_pid_does.exe" $dmp @all
# A2 那个回路的每一行（两份拷贝都可以给）
& "$bin\probe_does_the_database_place_a_sheet_where_the_pid_does.exe" $dmp --tag 0114301 `
    "$corpus\集注站 轻烃分离器I气出口接口位置改造工艺及自控流程图.pid"
# 哪些笔声明纸色或白色：符号库与模板；11 张图
& "$bin\probe_which_strokes_state_the_paper_colour.exe" test-file\symbols-full 'test-file\Ref\Template Files'
& "$bin\probe_which_strokes_state_the_paper_colour.exe" @all
# 哪些文字记录带盒
& "$bin\probe_which_text_records_state_a_box.exe" @all
# 已有的探针：.pid 里有没有区分带框与不带框的字段（--bare 两张、--framed 其余带框的）
& "$bin\probe_does_the_pid_say_the_border_prints.exe" --bare <闪蒸A.pid> <低温A.pid> --framed <其余带框的 .pid …>
```

`test-file\symbols-full` 是本机的完整符号库（不入库）；没有它时符号库那一步只能扫 `test-file\symbols`。

门禁（不往共享语料交付）：

```powershell
$env:PID_ACCEPTANCE_DELIVER = '0'
$env:PID_ACCEPTANCE_OUT = 'D:\work\plant-code\cad\pid-parse-wt\accept-evidence\target\accept-evidence\gate-baseline'
rustup run stable cargo test --locked -p pid-plot --test reference_plots_acceptance -- --nocapture
```

一次性的 Python 脚本（Python 3.12，olefile、PyMuPDF）在 `target\scratch\`，不入库：`expdmp.py`（`exp` 转储
读取）、`dmp_a2.py`、`dmp_versions.py`、`dmp_bare_vs_framed.py`、`dmp_skid.py`（数据库）；`pid_summary.py`、
`pid_links.py`、`pid_grep.py`、`a2_xml_strings.py`（版本与原始字节）；`ref_pdf_meta.py`、`ref_pdf_layers.py`、
`pdf_ops_near.py`、`pdf_colour_census.py`、`white_strokes.py`、`pdf_caps.py`、`long_edges.py`（参考 PDF 的内容
流）；`crop.py`（裁图）；`label_grid.py`、`label_origins.py`、`a2_device_mapping.py`、`a2_device_residuals.py`、
`boxed_label_rule.py`、`textbox_records.py`、`sp_colon_exact.py`（第 6 项）。

## 七、没做的、没验证的

- 没有修任何画法。两类颜色缺陷的修法要重写金样（提案 1），其余要动 `write.rs` 等别人正在改的文件。
- 提案 1 的补丁在 53 张工厂备份上只跑了门禁（通过），没有逐页看过改色以后的页面；那 53 张没有参考图。
- 提案 3、7 没有试；提案 5、6 只有探针的预测，没有在 `write.rs` 里验证，对其他标签的副作用不知道。
- 闪蒸A、低温A 的图框是 (b) 还是 (c)：没有定。
- A2 那 4 条带盒标签为什么高 1 个设备单位：没有定。A2 的映射里 x 偏移 −1 个单位的来源：没有定。
- 数据库的 `T_DRAWINGVERSION`（BLOB）没有读。
- 文字排版的其余超限标签（模板字宽、借字）没有碰。
- 参考图的白色记号后画（小缺口）、圆头线帽只做了渲染对照，没有量长度。
- 没有跑 `--workspace` 的构建和测试，也没有跑会往共享语料 `plot\` 写文件的四个测试
  （`write_five_drawings`、`compare_five_drawings`、`plot_compare`、`plot_text_sources`）。本分支提交后跑过的是：
  `cargo fmt --all -- --check`、`pid-plot` 全部目标与三个根探针的 clippy（`-D warnings`）、`pid-plot` 的库测试
  与 `reference_plots_acceptance`。

## 证据文件

都在 `D:\work\plant-code\cad\pid-parse-wt\accept-evidence\target\accept-evidence\`（不入库）：

- 探针输出：`probes\*.txt`（每个探针一份；数据库探针三份：`.a2`、`.eleven`、`.eleven-backup`）。
- 门禁输出：`gate-baseline\`（含 `whole-page\`）、`gate-baseline.log`。
- 第 6 项：`label-grid.txt`、`a2-device-mapping.txt`、`a2-device-residuals.txt`、`boxed-label-rule.txt`、
  `sp-colon-exact.txt`、`probe-sp-colon.txt`。
- 我方 11 页的基线导出：`ours\*.pdf`；A2 两个版本的导出：`a2\rev53`、`a2\rev61`；各图两份拷贝的导出：`versions\`。
- 渲染对照：`a2-cluster-reference.png` / `a2-cluster-ours-rev53.png`（及 `-zoom`）；
  `title-block-flashB-reference.png` / `-ours.png` / `-delivered.png`；`whole-A2-reference.png` /
  `whole-A2-delivered.png`；`whole-flashB-reference.png` / `whole-flashB-delivered.png`；
  `item-note-I-reference.png` / `item-note-I-ours.png`；`remarks-tick-I-reference.png` / `-ours.png`；
  `white-wipe-I-reference.png` / `-ours.png`；`pulsation-damper-I-reference.png` / `-ours.png`。
- 提案：`proposal\0001-paper-word-and-white-on-the-sheet.patch`、
  `proposal\0002-round-caps-and-the-eight-unit-pen.patch`；实验读数 `proposal\probe-paint-with-colour-fix.txt`、
  `proposal\probe-edges-with-both-fixes.txt`；测试日志 `proposal\tests-with-colour-fix.log`、
  `proposal\tests-with-both-fixes.log`、`proposal\tests-root-with-colour-fix.log`、
  `proposal\export-plant-backup-with-colour-fix.log`、`proposal\tdd-0001-*.log`；
  实验导出 `proposal\ours-colour-fix\`、`proposal\ours-both-fixes\`；实验渲染
  `proposal\title-block-flashB-ours-colour-fix.png`、`proposal\whole-A2-ours-colour-fix.png`、
  `proposal\item-note-I-ours-colour-fix.png`、`proposal\white-wipe-I-ours-both-fixes.png`、
  `proposal\pulsation-damper-I-ours-both-fixes.png`。
