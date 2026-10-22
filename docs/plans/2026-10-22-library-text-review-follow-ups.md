# 库侧文字三种形状的审查收口：收紧解码、定选表规则、补文档与回归（2026-10-22 开单，同日 Plannotator 批准，无批注）

> 用户 2026-10-22：「审核最近的代码修改」→「使用 plannatator 制定下一步的计划」。
> 开单只写本文件，不改代码，不提交。
> **2026-10-22 Plannotator 批准**（`{"decision":"approved"}`，无批注），D1–D6 按推荐项。批准之后才按 §4 落地。
> 审查对象：工作区里未提交的「库侧按 `igTextBox` 三种形状读文字」（`src/symbol_library.rs`、`src/parsers/sheet_records.rs`、`examples/probe_sym_text_runs.rs`、`examples/probe_sym_text_shape_gap.rs`、`docs/analysis/2026-10-22-the-symbol-library-never-got-the-third-text-shape.md`、`docs/analysis/2026-08-13-igtextbox-has-three-shapes.md`、`docs/analysis/2026-10-22-the-normal-view-that-drew-nothing.md`、CHANGELOG），以及同日已提交、与它互相影响的选表逻辑（`81c5fa5`、`2e61816` 的 `displayed_primitives`）。基线 `72610a5`。
> 审查时本机命令执行是关的，没有 `git diff`，是对照当前代码和 CHANGELOG / 分析页的描述审的。批准后用 `git status` / `git diff` 核对过：工作区改动正是上面这些路径（分支 `codex/phase32c-bundle-closeout`，领先 origin 16 笔），`src/` 的 diff 与审查结论一致；这次删掉的旧单测 `text_record_is_skipped_when_the_count_does_not_account_for_the_payload`（payload 多一个字节就拒）正是 R2 说的那道检查。
> 本单的编译、测试、探针都由用户在本机跑，读数贴回后写进分析页。

## 一句话

三种形状的读法本身是对的：两侧共用 `igtextbox_body_shape`，尾块从 `22 + body_len` 起算，插入点读 `tail_start`。但「计数跑出 body」那条拒收不起作用；尾巴放宽成「留得下」以后，库侧也没有了防计数多读的检查；新读出的 606 条文字还会参与选表和包围盒。先量三件事，再收紧解码、定选表规则、补文档和真实文件回归。库与缓存体的 run 这次只量不改。

## 1. 审查发现

