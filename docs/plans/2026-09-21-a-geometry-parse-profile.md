# `ParseProfile::Geometry`：给渲染消费者一条只跑它要的 pass 的路 · 小计划（2026-09-21 开单，2026-09-22 落地关闭）

> 承接 `docs/analysis/2026-09-21-parsing-pipeline-audit.md` ②，与 `docs/light-parse-design.md`（Light profile 的由来）。
> 现状：OCS 每打开一张 `.pid` 都以 `ParseOptions::default()`（Full）解——启发式文本 / 坐标探针、geometry hints、spatial analysis、
> 字符串扫描、object inventory / graph、crossref、layout、DocVersion2、doc registry 全跑；`build_normalized_geometry` 与 OCS 实际消费的只是其中一部分。
> `Light` 又砍过头（不跑 `jsite` → 没符号体；不跑 `psm_tables` → 没图层；不跑 `dynamic_attrs` → 没端点）。
> **开工前提：同 `2026-09-21-style-link-reads-the-parsed-document.md`。pid-parse 侧 G1–G3 可先做。**
> **2026-09-22 前提已满足**（OCS `f417782a` / `61153b6c`），S 单同日关闭（pid-parse `74bee65` / OCS `bbdc3d80`，`style_tables` 已在文档上）。
> **同日用户批准本单、G1 量完**：「不跑」列有一项在被画的连通线来路上，三项挪回「跑」（见 G-D2 与进度）；**G2 + G3 + G5 + G4 同日落地**（pid-parse `fae2ac9`、OCS `2e9e10f5`，见进度），本单关闭。

## 一句话

加第三个 profile `Geometry`：**跑「画一张图」所需的每一步、不跑只给 `pid_inspect` / 探针 / 报告用的启发式与派生 pass**。
验收不是「快了多少」，而是**四图在 Geometry 与 Full 下 `NormalizedPidGeometry` 的 Decoded / Inferred 实体逐条相同**、OCS `--export` 逐图对数一致——
少跑的必须是 OCS 本来就丢掉的。

## 事实（pid-parse `7498bd9`，OCS `96c01925`）

| 项 | 出处 |
|---|---|
| `ParseProfile::{Full, Light}`；`ParseOptions{profile, scan_strings, parse_xml, parse_jsite_properties, keep_unknown_streams, max_preview_strings}`；`light()` 关 XML 与 JSite 属性 | `config.rs` |
| 管线门：`light_profile` 一个布尔在 `reader.rs` 上分四处——`tagged_text` / `jsite` 各带自己的开关再 `&& !light`；`dynamic_attrs` → `layout` 一整段 `if !light_profile` | `cfb/reader.rs:60`, `129–151` |
| 每条 `Sheet*` 固定做：cluster 头、DA 属性记录、`probe_sheet_stream(.., &Default::default())`（**没传 `ParseOptions`**）、`decode_all_families_into`、双普查、`collect_normalized_f64_pairs` + spatial analysis；`scan_strings` 只管 cluster 的 `extracted_strings` | `streams/cluster.rs:108–143`, `220–292` |
| `build_normalized_geometry` 读 `doc.sheet_streams`（`geometry.decoded_*` 15 组、`texts`、`coordinate_hints`、`object_geometry_hints`、`endpoints`、`undecoded_type_codes`、`refused_records`；`sheet.extracted_texts` / `primitive`）、`doc.jsites`（`nested_geometry`、`stroke_styles`、`symbol_information`、`local_symbol_path`、`name`、`path`）、`doc.sheet_layers`、`doc.drawing_meta`（`Template`） | `geometry.rs`（`rg "doc\.\w+|site\.\w+|geometry\.\w+"`） |
| `PidSemanticIndex`（OCS `load_beside`）只读 `sheet_streams[].geometry.decoded_*` | `semantics.rs` |
| OCS 还读：`parsed.sheet_layers["/"]`（图层表 + `displayed`）、`view_filter_sets`（经 `sheet_layers[].displayed`） | OCS `pid.rs:528–540` |
| OCS 对三档的处理：Decoded 画；Inferred 只画 `Annotation` tick 与在页 `Line`（连通线，0201 有 25 条，来自端点记录 → 需要 `dynamic_attrs` 的 trailer 才能二次扫描出 `endpoints`）；ProbeOnly 一律不画 | OCS `pid.rs:705–723`, `2346–2374`；`reader.rs:169–235` |
| `texts` / `coordinate_hints` / `object_geometry_hints` 的去向：`build_normalized_geometry` 里成 ProbeOnly 或部分 Inferred 实体；**OCS 是否画过其中任何一条，本单 G1 先量**（`Inferred` 且 kind 为 `Line` 的实体来源要分清是端点还是坐标对） | `geometry.rs:1017–1131`, `1175`, `1257–1281` |
| **G1 实测（2026-09-22，`pid_inspect --geometry-json`，Full，四图）**——按 `confidence` / `kind` / `source.record_id` 前缀：Inferred **Point** `coordinate-hint` 四图各 **64**（滑窗上限，D06 / 0201 / 0202 / 工艺 = 64 / 64 / 69 / 64）、`geometry-hint` 0201 **53** / 0202 **7**；Inferred **Line** `endpoint-line`（`record_kind = endpoint_pair`）0201 **49** / 0202 **3** / D06 0 / 工艺 0；ProbeOnly `text-probe` 8 / 9 / 19 / 12、`endpoint-probe` 0201 10 / 0202 23、`jstyle-override` 3 / 20 / 30 / 47；`annotation` 四图 **0**。OCS `--export`（09-22 H 单对数）画的 Inferred 只有 `role=connectivity` = 0201 **25**（49 条里两端都落页内且 ≥ 0.1 mm 的），0202 / D06 / 工艺 0；Inferred Point、ProbeOnly 一条不画 | 本单 G1 |
| **`endpoint-line` 的来路**（G1 顺着代码走）：`dynamic_attrs`（DA trailer 的关系 `field_x` + 身份索引）→ `populate_sheet_endpoints`（`Sheet*` 端点记录）→ `build_object_graph`（对象 `field_x` 集，只吃 DA，不依赖 `build_object_inventory`）→ `crossref::build_graph` 的 `relationship_endpoint_links`（关系 ↔ 所在 sheet + 两端 `field_x`）→ `populate_geometry_hints`（**对有关系链接的 sheet 再跑一遍 `probe_sheet_stream` 取 `chunks`**，`field_x_windows` / 特征 / 身份评分 ≥ 70 → `object_geometry_hints`）→ `build_normalized_geometry` 的 `object_positions`（`geometry.rs:965–996`）→ `endpoint-line` | `cfb/reader.rs:138–150`, `236–310`; `crossref.rs:419`; `geometry.rs:965`, `1180–1260` |
| `ImportSummary.missing` = dropped + refused 之和；两者来自双普查，与 `sheet_probe` 无关 | OCS `pid.rs:898–908` |

