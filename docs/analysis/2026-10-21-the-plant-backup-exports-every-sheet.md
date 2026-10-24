# 备份里的 53 张图全部出 PDF：字体只嵌子集，符号路径容忍前导空格（2026-10-21）

上一轮把仓库自带的 7 个 `.pid` 都出成了 PDF。完整数据在
`D:\work\plant-code\cad\pid-test-data`：`验收数据/` 是五张参考图（PDF + JPG + 参考出图），
真正的全量在 `PlantData~2~711.zip`（21.4 MB）里 —— 一个装置的 **53 张图**，文件名是 GBK。
把它们全部导出，才发现三件事：一张图**写不出**、每页 **6.8 MB**、以及 82 处符号
**看得到名字却找不到文件**。

改完：**53/53 出页**，每页 **175–846 KB**（改前 6.6–7.1 MB／合计 366 MB → 25.7 MB），
缺符号从 **82 处降到 18 处**，五张参考图的读数逐位未动。

## 〇、把备份拆成可跑的数据

`PlantData~2~711.zip` 的条目名是 CP437 里的 GBK 字节，Info-ZIP 直接解会得到乱码。用
`zipfile` 自己按 GBK 解码：

```python
import zipfile, pathlib
z = zipfile.ZipFile('PlantData~2~711.zip')
for info in z.infolist():
    raw = info.filename
    name = raw if info.flag_bits & 0x800 else raw.encode('cp437').decode('gbk', 'replace')
    if info.is_dir():
        continue
    path = pathlib.Path('backup-plant-data') / name
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(z.read(info))
```

拆出来 53 个 `.pid`（`00/00`…`00/02` 三层），合计 **86,814 个图元**，页幅分布：

| 声明的页幅（mm） | 张数 | 说明 |
|---|---:|---|
| 841.00 × 594.00 | 20 | A1 |
| 594.33 × 420.57 | 9 | 图纸里写的就是这个数，不是 A2 |
| 1189.00 × 841.00 | 7 | A0 |
| 594.00 × 420.00 | 7 | A2 |
| 841.15 × 594.40 | 2 | |
| 1189.31 × 841.31 / 843.46 × 596.71 / 843.09 × 596.09 / 842.54 × 595.54 | 各 1 | 图纸自带的自定义幅面 |
| 未声明 | 4 | 由内容落回 ISO：A0 ×1、A1 ×2、A2 ×1 |

自定义幅面（1189.31、843.46、594.33 …）是**图纸自己声明的**，不是我们算出来的，所以照原样
出页；只有未声明的四张走 `resolved_page`，都落到了正好的 A0/A1/A2。

## 一、`D03`：一张有页幅、没有内容的图

跑第一遍 52 成 1 败：

```
FAIL D03  D03.pid: font: no font was provided
```

`D03` 的 Sheet 里**一个图元都没有**（发出 XML 的那条链也是 0 个对象：`D03_Data.xml` 只有
458 字节、0 个 `Entity`），页幅却写着 594.33 × 420.57。`prepare_fonts` 按「模型里出现的
文字」取字体，没有文字就一个面都不载；`write_plot` 于是拿空的字体表当错误 —— 但这一页根本
没有字要画。

`write_plot` 现在只在**确实有字而没有面**时拒绝：

```rust
if fonts.is_empty() && has_inked_text(model) {
    return Err(PlotWriteError::Font("no font was provided".into()));
}
```

`has_inked_text` 与 `prepare_fonts` 用同一个判据（`PaperShape::Text` 且值非空），所以
「载了零个面」只可能意味着「一个非空文字也没有」。`D03` 现在输出 722 字节的 A2 空白页；
有字而没面的情况仍是硬错（`lettering_without_a_face_is_refused`），免得整页标签被悄悄丢掉
却看起来像成功。

## 二、每页 6.8 MB：`klippa` 把位图表整段带过来了

改前每页 6.6–7.1 MB，53 张合计 **366 MB**；参考出图同类页只有 50–120 KB。`pdffonts` 一看：

```
name      type           emb sub
Arial     CID TrueType  yes  no
SimSun    CID TrueType  yes  no
```

「嵌进去了，但不是子集」。`fonts::subset_or_whole` 的逻辑是「先子集，子集不可用再整份嵌」，
所以问题在于子集自身太大。新探针 `pid-plot/examples/probe_font_subset` 把每一步单独打出来：

