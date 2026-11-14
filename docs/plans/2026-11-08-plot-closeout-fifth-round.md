# 出图收口第五轮：剩下的 PDF 导出修复开发计划（2026-11-08 重写）

> 状态：**待批**。基点 `main` `e5fa361`：本地与 `origin/main` 0/0 同步，`main` 已推送。
> 本版取代同一路径 11:24 那一版。那一版有三处前提已过期：基点 `c9202ff`；「`main` 超前 49 个提交、按既定不推送」；D4「CI 上出图门禁全部软跳过」。
> 依据：`docs/plans/2026-10-29-plot-pdf-correctness.md`（「正确」的定义与前四轮）；
> `docs/analysis/2026-11-07-every-label-miss-is-a-reference-font-rule-or-the-pixel-grid.md`（普查收口：剩 64 对）；
> `docs/analysis/2026-11-08-the-signal-dash-is-fitted-to-every-piece.md`；`.kiro/specs/pdf-export-ledger/`；
> `docs/plans/2026-11-05-reference-plots-to-obtain.md`；CI run 42443598516（绿）与 41856784362 / 41863541671 / 42065513836（红）保存的日志。
> 标注：**未核实** = 本轮没拿到证据，写明原因和由哪一步补上；**推断** = 由已核实的事实推出，没有直接记录。

## 一句话

画法上仍然没有「已知画错、该修没修」的条目。普查收口（`c9202ff`）以来，`pid-plot/src` 和根 crate 的 `src` 都没有提交，普查读数仍然适用。
剩下四件事：

1. 交付 PDF 没有写出入口，11 张落后画法代码 18–29 个提交（D1，S1–S2）。
2. 文字残差还有 8 对原因未查清（D3，S3–S4）。
3. 42 张无参考页没有记账，少画东西门禁看不见（D2，S5–S7）。
4. CI 上真正带断言跑的出图门禁只有 A2 模板一条（D4，S8）。

`main` 已推送，以后每次推 `main` 都会跑 CI，所以 D4 的收益从下一次推送起就兑现。D4 的推荐项因此从「本单不动」改为「把不依赖字体的 `PaperModel` 金样放进 CI」。
顺序按先小后大：交付 → 8 对文字 → 出图记账 → 收尾。S8 与其他步骤无依赖，可以并行。

## 1. 现状（2026-11-08 重新核实，`main` `e5fa361`）

### 1.1 基点与工作树

| 项 | 读数 | 证据 |
|---|---|---|
| HEAD / 同步 | `e5fa361`；`origin/main...HEAD` = `0 0` | `git rev-parse --short HEAD`；`git rev-list --left-right --count origin/main...HEAD` |
| 最近提交 | `e5fa361`（11-08 11:38）`test(pid-plot): report a fixture's symbol gaps when the bundle is absent`；`667c325`（11-08 11:02）`test(pid-plot): gate the A2 template on the names, not the library's presence`；`c9202ff` 普查收口 | `git log --oneline -4` |
| 普查之后画法代码 | 无提交 | `git log --oneline c9202ff..HEAD -- pid-plot/src src` 输出为空 |
| 他人未提交改动 | 18 个已跟踪改动全属 publish 线：`src/publish/*` 11 个、`src/bin/pid_publish_xml.rs`、`docs/baselines/publish-diff-accepted.json`、`tests/parser_panic_safety.rs`、`tests/publish_*.rs` 3 个、`vendor/oxidized-mdf/src/pages.rs`；与 `pid-plot/`、`AGENTS.md`、`.github/` 不重叠 | `git status --porcelain` |
| 未跟踪 | 本计划、6 个 `pid-plot/examples/tmp_*.rs`、`.kiro/`、`.cloud-attachments/` | 同上 |

### 1.2 交付 PDF（D1）

- **落后多少。** `D:\work\cad\pid-test-data\验收数据\plot` 下有 11 张 PDF：
  - 6 张写于 11-05 20:13–20:14：轻烃分离器 I 气出口（A2）、空冷器 A/B/C、J K、L M。
  - 5 张写于 11-07 14:54：I井场、闪蒸 A/B、低温 A、换热 CD。同目录还有 5 个 `edge-*.txt`，写于 11-07 14:43。
  - `pid-plot/src` 最后一次提交是 `fb24052`（11-08 04:09）。第一批之后有 29 个提交改过 `pid-plot/src`，第二批之后有 18 个（`git rev-list --count --since=<写出时间> HEAD -- pid-plot/src`）。11 张全部落后。
- **谁在写。**
  - `pid-plot/tests/write_five_drawings.rs` 只写 L11–17 列出的那 5 张，目标是 `<PID_PARSE_SELF_CHECK_DIR>\plot\<name>.pdf`（L20–21，默认 `D:\work\cad\PID自研软件验证`；L56、L61）。这个目录 11-08 11:36 刚刷新过。
  - `pid-plot/tests/compare_five_drawings.rs:111–113` 往同一个 `plot` 目录写 `edge-<name>.txt`。
  - `验收数据\plot` 第二批的 5 张 PDF 和 5 个 edge 文件，名字与这两个测试的输出一致。推断是当时把 `PID_PARSE_SELF_CHECK_DIR` 指到 `验收数据` 跑出来的（**推断**：没有写出记录可查）。
  - 第一批 6 张没有任何代码负责写。