## 决策（按推荐落笔，等批）

| # | 决策 | 结论 | 状态 |
|---|---|---|---|
| G-D1 | 加不加第三个 profile | **加 `ParseProfile::Geometry`** + `ParseOptions::geometry()`。备选「让 OCS 自己拼开关」：今天的开关粒度到不了 sheet 探针与派生 pass，拼不出来 | ⭕ |
| G-D2 | Geometry 跑什么 | **跑**：streams inventory、`tagged_text`（`drawing_meta.Template` 页幅回退要它；General 可关）、`jsite`（嵌套几何 + `stroke_styles` + `symbol_information`；**JProperties 保留**，`local_symbol_path` / `name` 从它来）、`cluster`（含每条 `Sheet*` 的 15 族解码 + 双普查）、`dynamic_attrs`（端点二次扫描要 trailer）、`psm_tables`（`sheet_layers` / `view_filter_sets`）、Sheet 端点二次扫描、`style_tables`（① 落地后）。**不跑**：`probe_sheet_stream` 的文本 / 坐标探针、spatial analysis、`scan_strings`、`populate_geometry_hints`、object inventory / graph、`crossref`、`layout`、DocVersion2、`doc_registry`、`summary`（`SummaryInformation` 只进报告）。G1 量完若发现某项被画了，挪回「跑」列并改本表。**G1 结果（2026-09-22）：有一项被画了**——0201 的 25 条 `PID-CONNECTIVITY`（Inferred `endpoint-line`）的端点位置来自 `populate_geometry_hints`，而它吃 `crossref::build_graph` 的 `relationship_endpoint_links`、后者吃 `build_object_graph`。**三项挪回「跑」列**：`build_object_graph`、`crossref::build_graph`、`populate_geometry_hints`（含它自己对有关系链接的 sheet 再跑的那遍 `probe_sheet_stream`——这遍不能省，`chunks` 是特征输入）。`build_object_inventory` 不在链上，仍不跑。**仍不跑**：`cluster` pass 里那遍 `probe_sheet_stream`（`texts` / `coordinate_hints` 只成 ProbeOnly / Inferred Point，OCS 一条不画）、spatial analysis、`scan_strings`、`layout`、DocVersion2、`doc_registry`、`summary`。备选「连通线也不要」：Geometry 下不画 25 条诊断链（层默认关），换来 `crossref` / 对象图两段也省——但那是改 OCS 画什么，不是本单的事 | ⭕ |
| G-D3 | 门怎么开 | `reader.rs` 那个 `light_profile` 布尔换成一组按 pass 命名的 `ParseOptions` 方法（`runs_probes()` / `runs_derived_passes()` / `runs_registry()` …），三个 profile 各自回答；`parse_clusters` 把 `options` 传进 `probe_sheet_stream`，探针关时 `texts` / `coordinate_hints` 为空、`spatial_analysis` 为 `None`。备选「再加一个布尔」：四处 `if` 变八处，下一个 profile 再翻倍 | ⭕ |
| G-D4 | `SheetGeometry` 为 `None` 的判定 | 今天「无 text、无 hint、无族记录 → `None`」；探针关了以后前两项恒空，判定退化为「无族记录 → `None`」，语义正好——一条没画东西的 sheet 本来就不该有几何 | ⭕ |
| G-D5 | `build_normalized_geometry` 的 warnings | 探针关了以后「geometry decode remains partial …」那句每图必有的话仍会出；**不动**（它是 Full 下的真话，Geometry 下 OCS 不显示 warnings）。要改另开 | ⭕ |
| G-D6 | OCS 用哪个 | `load_pid` 改 `PidParser::with_options(ParseOptions::geometry())`；`pid_probe` / `pid_plot_dump` 两个探针 example **留 Full**（它们要看探针） | ⭕ |
| G-D7 | 测试口径 | pid-parse 新测：五张 fixture 各解 Full 与 Geometry，`build_normalized_geometry` 的 `entities` 按 `(confidence, kind, oid)` 过滤掉 ProbeOnly 后逐条相等、`symbol_definitions` / `page_dimensions_mm` / `dropped` / `refused` 相等；OCS `pid_import` 条数不变 | ⭕ |