| # | 发现 | 落点 | 去处 |
|---|---|---|---|
| R1 | `text_layout` 的 `text_end > tail_start` 永不成立：三种形状的 `body_len` 都由计数算出，形状 1 / 2 的 `tail_start == text_end`，形状 3 多 `8*(A+B)`。`text_record_is_skipped_when_the_count_reaches_past_the_body` 实际是被「留得下尾块」拒掉的；CHANGELOG 与分析页 §四 的「仍拒」清单同样写错 | `src/symbol_library.rs` | S2 |
| R2 | 放宽成「留得下」后，库侧不再有防计数多读的检查。绘图侧靠方向对（单位向量或字高带；`IGTEXTBOX_DIRECTION_UNIT_TOLERANCE` 的文档说这是唯一能发现多读的检查），库侧只剩 UTF-16 合法、无 NUL、插入点有限且不超过 10 m。形状 1、3 只有一份计数；多读 8 个字时「插入点」正好读到真方向对 `(1.0, 0.0)`，可能被收下。`coordinate()` 也放过次正规数 | 同上 | M1 → S2（D2） |
| R3 | `displayed_primitives` 的 `drawing` 与 `fattest` 都把 Text 计入。81 个「空表」现在挂着自己的标签，而它们的本体已被跨表去重让给字面序在前的孪生表；标签若在显示层上，干净表会只带着标签当选，本体丢掉（`2e61816` 修掉的那一类）。现有证据（`probe_normal_view_too_wide`、`probe_library_vs_cached_bodies` 的 190 对）只覆盖图纸里放置过、带缓存体的符号 | 同上 | M2 → S3（D4） |
| R4 | `bounds()` 计入全部图元，隐藏层上的模板文字也算：`(84.2, 129.2) mm` 19 条、`Fail Action.sym` 的 `(107.95, 139.7) mm`，包围盒会被撑到一百多毫米。仓库内只有两个单测调用，但它是公开 API | 同上 | M2 → S4（D5） |
| R5 | `decode_igtextboxes` 文档的校验规则过时：写 `>= 68`（代码是 60）、长度在 `+30`（实际按形状在 +22 / +30 / +26）、文字后 4 个 double（实际在 body 后，形状 3 在 run 后）、只认单位向量（实际也收字高缩放的方向对） | `src/parsers/sheet_records.rs` | S4 |
| R6 | `SymbolLettering::anchor_code`、`SheetIgTextBoxDecoded::anchor_code` 的文档还用 `(code & 0x0F) % 3` 解释竖直锚点，10-19 已推翻（符号标签另有读法） | 两个文件 | S4 |
| R7 | `displayed_primitives` 文档与注释里的 431 / 868 / 163 是文字修好之前量的 | `src/symbol_library.rs` | M2 → S4 |
| R8 | `fattest` 为 `None` 时没有任何显示图元，后面 `Sheet6` / `unknown` / `alternate` / 全部 四个分支怎么选，`visible` 都为空、落到整体返回：行为上是死代码 | 同上 | S4 |
| R9 | `lettering_of` 文档里链接外又套了引号和括号；`text_record_decodes_shape_three_past_its_counters` 用 `.len()` 当字符数 | 同上 | S4 |
| R10 | 尾后 32 / 40 / 48 / 56 字节很可能是绘图侧已认识的 kind-2 尾：`+20 == 2` 时尾块 = 36 字节定位尾 + 4 字节标志字 + 文本框宽高（尾 `+40` / `+48`）+ 三个计数 = 68（`IGTEXTBOX_TAIL_KIND2_LEN`），比 36 多 32；再多的 8 / 16 / 24 是标志字控制的可选字段 | 两侧 | M1 → S2（D3） |
| R11 | 只有合成字节单测，没有真实文件回归；库侧 `decode_text` 没有截断扫描（绘图侧有 `trunc_len` 那组） | `src/symbol_library.rs` | S2、S5 |
| R12 | 符号文字的字样只走 `+14` 段落跳：缓存体的 `SymbolLettering::from_decoded` 与库侧的 `lettering_of` 都不读 run，而绘图侧 `style_link::resolve_text_style` 已让 run 优先。库侧 2266 条可比记录里 1350 条 run 字高与段落默认不同 | `src/symbol_library.rs`、`src/style_link.rs` | M3（D6） |

## 2. 范围

**做**：R1–R11 收口；M1–M3 三项测量；按 D4 定选表规则；真实文件回归。

**不做**：run 落地（除非 D6 选 (b)）；库里带框的标签按框边起排（除非 D3 选 (b)）；绘图侧 `decode_igtextbox_payload` 的长度规则（它有方向对兜底，不动）；41 条 selector-1 样式 id 未定义、2 条空文本记录；OpenCADStudio 侧改动。

## 3. 要拍板的

