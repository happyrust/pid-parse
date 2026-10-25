# `JStyleOverride` 自带的宽 / 色 / 虚线位要生效 · 修复计划（2026-10-10 开单，同日 Plannotator 批准，五条决策按推荐）

> 用户 2026-10-09 23:51（zhimo 会话 fable-5-1-6）：「按 A 的修法在 pid-parse 里让 `resolve_line_style` 应用覆盖的宽 / 色 / 虚线位，探针和分析页一起入库，再重签 OCS 的 `pid_import` 和六张哈希」；
> 2026-10-10（会话 fable-5-1-17 接手）：「使用 plannator 制定修复计划」。开单时只做计划，**没改任何代码**。
> **2026-10-10 Plannotator 批准**（`{"decision":"approved"}`，无批注）；用户随后：「把计划文件挖到 pid-parse 的 docs/plans 里再开工」——本文件遂由 OCS `docs/plans/` 挪到这里（pid-parse 是改码的仓），§3 五条决策全部按推荐项执行。
> 来源：10-09 用 main `aaedd334` 的 exe 开 `DWG-0201GP06-01.pid` 与 SmartPlant 参考图 `D:\work\cad\图片验证\pid-1.png` 逐实体对比，七条差异里的 **#2「设备表 14 条格线 OCS 画成虚线、参考图是实线」**，
> 以及随后在 pid-parse 上跑的临时探针给出的根因（见 §1）。10-09 的临时证据目录 `%TEMP%\ocs-merge\compare\`（探针源码、`probe-output.txt`、对照截图）**已被系统清掉**，下面引用的数字来自 10-09 的报告，A0 会重新量出来。
> 基线：OCS `main` `aaedd334`（工作树干净，只有未跟踪的 `.kiro/` 与 `docs/evidence/2026-10-09-pid-gui-check-after-upstream-merge/`）；pid-parse `1ed140a`（`codex/phase32c-bundle-closeout`；OCS 两棵工作树都以 `path = "../pid-parse"` 依赖它这棵树）；
> `ocs-pid-0928` 工作树 = `pid-web-open` `84b24e07`，领先 `main` 24 个提交，且有另一会话（opus-5-5-8，X2 文字基线）**未提交**的 5 个文件（`src/io/pid/{build,mod,tests,text}.rs`、`tests/pid_import.rs`）。
> pid-parse 工作树上另一会话的 4 M + 1 ??（`docs/analysis/2026-09-20-…`、`2026-09-21-parsing-pipeline-audit.md`、`docs/plans/2026-09-21-style-link-…`、`examples/probe_jflavorholder_…`、`examples/probe_text_anchor_by_kind.rs`）本单**一个字节不碰**。
> 上一单：`pid-web-open` 上的 `2026-10-08-pid-afternoon-review-uid-join-and-merge-handoff.md`（D2 已落地、X2 等提交）。本单与 X2 **不相交**（X2 改文字，本单改线）。

## 一句话

设备表 14 条格线的样式链是 `igLine2d +14 → JStyleOverride(86 / 101) → JStyleSimpleLine 85 "Dashed" 0.35 mm`，`resolve_line_style` 一直只顺着 `+22` 的 `base_reference` 取**基样式**的宽 / 色 / 虚线，
覆盖记录自己 90 字节里的字段一个没读。把语料里 13 条被线几何经过的覆盖摆在一起看，`+30` 是「覆盖了哪些属性」的位掩码：bit0 颜色（`+34` COLORREF）、bit1 线宽（`+42` f64 米）、bit7 虚线（槽位 0 = 不画虚线）——
SmartPlant 实际画的是**横线实线 0.13 mm、竖线实线 0.18 mm**，参考图证实。修法是让走覆盖的那条分支读掩码、逐位替换；OCS 不改码，只重钉 `pid_import` 与批量基线六张 `dxf_sha256`。

## 1. 根因与证据（10-09 探针，corpus + 屏幕两证；native-reader 一证见 A1）

| 事实 | 证据 |
|---|---|
| 0201 样式带虚线的线 15 条：**14 条格线全 `hop=ViaOverride`**（覆盖 86 ×11、101 ×3 → 基样式 85 `Dashed`），唯一 SmartPlant 也画虚的 LIA–LIT 电信号线是 **`hop=Direct`**（样式 79 `Electric`，`PID-DASH-2` 1.905 / 1.27 mm） | 分界线就在 hop 上；不是正负号约定（两条虚线样式的段长都是全负数） |
| `+30` bit0 `0x01` = 颜色 → `+34` | 0202 覆盖 109 `+34`=`0x000080` 暗红（设备色）；0201 覆盖 50 `+34`=0 黑，它管的 3 条 `i=0.3 %` 坡度三角 OCS 今天按基样式画成粉红 `#FE0060`，**参考图里那一块没有一个粉色像素**（`ref-slope-x8.png`，已丢） |
| `+30` bit1 `0x02` = 线宽 → `+42` f64 米 | 13 条里的值全是 0.13 / 0.18 / 0.35 mm，都在 ISO 梯子上 |
| `+30` bit7 `0x80` = 虚线类型，槽位 0 = 不画 | **只有基样式是 `Dashed` 的覆盖才有这一位，而且个个都有**（0201 86 / 101，0202 136 / 138 / 140；140 的掩码就是单独一个 `0x80`）；参考图里设备表是实线 |
| `0x100` / `0x4000_0000` | 填充族覆盖（基样式 `0x002A Solid`）的位，另一族，**不在本单** |
| 字段位置与 08-04 native-reader 版式一致 | `docs/analysis/2026-08-04-jstyleoverride-native-reader-settles-it.md` §1：v3 `+30`→`this+96`(u32)、`+34`→`this+100`(u32)、`+42`→`this+104`(f64)；Phase 16 审计解码器 `SheetJStyleOverrideDecoded` 的 `field_b_u32` / `field_c_u32` / `field_1_f64` 就是这三格（它的 `+0` = 本文的 `+26`） |

