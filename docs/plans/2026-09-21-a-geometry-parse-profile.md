# `ParseProfile::Geometry`：给渲染消费者一条只跑它要的 pass 的路 · 小计划（2026-09-21 开单，待合并完再动）

> 承接 `docs/analysis/2026-09-21-parsing-pipeline-audit.md` ②，与 `docs/light-parse-design.md`（Light profile 的由来）。
> 现状：OCS 每打开一张 `.pid` 都以 `ParseOptions::default()`（Full）解——启发式文本 / 坐标探针、geometry hints、spatial analysis、
> 字符串扫描、object inventory / graph、crossref、layout、DocVersion2、doc registry 全跑；`build_normalized_geometry` 与 OCS 实际消费的只是其中一部分。
> `Light` 又砍过头（不跑 `jsite` → 没符号体；不跑 `psm_tables` → 没图层；不跑 `dynamic_attrs` → 没端点）。
> **开工前提：同 `2026-09-21-style-link-reads-the-parsed-document.md`。pid-parse 侧 G1–G3 可先做。**
> **2026-09-22 前提已满足**（OCS `f417782a` / `61153b6c`），S 单同日关闭（pid-parse `74bee65` / OCS `bbdc3d80`，`style_tables` 已在文档上）。
> **同日用户批准本单、G1 量完**：「不跑」列有一项在被画的连通线来路上，三项挪回「跑」（见 G-D2 与进度）；G2–G5 待做。

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
  G-D2 / G2 已按此改写。G2–G5 待做；G-D1–G-D7 按用户「批准 G 单」视为放行（G-D2 以本次改写后的为准）。

## 门禁记录

- 2026-09-21：开单（会话 fable-5-1-28），待批。
- 2026-09-22：用户「批准 G 单并先做 G1」（会话 fable-5-1-47）→ 七条决策放行；G1 结果改写 G-D2 的「跑 / 不跑」列（三项挪回）。
