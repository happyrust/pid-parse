# `style_link` 改吃解好的 `PidDocument`，一张图只开一次 · 小计划（2026-09-21 开单，待合并完再动）

> 承接 `docs/analysis/2026-09-21-parsing-pipeline-audit.md` ①。
> 现状：OCS `load_pid` 先 `PidParser::parse_file` 解一遍，再调 `style_link` 五个 `*_for_file(path)` 入口，每个入口**重开 CFB、重走每条 `Sheet*`、
> 重解六族记录**。一张图开 6 次、Sheet 记录解 2 遍，且样式路径与几何路径各调一次解码器，一致性靠巧合。
> **开工前提：OCS 上游合并（2026-09-21 15:55 起）提交落地、`stash@{0}` 的 09-21 H1/H2 处理完。** pid-parse 侧不受合并影响，可先做 S1–S3。
> **2026-09-22 前提已满足**：OCS 合并 `f417782a`、H1/H2 `61153b6c`（`pid_import` 49/49）。S1–S5 仍待批、未开工。

## 一句话

`PidDocument` 已经有 join 的两边——每条 `Sheet*` 的 `decoded_iglines / igpoints / iglinestrings / igsymbols / igtextboxes / igboundaries`
各带 `oid` + `index`（`igSymbol2d` 是 `style_ref`），嵌套站点的 `StyleCluster` 也已解成 `DocumentStyleTable` 挂在 `JSite.stroke_styles` 上——
只差把**根 `/StyleCluster` 解好的表也存进文档**，再给 `style_link` 一组 `*_for_document(&PidDocument)`；`*_for_file` 变成「解一次、转调」的薄壳，
行为不变。OCS 把五次 `*_for_file(path)` 换成五次 `*_for_document(&parsed)`。

## 事实（pid-parse `7498bd9`，OCS `96c01925`）

| 项 | 出处 |
|---|---|
| 五个入口：`fill_styles_for_file` / `style_names_for_file` / `style_libraries_for_file` / `text_heights_for_file` / `line_styles_for_file`，全部走 `for_each_document(path, visit)` | `style_link.rs:2014–2146`, `2154–2190` |
| `for_each_document`：`File::open` + `CompoundFile::open`，`walk()` 找叶名以 `Sheet` 开头的流，每条读字节、按 `stylecluster_path_for_sheet` 读所属 `StyleCluster`、`DocumentStyleTable::from_stylecluster_bytes`，**表为空的 sheet 跳过** | `style_link.rs:2154–2190`, `2209–2214` |
| `visit` 里的解码：`decode_iglines / decode_igpoints / decode_iglinestrings / decode_igsymbols`（line）、`decode_igtextboxes`（text）、`decode_igboundaries`（fill）；`style_names` / `style_libraries` 只读表不读 sheet | `style_link.rs:2016`, `2088`, `2115–2138` |
| 文档侧已有：`SheetGeometry.decoded_iglines: Vec<DecodedIgLine2dRecord>{oid, index, …}`；`decoded_igpoints` / `decoded_iglinestrings` / `decoded_igtextboxes` / `decoded_igboundaries` 同样 `oid + index`；`decoded_igsymbols{oid, style_ref}` | `model/sheet.rs:449`, `523`, `600`, `650`, `718`, `1098` |
| 根 `/StyleCluster` 字节在 `parse_clusters` 里读到手，只留 `header` + `string_table` + `probe_info` 进 `ClusterInfo` | `streams/cluster.rs:42–84` |
| 嵌套 `/JSite<N>/StyleCluster` 在 `jsite` pass 里读、`DocumentStyleTable::from_stylecluster_bytes`，只留 id → `PrimitiveStyle` 的 `stroke_styles` | `streams/jsite.rs:109–125`, `206–220`；`model/mod.rs:588–604` |
| `DocumentStyleTable`：`Debug + Clone + Default + PartialEq`，`records()` / `resolve_line_style` / `resolve_text_height` / `resolve_fill` / `name_of_style` / `style_library_source` / `is_empty` | `style_link.rs:1137` |
| OCS 调用：`pid.rs:497–500` `parse_file`；`562` line / `586` names / `598` libraries / `611` heights / `625` fills；三处失败置 `style_tables_failed`，names / libraries 失败只记日志 | OCS `src/io/pid.rs` |
| 几何实体走的是 `decode_all_families_into`（注册表，带拒收普查）；`style_link` 直调 `decode_*`。今天两者调同一函数所以一致，**无测试钉** | `model/sheet_families.rs:440`；`streams/cluster.rs:245–263` |

## 决策（按推荐落笔，等批）

