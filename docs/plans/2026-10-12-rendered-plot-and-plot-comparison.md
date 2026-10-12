# 我方出图与出图对比 · 开发计划（2026-10-12 开单）

> 用户 2026-10-12（zhimo 会话 opus-5-5-12）：以 `D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pdf` 为参考，「将 pid-parse 通过我们的解析，然后转换成 pdf 格式，从而来实现格式的对比。使用 grill-with-doc 帮我制定开发方案」；第 3 轮前又定「使用 https://github.com/storytold/pdfcraft」。三轮 grill 的 Q1–Q17 全按推荐定下，只有 Q2 改选 B。
> 开单基线：pid-parse `a287078`（`codex/phase32c-bundle-closeout`）。开单时工作树上已有别的会话的 8 M + 11 ??，本单一个字节不碰。
> 已落地：M0 `c555a53`、M1 `e408267`、M2 `840397b`（2026-10-12）。M3 的 `pid_plot` 已在工作区，还没提交。M4 起还没做。本文和两份 ADR 仍未提交。
> 词表见 `CONTEXT.md`：参考出图、我方出图、出图对比。决定见 ADR-0004（pid-parse 自己画，不经 OpenCADStudio）、ADR-0005（用 pdfcraft 写和读，只在 `pid-plot` 里）。
> 与 publish 组的目标（`docs/plans/2026-10-11-publish-native-evidence-via-ida-bridge.md`）是两条线，互不改动（Q6）。

## 一句话

在 pid-parse 仓库里新开 `pid-plot` crate：把 pid-parse 解析出的几何、样式和 `.sym` 符号本体排成纸面图元，用 pdfcraft 写成我方出图；再用 pdfcraft 把参考出图和我方出图都读回成「墨线段 + 文字」，按纸面容差配对，五张图各出三项数并按棘轮钉住，另出一张叠图给人看。
第一阶段只比内容（线、字、符号有没有、画在哪）；线宽、虚线节距、字体留到第二阶段。

## 1. 现状

### 1.1 参考出图

| 图 | 图幅 | 链接的模板 |
|---|---|---|
| I井场 注采阀组工艺及自控流程图 | A0（1189 × 841 mm） | `CPE-SQ-A0.igr` |
| 一级闪蒸分离器A / 一级闪蒸分离器B / 低温分离器A / 换热器CD及外输阀组 | A1（841 × 594 mm） | `CPE-SQ-A1.igr` |

- 五张都是 2026-10-12 11:53–11:59 由 SmartPlant 经 Amyuni PDF Converter 4.5.2.9 打出的单页纯矢量 PDF（PDF 1.3），没有图片。
- 打印驱动已经把圆拆成折线、把虚线拆成一段段实线：PDF 里没有贝塞尔曲线，也没有虚线样式。
- I井场：792 条路径、11,291 段直线、2 个四边形；线宽只有 0.48 / 0.96 / 1.92 pt（0.17 / 0.34 / 0.68 mm）三档，另有 80 条只填不描的路径；590 段文字可直接抽出（ArialNarrow 483、Arial 48、SimSun 40、FangSong 17、NSimSun 2），字体都嵌了子集。另外四张的数在 M1 记。
- 之前对图用的 JPG 不再作参考（Q5）。

### 1.2 `.pid` 里有什么、没什么

- 有：几何（`NormalizedPidGeometry`：线、弧、折线、点、文字、符号实例，每个带出处和 Decoded / Inferred / ProbeOnly 置信度）；单位（米）和图幅（`page_dimensions_mm`）；线、填充、文字样式（`style_link`）；符号实例的 `symbol_path`、旋转、插入点。
- 没有图框和标题栏：每张 `.pid` 只存了 `JSite23/\x01Ole` 一个 OLE 链接，指向 `C:\PID_SQProject\SQPlant\Ref\Template Files\CPE-SQ-A0.igr`（I井场）或 `CPE-SQ-A1.igr`（另外四张）。
- 没有符号本体：在外部 `.sym` 库里。
- 本机没有 `C:\PID_SQProject`，也找不到任何 `.igr`。唯一的符号库是 `test-file/symbols-full`（620 个 `.sym`），按文件名能对上的：I井场 39 / 59、分离器A 23 / 29、分离器B 23 / 28、低温分离器A 26 / 33、换热器CD 29 / 38。缺的多是项目自有符号（`off-unit sq`、`flange type 2`、`wellhead christmas tree 02` 等）。

