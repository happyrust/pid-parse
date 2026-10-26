# 出图收口之后：先落已写好的弧与图框，再点名剩下的多画（2026-10-25 开单）

> 用户 2026-10-25：审核工作区里未提交的出图增量之后，「提出下一步的开发计划，使用 plannatator」。
> 开单只写本文件，不改代码，不提交。
> 审查对象：工作区已暂存、未提交的出图增量（49 个文件，+3752 / −199，其中出图部分见 §1）。基线 `master` @ `49bc834`。
> 本单的编译、测试由用户在本机跑。
> **2026-10-25 Plannotator 批准**（`{"decision":"approved"}`，无批注），D1–D6 按推荐项。S0 `8f1bbc2`、S1 `1b21f21` 已落地。M1 只加了探针，数字等本机跑出来再写进分析页。

## 一句话

弧的方向位和图框家具已经写在工作区里，五张参考图没有「画在错的一侧」的弧了。下一步先把这一批按两笔落下，再量出剩下的多画是谁；没有点名之前不改笔宽、不抬出页文字、不改 0.78。

## 1. 审查发现

工作区里这一批和三篇分析对得上：`docs/analysis/2026-10-23-the-arc-trailing-byte-is-the-direction.md`、`2026-10-23-the-border-template-draws-the-sheet-furniture.md`、`2026-10-23-the-furniture-text-height-already-matched.md`。`igArc2d` 尾字节 `0` 顺时针、`1` 逆时针，符号库、`geometry` 发射、椭圆弧、`write::directed_arc` 同一条规则。旧 JSON 没有 `flag` 时 `serde(default)` 成 0，仍按以前的「全部顺时针」画。零长虚线改实线。PDF 的 `re` 矩形走 `draw_rect`，变换在 `draw_path` 里乘上。

改后读数（分析页，本单没有重跑）：

| 图 | 覆盖率 | 多画 | 文字 |
|---|---:|---:|---:|
| I井场 | 0.9999 | 25.5 mm | 546 / 546 |
| 一级闪蒸分离器A | 0.9998 | 22.6 mm | 151 / 151 |
| 一级闪蒸分离器B | 0.9997 | 1.5 mm | 123 / 123 |
| 低温分离器A | 0.9986 | 37.9 mm | 167 / 167 |
| 换热器CD | 0.9993 | 24.6 mm | 249 / 249 |

A 与低温 A 的多画里各有约 15 mm 是图框：这两张参考出图没有家具，`.pid` 声明了同一套 A1 框。I 井场从 41.8 mm 降到 25.5 mm，降掉的是五条画反的弧；**25.5 mm 还没有点名**。

| # | 发现 | 落点 | 去处 |
|---|---|---|---|
| R1 | 方向位、图框、0.78 em、零长虚线、矩形读回都在工作区，CHANGELOG 已写进 `[Unreleased]`，没有提交 | 见 §4 S0 | S0（D1） |
| R2 | I 井场剩 25.5 mm 多画、分离器 A 22.6、低温 A 37.9、换热器 CD 24.6、B 1.5，没有按符号 / 图纸线 / 家具拆开 | 对比报告 | M1 → 另开单（D2） |
| R3 | `src/model/sheet.rs` 弧的注释仍写「一定顺时针离开起点」；`src/model/mod.rs` 里还有一句来源说明同样写。字节在 `flag` 里，读这两处会按旧规则画 | 两个文件 | S1 |
| R4 | `flag == 0` 才算顺时针，其它任何值都当逆时针。目前只量过 0 和 1 | `symbol_library.rs`、`geometry.rs` | M1 顺带；不为 0 就停（D3） |
| R5 | `directed_arc` 的 `rem_euclid` 把整好 360° 收成 0，再变成零长弧。整圆走 Circle | `pid-plot/src/write.rs` | S1 加一条单测锁住「整圈不是这条路」；不改算法，除非 M1 找到这种记录 |
| R6 | 0.78 不是 `.igr` 里的字段，是两张带家具的图上 0.765–0.791 的拟合，加到每一条家具文字 | `write.rs` `PLATE_TEXT_ADVANCE` | 本单不改（D4） |
| R7 | 主对比仍把会签栏和标题栏 mask 掉。家具退化不会让五张图的分数变红。`pid-plot/tests/border_template.rs` 已在工作区 | `plot_compare`、`border_template.rs` | 保持（D5）。S0 把这个测试一起落下 |
| R8 | 量到但本轮没动的：N井场一条标签出页 1.55 mm；Q井场 10 mm 橙色笔被折成 0.678 mm；D04 重复图元、B井场同位同字（源图重复，纸上不变）；字体没有 subset 前缀；48 张备份没有参考出图 | 分析页 §三 | 本单不做（D6） |