**预计语料影响**（A0 重新量，以探针为准）：0201 14 条格线变实、宽改 0.13 / 0.18；7 条 `Normal` 覆盖线 0.35 → 0.13；3 条坡度标记粉 → 黑。0202 8 条格线变实、19 条变细、1 个放置改暗红 0.35。工艺 6 条变细。D06、A01 预计不变（D06 `via_override` 0；A01 由探针确认）。

## 2. 范围

**做**：pid-parse `style_link::resolve_line_style` 走覆盖时应用 `+30` 掩码；探针入库；分析页（08-07 虚线篇的续篇）；棘轮重钉；CHANGELOG / `task_plan.md`；OCS `tests/pid_import.rs` 重钉；批量基线 `report.csv` 重签 + README 加一段；两仓各一个提交。
**不做**：B 标签锚点十字（OCS `build.rs`，另开小单）；#3 白底 / #4 线宽显示（显示偏好，你定）；#1 文字基线（X2，`pid-web-open`）；覆盖的填充位；`resolve_fill` / `resolve_text_*` 的覆盖语义（语料上没有被几何经过的此类覆盖，不碰）。

## 3. 要你拍板的

| # | 问题 | 选项 | 推荐 |
|---|---|---|---|
| A-D1 | bit7 置位时虚线槽位怎么读 | (a) 槽位 0 → `dash = None`；非零 → 按 style id 查 `JStyleSimpleDashType`，查不到 → `None`（语料全 0，非零分支只有合成字节单测覆盖）；(b) 一律 `None` | **(a)**：和 `dash_of` 同一套「悬空引用 = 实线」语义，多的只是一行查表 |
| A-D2 | 公开类型怎么变 | (a) `ResolvedLineStyle` **形状不变**（`style_id` / 名字仍是基样式的，`hop` 仍是 `ViaOverride { override_id }`），覆盖读到的四格挂在 `StyleRecord` 新增的 `Option` 字段上，想知道「覆盖了什么」用 `table.get(override_id)`；(b) `ResolvedLineStyle` 加 `overridden: OverrideMask` 字段 | **(a)**：OCS `src/io/pid/tests.rs:448 / :559` 直接构造 `ResolvedLineStyle { …, hop }`，(b) 会逼 OCS 改码，违背「OCS 不改码」 |
| A-D3 | native-reader 一证 | (a) 先花 ≤ 1 h 在 `style.dll` 里核 `JStyleOverride` 读 `this+96` 各位的方法，把位名写进分析页；(b) 跳过，分析页标「corpus + 屏幕两证」 | **(a)**，时间盒 1 h；超时按 (b) 入页、在 §7 登记 |
| O-D1 | OCS 重签在哪棵树 | (a) `main`（干净；10-09 对比用的就是它的 exe；`pid-web-open` 合回时再按同样数字重签一次）；(b) `pid-web-open`（PID 主线、上一单 D2 的做法；但 X2 未提交占着 `tests/pid_import.rs`，得等它提交或拿锁协调）；(c) 两棵都签 | **(a)**，X2 提交后若你要，我接着在 `pid-web-open` 上重做 O1–O2（纯数字，半小时） |
| O-D2 | 要不要 GUI 复核一张 | (a) 重编 OCS、Vulkan 开 0201、截设备表局部与参考图并排进 `docs/evidence/2026-10-10-…`；(b) 只信 DXF 逐实体比 | **(a)**，这是用户肉眼发现的差异，收口也该肉眼看一次（约 20 min，含编译） |

