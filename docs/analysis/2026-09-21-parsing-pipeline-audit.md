# `.pid` 解析流程审核：从 `load_pid(path)` 到屏幕，谁调谁、哪里重复、哪里靠运气（2026-09-21）

> 承接 `2026-07-27-pid-load-status-snapshot.md`（上一次对「能解到哪」的快照）与 OCS
> `docs/plans/2026-09-21-the-layer-slot-takes-the-sheet-layer-by-default-and-the-switch-retires.md`（图层槽默认值那一单）。
> 本文不问「解出多少」，问**流程本身**：pid-parse `7498bd9` + OCS HEAD `96c01925` 上，一张 `.pid` 打开时代码走哪条路、
> 每一步吃什么产出什么、哪几处是结构性的弱点。**只读审核，未跑代码**（OCS 树当日在合并上游，cargo 不插队）。
> 开出的三张单：`docs/plans/2026-09-21-style-link-reads-the-parsed-document.md`（①）、
> `docs/plans/2026-09-21-a-geometry-parse-profile.md`（②）、OCS `docs/plans/2026-09-21-load-pid-returns-its-summary.md`（⑤）。
> **2026-09-22 结算**：① 落地（pid-parse `74bee65` / OCS `bbdc3d80`，一张图只开一次、样式路与几何路同一份解码结果）；② 落地（pid-parse `fae2ac9` / OCS `2e9e10f5`，
> `ParseProfile::Geometry`；G1 量出 25 条连通线的端点位置靠 geometry hints ← crossref ← 对象图，这三段 Geometry 下照跑）；③ ④ ⑤ ⑥ 归入 OCS ⑤ 单（`a6b91454` 开单）——**同日批准、落地**（OCS `edc6b495` Q1–Q3：`load_pid -> PidImport { document, summary }`，摘要以文档自定义属性随文档穿管线、开图完成时取走，`IMPORT_SUMMARIES` 删；`symbol_library` / `unit` 进摘要，米回退升 warn；`ec14ce55` Q4：`load_pid` 切四段只搬不改，四图 `--export` 三版字节一条线）；⑦ ⑧ 照旧登记不做。

## 一句话

流程的骨架是对的——容器优先、证据分层、丢失可见、样式按存储作用域——但**一张图要开六次文件、Sheet 记录解两遍**
（`style_link` 五个入口各自重开 CFB、重解记录），**Full profile 把只给探针和报告用的启发式 pass 全跑一遍**而 OCS 只消费其中一小半，
OCS 侧 `load_pid` 是一个 460 行的函数、结果经全局静态 `Mutex` 按路径回传。前两条是 pid-parse 的接口形状问题，后两条是 OCS 的结构问题；
都不影响正确性，但每一条都是「两条路径可能悄悄不一致」的口子。

## 一、流程实况（OCS `src/io/pid.rs::load_pid` 起）