## 工作项（批了再做）

- **G1 先量（只读，不改代码）**：四图 Full 下 `NormalizedPidGeometry` 的 Inferred 实体按来源分类（端点 / 坐标对 / hint），对上 OCS `--export` 的 `role=connectivity` / `annotation` 计数；确认 G-D2 的「不跑」列没有一项被画过。结果写回本单事实表。
  **✅ 2026-09-22 已量**（事实表末两行、G-D2 结果）：「不跑」列里 **`populate_geometry_hints` / `crossref` / 对象图三项在 0201 那 25 条连通线的来路上**，挪回「跑」；其余（`cluster` 里那遍 sheet 探针、spatial、`scan_strings`、`layout`、DocVersion2、`doc_registry`、`summary`、`object_inventory`）四图上没有一条被画的实体依赖它们。
- **G2 pid-parse `config` + `reader` + `cluster`**：`Geometry` profile、按 pass 命名的门、`options` 传进 `probe_sheet_stream`（G-D1 / G-D3 / G-D4）；门要按 G1 改后的 G-D2 开——`dynamic_attrs` → 端点扫描 → `build_object_graph` → `crossref` → `populate_geometry_hints` 这一段在 Geometry 下**照跑**，`build_object_inventory` / `layout` / DocVersion2 / `doc_registry` 不跑。一提交。
- **G3 pid-parse 测试**：G-D7 那条；`light_profile_…` 现有单测不动。同一提交。
- **G4 OCS `src/io/pid.rs`**：`with_options(ParseOptions::geometry())`（G-D6）；`Cargo.toml` 跟到 G2。**等合并落地后动。** 一提交。
- **G5 台账**：`docs/light-parse-design.md` 加「Geometry」一节；`architecture-guide.md` 读取路径图标出三个 profile 各跑到哪；本单头部写哈希。

## 验收

- pid-parse：G-D7 新测绿；`parse_real_files` 数字不降；clippy 双工具链零告警。
- OCS：`pid_import` 全绿、条数不变；四图 `--export` 实体数 / 角色数逐图与 09-21 H 单事实表一致。
- 顺带量一下（不作验收门）：四图 `parse_file` 墙钟 Full vs Geometry，写进本单进度。

## 登记不做

| 项 | 理由 |
|---|---|
| 删 `Light` | `pid_inspect --light` 与批量盘点在用 |
| 把 `sheet_probe` / geometry hints 从 crate 里拿掉 | 它们是逆向的眼睛；本单只让 OCS 不跑 |
| Geometry 下把 `warnings` 也裁掉 | G-D5；另开 |
| 并行解 sheet | 与 profile 无关；先量出 Geometry 的墙钟再说 |
| Geometry 下不画连通线、连带省掉 `crossref` / 对象图 | 那是改 OCS 画什么（`PID-CONNECTIVITY` 是 08-29 起就有的诊断层），不是解析 profile 的事；要的话另开 |

## 进度

- **2026-09-22（会话 fable-5-1-47）G1 ✅**：用户「批准 G 单并先做 G1」。只读、未改代码。`pid_inspect --geometry-json` 四图，按 `confidence` / `kind` / `record_id` 前缀分类
  （数字在事实表末两行）；OCS 侧对数用 09-22 H 单 `--export` 的角色计数（0201 connectivity 25，其余三图 0，`annotation` 四图 0）。
  **结论：G-D2 原「不跑」列有一项被画**——`populate_geometry_hints` 给 `endpoint-line` 供端点位置，它又靠 `crossref` 与对象图；三项挪回「跑」列，
  G-D2 / G2 已按此改写。G-D1–G-D7 按用户「批准 G 单」视为放行（G-D2 以本次改写后的为准）。