## 4. 步骤

### pid-parse（都在 `D:\work\plant-code\cad\pid-parse`，分支 `codex/phase32c-bundle-closeout`）

- **A0 探针入库** `examples/probe_override_applies_its_own_fields.rs`（重做 10-09 的临时探针）：用现有公开 API（`line_styles_for_document` / `style_names_for_document` / `DocumentStyleTable::records()` / `symbol_definition`）加 `cfb` 读原字节，五张图逐张输出
  ① 每条 `0x0030` 覆盖的 `+26..+90` 逐字段（u32 / f64 / u16 原值）、`+30` 掩码按位拆开；② 每条覆盖被哪些几何（oid、族、所在流）经过、基样式 id / 名 / 宽 / 色 / 虚线；③ **「前 → 后」表**：应用掩码后每条线的宽 / 色 / 虚线会变成什么，按图汇总成调色盘差分；④ 确认 bit7 的槽位偏移（候选 `+38` / `+74` / `+78` / `+82`：在 13 条里找「bit7 置位的全 0、不置位的也 0」说明不了，要连同 JSite 存储里全部 48 条覆盖一起看哪一格在 `0x0030` 上像 style id）。
  输出进分析页；`src` 不动；clippy 两种特性过。
- **A1 native-reader 核位**（A-D3，时间盒 1 h）：IDA 开 `dlls/style.dll.i64`，从 `JStyleOverride` v3 worker `sub_10028130` 写 `this+96` 处往上找读它的方法（`IJStyleOverride*` 的 get / put 或「取有效样式」的合成函数），把每一位对应的 put 方法名记下。找到就把 §1 的三行升为 native-reader 级。
- **A2 代码**（`src/style_link.rs`）：
  - `StyleRecord` 新增 `override_mask: Option<u32>`、`override_colour: Option<u32>`、`override_width_m: Option<f64>`、`override_dash_reference: Option<u32>`，只在 `type_code == 0x0030` 时填（`read_override_*`，偏移以常量命名，注释写明 level）；`override_width_m` 走和 `read_symbology` 同一道合理性闸（0 或非正常 / 超 `MAX_PLAUSIBLE_WIDTH_M` → `None` = 不替换）；`override_colour` 高字节非零或 `0xFFFF_FFFE` 哨兵 → `None`。
  - `resolve_line_style` 的覆盖分支：以基样式为底，bit1 且宽可读 → 替宽；bit0 且色可读 → 替色；bit7 → 按 A-D1 替 `dash`；`marker` 照旧取基样式的（覆盖不动终结符，语料无反例，注释登记）。掩码没置的位、不认识的位一律不动。
  - 模块头文档（第 1–113 行）加一段：覆盖不只是指针，它自己也带一层属性；`docs/pid-format-guide.md` 若有 `JStyleOverride` 字段表就同步一行。
  - 单测（合成字节）≥ 6：掩码 0 等同今天；bit0 只换色；bit1 只换宽；bit7 槽 0 关虚线；bit7 槽非零指向一个 `0x002F` 则取它；宽不合理 / 色是哨兵时不替换；未知位忽略。现有 `an_override_resolves_through_the_base_reference` 等 4 条照旧过。