| 步 | 代码 | 吃 | 产出 |
|---|---|---|---|
| 1 | `PidParser::new().parse_file(path)` → `cfb::reader::parse_pid_package_from_cfb`（`reader.rs:55–158`） | `.pid` | `PidDocument`：CFB 树、CLSID / 时间戳 / state bits、全部流的 inventory |
| 1a | 顺序语义 pass（`reader.rs:127–151`）：`summary` → `tagged_text`（Drawing / General XML）→ `jsite`（`JSite<N>`：JProperties + **嵌套 `PSMcluster0` 几何 = 符号定义缓存** + 该存储自己的 `StyleCluster` 解成 `stroke_styles`）→ `cluster`（`PSMcluster0` / `StyleCluster` / DA 元数据 + **每条 `Sheet*`**）→ `dynamic_attrs` → `psm_tables`（roots / cluster / segment / spacemap + **`sheet_layers` + `view_filter_sets`**）→ `doc_registry` → DocVersion2 → Sheet 端点二次扫描 → object inventory / graph → `crossref` → geometry hints → `layout` | 各流字节 | 同一个 `PidDocument` 逐步富化 |
| 1b | 每条 `Sheet*`（`streams/cluster.rs:108–143`）：cluster 头、DA 属性记录、`sheet_probe::probe_sheet_stream`（启发式文本 run / 坐标对）、`decode_all_families_into`（`model/sheet_families.rs` 15 族注册表）、`undecoded_type_code_census` + `refused_record_census`（与解码同一份 claimed ranges）、spatial analysis | Sheet 字节 | `SheetStream.geometry: SheetGeometry`（`decoded_*` 15 组 + `texts` / `coordinate_hints` / `undecoded_type_codes` / `refused_records`） |
| 2 | `build_normalized_geometry(&doc)`（`geometry.rs:843`） | `doc.sheet_streams` / `jsites` / `sheet_layers` / `drawing_meta` | `NormalizedPidGeometry`：页幅（`igSmartFrame` 按 bit 去重取一，否则模板名回退 A0–A4）、实体三档（Decoded / Inferred / ProbeOnly）、`symbol_definitions`、`source_layer`、dropped / refused 清单、warnings |
| 3 | OCS 备文档（`pid.rs:502–547`） | — | linetypes、`lineweight_display = true`、图层表（Sheet 模式读 `sheet_layers["/"]` + `displayed` 位） |
| 4 | `discover_symbol_library(path)`（`pid.rs:549`, `2937`） | env `PID_SYMBOL_LIBRARY`，否则从图纸目录**向上 5 层**找 `.sym` | `Option<SymbolLibrary>` |
| 5 | `style_link::line_styles_for_file` / `style_names_for_file` / `style_libraries_for_file` / `text_heights_for_file` / `fill_styles_for_file`（`pid.rs:562–632`；`style_link.rs:2014–2146`） | **各自 `File::open` + `CompoundFile::open` 重开文件**，`for_each_document`（`2154–2190`）重走每条 Sheet、`decode_iglines / igpoints / iglinestrings / igsymbols / igtextboxes / igboundaries` **重解**，按 `stylecluster_path_for_sheet` 取该 sheet 所属 `StyleCluster` | 五张 `(stream, oid) → 样式` 索引 |
| 6 | `register_dash_linetypes` / `register_text_styles`；`PidSemanticIndex::load_beside(path, &parsed)`（旁边 `_Data.xml`，GraphicOID 两跳）；注册 XDATA APPID | — | 文档表项 |
| 7 | 实体循环（`pid.rs:677–866`）：fill → symbology → style 名 → 学科层 → build（Decoded：`build_fill` / `build_entities`，**缓存体优先、库兜底**；Inferred：`build_inferred` 只画注解 tick 与在页连通线；ProbeOnly：不画）→ bounds → 字高 / 色 / 对齐 / 字体 → 拆行 → 图层分布 → 语义命中 → `PlacementMeasures` → 逐实体 apply → `role_of_layer` → `attach_pid_metadata`（XDATA）→ 图层槽（按 mode）→ hidden | `geometry.entities` | `CadDocument` 实体 |
| 8 | 收尾（`pid.rs:868–949`）：`decoded == 0` → `Err`；`report_import` 日志；页框；取景；`PidViewFilter` 存 + 应用；`ImportSummary` 塞进 **`static IMPORT_SUMMARIES: Mutex<BTreeMap<PathBuf, _>>`**（`pid.rs:476`），由 `app/update/file.rs:1823` 的打开完成回调 `take_import_summary(&path)` 取走 | — | `CadDocument` + 命令行摘要 |

调用方唯一一处：`src/io/mod.rs:1245` `pid::load_pid(path)`。

## 二、站得住的

- **证据分层严格。** Decode / Probe 分开（`current-architecture-principles.md` §5），OCS 只画 Decoded（+两类 Inferred），ProbeOnly 一律 `Vec::new()`（`pid.rs:722`）——启发式永远不会被当成内容。
- **丢失可见。** undecoded（无解码器）+ refused（解码器拒收）双普查用的是解码注册表**同一份** claimed ranges（`cluster.rs:245–263`），OCS 摘要 `missing` 直接取和（`pid.rs:898–908`），`tests/render_gap_census.rs` 钉数。
- **样式作用域按存储。** `/JSite329/Sheet6` 只对 `/JSite329/StyleCluster`（`stylecluster_path_for_sheet`），嵌套站点的 `stroke_styles` 同理挂在站点上（`model/mod.rs:588`）——style id 每个存储从 1 重编，这条规则是对的。
- **panic-free，`?` 只在 IO / CFB 层。** 管线里所有 `?` 都是 `read_to_end` / `open_stream`；内容解码器形状不符出 0 条。坏的元数据流不拖垮几何。
- **出处进 XDATA。** `role= / style= / sheet_layer= / driving= / extent=` 随实体走，图纸自己的 view filter 随 DWG / DXF 保存。
- **棘轮到位。** pid-parse `parse_real_files` 135 条 + `render_gap_census`；OCS `pid_import` 53 条（09-21 单落地后 49）。

