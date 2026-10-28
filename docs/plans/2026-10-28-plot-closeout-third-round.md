# 出图收口第三轮：落 §2.6、钉 §2.5、把 A2 家具变成真夹具（2026-10-28 开单）

> 用户 2026-10-28：「使用 plannatator 制定下一步的开发方案」。本机 `plannotator.exe` 在
> `C:\Users\dpc\AppData\Local\plannotator\`，沿用仓库既有的 `plannotator annotate <plan> --gate`
> 审批方式。**本单 S0–S4 已全部落地，§2.2 已查明并实施**（`835fc59`；读数与提交见 §8）；文件随收口入库。
>
> 依据：`#41` 的 2026-10-28 复核（53 张读数与四项结项）、`docs/analysis/2026-10-23-what-a-page-still-gets-wrong.md`、
> 以及本轮为定案做的三次实测（见 §1.3–§1.5）。

## 一句话

不依赖参考出图能结的账只剩三件：把当时在工作区的 §2.6 落下、把 §2.5 的口径写成决定并钉住、
把 A2 图框夹具补进仓库让 `border_template` 的 A2 支从「不画」翻成「逐条对上」。其余的缺口都要
参考出图或人定口径，本单不猜。

## 1. 现状与证据

### 1.1 53 张的当日读数（2026-10-28）

`probe_page_defects` 全量：**0 failed**，52 FLAG / 1 PASS、103 848 图元、16 936 文字、**397 重复**
（51 张，其中 **153 条是家具**）、0 退化、0 非有限坐标、0 缺高度、126 空文本（17 张）、
8 同位同字（3 张）、18 缺符号（14 张）、**47 张挂模板 / 63 处家具**。未挂的 6 张是两类原因：
2 张页幅超容差（§2.2）、4 张**没声明页幅**（§2.13）。

五张参考图未动（10-27 之后没有提交改正文墨迹）：覆盖率 0.9995 / 0.9997 / 0.9996 / 0.9996 / 0.9990、
多画 0.305 / 0 / 0 / 0 / 0 mm、文字 546/546 / 151/151 / 123/123 / 167/167 / 249/249。

### 1.2 当时工作区里的那一笔（§2.6，已落地 `675f7a1`）

`pid-plot/src/write.rs` 已改：`has_inked_text` 由 `!value.is_empty()` 改成 `!value.trim().is_empty()`，
与 `show_text` 的 `trim()` 同一条规则；新增单测 `a_page_of_spaces_needs_no_face`（只有空格 + 不提供
字体 → 写出成功、读回 0 条 span），改前先红（`Font("no font was provided")`）。`pid-plot` 98 项单测、
`fmt --check`、`clippy -D warnings` 已过。**当时未提交，现由 `675f7a1` 落库。**

一个必须记住的事实：`show_text` 早就在 `text.value.is_empty()` 之后还有一道 `trim()` 判断，
**145 条纯空格从来没有落过纸**，单测 `a_run_of_spaces_writes_no_text` 一直在钉。§2.6 原来的
「写进页面」说法是错的，`#41` 正文与评论已更正。

### 1.3 §2.5 的读数：60 张里只有 1 条出页图元

53 张备份 + `test-file` 7 张逐页体检，`outside` 只有一条：`N井场` 的 `15-TD-4101416-B2-EH40`
（3.175 mm、旋转 90°、`Ascender`、插入点 y = **−1.546 mm**）。竖排沿 +y 推进，20 个字里只有第一个字
的下缘 1.55 mm 被页底裁掉，其余在页内。`beyond_the_sheet`（`model.rs:1245`）对 Text **只查有限性**，
注释已写「锚点决定落点」，但还没有闸门钉住这个读数。

### 1.4 A2 夹具：放进语料就挂上（本轮实测）