- **A3 棘轮与全量**：`tests/style_link_ratchet.rs` 的 `EXPECTED_PALETTE`（12 项，`0.350mm #000000` 196 / `0.130mm #000000` 182 / `0.350mm #FE0060` 6 等）按 A0 的差分重钉，并**加一条**专门钉 0201 格线：14 条 oid 全部 `dash == None`、宽 0.13 / 0.18、`hop == ViaOverride`；`via_override` 计数不变（走的还是同一条路）。
  `geometry_golden_snapshot`（只序列化几何）、`parse_real_files` 的 Phase 16 审计解码器、`render_gap_census`、`geometry_profile` 预期不变——变了就是探针没量到的影响，先弄清再改。
  跑：`cargo test --workspace`（上次 42 个二进制 1503 / 3 忽略）、`--no-default-features`、`clippy --workspace --all-targets -D warnings` 两种特性、`cargo check --lib --no-default-features --target wasm32-unknown-unknown`、`cargo fmt --all --check`（另一会话的 `probe_text_anchor_by_kind.rs` 本来就不干净，忽略它）。
- **A4 文档**：`docs/analysis/2026-10-10-an-override-repaints-width-colour-and-switches-the-dash-off.md`（08-07 虚线篇的续篇：路径、13 条覆盖逐字段表、掩码各位的三证等级、前 → 后差分、与参考图的对照、A-D1 的理由）；CHANGELOG `[Unreleased]` 一段；`task_plan.md` 「当前阶段」一段；08-07 篇末尾加一句「覆盖可把虚线关掉，见 10-10 篇」。
- **A5 提交**：`git add` 只加我碰过的明确路径（`src/style_link.rs`、`tests/style_link_ratchet.rs`、新探针、新分析页、08-07 篇、`CHANGELOG.md`、`task_plan.md`、`docs/pid-format-guide.md` 若改、本计划），提交信息按仓库惯例写清数字与验证。

### OCS（按 O-D1 的树；下面按推荐的 `main` 写）

- **O1 `pid_import` 重钉**（`tests/pid_import.rs`，预期红的三处）：
  - `line_work_carries_the_width_and_colour_the_drawing_states`（:146）：0201 调色盘 `" 35 #000000" 43` / `" 35 #FE0060" 3` 等按 A0 差分重钉（预计新增 `" 13 #000000"`、`" 18 #000000"` 两项，`#FE0060` 归零后删行）。
  - `dashed_line_work_carries_a_linetype_matching_the_decoded_pattern`（:360）：3.5 / 1.75 mm 不再从 0201 的图纸线上出现（14 条格线走 `Continuous`），只剩电信号线的 1.905 / 1.27；改成钉「`used` 恰好一个 `PID-DASH-*`、图案 = imperial、14 条格线 oid 的 `linetype == Continuous`」，注释写明原因与来源。
  - 0202 / 工艺若有调色盘或虚线计数断言被探针差分波及（0202 8 条格线、19 条变细、1 个放置暗红；工艺 6 条）→ 一并重钉；`a_cached_strokes_dash_is_its_own_storages_not_its_placements`（:440）看 A0 对 JSite 覆盖的统计决定。
  - `cargo test --test pid_import`（main 今天 76 条）与 `cargo test --lib io::pid` 全绿；OCS **`src` 不改**。
- **O2 批量基线重签**：`cargo run --example pid_batch_report -- ..\pid-parse\test-file --export-dir $env:TEMP\pid-batch > report-new.csv`，按 README 的 `Compare-Object` 比；预期**只变 `dxf_sha256`**，`drawn` / `decoded` / `missing` / `refused` 等 17 列不动；预计变 4 行（0201、0202、publish-0202、工艺），D06 / A01 不变。
  改前 / 改后 DXF 逐实体比（去句柄类组码）：**只允许** 线宽 `370`、颜色 `62` / `420`、线型 `6` 三类组码变，实体数、坐标、图层一个不动；`LTYPE` 表若 `PID-DASH-1` 在 0201 上无人引用则可能少一条，登记。
  `report.csv` 覆盖、README 加「2026-10-10 重签」一段（OCS / pid-parse 两个提交号、变了哪几行、为什么）。
- **O3 GUI 复核**（O-D2）：`cargo build`，`--backend vulkan --http <port>` 开 0201，REST 截设备表局部，与 10-09 的参考图同区并排；证据进 `docs/evidence/2026-10-10-pid-table-lines-after-override-fix/`（连同 10-09 那个未提交的 evidence 目录一起入库，由你定）。用完 kill，不留后台进程。
- **O4 提交 OCS**：只加 `tests/pid_import.rs`、`docs/evidence/2026-09-24-pid-batch-baseline/{report.csv,README.md}`、O3 的 evidence 目录；本计划在 pid-parse，「进度」段在那边补。