- **门禁看不出交付过期。** `pid-plot/tests/reference_plots_acceptance.rs::eleven_reference_plots_hold_their_readings`（L547）在 L600 把每页写进内存（`write_plot`），再逐项断言（L581–L714 的循环）。它既不读也不写 `验收数据\plot`，所以门禁全绿也看不出交付已经落后。
- **注释。**
  - 只有 `pid-plot/tests/plot_compare.rs:300` 明确写着交付由 `write_five_drawings` 写（「still the deliverable — `write_five_drawings` writes it」）。
  - 另外三处，`pid-plot/tests/export_every_fixture.rs:4`、`pid-plot/src/model.rs:2666`、`:3926`，提到它是作为逐端点比对的测试，不涉及谁写交付。这三处只看了 grep 命中的那一行，上下文**未核实**。
  - 旧计划说「四处注释说交付由它写」，说多了。
  - `AGENTS.md` 门禁表（L61–L77）没有任何一行说明交付由谁写。

### 1.3 出图记账规格（D2）

- `.kiro/specs/pdf-export-ledger/tasks.md`：51 个勾选框，0 个已勾。
- **`.kiro/` 未跟踪**（`git status` 显示 `?? .kiro/`），规格不在 `origin/main` 上。从干净检出接手的会话或子 agent 看不到它，所以派单前要先提交（S5.0）。
- 代码没有实施。`git grep -i -E 'ledger|DropReason' -- pid-plot/src src` 命中 39 处，全在根 crate：`src/export_bundle.rs` 38 处、`src/symbol_library.rs` 1 处。
  这是 `ExportBundleConfidenceLedger`（写 `audit/confidence_ledger.json`；`src/export_bundle.rs` 的首个提交是 `9535ea4`，2026-06-19），与本规格无关。

### 1.4 标签普查剩下的 64 对（D3）

分类表在 `docs/analysis/2026-11-07-every-label-miss-is-a-reference-font-rule-or-the-pixel-grid.md` L249–L260，基于 `c9202ff`。此后画法代码没变（见 1.1），分类照旧：

| 原因 | 对数 | 归属 |
|---|---:|---|
| `template-scale` 模板字横向缩放 | 40 | 源侧，不修 |
| `font-link` 借字（`℃` 11、A2 `动火点` 2、A2 `℃` 1） | 14 | 环境，暂不修 |
| `chain-quantization` JK `1-1/2"` 与 `FC` 合串 | 1 | 源侧，不修 |
| `a2-title-block` A2 公司名 | 1 | CT-4 |
| `line-metrics` 模板标题 `华北油田苏桥储气库群`（I、LM、闪蒸 B） | 3 | **未查清**（L264：起点逐像素相同，整串宽 +0.053 mm） |
| `two-glyph-spacing` 空冷 A/B/C 的 `SP:` | 3 | **未查清**（L265：参考冒号 21 px、我方 22 px；I井场同一条标签的参考在 22 px） |
| `unexplained` A2 两条 `仪表风管线` | 2 | **未查清**（L266：竖直低 0.198 mm；A2 是 594.33 × 420.57 mm 画在 594 × 420 mm 纸上，没套像素网格） |

合计 64：源侧或环境 56，未查清 8。普查探针在 `pid-plot/examples/probe_why_paired_labels_miss_the_reference.rs`。

### 1.5 CI 上的出图门禁（D4，按新事实重写）

- **CI 跑什么。** `.github/workflows/ci.yml` 在 push 和 PR 到 `main` 时触发（L3–7），runner 是 ubuntu-latest（L20）：
  - `cargo test --locked --workspace --all-targets`（L49）
  - `clippy -D warnings`（L64）、`fmt --check`（L69）
  - missing_docs 棘轮（L77）、byte-audit（L84）
  - 另有一个 `cargo audit` job（L92–115）
- **CI 上没有字体目录。** `pid-plot/src/fonts.rs:63–73` 的 `font_dirs` 只找三处：`PID_PLOT_FONTS`、`<corpus>\fonts`、`test-file/fonts`。CI 上前两处不存在；`git ls-files test-file/fonts` 为 0，第三处也不存在。
- **CI 上没有完整符号库。** `.gitignore:47` 是 `test-file/symbols-full/`。本地有 618 个 `.sym`，CI 检出里没有。
- **三次红的真实原因。** 41856784362、41863541671、42065513836 保存的日志里，失败的都**只有一条**：`border_template::an_a2_sheet_draws_its_linked_a2_template`，panic 在修复前的 `pid-plot/tests/border_template.rs:786`（`missing_symbols` 非空）。`export_every_fixture` 三次都没有失败。
- **`667c325` 之后：CI 上有一条门禁带断言地跑。** 这条测试改成按名字集合判（L813 起）：完整库下一个都不许缺，缺库时只许那 6 个 bundled-only 名字缺。缺库时它在 stderr 说明原因。
  这条测试不需要字体，在 CI 上能建出 `PaperModel` 并断言。修复前它就是在 CI 上红在这里的，说明建模本身在 CI 上跑得通。