```
simsun.ttf: 18314228 bytes, validate Err("the selected font exceeds 16 MiB")
subset: 6366516 bytes, validate Ok("ok")
```

子集只有 6.36 MB，和一页 6.8 MB 对得上。查表结构就明白钱花在哪：

| 表 | 原文件 | 说明 |
|---|---:|---|
| `glyf` | 11,753,556 | 轮廓 |
| `EBDT` | 5,532,156 | 单色点阵 |
| `EBLC` | 683,696 | 点阵索引 |

`EBDT + EBLC` ≈ **6.2 MB**，而子集无论要几个字都是 6.36 MB —— `klippa` 会把
`EBDT` / `EBLC` / `EBSC` 原样透传（它没有点阵子集实现）。PDF 画的是轮廓，从不画点阵 strike，
所以 `subset_face` 把这三张表放进 `drop_tables`：

| 面 | 原文件 | 子集（`PT 1001101A 阀泵换热器 低温分离`）| 改动 |
|---|---:|---:|---:|
| SimSun | 18,314,228 | **150,632** | −99.2% |
| NSimSun | 18,314,232 | **153,376** | −99.2% |
| FangSong | 10,570,564 | **73,652** | −99.3% |

三份子集都通过 `pdfcraft_fonts::embedding::validate`，且覆盖了请求的每一个字（探针现在也
打印「未覆盖字符」）。整页随之落到 175–846 KB：`D01` 175 KB…`EF井场` 846 KB，空图 `D03`
722 B；53 张合计 25.7 MB。

单测 `a_cjk_face_is_subset_to_the_drawing_not_embedded_whole` 在没有面文件的机器上软跳过，
有面时断言 SimSun 子集 < 2 MiB 且仍覆盖 `阀`、`泵`、`℃` 一类字。

## 三、82 处「有名字、没文件」：路径里多了一个空格

审计把每个缺失的符号连同**完整相对路径**打出来，其中两条最显眼：

```
piping\piping opc's\ off-unit sq.sym                      43 处
piping\specialty components\strainers\ filter type 2.sym  21 处
```

两条都是**最后一段前面多一个空格**。库里明明有 `Piping/Piping OPC's/Off-Unit SQ.sym`
和 `Piping/Specialty Components/Strainers/Filter Type 2.sym`。翻 `.pid` 的 UTF-16 字符串
就清楚了 —— 存进去的就是带空格的：

```python
'\Symbols\Piping\Piping OPC's\ Off-Unit SQ.sym'
```

而厂商自己的 `RefData~4~681.zip` 里，这两个条目**也是带空格的**
（`Piping/Piping OPC's/ Off-Unit SQ.sym`、`.../Strainers/ Filter Type 2.sym`）；是解压到
磁盘那一步把行首空格吃掉了，于是「图纸的说法」和「磁盘上的文件名」只差一个空格。

`symbol_library::read_from_roots` 现在按两个候选找：先照存的写法，再把每个路径段的**前导
空格**去掉（`symbol_candidates`；只有确实需要去空格时才有第二个候选，含内部空格的名字不受
影响）。43 + 21 处符号因此画上纸，五张参考图读数逐位不变（它们的修订版里没有这两处引用）。

剩下 18 处是**符号库里没有、但画面上并不缺**：`.pid` 自带符号本体的缓存（
`NormalizedPidGeometry::symbol_definitions`，`lay_out_drawing` 优先用它），这 18 处都靠图内本体画了出来。53 张逐张查
`PaperModel::missing_boxes`（既无库本体、也无内嵌本体、只好画占位框的集合）：**全部 0**。

| 库里没有的符号（图纸里存的写法） | 处数 | 同一路径上库里有什么 |
|---|---:|---|
| `Piping\Specialty Components\Misc\Pulsation Damper.sym` | 13 | 什么都没有 |
| `Equipment\Other Equipment\Wellhead Christmas Tree.sym` | 4 | 只有 `Wellhead Christmas Tree 01.sym` / `02.sym` / `Wellhead Control Panel.sym` |
| `Instrumentation\Off-Line\Without Implied Components\Temperature\Clone of TT-Temperature Transmitter(1).sym` | 1 | 只有 `TT-Temperature Transmitter.sym` / `TT-Surface Temperature Transmitter.sym` |