- **2026-09-22（同会话）✅ G2 + G3 + G5（pid-parse 侧）落地**，用户点选「开工 G2 + G3」。
  - **G2** `config.rs`：`ParseProfile::Geometry`、`ParseOptions::geometry()`（`scan_strings` / `keep_unknown_streams` 关，XML / JSite 属性开），
    七个按 pass 命名的谓词 `runs_summary / runs_tagged_text / runs_jsites / runs_sheet_probes / runs_semantic_passes / runs_registry / runs_derived_passes`（G-D1 / G-D3）。
    `cfb/reader.rs`：`light_profile` 布尔删，管线每个 pass 一个 `if options.runs_*()`；`summary` 也进了门（Geometry 不跑）；`doc_registry` / DocVersion2 归 `runs_registry`，
    `build_object_inventory` / `layout` 归 `runs_derived_passes`，对象图 / crossref / geometry hints 随 `runs_semantic_passes`（按 G1）。
    `streams/cluster.rs`：`parse_clusters` 按 `runs_sheet_probes` 决定要不要 `probe_sheet_stream`；`sheet_geometry_from_probe(Option<&SheetProbeReport>, …)`，
    探针关时 `texts` / `coordinate_hints` 空、`spatial_analysis` `None`，sheet 为 `None` 当且仅当无族记录（G-D4）。**没有传 `SheetProbeOptions` 进去**——探针整个关掉了，
    没剩下要调的旋钮；`populate_geometry_hints` 里那遍探针照旧 `Default`。
  - **G3** `tests/geometry_profile.rs`（新）：`the_geometry_profile_draws_what_full_draws_and_skips_only_probe_yield`——五图（四主图 + A01）Full 与 Geometry 各解一次，
    **画的实体**（`Decoded` + 非 `Point` 的 `Inferred`）逐条 `assert_eq!`、`symbol_definitions` / `page_dimensions_mm` / dropped / refused 相等、Full 多出的只能是
    ProbeOnly 或 `coordinate-hint` 的 Inferred 点、Geometry 没有 Full 没有的；逐 sheet：六族 + 两普查 + 端点 + `object_geometry_hints` 相等、`texts` / `coordinate_hints` 空、
    `spatial_analysis` `None`；文档级：`sheet_layers` / `view_filter_sets` / `style_tables` / `drawing_meta` 相等、每个 JSite 的嵌套本体 / 笔画样式 / 参数化链 / 符号路径相等、
    `summary` / `object_inventory` / `layout` / DocVersion2 / 注册表 / 未知流 为空。`the_pass_gates_answer_for_each_profile` 钉七谓词 × 三 profile。
    `cluster` 单测加 `without_the_probes_a_sheet_with_no_family_records_has_no_geometry`。**口径与 G-D7 的差别**：G-D7 写「过滤掉 ProbeOnly 后逐条相等」，
    但 `coordinate-hint` 的 Inferred 点本来就是探针产物、Geometry 下必然没有——改成「画的相等 + 少的只能是探针产物」两条，比原句严。
  - **G5** 台账：`docs/light-parse-design.md` 加「Geometry Profile」一节（三列矩阵）+ 实现注记改写；`architecture-guide.md` 读取路径图改成 13 步 × 三 profile 表；
    `CHANGELOG.md [Unreleased]` 一节；`task_plan.md` 当前阶段一段。
  - **验证**：`cargo check --lib --tests --examples` 干净；`--lib` **1116 / 1116**（1115 → 1116）；`--test geometry_profile` 2/2（5/5 图）；
    `--test parse_real_files` 135/135、`--test render_gap_census` 4/4、`--test style_link_ratchet` 15/15；`cargo clippy --all-targets -- -D warnings` 零告警（stable）；
    rustfmt 四个改动文件干净。
  - **墙钟**（debug，进程内先 Geometry 后 Full）：D06 25 / 24 ms、0202 159 / 172、工艺 69 / 86、A01 31 / 37；0201 3.35 s / 4.45 s——但先 Full 后 Geometry 时是
    3.5–3.8 / 4.6 s，**同一进程里第二次解 0201 不论哪个 profile 都慢约 1 s**（测量方法的事，不是 profile 的），0201 的 ~3.4 s 由两个 profile 都跑的某段主导，
    候选 `populate_geometry_hints` 的窗口评分或某条大 `Sheet*` 的族解码——**另量，不在本单**。顺带：登记不做那行写的 `pid_inspect --light`，`pid_inspect` 其实没有这个开关（`rg '"--light"'` 零命中）。
- **2026-09-22（同会话）✅ G4 落地，OCS `2e9e10f5`**（pid-parse 侧 `fae2ac9`）：`load_pid` 改 `PidParser::with_options(ParseOptions::geometry())`，
  `pid_probe` / `pid_plot_dump` 留 Full。**验证**：`--test pid_import` 49/49、`--lib io::pid` 52/52；四图 `--export` DXF 与 Full 版二进制（`bbdc3d80`）**字节相同**；
  墙钟在噪声内不变（0201 ~4.1–4.3 s、0202 ~0.53、D06 ~0.42、工艺 ~0.48–0.53 s，进程启动 + 两 profile 共有的那段主导）。**G1–G5 全部落地，本单关闭。**
  开着的只有登记项：0201 的 ~3.4 s 花在哪一段（另量）——**2026-09-23 已量，见下一条；登记项关闭。**