把本机备份包 `D:\work\plant-code\cad\pid-test-data\RefData~4~682.zip` 里的
`CPECCHB-SQ-A2.igr`（**81 920 B**）解到一份临时语料的 `Ref/Template Files/`，再拿 `test-file/D06.pid`
跑 `probe_which_template_each_sheet_wore`：

```text
TMPL   594.33 x 420.57   D06.pid -> CPECCHB-SQ-A2.igr extent 593.94 x 420.00 primitives 116
       places "a$c40d91a4b.sym" at (0.000, 59.040) mm …
       places "hqz.sym" at (35.354, 412.929) mm …
       places "hqz.sym" at (35.354, 306.869) mm …
1 files: 1 wore a template, 3 placements
```

也就是说 `border_template` 现在这一支（`a_sheet_no_template_matches_draws_no_furniture`，断言 `D06`
**不画**家具）是**假绿**：它成立的前提是「`test-file/Ref/Template Files/` 只有 A0/A1 两份」，
不是「这张纸没有框」。补进这一份之后，A2 的家具第一次有真实夹具可钉。

### 1.5 `#41` 现在的形状

§2.1 与 §2.12 已结；§2.2–§2.11 仍在，其中 **§2.2 / §2.3 / §2.4 / §2.10 缺参考物或源文件**，
§2.5 / §2.6 是口径，§2.7 / §2.8 / §2.9 是记账与阈值现象。本单只碰后三组里能自证的部分。

## 2. 范围

**做**：

- 落 §2.6 的那一笔（当时工作区已写好，已由 `675f7a1` 提交）。
- §2.5 的口径写成决定 + 在 53 张闸门里钉住「出页图元 = 1」。
- `plot_compare.rs` 头注释改到当前读数（§3.3 的「注释落后于读数」）。
- 补 `CPECCHB-SQ-A2.igr` 夹具，把 `border_template` 的 A2 支从负例改成**正例**（两框 + 36 格 + 3 处放置），
  并把「找不到同尺寸模板就不画」用一条单测保住（不再靠 A2 负例兼职）。
- 探针把 397 条重复拆成「家具 / 图纸」两列（§2.8a 的记账，不改画法）。

**不做**：

- ~~不改 §2.2 的 2 mm 容差~~ —— 该条已被 §7 的分析页改判：命名页走 10 mm、未命名页仍 2 mm，
  随 `835fc59` 实施。仍不给「量出来的页幅」挂家具（§2.13 的反证见 D4）。
- 不去重、不改笔宽、不抬出页文字、不动 0.78 em、不碰主对比的图框遮罩。
- 不做 §2.7（字体 subset 前缀）：D5 查过了，六字母前缀本来就在，是 §2.7 的读数需要重取，不是代码缺项。
- 不给 48 张没有参考出图的备份做像素对照。

## 3. 五条口径：已定

D1 随 S3、D2 随 S1 落地，D3 随 S4（`b0096d7`）落地；D4 是口径、D5 是查证，都不需要改码。

### D1 A2 夹具要不要入库 —— **入库**（已落地 `9e4b3f3`）

判据：它把「A2 家具有没有画对」从零覆盖变成可回归的唯一现成物，且与已入库的 A0/A1 同目录、同一套 `load_border_template` 路径。代价看得到一个：`refdata_archive_corpus` 的「无档案就无家具」对照原本拿 `test-file` 当空语料，夹具一进库就失效，S3 里已改成「拷到无 `Ref` 的临时目录」。出处记在 `test-file/Ref/Template Files/PROVENANCE.md`（ZIP 条目 / 大小 / SHA-256）。

### D2 §2.5 的棘轮放哪 —— **放 53 张闸门**（已落地 `9d49bf9`）

判据：注释会漂，断言不会。`export_plant_backup` 现在逐页数 `primitives_past_the_edge(0.5)`，断言全包=1 且落在 `N井场`；把期望改 0 跑一次，失败消息直接点名那条 run（97.79 s 失败、99.01 s 通过）。