- **`e5fa361` 没有改变 CI 上的行为。**
  - `export_every_fixture::every_fixture_drawing_exports_a_pdf` 在 L51–55 发现没有字体目录，就在 CI 上软跳过了。L81–113 的导出断言和 `e5fa361` 加的 L128–146 分支都到不了。
  - `e5fa361` 修的是「有字体、但没有 `symbols-full`」的检出，比如另一台机器的本地克隆：这种检出下，它打印缺口数后返回，不再红。
  - 所以说「这两个测试现在在 CI 上有断言地跑」，只对 `border_template` 那一条成立。
- **其余文件按代码判断为软跳过。** 另外 11 个 pid-plot 测试文件的软跳过条件，都是 CI 上不存在的外部目录或字体目录：自检目录、`验收数据`、解压的备份、夹具备份。
  - 这 11 个是 `compare_five_drawings`、`export_plant_backup`、`paper_model`、`plot_compare`、`plot_text_sources`、`read_reference_plots`、`refdata_archive_corpus`、`reference_plots_acceptance`、`style_witness`、`template_symbol_gap`、`write_five_drawings`。
  - 各自的跳过分支举例：`reference_plots_acceptance.rs:552`、`write_five_drawings.rs:31` / `:36`、`style_witness.rs:161`、`refdata_archive_corpus.rs:123`、`paper_model.rs:42`。
  - 逐条测试是真跑还是软跳过，**未核实**。一个原因是保存的日志里 cargo 的 `Running` 行带颜色码残留，本轮没解析出来；更根本的是软跳过的测试在日志里同样显示 `ok`，光看日志区分不了。S8.0 用模拟 CI 的检出给出定论。
  - pid-plot 的库内单元测试随 `--workspace` 在 CI 上照跑，条数未核实。
- **绿的那次。** run 42443598516 保存的日志（`%TEMP%\pidci-42443598516-full.log`，5514 行）里没有失败测试，也没有 panic。两个 job 的 success 结论取自用户核对的 run 页面，本轮没再用 `gh` 查。
- **推送改变了什么。** `main` 已推送，`ci.yml` L3–7 让以后每次推 `main` 都跑 CI。任何能在 CI 上断言的门禁，从下一次推送起就生效。
  旧计划 D4 推荐「本单不动」，理由是「收益要等推送后才兑现」，这条理由不再成立。
- **没变的部分。** PDF 字节级、读数级的门禁（参考图验收、53 张备份、样式见证）都要字体和语料，在 CI 上仍然全部软跳过。PDF 的回退仍然只有在本机跑门禁时看得出。

## 2. 与旧计划（11:24 版）的差异

| 前提 | 旧版 | 本版 | 因此改了哪些步骤 |
|---|---|---|---|
| 基点 | `c9202ff`，超前 `origin/main` 49 个提交，不推送 | `e5fa361`，0/0，已推送 | S0 的验证读数；S9 去掉「不推送」，改成推送后看 CI；S1 的 `BUILD.txt` 记的 HEAD 可以在 origin 上查到 |
| D4 现状 | 「CI 上出图门禁全部软跳过」 | A2 模板一条带断言地跑；`export_every_fixture` 在 CI 上仍因没有字体软跳过；三次红只红在 A2 那一条 | D4 重写 |
| D4 收益 | 要等推送后才兑现 | 下一次推送起就兑现 | D4 推荐项从「不动」改为 (a) `PaperModel` 金样；新增 S8 |
| 记账规格 | 「已写好」 | 写好但未跟踪，不在 origin | 新增 S5.0「先提交规格」 |
| 交付落后 | 「20 多 / 10 多个提交」 | 29 / 18 个提交 | 只是读数 |
| 自检目录 | 11-08 10:46 已刷新 | 11-08 11:36 又刷新了一次（5 张） | 只是读数 |
| 注释 | 四处说交付由 `write_five_drawings` 写 | 只有 `plot_compare.rs:300` 是这个意思 | S2 只改这一处；其余三处执行时读了上下文再定 |
| 第 8 步 | S8「门禁与记录」 | S8 是 D4 的 CI 金样，收尾挪到 S9 | 只是编号 |

## 3. 决策（批准即按推荐项执行；要改哪条直接说）

### D1 交付 PDF 的写出入口

现状见 1.2：11 张交付没有代码负责写，门禁也不读它们。