- **2026-09-23（会话 fable-5-1-23）✅ 登记项量完：0201 的 ~3.4 s 在 `populate_geometry_hints` 里的 `sheet_probe::field_x_window_identities`。**
  方法：`reader.rs` / `cluster.rs` / `sheet_probe.rs` 临时插 `Instant` 计时（量完 `git checkout` 撤回，无提交），每个 profile 单开一个进程解一次（避开 09-22 那个「同进程第二次慢 1 s」的干扰），debug 与 release 各量。
  - **debug，Geometry**：总 3.20–3.57 s；`populate_geometry_hints` 3.14–3.50 s（≈ 98 %），其中 **`field_x_window_identities` 2.88 s（≈ 90 %）**、
    `score_field_x_window_features_with_identities` 0.21–0.28 s（≈ 7 %）；同段里 `field_x_windows` 17 ms、`field_x_window_features` 17 ms、那遍 `probe_sheet_stream` 4 ms。
    管线其余全部相加 < 70 ms：`jsites` 15、`parse_clusters` 23（`decode_all_families_into` 10.7 + `claimed ranges` 10.6——族解码走了两遍）、`dynamic_attrs` 4.3、
    `populate_sheet_endpoints` 3.6、`collect_streams_and_bytes` 3.0、`psm_tables` 2.6、`crossref` 0.9、`build_object_graph` 0.6。Full 3.21 s，与 Geometry 同——热段在两个 profile 都跑的语义 pass 里，09-22 的推测（「`populate_geometry_hints` 的窗口评分」）成立，「某条大 `Sheet*` 的族解码」不成立。
  - **`field_x_window_identities` 内部**（`sheet_probe.rs:1185`，三个滑窗循环）：u32 `record_id` 循环 **2.82 s**——1,160,777 个字节位，每位对 `identity_index.by_field_x.values().find(|i| i.record_id == value)` 线性扫 94 条，≈ 1.1 亿次 BTreeMap 迭代；
    utf16-hex32 循环 63 ms（每位先 `String::with_capacity(32)` 再校验）；ascii-hex32 循环 10 ms。
  - **为什么是 0201**：Sheet6 只有 29,594 字节，但 57 个关系 `field_x` 都是小整数，`field_x_windows` 逐字节比对 u32 LE 命中 **6,025** 个窗口（每 5 个字节位一个），
    每窗 4 + 2 × 96 = 196 字节 → 窗口字节总计 **1,178,852 = 图纸的 39.8 倍**，三个循环各扫一遍。对照 0202：28 个 `field_x` → 156 窗口 → 1.3 倍 → `field_x_window_identities` 61 ms、整图 120 ms。
  - **release**：总 208 ms，`populate_geometry_hints` 191 ms（92 %）、`field_x_window_identities` 129 ms、`score_features` 52 ms——debug 放大 ≈ 17 倍，热段不变。
  - **候选改法（未做，另开）**：① `record_id → identity` 建一次 `HashMap`，第三个循环从 O(位 × 94) 降到 O(位)；② 三种身份先对整条 sheet 各扫一遍、再按 `[window_start, window_end)` 二分派给窗口，去掉 39.8 倍的重叠；
    ③ `score_..._with_identities` 按 `field_x` 建索引，取代每个 score 对 425 条 `identities` 的线性 `find`；④ `utf16_le_hex_32` 先校验再分配；⑤ `field_x_windows` 的 `field_xs.contains` 换 `binary_search`（已排序去重）。
    ① ③ 预计把 debug 的 3.2 s 压到 ~100 ms 量级；产物（0201 的 53 条 hint、25 条连通线）不该变，按 `tests/geometry_profile.rs` 与 OCS 四图 `--export` 字节对数验收。
- **2026-09-23（同会话）✅ 候选改法 ① ③ 落地**，用户点选「做候选改法 ①③」。`sheet_probe.rs` 一个文件、+28 / −17 行，无新公开面：
  - ① `field_x_window_identities`：进函数先把 `identity_index.by_field_x.values()` 按 `record_id` 收进一张 `HashMap`（`entry().or_insert`，首个为准——`by_field_x` 按 `field_x` 升序迭代，
    原 `find` 取的正是首个命中，答案逐位相同），u32 循环里 `find` 换 `get`。
  - ③ `score_field_x_window_features_with_identities`：先把 `resolves_to_same_object` 的 `identities` 按 `field_x` 收进 `HashMap`（首个为准，保持 `identities` 顺序语义），
    每个 score 一次 `get`；`identity_supports_score` 无人再用，删。
  - **验证**：rustfmt 干净、`cargo clippy --all-targets -- -D warnings` 零告警；`--lib` **1116/1116**、`--test geometry_profile` 2/2、`--test parse_real_files` **135/135**、
    `--test render_gap_census` 4/4、`--test style_link_ratchet` 15/15（这条从 ~10 s 到 1.5 s——八次整解里的 0201 不再拖）。
    **OCS 四图 `--export` DXF 与改前 SHA-256 逐一相等**（改前先用未改的 pid-parse 编 debug 版导出作基线：0201 188 308 B `2B1022B5…` / 0202 189 053 B `340ED098…` /
    D06 90 882 B `763CAD1A…` / 工艺 319 578 B `B04C7215…`，与 `ec14ce55` 记录的同值；改后重编再导，四个哈希、四个字节数不变）。0201 的 53 条 hint 不变。
  - **墙钟**（每个 profile 单开进程）：0201 debug Geometry **3.20–3.57 s → 0.51 s**、Full 3.21 → 0.54–0.57 s；release 208 → 110–133 ms。0202 65 / D06 22 / 工艺 58 ms（本来就不在热段上，不变）。
    OCS `--export` 0201 进程墙钟 3.78 → **0.92 s**，其余三图 0.4 s 不变。
  - **还剩什么**：0201 debug 剩的 ~0.5 s 里估 ~0.3 s 是那 116 万个字节位各做一次 `HashMap` 查找本身（debug 下的 SipHash），加 utf16 循环 63 ms——都是 39.8 倍窗口重叠喂出来的，
    要再压就做 ②（整条 sheet 各扫一遍再按窗口区间派发）；④ ⑤ 是小头。未做，等拍。