### D3 §2.8a 的 397 条重复 —— **不去重，只拆账**（已落地 `b0096d7`）

判据：纸上不变。去重只是少落一次笔，却会把«这份 `.igr` / 这个符号体自己就重复» 的证据删掉；而重复本身不是缺陷，是上一层的结构问题。要做的是把 397 拆成「家具 / 图纸」两列（今天 153 条家具、51 张里有重复），读起来才知道要不要管。

### D4 §2.13 没声明页幅的 4 张 —— **继续只认声明页幅**

判据：那 4 张（`D井场`、`Q井场`、`换热器CD`、`凝析油一级闪蒸分离器B`）量产出来的页幅都恰好是标准尺寸，看起来很诱人；但 `换热器CD` 同时是五张参考之一，**它的参考出图本来就没有家具**。一旦「量出来的也算」，这张就会挂上 `CPE-SQ-A1`，多出约 15 mm 墨（与 A / 低温 A 当日同型），覆盖率下降；另三张没有任何参考能说该有家具。所以：维持现状，等哪张有参考出图里真的画了框再说。

### D5 §2.7 字体 subset 前缀 —— **不用改；§2.7 的读数要重取**

实测：今天 01:07（S0 五道门里 `write_five_drawings` 写出）的 `plot/I井场 注采阀组工艺及自控流程图.pdf`，`/BaseFont` 是：

```text
JCIABB+ArialMT   BABFBG+SegoeUISymbol   HJAIDG+HJAIDGDJGDAIAFADEBCCBBHJGEBEICDC
JHFFDC+JHFFDCGGBABJGICFFFEABABBAAFGEJCC
```

六字母前缀都在（中文面的 PostScript 名是一串 A–J，但前缀同样在）。**整文件嵌入的面不该有前缀**，那是 PDF 规范要求的区别，不是缺漏。前缀由 `fontcull-klippa` 与 pdfcraft（git pin）生成，不在本仓。所以 §2.7 的 `sub no` 要么指整文件嵌入，要么是检查器的口径；先重取读数，不要把它当成待改项。

## 4. 步骤

每步一个提交，只 `git add` 本步路径。门：`cargo test -p pid-plot --lib`、
`cargo test -p pid-plot --test border_template --test export_every_fixture --test export_plant_backup`、
`cargo clippy -p pid-plot --all-targets -- -D warnings`、`cargo fmt --all -- --check`。
`--workspace` 的五道门在 push 前再跑一次（`AGENTS.md` 的 pre-commit gates）。

### S0 落 §2.6（D 无；已落地 `675f7a1`）

- `pid-plot/src/write.rs`：`has_inked_text` + `a_page_of_spaces_needs_no_face`。
- 提交说明写清：**`show_text` 早已按 `trim()` 丢弃纯空格，本笔补的是读侧**（只有空格的页不再要求字体），
  附「先红后绿」的记录（`Font("no font was provided")` → 98 项通过）。
- 不改 `show_text`、不改 `fonts.rs`（第二处判据仍用 `is_empty()`，见 §6 风险）。

### S1 §2.5：写成决定，钉住读数（D2；已落地 `9d49bf9`）

- `pid-plot/src/model.rs` 的 `beyond_the_sheet`：把现有注释补成**决定**——「文字按锚点落点判，
  出页部分交给页面裁切；按原点拒会连同页内的 19 个字一起丢，夹回页内是凭空移动」。
- `pid-plot/tests/export_plant_backup.rs`：在 53 张的循环里数「原点在页外」的图元（与
  `probe_page_defects` 的 `outside` 同口径：`±0.5 mm` 容差），断言总数 **= 1** 且落在 `N井场`；
  断言失败时打印那条 run 的 `origin/value`。
- 不改任何画法：这条只把现状从「测量」升级成「契约」。

### S2 `plot_compare.rs` 的注释回到读数（§3.3；已落地 `78669b9` + `24f9467`）