| # | 问题 | 选项 | 推荐 |
|---|---|---|---|
| D1 | 工作区里的三种形状改动怎么提交 | (a) 原样一笔提交，审查发现各自成笔；(b) 先改掉 R1（死判断与写错的「仍拒」）再一起提交 | **(a)**。审查对象保持原样，后面每笔只做一件事 |
| D2 | 库侧防多读（R2） | (a) 照绘图侧：方向对须是单位向量或落在字高带，否则拒；判据提成 `sheet_records` 里一个 `pub(crate)` 函数两侧共用，绘图侧行为不变；(b) 只在 `lettering_of` 里记下不合格，不拒收 | **(a)**，前提是 M1 量出 2864 条里不合格为 0；不为 0 先停下来看那几条 |
| D3 | 尾后字节（R10） | (a) M1 确认是 kind-2 尾后，用尾型做精确长度校验：kind 1 尾块正好结束 payload，kind 2 至少 68 字节且余数是 8 的倍数（标志字与字节数的对应若量得出就照它）；(b) 在 (a) 之上把文本框读进 `SymbolLettering`，pid-plot 让库里带框的标签从框边起排；(c) 不动，仍「留得下」 | **(a)**。(b) 只影响靠库画的放置，语料里是 0；M1 若不是干净的划分就退到 (c) |
| D4 | 文字参与选表（R3） | (a) 先量（M2）：带文字和只数几何两种选法在全库没有差异，就只加单测锁住现状；有差异再在 (b) / (c) 里选；(b) `drawing` / `fattest` 只数几何，文字只做平局，纯文字符号单独兜底；(c) 治根：读表时让共享笔画留在正常视图那张表上（改 `read_symbol_geometry` 的去重顺序），干净表保住自己的本体和标签 | **(a)**。(c) 会改 `primitives` 的归属（公开字段），没有数字不动 |
| D5 | `bounds()`（R4） | (a) 语义不变，CHANGELOG 写明会计入隐藏层文字；(b) 改成只算 `displayed_primitives`；(c) 新增 `displayed_bounds()`，`bounds()` 不变 | **(a)**。仓库内没有生产调用；若 OpenCADStudio 用它摆放或框选，改选 (c) |
| D6 | 符号文字的 run（R12） | (a) 本单只做 M3 测量，落地另开单；(b) 本单落地：缓存体 `from_decoded` 走 `resolve_text_style(...).effective()`（提为 `pub(crate)`），库侧用 `igtextbox_runs`（提为 `pub(crate)`）读出 run 后按同一规则取字样；(c) 不做 | **(a)**。它改页面上符号文字的字号，会动 `plot_compare` 的文字门限，值得单独一单 |

## 4. 步骤

每步一个提交，只 `git add` 本步路径；别的会话弄脏的文件不收。每步的门：`cargo test -p pid-parse --lib symbol_library`、`cargo test -p pid-parse --lib sheet_records`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo fmt --all -- --check`；动到选表或文字时再加 `cargo test -p pid-plot --test plot_compare --test export_every_fixture --test export_plant_backup`，以及只对 `-p pid-parse` 的 `--no-default-features` 与 wasm32 检查。门由用户在本机跑。

### S0 现有改动落一笔（D1）

**已落地** `b45f937`（8 个文件，原样）。

以 `git status` / `git diff` 为准，预计是开头「审查对象」那几条路径；CHANGELOG 只收与本改动有关的段落。提交信息沿用分析页的结论和读数。

### S1 测量（不改 `src/`）

- **M1**（扩 `examples/probe_sym_text_shape_gap`）：按绘图侧规则数方向对不合格的条数；按 `+20`（尾型）× 尾后字节数分组；kind-2 记录的标志字（尾 `+36`）与尾后多出字节的对应；文本框宽高是否可用。
- **M2**（新探针 `examples/probe_sym_pick_with_and_without_text`）：两个库根的每个 `.sym`，比较现行 `displayed_primitives` 选中的表，与把 Text 从 `drawing` / `fattest` 计数里拿掉后选中的表；列出不同的符号、两种选法的表号、决定性那张表上的文字及所在层。顺带：`bounds()` 全部图元与只算显示图元相比、包围盒放大超过 2 倍的符号数；重量 R7 的 431 / 868 / 163。
- **M3**（扩 `examples/probe_sym_text_runs` 或新探针）：缓存体（`test-file` 下 7 个 `.pid` 与 53 张备份）里每条符号文字，段落默认与 run 在字高 / 字体 / 颜色上的分歧条数；五张参考图上会因 run 变字号的条数；顺带看 `Electric Motor Type 1.sym` 那条 `M` 有没有 run、run 指的字高是多少。

读数追加到 `docs/analysis/2026-10-22-the-symbol-library-never-got-the-third-text-shape.md` 新一节「§六 审查之后的三项测量」；CHANGELOG 和 `task_plan.md` 各记一笔。

### S2 解码收紧（R1、R2、R10、R11 的一半；D2、D3）

- 删 `text_end > tail_start`。`sheet_records` 新增 `pub(crate)` 的方向对判据，`decode_igtextbox_payload` 改用它（行为不变），库侧 `decode_text` 用它拒收、`lettering_of` 用它取字高；`lettering_of` 里「都不像就当 0.0」那一支随之去掉。
- D3 (a) 时按尾型做精确长度校验。
- 单测：count-past-body 那条改写成「尾后 48 字节 + 计数多读 8 字 → 拒」；方向对不合格 → 拒、字高缩放 → 收；截断扫描（形状 3 + 尾后字段，每个前缀不 panic、尾块不完整返回 `None`）；D3 (a) 时 kind 1 尾后多字节 → 拒、kind 2 的 68 + 8k → 收。
- 改正 `text_layout` 的注释，以及 CHANGELOG、分析页 §四 的「仍拒」清单。
- 验收：M1 的 2864 / 2866 不变（或写明哪几条、为什么被拒）；`probe_normal_view_too_wide` 的 11 条规则、`probe_library_vs_cached_bodies` 的读数不变。

### S3 选表规则（R3；D4）

M2 没有差异：加单测锁住现状，写明「干净表上只有显示层的标签、本体在孪生表上」时选哪张。有差异：按用户在 (b) / (c) 里的选择改，加同一条单测；重跑 M2 和两个选表探针，读数写进分析页。

### S4 文档与清理（R4–R9；D5）

- `decode_igtextboxes` 的校验规则按现行代码重写。
- 两处 `anchor_code` 文档按 10-19 的结论改写（`docs/analysis/2026-10-19-a-pair-of-flags-not-an-index.md`、`docs/analysis/2026-10-19-symbol-label-nibble-hangs-from-the-middle.md`），不再写 `% 3`。
- `displayed_primitives` 文档与注释里的数字换成 M2 重量的数。
- `displayed_primitives` 收成 drawing → fattest → 整体返回，删掉 `unknown` / `alternate` 两个集合；加单测「没有任何显示图元 → 整体返回」，行为不变。S3 若按 D4 (b) 改成只数几何，这几支对纯文字符号又会起作用，收之前按 S3 的结果重看。
- `lettering_of` 文档的链接；单测里的 `.len()` 改成 `encode_utf16().count()`。
- `bounds()` 按 D5。

### S5 真实文件回归（R11 的另一半）

在 `symbol_library` 单测里加软跳过的 `test-file/symbols-full` 用例：`Fail Action.sym` → `CSC NULL` @ `(0.10795, 0.1397)`（形状 3）；`Nozzle Rating.sym` → `TBD`（形状 2、尾后 48 字节）；`Nozzle NPD.sym` → `1"`（形状 3）。再加一条全库棘轮：`test-file/symbols-full` 上 `0x004D` 被跳过的条数钉成实测值（预计不超过 2）。