- **2026-09-23（同会话）✅ 候选改法 ② 落地**，用户点选「做 ②」。仍只动 `sheet_probe.rs`，无新公开面：
  - `field_x_window_identities` 改成**三种身份各对整条 sheet 扫一遍**（`data.windows(32 / 64 / 4)`，ASCII 32-hex / UTF-16LE 32-hex / u32 `record_id`，命中且能解析到 `field_x` 的
    收成按 offset 升序的 `IdentityHit` 表），再**每个窗口按 `[window_start, min(window_end, len) − 值长]` 用 `partition_point` 二分取命中**。
    每窗输出仍是 ASCII → UTF-16 → `record_id`、各按 offset 升序，`delta_from_field` / `resolves_to_same_object` 仍按该窗口算——与逐窗重扫逐条相同。
    新单测 `field_x_window_identities_report_a_hit_once_per_window_that_contains_it`：两个重叠窗口共用一条 `record_id`，各报一次、各带自己的 delta；只落在第一个窗口里的另一条只报一次。
  - **验证**：rustfmt 干净、`cargo clippy --all-targets -- -D warnings` 零告警；`--lib` **1117/1117**（+1）、`--test geometry_profile` 2/2、`--test parse_real_files` **135/135**、
    `--test render_gap_census` 4/4、`--test style_link_ratchet` 15/15；**OCS 四图 `--export` DXF 与改前基线 SHA-256、字节数逐一相等**（同上一条的基线）；0201 53 条 hint 不变。
  - **墙钟**：`field_x_window_identities` 在 0201 上 **2 880 ms（原）→ ① 后 ~300 ms（估）→ 8.9 ms**；0201 debug Geometry 整解 **0.51 → 0.31–0.32 s**、Full 0.33–0.35 s；
    release 110–133 → **77–84 ms**；OCS `--export` 0201 进程墙钟 0.92 → **0.66–0.75 s**（0202 0.47 s）。0202 / D06 / 工艺 65 / 22 / 58 ms 不变。
  - **现在的热段换人了**：剩 0.31 s 里 **`score_field_x_window_features`（基础评分，不含身份那步）197 ms**，其次 `field_x_windows` 17 / `field_x_window_features` 16 / 那遍 `probe_sheet_stream` 4 ms，
    管线其余 ~60 ms。6 025 个窗口每个都要评分——再往下要么让 `field_x_windows` 少出窝（例如先按 `endpoint_record_signature_start` 或 chunk 边界筛），要么给评分里的
    `repeated_delta_support` / 候选查找建索引；都改的是探针启发式的实现，产物应不变，仍按四图 `--export` 字节验收。未做，等拍。
- **2026-09-23（同会话）量：`score_field_x_window_features` 的 197 ms 在哪一步**，用户点选「先量不改」。临时计时（撤回，无提交），0201 Sheet6，debug，三次一致：
  - **`stable_marker_support` 125–151 ms（≈ 65–75 %）**；**per-feature `map` 60 ms（≈ 30 %）**；`stable_chunk_shape_support` 0.3 ms、`stable_f64_pair_shape_support` 0.2–0.6 ms。
    （上一条猜的 `repeated_delta_support` 不在这条函数里——它属于旧的 `score_field_x_windows`，本管线不调。）
  - 数字：features 6 025（135 条是端点记录引用，直接 −100）；**`stable_markers` 合计 209 286**（每窗 ≈ 35：`marker_candidates` 收窗口内每个 4 字节对齐、非零、≠ `field_x` 的 u32）。
    `stable_marker_support` 对这 209 K 条做 `BTreeMap<(delta, value), HashSet<u32>>` 的 `entry().or_default().insert()`，得 **90 279 个不同键**（绝大多数 support = 1），再 `into_iter` 重建成第二张 90 K 的 BTreeMap；
    per-feature 循环里每窗最多 35 次对这张 90 K BTreeMap 的 `get`（命中 support ≥ 3 即 break）。其余：chunk_shape 291 / f64_shape 55 / candidate_position 815；chunk_support 272 键、f64_support 2 键。
  - 根子还是 6 025 窗 × 196 字节 = 39.8 倍重叠：sheet 本身只有 ~7 400 个对齐 u32 位，却被当标记数了 209 K 次。
  - **候选（未做）**：(a) `stable_marker_support` 的两张 BTreeMap 换 `HashMap`（90 K 键，O(1)），估 130 → ~30 ms；(b) per-feature 的 marker `get` 跟着换，60 → ~30 ms——两条都不改产物；
    (c) 结构性标记（`is_structural_marker_value`）现在是在 `stable_marker_support` 里过滤、`marker_candidates` 仍收——若在收集时就丢，`SheetFieldXWindowFeatures.stable_markers` 是公开产物（探针 JSON），会变，不做；
    (d) 真正的杠杆仍是窗口数：`field_x_windows` 逐字节比对小整数命中 6 025 窗，若只认 4 字节对齐的命中，窗口数与 209 K 标记都会大降，但这改的是启发式的输入，53 条 hint 是否不变要先量。