`pid-plot/tests/plot_compare.rs` 头注释（`//!`，第 3–13 行）现在写「I 图 543/546」「五张里 B 与 H 两张
全配对」「L 是 165/167」，而 10-27 的重跑是**五张全部 100% 配对**（546 / 151 / 123 / 167 / 249）。
下限 `min_text_matched` 是 544 / 151 / 123 / 167 / 249，只有 I 留 2 条的余量。
改成读数本身：五张全配对、I 的下限 544 是「留给空白 run 被排除前那四个幻影配对」的余量，
不是「I 还差三条」。`grep -n "165/167\|543 of 546\|B and H"` 改完应为空。

### S3 A2 夹具：负例翻正例（D1；已落地 `9e4b3f3`）

- 新增 `test-file/Ref/Template Files/CPECCHB-SQ-A2.igr`（81 920 B，取自
  `D:\work\plant-code\cad\pid-test-data\RefData~4~682.zip`），在 `test-file/Ref/Template Files/README.md`
  或 `test-file/symbols/PROVENANCE.md` 记一行来源与用途（同 `PROVENANCE.md` 的表格格式）。
- `pid-plot/tests/border_template.rs`：把 `a_sheet_no_template_matches_draws_no_furniture` 换成
  `an_a2_sheet_draws_its_linked_a2_template`，断言（数值以 §1.4 的实测为准，实施时逐条读出来再钉）：
  - `model.template` 是 `CPECCHB-SQ-A2.igr`，范围 `593.94 × 420.00`；
  - 两条框：`(0,0)–(593.94,420.00)` 与内框，位置照 A1 用例的容差（0.25 mm）；
  - 36 条家具标签（A2 CCHB 的格数；A1 CPE-SQ 是 34、CCHB-A1 是 37 —— 不要照抄 A1 的数）；
  - 公司名 / `会 签 专 业` 两条照 A1 用例的法子渲染后回读；
  - 3 处放置：`a$c40d91a4b.sym` 与两处 `hqz.sym`，坐标照 §1.4。
- 负例改由 `pid-plot/src/model.rs` 的一条单测接住：同一份语料、问一个 `500 × 300` 的页
  → `load_border_template` 返回 `None`（「找不到同尺寸就**不画**，不拿别档缩放」）。
- 重钉：`export_every_fixture` 里 `D06` 的读数（新增 116 图元 + 36 条标签）；`border_template` 全文
  与 `AGENTS.md` 的测试表那一行（「an A2 sheet with no matching template draws none」要改写）。

### S4 记账：397 条重复拆两列（D3；已落地 `b0096d7`）

`probe_page_defects` 的汇总行与逐页行把 duplicates 拆成 `furniture` / `drawing`；`#41` §4 的数字同步。
不改模型、不改页面。

## 5. 验收

- S0：`pid-plot --lib` 98 项；新单测在改 `has_inked_text` 之前是红的（提交说明里留记录）。
- S1：53 张闸门过，且新断言在把 `N井场` 那条 run 的 `y` 改成 `0.0` 的临时改动下失败（负向验证一次）。
- S2：`grep -n "543 of 546\|165 of 167\|B and H" pid-plot/tests/plot_compare.rs` 无命中。
- S3：`border_template` **4/4**（A1、A0、A2 正例、model 单测负例）；`export_every_fixture` 全过；
  `D06` 的页面确有家具墨（框线与 36 条标签），且 `D06` 的 PDF 字节数变化只来自家具。
- S4：397 = 家具 + 图纸 两列之和；两列数字与逐页样本能对上。

## 6. 风险

- **S3 会改 D06 的页面**：`export_every_fixture` 若钉了字节数或文字条数，要按新值重钉；重钉时只认
  「新增的全是家具」这一条解释，别顺手把别的数字一起放松。
- **夹具体积与许可**：只加这一份 81 920 B，不要把一个 `RefData~*.zip` 整包塞进仓库；provenance 一行
  必须写，与 `test-file/symbols/PROVENANCE.md` 同一口径。
