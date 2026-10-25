# 出图对比收口：占位文字、图框遮罩、棘轮（2026-10-12 开单，同日 Plannotator 批准，无批注）

> 用户 2026-10-12（zhimo 会话 grok-4.7-102）：「分析现在 pdf 导出的实现进度，制定下一步的开发方案，使用 plannator 制定计划」。
> 开单只写本文件，不改 `pid-plot` 代码，不提交。
> **2026-10-12 Plannotator 批准**（`{"decision":"approved"}`，无批注）。D1–D3 按推荐项：不画整段 `NULL`、位号另开主库单、图框遮罩从参考 PDF 的页边文字量。批准之后才按 §4 落地。
> 上一单是 `docs/plans/2026-10-12-rendered-plot-and-plot-comparison.md`（grill Q1–Q17）。那份里程碑表停在「M3 工作区未提交、M4 起还没做」，和树不一致。本文件替换它的进度，不改它的决定。
> 基线：`codex/phase32c-bundle-closeout` @ `840397b`。M3/M4 的源码在工作区，未提交。
>
> **2026-10-15 已落地。** S0 的 `352ece8`（写）、`150ab41`（比）、`b3e9a11`（ADR 与上一份计划）之后，S1–S4 一并落地：`bef63fa`（S1 占位 `NULL`，含 `NULLNULL`）、`e56d061`（段落锚点：居中 / 右对齐沿基线回退，并加语料上一层的 `Ref` 库查找）、`720923c`（读回器把 `1"` 与 `NO` 分成两段；字体子集各用独立临时目录）、`16c77d8`（S2 图框遮罩 + S3 出处，含 `pid-plot/assets/template-masks.json` 与两页分析）、本轮最后一笔（S4 棘轮 `tests/plot_compare.rs`、叠图 PNG 断言，以及这一行）。**S2 与 S3 合成一笔**：两份改动落在同一个 `compare.rs` 的同一批结构体上，硬拆会留下编译不过的中间提交。锚点、读回器分句、字体子集清理不在本单范围，是工作区里带着的后续迭代，随 `e56d061`、`720923c` 一并落地。
>
> 验收（2026-10-15，`cargo test --locked -p pid-plot` 35 通过）：五张我方 PDF 各 1 页，纸面 1189×841 / 841×594 mm，独立 `NULL` 段 0（PyMuPDF 复核）；五张拟合都在阈值内；`tests/plot_compare.rs` 钉住覆盖率、多画、文字对上三项。两页分析的数字是当次重跑的读数，见 `docs/analysis/2026-10-12-plot-comparison-i-wellsite.md`、`docs/analysis/2026-10-12-template-mask-samples.md`。§1 表的「工作区，未提交」以本行为准。

## 一句话

线已经对上，文字对比现在没有信息量。五张图的拟合都在阈值内，线覆盖率 0.74–0.94；文字对上是 0 或 1，因为我方出图把符号模板里的占位 `NULL` 画了出来。下一步先让文字对比说得清，再钉三项数。线宽、虚线、字体对照仍留在第二阶段。

## 1. 进度（对照源码和已写出的报告，2026-10-12 晚）