### 1.3 现成的出图代码

- OpenCADStudio `src/io/pid`（约 3,600 行，不含测试）已经把上面这些排成可画的图元：`symbols.rs`（找库、展开符号）、`build.rs`（放置）、`styles.rs`、`text.rs`、`page.rs`（米换毫米、图框只画外框）。`pid-plot` 以它为参照另写一份，不依赖 OpenCADStudio（ADR-0004）。
- OpenCADStudio main 有无头 `--plot-svg`；`--plot-pdf` 在 `ocs-plotpdf` 工作树里暂存未提交，是别人的活，本单不碰。

### 1.4 pdfcraft（`storytold/pdfcraft` 0.6.0）

- MIT OR Apache-2.0；edition 2024；MSRV 1.95，与 pid-parse 相同；不在 crates.io。
- 写：`pdfcraft-cos`（对象）加 `pdfcraft-content`（内容流算子、`serialize_ops`）。`pdfcraft-create` 只做空白页、图片和纯文本，不用。
- 字体：`pdfcraft-fonts::embedding::embed` 整个嵌入 TrueType 字体，ToUnicode、CIDToGIDMap 和字宽一致；只收声明了开源许可的字体，不做子集。
- 读：`pdfcraft-render::text` 是一个 hayro Device，逐字给出 Unicode 和字框，坐标是左上原点、y 向下的页面视图空间；线要照同样的办法自己挂一个 hayro Device 收集。`pdfcraft-render` 也能渲位图。
- `pdfcraft-compare` 只做按阅读顺序的逐词文本差异，不比几何，不用。

## 2. 范围

**做**：`pid-plot` crate；读参考出图；纸面图元表；写我方出图；出图对比（矢量配对加叠图）；五张图的棘轮门；第一批差异的分析页。
**不做**：第二阶段的外观（线宽、虚线节距、字体对照，另开计划）；在本单里改 pid-parse 主库的解析（对比发现的解析问题另开单）；改 OpenCADStudio；画图框和标题栏（模板到手另议，见 §6）；比 PDF 文件结构（Q1）；往 `.pid` 写回。

## 3. 已定的决定