- **(a) 推荐：验收门禁过钉之后统一落盘。**
  - **做法。** `eleven_reference_plots_hold_their_readings` 在 L600 已经有每页的字节 `our_bytes`。先把它们收起来，等循环结束、11 页全部过钉之后，一次写到 `<PID_ACCEPTANCE_CORPUS>\plot\<name>.pdf`，同时写 `plot\BUILD.txt`。
    `BUILD.txt` 的内容是 HEAD、`pid-plot` 和 `src` 有没有未提交改动、写出时间。设开关 `PID_ACCEPTANCE_DELIVER=0`，可以关掉落盘。
  - **效果。** 交付就是这次构建、这次门禁验过的那份字节。任何一页没过钉，就一张都不写，不会出现新旧混在一起的交付。`main` 已推送，`BUILD.txt` 里的 HEAD 在 origin 上可查。
  - **代价。** 改一个测试文件；门禁每跑一次多写 11 个文件。
  - **为什么优于 (b)。** 不增加人工步骤。
- (b) 门禁只报不写：比对盘上的交付和内存里的字节，不一致就列出过期文件，但不失败；另加一个手动的批量写出命令。交付只在有意时才变，但仍然要有人记得跑。
- (c) 维持现状，收口时手工重写。这次已经证明会落后。

### D2 出图记账做到哪一步

现状见 1.3：规格写好了但未跟踪；51 个任务（13 项，含 4 个 checkpoint）一个没做。
它要解决的是无参考页少画东西门禁看不见：给每页记一本账，每条图形记录、实体、页面图元、笔画段，要么上了页，要么归到一个有名字的丢弃原因。

- **(a) 推荐：先提交规格（S5.0），再按 `tasks.md` 全量实施。**
  - **做法。** 4 个 checkpoint 各停一次，汇报读数。硬护栏是任务 1.2 / 1.3 的字节对照：实施前后，备份 53 页、夹具 7 页、参考 11 页的 PDF 和 `PaperModel` 的 SHA-256 必须逐页相同。
  - **效果。** 「未归因丢弃 = 0」进 `export_plant_backup` 和 `export_every_fixture` 两道门。无参考页少画一批东西，在本机门禁上会直接红。
  - **限制。** 这两道门在 CI 上都软跳过（没有备份，没有字体），所以记账的保护只在本机。要在 CI 上也有保护，得靠 D4(a) 那类不依赖字体的检查。
  - **代价。** 改动面最大：根 crate 的发射器、`pid-plot` 的排版和写出、两道门禁、CLI。`export_plant_backup` 会变慢（旧读数约 90 s，本轮未复测），设计里有上限，超了就停下汇报。
- (b) 只做到任务 8「汇总」：先出账本和报告，不接门禁、不取基线。改动少一半，但回退仍然看不见。
- (c) 暂缓到补齐参考图之后。但参考图最多再覆盖 3 张，记账覆盖全部 53 张，两者不能互相替代。

### D3 剩下的 8 对文字

现状见 1.4：3 类共 8 对原因未查清，都在 0.2 mm 以内。

- **(a) 推荐：每类一个只读探针，各查一轮（S3）。**
  - 查到规则就修（S4），复跑普查，超限只许减不许增。
  - 查不到，就在分析页登记「已查、未解」，写明查了什么，然后收口。
  - 代价小：3 个 example。不为凑数改阈值。
- (b) 不查，8 对全部登记为「未解、不修」，直接收口。

### D4 CI 上的出图门禁（基于 1.5 的新事实重写）

现状：

- CI 上带断言跑的 pid-plot 出图门禁只有 A2 模板一条。
- `export_every_fixture` 因为没有字体目录，在 CI 上软跳过。
- 其余出图门禁按代码判断都在 CI 上软跳过，逐条定论待 S8.0。
- `main` 已推送，以后每次推 `main` 都跑 CI，所以加进 CI 的断言从下一次推送起就生效。这是推荐项变了的原因。

- **(a) 推荐：不依赖字体的 `PaperModel` 金样进 CI（S8）。**
  - **做法。** 输入是仓库内 `test-file/**/*.pid`，也就是 `export_every_fixture.rs:27` 的 `collect_pids` 收的那批。用 `lay_out_drawing` 建出 `PaperModel`（`export_every_fixture.rs:60`，不需要字体），序列化成 JSON，和提交在 `pid-plot/tests/golden/paper/` 下的金样比。
    根 crate 已有同类先例 `tests/golden/geometry/*.json`（6 个文件）。
  - **可行性依据。** `pid-plot/src/model.rs` 里的模型类型 derive 了 `Serialize`（L51、L108、L160、L389、L400 等）。A2 模板那条测试证明，CI 上建模本身跑得通。
  - **要先探的两件事（未核实，S8.1 先探再写）。** 一是整棵 `PaperModel` 树能不能序列化、结果是否稳定；二是怎样保证金样在「没有 `symbols-full`」这个条件下生成。
    缺 bundled-only 符号的那 6 个名字会改变模型，所以本机带着完整库生成的金样，到 CI 上会对不上。
  - **效果。** 仓库内夹具的排版一回退，下一次推 `main` 时 CI 就红。
  - **代价。** 一个测试文件，加上金样（总大小由 S8.1 量出）。每次有意改排版，都要用 `PID_PLOT_BLESS=1` 重写金样，提交 diff 会变大。
  - **为什么优于 (b)、(c)。** 比 (b) 多一道真断言；不需要 (c) 那台机器。