## 5. 验收

- 库侧 `0x004D` 读数 2864 / 2866 不变，或变化有逐条理由；「计数多读 + 尾后字段」的合成记录被拒。
- M2 的差异清单为空，或按 D4 改完后为空、每条写明。
- 五张参考图读数逐位不变；`export_every_fixture`、`export_plant_backup` 通过。
- `decode_igtextboxes`、`anchor_code`、`displayed_primitives` 的文档与代码和数据一致；CHANGELOG 与分析页的「仍拒」清单正确。

## 6. 风险

- 方向对检查若拒掉真实记录（M1 不为 0），先停，不硬上。
- D4 (c) 会改 `primitives` 的归属，直接遍历 `primitives` 的消费者（OpenCADStudio、探针）都要回归。
- S0 提交时工作区里可能有别的会话的文件；只加本单路径。
- D6 (b) 会改页面上符号文字的字号，`plot_compare` 的文字对上与类别位移门限可能要重锁。

## 门禁记录

- 2026-10-22：审查后开单，D1–D6 待 Plannotator 审。
- 2026-10-22：Plannotator 批准（`plannotator annotate docs/plans/2026-10-22-library-text-review-follow-ups.md --gate --json` → `{"decision":"approved"}`，无批注），D1–D6 按推荐项。
- 2026-10-22：用户「直接开工修复」→ S0 落地 `b45f937`；S1 的 M1 / M2、S2、S5 先写好（M1 扩 `probe_sym_text_shape_gap`，M2 新探针 `probe_sym_pick_with_and_without_text`，S5 放在新的集成测试 `tests/symbol_library_text.rs`，与 S2 分文件），门由用户跑过再分三笔提交；M3 下一批。