「什么都没有」是穷举过的（2026-10-22）：把 `pid-test-data` 整个目录按名字扫一遍（含解出来的
`Ref/Symbols`、`验收数据/Ref`、`backup-plant-data`），再把每个以 `PK` 开头的文件当 zip 递归
打开（7 个 `.zip` 加上没有扩展名、实际是 zip 的 `RefData~4~703`，共 **861 条**条目，含包内嵌
包），三条路径 **0 命中**；同名前缀只在 `RefData~4~681.zip` 里命中上面那 5 个近似文件。顺带
确认了另外三个 `RefData~4~*` 不是符号库：`~680` 是 332 KB 的规则表 CSV，`~683` 是一张
`Sheet6`/`PSMcluster` 齐全的图纸模板（CFB），`~709` 是 XML。仓库的 `test-file/symbols-full`
与语料 `D:\work\cad\PID自研软件验证\Ref` 同样 0 命中。

`Export.dmp` 里能搜到这三条路径本身——说明**数据库知道它们被引用**（`…\Pulsation Damper.sym`
35 行、裸名 `…\Wellhead Christmas Tree.sym` 13 行、`Clone of TT-Temperature Transmitter(1).sym`
2 行），但那是放置行/引用，不是几何：包里确实没随发这三个 `.sym`。图纸里每条放置会出现 4 次
路径字符串（符号定义 + 放置各一对），所以 52 次 = 13 张图各 1 处、16 次 = 4 张图各 1 处、
`D07.pid` 1 处，正好是库里缺的那 18 处——不是画面上缺的。

`pid-plot/tests/export_plant_backup.rs` 把这三条钉成白名单（比对用小写规范化后的路径）：再多
缺一个就红；同时钉住 `missing_boxes` 为空——哪天某个放置既取不到库本体、连图内本体也没了，
门禁会立刻红。**没有**按 01/02 或去掉 `(1)` 去顶替库：那是猜测，不是证据；这两处图上本来
就画着图纸自己存的本体，不需要猜。

## 四、五张参考图：一个数都没动

备份里的 `集注站` 五张是**更新的修订版**（字节数与 `验收数据/` 里的不同），所以参考对比仍以
`验收数据/*.pdf` 为准，用新版 `.pid` 重跑：

| 图 | 覆盖率 | 多画 mm | 文字 | 页幅 |
|---|---:|---:|---:|---|
| I井场 | 0.9989 | 41.8 | 544/546 | 1189 × 841 |
| 一级闪蒸分离器A | 0.9998 | 7.6 | 151/151 | 841 × 594 |
| 一级闪蒸分离器B | 0.9979 | 1.5 | 123/123 | 841 × 594 |
| 低温分离器A | 0.9986 | 22.9 | 167/167 | 841 × 594 |
| 换热器CD | 0.9993 | 24.6 | 249/249 | 未声明 → 841 × 594 |

与上一轮完全一致：换行拆分、联合符号库、推断几何不上纸都不受影响，字体子集只改**嵌入方式**，
不改字形、字宽或落点（`write_five_drawings` 逐段比对读回的 span 仍是原样）。

## 五、门禁与工具

- `pid-plot/tests/export_plant_backup`（新增）：找到 `backup-plant-data`（`PID_TEST_DATA_DIR`、
  语料目录旁、`D:\work\cad\pid-test-data`、`D:\work\plant-code\cad\pid-test-data` 依次找）
  时，把 53 张逐个「布局 → 字体 → 写 → 读回」，断言 —— 页幅与声明一致、
  `off_sheet_primitives == 0`、没有带断行的标签进页、缺失符号 ⊆ 三条白名单、每页 < 2 MiB
  （点阵表回归会立刻超线）、非空图必须读出墨和字。没有该目录（CI）时软跳过，实测 70.7 s、
  `53 backup drawings exported, 4 faces embedded`。
- `pid-plot/src/fonts.rs` 新增子集上限单测（见上）。
- `pid-plot/src/write.rs` 新增两条：空图仍写声明页幅、有字无面仍报错。
- `pid-plot/examples/probe_font_subset`（新增）：把 parse / plan / subset / validate 每步的
  大小与错误打出来，是这次定位 `EBDT` 的探针。
- `pid-plot/examples/probe_export_audit` 全量跑：`53 drawings, 0 failed`。
- `pid-plot/examples/probe_cached_bodies_fill_library_gaps`（新增）：逐图打 `primitives` /
  `library_gaps` / `undrawn`，回答「库里没有的那个符号到底画上了没有」。53 张实测：
  `primitives=96490 library_gaps=18 undrawn=0`。