- (b) 只把 CI 覆盖写清楚，不加门禁：在 `AGENTS.md` 门禁表和 `ci.yml` 注释里写明，CI 上出图门禁哪条真跑、哪些软跳过、为什么。代价最小，但回退在 CI 上照样看不见。不管 D4 选哪项，这部分文档都会做（S8.3）。
- (c) 自托管 Windows runner，带字体和语料跑全量出图门禁。最完整，但要一台常开的机器、注册 runner，还要解决 runner 上字体和语料的授权。不在本单。
- 不推荐：把字体提交进 `test-file/fonts`，让 `export_every_fixture` 在 CI 上真跑。
  样式点名的是中文 Windows 的系统字体，再分发的许可**未核实**，大概率不允许。而且 `fonts.rs:75–77` 要求样式点名的那个字面必须在，换成替代字体也过不了。

## 4. 步骤

每一步都写明改哪些文件、为什么、怎么验证（命令和期望读数）、失败怎么回退、代价多大。行号以 `e5fa361` 为准；执行时若 `pid-plot/src` 已有新提交，先重新定位行号。

本机还有其他会话在编译，共用一个 target 目录会互相干扰。所以所有 `cargo` 命令都加 `CARGO_TARGET_DIR=<本会话独立目录>`。代价是第一次要全量编译，之后增量。

### S0 前置（每个执行会话开工前跑一次）

- **改：** 无。
- **验证：**
  - `git rev-parse --short HEAD`：记下读数。不要求等于 `e5fa361`；若不等，用 `git log --oneline e5fa361..HEAD -- pid-plot src` 看画法代码有没有变，变了就重新定位本计划的行号。
  - `git rev-list --left-right --count origin/main...HEAD`：期望 `0 0`。
  - `git status --porcelain -- pid-plot AGENTS.md .github`：期望只有 6 个 `?? pid-plot/examples/tmp_*.rs`。
- **回退：** 读数不符就停下报告，不动手。
- **代价：** 几秒。

### S1（D1）交付落盘

- **改：** `pid-plot/tests/reference_plots_acceptance.rs`。
  - L600 后把 `(name, our_bytes)` 收进一个列表。
  - L714 那组断言之后、循环结束处，统一写 `<corpus>\plot\<name>.pdf` 和 `BUILD.txt`。corpus 沿用 L511 的 `PID_ACCEPTANCE_CORPUS` 解析。
  - 读 `PID_ACCEPTANCE_DELIVER`，值为 `0` 就不写。
  - `PINS` 的 11 页和交付的 11 个 PDF 同名：这是按名字对照得出的，`PINS` 的名字串没有逐个核实。执行时先打印出来比一遍。