| # | 决策 | 结论 | 状态 |
|---|---|---|---|
| S-D1 | 表存哪 | **`PidDocument.style_tables: BTreeMap<String, DocumentStyleTable>`**，键 = 存储路径（根 `"/"`、嵌套 `"/JSite329"`），与 `sheet_layers` / `view_filter_sets` 同一套键。备选「挂在 `SheetStream` 上」：同一存储多条 sheet 会存多份；备选「改吃 `PidPackage` raw streams」：OCS 要换 `parse_package`、全部流字节留内存，重 | ⭕ |
| S-D2 | 谁填 | 根表在 `parse_clusters` 读 `/StyleCluster` 那一步顺手 `from_stylecluster_bytes` 存入；嵌套表在 `jsite` pass 已经建了，**存整表**而不只 `stroke_styles`（`stroke_styles` 保留，照旧从整表投影，不改它的消费者） | ⭕ |
| S-D3 | 新入口形状 | 五个 `*_for_document(doc: &PidDocument) -> XxxIndex`（**不返回 `Result`**：文档已在手，没有 IO 可失败）；`*_for_file(path)` 改为 `parse_file` + 转调，签名不变。`for_each_document` 改成遍历 `doc.sheet_streams`，表取 `style_tables[storage_of(sheet.path)]`，空表跳过——口径与今天一字不差 | ⭕ |
| S-D4 | `*_for_document` 里的解码来源 | **读 `sheet.geometry.decoded_*`，不再调 `decode_*`**。这就把「样式路径与几何路径用同一份解码结果」从巧合变成结构。`geometry` 为 `None` 的 sheet（探针无所得）跳过 | ⭕ |
| S-D5 | `ParseOptions` | 表的解码开销小（每存储一条流），**不加开关**，Full / Light / 将来的 Geometry 都填 | ⭕ |
| S-D6 | OCS 错误口径 | 五次调用改成 `*_for_document(&parsed)`，不再有 `Err` 分支；`style_tables_failed` 改为「`parsed.style_tables.get("/")` 缺或为空」——含义从「文件打不开」变成「表读不出」，正是这个标志本来想说的。日志与摘要文案不变 | ⭕ |
| S-D7 | `serde` | `style_tables` 标 `#[serde(skip)]`：`pid_inspect --json` 的 schema 不变，`DocumentStyleTable` 也不用为此 `Serialize` | ⭕ |

## 工作项（批了再做）

- **S1 pid-parse `model`**：`PidDocument.style_tables`（S-D1 / S-D7）。
- **S2 pid-parse `streams`**：`cluster.rs` 填根表；`jsite.rs` 存嵌套整表（S-D2）。一提交。
- **S3 pid-parse `style_link`**：五个 `*_for_document`；`*_for_file` 改薄壳；`for_each_document` 改文档遍历，`visit` 签名从 `(&str, &[u8], &DocumentStyleTable)` 改为 `(&str, &SheetGeometry, &DocumentStyleTable)`（S-D3 / S-D4）。
  单测：同一 fixture 上 `*_for_file` 与 `*_for_document(&parse_file(..))` 五张索引逐项相等（钉「两条路一致」）。一提交。
- **S4 OCS `src/io/pid.rs`**：五处改 `*_for_document(&parsed)`；`style_tables_failed` 按 S-D6；`Cargo.toml` 的 pid-parse 依赖跟到 S3 提交。**等合并落地后动。** 一提交。
- **S5 台账**：`docs/architecture-guide.md` 读取路径加一行「StyleCluster → `style_tables`」；本单头部写哈希。

## 验收

- pid-parse：`cargo test --lib --test parse_real_files` 数字不降；新单测「五张索引 file == document」绿；`clippy --all-targets -D warnings` 零告警（双工具链）。
- OCS：`pid_import` 全绿、条数不变（09-21 H 单落地后的数字）；四图 `--export` 的实体数 / 角色数 / 颜色 / 线宽与 09-21 H 单事实表一致。
- `rg "_for_file\(" OpenCADStudio/src` 零命中；一张图打开时 `File::open` 只发生一次（`strace` / Process Monitor 或在 `for_each_document` 加临时计数任选）。

## 登记不做

| 项 | 理由 |
|---|---|
| 删 `*_for_file` | `pid_inspect` 与探针 example 还在用；薄壳零成本 |
| `symbol_library.rs` 读 `.sym` 的 `StyleCluster` 也走 `style_tables` | `.sym` 不是 `PidDocument`，另一条路；今天已按同一 `DocumentStyleTable` 解，不重复 |
| 把 `style_link` 的 join 挪进 `build_normalized_geometry`（实体自带样式） | 那是把两个仓的分工重划；本单只消重复、钉一致 |