- **2026-09-24（会话 opus-5-5-2，接手 fable-5-1-23）✅ 候选 (a)(b) 落地**，用户点选「做 (a)(b)」。只动 `sheet_probe.rs` 两行：`stable_marker_support` 的累计表与返回表 `BTreeMap` → `HashMap`
  （返回类型随之变；调用方 `score_field_x_window_features`、单测、`parse_real_files` 那条诊断只 `get` 或收集后全排序，不受影响），per-feature 循环的 marker `get` 跟着成 O(1)。
  - **验证**：rustfmt 干净、`cargo clippy --all-targets -- -D warnings` 零告警；`--lib` **1117/1117**、`--test geometry_profile` 2/2、`--test parse_real_files` **135/135**、
    `--test render_gap_census` 4/4、`--test style_link_ratchet` 15/15；**OCS 四图 `--export` DXF 与改前基线 SHA-256、字节数逐一相等**（先用改前的 OCS 二进制在本机重导一遍，四个哈希与基线同值，
    再编改后版导出比对）；临时计时 example 里 0201 的 6 025 条 score 的 Debug 摘要改前改后同值（debug 与 release 一致）。
  - **墙钟**（临时 example，已删；每个模式单开进程，改前改后交替跑）：`score_field_x_window_features` debug **192–205 → 168–181 ms**、release 45–46 → 38–40 ms；
    0201 Geometry 整解 debug **302–314 → 280–286 ms**、release 77–79 → 70.5–70.8 ms；OCS `--export` 0201 进程墙钟 0.76–0.83 → 0.74–0.83 s（差在噪声里）。
  - **估错的一半**：(a) 估 130 → ~30 ms，实测 `stable_marker_support` debug 132–138 → 134–142 ms、release ~34 ms，**不变**；见效的只有 (b)（per-feature 那段 debug ~60 → ~35 ms、release ~12 → ~5 ms）。
    成本不在 BTreeMap 查找，而在 209 K 次 `entry` + 内层 `HashSet` 插入与 90 K 个小 `HashSet` 的分配——debug 下光把 209 K 个键各 SipHash 一次就 23 ms。
    临时 example 里另试了三种同产物的写法：扁平 `HashSet<(delta, value, field_x)>` 去重计数 111–125 ms、三元组排序去重数段 96–133 ms、`HashMap<键, Vec<u32>>` 线性去重 101–114 ms——都到不了 ~30 ms，没换。
  - **还剩什么**：0201 debug 整解 ~0.28 s 里 `stable_marker_support` ~135 ms 仍是最大头，换数据结构压不下去，只剩 (d) 这条杠杆（让 `field_x_windows` 少出窝、209 K 标记跟着降）；
    改的是启发式的输入，53 条 hint / 四图字节会不会变要先量。未做，等拍。
- **2026-09-24（同会话）量：(d)「`field_x_windows` 只认 4 字节对齐的命中」**，用户点选「先量不改」。**结论：不可行**——0201 丢 35 / 53 条 hint，OCS 画的 25 条连通线全没。
  - 方法：临时 example（已删）按 `populate_geometry_hints` 原样跑一遍，窗口表整表与「只留 `offset % 4 == 0`」各跑一次比 hint（整表那次与文档里的 hint 逐条相等，复刻无误）；
    五图里只有 0201 / 0202 有关系端点 `field_x`，D06 / 工艺 / A01 这一步本来就不开窗。
  - **命中不按 4 字节对齐**：0201 的 6 025 个命中 `offset % 4` = 1 679 / 1 329 / 1 622 / 1 395，四种余数差不多平分；53 条 hint 的 `offset % 4` = 18 / 3 / 29 / 3（余 2 的最多）。
  - 0201：窗口 6 025 → 1 679、标记 209 286 → 59 587、marker 支持键 90 279 → 24 894；**hint 53 → 18**（丢 35、多 0；留下的 18 条里 17 条逐字节相等、1 条只差 `note` 里的支持数），
    `object_positions` 52 → 18；评分段（features + identities + scores + hints）debug 195 → 56 ms。0202：窗口 156 → 38，**hint 7 → 1**（丢的 6 条里有一条本身就在对齐位上——窗口还在，是跨窗口的支持 / 身份证据少了，没过门槛）。
  - 另量了只认 2 字节对齐：0201 窗口 3 301、hint 47 条（44 条逐字节相等），`object_positions` 丢 5 个；0202 hint 3 条——同样会丢。
  - **四图 `--export`**：临时补丁（`for offset in (0..=end).step_by(4)`）编 OCS 导出后立即 `git checkout` 撤回；同一时刻的 HEAD 再编一版导出对照。0202 / D06 / 工艺与对照**字节相同**，
    0201 188 539 → 182 360 B：按实体类型 × 图层数，唯一的差别是 **`LINE` @ `PID-CONNECTIVITY` 25 → 0**（实体 335 → 310），其余是随之的句柄重排与图层表少一项。
  - 对照没有用 09-23 基线：09-24 11:39 起另一会话在做文字按 run 取样式（pid-parse `886c431` + OCS `src/io/pid/*` 未提交改动），此刻工作树编出的四图都不再等于基线，与本条无关。
- **2026-09-24（同会话）✅ OCS 的 debug 版单独优化 pid-parse**，用户点选「给 OCS 的 debug 版单独优化 pid-parse」。OCS `c926b170`：`Cargo.toml` 加 `[profile.dev.package.pid-parse] opt-level = 2`
  （+5 行，只改编译配置；release 与 pid-parse 自己的工作区不受影响）。
  - **为什么是 2 不是 1**：pid-parse 单独解 0201（Geometry，debug，每次单开进程，隔离 worktree 钉在 `b1a4df4`）opt-level 0 / 1 / 2 = 279–300 / 162–181 / **74–86 ms**（release ~71 ms）；
    opt-level 1 几乎不内联 sheet probe 耗时所在的 hashbrown / SipHash 路径，只省 ~40 %。其余三图（0 → 2）：0202 64 → 18、D06 22 → 11、工艺 57 → 15 ms。
  - **OCS `--export` 进程墙钟**（五轮交替）：0201 627–689 → **417–467 ms**，0202 409–469 → 355–415，工艺 406–471 → 364–423，D06 基本不变。
  - **编译**（`cargo --timings`）：pid-parse 这个单元 15.4 s（0）→ 24.6 s（1）→ 28.9 s（2）；但 OpenCADStudio 从 pid-parse 的元数据起跑（0 在第 12.8 s、2 在第 13.9 s），自己要编 71–83 s，
    pid-parse 多出来的 codegen 在这段里并行跑完——改了 pid-parse 再编 OCS，关键路径只多 ~1 s；只改 OCS 时不变。
  - **验证**：pid-parse 按 0 / 1 / 2 与落地配置各编一版 OCS，四图 `--export` 全部等于当前基线（T2 之后：0201 `5F082D23…` / 0202 `6FABDF0F…` / D06 `907EB0A9…` / 工艺 `9D0A54BD…`）；
    0 与 2 两版背靠背从同一份源码编出；落地后的 `cargo build` 没有重编 pid-parse（沿用 2 那次的产物，指纹一致），即落地配置就是验过的配置。
  - 过程：这一轮没在共享工作树上打临时补丁——变体用 `cargo --config` 传，pid-parse 的单独计时用 `D:\Rust\tmp` 下的隔离 worktree（已移除）。上一条 (d) 的临时补丁曾被另一会话的
    `pid_import` 编进去、红过一次（OCS `818e5ae9` 有过程登记）。