| 步 | 状态 | 落点 |
|---|---|---|
| M0 骨架 | 已提交 `c555a53` | `pid-plot` 成员；pdfcraft 钉在 `0e55d07` |
| M1 读参考出图 | 已提交 `e408267` | `pid-plot::read`；五张线段 / 文字条数钉在 `docs/analysis/2026-10-12-reference-plot-ink.md` |
| M2 纸面图元表 | 已提交 `840397b` | `pid-plot::model`、`lay_out_drawing` |
| M3 写我方出图 | 工作区，未提交 | `pid-plot/src/write.rs`、`src/bin/pid_plot.rs`、`tests/write_five_drawings.rs`。笔画固定 0.25 mm。往返按端点对纸面表 |
| M4 出图对比 | 工作区，未提交 | `pid-plot/src/compare.rs`、`src/bin/pid_plot_compare.rs`、`tests/compare_five_drawings.rs`。报告已在语料 `plot/compare/`。叠图 PNG 不在盘上 |
| M5 棘轮和分析页 | 没做 | 没有 `tests/plot_compare.rs`，没有 `docs/analysis/` 差异页 |
| M6 换 SQPlant 符号库 | 等用户拷 `Ref\` | 不动 |

未提交、本单不碰、留给落地时按步 `git add` 的路径：

- `pid-plot/Cargo.toml`、`pid-plot/src/lib.rs`、`pid-plot/src/model.rs`（已改，含 30 mm 缺符号方框）
- `pid-plot/src/write.rs`、`pid-plot/src/compare.rs`
- `pid-plot/src/bin/pid_plot.rs`、`pid-plot/src/bin/pid_plot_compare.rs`
- `pid-plot/tests/write_five_drawings.rs`、`pid-plot/tests/compare_five_drawings.rs`
- `docs/adr/0004-pid-parse-draws-its-own-rendered-plot.md`、`docs/adr/0005-plots-are-written-and-read-with-pdfcraft-in-pid-plot.md`
- `docs/plans/2026-10-12-rendered-plot-and-plot-comparison.md`

`--model-json` 仍然没有。`lay_out_drawing` 只打印条数。

### 1.1 五张报告里的三项数

报告在 `D:\work\cad\PID自研软件验证\plot\compare\`。拟合全部在 Q9 内（缩放偏离 1 不到 0.02%，平移不到 0.05 mm）。

| 图 | 参考线长 mm | 覆盖率 | 多画 mm | 文字对上 | 我方文字条数 | 其中含 NULL |
|---|---:|---:|---:|---:|---:|---:|
| I井场 | 28841 | 0.7712 | 8368 | 0 / 507 | 266 | 214 |
| 一级闪蒸分离器A | 9622 | 0.9393 | 2281 | 1 / 152 | 37 | 27 |
| 一级闪蒸分离器B | 10478 | 0.7353 | 2262 | 0 / 147 | 37 | 27 |
| 低温分离器A | 10114 | 0.9172 | 2498 | 0 / 171 | 59 | 44 |
| 换热器CD | 14565 | 0.9348 | 3016 | 0 / 250 | 105 | 82 |

I井场我方文字去重只有 10 个字符串，出现最多的是 `NULL` 115、`NULLNULL` 99，然后是 `PG` `PT` `M` `TG` `TTT` `LTDP`。和参考文字相同的只有 `M` `PG` `PT` `S` `TG`。

参考侧没对上的开头几条是图框会签栏，落在纸面左侧 x ≈ 24 mm：`日期`、`修改号`、`签署`、`会签专业`。现在的模板遮罩只盖右下角 180 × 50 mm，盖不住这一列。四张 A1 的 `.masks.json` 是空数组，缺符号方框没有套进去；I井场那份有 58 个方框。

### 1.2 为什么文字对不上

`src/symbol_library.rs` 把符号自带的 `igTextBox` 解成原文，并写明：库是模板，内容经常是占位 `NULL`，表示放置时由图纸填值，画不画由出图的人定。`pid-plot::model` 现在把这段原文放进纸面表，`write` 再写进 PDF。相邻两条 `NULL` 被读回时粘成 `NULLNULL`。

图纸自己的 `PidGraphicKind::Text` 是另一路，高度来自记录。短字符串（`PG`、`PT`、`LTDP`）在这一路里。设备位号、管线号不在我方这 10 个字符串里，参考 PDF 里有。那是解析缺口，不是本单改主库能顺手补上的。

## 2. 范围

**做**：占位 `NULL` 不进我方出图；五张重新出图、重新对比；图框遮罩按参考 PDF 的页边文字量，A0 / A1 各存一份矩形；五张都套缺符号方框；差异带上出处；三项数棘轮；I井场分析页；叠图 PNG 落盘。

**不做**：第二阶段外观（线宽三档、虚线节距、宋体 / 仿宋 / Arial Narrow）；改 pid-parse 主库把设备位号填进符号；画 `.igr` 图框和标题栏；换 SQPlant 符号库（M6，仍等 `Ref\`）；OpenCADStudio；把现在的 0 对上钉成基线。

## 3. 要拍板的

| # | 问题 | 选项 | 推荐 |
|---|---|---|---|
| D1 | 符号正文里整段就是 `NULL` 的字怎么处理 | (a) 不放进纸面表，因此不进 PDF、不进三项数。图纸自己的 `PidGraphicKind::Text` 即使内容是 `NULL` 也照画。`HH=NULL` 这种还带着字的照画。(b) 仍画，但放进第三个图层，对比时整层排除 | **(a)**。库的注释已经把「画不画」交给出图方；第三条图层会把还没信息量的字留在文件里 |
| D2 | 参考 PDF 里有、图纸文字记录里没有的位号 | (a) 分析页记成「解析：放置时填值还没进几何」，另开主库单，本单不造字。(b) 本单就从标注依赖里把位号填进纸面表 | **(a)**。上一单定了对比发现的解析问题另开单 |
| D3 | 图框遮罩 | (a) 从五张参考 PDF 的页边文字簇量矩形，A0、A1 各一份，放进仓库。不等 `.igr`。(b) 继续用右下角 180 × 50，等模板文件 | **(a)**。会签栏在 x ≈ 24 mm，现遮罩盖不住，文字对上会被图框拖死 |

## 4. 步骤

每步一个提交，只 `git add` 本步路径。门照上一单：workspace 的 build / test / clippy，`cargo fmt --all -- --check`，missing-docs，外加只对 `-p pid-parse` 的 `--no-default-features` 和 wasm32。别的会话弄脏的文件不收进提交。开单这一轮不做这些。

### S0 把已有的 M3、M4 落成两个提交

**已落地** `352ece8`（写）、`150ab41`（比）、`b3e9a11`（ADR-0004、ADR-0005、上一份计划）。文档单独一笔，不和代码混。本文件留到 S4。

写：`write.rs`、`pid_plot.rs`、`write_five_drawings.rs`、`lib.rs` 的 `write` 模块。比：`compare.rs`、`pid_plot_compare.rs`、`compare_five_drawings.rs`、`model.rs` 的 30 mm 缺符号方框、`png`。`cargo test -p pid-plot --lib` 9 通过。平移 10 mm 的单测原来把参考线放在两条我方线正中间，最近邻配错，覆盖率是 0；改成线距 40 mm、平移 10 mm，每条只有一个邻居。拟合代码没改。

### S1 占位 `NULL` 不画（D1）

`pid-plot/src/model.rs` 展开符号时，空白归一后整段等于 `NULL` 的 `SymbolPrimitive::Text` 不放进纸面表。图纸文字记录不走这条。

单测两条：符号里只有 `NULL` 时纸面表没有这段字；符号里是 `HH=NULL` 时字还在。

然后重跑 `tests/write_five_drawings.rs` 和 `tests/compare_five_drawings.rs`。预期我方文字条数去掉 NULL / NULLNULL 那一截，I井场从 266 降到几十条，对上条数允许仍接近 0（图框和位号都还在）。

### S2 遮罩（D3）

- 从参考 PDF 读回的文字里，把贴着页边的簇收成矩形。I井场先核对会签栏（x ≈ 24 mm 那一列）落在矩形内。A1 四张用同一套 A1 矩形，不各画一套。
- 矩形进仓库，例如 `pid-plot/assets/template-masks.json`，测试读它，不再用写死的 180 × 50 当唯一遮罩。右下角标题栏若量出来就留着。
- 五张都把 `missing_boxes` 套进遮罩。分离器 A 今天 `masked_reference_length_mm = 0`、遮罩文件是空数组，这一步之后不该再是这样。
- 缺符号仍是插入点周围 30 mm 方框。不在本步改尺寸。

### S3 出处和分析页（D2）

对比报告里，没对上的我方文字带上纸面表已有的 `stream_path`、`byte_range`、`confidence`、`symbol`。没盖住的参考线长按所属符号或「图纸线 / 图框」归堆，不要求每一毫米都有字节区间。

分析页 `docs/analysis/2026-10-12-plot-comparison-i-wellsite.md` 写 I井场：

- 拟合参数和三项数（S1、S2 之后的新数）。
- 我方文字去重表：哪些是图纸文字，哪些被 S1 丢掉。
- 参考里有、我方没有的成组字符串（位号、管线号、图框）。位号标成「解析，另开单」。
- 多画的线长里，缺符号遮罩内外各多少。遮罩外最大的几处写到符号名或流路径。

### S4 棘轮

`pid-plot/tests/plot_compare.rs` 钉五张的覆盖率、多画毫米、文字对上条数，只升不降。数字用 S1–S3 重跑之后的数，不用 §1.1 这张旧表。语料不在就跳过。

叠图写到 `plot/compare/*.png`（参考蓝、我方红）。`compare_five_drawings` 断言 PNG 大于 100 字节；今天目录里没有 PNG。

本文件补一行「已落地」和提交号。

## 5. 验收

- 五张我方 PDF 里不再出现单独的 `NULL` / `NULLNULL` 段。`HH=` 这类符号自带字还在。
- 五张拟合仍在 Q9 内。盖住会签栏之后，I井场参考文字条数少于今天的 507。
- 三项数有测试钉住，分析页能把 I井场最大的差异分成解析、排版、参考数据缺。
- 主库解析函数没有为了本单改行为。wasm32 和 `--no-default-features` 仍只跑 `-p pid-parse`。

## 6. 风险

- 只删 `NULL` 之后文字对上仍可能接近 0。那就是位号和图框，S3 写清楚，不用三项数装成「文字已对齐」。
- 页边矩形量宽了会把工艺文字算进「参考数据缺」。矩形旁要附一份被盖住的字符串样本。
- 重出图依赖 `C:\Windows\Fonts\NotoSansSC-VF.ttf` 和 python 子集化。字体不在就跳过，和今天的写测试一样。
- S0 提交时工作区里还有别的会话的文件。只加 §1 列出的路径。