## 三、要相的（按影响排）

### ① 一份文件开六次、Sheet 记录解两遍 → 单 ①

`style_link` 五个入口都吃 `&Path`。`parse_file` 已经把 `decoded_iglines / igpoints / iglinestrings / igsymbols / igtextboxes / igboundaries` 放进
`SheetGeometry`（每条都带 `oid` + `index` / `style_ref`，正是 `style_link` 要的 join 键），`style_link` 又各自 `decode_*` 一遍。

- 代价：每张图 1 + 5 = 6 次 CFB 打开，Sheet 六族解码 ×2。语料五张图不觉，几百张批处理会。
- **更要紧的是两条路径可能不一致**：注册表那条（`decode_all_families_into`）带拒收普查，`style_link` 直调 `decode_*` 那条没有。
  某族解码器一改拒收规则，几何实体与样式索引就可能对不上 `oid`——一边画了、一边没样式，或反过来。今天两条路调的是同一个函数所以一致，
  但这是**巧合成立**，没有测试钉着。
- 错误口径也不一样：`parse_file` 失败 → 整个导入 `Err`；`*_for_file` 失败 → `style_tables_failed = true` 继续画默认样式。前者对，后者也对，
  但两条路各开一次文件意味着「文件坏了」会在两处以两种方式冒出来。
- 根 `/StyleCluster` 的字节在 `parse_clusters` 里已经读到手（`cluster.rs:44–46`），只留了头和字符串表；嵌套 `StyleCluster` 在 `jsite` pass 里
  已经解成 `DocumentStyleTable`（`jsite.rs:117`, `212`）。**把它存进 `PidDocument`、让 `style_link` 加 `*_for_document(&PidDocument)`，
  `*_for_file` 变薄壳**，OCS 改吃一次解析结果。

### ② Full profile 全跑，OCS 只用一小半 → 单 ②

每次打开都跑：`sheet_probe`（启发式文本 / 坐标）、`populate_geometry_hints`、spatial analysis、`scan_strings`、object inventory / graph、
`crossref`、`layout`、DocVersion2、doc registry。OCS 实际消费（`geometry.rs` 读 `doc.sheet_streams / jsites / sheet_layers / drawing_meta`；
`semantics.rs` 读 `sheet_streams[].geometry.decoded_*`）：15 族解码、`sheet_layers`、`view_filter_sets`、jsites 嵌套几何 + `stroke_styles`、
`drawing_meta.Template`、端点记录（Inferred 连通线）、dropped / refused。

- `Light` 砍过头：不跑 `jsite` → 没符号体；不跑 `psm_tables` → 没图层；不跑 `dynamic_attrs` → 没端点。**缺一个 `Geometry` profile。**
- `probe_sheet_stream(&name, &path, &data, &Default::default())`（`cluster.rs:120–121`）没把 `ParseOptions` 传下去，探针开关调不了。
- 启发式 pass 的产物（`texts` / `coordinate_hints` / `object_geometry_hints`）在 `build_normalized_geometry` 里变成 ProbeOnly / 部分 Inferred 实体，
  OCS 循环里再逐个跳过——白做，还往 `warnings` 里塞「geometry decode remains partial」这类每图必有的话。

### ③ 导入结果依赖文件系统位置

符号体先取图纸缓存，缺了才去库；库靠 env 或向上 5 层搜。同一 `.pid` 挪个目录可能画得不一样。09-20 库优先开关退役后库只是兜底，风险已缩小；
摘要有 `cache_bodies / library_bodies` 能看出来，但**命中的库路径没进摘要**。小事，随 ⑤ 一起做：`ImportSummary` 加一格 `symbol_library: Option<PathBuf>`。
**2026-09-22 已落地**（OCS `edc6b495`）：`ImportSummary.symbol_library: Vec<PathBuf>` = `SymbolLibrary::roots()`，无库为空；`import_without_library` 测试断言临时目录下为空。

### ④ `load_pid_with_layer_mode` 460 行一口气