- **2026-09-24（同会话）量：OCS debug 版再给 acadrust / 全部依赖开 opt-level 2 值不值**，用户点选「量一下」。只量不改。
  - 方法：OCS `1d754928` 的隔离 worktree（bin 改名，不动共享可执行文件）编三版——现状（只有 pid-parse 2）/ 加 `acadrust` 2 / 全部非成员依赖 `"*"` 2（单独 target 目录）；负载三版交替跑。
  - **结果**（现状 → acadrust → 全部）：5 MB DWG `--export` 成 DXF 进程 1.42–1.69 → 1.16–1.27 → 1.09–1.13 s，其中读 DWG 227–268 → 87–116 → 89–119 ms、写 DXF 534–708 → 446–472 → 382–405 ms；
    0201 `--export` 693–990 → 620–884 → 591–604 ms（嘈杂，同轮约 −100 / −130 ms）；`--plot-svg` 5 MB DWG 75–78 → 76–77 → 71 s（scene+pages 53–59 s、写 SVG 17 s 基本不动），
    763 KB P&ID DXF 18.5–21.7 → 16.7–22.6 → 16.1–17.8 s，256 KB DWG 三版都 ~19 s。四类产物（0201 DXF、27.6 MB 的 DWG→DXF、两份 SVG）三版哈希全相同。
  - **编译**：acadrust 这个单元 opt 0 41 s → opt 2 70–72 s；它还经 `ocs_plugin_api`（OCS 构建脚本的 build-dependency）给构建脚本再编一份，那份得整个编完构建脚本才能跑，
    所以 **acadrust 重编时关键路径 +~30 s**（只在 rev 变或 clean 时发生）。全部依赖 opt 2：干净目录从零 5 min（32 线程、566 个单元、4.1 GB）；放进共享 target 等于一次性重编 ~560 个依赖单元、
    多占几 GB，本地 path 依赖 iced / bevy 改了也按 opt 2 重编。
  - **结论**：acadrust 值得（DWG 读快 ~2.5 倍、DXF 写快 ~20 %，`.pid` 导出也少 ~0.1 s，代价只在 rev 变时付）；`"*"` 在 acadrust 之上只再多几个百分点，不值。
    大图在 debug 下慢的大头是 OCS 自己的场景构建与 SVG 写出（成员 crate、opt 0），调依赖的优化级别解决不了。未改，等拍。
- **2026-09-24（同会话）✅ acadrust 也按 opt-level 2 编，本条性能链收尾**，用户点选「把 acadrust 也加进 OCS 的 profile」并「到此收尾」。
  - OCS `2582a0eb`：`Cargo.toml` 再加 `[profile.dev.package.acadrust] opt-level = 2`（+5 行）。同一份源码（OCS `6b629e2e` 的隔离 worktree）编两版：四图 `.pid` 与三张 DWG 的
    `--export` 七份产物**字节相同**（`.pid` 四份等于当时的新基线 0201 `B9C0890C…` / 0202 `84743800…` / D06 `240B3785…` / 工艺 `E4B45DDA…`）；DWG 读 245 → 91 ms（5 MB HVAC）/ 36 → 16 ms / 42 → 14 ms，
    写 DXF 595 → 444 / 50 → 36 / 34 → 26 ms。落地的 TOML 写法与验过的 `--config` 指纹一致（先按 TOML 编 acadrust，再带 `--config` 复跑为 Fresh）。
  - **全链回顾（0201，debug）**：pid-parse 自身解析 3.2–3.6 s →（①③）0.51 s →（②）0.31 s →（(a)(b)）0.28 s；OCS 的 debug 版把 pid-parse 按 opt-level 2 编 → ~80 ms；release 208 → ~71 ms。
    OCS `--export` 0201 进程墙钟 3.78 s → ~0.42 s（其中 ~0.35 s 是进程启动与 GPU 探测）。产物全程字节不变。
  - 没做的：(d) 量过不可行（0201 丢 35 / 53 条 hint）；全部依赖 `"*"` 量过不值；大图在 debug 下的场景构建与 SVG 写出是 OCS 自己的代码，不在本链范围。
  - 提交：pid-parse `cd100b7` / `8109e1f` / `8968bd7`（代码）与各条文档；OCS `c926b170` / `2582a0eb`（编译配置）。本链关闭。

## 门禁记录

- 2026-09-21：开单（会话 fable-5-1-28），待批。
- 2026-09-22：用户「批准 G 单并先做 G1」（会话 fable-5-1-47）→ 七条决策放行；G1 结果改写 G-D2 的「跑 / 不跑」列（三项挪回）。