- **为什么：** 见 1.2，交付没有写出入口，门禁验过的字节也不落盘。
- **验证：**
  - 执行前，先把 `验收数据\plot` 的 11 个 PDF 复制到 `验收数据\plot\.prev-e5fa361\`。
  - `cargo test -p pid-plot --test reference_plots_acceptance -- --nocapture`：期望 2 passed，并且没有 `skipping the acceptance gate` 字样。
  - `Get-ChildItem 'D:\work\cad\pid-test-data\验收数据\plot\*.pdf'`：期望 11 个文件的 LastWriteTime 都是这次。
  - `BUILD.txt`：期望 HEAD 等于这次的 HEAD，未提交改动为否。
  - 同一个 HEAD 连跑两次，11 个文件的 SHA-256（`Get-FileHash`）逐个相同。旧计划的读数是写出器不写时间戳，`pid-plot/src` 里没有 `CreationDate` / `SystemTime`；本轮未复核，以这一条为准。
  - 设 `PID_ACCEPTANCE_DELIVER=0` 再跑一次：期望时间戳不变。
- **回退：** revert 这个提交，从 `.prev-e5fa361\` 拷回旧交付。
- **代价：** 一个测试文件，约 40–60 行；门禁多写 11 个文件；机器耗时可以忽略。

### S2（D1）注释与文档

- **改：**
  - `pid-plot/tests/plot_compare.rs:300`：「交付由 `write_five_drawings` 写」改成实际情况。它只写自检目录的 5 张；交付的 11 张由验收门禁写（S1）。
  - `pid-plot/tests/export_every_fixture.rs:4`、`pid-plot/src/model.rs:2666`、`:3926`：先读上下文，只在确实说到交付时才改，默认不改。
  - `AGENTS.md:77`，也就是 `reference_plots_acceptance` 那一行：补一句「过钉后写 `<corpus>\plot\*.pdf` 与 `BUILD.txt`，`PID_ACCEPTANCE_DELIVER=0` 关闭」。
  - `CHANGELOG.md` 加一条。
- **为什么：** 让注释和文档与 S1 之后的事实一致，免得下一个人照着注释去跑 `write_five_drawings` 刷交付。
- **验证：**
  - `git grep -n write_five_drawings -- pid-plot AGENTS.md`：期望每处说法都和上面一致。
  - `cargo fmt --all -- --check` 和 `cargo clippy -p pid-plot --all-targets -- -D warnings`：期望通过。
- **回退：** revert。
- **代价：** 纯注释和文档，约 10 行。

### S3（D3）三个只读探针，每类查一轮

- **改：** 新增 3 个 example，命名仿照现有的 `probe_*`，结论写进普查分析页 L249 那节之后的新小节。
  - `pid-plot/examples/probe_template_title_width.rs`：查模板标题 3 对。逐字比较我方的 `lpDx` 和参考的 `Tc`，看 0.3 px 是出在末字的自然宽，还是出在字宽 800 的取整（分析页 L264）。
  - `pid-plot/examples/probe_sp_colon_step.rs`：查 `SP:` 3 对。把空冷三张和 I井场 同一条标签的参考 run（`Tf`、`Tc`、`Tm`、设备纸宽）并排，找出让冒号落在 21 px 的那个量（L265）。
  - `pid-plot/examples/probe_a2_device_grid.rs`：查 A2 2 对。量 A2 参考图里 run 的起点是否落在某个设备网格上（L266）。如果有网格，`仪表风管线` 可能一并得到解释。
- **为什么：** 这 8 对是普查里仅剩的「原因未查清」。
- **验证：** 每个探针跑 `cargo run -p pid-plot --example <名字>`，各输出一张表。期望读数二选一：要么「规则 X 解释了 n/n 对」，要么「查了 A、B、C，都不解释」。
- **回退：** 探针只读，不进门禁，删掉文件即可。
- **代价：** 3 个 example，每个约 100–200 行；需要本机的 `验收数据` 和字体。可以和 S1 并行，两边各用独立的 target 目录。

### S4（D3）修复（S3 查到规则才做）

- **改：** 按规则改 `pid-plot/src` 的文字落点，一类一个提交。具体文件和行号**未定位**：要等 S3 查出规则才知道。
- **验证：**
  - `cargo run -p pid-plot --example probe_why_paired_labels_miss_the_reference`：期望超限总数 ≤ 64；对应类的对数清零；新增为 0。
  - `reference_plots_acceptance` 的钉子按新读数收紧，然后本机跑 `cargo test -p pid-plot` 全量（带字体和两份语料），期望全绿。
  - 查不到规则的类：在分析页登记「已查、未解」以及查了什么，不改代码。
- **回退：** 任何一类的新增超限 > 0，就 revert 这一类的提交。
- **代价：** 取决于规则，预计集中在文字落点的一处函数（**未核实**）。S4 和 S1 都改 `reference_plots_acceptance.rs`，所以 S4 要排在 S1 之后。

### S5（D2）记账前置

- **S5.0 提交规格（要你批准）。**
  - **改：** 把 `.kiro/specs/pdf-export-ledger/` 单独做成一个 docs 提交，只加这个目录。`.kiro/` 下其他内容不动。
  - **为什么：** 规格未跟踪，不在 origin 上，派出去的会话看不到。
  - **验证：** `git ls-files .kiro/specs/pdf-export-ledger` 不为空。
  - **回退：** revert。
  - **代价：** 只有文档。
- **S5.1 记账任务 1。** 做三件事：并发检查；字节对照探针 `probe_export_bytes_unchanged`，覆盖备份 53、夹具 7、参考 11，共 71 页的 PDF 和 `PaperModel` SHA-256；实施前的快照和耗时。
  - **改：** 新增 1 个 example，快照存在 `target/` 下，不提交。
  - **先查重叠：** 记账要改的根 crate 文件，不能和 publish 线未提交的 `src/publish/*` 等 18 个文件重叠。有重叠就停下问。
  - **验证：**
    - 同一个 HEAD 连跑两次，71 个哈希完全相同。
    - 记下 `export_plant_backup` 的耗时，作为任务 9 的基准。
  - **回退：** 删掉探针即可。
  - **代价：** 1 个 example，加一次全量导出的机器耗时。

### S6（D2）记账任务 2–8

- **改：** 原因目录和账本类型、根 crate 发射器的账、写出端判定、排版记账、汇总。具体文件和行号按 `.kiro/specs/pdf-export-ledger/tasks.md` 逐项定位。设计文件本轮没有重读，行号**未核实**。
- **为什么：** 见 D2。
- **验证：**
  - 每个任务做完都跑 S5.1 的字节对照，期望 71 页 0 差异。
  - `cargo test -p pid-plot` 本机全量；动了根 crate 的，再加根 crate 的 `cargo test`。
  - 任务 5、任务 7 两个 checkpoint 各停下汇报一次读数。
- **回退：** 每个任务一个提交，字节对照出现任何差异就 revert 当前任务。
- **代价：** 本单最大的一块，跨两个 crate。

### S7（D2）记账任务 9–13

- **改：** 基线和门禁（`pid-plot/tests/export_plant_backup.rs`、`pid-plot/tests/export_every_fixture.rs`）、接 CLI、首次基线和文档、收尾时的字节对照。
- **验证：**
  - 字节对照 0 差异。
  - 53 页加 7 个夹具「未归因丢弃 = 0」。
  - 现有门禁的读数不变。
  - `export_plant_backup` 的耗时不超过设计上限，超了就停下汇报。
- **回退：** 一样，每个任务一个提交。
- **代价：** 中等。提醒一句：这两道门在 CI 上软跳过，记账的保护只在本机。

### S8（D4）CI 上的 `PaperModel` 金样

S8 与 S1–S7 没有依赖，可以并行。

- **S8.0 模拟 CI 的检出，给出「CI 上哪些真跑」的定论。**
  - **做法：**
    - `git worktree add <临时目录> e5fa361`：干净检出里没有 `symbols-full`，也没有 `test-file/fonts`，和 CI 一样。
    - 把 `PID_PARSE_SELF_CHECK_DIR`、`PID_ACCEPTANCE_CORPUS`、`PID_TEST_DATA_DIR` 指到一个不存在的目录，不设 `PID_PLOT_FONTS`。
    - 在临时检出里跑 `cargo test -p pid-plot -- --nocapture 2>&1 | Select-String 'skipping|note:|test result'`。
  - **期望读数：**
    - 每个测试要么打印 `skipping …`，要么没有跳过字样，也就是真跑。
    - `an_a2_sheet_draws_its_linked_a2_template` 打印 `note: … bundled-only symbol(s)`（期望 6 个）并通过。
    - `every_fixture_drawing_exports_a_pdf` 打印 `skipping fixture export; no font directory is present`。
  - **局限：** `refdata_archive_corpus` 的备份路径不走环境变量（只看了文件头 L50），这台机器上可能仍然找得到备份。它的读数单列，不算「CI 上真跑」。
  - **产出：** 一张逐条的表，写进 S8.3 的文档。
  - **代价：** 临时检出加独立 target 目录，要全量编译一次。不改仓库，跑完 `git worktree remove`。
- **S8.1 可行性探针，不改门禁。**
  - **做法：** 在 S8.0 的临时检出里，对每个 `test-file/**/*.pid` 建两次 `PaperModel`，各 `serde_json::to_string` 一次，比较两次是否逐字节相同，并量 JSON 的总大小。
  - **期望读数：** 两次完全相同；总大小有个上限，超过 5 MB 就停下汇报，改做 D4(b)。
  - **不确定时：** 如果模型里有绝对路径或 `HashMap` 顺序这类不稳定因素，这一步会暴露出来。修掉它们属于改 `pid-plot/src`，要另外批。
- **S8.2 金样测试。**
  - **改：** 新增 `pid-plot/tests/paper_model_golden.rs` 和 `pid-plot/tests/golden/paper/<夹具>.json`。设 `PID_PLOT_BLESS=1` 时重写金样。这个测试不设任何软跳过分支，输入全在仓库里。
  - **验证：**
    - 在临时检出里跑 `cargo test -p pid-plot --test paper_model_golden`：期望通过。
    - 在本机工作树里也跑一遍：期望通过。前提是金样在「没有 `symbols-full`」的条件下生成，并且测试本身限定只用仓库内的符号库；怎么限定，取决于 S8.1 的结论。
    - 本地临时改一个排版常量，不提交：期望测试红，并打印可读的 diff。
    - 推送之后，CI 日志里出现 `test … paper_model_golden … ok`。
  - **回退：** 删掉测试和金样，一个提交。
  - **代价：** 一个测试文件约 150 行，加上金样；以后每次有意改排版，都要多跑一次 bless。
- **S8.3 文档。**
  - **改：**
    - `AGENTS.md` 门禁表加一行，写金样测试，注明 CI 上真跑。
    - 写明 CI 上 pid-plot 出图门禁的覆盖情况，用 S8.0 那张表。
    - `.github/workflows/ci.yml` L42–48 的注释补一句：出图门禁在 CI 上软跳过及原因。
  - **验证：** fmt 通过；推送后 CI 绿。
  - **注意：** 和 S2 都改 `AGENTS.md`，两步排先后，不要同时改。

### S9 收尾门禁与记录

- **验证：**
  - 本机跑 `cargo test -p pid-plot` 全量（带字体和两份语料，用独立的 target 目录）。
  - `cargo clippy --workspace --all-targets -- -D warnings`。
  - `cargo fmt --all -- --check`。
  - 动了根 crate 的，再加根 crate 的 `cargo test` 和 `bash .github/scripts/check-missing-docs.sh`。
  - 同步更新 CHANGELOG、本单的「进度」，以及 `docs/plans/2026-10-29-plot-pdf-correctness.md` 的后续段。
- **推送：** 每批门禁全绿后推 `origin/main`。是每批都推还是攒几批，批准本计划时定，默认每批推一次，让 CI 跑到。
  推送后看 CI：期望 `test (stable)` 和 `cargo audit` 都 success；批了 D4(a) 的，金样测试要出现在日志里。
- **回退：** CI 红就先 revert 当批再查，不在 `main` 上叠修。

## 5. 分派

| 会话 | 步骤 | 前置 | 冲突 |
|---|---|---|---|
| 甲 | S1 → S2 | S0 | S2 与 S8.3 都改 `AGENTS.md`，排先后 |
| 乙 | S3 →（S4） | S0；S4 排在 S1 之后 | S4 与 S1 都改 `reference_plots_acceptance.rs` |
| 丙 | S8.0 → S8.1 →（S8.2 → S8.3） | S0 | 用临时检出，不碰主工作树 |
| 丁 | S5.0 → S5.1 → S6 → S7 | S5.0 要你批准提交 | 根 crate 与 publish 线未提交改动先查重叠 |

所有会话都只暂存自己的文件，不碰 publish 线的 18 个改动和 6 个 `tmp_*.rs`，不 stash 也不 revert 别人的改动。

## 6. 验收

- **交付：** `验收数据\plot` 的 11 张由最后一个提交写出，`BUILD.txt` 显示没有未提交改动，逐张读数等于钉子。
- **文字：** 普查超限 ≤ 64；8 对每一对都有结论，要么已修，要么已查未解并写明查了什么。
- **记账（批了 D2(a) 才算）：** 字节对照 0 差异；53 页加 7 个夹具「未归因丢弃 = 0」；现有门禁读数不变。
- **CI（批了 D4(a) 才算）：** 推送后 CI 绿，金样测试在 CI 上真跑；`AGENTS.md` 写明 CI 的出图覆盖。
- **门禁：** 本机全绿，CI 全绿。

## 7. 风险

- **D1(a) 在有未提交改动的工作树上跑门禁，也会覆盖交付。** `BUILD.txt` 会标出来，收口时在干净的树上重跑一次就行。
  要更严的话，可以改成「工作树不干净就不落盘」，批注里说一声。两个检出同时跑门禁会互相覆盖交付，用 `PID_ACCEPTANCE_DELIVER=0` 关掉其中一个。
- **D2 改动面大。** 字节对照是硬门槛，任何一页字节变了就停；耗时超过设计上限也停下汇报。
- **D3 可能查不到规则。** 每类只查一轮，不为凑数改阈值。
- **D4(a) 的风险。**
  - 金样会随有意的排版改动反复刷新，提交 diff 变大。
  - 模型可能不稳定，或者受是否有 `symbols-full` 影响。S8.1 不通过就退回 (b)，不硬上。
- **共用 target 目录。** 机器上有别的会话在编译，所有 `cargo` 命令都用独立的 `CARGO_TARGET_DIR`。代价是第一次要全量编译。

## 8. 等待清单（本单不碰，缺什么写什么）

下表沿用旧版，本轮没有重读 `2026-11-05-reference-plots-to-obtain.md`（**未复核**）。

| 项 | 缺什么 | 有了做什么 |
|---|---|---|
| 16 张未见证样式、`CPE-SQ-A2.igr` 掩膜 | 3 张 SmartPlant 原厂 PDF：N/O/P/Q 井场任一张、`test\U01\D04.pid`、任一张收球筒。要 Amyuni 驱动出的，与 `验收数据` 同一来路 | 按 `2026-11-05-reference-plots-to-obtain.md` 的「拿到之后怎么接」入验收集，未见证页 16 → 0 |
| 闪蒸 A / 低温 A 的图框差异 | 出图当时的工程版本，或出图机的环境 | 判断是环境差异还是版本差异 |
| `1200td`、`N井场` 页幅大 2–3 mm 的放置子问题 | 它们的参考出图 | 核实「出图器会不会拉伸图框」 |

## 9. 明确不在本单

- A2 仪表簇：`.pid` 里存的是拖动后的位置，属源侧。
- 闪蒸 A / 低温 A 多画的 `CPE-SQ-A1.igr` 图框：`.pid` 里没有区分字段。
- 文字残差里已归源侧或环境的 56 对：模板字横向缩放 40、借字 14、两条标签合串取整 1、A2 公司名 1（CT-4）。
- 约 400 条重复图元、100 多条空文本：已定只拆账、不去重。
- D4(c) 自托管 runner；把 Windows 字体提交进仓库。
- publish 线的 18 个未提交改动、6 个 `pid-plot/examples/tmp_*.rs`、goally `ct*` 产物：都不动。

## 10. 进度

- 2026-11-08 11:24 开单，基点 `c9202ff`。
- 2026-11-08 重写：基点改为 `e5fa361`（已推送，与 origin 0/0）；D4 按 CI 的新事实重写，推荐项改为 (a)；补上「规格未跟踪」、注释范围、落后提交数这几处前提；新增 S8（CI 金样），收尾挪到 S9。待批。