| # | 问题 | 定了什么 |
|---|---|---|
| Q1 | 比什么 | 先比内容：线、字、符号有没有、画在哪。第二阶段比线宽、虚线节距、字体、字高。不比 PDF 文件结构 |
| Q2 | 谁画 | pid-parse 自己画，不经 OpenCADStudio（ADR-0004） |
| Q3 | 怎么比 | 矢量配对出数当门槛，叠图给人看 |
| Q4 | 范围 | 五张图每次一起跑；修差异先从 I井场 下手 |
| Q5 | 参考 | 矢量 PDF 是唯一的图面参考，JPG 只留作历史；d-122 / d-1719 / d-1793 以后对着 PDF 重核 |
| Q6 | 与 publish 的关系 | 另开一条线，publish 目标不动 |
| Q7 | 参考数据 | 先用 `symbols-full` 开工：缺的符号和图框、标题栏区域单列「参考数据缺」，不算错。用户从 SmartPlant 机器拷来 `SQPlant\Ref` 的 `Symbols` 和 `Template Files`，放到 `D:\work\cad\PID自研软件验证\Ref\`（目录结构照原样），到了就换 |
| Q8 | 代码放哪 | 同一仓库新开 workspace 成员 `pid-plot`，依赖 pid-parse 主库；主库代码不动 |
| Q9 | 坐标对齐 | 每张图拟合一个整体平移加等比缩放，参数写进报告；缩放偏离 1 超过 0.2% 或平移超过 5 mm 判失败 |
| Q10 | 容差与门槛 | 线：纸面 0.5 mm。字：空白归一后字符串相同，且起点差不超过 1 mm。每张图三项数：参考线长覆盖率、多画的线长、文字对上条数。首跑钉成基线，只升不降，不设绝对目标 |
| Q11 | 虚线与圆 | 第一阶段只比中心线：共线、间隙小于 3 mm 的短段先连成一条；我们的弧按 0.05 mm 弦高拆折线 |
| Q12 | 置信度 | 画 Decoded 和 Inferred，各占一个 PDF 图层，报告分开计数；ProbeOnly 不画 |
| Q13 | 对比工具 | Rust，写在 `pid-plot` 里；门是集成测试，语料不在就跳过 |
| Q14 | 产物 | 我方出图写到 `D:\work\cad\PID自研软件验证\plot\`；报告和叠图写到 `plot\compare\`；结论写进 `docs/analysis/` |
| Q15 | pdfcraft 管哪一步 | 写和读都用它；PyMuPDF 只在起步时独立核对一次抽取数 |
| Q16 | 怎么引入 | pdfcraft 四块 git 钉在 `0e55d07`，只在 `pid-plot` 里；hayro / hayro-interpret / hayro-syntax / lopdf 用 `vendor/pdfcraft-pin` 的 path patch（ADR-0005） |
| Q17 | 第一阶段字体 | 开源许可的中文 TrueType 字体（Noto Sans SC / 思源黑体）整个嵌入；和参考字体对照留到第二阶段 |

## 4. 设计

### 4.1 crate 与数据流

```text
.pid ──pid-parse──▶ NormalizedPidGeometry + style_link + symbol_library (.sym)
                                   │
                                   ▼
            pid-plot::model  纸面图元表（mm，y 向上；每项带出处、置信度、所属符号）
                │
                ▼
            pid-plot::write ──pdfcraft──▶ 我方出图 .pdf
                                              │
            参考出图 .pdf ──┐                 │
                            ▼                 ▼
            pid-plot::read（pdfcraft-render / hayro）▶ 墨线段 + 文字（mm，y 向上）
                                   │
                                   ▼
            pid-plot::compare  归一 → 整体拟合 → 配对 → 遮罩 → 报告 JSON、差异清单、叠图 PNG