## 5. 验收

1. 0201 设备表 14 条格线：`dash == None`、宽 0.13（横）/ 0.18（竖）mm、黑、`hop == ViaOverride`；LIA–LIT 电信号线仍虚（`Direct`，不受影响）；3 条坡度标记黑。
2. pid-parse：全量测试、`--no-default-features`、clippy 两种、wasm32 check、fmt 全过；棘轮数字与探针差分逐项对得上；golden 不变。
3. OCS：`src` 零改动；`pid_import` 全绿；`report.csv` 只有 `dxf_sha256` 列变、且只变探针预告的那几行；DXF 逐实体比只有 `370` / `62` / `420` / `6` 变。
4. 文档：探针、分析页、CHANGELOG、`task_plan.md`、README 重签段、本计划「进度」段齐；每条数字都能从探针输出里指回去。
5. 肉眼：设备表格线在 OCS 里是细实线，与参考图 `cmp-1-table` 同一观感（O-D2 选 (a) 时）。

## 6. 风险与边界

- **掩码还有别的位**：13 条里只见 bit0 / bit1 / bit7 与填充族两位；A0 把 48 条全列出来，发现新位只登记不实现。
- **宽 / 色读到不合理值**：不替换、沿用基样式（闸与 `read_symbology` 同一套），绝不让一条线因为覆盖读坏而消失或变白。
- **JSite 存储里的覆盖**：符号本体逐笔样式也经 `resolve_line_style`（`symbol_library.rs:576`、`streams/jsite.rs:135`），A0 统计它们有没有置位；有则 OCS 符号调色盘断言（:2648 一类）要跟着重钉。
- **两棵 OCS 树共用一棵 pid-parse**：A5 一提交，`pid-web-open` 的 `pid_import` 立刻红，直到它按 O1 重钉；O-D1 选 (a) 时在本计划与 `pid-web-open` 的 10-08 单里各登记一句。
- **测试数会变**：pid-parse `--lib` +≥6、棘轮 +1；写进 CHANGELOG。
- **不做的事**：不改 `decode_jstyle_overrides`（Phase 16 审计 DTO 字段名保留）；不动 `resolve_fill`；不碰别的会话的 5 + 5 个未提交文件。

## 7. 进度

- 2026-10-10 开单（OCS `main` `aaedd334`、pid-parse `1ed140a`，会话 fable-5-1-17）；Plannotator 批准，五条决策按推荐；计划文件挪到 pid-parse。
- **A0 落地**：`examples/probe_override_applies_its_own_fields.rs`。5 张图 **53 条**覆盖（不是 10-09 数的 13 条——那次只数了根表；`JSite*` 本体表里还有 40 条），全部 90 字节，`+50/+58/+66/+74/+82/+86/+88` 全 0。
  掩码只见 `0x02` / `0x03` / `0x80` / `0x82`（线）与 `0x40000100`（填充 ×4，线几何不经过）。**bit7 的槽位是 `+78`**：53 条里只在 bit7 置位时非零，非零的两条（0201 `/JSite329` 48、`/JSite396` 9）都指向本表 `0x002F "Dash"`——覆盖也能把实线基样式变虚。
  图纸级差分：0201 `0.350 #000000` 48→27、`#FE0060` 3→0、新增 0.13 ×21 / 0.18 ×3，虚线 15→1；0202 59→38、`#FE0060` 3→0、新增 0.13 ×22 / 0.18 ×2、一个放置 → `0.35 #800000`，虚线 9→1；工艺 6 条 0.35→0.13；**A01 46 条管线加粗（37 → 0.50、9 → 0.70）**——10-09「A01 不变」的预估是错的；D06 不变。
  缓存本体：09-20 P-E8 的 57 笔「可见虚线」全是 bit7 覆盖 → 实线；真正虚的只剩 0202 污水池直接点名 `Dashed` 的 3 条 + 0201 `/JSite396` 本体 119 的一条 0.01 mm 发丝线（覆盖 9 变虚）。