## 2. 范围

**做**：把已写好的弧方向和图框落成提交；改掉两处过时的「一定顺时针」说明；量剩下的多画并写进分析页。量完之前不改画法。

**不做**：改 0.78；拆掉主对比的图框遮罩；为 N井场那 1.55 mm 或 Q井场的 10 mm 笔写新规则；给 48 张没有参考的备份做像素对照；六个 CLI 的 `--help` 改走 stdout（工作区里另一笔，本单不收）；publish 硬化（已经在 `49bc834` 一路上）；OpenCADStudio。

## 3. 要拍板的

| # | 问题 | 选项 | 推荐 |
|---|---|---|---|
| D1 | 工作区里这一批怎么提交 | (a) 弧方向一笔，图框 / 0.78 / 零长虚线 / 矩形读回一笔，分析页随各自的代码。(b) 原样一笔 | **(a)**。两件事各有一篇分析，拆开以后能单独回退 |
| D2 | 剩下的多画 | (a) 先量（M1），点名之前不改画法，量完另开单。(b) 现在就改笔宽和出页文字 | **(a)**。25.5 mm 还没有归属 |
| D3 | 尾字节不是 0 也不是 1 | (a) M1 顺带数一下；为 0 就在 S1 写明「只见过 0 和 1，其它值当逆时针」。不为 0 先停，把那几条贴进分析页。(b) 现在就把非 0/1 拒掉 | **(a)** |
| D4 | 家具横向 0.78 | (a) 保持。(b) 改回按字高排 | **(a)**。两张图上字高已经一致，差的是步进 |
| D5 | 主对比的图框遮罩 | (a) 遮罩留着，家具由 `border_template` 钉住。(b) 拿掉遮罩，让五张图的文字分数把家具算进去 | **(a)**。拿掉会把「参考没画框」的 A / 低温 A 算成我们多字 |
| D6 | N井场出页 1.55 mm、Q井场 10 mm 笔 | (a) 本单不做，M1 若发现它们占了剩下的多画再开单。(b) 本单就定规则 | **(a)** |

## 4. 步骤