```

- 根 `Cargo.toml` 有 `[workspace]`。根包是隐式成员，`members` 里不要写 `"."`（一写，树里每个包都会变成成员）。现在是 `members = ["pid-plot"]`，并 exclude `vendor/oxidized-mdf` 和 `vendor/pdfcraft-pin` 下的 hayro、hayro-interpret、hayro-syntax、lopdf。
- wasm32 和 `--no-default-features` 只对 `-p pid-parse` 跑，写在 `AGENTS.md`。`.github/workflows/ci.yml` 里没有这两步，也不要把它们放进 `--workspace`。
- 已经在树上的程序：`read_reference_plot`（一张 PDF → 墨线段和文字条数）、`lay_out_drawing`（一张 `.pid` → 纸面图元和缺符号数）、`pid_plot`（一张 `.pid` → PDF，Decoded / Inferred 两个图层）。还没有：`pid_plot_compare`、门测试 `pid-plot/tests/plot_compare.rs`。`--model-json` 还没有。
- 语料目录沿用 `tests/publish_from_pid.rs` 的 `PID_PARSE_SELF_CHECK_DIR`（缺省 `D:\work\cad\PID自研软件验证`）。
- 符号库按这个顺序找：`PID_SYMBOL_LIBRARY`（与 OpenCADStudio 同名）→ `<语料>\Ref\` 下名字含 symbol 的目录 → `test-file/symbols-full`。报告写明用的是哪个库。
- 中文字体由 `PID_PLOT_CJK_FONT` 指定，字体文件不进仓库。

### 4.2 读（参考出图和我方出图共用一套）

- 描边路径：施加 CTM；曲线按 0.05 mm 弦高拆折线；全部拆成线段，记线宽。只填不描的路径取外轮廓。
- 文字：逐字按基线聚成文字段，记字符串、起点、字高、方向。
- `pdfcraft-render` 给的是 y 向下的页面视图坐标，读出来一律换成纸面毫米、y 向上，和纸面图元表同一个坐标系。

### 4.3 归一与配对（第一阶段）

- 两边都把共线、间隙小于 3 mm 的线段连成中心线，所以虚线拆没拆都一样。
- 整体拟合：用两边的长直线解平移加等比缩放（最小二乘，必要时 RANSAC 去掉离群），阈值见 Q9。
- 线：参考中心线每 0.5 mm 取一个采样点，离我们最近的线段不超过 0.5 mm 就算覆盖；覆盖率 = 覆盖长度 / 参考总长。反过来算我们多画的长度。
- 字：空白归一后字符串相同、起点差不超过 1 mm 算对上；没对上的两边各列一张清单。
- 遮罩（Q7）：模板区域（图框、标题栏、会签栏，A0 和 A1 各量一次，矩形存进仓库）和缺 `.sym` 的符号放置处（范围优先取 `.pid` 里记的符号外框，没有就取插入点周围的固定方框，尺寸在 M4 定）。遮罩内的参考线长和文字单列为「参考数据缺」，不进三项数。
- 每条差异都带我们这边的出处（流路径、字节区间、置信度、所属符号），用来分清是解析、排版还是参考数据的问题。

### 4.4 写

- 页面尺寸取 `page_dimensions_mm`；坐标从米乘 1000 到毫米，再换成 pt。
- 两个 OCG 图层：Decoded、Inferred；ProbeOnly 不画。
- 线宽、虚线、文字的字高、旋转和对齐都照样式表画；第一阶段画了但不比。
- 往返自检：我方出图经 §4.2 读回，必须和纸面图元表逐项一致。不一致就是写或读的毛病，先修，不进对比。

## 5. 里程碑

每步一个提交，只 `git add` 本步碰过的明确路径。门照 AGENTS.md 的五道（`cargo build` / `test` / `clippy --locked --workspace --all-targets`、`cargo fmt --all -- --check`、missing-docs 棘轮），外加只对 `-p pid-parse` 的 `--no-default-features` 和 wasm32 lib 检查。这两道不在 `ci.yml` 里。跑不了的标 NOT RUN 并写原因。

| 步 | 内容 | 完成判据 |
|---|---|---|
| M0 骨架 | **已落地** `c555a53`。`members = ["pid-plot"]`，不写 `"."`。pdfcraft 的 cos / content / fonts / render 钉在 `0e55d07b5fcc7d36747c7c56a6b98b1aabb9ce3e`。hayro、hayro-interpret、hayro-syntax、lopdf 用 `vendor/pdfcraft-pin` 的 path patch（同一提交的 `vendor/` 镜像）。wasm32 和 `--no-default-features` 写在 `AGENTS.md`，只跑 `-p pid-parse`，不改 `ci.yml` | `cargo tree -p pid-parse` 没有 pdfcraft。2026-10-12 傍晚：pid-plot clippy、workspace 测试、missing-docs 0/0、parser `--no-default-features` 850 通过、wasm32 check 退出 0。`cargo fmt --all -- --check` 被别的会话的 `examples/probe_oracle_exp_small_classes.rs` 挡住 |
| M1 读参考出图 | **已落地** `e408267`。`pid-plot::read` 和 `read_reference_plot`。五张的数在 `docs/analysis/2026-10-12-reference-plot-ink.md` | 测试 `reference_plots_match_pymupdf_counts` 钉住：I井场 11,291 / 590，分离器A 1,451 / 156，分离器B 1,662 / 165，低温分离器A 1,976 / 175，换热器CD 3,695 / 254。语料不在就跳过 |
| M2 纸面图元表 | **已落地** `840397b`。`pid-plot::model` 和 `lay_out_drawing`，符号库先用 `symbols-full`。`--model-json` 还没有 | 测试 `five_drawings_match_the_symbol_gap` 钉住 §1.2 的缺口：I井场 39/59，分离器A 23/29，分离器B 23/28，低温分离器A 26/33，换热器CD 29/38。放置、旋转、镜像、文字锚点有单测 |
| M3 写我方出图 | **工作区，未提交**。`pid-plot::write` 加 `pid_plot`。Decoded / Inferred 两个 OCG。笔画是固定 0.25 mm（纸面表还没有线型）。弧和圆按 0.05 mm 弦高拆成直线再写。字体由 `PID_PLOT_CJK_FONT` 整份嵌入；本机 Noto Sans SC 是超过 16 MiB 的可变字体，pdfcraft 拒收，测试里先实例化再按五张图用到的字子集 | 五张写到 `plot\`。往返：读回的线段和文字与纸面表逐项一致。PyMuPDF 打得开并抽到这些字。换热器 CD 的 `.pid` 没有页尺寸，按图元外框套进最小的 A 系列图幅 |
| M4 出图对比 | `pid-plot::compare` 加 `pid_plot_compare`：归一、拟合、配对、遮罩、报告、叠图 | 五张的报告和叠图写到 `plot\compare\`；拟合参数在阈值内，不在就作为一条发现写清楚 |
| M5 门与基线 | `pid-plot/tests/plot_compare.rs` 钉住五张的三项数，只升不降；分析页写第一批差异，按「解析 / 排版 / 参考数据缺」归类 | 门测试过；分析页列出 I井场 差异最大的若干处和归因 |
| M6 参考数据到位（等用户拷来） | 换成 SQPlant 的 `Symbols`；遮罩缩到只剩模板区域；模板画不画另开一轮 grill | 缺符号清零；重钉基线（只升） |

M5 之后按归因逐条修：解析问题回 pid-parse 主库另开单，排版问题在 `pid-plot` 里修，每修一处基线跟着升。内容差异收敛后，再开第二阶段的计划。

## 6. 还没定、等外部条件的

- 模板画不画、怎么画：`.igr` 和 `.sym` 是同一家的格式，大概率能用同一套解码器读（未验证）；标题栏里的图号、日期这些字是模板自带的还是图纸属性填进去的，要等模板到手再查。M6 时单独 grill。
- 第二阶段（外观）：三档线宽怎么从样式映射、虚线节距、宋体 / 仿宋 / Arial Narrow 能不能嵌（pdfcraft 的许可检查会拦）。M5 之后另开计划。
- d-122 / d-1719 / d-1793 对着参考出图重核：等 M4 有了对比工具再做。

## 7. 风险

- pdfcraft 是 0.x，每天在变：钉提交；升版单独一个提交，跑全量门。
- 写和读用同一个库，可能错得互相一致：M1 用 PyMuPDF 核数，M3 用 PyMuPDF 抽字兜底。
- 打印驱动可能带整体偏移或缩放：Q9 的拟合吸收掉，参数写进报告，超阈值算发现。
- 遮罩会藏住遮罩里的真问题：遮罩内的量单列；M6 之后遮罩只剩模板区域。
- `pid-plot` 的排版是 OpenCADStudio `src/io/pid` 之外的第二份实现，两边可能分叉（ADR-0004）。
- 整套中文字体嵌进去，每张 PDF 多十几 MB；只放在验证目录，不进仓库。
- pdfcraft 带进 hayro、lopdf、skrifa 等依赖，CI 编译时间会变长。

## 8. 复现

现在能跑（M1、M2、M3）。`pid_plot` 还要 `PID_PLOT_CJK_FONT` 指向一份不超过 16 MiB 的静态 TrueType/OpenType：

```powershell
$env:PID_PLOT_CJK_FONT = "C:\path\to\NotoSansSC-Regular.otf"
cargo run -p pid-plot --bin read_reference_plot -- "D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pdf"
cargo run -p pid-plot --bin lay_out_drawing -- "D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid"
cargo run -p pid-plot --bin pid_plot -- "D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid" "D:\work\cad\PID自研软件验证\plot\I井场 注采阀组工艺及自控流程图.pdf"
```

M4 落地后才有（程序还没有）：

```powershell
cargo run -p pid-plot --bin pid_plot_compare -- "D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pdf" "D:\work\cad\PID自研软件验证\plot\I井场 注采阀组工艺及自控流程图.pdf"
cargo test -p pid-plot --test plot_compare
```