- **A1 落地（时间盒内，约 15 min）**：headless IDA（`idapro` 3.12，开 `style.dll.i64` 副本不存盘）。`sub_1002A080` 是覆盖的合成例程，逐位看 `this+96`：bit0 色 `+100`、bit1 宽 `+104`、bit7 虚线 `+188`（按 id 懒解析，0 = NULL）、bit8 填充色 `+152`、bit2/3/5 三个 f64、bit4 u32、bit6 两个 u16；`sub_10027C10` 是虚线 getter（bit7 清 → NULL）；`sub_10029390` 是引用变更钩子；颜色 `0x80000000` = 随层、无条件压过。三行结论升为 **native-reader 级**。
- **A2 落地**：`StyleRecord` +4 字段、`resolve_line_style` 覆盖分支逐位替换、`dash_named`、模块头与常量文档；单测 +8（`--lib style_link` 57 → 65）。`ResolvedLineStyle` 形状不变（A-D2 (a)）。
- **A3 落地**：棘轮 `EXPECTED_PALETTE` 重钉（12 项 → 12 项，`0.350mm #FE0060` 删、`0.180mm #000000` 5 新增）、`EXPECTED_CROSSINGS` 4 → 5、`EXPECTED_SPANS` 1 → 3、新增 `dwg0201s_table_rules_draw_thin_and_solid_through_their_overrides`（按 oid 钉 14 条格线 + 电信号线仍虚）；`parse_real_files::a_cached_body_…` 重钉（0201 虚 0→1、0202 12→3、工艺 45→0，调色盘全变 0.13；与库 `.sym` 的比对改为「缓存的每种 look 都是 `.sym` 自己 `StyleCluster` 定义得出的」——`Item Note & Label` 的缓存是 `PSMcluster0` 里的锚点十字，库读取器只走 `Sheet*`，拿不到那两笔）。golden / profile / census / semantic_join 不变。
- **A4 落地**：`docs/analysis/2026-10-10-an-override-repaints-width-colour-and-switches-the-dash-off.md`；08-07 虚线篇 §7 加第 4 条；CHANGELOG / `task_plan.md`。
- 新登记：0201 `/JSite396` 本体 119 那条 0.01 mm 虚线在 OCS 里会按放置线宽重涂成一条可见虚线（P-E1 的备选「逐笔更细时取逐笔」该重议）；`0x80000000` 随层色本 crate 以保基色近似。
- **O1–O4 落地**（OCS `main` `b795b0f7`，接在 `aaedd334` 之后；fable-5-1-17 做，fable-5-1-25 复核并提交）：`tests/pid_import.rs` 三处重钉——0201 调色盘 `35 #000000` 43→22、`35 #FE0060` 删、新增 `13 #000000` 21 / `18 #000000` 3；图纸虚线钉成「恰好一条、只有 1.905 / 1.27 mm」（电信号线）；缓存本体虚线 工艺 45 / 0202 12 / 0201 0 → 0 / 3 / 1（0202 的三笔涂放置的 `35 #800000`）。`pid_import` 52 / 52，`--lib io::pid` 54 / 54，OCS `src` 零改动。
  批量基线**六行哈希全变**（不是预计的 5 行），两层原因 README 分开写：`main` 的基线自 09-28 没再签过——上游合并后 codec 多写 `$XCLIPFRAME` / `$DWFFRAME` / `$DGNFRAME`，A01 `drawn` 122→124 是 09-30 椭圆弧那单——所以「改前」用 `1ed140a` 的 pid-parse 在同一棵树重跑；改前→改后 17 列不动、六张实体数不动（60 / 335 / 310 / 125 / 310 / 579），DXF 逐实体比只有 `370` / `62` / `420` / `6` 变，D06 实体一字不变、只少一条无人引用的 `PID-DASH-*`。复核时重跑 `pid_batch_report` 与 `report.csv` 逐哈希相同。
  GUI（Vulkan，REST capture，证据 `docs/evidence/2026-10-10-pid-table-rules-after-override-fix/`）：14 条格线细实线、坡度三角黑、电信号线仍虚。按改后 DXF 核：**0.13 mm 的 11 条 = 10 条横线 + 中间 x=510.3 那条竖线，0.18 mm 的 3 条 = x=472.2 / 525.8 / 573.8 三条竖线**——§1 / §5 写的「横 0.13、竖 0.18」按此修正，宽度本身与棘轮按 oid 钉的一致。
  未做：`pid-web-open`（`ocs-pid-0928`）的 `pid_import` 自 `931ddca` 起是红的，等 X2 提交后按同样数字重钉；10-08 单那边的一句还没登记（那棵树是另一会话的）。OCS 10-09 的 GUI evidence 目录仍未跟踪，是否入库由用户定。