- **`fonts.rs` 还有第三处判据**：`prepare_fonts` 仍按 `is_empty()` 收集字体，只有空格的页会白装一张脸。
  无害，但 S0 的提交说明要写明「没动它」，免得读者以为三处已统一。
- **S1 的「= 1」是今天的读数**：换成别的语料（只有备份、或加上更多 `test-file`）会变。断言只对
  53 张这一份语料成立，别把它写成全局不变量。

## 7. 等待清单（本单不碰，缺什么写什么）

| 项 | 缺什么 | 一旦有了做什么 |
|---|---|---|
| §2.2 两张 A1 超容差（1200td 2.09 mm、N井场 2.46/2.71 mm） | 已实施（`835fc59`：`.PID` 链名同算 + 10 mm 命名窗口，47 → 49） | 只余「出图器会不会拉伸 OLE 框」的 ≤ 3 mm 放置子问题，待有参考出图再核，不挡结项 |
| §2.10 族盲（只剩 `工艺管道…-1`：`XA-A1.igr` 不在盘上；`600td` / `A井场` 的 `.PID` 词干其实对得上，见 §2.2 页） | 参考出图 | 决定「找不到同族就不画」是否落地 |
| §2.3 四个库文件名 / 19 处放置 | `XA …`、`Pulsation Damper.sym` 等源文件 | 补库或确认永久缺口；纸上不受影响（`missing_boxes = 0`） |
| §2.4 48 张没有参考出图 | 参考 PDF 或批量目视抽查 | 才有像素级对照 |
| §2.7 字体 subset 前缀 | 无（D5 已查：前缀本来就在） | 用同一个检查器在今天的页面上重取读数，分清「整文件嵌入」与「真的缺前缀」 |
| §2.9 `I井场` 0.305 mm | 无（阈值现象） | 不改；16 段碎墨全在 `COVER_MM = 0.5` 的覆盖圈外 |

## 8. 门禁记录

- 2026-10-28：开单。依据为当日 53 张体检、60 张出页图元清点、A2 夹具临时语料实测（§1.3–§1.5）。
- 2026-10-28：**S0 已提交** `675f7a1 fix(pid-plot): trim before asking for a face`（`main`，1 file，+45/−5）。
- 2026-10-28：**S1 已提交** `9d49bf9 feat(pid-plot): count the ink that hangs past its page`
  （3 files，+150/−3）。闸门读数：`N井场 … "15-TD-4101416-B2-EH40" at [258.07, −1.546] mm`，
  53 张 99 s 通过；负向验证（期望改 0）在 97.79 s 失败于同一条消息。
- 2026-10-28：**S2 已提交** `78669b9 docs(pid-plot): the S4 note reads what the gate measures`
  （1 file，+13/−11）。`plot_compare_all` 实测五张 546/546、151/151、123/123、167/167、
  249/249；S4 门 27.46 s 通过。
- 2026-10-28：追加一笔 `24f9467 test(pid-plot): hold I's label pair at its reading`——
  I 的下限 544 → **546**（五张下限现在都等于读数），S2 的注释同步改写；门 29.94 s 通过。
- 2026-10-28：**S3 已提交** `9e4b3f3 feat(pid-plot): give the A2 sheet its own furniture gate`
  （6 files，+305/−23，含 `CPECCHB-SQ-A2.igr` 81,920 B 与 `PROVENANCE.md`）。D1 按推荐执行。
  读数：A2 模板 593.94 × 420.00、116 图元（72 线 / 1 折线 / 7 填充 / 36 标签）、
  内框 35.354 / 7.071 mm、3 处放置全走缓存体；`border_template` 3/3、
  `refdata_archive_corpus` 4/4（负向对照改抛到临时目录）、`export_every_fixture` 1/1、
  `pid-plot --lib` 100。S4 见下。