## 六、仍未闭合

- **18 处符号不在库里**（画面上不缺，见上表）：三条文件名在整个数据包里不存在，要么拿到那份
  项目库，要么就是图纸引用了一个未随包发布的符号。真正的风险不是缺符号，而是**库本体与图内
  缓存体不一致时无从察觉**：今天优先用图内本体，如果装上那份项目库、两边画得不一样，只有
  一拼才知道。
- **自定义页幅**：9 张声明 594.33 × 420.57、1 张 1189.31 × 841.31 等，是图纸自己的数，我们
  照原样出页；但没有这些图的参考出图，无法确认厂商出图时是否也印这个尺寸（还是归到 A2/A0）。
- **参考出图只有 5 张**：其余 48 张只有结构与审计（页幅、图元普查、符号覆盖、读回段数），
  没有像素级对照。
- **`D03` 是空图**：空白页是对的，但如果哪天解码器漏了一个族，也会表现为「空图 + 有页幅」。
  `export_plant_backup` 目前无法区分这两者 —— 要有 `D03` 之外的参考才能钉住。

## 七、把符号库和图纸自带的本体逐条比一遍（2026-10-22）

同一个符号现在有两份几何：图纸自己存的那份（`symbol_definitions`，`lay_out_drawing` 优先用）
和分享目录里的 `.sym`。两者不一致时到底谁对？探针
`pid-plot/examples/probe_library_vs_cached_bodies` 逐条比：只比形状（文字是实例数据，库里的
`NULL` 占位不算），比每种图元的条数和局部包围盒（容差 1 µm），参数化本体（`template`
非空，尺寸由实例变量决定）另计。

```
cargo run -p pid-plot --example probe_library_vs_cached_bodies -- <图或目录>...
```

53 张备份去重后 1846 个放置本体，其中 1828 对两边都有：

| 结果 | 数量 | 占比 |
|---|---:|---:|
| 完全一致（种类数 + 包围盒）| 1069 | 58.5 % |
| 参数化本体，尺寸本就不一样 | 50 | 2.7 % |
| 不一致 | 709 | 38.8 % |
| 库里没有这个符号 | 18 | — |
| 只能靠库（图里没存本体）的放置 | **0** | — |

7 张 fixture 再跑一遍，同一结论：68 对里 43 一致、6 参数化、19 不一致、3 库缺，只能靠库的
放置同样是 **0**。

差异的形态大多数很轻：库里多 1 条线 158 处、多 3 条 88 处、少 2 条 89 处；124 处包围盒完全
相同（同一轮廓里笔画数不同）；115 处种类数相同但包围盒不同（多半是符号修订改了尺寸）；
最极端的是 `Wellhead Christmas Tree 02.sym`：缓存 45 线 + 23 圆 + 1 B 样条 + 11 填充，库这边
只剩 1 条 B 样条。

**哪边是对的？缓存那边有参考出图背书。** `I井场` 是五张参考图之一（覆盖率 0.9989），它的 31
处不一致里就包括 `Wellhead Christmas Tree 02`；纸上画的是缓存本体，而库本体经
`displayed_primitives()` 只剩一条曲线。覆盖率 0.9989 说明厂商打印的正是缓存那份，
所以「图内本体优先」这次是证据支持的选择，不是随便定的。

**库侧那个隐患当天就查清了**：`displayed_primitives()` 在这 1828 个本体上滤掉
**5356 / 15316 条笔画（35 %）**，其中 **411 个本体丢掉一半以上**
（`Wellhead Christmas Tree 02.sym` 84 → 1、`Check Valve Type 2.sym` 16 → 1、
`Horizontal Drum.sym` 5 → 1、`Concentric Reducer.sym` 5 → 1）。原因不是图层读错，而是
「没有正常视图」时兜底选了 `Sheet6`：那份 `.sym` 只有一个视图集、管的却是 `Sheet374`（位图全开，
被认成备用视图），而 `Sheet6` 只是 1 条 B 样条的存根。已改（只改这一支：`normal` 为空时取
按自己视图集算笔画最多的表），一致率 58.5 % → 70.1 %。今天没有任何一张图依赖库本体
（`library_only_placements = 0`），所以**页面一个像素没动**；改的是「库当后备时会画成什么样」。
根因、打分与改后的数见 `docs/analysis/2026-10-22-the-sheet-pick-that-shrank-a-symbol.md`。
