# pid-parse Agent Guide

Layered parser for SmartPlant / Smart P&ID `.pid` files with a
publish XML pipeline producing `<DrawingName>_Data.xml` / `_Meta.xml`
from four inputs: a `.pid` alone, `.pid` + Oracle `exp` dump
(± `SPPIDDataMap.xml`), a whole backup-package directory
(`PlantData` zips + `Export.dmp`, either Oracle `exp` or SQL Server
MTF staged to a temp `.mdf`), or a single `Export.mdf` /
legacy `Export_v2.sqlite`.

## Agent skills

### Issue tracker

Issues and specs live as GitHub issues on `happyrust/pid-parse` (via the `gh` CLI). See `docs/agents/issue-tracker.md`.

### Triage labels

Five canonical roles, each label string equal to its name (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`). See `docs/agents/triage-labels.md`.

### Domain docs

Single-context — `CONTEXT.md` + `docs/adr/` at the repo root. See `docs/agents/domain.md`.

## Architecture

| Layer | Path | Role |
|---|---|---|
| Vendored MDF parser | `vendor/oxidized-mdf/` | Reads SQL Server MDF files, GPL-3.0 |
| Publish `.pid` line | `src/publish/{pid_load,pid_writer}.rs` | `.pid` → `PidPublishDrawing` → `_Data.xml` / `_Meta.xml` |
| Publish exp enrichment | `src/publish/{oracle_exp,pipeline}.rs` | `ExpTables` (one read) → ports / connectivity / compositions / notes / OPC; `T_PIPERUN` → synthesized `PIDPipeline` + `PipingConnectors` |
| Publish package mode | `src/publish/backup_package.rs` | Backup dir → one `ExpTables` + `SPPIDDataMap.xml` → every `.pid` → 三件 + `_Index.json`; MTF dumps stage to a temp `.mdf` |
| Publish DB line | `src/publish/{mdf_load,sqlite_load}.rs` | MDF → in-memory SQLite staging → `PublishDrawing` DTO |
| Publish shared | `src/publish/{model,xml_writer,catalog,diff}.rs` | DTO + `PublishStyle`; `_Data.xml`/`_Meta.xml` writers; `PublishItemCatalog` — one row per item type feeding both lines (sqlite_load chains, xml_writer dispatch + a01 rank, pid_load class rule, diff tag inventory = DB ∪ `.pid`, `_Coverage.json` derived-class rows from the tag roles); `--diff-against` parity |
| Reference-data archives | `src/refdata_archive.rs` | `.igr` templates and `.sym` bodies read straight out of a backup's `RefData~*.zip` (feature `refdata-archive` — the `zip` dependency alone, which is what `pid-plot` rides); entry names carry a UTF-8 BOM and are GBK-capable, so members are walked by index and matched on a normalized name |
| Inspect orchestration | `src/inspect/commands.rs` | `InspectCommand::run(&PidPackage) -> InspectOutput`; `src/bin/pid_inspect.rs` is arg-parse + print only |
| CLI entry | `src/bin/pid_publish_xml.rs` | `<file.pid|backup dir> --out-dir DIR [--oracle-exp|--datamap|--plant|--diff-against]` / `<mdf> --drawing UID` |

## Vendored `oxidized-mdf` (key constraints)

- **Sync API** — no async runtime needed; `MdfDatabase::open()` / `db.rows()` return sync `Result` / `Iterator`.
- **Panic-free** — all three source files (`pages.rs`, `lib.rs`, `sys.rs`) propagate errors via `Result` / `Error::ParseError`; zero `panic!` / `unwrap()` / `todo!()` in production code. GPL-3.0 §5(a) modification notices at top of each file.
- **Parsing engine** — `nom 8` for envelope-layer byte parsing; stdlib `from_le_bytes` for value-layer decoding.
- **Dependencies** — bitvec, chrono, encoding_rs, log, nom, rust_decimal, uuid. No async crates, no byteorder.
- **License** — GPL-3.0. Parent crate is MIT/Apache-2.0; combined binary is GPL-3.0 (see README License section).
- **License** — GPL-3.0 (vendored from [f3rn0s/oxidized-mdf](https://gitlab.com/f3rn0s/oxidized-mdf)). OK for internal use; public distribution of `pid-parse` requires license alignment.
- **Page reader** — forward-only; `mdf_load.rs` re-opens per table to stay deterministic.

## Test gates

| Test file | Scope | Fixture |
|---|---|---|
| `vendor/oxidized-mdf` unit tests | Parser internals | Inline byte vectors |
| `tests/publish_mdf_load.rs` | MDF → SQLite staging | `test-file/…/Export.mdf` |
| `tests/publish_sqlite_load.rs` | SQLite → DTO business fields | TEST02_p fixture |
| `tests/publish_xml_cli.rs` | End-to-end CLI | `test-file/…/Export.mdf` |
| `tests/publish_meta_parity.rs` | Meta XML shape + DWG compare | A01 ref + optional DWG fixture |
| `tests/publish_a01_raw_residual.rs` | Residual value scanning | `test-file/…/Export.mdf` |
| `tests/publish_from_pid.rs` | `.pid` / `--oracle-exp` / whole-package publish pins | `D:\work\cad\PID自研软件验证` + `D:\work\cad\pid-test-data` (soft-skips absent) |
| `tests/publish_{attribute,interface,rel,dwg_generated}_parity.rs` / `publish_writer_coverage.rs` / `publish_backlog_inventory.rs` / `publish_dwg_mirror.rs` | DB-line parity + writer coverage + backlog inventory | A01 ref + optional DWG fixture |
| `tests/publish_diff_accepted_baseline.rs` | Reads `docs/baselines/publish-diff-accepted.json` and classifies every line a `--diff-against` run prints as registered / disappeared / unregistered: A01, I井场, and six `验收数据` drawings (J K / L M井场, 轻烃分离器I, 空冷器A/B/C) each in `.pid`-alone and `--oracle-exp` mode, plus each figure's `FluidSystem` line against the per-`DocUID` bound in `FLUID_SYSTEM_REFERENCE_EMPTY_ALLOWANCES` | `test-file/…/A01` + `D:\work\cad\PID自研软件验证` + `PID_ACCEPTANCE_CORPUS` (default `D:\work\cad\pid-test-data\验收数据`) + `PID_TEST_DATA_DIR` (default `D:\work\cad\pid-test-data`, for `Export.dmp` / `RefData~4~804.zip`) (soft-skips absent) |
| `tests/plant_backup_census.rs` | What the 53-sheet plant backup leaves off its pages (census: `examples/probe_which_records_reach_no_page.rs`): the parser's sheet census over all 53 drawings pinned by type code (undecoded: the five `0x0019` `KeyPoint Constraint`s of `D04`'s one `ComplexString`, which the native graphic predicate does not take for drawable; refused: none; 1549 records the native reader skips; 51 `igSmartFrame2d`; one `ComplexString` of six curves and five constraints, no Group; not one record of `0x0010` / `0x0030` on a sheet chain) — read on each stream's own chain, so no phantom code out of the stream header — and `EF井场`'s hand-drawn dot (oid 52844, a `0x73` circle member) projecting as a 26-point fill ring. A run that held the pins prints one `plant backup census: …` line with what it read; one that skipped prints `skipping:` instead | `backup-plant-data/**/*.pid` under `PID_TEST_DATA_DIR` or either `pid-test-data` (soft-skips when absent) |
| `tests/parse_real_files.rs::primitive_line_decoder_emits_decoded_lines_with_provenance` | Phase 14 GLine2d cross-fixture | `test-file/*.pid` |
| `tests/parse_real_files.rs::a_sheet_chain_holds_no_jstyle_override_and_the_old_hits_were_block_length_words`, `::a_sheet_chain_holds_no_0x0010_record_and_the_old_hits_were_sub_type_words`, `::the_old_attribute_fragments_were_the_inside_of_text_boxes_and_placements` | No sheet chain of four fixtures holds a record of type `0x0030` or `0x0010`, and the three families that used to be found by sliding a decoder over the stream decode nothing. The hits those scans returned are re-found at every offset and each is held to what it was: 98 "`JStyleOverride`" on the length word of the block a `DependencyObject` ends with, 582 "`0x0010` sub-records" on a graphic record's own sub-type word and style id (`+12` / `+14`, all but one), 84 "attribute fragments" inside an `igTextBox` (77, each the text the box's own decoder reads) or an `igSymbol2d` (7) | `test-file/*.pid` |
| `tests/parse_real_files.rs::iglines_decoder_emits_decoded_iglines_with_provenance` | Phase 14 Slice J igLine2d (PSM 0x0018) | `test-file/*.pid` |
| `tests/parse_real_files.rs::iglinestrings_decoder_emits_decoded_polylines_with_provenance` | Phase 14 Slice K igLineString2d (PSM 0x0084) | `test-file/*.pid` |
| `tests/parse_real_files.rs::igpoints_decoder_emits_decoded_points_with_provenance` | Phase 14 Slice L igPoint2d (PSM 0x005E) | `test-file/*.pid` |
| `tests/parse_real_files.rs::igtextboxes_decoder_emits_decoded_texts_with_provenance` | Phase 14 Slice M igTextBox (PSM 0x004D) | `test-file/*.pid` |
| `tests/parse_real_files.rs::igsymbols_decoder_emits_decoded_symbols_with_provenance` | Phase 14 Slice N igSymbol2d (PSM 0x00CE) | `test-file/*.pid` |
| `tests/parse_real_files.rs::igboundaries_decoder_emits_typed_audit_records_with_provenance` | Phase 34-D igBoundary2d (PSM 0x0013) exact-count ratchet | `test-file/*.pid` |
| `tests/parse_real_files.rs::dwg0201_emits_decoded_primitive_lines_without_inferred_regression` | Phase 14 Slice E AC8 guard | `DWG-0201GP06-01.pid` |
| `tests/sheet_family_wiring.rs` | Three holds on every committed drawing and template. Each family's wired `SheetGeometry` field has as many records as a fresh registry re-decode of the same bytes. Every decoded record starts where its stream's own record chain starts one, so no family decodes a byte pattern inside another record. A `ComplexString` or a Group draws through the records it lists: each member is a live record of the same chain that names it as its owner, a string's curves are `igLine2d` / `igArc2d` on its layer with its style id, a group's members are the two rectangles of a border or the 37 records of a logo, and every line member's range box is the line itself when read at the record's `range_scale` — the eleven templates inside the committed `RefData~*.zip` are read for this half too (12 groups, 5 of them at scale 2; the 16 lines of `A0-W.igr`'s logo lie half a metre from their boxes if the unit is taken for 1/32768 m) | `test-file/**/*.pid`, `*.igr` and the committed archives; runs in CI |
| `tests/sheet_chain_faults.rs` | A chain stream whose framing does not hold is read as far as it holds, and says so. `DWG-0201GP06-01.pid`'s `/Sheet6` is damaged six ways — a record length of zero in the middle, the last record one byte too long, four stray bytes at the end, cut off at a record boundary, the header's record count off by one, a wrong magic — and each raises exactly one `StreamChainFault` and one warning that names the stream, the offset the walk stops at and the bytes left unread; every record before the damage decodes, family by family, as on the intact file, and no `Decoded` entity comes from past the stop. The third test parses every drawing, template and symbol in reach, each distinct file once, and holds it to no fault and each of its chain streams to the magic, a walk that ends on the last byte and the header's live-record count: `test-file` with its archives always (floors pinned: 655 files, 1 338 sheet streams, 58 symbol caches, 32 514 records), both `pid-test-data` and the share where a machine has them | `test-file/**` (committed; runs in CI); the outside corpora where present — each prints what it read |
| `pid-plot/tests/cli_pid_plot.rs`：断链的 4 条（这个文件连同下面那一行的 10 条共 14 条） | 记录链立不住的流在 `pid_plot` 上：`DWG-0201GP06-01.pid` 的 `/Sheet6` 改坏一个长度字段的三份副本（中间一条记录长度为零、最后一条多一个字节、流尾多出四个字节）各导出一页，退出码不变，stderr 恰好一行 `warning:`，点名副本、`/Sheet6`、链停下的偏移与没读的字节数；页面与「同一张图、这条流干净地截到停点为止」的页逐图元相同，停点之前的图元与完好图纸逐个同一（位置相同当且仅当读到的页框相同：这张图唯一活着的页框是流的最后一条记录）；账本把这条流记成一项可疑的 `sheet_stream_without_geometry`，字节数是没读的那部分，守恒与交叉核对照过。第四份原样重写的副本一个字都不说，页面与账本同完好图纸 | `test-file/DWG-0201GP06-01.pid`（入库）；四条在没有字体目录时照样断言 stderr、页面模型与账本，只是退出码断言为 1（停在第一个缺的字型上，警告在要字体之前打印） |
| `tests/boundary_loops.rs` | A boundary of two loops fills the area between them. `XA-A1.igr`'s oids 5074 and 5076 — outer loops of 104 and 162 lines, counters of 77 and 54 — decode with their loops, anchor and member references, and each reaches the sheet as one fill ring whose area is the outer loop's less the counter's, with the record's anchor inside it and a point of the counter outside | `test-file/Ref/Template Files/XA-A1.igr` (committed; runs in CI) |
| `tests/elliptical_arcs.rs` | `0x007E igEllipticalArc2d` on both of its paths. In cached symbol bodies: A01's four head arcs decode and bulge outward from their vessels. On a sheet's own chain: `XA-A1.igr`'s two arcs close the counters of an outlined `B`, each starting and ending on the ends of the counter's edges and bulging away from it, and every stroke of that wordmark — the lines, the two arcs, the 23 `igBspCurve2d` of the letters' bowls — draws with the pen its style id names (0.25 mm) rather than the default | `test-file/export-test/publish-data/A01/A01.pid`, `test-file/Ref/Template Files/XA-A1.igr` (committed; soft-skips when absent) |
| `tests/sym_elliptical_arc_matches_cached_body.rs` | A `.sym` body's elliptical arc and the same arc in a drawing's cached body trace the identical rational B-spline (`Diaphragm Actuator.sym` against `I井场`'s `/JSite37311`). The backup is looked for under `PID_TEST_DATA_DIR` and the two `pid-test-data` copies, and the first root that holds **both** the drawing and `RefData~4~681.zip` is the one read: taking the first root that was merely a directory skipped this test on a machine whose first copy is a package with no `backup-plant-data` shelled out | the plant backup drawing and its `RefData~4~681.zip` (soft-skips when no root has both; feature `refdata-archive`) |
| `tests/render_gap_census.rs` | A sheet record that does not reach the drawing is named. Per fixture, the refused and the undecoded graphic records are pinned in records — none of either on any fixture — every refused record has a warning, the lines a retired rule refused are counted, and the records the native reader skips are counted apart. The two documents that last carried a refusal, `A井场` of the plant backup and the committed `XA-A1.igr`, are pinned to none, in the prose and in the structured list alike; with no document left to read a refusal's wording on, that is held on synthetic rows in `geometry.rs` (`a_refusal_reads_differently_from_a_missing_decoder`, `a_sheet_whose_only_gap_is_a_refusal_reports_no_missing_decoder`) | `test-file/*.pid`, `XA-A1.igr` (committed); the backup sheet where present |
| `tests/parser_panic_safety.rs` | No byte-level parser entry point panics. An adversarial corpus and its truncations go through every stream entry point. Every `decode_*_at(data, offset)` — the 27 of `AT_ENTRIES`, which `every_at_entry_point_is_listed` checks against the sources — is called at every offset of every input, one past its end and where `offset + 6` overflows. And because a family decodes only where a record chain starts a record, 290 planted chains (the magic, the record count, one record of each family's type code for each of 28 lengths, filled with noise, zeros or `0xFF`) carry the stream entry points past the chain into every payload decoder: 8 091 records of the right length and the wrong content | inline bytes; runs in CI |
| `pid-plot/tests/export_every_fixture.rs` | Every fixture `.pid` exports its whole page: laid out from the committed sources the ledger gate states, each of the seven wears the border template `FIXTURE_TEMPLATES` names (a drawing with no row fails rather than passing unframed), the writer's `mismatches` is empty, `cross_check` finds nothing (8.6 included, with real faces), and the PDF reads back with exactly the ink segments the model strokes and the characters it letters, the template's among them. None names a symbol no library carries — the 熊安 fixture's own `Xa …` symbols and its `XA-A1.igr` are committed under `test-file/Ref` | every `test-file/**/*.pid`; faces from `PID_PLOT_FONTS` or the share's `fonts` (the export soft-skips without a font directory; the walk and the template table still assert) |
| `pid-plot/tests/export_plant_backup.rs` | 53-sheet plant backup exports, page size / off-sheet / page-weight ratchets, exactly one primitive past its page (the sideways `N井场` run whose first glyph is clipped), known library gaps only, and no drawing's own ink wider than the widest pen (a validation mark such as `BandAid`'s 10 mm is not a pen). Which template each sheet wears is pinned exactly — `CPE-SQ-A0/A1/A2` on 8/22/7 sheets, `CPECCHB-SQ-A1/A2` on 4/9, the two `W` templates on **0** (so their 50 pinned label cells are held against no page until a sheet wears one), three bare sheets by name — with the 175 label cells and 64 template placements the run holds against a page. The readings a run used to print without asserting are pinned as well: the 15 sheets whose frame places the template off the page origin, by page key with the offset in millimetres (`FRAME_OFFSET_SHEETS` — all nine `CPECCHB-SQ-A2` sheets at (0.388407, 0.565648), `N井场`/`O井场`/`P井场`, `A井场` and the two `凝析油稳定塔` sheets); all 64 template placements naming a file no library carries (`TEMPLATE_NAME_GAPS`); which label is past its page and where it starts (`PAST_THE_PAGE`: `15-TD-4101416-B2-EH40` at (259.113810, −0.207788) mm); and each sheet's template coming from the kind of source its corpus calls for — disk exactly when a template directory of the corpus holds a file of that name, a `RefData~*.zip` otherwise, which is 0 disk / 50 archive in the package and 50 disk / 0 archive in the share, held sheet by sheet rather than as two totals. A failure names the measured value, the pin and the sheets that differ. The typefaces a run embeds are printed by name and deliberately not pinned (which faces a machine has is its own business; the ledger gate skips its two font marks when they are not the baseline's). **Also carries the export-ledger gate** for this corpus: each page's `ExportLedger` is assembled from the same `PaperModel` and write result, `cross_check` must find nothing, the face-free account must match the written one except for the two font marks, and every page is compared against `docs/baselines/export-ledger-backup.json` (suspect counts may only fall, intentional counts are pinned exactly). That comparison needs the layout to read a backup package, so a package in reach is the default: with `PID_PARSE_SELF_CHECK_DIR` unset the corpus is the directory above the `backup-plant-data` the gate found whenever that directory holds `RefData~*.zip`, and the share otherwise. A run that names a corpus reads it, and a named share skips the count comparison (requirement 13.7) and passes on the unattributed and cross-check rules alone. A package picked by default is held to more: if its symbol or template sources are not the baseline's the comparison is skipped all the same, and the test then **fails** for having compared nothing (the baseline's symbol sources include this machine's gitignored `test-file/symbols-full`, so a checkout without that directory has to name its corpus). The first line a run prints names the corpus and says why it was picked | `backup-plant-data/**/*.pid` under `PID_TEST_DATA_DIR`, the named corpus (the share when none is named), or either `pid-test-data` (soft-skips when absent); faces from `PID_PLOT_FONTS`, the corpus's `fonts`, `test-file/fonts`, then the share's `fonts` for a package picked by default (no face directory at all fails rather than skips) |
| `pid-plot/tests/export_ledger_fixtures.rs` | 7 张仓库夹具的出图记账门禁：每页 `assemble` 后五层（解析 / 排版 / 符号体 / 写出 / 段）未归因全为 0，`cross_check`（8.1–8.5 与 8.7；8.6 要字体，由 `export_every_fixture` 与 `export_plant_backup` 在写出的页上执行）无不符，同一页排两遍、两次序列化逐字节相同，序列化结果与语料描述头不含本机目录、任意盘符路径或 UNC 路径（原样与 JSON 转义两种写法都查），最后与 `docs/baselines/export-ledger-fixtures.json` 逐页比对（可疑项只许降、有意项精确钉）；语料描述与基线不同时**失败**，不跳过比对（`GateReport::counts_compared()`）。**没有字体需求，也没有任何软跳过分支**：7 张夹具、`test-file/symbols`、`test-file/Ref/Symbols`、`test-file/Ref/Template Files` 与 `test-file/backup-test/*_p/RefData~*.zip` 全部已入库，所以本机与 CI 都真跑。`UPDATE_EXPORT_LEDGER=1` 重写基线（可疑项会升高则拒绝写入） | 全部在仓库内（`test-file/**`），CI 上真跑 |
| `pid-plot/tests/cli_pid_plot.rs` | `pid_plot`（实际用来导出 PDF 的程序）作为进程起起来的 10 个用例：缺参数退出码 2 并打印 usage；图纸不存在退出码 1、不写 PDF；命令行上给的 corpus 不是目录时退出码 2、点名该路径、不写 PDF；没给 corpus 而回退目录不存在时只在 stderr 警告（警告要说出它「不是目录」）、照常导出；corpus 是目录、却够不到图纸点名的模板时（`D06.pid` 配一个空 corpus），退出码与写出的页不变，stderr 恰好一行 `warning:`，点名图纸、模板名（`PaperModel::missing_templates` 的每个词干，印成 `<词干>*.igr`）与 `border_template_places` 报的每个模板目录；没点名模板的图（`A01.pid`）配同样的空 corpus 一行警告都没有；`test-file/D06.pid` 以 `test-file` 为 corpus 导出后，stdout 首行（图元数、页幅、字节数）等于库内同一排版与盘上文件，PDF 读回有墨有字，账本汇总未归因为 0。子进程读的四个环境变量逐例显式设定 | `test-file/D06.pid` + `CPECCHB-SQ-A2.igr`、`test-file/export-test/publish-data/A01/A01.pid`（入库）；导出要字体，没有字体目录时 D06 导出那一条软跳过，其余 9 条到处都断言（两条模板用例没有字体时断言退出码 1 与 stderr，警告在要字体之前打印） |
| `pid-plot/src/write.rs::an_acceptance_sheet_letters_alone_as_it_does_among_the_eleven` | 借字的字型与同批图纸无关：验收语料的每张图单张备字（`pid_plot` 的做法）与全部图一起备字（验收门禁的做法），逐条标签比写出器发出的 `Tf` / `Tj`，每段文字的字型都相同；A2 图 `轻烃分离器I` 那两处点名 Arial Narrow 的「动火点」钉为整条借 SimSun。规则是 `fonts::borrow_order`：标签里有汉字先借 SimSun 再借 FangSong，没有汉字（`120℃` 的 `℃`）先借 FangSong 再借 SimSun，出借字型按整个字库判定，不看这次备字切出的子集。同一规则另有 7 条不依赖语料的库内用例（`fonts.rs` 4 条、`write.rs` 3 条），其中 5 条用 pdfcraft 自带的 Inter 造字型，在没有授权字体的机器上钉住单张与成批同字型、整条借字不被切开、run 的字型缺文件与 run 点名别的标签自有字型四种情形 | `PID_ACCEPTANCE_CORPUS`（默认 `验收数据`）加共享语料的 `fonts`（缺任一个则打印原因并软跳过）；7 条单测只用 pdfcraft 内嵌的 Inter，没有软跳过分支 |
| `pid-plot/tests/border_template.rs` | A0/A1/A2 sheets draw the frame, title block and sign-off column their linked `.igr` carries: both frames and 12 furniture labels on the `CPE-SQ` pages with the company name and `会 签 专 业` read back, and the A2 CCHB case on its own 35.354/7.071 mm insets, 116 strokes and 36 cells; a page with no same-size template draws none (pinned by `model.rs`'s extent unit test, no longer by an A2 negative) | the corpus `.pid` + `Ref/Template Files`; `test-file/D06.pid` + `CPECCHB-SQ-A2.igr` (soft-skips when absent) |
| `pid-plot/tests/style_witness.rs` | The plant backup draws only style vocabulary a reference plot has exercised: the eleven acceptance pages' 10 pen widths / 2 dash patterns / 4 faces / 3 templates are pinned, every unexercised entry is pinned with its drawing-vs-furniture split (3 of the 4 unexercised widths are `CPECCHB-SQ-A1` furniture, `0.5000 mm` is also `D04`'s own ink, the 2 unexercised dash patterns are the drawing's own ink), and the four widths the open band boundaries cannot settle are pinned — `0.3000 mm` inside `(0.25, 0.35)`, `0.5000 mm` inside `(0.35, 0.70)`, `0.7500 mm` and `0.7999 mm` inside `(0.70, 1.06)` | `PID_ACCEPTANCE_CORPUS` (default `验收数据`; no fallback to the five-sheet corpus) and `backup-plant-data/**/*.pid` under `PID_TEST_DATA_DIR`, the corpus, or either `pid-test-data` (soft-skips when absent) |
| `pid-plot/tests/refdata_archive_corpus.rs` | A backup package supplies its own reference data: the named `.igr` comes out of its `RefData~4~682.zip`, a symbol body resolves from `~681.zip` with no directory root at all, and the same drawings — copied to a directory with no `Ref` and no archive in reach — draw nothing | `test-file/backup-test/DWG-0202GP06-01_p` + the repo's `test-file/*.pid` |
| `pid-plot/tests/reference_plots_acceptance.rs` | 11 张带原厂参考图的图纸（A0×3 / A1×4 / A2×1 与原先 5 张同目录）：页幅、coverage 下限、extra 上限、参考标签数、pen 一致数与不符上限、缺符号名单；另钉 A0/A1/A2 三张纸的掩膜矩形（A2 左列 ≤ 36 mm、不吞工艺接续）。A1 三张的 8.210 mm 已归零（单记号改为不早于段中点：16.09 mm 竖管 5.08 → 8.05，参考 8.04 / 8.07）；箱体下沿那圈「波形管束」是 `.sym` 与缓存体都有的两条 `0x0063 igEllipse2d` 风机椭圆，解码后 `uncovered` 23.031 → 5.757 mm，见计划 §7.1；剩下的 `/Sheet6` 划线端头在信号线虚线按每段直线重排之后也归零（`docs/analysis/2026-11-08-the-signal-dash-is-fitted-to-every-piece.md`），现在除 A2 那张外的十页 coverage 1.000000、uncovered 0.000 mm、extra 0.000 mm。配上的标签管两层：两条字符类阈值（平均下移 ±0.04、平均宽差 ±0.02 字高）管每一个类——不论条数、`unknown` 也算——掩膜视图与整页视图都查，唯一超限的 `Left/Ascender part`（以 `℃` 结尾的标签被切开的两段，8 页）钉在读数 +0.0294 上、只许不变宽，哪一页回到阈值以内就得从名单里拿掉；锚点偏离参考超过 0.15 mm 或 run 比参考宽窄超过 0.05 mm 的标签按页钉条数（`WholePage::text_off`，11 页合计 64，与文字普查逐页相同），多一条少一条都失败并逐条列出。一对 run 属于哪个类由 `compare.rs` 的来源查找决定：从标签自己的锚点量、按整条 run 链拼出的文字找、被切开的一段认它那一行拼出的标签（类名带 ` part`），见 `docs/analysis/2026-11-10-every-lettering-class-is-held-and-every-label-counted.md`。11 页全部过钉之后，这道门把刚验过的字节统一写到 `<corpus>\plot\<name>.pdf`（11 张交付）并写 `BUILD.txt`（HEAD / plot_src / written_unix / pages）；任一页未过钉则一张都不写，`PID_ACCEPTANCE_DELIVER=0` 关闭落盘（第二个检出跑同一语料时用） | `D:\work\cad\pid-test-data\验收数据`（soft-skips when absent） |
| `pid-plot/tests/paper_model_golden.rs` | 7 张仓库夹具的整棵 `PaperModel` 逐字节钉死（页幅、模板与其家具、每个图元的纸面毫米）。**没有任何软跳过分支**：夹具、模板、项目符号与备份包里的 `RefData~4~681.zip` / `~682.zip` 全部已入库。符号来源是**声明**的（`test-file/Ref/Symbols` + 备份包 `~681.zip`），不走 `lay_out_drawing` 的自动搜索，因此不会被本机 gitignore 的 `symbols-full` 影响；序列化前把 `\` 归一为 `/`、仓库根替换成 `<repo>`，并把每个浮点量化到 1 µm（跨平台 libm 的末位差异，见下）。`PID_PLOT_BLESS=1` 重写金样（7 个文件共 3.97 MiB）。失败时打印第一个差异的 JSON 路径与两侧值，例如 `template.extent_mm[0]: expected 593.939394, actual 594.236364` | 全部在仓库内（`test-file/**`），CI 上真跑 |

DWG-specific tests soft-skip when `test-file/backup-test/DWG-0202GP06-01_p/extracted/Export.mdf` is absent.

### CI 覆盖：pid-plot 的测试文件哪些真跑、哪些软跳过

`pid-plot/tests/` 下 13 个既有测试文件，加上本 mission 新增的
`paper_model_golden.rs`、`export_ledger_fixtures.rs` 与 `cli_pid_plot.rs`，共 16 个。下表是**实测**定论，不是推断（2026-11-10 改过的四行例外，见表后的说明）：用
`git worktree add --detach … <HEAD>` 建一个模拟 CI 的干净检出（无
`test-file/symbols-full`、无 `test-file/fonts`、无 `PID_PLOT_FONTS`，且
`PID_PARSE_SELF_CHECK_DIR` / `PID_ACCEPTANCE_CORPUS` / `PID_TEST_DATA_DIR`
三个变量都指向不存在的目录），`rustup run stable cargo test -p pid-plot --
--nocapture`（stable 1.99.1，与 CI 的 `test (stable)` 同档）。判定依据是该
测试自己打出的 stderr 行，而不是「它没报错」。一个文件里的多个用例可能分属
两类，所以「混合」那一列写清哪几条真跑、哪几条跳过。

**复现这张表必须加 `-- --nocapture`。** CI 的 `cargo test --locked
--workspace --all-targets` 不带该参数，通过用例的 `eprintln!` 被测试框架吞掉，
CI 日志里 `skipping` 命中 0 次——按 CI 日志反推会得出「全部真跑」的错误结论。

| 测试文件 | 定论 | 判定依据（stderr 原文或断言实际执行） |
|---|---|---|
| `paper_model_golden.rs` | **真跑** | 无任何 `skipping` 行；7 行 `… matches its golden` 逐张打印，末行 `paper model golden: 7 fixtures, <字节数> bytes (<MiB>)`——量这张表时是 `4162420 bytes (3.97 MiB)`，金样每重写一次这个数就变，见下面「金样可行性」。干净检出里 7.28 s 通过 |
| `refdata_archive_corpus.rs` | **真跑** | 4 个用例全 `ok`（0.77 s）；夹具 `test-file/backup-test/DWG-0202GP06-01_p` 已入库，`backup_dir()` 返回 `Some`，`skipping the archive corpus` 一次都没出现 |
| `border_template.rs` | **混合（真跑 A2 与 A01，A0/A1 软跳过）** | 4 个用例全 `ok`，但只有 `an_a2_sheet_draws_its_linked_a2_template`（`D06.pid` + `CPECCHB-SQ-A2.igr`）与 `the_a01_sheets_embedded_frame_draws_the_a2_template_it_states`（`export-test/publish-data/A01/A01.pid` + `CPE-SQ-A2.igr`）的建模段真跑——两者的夹具都在仓库里。A0/A1 两条各打印一行 `skipping border template; C:\nonexistent-pid-corpus\I井场 注采阀组工艺及自控流程图.pid is absent` 与 `skipping border template; C:\nonexistent-pid-corpus\集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图.pid is absent`，因为 `corpus_dir()` 指向不存在的目录。两条 A2 用例的渲染段也打印 `skipping the page render; no font directory is present`，但建模段已过钉 |
| `paper_model.rs` | **软跳过** | `skipping paper models; C:\nonexistent-pid-corpus is absent`；该门读 `PID_PARSE_SELF_CHECK_DIR` 下的五张图，仓库里没有 |
| `plot_compare.rs` | **软跳过** | `skipping plot ratchet; …I井场 注采阀组工艺及自控流程图.pdf is absent` |
| `plot_text_sources.rs` | **软跳过** | `skipping sourced compare; …I井场 … .pdf is absent` |
| `read_reference_plots.rs` | **软跳过** | `skipping reference-plot counts; … is absent` |
| `reference_plots_acceptance.rs` | **混合（`eleven_reference_plots` 软跳过，`only_the_surveyed_sheets_carry_furniture_masks` 真跑）** | 两个用例都 `ok`，但只有后者执行了断言：它只读 `MASK_SHEETS` 常量表与 `template_masks` 的纯函数，不需要任何语料。前者打印 `skipping the acceptance gate; C:\nonexistent-pid-acceptance\I井场 注采阀组工艺及自控流程图.pdf is absent` 后早退，一条页面断言都没跑 |
| `style_witness.rs` | **软跳过** | `skipping the style-witness gate; no extracted plant backup here`；`backup_dir()` 返回 `None`，53 张备份不在检出里 |
| `export_every_fixture.rs` | **混合（走查与模板表真跑，导出段软跳过）** | 导出段打 `skipping fixture export; no font directory is present`：夹具在仓库里，缺的是字体。在它之前有两条断言不需要字体：走查找到不少于 7 张图，每张在 `FIXTURE_TEMPLATES` 里有一行 |
| `write_five_drawings.rs` | **混合** | 4 个用例 `ok`；3 条纯单测（`an_inserted_space_span_is_not_a_placed_run` / `a_glued_inch_mark_still_covers_both_placed_runs` / `a_missing_placed_run_is_reported`）真跑，落盘那一条打印 `skipping rendered plots; … .pid is absent` |
| `compare_five_drawings.rs` | **软跳过** | 只有一个用例 `five_drawings_write_compare_reports`，参考图不在时打 `skipping plot compare; …I井场 … .pdf is absent`。原来的 `survey_margin_text_once` 没有断言，已搬成示例 `probe_margin_text_survey`，`skip survey` 这一行不再存在 |
| `template_symbol_gap.rs` | **软跳过** | `skipping template symbol gap; …集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图.pid is absent` |
| `export_plant_backup.rs` | **软跳过（需临时打掉 `export_plant_backup.rs:208-209` 的两个硬编码绝对根才等价）** | `backup_dir()` 在 `PID_TEST_DATA_DIR` 之后还硬编码了 `D:\work\cad\pid-test-data` 与 `D:\work\plant-code\cad\pid-test-data` 两个绝对根。在 Linux runner 上这两个路径不存在，`backup_dir()` 返回 `None`，两条用例各打印一行 `skipping the D04 refusal pin; no backup-plant-data directory is present` 与 `skipping the backup export; no backup-plant-data directory is present` 并 `ok`。本机模拟时这两个根恰好存在（语料在这台机器上），于是两条用例都真跑：`d04_igsymbol2d_refused_count_is_zero` 通过（它只解析 `D04`，不读语料），`every_backup_drawing_exports_a_readable_page` **失败**：`0 sheets drew furniture; 50 did when this ratchet was set`（`export_plant_backup.rs:1785`）——因为 `PID_PARSE_SELF_CHECK_DIR` 指向不存在的目录，点了名的语料以它为准，模板与 `Ref` 都读不到（2026-11-10 在本机只设这一个变量重跑过，结果如此）。不设该变量时本机默认选中备份包，这道门真跑并逐页比 53 页的账本基线；默认选中包而一页都没比成（语料描述与基线不同）时它现在失败。把这两个绝对根从 `backup_dir()` 里去掉（不提交，仅用于模拟）后，行为才与 Linux runner 一致。`style_witness.rs:114-115` 有同样的两个绝对根，同样处理后软跳过 |
| `export_ledger_fixtures.rs` | **真跑** | 无任何 `skipping` 行；7 行 `<page>: 未发现可归属缺陷` 逐页打印，末行 `export-ledger: 7 页全部通过（可疑项 0 个，未归因 0）`（量这张表时是 12 个；2026-11-10 起夹具与备份两本账的可疑项都是 0）。全部输入都在仓库内（`test-file/**`），`write_ledger` 不读字型，所以干净检出与 CI 上同样真跑。`docs/baselines/export-ledger-fixtures.json` 已入库，因此不带 `UPDATE_EXPORT_LEDGER` 时直接比对。语料描述与基线不同时这道门现在失败，不再跳过比对后照样通过 |
| `cli_pid_plot.rs` | **混合（13 条真跑，D06 导出软跳过）** | 没有字体目录时 `d06_exports_the_page_the_library_lays_out` 打 `skipping the D06 export; no font directory is present (searched …)`；其余 13 条不需要字体。两条回退目录的用例、两条模板用例与四条断链用例在没有字体时断言退出码 1（过了 corpus 检查、停在第一个缺的字型上），各打一行 `<case>: no font directory is present (searched …), so the run can only get as far as the first missing face`；模板那条与三条断链的照样断言 stderr 上的 `warning:`，断链的四条另断言页面模型与账本（都不要字体） |

**结论**：**真跑 3 个 + 混合 5 个 + 软跳过 8 个 = 16 个**测试文件。

真跑的三个文件是 `paper_model_golden`（7 页整棵模型）、
`refdata_archive_corpus`（4 个用例）与 `export_ledger_fixtures`（7 页的
五层守恒 + 交叉核对 + 确定性与路径扫描 + 基线比对）。混合的五个文件各有一部分用例真跑：
`border_template` 真跑两条 A2 建模段（A0/A1 软跳过）、`write_five_drawings`
真跑三条纯单测（落盘那条跳过）、`reference_plots_acceptance` 真跑
`only_the_surveyed_sheets_carry_furniture_masks`（`eleven_reference_plots`
跳过）、`export_every_fixture` 真跑走查与模板表两条断言（导出段跳过）、
`cli_pid_plot` 真跑 13 条不需要字体的用例（D06 导出跳过）。其余 8 个文件在 CI 上
一条出图断言都不执行，因为它们读的语料（验收数据、备份语料、字体）都不在仓库里。

**2026-11-10 改过、没有在干净检出里重测的四行**：`cli_pid_plot`、
`export_every_fixture`、`compare_five_drawings` 与 `export_ledger_fixtures`
是照代码和本机「藏起字体目录」的模拟更新的，不是按上面的方法量的。下次动 CI
覆盖面时连同整张表重测。`cli_pid_plot` 那一行当天是 9 条，是同一天加了两条模板用例
之后在本机量的：`PID_PARSE_SELF_CHECK_DIR` 指向不存在的目录（本机没有
`test-file/fonts`，字体搜索因此落空），10 条全过，stderr 上四行
`no font directory is present` 加一行 `skipping the D06 export`。2026-11-11 加了
四条断链用例之后用同一个办法重量：14 条全过，八行 `<case>: no font directory is
present` 加一行 `skipping the D06 export`，表里的 13 条出自这一次。

**金样可行性（S8.1）**：`cargo run -q -p pid-plot --example
probe_paper_model_golden_size` 量了 7 张夹具的 `PaperModel` 序列化。每张各建两次
模型并各序列化一次，7 张全部逐字节相同（`unstable 0`），所以没有 `HashMap`
迭代顺序、没有时间戳、没有绝对路径漂移。逐张字节数（S8.1 当时的读数）：

| 夹具 | 字节 | 图元 |
|---|---:|---:|
| `d06` | 98 971 | 156 |
| `dwg-0201gp06-01` | 197 944 | 358 |
| `dwg-0202gp06-01` | 193 671 | 340 |
| `gongyi-guandao-1` | 2 907 733 | 5 478 |
| `jizhuzhan-flash-b` | 419 897 | 777 |
| `publish-a01` | 150 533 | 279 |
| `publish-dwg-0202gp06-01` | 193 671 | 340 |
| **合计** | **4 162 420（3.97 MiB）** | |

低于 5 MB 上限，所以按 D4(a) 上了金样门禁而不是退回 D4(b)。三个路径字段的处理
见 `paper_model_golden.rs` 的模块注释：`library` 用声明的相对标签，
`template.path` 与 `template.symbols[].key` 归一化到 `<repo>`；`.pid` 自己存下的
作者机绝对路径原样保留（那是源字节，跨平台一致）。

上面那张表是定下「上金样」时的读数，不随金样重写而改；现值不要从这里抄，
`paper_model_golden` 每次运行的末行会打印。此后它动过的记录：2026-11-11
（`parse-100` 线，`8640d51`）`XA-A1.igr` 上多画出两条椭圆弧和两个带孔的填充、
23 条 B 样条得了自己的笔，只有 `gongyi-guandao-1` 变了——图元 5 478 → 5 482，
2 907 733 → 2 945 007 字节——那条线上七份合计 4 199 694 字节（4.01 MiB），仍在
5 MB 上限之内。

**浮点必须量化到 1 µm，这一条是 CI 教出来的。** 第一次推送（`5c8c3da`）在
`test (stable)` 上红了：`publish-a01` 的 `primitives[167].shape.end_angle` 在
Windows 上是 `-0.4081037359805999`，在 Linux runner 上是 `-0.4081037359806` ——
差 1 ulp，画法完全相同。`sin_cos` / `hypot` / `atan2` 都来自平台的 libm，所以
只要模型里有角度或长度，原始 `f64` 就不可跨平台比。现在序列化前把每个浮点按
十进制拼写四舍五入到 6 位小数（`GOLDEN_DECIMALS`）；1 µm 比本语料需要分辨的
最小读数（0.0532 mm 的文字残差）细 50 倍，不损失任何可测差异。量化后总量从
4.25 MiB 降到 3.97 MiB。

**`export_plant_backup` 的两个硬编码根是一处真实隐患**：它们让「本机 CI 模拟」
与「Linux runner」在同一份代码上走出两条不同分支，而失败模式是 `panic!` 而不是
跳过。这条不影响本 mission 的验收（CI 上确实软跳过），但后续要改 CI 覆盖面时
得先处理它。

D06 fixture test
(`d06_pid_parses_with_expected_structure_and_geometry_summary`) covers
`test-file/D06.pid` — a compact PID with relationship identity in
`P&IDAttributes` rather than `class_id == 0xF6` trailers.  The test
ratchets stream count, PSM tables, DA records, object graph (10 objects
+ 10 unresolved relationships), Sheet6 geometry, and normalized totals.
Soft-skips when `test-file/D06.pid` is absent.

## SmartPlant Sheet PSM decoders — registry-driven families

`src/parsers/sheet_records.rs` ships typed PSM-record decoders for the
SmartPlant `Sheet*` stream record families; the authoritative list is
the `SHEET_RECORD_FAMILIES` registry in `src/model/sheet_families.rs`
(20 rows: the thirteen below plus `igBspCurve2d`, `igRectangle2d`,
`igDimension`, `igSmartFrame2d`, `DependencyObject`, and the two `0x0010`
rows `SubRecord0x0010` / `AttributeFragment`, which — like
`JStyleOverride` — decode nothing on any sheet; see the caveats).

| Slice | PSM Type | Decoder | DTO | Sigma Class |
|---|---|---|---|---|
| D-E | `0x3FE6` | `decode_primitive_lines` | `SheetPrimitiveLineDecoded` | `GLine2d` (SmartPlant ext.) |
| Phase 16 D | `0x0030` | `decode_jstyle_overrides` | `SheetJStyleOverrideDecoded` | `JStyleOverride` (RAD `style.dll`, **not** an arc; no sheet chain holds one — the class's records are in `StyleCluster`) |
| J | `0x0018` | `decode_iglines` | `SheetIgLine2dDecoded` | `igLine2d` (IGDS standard) |
| K | `0x0084` | `decode_iglinestrings` | `SheetIgLineString2dDecoded` | `igLineString2d` (IGDS standard) |
| L | `0x005E` | `decode_igpoints` | `SheetIgPoint2dDecoded` | `igPoint2d` (IGDS standard) |
| M | `0x004D` | `decode_igtextboxes` | `SheetIgTextBoxDecoded` | `igTextBox` (IGDS, UTF-16LE) |
| N | `0x00CE` | `decode_igsymbols` | `SheetIgSymbol2dDecoded` | `igSymbol2d` (IGDS, SmartPlant symbols) |
| — | `0x0061` | `decode_igarcs` | `DecodedIgArc2dRecord` | `igArc2d` (IGDS standard; emits `Arc`) |
| — | `0x0059` | `decode_igcircles` | `DecodedIgCircle2dRecord` | `igCircle2d` (IGDS standard; emits `Circle`) |
| 34-D | `0x0013` | `decode_igboundaries` | `SheetIgBoundary2dDecoded` | `igBoundary2d` (IGDS standard; a fill — one closed `Polyline` ring per record, to be filled, not stroked) |
| — | `0x007E` | `decode_igellipticalarcs` | `DecodedIgEllipticalArc2dRecord` | `igEllipticalArc2d` (IGDS standard; emits a sampled `Polyline`) |
| — | `0x0021` | `decode_igcomplexstrings` | `DecodedIgComplexString2dRecord` | `ComplexString` (an association, audit-only: its curves are its member records) |
| — | `0x007B` | `decode_iggroups` | `DecodedIgGroupRecord` | Group (an association, audit-only: its members draw themselves) |

Cross-fixture decoded geometry spans the GLine2d / igLine2d /
polyline / point / text / symbol families (Phase 16 reclassified
`0x0030` records as non-geometry `JStyleOverride`; Phase 17 removed
the legacy `PrimitiveArc` compatibility surface).
All decoders are panic-safe (validated via
`tests/parser_panic_safety.rs` adversarial matrix) and bounds-checked.

Decoded records flow through `streams/cluster.rs` →
`model::SheetGeometry::decoded_{primitive_lines, jstyle_overrides,
iglines, iglinestrings, igpoints, igtextboxes, igsymbols, igarcs,
igcircles, igboundaries, …}` → `geometry::build_normalized_geometry`
to emit `PidGraphicEntity { confidence: Decoded, kind: Line | Arc |
Circle | Polyline | Point | Text | SymbolInstance, source:
PidGraphicProvenance { stream_path, byte_range, record_kind,
graphic_oid, note } }`. The `note` carries the byte-level evidence
chain. (A decoded `igBoundary2d` is emitted as the fill ring of its
loops — see the `0x0013` caveat below.)

**Key insight (Slice J discovery)**: Intergraph Sigma uses its IGDS
class tags directly as PSM type codes for standard primitives.
SmartPlant extends with `GLine2d` (`0x3FE6`), but the bulk of
geometry is standard IGDS records using IGDS class tags as PSM type
codes. This unlocked Slices J–N without needing `radsrvitem.dll`
decompilation.

**Caveats**:
- `0x0030` is `JStyleOverride`, **not** `GArc2d`: the Phase 14 arc
  interpretation was a misidentification. Phase 16 IDA evidence
  (`radsrvitem.dll` type-code mapper → CLSID in `style.dll`) proved
  the JStyleOverride identity; Phase 17 removed the `PrimitiveArc`
  compatibility decoder. Never re-introduce an arc reading for
  `0x0030` (the real IGDS arc is `0x0061 igArc2d`). **No `Sheet*`
  chain holds a record of this type.** The class's records are in
  `StyleCluster` — 3 253 across the corpora, every payload the 90
  bytes of `style.dll`'s version-3 layout — and
  `style_link::DocumentStyleTable` reads them there, on that stream's
  own chain. What the Sheet family returned (13 671 hits across the
  corpora) was the length word of the block every `DependencyObject`
  ends with: a 48-byte block spells its length `30 00`, a scan that
  slid a byte at a time read that as a type word, and each "record"
  ran on past its host's end. Since the families decode on the record
  chain, `decode_jstyle_overrides` returns nothing on any sheet, and
  the `ProbeOnly` `PidGraphicKind::Unknown` entity each hit surfaced
  as is gone (the `Inferred` `Annotation` before it had been withdrawn
  in August, when `style.dll`'s serializer showed payload `+0..15` to
  be four separate four-byte reads). The family's registry row, DTO
  and emitter are still in place and decode nothing; retiring them is
  a later change. See
  `docs/analysis/2026-11-10-what-the-sixty-unreferenced-pages-leave-undrawn.md`
  §12 and
  `docs/analysis/2026-08-04-jstyleoverride-native-reader-settles-it.md`.
- `0x00FA` is `DependencyObject` (`imagdex.dex`), **not**
  `GraphicGroup`: Phase 15 named it on record shape alone, and the two
  OID references its payload carries are the two ends of a dependency,
  not an object and its graphics. The type-code table in
  `radsrvitem.dll` plus the RAD CLSID registry in `jutil.dll` settle
  the identity
  (`docs/analysis/2026-08-04-psm-type-code-registry.md`). It stays
  audit-only either way.
- Standard IGDS primitives are no longer sheet-absent: `igCircle2d`
  (`0x0059`) has since been promoted to a full Sheet family after W's
  `Sheet6` yielded 17 top-level records — the instrument balloons
  (r = 6.35 mm) and sensing-bulb centre dots (r = 1.4 mm) — and
  `igArc2d` (`0x0061`) is emitted because the linked `.igr` border
  templates draw logo glyphs and rounded corners with it. An arc's
  trailing byte (`payload +58`) is its direction — `0` clockwise,
  `1` counter-clockwise — the same byte `igEllipticalArc2d` carries;
  it is threaded through as `clockwise` and **never** folded to the
  minor sweep (the fold was the old guess and it mis-drew
  `FT-Ultrasonic`'s wave, `Local Vent`'s canopy and the `Remarks`
  cloud). See
  `docs/analysis/2026-10-23-the-arc-trailing-byte-is-the-direction.md`.
  `igRectangle2d`
  (`0x0020`) and `igBspCurve2d` (`0x005D`) also have Sheet family rows,
  and so does `igEllipticalArc2d` (`0x007E`) since 2026-11-10: a sheet's
  own chain can hold both curve families. `XA-A1.igr`'s `/Sheet6`
  carries 23 `igBspCurve2d` (the bowls of its wordmark) and the only
  two sheet-level `igEllipticalArc2d` of the corpora; both families
  emit a sampled `Polyline`, and both take their pen from the style id
  at `+14` through `style_link::line_styles_for_document`.
  In the test-file corpus the remaining members (12 circles, 12 arcs,
  3 rectangles, one BspCurve) still live in the `PSMcluster0` streams
  of nested `JSite` storages, each sitting on a sheet layer; those are
  symbol-body geometry and stay held back from page emission until a
  transform is proven. `igEllipticalArc2d` (0x007E) is no longer
  absent: beyond A01's two cached vessel heads, the library writes it
  into `.sym` bodies too — `2to1Parametric V Drum.sym`, `Diaphragm
  Actuator.sym` and `PCV-Self-Actuated Regulator Valve.sym` each carry
  a dome/head arc, read by `symbol_library::decode_primitive`'s
  `TYPE_ELLIPTICALARC` arm through the same geometry function the
  drawing-side decoder uses, so the two paths can never disagree on
  the curve (`tests/sym_elliptical_arc_matches_cached_body.rs`).
  See `docs/analysis/2026-08-27-aux-hi-is-the-sheet-layer.md` §7.
- Payload `+8` (`aux_hi`, the high half of the PSM envelope's `aux`)
  is **the oid of the sheet layer the object sits on** — never a
  framing constant. Grouping a storage's objects by it reproduces
  each `JSheetLayer`'s own `+12` object tally exactly (290/290
  layers, 1240/1240 objects cross-fixture). The vendor names the
  same edge `HGeomGetLayer` / `HGeomPutLayer` (`imagdex.dex`), a
  Put/Get pair at vtable `+52` / `+56` of interface
  `204D4DD1-B174-11CE-B914-08003601C6EB`; at runtime the graphic
  holds a COM reference to the layer object, and the record field is
  that reference serialized. Any rule of the form `aux_hi == <n>` is
  a filter that admits one layer and refuses the rest; that is what
  silently dropped 88 `igLine2d` including all of A01's page border
  (`aux_hi == 8` is the `Default` layer). The `igBoundary2d` copy of
  that rule is gone as of 2026-08-27; the field is now
  `sheet_layer_ref` and is never validated.
- `0x0013 igBoundary2d` (Phase 34-D) is a fully-typed **fill**
  record: `segment_count` groups of `0x67 tag + 4×f64` re-list the
  geometry of the `igLine2d` records named by its trailer member
  references (60/60 forward matches cross-fixture), followed by an
  anchor point. `decode_igboundaries` exposes every byte
  (`SheetGeometry::decoded_igboundaries`, `closed_loop` flag,
  byte-audit `Decoded`). The `u32` at payload `+18` is the **number
  of loops** (`loop_segment_counts`, `loops()`): one on 2 156 of the
  2 158 boundaries the corpora keep on sheet chains, two on
  `XA-A1.igr`'s oids 5074 and 5076, whose second loop is the counter
  of a logo letter. The anchor is the point the fill was seeded at:
  inside the loop on every one-loop record, between the two loops on
  those two. The family emits — it was audit-only until the fills
  turned out to have a consumer — one closed `Polyline` per record,
  to be filled rather than stroked, because the member lines already
  stroke the outline. A fill with a hole is still one ring: every
  area is walked counter-clockwise and every hole clockwise, joined
  by a crossing walked both ways (`boundary_fill_ring`), so it paints
  the same under the even-odd rule the vendor's plots fill with
  (`f*`) and by winding number. See
  `docs/analysis/2026-07-07-phase34d-0013-igboundary2d-grammar-decode.md`,
  `docs/analysis/2026-08-10-fill-has-a-consumer-after-all.md` and
  `docs/analysis/2026-11-10-what-the-sixty-unreferenced-pages-leave-undrawn.md`
  §10.6.
- `0x0010` is **not a record family**. No chain of any stream in the
  corpora — sheet, cluster or style — holds a record of this type
  code. The Phase 18 "polymorphic sub-record family", the Phase 19
  `leading_word` distribution and the Phase 26 attribute fragments
  were all found by sliding a decoder over a `Sheet*` stream a byte at
  a time, and what that matched is a graphic record's own sub-header:
  every graphic record's payload opens `oid, owner, layer` and then a
  `u16` sub-type word at `+12`, which is `0x0010` on most drawing-side
  records, followed at `+14` by the record's style id. Read as an
  envelope that is type `0x0010` with `bytes_to_follow` = the style
  id. Of 66 214 such hits across the corpora 65 543 sit exactly on a
  host's `+12` and claim the host's style id as their length, and
  12 498 of the 12 531 "attribute fragments" are the text of the
  `igTextBox` they sit in, which that family decodes anyway. Since the
  families decode on the record chain, `decode_sub_records_0x0010` and
  `decode_attribute_fragments` return nothing on any sheet, and
  `SheetGeometry::decoded_sub_records_0x0010` /
  `decoded_attribute_fragments` are empty. The two registry rows,
  their DTOs and no-op emitters are still in place; retiring them is
  a later change. The `_at` entry points still read at any offset,
  which is how `examples/probe_what_the_chainless_families_matched.rs`
  re-finds the old hits. Nothing was ever reverse-engineered for this
  code: "referenced from JStyleOverride `+38..41` / `+56..59`" was one
  phantom pointing at another. See
  `docs/analysis/2026-11-10-what-the-sixty-unreferenced-pages-leave-undrawn.md`
  §3 and §12.
- **Every family decodes on the stream's own record chain, and
  nowhere else.** A `Sheet*` stream — and a symbol cache's
  `JSite*/PSMcluster0` — is an 8-byte header (the magic `0x6C90F544`,
  then the number of records the native reader reads) and records end
  to end. `SheetChain::walk` reads that framing, and
  `PsmRecordDecoder::scan` and every `decode_*s(data)` decode at the
  offsets it yields. A `decode_*s` call on a bare run of records with
  no header in front therefore decodes nothing; `decode_*_at(data,
  offset)` still reads at any offset. The scan that slid a byte at a
  time accepted byte patterns inside other records: the three families
  above, whole, and 13 `igSmartFrame2d`.
- **A chain that does not hold is read as far as it holds, and
  reported.** When the walk stops before the stream's end — a length
  of zero, a length past the end, fewer than six bytes left, a stream
  shorter than its header — the records before the stop decode as on
  any stream and nothing after it is read. The walk is not picked up
  again: the first offset from which a chain walks to the stream's end
  lies inside the record for 21 214 of 173 982 corpus records, and for
  743 even when the header's count is held too
  (`examples/probe_can_a_stopped_chain_be_picked_up_again.rs`). A
  header whose magic or live-record count disagrees with a chain that
  does reach the end is reported the same way; the count is what gives
  away a stream cut off at a record boundary. The report is a
  `StreamChainFault` in `PidDocument::chain_faults` and
  `NormalizedPidGeometry::chain_faults`, one sentence in the
  geometry's `warnings` naming the stream, the stop offset and the
  bytes left unread, one suspect `sheet_stream_without_geometry` item
  in the export ledger, and one `warning:` line on `pid_plot`'s
  stderr; the exit code does not change. A stop ahead of the sheet's
  one live border frame also costs the page it states: the drawing is
  then laid out as a sheet with no frame is. No stream of any corpus
  raises a fault (`tests/sheet_chain_faults.rs`).
  `sheet_record_starts` keeps the all-or-nothing reading for the
  publish line's row reader and the semantic join, which read nothing
  from such a stream and do not report it.
- `0x0021 ComplexString` and `0x007B Group` are **associations**: a
  member count, a double, and one 14-byte entry per member (oid, range
  box, a word). Neither states a coordinate and neither emits: each
  member is a live record of the same chain that names the association
  as its owner and draws through its own family. The double is the
  unit of the members' range boxes, `range_scale` (`head_double`
  until 2026-11-11): a box is in `range_scale / 32768` m — 1.0 on
  fourteen of the corpora's twenty records, 2.0 on six
  (`CPECCHB-SQ-A1.igr`'s outer frame, five groups of the A0 / A1 `-W`
  templates). A `ComplexString`'s curve members are `igLine2d` /
  `igArc2d` with its layer and style id, joined by `0x0019 KeyPoint
  Constraint` members, which have no decoder and need none; a Group
  lists the two `igRectangle2d` of a border or, in `A0-W.igr` to
  `A3-W.igr`, the 37 records of a logo. See
  `docs/analysis/2026-11-10-what-the-sixty-unreferenced-pages-leave-undrawn.md`
  §10.4 and §16.

Phase 14 milestones are tracked in
`goals/phase14-sppid-sheet-geometry/progress.jsonl`. See
`docs/plans/2026-05-14-phase14-decoder-suite-final-summary.md`
for the full Phase 14 summary report.

### Adding a new record family (two-seam template, v0.11.8+)

The historical "seven-layer template" collapsed into two seams
(RFC `docs/plans/2026-07-16-psm-decoder-deepening-refactor-rfc-cn.md`,
landed as M1/M2 of the 2026-07-18 master plan):

- **L4 decode seam — `PsmRecordDecoder`** (`src/parsers/sheet_records.rs`):
  implement the trait on a unit struct (`type_code` /
  `min_record_len` / `decode_at` / `advance_of`); the shared default
  `scan()` decodes at each record the stream's own chain starts
  (`SheetChain::walk`), never at an offset inside another record, and
  `parse_psm_header` owns the 6-byte envelope. Only family-specific
  payload validation lives in your `decode_<type>_payload`. Add thin
  `decode_<type>s` / `decode_<type>_at` wrappers for API symmetry, and
  register both in `tests/parser_panic_safety.rs`: the stream entry
  point in `exercise_all_parsers`, the `_at` in `AT_ENTRIES`, whose
  type code also plants the chains that reach your payload decoder.
- **L6 emission seam — `GeometryEmitter`** (`src/geometry.rs`):
  implement `emit(sheet, out)` and register in the `EMITTERS` table.
  Audit-only families register an explicit **no-op** emitter (policy
  is visible in the table, tested by
  `audit_only_families_register_no_op_emitters`).

Unchanged supporting steps per family: probe example
(`examples/probe_<type>_shape.rs`), 6–12 unit tests (canonical +
every validation rejection + panic safety), model DTO
`Decoded<Type>Record` + `From` + `SheetGeometry` field in
`src/model/sheet.rs`, `streams/cluster.rs` wiring, schema needle,
byte-audit trace, cross-fixture ratchet in
`tests/parse_real_files.rs`. Registry-driven wiring for these
remaining parallel lists is planned as M3 (`SheetRecordFamily`
registry) in
`docs/plans/2026-07-18-architecture-deepening-master-plan-cn.md`.

The geometry output conservation gate for any refactor in this area
is `tests/geometry_golden_snapshot.rs`
(`UPDATE_GEOMETRY_GOLDEN=1` to bless intentional changes).

## Probe examples (`examples/`, `pid-plot/examples/`)

A probe is a throwaway `main` that asks one local fixture a question
and prints the answer — how a decoder question gets settled before it
earns a test. The committed form is named for what it settles, not for
what it reads: `examples/probe_<the question>.rs` (268 of the 276
tracked root examples carry that prefix; the other eight predate the
convention and keep their names).

`probe_complex_dash_marks` reports a mark's **anchor**, not the centroid of
its ink: the L-shaped glyph's two arms sit 0.635 mm apart along the run, so a
centroid reading is 0.64–0.71 mm early for every mark of that glyph. Each
stroke is matched to the arm sharing its direction, so a run whose across arm
merges with the pipe wall still reads true, and the side window is the glyph's
own reach across the run (its widest local y plus 0.25 mm) rather than a fixed
2.2 mm, which used to admit the neighbouring wall's along arm.

`--sweep` runs that reading over every `.pid`/`.pdf` pair under
`PID_PARSE_SELF_CHECK_DIR`, one line per sheet, and exits non-zero when a run's
mark count leaves the reference's or a centre drifts past 0.20 mm. The corpus
reads 134 runs, 0 count mismatches, 0.19 mm worst.

Four probes carry the evidence for reading a sheet by its record chain.
`probe_which_decoded_records_start_off_the_chain` lists, per family, the
decoded records that start where their stream's chain starts none: 92 405
before the families decoded on the chain, none since.
`probe_what_the_chainless_families_matched` says what three of those families
(`SubRecord0x0010`, `AttributeFragment`, `JStyleOverride`) had been decoding: it
walks every stream that opens with the cluster magic and tallies where records
of `0x0030` and `0x0010` really live (3 253 of `0x0030`, all in `StyleCluster`;
none of `0x0010`), then repeats the old sliding walk through the three `_at`
entry points and files each hit under the chain record it sits in; it exits
non-zero if a sheet chain holds a record of either code, or a hit turns up that
its host does not account for.
`probe_can_a_stopped_chain_be_picked_up_again` takes each record of each intact
sheet stream in turn for the one a walk stops on and searches on from its
envelope for the first offset from which a chain walks exactly to the stream's
end: of 173 982 records the search lands inside the record on 21 214, and still
on 743 when the picked-up chain has to add up to the header's record count —
which is why a broken chain is read as far as it holds and no further. It reads
the compound files itself, not through the parser.
`probe_what_the_byte_audit_no_longer_claims` measures what `--byte-audit` stopped
marking consumed when four families lost the hits a sliding scan had found off
the chain: over the 53 backup sheets, coverage of the sheet streams went from
99.24% to 98.65% (68 552 bytes on 33 streams; `A井场`'s `/Sheet6` from 99.0% to
97.9%), and every one of those bytes lies in a superseded copy of a record —
`0x8000` in the type word — which the native reader skips and no family decodes.

`pid-plot/examples/probe_export_bytes_unchanged` records 71 pages — the 53
backup sheets, the 7 fixtures, the 11 reference sheets — and compares two
recordings (`record <label>`, `compare <a> <b>`, `show <label>`; a label or the
path of a recording). With no variable set each batch is laid out the way its
own gate lays it out: the backup against the package it was shelled out of, the
fixtures against `test-file`, so that each wears its committed template, the
reference sheets against the acceptance corpus. A page keeps its PDF's hash and
the model's taken apart four ways — geometry, ink, provenance, facts — hashed
with the checkout root written `<repo>`, so recordings made in two worktrees
compare directly and `compare` names each page that differs and which part of
it moved. That is how a merge is checked: record each side, record the merged
tree, read off the pages that moved.
`pid-plot/examples/probe_what_the_ledger_baselines_hold` sums the two committed
ledger baselines by reason, layer and domain, and needs no corpus.

`pid-plot/examples/probe_does_a_sheet_letter_alone_as_in_a_batch` asks whether
a sheet letters every label in the same faces when its faces are prepared for
it alone (what `pid_plot` does) and when they are prepared for all the drawings
it is given (what a gate does). It writes each page both ways and reads every
run's face back off the PDF — `Tf` by the font's `BaseFont`, `Tj` by its
`ToUnicode` — so it repeats none of the writer's rule, and exits non-zero when
a label differs. The five runs its module docs list read 76 sheets and none
differing.

`pid-plot/examples/probe_which_label_a_paired_run_is_read_against` prints, for
the eleven acceptance sheets, what the acceptance gate's two lettering checks
rest on: each pair the comparison read as a piece of a label (a ` part` class)
or found no label for (`unknown`) with the paper-table label it was read
against, each label past the 0.15 mm / 0.05 mm limits the gate counts as
`WholePage::text_off`, and every lettering class with its means, whole page and
masked. It reads 24 pieces, 12 pairs with no label and 64 labels past the
limits.

A `tmp_`-prefixed example is the same tool before its answer is worth
anything: `examples/tmp_<subject>.rs` is gitignored, nothing cites it,
and it disappears once the answer lands. When the numbers do turn
out to matter, rename it to `probe_<the question>.rs` and commit it —
that promotion is what lets a later session re-run the evidence.
`pid-plot/examples/` follows the same naming; the ignore pattern covers
only the root directory, so keep scratch there out of commits by hand.

## Common commands

```bash
cargo test                                        # 1000+ tests (851 unit + 91 integration, 2 DWG-gated skipped)
cargo test --test publish_xml_cli -- --nocapture   # CLI integration
cd vendor/oxidized-mdf && cargo test --lib         # vendored unit tests (31 tests)
```

Give every extra checkout of this repo (a `git worktree`, a scratch
copy) its own `CARGO_TARGET_DIR`. Cargo keys a workspace member by its
path relative to the workspace root, so checkouts sharing a target
directory share build fingerprints: a build in one checkout made after
the other's last source edit looks up to date to the other, whose tests
then run the first checkout's code without recompiling. If that has
already happened, update the modification time of `src/lib.rs` in the
checkout under test to force a rebuild.

A `cargo` that warns "ignoring the configured target directory … this
cargo always uses `./target`" (a fork, tagged as such by `--version`)
keeps every checkout's intermediate artifacts and test executables in
one directory under `CARGO_HOME\targets` regardless — the sharing above,
with no variable that opts out. Run the toolchain's own cargo there,
`rustup run nightly cargo …`, which honours `CARGO_TARGET_DIR`.

## Pre-commit gates (CI mirrors these)

Run **all five** before `git push`. CI (`.github/workflows/ci.yml`)
fails on any drift and will block merges:

```bash
cargo build --locked --workspace --all-targets
cargo test  --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all -- --check        # apply with `cargo fmt --all`
bash .github/scripts/check-missing-docs.sh   # rustdoc ratchet
```

`pid-plot` links pdfcraft, so these two stay on the parser crate and must
not be widened to `--workspace`:

```bash
cargo test --locked -p pid-parse --no-default-features --lib
cargo check --lib --locked -p pid-parse --no-default-features --target wasm32-unknown-unknown
```

### `missing_docs` ratchet

`.github/missing-docs-baseline.txt` stores the maximum number of
`rustdoc -W missing-docs` warnings the crate is allowed to produce.
The gate is **ratchet-down only**:

- Adding a `pub` item without `///` docs bumps the count → CI fails.
  Document the new item.
- Intentionally lowering the count (by documenting more items)
  requires also editing the baseline file to the new lower number,
  in the same PR. That keeps every doc improvement visible in
  git history.
- Raising the baseline is almost never the right answer; treat it
  as a temporary crutch only.
- The script fails when `cargo rustdoc` itself exits non-zero, whatever
  the count. The crate denies `rustdoc::broken_intra_doc_links` and
  `rustdoc::private_intra_doc_links`, so a link in public
  documentation to a private item stops rustdoc before the crate is
  documented — and the count of what it had printed by then, zero,
  used to pass. Name a private item in plain code, `` `name` ``, not as
  a link. Where the `cargo` on `PATH` is the fork that ignores
  `CARGO_TARGET_DIR` (see Common commands), put the toolchain's own
  `bin` first on `PATH` for the script: `rustup run stable bash …`
  does not reorder a Git Bash `PATH`.

See `.github/scripts/check-missing-docs.sh` for the exact command
(`cargo rustdoc --lib --locked -- -W missing-docs` + grep count).

- `--workspace` makes the vendored `oxidized-mdf` crate a hard gate.
- `RUSTFLAGS=-Dwarnings` (set in CI env) promotes compiler warnings to
  errors; keep local output clean by the same bar.

### Security audit (CI-only)

CI also runs an independent `cargo audit` job in parallel with
the test matrix. It scans `Cargo.lock` against the [RustSec
advisory database](https://rustsec.org/) and fails the build the
moment a known CVE surfaces in any transitive dependency.

- Reproduce locally: `cargo install cargo-audit --locked && cargo audit`.
  The `--locked` belongs to `cargo install` (so the install
  obeys the lockfile of `cargo-audit` itself); `cargo audit` does
  not accept `--locked` as a flag and errors out if given one.
- `cargo audit` reads but never rewrites the committed
  `Cargo.lock`, so the scan is byte-for-byte deterministic on
  every CI run.
- Failure path: bump the offending crate in `Cargo.toml`
  (or whichever `vendor/<crate>/Cargo.toml` carries it),
  run `cargo update -p <crate>`, then re-run `cargo audit`
  to confirm the advisory disappears before pushing.

## Parser hardening playbook

Cadence used by PRs #3–#7 to harden `src/parsers/*` one edge case
at a time. Every PR follows the same TDD loop and ships as a
single squash-merge:

1. **Reproduce as a red test.** Add a focused `#[test]` in the
   parser's own `#[cfg(test)] mod tests` that constructs the
   panicking / wrong-result input and asserts the desired
   behaviour. Run `cargo test --lib parsers::<module>` and
   confirm it fails with the exact panic / diagnostic you expect.
2. **Minimum patch.** Prefer one of these guards over
   restructuring:
   - `text.get(a..b)` / `slice.get(a..b)` instead of
     `&text[a..b]` for char-boundary or out-of-range safety.
   - `checked_add` / `checked_mul` for size arithmetic on
     untrusted `u32` lengths.
   - `(cursor + len).min(data.len())` to clamp end indexes after
     the corresponding `cursor + len <= data.len()` check.
   - Early `return None` for `0`-valued lengths and obviously
     bogus discriminators / magic bytes.
3. **Run all five pre-commit gates** (see above) and confirm the
   new test goes green, every other test still passes, and the
   `missing_docs` count is unchanged.
4. **One commit, one PR.** Commit message format
   `fix(parser): <imperative summary>`. PR body keeps a short
   rationale + the test plan checklist. Squash-merge with branch
   deletion: `gh pr merge <n> --squash --delete-branch`.
5. **Smoke-test guard.** `tests/parser_panic_safety.rs` walks
   every public byte-level parser entry against an adversarial
   corpus on every CI run; new entry points must be added there
   in the same PR that introduces them. A `decode_*_at(data,
   offset)` goes into its `AT_ENTRIES` table, which is called at
   every offset and checked against the sources; a record family's
   payload decoder is reached through the planted chains, which
   take their type codes from the same table.

### Where to look for the next target

When sweeping for new edge cases, start with these patterns:

- `String::from_utf8_lossy(data)` followed by `&text[a..b]`
  slicing — non-ASCII bytes inflate to the 3-byte `U+FFFD` and
  shift offsets; the trailing index can land inside a multi-byte
  char and panic. Guard with `text.get(a..b)` (regression: PR #6,
  `scan_guids`).
- Untrusted `u32 as usize` lengths fed into `cursor + len > …` —
  watch for missing `checked_add` / `checked_mul`, especially on
  32-bit targets where `len * 2` can wrap.
- `data[i]` / `data[i..j]` direct indexing without an upstream
  `i + N <= data.len()` guard.
- `s.find(…)` on `from_utf8_lossy` output where the byte index is
  later used with `&str` slicing.
- `unwrap()` / `expect()` on `Iterator::next()` after `peek()`
  returns `Some` — sound by contract but cosmetic noise to remove
  when the surrounding diff already touches the function.