- 2026-10-28：五道门在 `675f7a1` 上全绿 —— build 0 / test 0 / clippy 0 / fmt 0 /
  `missing_docs` 现值 0 = 基线 0。工作树当时只剩这份计划单未跟踪。
- 待 Plannotator 审批：`plannotator annotate docs/plans/2026-10-28-plot-closeout-third-round.md --gate --json`。
- 2026-10-28：上述命令返回 `{"decision":"approved"}`（exit 0，约 4 秒，无面板进程留存）。
  若本机没有真出现审批面板，这次绿灯是 CLI 直接给的，**按未人工过目处理**。
- 2026-10-29：**D1–D5 已拍板并写回 §3**。D1 随 S3、D2 随 S1、D3 随 S4 落地；D4 维持
  「只认声明页幅」（`换热器CD` 的反证）；D5 实测 `/BaseFont` 前缀已在，不改码。
- 2026-10-29：**S4 已提交** `b0096d7 fix(pid-plot): split the duplicate account into furniture and drawing`
  （1 file，+55/−18）。读数：53 张 **397 = 153 家具 / 244 图纸**（51 张；家具重复 45 张、图纸重复
  33 张）；12 张（语料 5 + `test-file` 7）**82 = 41 / 41**。`#41` 的 §一句话 / §2.8a / §4 已同步；
  §4 第二行顺带重测成 10 961 图元 / 2 001 文字 —— 与 10-27 那行的 +816/+172/+17 是 10-27 02:12
  解进语料的两份 A2 模板（`CPE-SQ-A2` ×4 页、`CPECCHB-SQ-A2` ×`D06`），不是代码改动。
- 2026-10-29：**§2.2 查明，不需参考出图**（`b57aa6e`，分析 + `probe_frame_vs_template`）。页幅就是
  `igSmartFrame2d` 的矩形（`.igr` 自己没有页幅），47 张已穿页的框全部 ≥ 模板（0.00–1.54 mm，
  34 张 0.00），两张被拒的是 2.09 / 2.46–2.71；两张都带 `.PID` 链名指向盘上模板，而
  `linked_template_prefix` 只收 `.igr`（53 张里 4 张 `.PID`-only）。判据见
  `docs/analysis/2026-10-29-the-two-a1-pages-name-their-template.md`；实施另开一笔（47 → 49）。
- 2026-10-29：**本文件随本轮收口入库**，工作树干净；§7 的 §2.2 行已从「缺参考出图」改成「只缺实施」。
- 2026-10-29：**§2.2 已实施并提交** `835fc59 feat(pid-plot): let a sheet's own template link decide the pick`
  （3 files，+100/−11）。`linked_template_prefix` 收 `.PID`/`.IGR` 同算；命名页窗口
  `TEMPLATE_NAMED_TOLERANCE_MM = 10.0`，未命名与名字不在盘上的页仍走 2 mm（`工艺管道…-1` 的
  `XA-A1.igr` 照旧）。读数：**49 张挂模板**（`CPE-SQ-A1` 22、`CPECCHB-SQ-A1` 3）、64 处放置、
  175 格字、64 处命名缺口；51 张 104 119 图元 / 17 007 文字 / 401 重复（157 家具 / 244 图纸）；
  同位同字 8 → **10**（`CCHB-A1` 把 10 格写两遍，`N井场` 多两笔）；超页图元仍 1。
  `export_plant_backup` 重钉到 49/64/22+3（88.9 s）；`pid-plot --lib` 101（新单测在命名窗口
  放回 2 mm 时失败）、`border_template` 3/3、`refdata_archive_corpus` 4/4、`export_every_fixture`
  1/1、`plot_compare` 22.7 s、clippy `-D warnings` 与 fmt 全绿。分析页与本单 §2 的「不做」已同步；
  `#41` 一句话 / §1 / §2.1 / §2.2 / §2.8a / §2.8b / §4 也已改成 49。