每步一个提交，只 `git add` 本步路径。别的会话弄脏的文件不收。门：`cargo test -p pid-plot --test plot_compare --test border_template --test export_every_fixture --test export_plant_backup`、`cargo test -p pid-parse --lib symbol_library geometry sheet_records`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo fmt --all -- --check`。门由用户在本机跑。

### S0 落已写好的出图增量（D1）

两笔，都不改行为，只把工作区里已经对上分析页的改动收进去。

**笔 1 · 弧的尾字节是方向**

- `src/geometry.rs`、`src/model/mod.rs`、`src/parsers/sheet_records.rs`、`src/symbol_library.rs`
- `pid-plot/src/model.rs`、`pid-plot/src/write.rs`、`pid-plot/src/compare.rs` 里只为 `clockwise` 改的那一部分。若和笔 2 落在同一批结构体上、拆开编不过，两笔合成一笔，提交说明里写明
- `examples/probe_arc2d_flag.rs`、`pid-plot/examples/probe_arc_sweep_side.rs`，以及只加了 `clockwise` 参数的旧探针
- `docs/analysis/2026-10-23-the-arc-trailing-byte-is-the-direction.md`、`docs/analysis/2026-09-19-igarc2d-sweeps-clockwise-from-start-to-end.md` 的就地更正
- CHANGELOG 里「尾字节就是方向位」那一节

**笔 2 · 图框家具、横向 0.78、零长虚线、矩形读回**

- `pid-plot/src/write.rs`、`pid-plot/src/read.rs`、`pid-plot/src/fonts.rs`、`pid-plot/src/model.rs` 的家具与模板
- `pid-plot/tests/border_template.rs`、`pid-plot/tests/plot_compare.rs`
- `pid-plot/examples/probe_page_defects.rs`、`examples/probe_igr_template.rs`、`examples/probe_template_text_fields.rs`
- 四篇 10-23 分析：图框、字高、复核、「还差什么」
- CHANGELOG 里家具与字高那两节；`AGENTS.md` 里只收讲图框 / 弧方向的那几行

不进这两笔的：`src/bin/*`、`tests/cli_help_streams.rs`、`tests/inspect_cli.rs`、`tests/publish_xml_cli.rs`、`tests/writer_validate_cli.rs`、`tests/parse_real_files.rs`、`tests/style_link_ratchet.rs`、`Cargo.toml`、`.gitignore`。落地时用 `git diff --cached` 再核一遍，路径不对就拆出去。

### S1 过时说明（R3、R5；D3 的文字）

- `src/model/sheet.rs` 弧的注释改成「离开起点的方向由 `flag` 说明：0 顺时针，1 逆时针」，指到 10-23 那篇分析。
- `src/model/mod.rs` 那句仍写「弧从起点顺时针扫到终点」的来源说明改成同一句话。
- `directed_arc` 旁边加一条单测：起终角相差整好 `2π` 时扫描为 0。这是锁住现状，不是改算法。M1 若找到这种记录，停下来另开单。
- D3 量完若没有第三种字节，在 `symbol_library.rs` 读字节的那行旁边写上「只见过 0 和 1」。

### M1 点名剩下的多画（D2；不改 `src/`）

扩 `pid-plot/examples/probe_page_defects` 或新增一个只读探针，对五张参考图把多画的毫米按三堆报出来：

1. 家具（遮罩矩形内）。A 与低温 A 预期各约 15 mm，标成「参考没画框」。
2. 符号体。按符号名列前几名，带上字节区间。
3. 图纸自己的线。

顺带：全库 `igArc2d` / 符号弧尾字节不是 0 也不是 1 的条数；起终角相差整好一圆的弧条数。

读数追加到 `docs/analysis/2026-10-23-pdf-export-recheck-after-the-border-template.md` 新一节「弧归位之后还剩的多画」。CHANGELOG 和 `task_plan.md` 各记一笔。点名之后若有一堆值得改，另开单；本单不接着改。

## 5. 验收

- S0 之后五张图的覆盖率、多画、文字与 §1 那张表一致；289 条弧仍是 0 条落在参考没画的一侧。
- `border_template` 过。遮罩区文字 I 井场 35/35、B 38/38，位置偏差不超过 0.25 mm。
- S1 之后两处「一定顺时针」的说明不再出现；整圆扫描的单测在。
- M1 的三堆之和等于 §1 的多画（允许 ±1 mm 的取整），每一堆写明是不是该改。

## 6. 风险

- S0 拆笔时 `model.rs` / `write.rs` 同时被两笔改到。编不过就合成一笔，不要留一个编不过的中间提交。
- 工作区里还有 `--help` 改 stdout。只加 §4 列出的路径。
- M1 若把家具算进「该改的多画」，会把参考没画框误判成缺陷。第 1 堆必须单列。
- D3 若不为 0，先停，不要把未见过的字节拒掉或当逆时针画出去。

## 门禁记录

- 2026-10-25：审查后开单，D1–D6 待 Plannotator 审。
- 2026-10-25：Plannotator 批准（`plannotator annotate docs/plans/2026-10-25-plot-ink-after-the-arc-byte.md --gate --json` → `{"decision":"approved"}`，无批注），D1–D6 按推荐项。还没落地。
- 2026-10-25：S0 落为一笔 `8f1bbc2`。`model.rs` / `write.rs` 同时带弧方向和家具，拆开会留一个编不过的中间提交，按 D1 合并。`tests/parse_real_files.rs` 的两处 `..` 一起进了：不进的话新字段让这条测试编不过。`--help`、`style_link_ratchet`、`.gitignore` 的探针约定仍留在工作区。
- 2026-10-25：S1 落为 `1b21f21`。两处「一定顺时针」改成 flag 0/1，并锁住整圈扫描为 0。「只见过 0 和 1」等 M1 的清点，这一笔没写。
- 2026-10-25：M1 写了探针 `pid-plot/examples/probe_extra_ink.rs`（分堆在 `extra_ink_piles`）。没跑，分析页不预写数字。