解析、图层表、样式、字体、语义、循环、统计、取景、过滤器、摘要 10+ 个关切在一个函数里，只能靠 `pid_import` 集成测试兜底。
09-21 单 H1 去掉 mode 分支会短一截，结构没变。建议随 ⑤ 拆「备文档 / 解样式 / 建实体 / 收尾」四段——**不另开单**，⑤ 的返回值改动会自然把收尾段切出来，其余顺手。
**2026-09-22 已落地**（OCS `ec14ce55`）：`prepare_document` 36 / `resolve_styles` 116 / `build_document_entities` 221 / `finish` 91 行，`load_pid` 剩 41 行编排；只搬不改，四图 `--export` 与切前字节相同。

### ⑤ `IMPORT_SUMMARIES` 全局静态 `Mutex` 按路径传摘要 → 单 ⑤（OCS）

`load_pid` 塞（`pid.rs:910`）、`file.rs:1823` 取。同一路径并发打开会串；回调没触发就泄漏一条；`take_` 语义要求调用方恰好取一次。
`io/mod.rs:1245` 是唯一调用方，直接让 `load_pid` 返回 `(CadDocument, ImportSummary)`（或把摘要挂在 `CadDocument` 的导入元数据上）更干净。
**2026-09-22 已落地**（OCS `edc6b495`）：两条都用上了——`load_pid -> Result<PidImport { document, summary }>`；`read_pid_path` 要交出外部类型 `ReadOutcome`，摘要就挂在文档的自定义属性上（`summary_info.custom_properties`，`PID_IMPORT_SUMMARY.<字段>`）穿过通用打开管线，`on_file_opened` / `io::load_file` 取走。先试的 XRecord 载体要花一个句柄、分配器不退，四图字节对不上，弃。`IMPORT_SUMMARIES` / `take_import_summary` / 测试的 `SUMMARY_MAILBOX` 全删。

### ⑥ 单位判定几乎无声

`mm_per_source_unit`（`pid.rs:1031`）取第一个 Decoded 实体的 `units`，只认 `m` / `mm`，其他一律回退到米并记一条 `info`。语料全是米所以没暴露；
英制工程会整体差 25.4 倍而只有一条 info 日志。至少升 `warn` 并进摘要（随 ⑤）。
**2026-09-22 已落地**（OCS `edc6b495`）：`ImportUnit::read` 取代 `mm_per_source_unit`，回退记 `warn`；`ImportSummary.unit: ImportUnit { Stated { unit, mm_per_unit }, AssumedMetre }`；回退时命令行多一行（21 语种）。判定逻辑未改。

### ⑦ 页幅回退只认 A0–A4

两个不同 `igSmartFrame` 页幅 → 放弃、回退模板名——保守正确。但 `infer_page_dimensions`（`geometry.rs:816`）只认 A0–A4，B 系 / ANSI → `None` →
无页框、取景退到 `SHEET_MARGIN` 窗口。语料没有，先登记不做。

### ⑧ 文字

25 / 184 字高回退到 2.5 mm（0.254 mm 哨兵样式在消费侧 `rad2d`）；富文本 run 合一。07-27 快照与 09-21 上午的完整度评估已说，不重复。

## 四、登记不做

| 项 | 理由 |
|---|---|
| 把 `sheet_probe` / geometry hints 整个删掉 | 它们是 `pid_inspect` 与逆向探针的眼睛，不是 OCS 的；② 只是让 OCS 不跑，不是让它们消失 |
| `style_link` 改吃 `PidPackage` raw streams | 那样 OCS 要从 `parse_file` 换成 `parse_package`（全部流字节留内存）；① 选存解好的 `DocumentStyleTable`，更轻 |
| B 系 / ANSI 页幅推断 | 语料没有；真来了先看 `igSmartFrame` 是否本来就有 |
| 把 `load_pid` 拆成独立模块 | 随 ⑤ 顺手拆段即可，不另立项 |

## 五、验证摘要

只读：pid-parse `lib.rs` / `api.rs` / `config.rs` / `cfb/reader.rs` 55–160 / `streams/cluster.rs` / `streams/jsite.rs`（`rg`）/ `style_link.rs` 2005–2215 /
`geometry.rs` 760–960 + 字段访问 `rg` / `semantics.rs` 字段访问 `rg` / `model/sheet.rs` 六族 DTO 的 `oid` + `index` 字段、
`docs/architecture-guide.md`、`docs/current-architecture-principles.md`；OCS `src/io/pid.rs` 489–950 + `build_inferred` / `mm_per_source_unit` /
`IMPORT_SUMMARIES`、`src/io/mod.rs:1245`、`src/app/update/file.rs:1823`。**未跑代码。**
