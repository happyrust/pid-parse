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
| `tests/parse_real_files.rs::a_sheet_chain_holds_no_gline2d_and_the_old_hits_were_a_page_frames_ratio` | No chain of five fixtures holds a record of type `0x3FE6`, no registry row reads it, and a chain walk stops on that word: the `GLine2d` family was retired on 2026-11-11. The three hits the Phase 14 byte-sliding scan counted (`DWG-0201` 2, `A01` 1) are re-found at every offset with the family's matcher, which lives on the test side, and each is held to what it was: 160 bytes into an `igSmartFrame2d`, on the top two bytes of the aspect ratio that record states at payload `+148` (its height over its width: `1/√2` on an ISO A sheet, give or take the frame's margins) | `test-file/*.pid` |
| `tests/parse_real_files.rs::a_sheet_chain_holds_no_jstyle_override_and_the_old_hits_were_block_length_words`, `::a_sheet_chain_holds_no_0x0010_record_and_the_old_hits_were_sub_type_words`, `::the_old_attribute_fragments_were_the_inside_of_text_boxes_and_placements` | No sheet chain of four fixtures holds a record of type `0x0030` or `0x0010`, and no registry row reads either: the three families that used to be found by sliding a decoder over the stream were retired on 2026-11-11. Their matchers are kept for this, on the test side (`tests/support/retired_matchers.rs`) — the hits those scans returned are re-found at every offset and each is held to what it was: 98 "`JStyleOverride`" on the length word of the block a `DependencyObject` ends with, 582 "`0x0010` sub-records" on a graphic record's own sub-type word and style id (`+12` / `+14`, all but one), 84 "attribute fragments" inside an `igTextBox` (77, each the text the box's own decoder reads) or an `igSymbol2d` (7) | `test-file/*.pid` |
| `tests/retired_matchers.rs` (with `tests/support/retired_matchers.rs`) | The matchers of the four retired families (`GLine2d`, `JStyleOverride` on a sheet, `SubRecord0x0010`, `AttributeFragment`) live on the test side: the library exports none of them, since each reads untrusted bytes and matches nothing a file holds. The support file is the code as it left `sheet_records.rs`; a test includes it with `#[path = "support/retired_matchers.rs"]`, a probe with `#[path = "../tests/support/retired_matchers.rs"]`. This test file holds the matchers' own rules, the 34 unit tests they had in the library — what each accepts and refuses at an offset, and that a chain hands a `0x3FE6` record or one flagged `0x4000` to no matcher | inline bytes; runs in CI |
| `tests/parse_real_files.rs::iglines_decoder_emits_decoded_iglines_with_provenance` | Phase 14 Slice J igLine2d (PSM 0x0018) | `test-file/*.pid` |
| `tests/parse_real_files.rs::iglinestrings_decoder_emits_decoded_polylines_with_provenance` | Phase 14 Slice K igLineString2d (PSM 0x0084) | `test-file/*.pid` |
| `tests/parse_real_files.rs::igpoints_decoder_emits_decoded_points_with_provenance` | Phase 14 Slice L igPoint2d (PSM 0x005E) | `test-file/*.pid` |
| `tests/parse_real_files.rs::igtextboxes_decoder_emits_decoded_texts_with_provenance` | Phase 14 Slice M igTextBox (PSM 0x004D) | `test-file/*.pid` |
| `tests/parse_real_files.rs::igsymbols_decoder_emits_decoded_symbols_with_provenance` | Phase 14 Slice N igSymbol2d (PSM 0x00CE) | `test-file/*.pid` |
| `tests/parse_real_files.rs::igboundaries_decoder_emits_typed_audit_records_with_provenance` | Phase 34-D igBoundary2d (PSM 0x0013) exact-count ratchet | `test-file/*.pid` |
| `tests/parse_real_files.rs::dwg0201_keeps_its_inferred_line_floor` | DWG-0201 keeps at least 49 EndpointPair-inferred lines, and every decoded line of the drawing is an `igLine2d` (the Phase 14 Slice E guard, less the `GLine2d` half it asserted until the family was retired) | `DWG-0201GP06-01.pid` |
| `tests/sheet_family_wiring.rs` | Four holds on every committed drawing and template. (The fourth, `a_range_scale_is_set_by_how_far_the_children_reach`: every record that lists children with a range box — a sheet's own `JSheet` table, a `ComplexString`, a Group; 24 sheets, 3 strings and 13 groups in the repository — states the scale its children's reach calls for, `1.0` under a metre from the origin to the far corner of their box and `2.0` beyond; sheets of both scales are read, and `A1-W.igr`'s border group at `2.0` beside its logo group at `1.0` is pinned. Moved to 1.05 m, the line names eight A1 records.) The first three: Each family's wired `SheetGeometry` field has as many records as a fresh registry re-decode of the same bytes. Every decoded record starts where its stream's own record chain starts one, so no family decodes a byte pattern inside another record. A `ComplexString` or a Group draws through the records it lists: each member is a live record of the same chain that names it as its owner, a string's curves are `igLine2d` / `igArc2d` on its layer with its style id, a group's members are the two rectangles of a border or the 37 records of a logo, and every line member's range box is the line itself when read at the record's `range_scale` — the eleven templates inside the committed `RefData~*.zip` are read for this half too (12 groups, 5 of them at scale 2; the 16 lines of `A0-W.igr`'s logo lie half a metre from their boxes if the unit is taken for 1/32768 m) | `test-file/**/*.pid`, `*.igr` and the committed archives; runs in CI |
| `tests/sheet_chain_faults.rs` | A chain stream whose framing does not hold is read as far as it holds, and says so. `DWG-0201GP06-01.pid`'s `/Sheet6` is damaged six ways — a record length of zero in the middle, the last record one byte too long, four stray bytes at the end, cut off at a record boundary, the header's record count off by one, a wrong magic — and each raises exactly one `StreamChainFault` and one warning that names the stream, the offset the walk stops at and the bytes left unread; every record before the damage decodes, family by family, as on the intact file, and no `Decoded` entity comes from past the stop. Each such record also keeps the place the sheet's own list gives it in the display order (`sheet_display_order`), the records drawn at one place keep their order, and nothing past the stop is ranked. A length that is wrong and still inside the stream is held separately: thirteen changes — the two of the second independent review and eleven that used to read as a sound chain — each stop at the first offset past the changed record on a type word no record opens with (`NoSuchClass`), and the fault names that word and the last record read; and every record of every committed chain stream has its length changed by ±1 … ±64 with the tallies pinned (`LENGTHS_ON_DISK`, `LENGTHS_ARCHIVED`): none reads as a sound chain through a record the file does not hold, no registry family decodes a record read off the chain, and the three retired matchers would accept 62 of them. Those are fourteen changes a record. The corpus test below makes all 128 of −64 … +64 and puts every record a walk then reads off a chain to all sixteen families: on the repository's files (3 776 624 changes, 61 060 such records, pinned) no family decodes one; on every corpus in reach (29 120 173 changes, 1 371 448 records) none decodes as a page frame, and four decode as `igComplexString2d`, all on one sheet of the plant backup — a ceiling the test holds. A `StyleCluster` whose chain stops is reported like a sheet's, and its table is the records the fault says the chain read: every single-length change of the fixture's three style tables that stops the walk (3 405; a walk with no tripwire reads past the stop on 122) builds the table of the stream cut at the stop. A symbol file's broken sheet is named on the body it leaves short. The corpus test parses every drawing, template and symbol in reach, each distinct file once, and holds it to no fault and each of its chain streams to the magic, a walk that ends on the last byte and the header's live-record count: `test-file` with its archives always (floors pinned: 675 files, 1 409 sheet streams, 78 symbol caches, 37 656 records), both `pid-test-data` and the share where a machine has them. The archives are a package's `RefData~*.zip` and its `PlantData~*.zip`, read for their `.igr`, `.sym` and `.pid` members — the drawings a whole-package publish reads among them (885 distinct documents on this machine, 265 492 records). The same walk tries the retired `GLine2d` matcher at every offset of every chain stream and holds each hit to sitting 160 bytes into a page frame, on the frame's own height over its width (13 across the corpora; the 10 in the repository pinned). It also holds every live page frame to decoding and every intact stream to no refused frame (91 across the corpora; the repository's 32 a floor) and tries the frame decoder at every offset off a chain where a walk could read a frame's type word, whatever wrong length leads there (179 places across the corpora, 83 of them frames by the decoder's first rules, none now; the repository's one pinned). With the frame rule taken away this test fails on the repository's files alone; with either half of it taken away — the extent being lengths, or the ratio — it fails where the outside corpora are, and the lib's unit tests fail everywhere | `test-file/**` (committed; runs in CI); the outside corpora where present — each prints what it read |
| `tests/sheet_display_order_fixtures.rs` | The display order of the committed files, read off the files themselves (the parser's own unit tests are on bytes built by hand). For each of the seven fixture drawings and the five border templates under `test-file/Ref/Template Files`: every top-level sheet stream that holds a record of the sheet's own has an order and one without holds no live record; the sheet's list names exactly the live records of the stream whose parent is the sheet; every live record takes a rank; and the objects listed (1 014 over the seven drawings), the records ranked, the place of the border frame (`DWG-0201GP06-01`: 91) and a digest of the order are pinned per file (`PINNED`). The counts it compares with are taken from the stream bytes with the chain walk, not through the module. `A01.pid`'s embedded sheet `/JSite204/Sheet6` is held to its own order — 97 objects, 107 ranked records — and to the place of the frame that embeds it, 0. A second case walks `test-file` and fails on a drawing or template `PINNED` does not list. With a record's place taken from its oid, two of the three cases fail | `test-file/**/*.pid` and `test-file/Ref/Template Files/*.igr` (committed; runs in CI; a missing file fails) |
| `pid-plot/tests/cli_pid_plot.rs`：断链的 7 条（这个文件连同下面那一行的 27 条共 34 条） | 记录链立不住的流在 `pid_plot` 上：`DWG-0201GP06-01.pid` 的 `/Sheet6` 改坏一个长度字段的三份副本（中间一条记录长度为零、最后一条多一个字节、流尾多出四个字节）各导出一页，退出码不变，stderr 恰好一行 `warning:`，点名副本、`/Sheet6`、链停下的偏移与没读的字节数；页面与「同一张图、这条流干净地截到停点为止」的页逐图元相同，停点之前的图元与完好图纸逐个同一（位置相同当且仅当读到的页框相同：这张图唯一活着的页框是流的最后一条记录）；页面按图纸自己的显示次序画读到的那些记录（`paint_sequence` 有值，且与截到停点的那一份相同）；账本把这条流记成一项可疑的 `sheet_stream_without_geometry`，字节数是没读的那部分，守恒与交叉核对照过。第四份原样重写的副本一个字都不说，页面与账本同完好图纸。第五份只把流头的记录数加一：链走到流尾、页面逐图元就是完好图纸，stderr 一行不带停点的 `warning:`（`live record(s) where the header states 355`），账本记一项、字节数 0。第六、七份是「长度错而未越界」：第 1 条记录长度 +8、第 31 条长度 −16，链从不是记录起点的偏移接着读、停在类型字不是记录开头的地方（`+143`、`+3062`）；除了前几份的各项，另断言停点是坏记录之后第一个偏移、告警说 `is not one a record opens with` 并点名坏记录是最后读到的那一条。把类型字绊线关掉，合并后的 26 条里 3 条失败、23 条通过：失败的是这两条和 `a_sound_stream_with_no_record_of_the_sheets_own_says_what_takes_no_place`（第九次审查在 26 条时量的；之后加的八条——第 27 条、符号文件的五条与页框被拒的两条——没有在那个改动下重量）（第一份原先读成 359 条记录的完好流，一个字都不说）。七份副本另各自断言链读到、却没有显示次序位置的记录：第 1 条长度 +8 那一份有 1 条（2 个图元；链只读到这一条，它挂在图纸的一个对象下面，那个对象在停点之后，所以流里没有图纸自己的记录可以对照图纸的表），账本在流那一项之外另记一项 `record_without_display_place`、可疑比完好图纸多 2 项，其余六份没有；七份上 `display_order_warnings` 都不另起一句——链停造成的没有位置，由链的那一行说：那一份上链的一行接着说「读到的记录没有一条是图纸自己的」「1 条记录、2 个图元没有位置」「页面是模板名给出的页幅 594 × 420」，其余带停点的副本上那一行不说这些；每份带停点的副本上 stderr 那一行就是库函数 `chain_fault_warnings` 的那一句 | `test-file/DWG-0201GP06-01.pid`（入库）；四条在没有字体目录时照样断言 stderr、页面模型与账本，只是退出码断言为 1（停在第一个缺的字型上，警告在要字体之前打印） |
| `tests/publish_chain_faults.rs` | `pid_publish_xml` says when a record chain of a `.pid` it publishes does not hold, for a single `.pid` and for each drawing of a backup package. The publish reader takes a stream's records all or nothing, so a copy of `DWG-0201GP06-01.pid` with one length of `/Sheet6` set to zero publishes 0 of 102 label values where the drawing publishes 92: the copy exits 0 with exactly one `warning:` naming the file, the stream, `stops at +16994 of 29594 bytes`, the 12 600 bytes unread and what the reader is left without, in the chain's own numbers — 120 of the 179 live records the chain read before the stop are attribute rows, text boxes, dependency objects or symbol placements and none of them is in the XML, and the other 175 the header states are not read — and the line is the library's sentence (`PublishChainFault::describe`) from the one read that makes the drawing (`read_publish_from_pid`). The sentence says nothing of what a stream draws: it is the framing (`StreamChainFault::describe_framing`) and the publish reader's own consequence. A `/StyleCluster` whose chain stops is said too, as costing the XML nothing the chain read, and the data and meta XML are the intact drawing's byte for byte. A copy whose `/Unclustered Dynamic Attributes` stops — on its first record, or in the middle with 115 rows read — exits 1 with the warning ahead of `no P&IDAttributes row carries a DrawingNo`, and the warning says the loss the error then reports: no live record read and 231 stated, or 115 rows not in the XML and 116 more not read. A header count one too many is said, with the difference, and the stream is still read. A package of two drawings, one damaged, publishes both, exits 0 and prints one warning led by the zip and the entry (`BackupPublishReport::chain_faults`); `_Index.json` lists that sentence among its `problems` and still counts no failure, the sound package's index lists none, and the index and the coverage reports keep their keys | `test-file/DWG-0201GP06-01.pid`, `test-file/D06.pid` (committed; runs in CI) |
| `tests/boundary_loops.rs` | A boundary of two loops fills the area between them. `XA-A1.igr`'s oids 5074 and 5076 — outer loops of 104 and 162 lines, counters of 77 and 54 — decode with their loops, anchor and member references, and each reaches the sheet as one fill ring whose area is the outer loop's less the counter's, with the record's anchor inside it and a point of the counter outside | `test-file/Ref/Template Files/XA-A1.igr` (committed; runs in CI) |
| `tests/elliptical_arcs.rs` | `0x007E igEllipticalArc2d` on both of its paths. In cached symbol bodies: A01's four head arcs decode and bulge outward from their vessels. On a sheet's own chain: `XA-A1.igr`'s two arcs close the counters of an outlined `B`, each starting and ending on the ends of the counter's edges and bulging away from it, and every stroke of that wordmark — the lines, the two arcs, the 23 `igBspCurve2d` of the letters' bowls — draws with the pen its style id names (0.25 mm) rather than the default. And the other direction byte: the four `-W` templates of the committed `RefData~4~682.zip` each keep four arcs and four B-splines in their logo; on `A1-W.igr` the arcs are the top cap, the bottom cap and two left-side pieces of one ellipse, clockwise, each on the ends of the logo's lines, the caps turning 106.26° on the far side of their chord | `test-file/export-test/publish-data/A01/A01.pid` (soft-skips when absent), `test-file/Ref/Template Files/XA-A1.igr` and `test-file/backup-test/DWG-0202GP06-01_p/RefData~4~682.zip` (committed; absent is a failure) |
| `tests/sym_elliptical_arc_matches_cached_body.rs` | A `.sym` body's elliptical arc and the same arc in a drawing's cached body trace the identical rational B-spline (`Diaphragm Actuator.sym` against `I井场`'s `/JSite37311`). The backup is looked for under `PID_TEST_DATA_DIR` and the two `pid-test-data` copies, and the first root that holds **both** the drawing and `RefData~4~681.zip` is the one read: taking the first root that was merely a directory skipped this test on a machine whose first copy is a package with no `backup-plant-data` shelled out | the plant backup drawing and its `RefData~4~681.zip` (soft-skips when no root has both; feature `refdata-archive`) |
| `tests/sym_sheet_layers_read_in_one_order.rs` | A symbol file's `sheet_layers` read in one order, and the file says which: storages in path order, the root first, and within a storage the order its `PSMcluster0` holds the layers in. Every `.sym` of the committed `RefData~4~681.zip` (616, pinned) is read through `read_symbol_geometry_from_bytes` and held to that order as worked out from the file's own streams; the 40 (pinned) whose storages keep tables that differ — the only ones whose layers can be listed in more than one order — are read four times. The test then runs itself as another process (`--exact`, with `PID_PARSE_SYM_LAYER_ORDER_ASKED` set), which reads the same files as often, and the two processes' readings must digest alike. With the tables gathered in a hash map again, 38 of the 40 read out of order in a run and the two digests differ | `test-file/backup-test/DWG-0202GP06-01_p/RefData~4~681.zip` (committed; runs in CI; feature `refdata-archive`) |
| `tests/symbol_library_text.rs`, `tests/symbol_library_sheet_pick.rs`, three tests of `tests/parse_real_files.rs` (`rectangles_own_their_edges_and_the_bspline_reaches_its_body`, `a_cached_body_carries_the_stroke_styles_its_own_storage_states`, `a_cached_arc_sweeps_clockwise_from_its_start_angle_to_its_end_angle`) and five unit tests of `src/symbol_library.rs` | The twelve tests that read a symbol of the plant's **whole** library rather than of the three-symbol sample under `test-file/symbols`: labels the text reader used to refuse, every text record of all 618 symbols (1 265 read, none refused, none unreadable), which sheet a body is drawn from, a cached body's strokes against the library's own style tables (17 / 11 / 6 / 4 / 2 placed symbols on the five fixtures), the `Remarks` cloud's arcs, and an arc's direction, a dash, lettering and an override read off one `.sym` each. They read the library out of the archives it is committed in — `tests/support/library_symbols.rs` for the integration tests, `library_symbol` in the unit-test module — so they assert on every checkout. Until 2026-11-12 they read `test-file/symbols-full`, which is gitignored, and ten of them returned without a word where it was absent; `a_symbol_carries_the_colour_and_width_its_own_style_table_states` looked for `ElecTraceLine.sym` where it never was, had run on no machine, and asserted a colour and a width the file does not state (its strokes are an override of `Normal` at 0.13 mm, black; the blue the trace-heating row draws in is the placement's `As Drawn`). A new test that needs a symbol of the whole library reads it the same way and not off the directory | `test-file/backup-test/DWG-0202GP06-01_p/RefData~4~681.zip` (616 `.sym`) and, for the two it lacks, `…/TEST02_p/RefData~4~681.zip` — together the 618 files of `symbols-full`, byte for byte (`examples/probe_is_the_unpacked_symbol_library_what_the_archives_hold.rs`). Committed; runs in CI. A build without the `refdata-archive` feature reads the directory, where a machine has it |
| `tests/soft_skip_spelling.rs` | A soft skip is spelled one way: no string literal of the crate's test code — the files under `tests/`, and each file under `src/` from its `#[cfg(test)]` module on — opens with `skip: ` or with `skipping ` and no colon, the two spellings the tests used beside `skipping: ` until 2026-11-12 (58 literals). That is what keeps `tools/soft_skips.py`'s count worth reading; a skip that prints nothing, or says it in other words, it cannot see | the crate's own sources (131 files of test code, 200 notices, on the tree with the parse line's sixth round merged); runs in CI |
| `tests/render_gap_census.rs` | A sheet record that does not reach the drawing is named. Per fixture, the refused and the undecoded graphic records are pinned in records — none of either on any fixture — every refused record has a warning, the lines a retired rule refused are counted, and the records the native reader skips are counted apart. The two documents that last carried a refusal, `A井场` of the plant backup and the committed `XA-A1.igr`, are pinned to none, in the prose and in the structured list alike; with no document left to read a refusal's wording on, that is held on synthetic rows in `geometry.rs` (`a_refusal_reads_differently_from_a_missing_decoder`, `a_sheet_whose_only_gap_is_a_refusal_reports_no_missing_decoder`) | `test-file/*.pid`, `XA-A1.igr` (committed); the backup sheet where present |
| `tests/parser_panic_safety.rs` | No byte-level parser entry point panics. An adversarial corpus and its truncations go through every stream entry point. Every `decode_*_at(data, offset)` — the 24 of `AT_ENTRIES` (the frame decoder's `smartframe_refusal_at` among them), which `every_at_entry_point_is_listed` checks against the sources, so that the four retired matchers cannot come back into `src/parsers` unlisted — is called at every offset of every input, one past its end and where `offset + 6` overflows. And because a family decodes only where a record chain starts a record, 270 planted streams (the magic, the record count, one record of each family's type code for each of 28 lengths, filled with noise, zeros or `0xFF`) carry the stream entry points past the chain into every payload decoder: 182 of them are chains and hand the decoders 5 070 records of the right length and the wrong content; the other 88 carry a type word no record opens with (the `0x4000` flag, and `0x3FE6`, planted for being past the class table) and are held to stopping on their first word | inline bytes; runs in CI |
| `pid-plot/tests/export_every_fixture.rs` | Every fixture `.pid` exports its whole page: laid out from the committed sources the ledger gate states, each of the seven wears the border template `FIXTURE_TEMPLATES` names (a drawing with no row fails rather than passing unframed), the writer's `mismatches` is empty, `cross_check` finds nothing (8.6 included, with real faces), and the PDF reads back with exactly the ink segments the model strokes and the characters it letters, the template's among them. None names a symbol no library carries — the 熊安 fixture's own `Xa …` symbols and its `XA-A1.igr` are committed under `test-file/Ref` The order the lettering is read back in is this test's own statement (`painted_order`: the decoded layer then the inferred one, each in the model's `paint_sequence`), not the writer's `written_order`, so a writer that walks its sequence another way fails here. How each page ends its strokes is read back too (`FIXTURE_ENDS`: the inked paths stroked butt-ended and the ones stroked round, per fixture — `工艺管道及仪表流程-1` 4 903 and 389), and no path is stroked any other way | every `test-file/**/*.pid`; faces from `PID_PLOT_FONTS` or the share's `fonts` (the export soft-skips without a font directory; the walk and the template table still assert) |
| `pid-plot/tests/export_plant_backup.rs` | 53-sheet plant backup exports, page size / off-sheet / page-weight ratchets, every page exactly the paper its sheet states (`PAPERS`: 17 sheets on 594 × 420, 27 on 841 × 594, 9 on 1189 × 841; none measured), exactly one primitive of a drawing's own past its page (the sideways `N井场` run whose first glyph is clipped), what each of the fifteen frame-offset sheets' template holds past the paper pinned sheet by sheet (`TEMPLATE_PAST_THE_PAPER`: the sheet outline, and on the nine `CPECCHB-SQ-A2` sheets the line that restates its right edge, with the reach past the right and top edges — in the model, not printed; and how the ledger files the unprinted segments, by sheet: `PLOTTED_PAST_THE_PAPER`, the nine sheets a reference plot shows placed so, three segments each under `segment_beyond_the_paper`, and `UNPLOTTED_PAST_THE_PAPER`, the six it does not, with the template each wears, two segments each under `segment_beyond_the_paper_no_plot_of_template`; nothing of a drawing's own is filed past the paper), known library gaps only, and no drawing's own ink wider than the widest pen (a validation mark such as `BandAid`'s 10 mm is not a pen). Which template each sheet wears is pinned exactly — `CPE-SQ-A0/A1/A2` on 8/22/7 sheets, `CPECCHB-SQ-A1/A2` on 4/9, the two `W` templates on **0** (so their 50 pinned label cells are held against no page until a sheet wears one), three bare sheets by name — with the 175 label cells and 64 template placements the run holds against a page. The readings a run used to print without asserting are pinned as well: the 15 sheets whose frame places the template off the page origin, by page key with the offset in millimetres (`FRAME_OFFSET_SHEETS` — all nine `CPECCHB-SQ-A2` sheets at (0.388407, 0.565648), `N井场`/`O井场`/`P井场`, `A井场` and the two `凝析油稳定塔` sheets); all 64 template placements naming a file no library carries (`TEMPLATE_NAME_GAPS`); which label is past its page and where it starts (`PAST_THE_PAGE`: `15-TD-4101416-B2-EH40` at (259.113810, −0.207788) mm); and each sheet's template coming from the kind of source its corpus calls for — disk exactly when a template directory of the corpus holds a file of that name, a `RefData~*.zip` otherwise, which is 0 disk / 50 archive in the package and 50 disk / 0 archive in the share, held sheet by sheet rather than as two totals. A failure names the measured value, the pin and the sheets that differ. The typefaces a run embeds are printed by name and deliberately not pinned (which faces a machine has is its own business; the ledger gate skips its two font marks when they are not the baseline's). **Also carries the export-ledger gate** for this corpus: each page's `ExportLedger` is assembled from the same `PaperModel` and write result, `cross_check` must find nothing, the face-free account must match the written one except for the two font marks, and every page is compared against `docs/baselines/export-ledger-backup.json` (suspect counts may only fall, intentional counts are pinned exactly). That comparison needs the layout to read a backup package, so a package in reach is the default: with `PID_PARSE_SELF_CHECK_DIR` unset the corpus is the directory above the `backup-plant-data` the gate found whenever that directory holds `RefData~*.zip`, and the share otherwise. A run that names a corpus reads it, and a named share skips the count comparison (requirement 13.7) and passes on the unattributed and cross-check rules alone. A package picked by default is held to more: if its symbol or template sources are not the baseline's the comparison is skipped all the same, and the test then **fails** for having compared nothing (the baseline's symbol sources include this machine's gitignored `test-file/symbols-full`, so a checkout without that directory has to name its corpus). The first line a run prints names the corpus and says why it was picked The caps are read back from the 53 PDFs: 4 374 inked paths stroked butt-ended and mitred, 79 651 round (`STROKE_ENDS`), and the one sheet whose own thin strokes its list puts ahead of its first wide pen, `凝析油一级闪蒸分离器B气相出口改造`, 47 and 430 (`THIN_AHEAD_OF_THE_WIDE_PEN`) — the one reading of any gate that tells the round-cap rule from the one before it (round ends from the drawing's first stroke on any pen reads 4 327 and 79 698) | `backup-plant-data/**/*.pid` under `PID_TEST_DATA_DIR`, the named corpus (the share when none is named), or either `pid-test-data` (soft-skips when absent); faces from `PID_PLOT_FONTS`, the corpus's `fonts`, `test-file/fonts`, then the share's `fonts` for a package picked by default (no face directory at all fails rather than skips) |
| `pid-plot/tests/export_ledger_fixtures.rs` | 7 张仓库夹具的出图记账门禁：每页 `assemble` 后五层（解析 / 排版 / 符号体 / 写出 / 段）未归因全为 0，`cross_check`（8.1–8.5 与 8.7；8.6 要字体，由 `export_every_fixture` 与 `export_plant_backup` 在写出的页上执行）无不符，同一页排两遍、两次序列化逐字节相同，序列化结果与语料描述头不含本机目录、任意盘符路径或 UNC 路径（原样与 JSON 转义两种写法都查），最后与 `docs/baselines/export-ledger-fixtures.json` 逐页比对（可疑项只许降、有意项精确钉）；语料描述与基线不同时**失败**，不跳过比对（`GateReport::counts_compared()`）。**没有字体需求，也没有任何软跳过分支**：7 张夹具、`test-file/symbols`、`test-file/Ref/Symbols`、`test-file/Ref/Template Files` 与 `test-file/backup-test/*_p/RefData~*.zip` 全部已入库，所以本机与 CI 都真跑。`UPDATE_EXPORT_LEDGER=1` 重写基线（可疑项会升高则拒绝写入） | 全部在仓库内（`test-file/**`），CI 上真跑 |
| `pid-plot/tests/cli_pid_plot.rs` | `pid_plot`（实际用来导出 PDF 的程序）作为进程起起来的 27 个用例：缺参数退出码 2 并打印 usage；图纸不存在退出码 1、不写 PDF；命令行上给的 corpus 不是目录时退出码 2、点名该路径、不写 PDF；没给 corpus 而回退目录不存在时只在 stderr 警告（警告要说出它「不是目录」）、照常导出；corpus 是目录、却够不到图纸点名的模板时（`D06.pid` 配一个空 corpus），退出码与写出的页不变，stderr 恰好一行 `warning:`，点名图纸、模板名（`PaperModel::missing_templates` 的每个词干，印成 `<词干>*.igr`）与 `border_template_places` 报的每个模板目录；没点名模板的图（`A01.pid`）配同样的空 corpus 一行警告都没有；`test-file/D06.pid` 以 `test-file` 为 corpus 导出后，stdout 首行（图元数、页幅、字节数）等于库内同一排版与盘上文件，PDF 读回有墨有字，账本汇总未归因为 0；`D06.pid` 的一份副本——`JSheet` 表的第一个 oid 换成图纸没有的，于是读不到显示次序——导出的页图元不变，14 条记录、40 个图元记成一项可疑的 `record_without_display_place`，stderr 恰好一行 `warning:`（点名 `/Sheet6`、条数、原因与画在哪里），入库的那一份一个字都不说；这份副本同时没有纸张（纸张写在同一条记录里），页面是页框矩形，同一行警告接着说这件事。**纸张不是页框的纸张**的六份 `D06.pid` 副本——宽度改成一半（297 × 420）、宽高对调（420 × 594）、只有高度错（594 × 450）、一边为 0（1000 × 0）、各加一米（1594 × 1420）、表的子对象数加 4（次序照读，纸张从错开 56 字节的地方读成 0 × 1000）——各自：页面是页框矩形 594.328 × 420.566、图元与入库的逐个相同、账本页级恰好一项可疑的 `paper_unlike_the_frame`、没有一项 `*beyond_the_paper`、有意项不多于入库那一页，stderr 恰好一行 `warning:`（图纸、读到的纸张、页框），退出码不变。另三条在 `DWG-0201GP06-01.pid` 的副本上：**流干净地截到第 1 条记录之后**（流头记录数改对，链完好，没有链故障）——次序读不到，1 条记录、2 个图元没有位置，stderr 恰好一行，说的是「流里没有图纸自己的记录，图纸的表无从对照」和「页面是模板名给出的页幅 594 × 420」，不说「没有表」「页框」「链不成立」；**链中间断开、表里又有一个没有记录带着的 oid**——两件事，两行 `warning:`，各按内容断言，账本两项都在（把「链停了就不另起一行」放宽成不看原因，这一条失败）；**去掉唯一活着的图框**（流截到最后一条记录之前）**再把纸张宽度改成一半**——没有图框时模板名不否决纸张：页面是纸张 297 × 420，页级恰好一项可疑的 `paper_unlike_the_template_name`（不是 `paper_unlike_the_frame`），stderr 一行说两个尺寸、说「没有读到图框」、说「页面是纸张」，不说「页框」「不采用」。另五条是**符号文件断链**（2026-11-12）。图纸缓存着自己放置的每个符号的体，放置从缓存画，只有图纸里没有缓存体的放置才读符号库，所以用例先造出这条路：`一级闪蒸分离器B` 夹具的一份副本，把压力表那一个放置点名的缓存体改成图纸没有的（`igSymbol2d` 记录的倒数第二个字，定义图纸的 oid），再配一个只有一份文件的符号库——入库的 `PG-Pressure Gauge.sym`。**完好的文件**：64 个放置从缓存画、1 个从库画，页面与入库夹具逐图元相同，可疑项相同，一个字都不说。**`/Sheet6` 一个长度字段改成零**：stderr 恰好一行 `warning:`（点名副本、符号文件、`/Sheet6`、停点与没读的字节数、放置数），账本在图纸域记一项可疑的 `symbol_file_chain_fault`（`<文件>:<流>`、计数 1、字节数是没读的那部分），整页的可疑项比配完好文件时恰好多这一项，`ExportLedger::summary().suspect`（门禁读的、程序打在 stdout 上的那个数）也恰好多 1，导出跑完时 stdout 上的 `suspect` 就是这个数，退出码不变；页上的压力表画成文件里另一张图纸 `/Sheet68` 的样子——圆之外多一个完好文件不写的 `PG`——其余图元不变；入库的夹具配同一个坏库不说也不记（它的压力表从缓存画）。**`/Sheet6` 一个类型字带上 `0x4000`**：链的走查报停，符号读取器照读，页面与配完好文件时逐图元相同，警告的句子与账本项照样有，`summary().suspect` 多 1（这一条不起子进程）。**一个文件两条流都坏**（`/Sheet68` 与 `/StyleCluster` 各一个长度字段改成零；两条都不是页面取墨的地方，页面与配完好文件时逐图元相同）：stderr 恰好两行 `warning:`、账本两项（各带自己那条流没读的字节数：49 与 894）、`summary().suspect` 多 2。**模板域**：边框模板的放置同样先用模板缓存的体，所以造一份 `D06` 的模板 `CPECCHB-SQ-A2.igr` 的副本，把它会签栏的符号 `hqz.sym` 的两个放置点名的缓存体改成模板没有的，放进用例自己的 corpus 的 `Ref\Template Files`，符号库里放一份叫 `hqz.sym` 的文件（内容是入库的压力表：手头没有哪个库带着真的 `hqz.sym`，站在会签栏里的是什么不是这条用例管的事）。配完好的文件：模板的三个放置 1 个从缓存画、2 个从库画，一个字都不说；配 `/Sheet68` 断链的文件：stderr 恰好一行、以 `in the border template, ` 开头、说 `2 placement(s)`，账本这一项记在模板域而不在图纸域，`summary().suspect` 多 1，页面与配完好文件时逐图元相同。点名的是模板里存的那个路径（`C:\Users\ADMINI~1.WIN\AppData\Local\Temp\hqz.sym`，作者机上的路径）：这种放置只凭文件名找到库文件，库不说实际读的是哪一份。把「模板域的发现交给账本」那一句去掉，模板这一条失败、其余 31 条（那时这个文件 32 条）与库内 365 条都过；把账本改成一个文件只记第一条流，两条流那一条与库内新加的一条失败（先红后绿）。**图纸自己的页框被页框判定拒收**（两条）：`D06.pid` 的两份副本，`/Sheet6` 上唯一活着的 `igSmartFrame2d` 记录只改八个字节——比值偏三百万分之一，或宽写成 0（宽为 0 的真页框）。stderr 恰好一行 `warning:`，点名副本、`/Sheet6`、记录的偏移与 oid、被拒的原因（解码器自己的那一句，数字由测试从副本的字节算出再比）、页面是图纸声明的纸张 594.000 x 420.000 mm；页级账本恰好一项 `page_frame_refused`（来源：流、oid、`0x003D`），整页的可疑项比入库图纸恰好多它和解析层的 `record_refused` 各一项，守恒与交叉核对照过；图纸自己的图元一个不少、模板照样找到；退出码不变；入库的 D06 一个字都不说。子进程读的四个环境变量逐例显式设定 | `test-file/D06.pid` + `CPECCHB-SQ-A2.igr`、`test-file/export-test/publish-data/A01/A01.pid`、`test-file/DWG-0201GP06-01.pid`、`test-file/集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图.pid` 与 `test-file/symbols/…/PG-Pressure Gauge.sym`（入库）；导出要字体，没有字体目录时 D06 导出那一条软跳过，其余 26 条到处都断言（两条模板用例、读不到次序那一条、六条纸张用例、`DWG-0201` 的三条、符号文件起子进程的四条与页框被拒的两条没有字体时断言退出码 1 与 stderr，警告在要字体之前打印） |
| `pid-plot/src/write.rs::an_acceptance_sheet_letters_alone_as_it_does_among_the_eleven` | 借字的字型与同批图纸无关：验收语料的每张图单张备字（`pid_plot` 的做法，2026-11-12 起也是验收门禁、墨迹门禁与 `write_five_drawings` 的做法）与全部图一起备字（备份与夹具两道导出门禁仍然这样做，见「排字规则由谁守」），逐条标签比写出器发出的 `Tf` / `Tj`，每段文字的字型都相同；A2 图 `轻烃分离器I` 那两处点名 Arial Narrow 的「动火点」钉为整条借 NSimSun（字体目录里没有 `nsimsun.ttf` 时是 SimSun）。规则是 `fonts::BORROW_ORDER`，所有标签同一张六个字型的次序：NSimSun、SimSun、FangSong、Arial、Arial Narrow、SimSun-ExtB——原厂参考图里借来的字全是新宋体，所以它排第一，没有这个文件的机器退到同族的宋体；出借字型按整个字库判定，不看这次备字切出的子集，六个都画不了的字单张、成批以同一条错误拒绝（点名字符与标签）。调用方自带的字型（`pid_plot` 用 `PID_PLOT_CJK_FONT` 嵌入、名字为空的那个）排在这张次序之后。借来的字怎么画另有一处，`pid-plot/src/write/borrowed.rs`，只管参考图有样本的那一类：图纸自己的、正排、点名 Arial Narrow、落在 24 个设备单位的字格上（字高 2.469 / 2.5 mm）、没有声明字宽、只有一种样式的标签，借来的是整条标签或末尾单独一个字。这一类照参考图画——`Tf 9.6`（20 个设备单位，即 GDI 给 Arial Narrow 在这个字格上的字符高度）、不做横向缩放、整条借字每个字按 Arial Narrow 的 `?` 走 9 个单位，全宽的字因此相叠，与参考图一样；末尾单独借一个字的标签（`L=35℃`）参考图逐字写在整设备单位上，所以借来的字之前的那一段逐字落在自己的步进上（一个 `TJ`，最后一个字后面也带调整量），`℃` 站在前一个数字的步进上而不是它的自然宽度之后——验收 11 页配上的 12 条逐字与参考图相差不到 0.005 个单位；样本够不到的标签（别的字型、别的字格、旋转、模板家具、声明了字宽、借来的字后面还有点名字型的字）照旧用所在那一段的字号与出借字型自己的字宽，见 `docs/analysis/2026-11-11-a-borrowed-glyph-is-set-at-the-named-fonts-character-height.md`。借字次序另有 17 条不依赖语料的库内用例（`fonts.rs` 9 条、`write.rs` 8 条），其中 15 条用 pdfcraft 自带的 Inter 造字型，在没有授权字体的机器上钉住单张与成批同字型、同批的图让别的字型的子集也装着被借的字时写出器照样按次序问（把 `lenders()` 改回按字型表次序，库内有 7 条失败，其中 5 条不依赖语料）、没有 NSimSun 时备字与写出都退到 SimSun、整条借字不被切开、点名字型画不了的空白字符（`动　火　点` 的 U+3000）照借而标签末尾的空白不借、run 的字型缺文件、run 点名别的标签自有字型、前面的出借字型都画不了的字单张与成批借同一个字型（没有字型能画则同一条错误）、调用方自带的字型不论在表里排第几都最后才问九种情形。参考图的画法另有 4 条不依赖语料的用例：样本那一类读回 20 个单位的字身与 9 个单位的步进、借来的末字之前的每个字各落在自己的步进上（`the_glyphs_before_a_borrowed_last_glyph_each_stand_on_their_step`）、样本够不到的八种标签保持所在那一段的字号、NSimSun 在参考字型表里与 SimSun 是同一组读数；读真字体的 `a_centred_borrowed_han_run_steps_back_by_the_reference_pitch` 从写出的 PDF 读回出借字型、20 个单位的字身与 38 个单位的 run 长 | `PID_ACCEPTANCE_CORPUS`（默认 `验收数据`）加共享语料的 `fonts`（语料里一张 `.pid` 都没有、或缺三个字型之一，则打印原因并软跳过；语料里有图却没有 A2 那张则**失败**，不带着没读的钉值通过）；那 21 条单测只用 pdfcraft 内嵌的 Inter 或不用字型，没有软跳过分支，造出的字体目录名带进程号、用例结束即删 |
| `pid-plot/src/write/escapement.rs`、`pid-plot/src/write/text_matrix.rs`、`pid-plot/src/write/script_runs.rs`、`pid-plot/src/write/letter_spacing.rs` 与 `write.rs` 里的十五条用例 | 文字对象怎么写进 PDF：转角、记录的分段、文字矩阵、按文字种类切对象、字距五条规则，各条的见证写在各条里。**转角**：出图程序把标签的转角以整十分之一度交给 GDI（`render.dll` 填 `LOGFONT.lfEscapement` 时就近取整），所以离正排或直角不到 0.05° 的标签就排在正排或直角上，`Tm` 写整数（`1 0 0 1`、`0 1.012 -1.012 0`）；别的转角原样保留，不归到十分之一度。备份 10 条、夹具 1 条离正排 9e-8～2e-4 弧度的标签因此是正排的字，不再按旋转的字放大 0.8%～1.2%；`escapement.rs` 的 5 条与 `write.rs` 的 `a_label_a_hair_off_level_is_lettered_level` 等 3 条钉住。这条规则是照 `render.dll` 填 `LOGFONT` 的那个例程读出来的，参考图见证的比它少：见证到的是 470 条旋转标签全部写在直角上、矩阵是整数，和正排的字长什么样（备份那 10 条排出来的 Arial `Tf 12.45`、Arial Narrow `Tf 10.17`）。**没有见证**的两件：「离直角多远还算在直角上」——参考图的标签离直角最远 1.2e-10°，0.012° 的标签按正排排只凭那个例程；「取就近而不是截断」——同一个库里给选进 DC 的字体定转角的另一个例程把同一个乘积截断，两种读法分得开的只有不足直角的标签，带参考图的是 45 条（模型里比直角少一两个浮点末位到 9e-13 弧度），最长 23 个设备单位，按 89.9° 排末字只横移 0.04 个单位、矩阵照样印成 `0 1.012 -1.012 0`，与直角是同一张页面，所以参考图分不开两种读法；哪个例程的值最后到了 GDI 没有追到（180° 的标签按前一个例程读成 0，按后一个读成 1800）。71 页里两种读法排出不同页面的只有备份 `低温分离器C` 的 `25-WO-0103528-B2-EH40` 一条（296 个单位长，截断时末尾几个字横移 1 个设备单位）；量法见 `escapement.rs` 的模块注释。**记录的分段**：记录里的每一段按它自己声明的字高量宽、定步进（`a_run_at_a_height_of_its_own_is_spaced_as_a_label_of_that_height`）；「每一段各自从整像素起笔」只用在各段同一种样式的记录上，那是 `SP:` 见证的一类，各段换字型或换字高的记录（夹具的 10 条管线号）后一段接在前一段的终点上（`only_a_record_in_one_style_starts_each_run_on_a_pixel_of_its_own`），这一类没有参考图，见 `docs/analysis/2026-11-11-nine-fixture-pipeline-labels-follow-the-record-run-rule-unwitnessed.md`。**文字矩阵**：旋转的或样式写了字宽的标签，`Tm` 写驱动的定点值——`L = trunc(across × 1250000)`、`Q = L div 256`、每项 `trunc(e × 1250000 / Q)`，印 `n/256` 三位小数——所以宋体 800‰ 在 30 单位字格上是 `0.797` 而不是 `0.8`，旋转的 Arial 是 `0 1.012 -1.012 0`；`the_fourteen_matrices_of_the_reference_plots_are_reproduced` 把 11 张参考图上全部 14 个非平凡矩阵当表钉住（宋体九档、旋转的宋体、Arial 的 `M`、旋转的 Arial 与 Arial Narrow）。矩阵按字体算，不按标签算（`write.rs::SetFont`）：一条标签里借来的字、记录里点名别的字型或别的字高的段各有自己的 `Tm`，从前一段的终点起笔（71 页上没有一条标签走到「后面的文字对象另写一个 `Tm`」——内容流里 `BT` 与 `Tm` 各 21 951 个，整条借字的标签变的是它唯一的那个矩阵——这半句只有库内三条用例在走）；正排不压窄的标签每个字体都是 `1 0 0 1`，所以参考图见证过的借字那一类不受影响，受影响的只有夹具上 33 个文字对象（没有见证，`a_lent_piece_of_a_turned_label_is_set_under_its_own_fonts_matrix` 等三条钉规则照此执行）。**按文字种类切对象**：一个字串在拉丁字母、汉字、全角字符之间换类的地方切开，每段一个文字对象，从「字串起点 + 前面各字的整像素步进」起笔、字距按本段的字算；`a_string_is_cut_where_the_reference_plots_cut_it` 钉参考图的切法（`DWG-0100` \| `集05-37`、`I` \| `井场` \| `（2` \| `口井` \| `）`、A2 证书行的五段），`a_run_starts_on_the_steps_before_it_and_is_spaced_by_its_own_glyphs` 钉证书行的起点 134 / 238 与 `Tc` −0.494 / −0.288。**字距**（`letter_spacing.rs`，2026-11-12）：一个字串（或它的一段文字种类）是一个文字对象，n 个字都在同一个 `Tj` 里，`Tc` 照驱动的规则写——每个字在设备字体里的前进量就近取到 1/256 个设备单位，`Tc = 截断到三位((S − 前 n−1 个字的前进量之和) × 0.48 / ((n−1) × m))`，`S` 是出图程序给前 n−1 个字的整设备单位步进之和（不是末字画出来的位置：标题栏两档压窄的宋体画出来的末字离步进 0.58～2.3 个单位，参考图也是），`m` 是矩阵沿基线那一项的 256 分之几。11 张参考图多于一个字的文字对象 3 986 个：按位置与文字配上 3 904 个、按文字另配上 80 个，我方的 `Tc` 与参考图写出来的字符全部相同，另 2 个是手头图纸没有的字串（读回用 `tools/pdf_letter_spacing.py`，只读两份 PDF，不经仓库的写出器）。只有后面还有一段接着它的笔位起笔的字串仍把最后一个字单写在 `Tc 0` 下（71 页上 123 个，全在夹具——`工艺管道及仪表流程-1` 121 个、两张 `DWG-0202GP06-01` 各 1 个——参考图没有这种标签；末字之前有两个字以上的 33 个，只有一个字的 90 个，其中 78 个是借来的汉字之间的两个空格；整串带字距的字串 17 320 个，两样合计 17 443 个。读法是 `tools/pdf_last_glyph_apart.py`，只读 PDF）。用例：`letter_spacing.rs` 5 条（标题栏三档 `0.124` / `0.169` / `0.155`、证书行、就近与截断、除以矩阵那一项、没有间隔是 0），`write.rs` 的 `a_string_is_one_text_object_under_the_drivers_letter_spacing`、`a_string_another_piece_carries_on_from_has_its_last_glyph_apart`、`a_record_run_with_no_start_of_its_own_carries_on_from_the_glyph_before_it`（同一种样式的记录的后一段，在没有设备网格的页面上没有自己的整像素起点，同样接着前一段写）与改成逐位相等的 `a_stated_width_is_set_under_the_matrix_the_driver_writes`。前进量恰好在两个 256 分之一正中间时取远离零的一侧，没有见证：参考图的五种字型按它们的字格（256、2288、2319 个设计单位）算不出恰好一半。详见 `docs/analysis/2026-11-12-the-driver-states-a-letter-spacing-in-256ths.md`。`write.rs` 的十五条从 `show_text` 写出的算符读回 `Tm`、`Td`、`Tc`、`TJ`。规则的搜索与常数扫描在 `tools/driver_text_matrix.py`，读内容流的工具在 `tools/pdf_text_objects.py`；见 `docs/analysis/2026-11-11-the-driver-states-a-font-transform-in-256ths.md` 与 `…-a-string-is-one-text-object-for-each-script-run.md` | 不需要语料：字型的设计数（宋体 256 / 256 / 128，Arial 2048 / 2288 / 904，Arial Narrow 2048 / 2319 / 741）写在用例里，`write.rs` 的用例用 pdfcraft 自带的 Inter 或拿它造的字型；没有软跳过分支 |
| `pid-plot/tests/border_template.rs` | A0/A1/A2 sheets draw the frame, title block and sign-off column their linked `.igr` carries: both frames and 12 furniture labels on the `CPE-SQ` pages with the company name and `会 签 专 业` read back, and the A2 CCHB case on its own 35.354/7.071 mm insets, 116 strokes and 36 cells; a page with no same-size template draws none (pinned by `model.rs`'s extent unit test, no longer by an A2 negative). Three more on the committed fixtures, which run in CI: `D06`'s page is the 594 × 420 mm paper its sheet states, its frame's 594.328 × 420.566 mm rectangle is kept beside it, and of the two template primitives the frame holds past the paper the outline is on the page less its top and right pieces and the line along its right edge is not printed (`segment_beyond_the_paper`, no suspect drop); `CPECCHB-SQ-A2.igr`'s inner frame — a `ComplexString` of four lines — is painted second, behind the outline and ahead of the title block, in the order the A2 reference plot strokes it; and every furniture primitive of `XA-A1.igr` is painted where the template's sheet lists its record, the two elliptical arcs at places 997 and 998 and the two fills with a counter at 3 906 and 4 586 of 5 024 | the corpus `.pid` + `Ref/Template Files`; `test-file/D06.pid` + `CPECCHB-SQ-A2.igr` (soft-skips when absent) |
| `pid-plot/tests/style_witness.rs` | The plant backup draws only style vocabulary a reference plot has exercised: the eleven acceptance pages' 10 pen widths / 2 dash patterns / 4 faces / 3 templates are pinned, every unexercised entry is pinned with its drawing-vs-furniture split (3 of the 4 unexercised widths are `CPECCHB-SQ-A1` furniture, `0.5000 mm` is also `D04`'s own ink, the 2 unexercised dash patterns are the drawing's own ink), and the four widths the open band boundaries cannot settle are pinned — `0.3000 mm` inside `(0.25, 0.35)`, `0.5000 mm` inside `(0.35, 0.70)`, `0.7500 mm` and `0.7999 mm` inside `(0.70, 1.06)`. What the writer's rule prints each of those four on is pinned as well (`UNEXERCISED_PENS`: one, two, four and four device units): a width prints on its whole units of 1/150 inch, cut and never under one, which is the pen of all 19 393 straight pieces the eleven plots show (`probe_which_pen_a_declared_width_prints_on`) and, for these four, the pen the five-band table before it gave | `PID_ACCEPTANCE_CORPUS` (default `验收数据`; no fallback to the five-sheet corpus) and `backup-plant-data/**/*.pid` under `PID_TEST_DATA_DIR`, the corpus, or either `pid-test-data` (soft-skips when absent) |
| `pid-plot/tests/refdata_archive_corpus.rs` | A backup package supplies its own reference data: the named `.igr` comes out of its `RefData~4~682.zip`, a symbol body resolves from `~681.zip` with no directory root at all, and the same drawings — copied to a directory with no `Ref` and no archive in reach — draw nothing | `test-file/backup-test/DWG-0202GP06-01_p` + the repo's `test-file/*.pid` |
| `pid-plot/tests/reference_plots_acceptance.rs` | 11 张带原厂参考图的图纸（A0×3 / A1×4 / A2×1 与原先 5 张同目录）：页幅、coverage 下限、extra 上限、参考标签数、pen 一致数与不符上限、缺符号名单；另钉 A0/A1/A2 三张纸的掩膜矩形（A2 左列 ≤ 36 mm、不吞工艺接续）。A1 三张的 8.210 mm 已归零（单记号改为不早于段中点：16.09 mm 竖管 5.08 → 8.05，参考 8.04 / 8.07）；箱体下沿那圈「波形管束」是 `.sym` 与缓存体都有的两条 `0x0063 igEllipse2d` 风机椭圆，解码后 `uncovered` 23.031 → 5.757 mm，见计划 §7.1；剩下的 `/Sheet6` 划线端头在信号线虚线按每段直线重排之后也归零（`docs/analysis/2026-11-08-the-signal-dash-is-fitted-to-every-piece.md`），现在除 A2 那张外的十页 coverage 1.000000、uncovered 0.000 mm、extra 0.000 mm。配上的标签管两层：两条字符类阈值（平均下移 ±0.04、平均宽差 ±0.02 字高）管每一个类——不论条数、`unknown` 也算——掩膜视图与整页视图都查，没有例外：原先唯一超限、钉在读数上的 `Left/Ascender part`（以 `℃` 结尾的标签被切开的两段，8 页，+0.0294）在借来的 `℃` 照参考图用 `Tf 9.6` 画之后读 −0.0003（`℃` 之前的字逐字落在步进上之后是 +0.00003），例外名单已经删掉；被切开的标签的每一段另有位置上限（`PIECE_SHIFT_MM` = 0.05 mm，管带 ` part` 的类的 `max_shift_mm`，两种视图都查：现在读 0.0015～0.0229 mm，`℃` 站在前一个数字自己的宽度之后时读 0.108～0.133 mm，那时两条阈值与下面的标签界限都放过它），每页归进这种类的对数精确钉（`WholePage::pieces`，11 页合计 24，少一对就失败，上限不会空着通过）；锚点偏离参考超过 0.15 mm 或 run 比参考宽窄超过 0.05 mm 的标签按页钉条数（`WholePage::text_off`，11 页合计 0：文字普查的 64 条里，14 条借来的 run 照参考图画之后回到界内，三张空冷器图的 `SP:` 在记录里的每一段各自从整像素起笔之后回到界内，44 条标题栏里压窄的宋体在 `Tm` 照出图驱动写成 256 分之几之后回到界内——`write/text_matrix.rs`，规则复现参考图全部 14 个非平凡的文字矩阵，J K 页上的 `1-1/2"` 与 `FC`——同一条基线上的两条标签，原先被读图并成一个 run——在页面按图纸的显示次序画之后读回成参考图那样的两个 run；最后 2 条是 A2 页两条带盒的 `仪表风管线`，在这一行按记录写的盒挂在盒顶之后回到界内（`model.rs` 的 `hang_in_the_box`，上移 0.173 mm）），多一条少一条都失败并逐条列出。我方每页的页面还要等于图纸写的纸张（1e-6 mm 以内；参考图的页面框是驱动写出的同一张纸，差在 0.0014 mm 以内）。一对 run 属于哪个类由 `compare.rs` 的来源查找决定：从标签自己的锚点量、按整条 run 链拼出的文字找、被切开的一段认它那一行拼出的标签（类名带 ` part`），见 `docs/analysis/2026-11-10-every-lettering-class-is-held-and-every-label-counted.md`。借来的 run 另按字型族与参考比（`WholePage::borrowed` / `borrowed_in_family`）：标签点名的字型画不了的字，两页各自换别的字型写；凡配上对的这种 run，从两份 PDF 读回各自的字体名（`read::read_plot_with_faces`，不重复写出器的借字规则），按测试里的明文表 `FAMILIES` 归族——宋体、新宋体一族，仿宋一族——看我方与参考是否同族。11 页合计 14 条——12 条 `℃` 与 A2 图两处 `动火点`——现在 14 条全部同族：我方与参考都是 NSimSun `Tf 9.6`（字体目录里没有 `nsimsun.ttf` 时我方是同族的 SimSun）；2026-11-11 之前 12 条 `℃` 是 FangSong、只有两处 `动火点` 同族。每页的条数精确钉，同族数只许升——少了失败，多了要求把钉值提上去；读到表里没有的字体名直接失败。A2 图被拖动的仪表簇里另有两条借来的 `℃`（`L=-5℃`、`H=-15℃`）与参考配不上对，不在这个读数里。**验过并交付的字节就是 `pid_plot` 单张导出的字节**：每页的字型逐张备（`prepare_fonts(&[model], …)`，`pid_plot` 的做法），写出的页另断言与 `pid_plot` 作为进程对同一张图写出的文件逐字节相同（`pid_plot_page`：同一份语料、同一张字体目录表经 `PID_PLOT_FONTS` 交给它、去掉 `PID_PLOT_CJK_FONT`）；11 张一起备字时这条断言在第一页就失败（463 985 对 432 254 字节，第 57 个字节起不同——排字与笔画相同，字体子集与字形编号不同）。11 页全部过钉之后，这道门把刚验过的字节统一写到 `<corpus>\plot\<name>.pdf`（11 张交付）并写 `BUILD.txt`（HEAD / plot_src / written_unix / pages）；任一页未过钉则一张都不写，`PID_ACCEPTANCE_DELIVER=0` 关闭落盘（第二个检出跑同一语料时用） | `D:\work\cad\pid-test-data\验收数据`（soft-skips when absent） |
| `pid-plot/tests/reference_plots_ink.rs` | 同样 11 张图，按**纸上看得见的墨**读（验收门禁比的是「画了什么」，不分颜色，也读不到图纸边上的线、线帽和先后）。字型逐张备，读的每一页另断言与 `pid_plot` 作为进程对同一张图写出的文件逐字节相同（`tests/support/exported_page.rs`，与验收门禁对自己交付的页做的是同一条断言），所以读的就是验收门禁验过并交付的那一页的字节——这句话 2026-11-12 之前只靠两道门禁恰好用同样的办法备字，改回成批备字没有东西变红；现在改回去在第一页失败（`I井场`：462 246 对 430 508 字节，第 57 个字节起不同）。两页都用 `ink::PageInk::read` 读成墨与纸色两份，每页 29 个读数，全部是等式：页幅与参考纸张之差；掩膜视图与整页的 coverage / 未覆盖 / 多余 / 标签配对；笔宽一致 / 不符 / 未配；笔画、标签与字的颜色（两页都是墨，还是一页是墨、另一页只是纸色）；两页用到的墨色是否相同；沿图纸边的笔画按位置配对并比笔宽（`ink::sheet_edges`）；线帽或拐角与参考图同一位置的笔画不同的墨长（`ink::ends_apart`）；参考图用纸色画在墨上的每一处，我方那两样东西的先后是否相同（`ink::wipe_order`：11 页 91 处，89 处相同）。一页与参考图印得一样时每个读数都是理想值，所以表（`SHEETS`）里每页只写它自己的数——标签数、配上的笔画数、图纸边数、白色记号数；达不到理想值的读数各占一行「已知缺陷」，写明它读多少、为什么（还没修 / 等现场数据 / 未定）。读数变差失败；变好也失败，直到收紧钉值或删掉那一行。现在 22 行：还没修的一行都没有（A2 页幅两行、图纸边一行在页面改成图纸写的纸张之后删掉了），12 行等现场数据（A2 晚一版的仪表回路），10 行未定（`一级闪蒸分离器A` / `低温分离器A`：参考图没有印图框，也不按手头这张图纸的显示次序画）。线帽一行都没有：11 页都是 0.000 mm。另一条用例钉 A2 图上盒比字高的 4 条标签（`低压放空总管`、`仪表风管线` 各两条）：我方基线离参考图不超过半个设备单位（现在读 −0.0505 / +0.0251 mm；这一行挂在盒顶之前是 +0.1222 / +0.1979）。再两条只读图、不写页（2026-11-12）。`a_label_and_its_origin_mark_are_painted_in_the_order_the_sheet_lists_them` 钉「标签与它的原点标记谁先画」：符号体带纸色原点标记的标签 11 页共 96 条，图纸把文字的拥有者列在符号紧后的 49 条、列在符号之前的 47 条，参考图 95 条照图纸列的次序画（每页四个数 `MARKED_LABELS`）；唯一不照的是 `一级闪蒸分离器A` 的 `P70`（`AGAINST_ITS_LIST`），而 `一级闪蒸分离器B` 上同样的记录（三条记录只差各自文档内部的编号：oid、样式号、缓存体的引用）列在同样的两个位置、参考图照列的画（`ITS_TWIN`）——同样的记录、两张参考图、两种次序，没有哪条读记录的规则能同时给出，所以那两行「未定」留着、页面不改。96 条之外另有 1 条没读进来——A2 图的 `SP=-8℃`，参考图在它的标记处没有纸色笔画（晚一版挪走的那一簇仪表，参考图把这段字写在 16.9 mm 外）——另有 23 个点名带标记符号的拥有者名下只有一个点、没有文字，不是标签；两样都钉着（`NO_LABEL`、`NOT_WHERE_ITS_PLOT_HAS_IT`），没有因为别的原因没读进来的。`the_eleven_pages_draw_every_placement_from_a_cached_body` 钉 11 页每个放置从哪里画（`PLACEMENTS`）：图纸自己的 2 395 个、模板的 12 个，全部从文件缓存的体画，没有一个从符号库画，没有一个缺体。`PID_INK_GATE_OUT=<dir>` 把打印的表写到 `<dir>/reference-plots-ink.txt`；这道门从不往语料里写东西 | `PID_ACCEPTANCE_CORPUS`（默认 `D:\work\cad\pid-test-data\验收数据`）；第一张参考图（或图纸）不在时四条读语料的用例各打印原因并软跳过，只读表的那条用例（`every_known_defect_is_one_reading_with_a_cause`）到处都跑 |
| `pid-plot/tests/paper_model_golden.rs` | 7 张仓库夹具的整棵 `PaperModel` 逐字节钉死（页幅、模板与其家具、每个图元的纸面毫米）。**没有任何软跳过分支**：夹具、模板、项目符号与备份包里的 `RefData~4~681.zip` / `~682.zip` 全部已入库。符号来源是**声明**的（`test-file/Ref/Symbols` + 备份包 `~681.zip`），不走 `lay_out_drawing` 的自动搜索，因此不会被本机 gitignore 的 `symbols-full` 影响；序列化前把 `\` 归一为 `/`、仓库根替换成 `<repo>`，并把每个浮点量化到 1 µm（跨平台 libm 的末位差异，见下）。`PID_PLOT_BLESS=1` 重写金样（7 个文件共 3.97 MiB）。失败时打印第一个差异的 JSON 路径与两侧值，例如 `template.extent_mm[0]: expected 593.939394, actual 594.236364`。第二条用例钉每张夹具的**绘制次序**：`paint_sequence` 写成图元下标，放在 `tests/golden/paint-order/<slug>.txt`（7 份、7 732 个图元、36 KiB）；次序在模型旁边、不在模型里，所以页面模型金样不含它。一张夹具没有次序、或有画了东西却没有位置的记录，这条用例失败；差异报第一个分开的位置和两侧的记录。`PID_PLOT_BLESS=1` 连同页面模型金样一起重写 | 全部在仓库内（`test-file/**`），CI 上真跑 |

### Soft skips of the root crate's tests

A root-crate test that leaves a check unmade for want of data — a corpus
that is not in the repository, a fixture a checkout does not hold — prints
**one line that starts `skipping: `**, says what is missing, and passes. A
loop prints one line for each thing it passes over. That is the only
spelling: `tests/soft_skip_spelling.rs` refuses `skip: …` and `skipping …`
without the colon in test code.

`cargo test` shows nothing a passing test prints, so a green run says
nothing of how many tests asserted. To see it:

```bash
python tools/soft_skips.py --run             # runs the root crate's tests with --nocapture, counts and lists the lines
python tools/soft_skips.py --run --by-test   # single-threaded, each line under the test that printed it
python tools/soft_skips.py <log>             # the same, of a log made with `-- --nocapture`
```

(`--cargo "rustup run stable cargo"` where the `cargo` on `PATH` is the
fork; the benches are left out, their harness takes no test flags.) A gate
run that wants the count in its summary runs the root crate's set with
`-- --nocapture` and hands the log to the tool. CI's own `cargo test` is
captured, so its log shows none of these lines.

Read on 2026-11-12
(`docs/analysis/2026-11-12-what-the-root-crates-tests-skip.md`, which lists
every test, what it lacks, and what #62 would have to supply):

| | this machine | the repository alone |
|---|---:|---:|
| tests that pass with a check unmade | 15 | 45 |
| lines that start `skipping: ` | 21 | 55 in the simulation, 53 expected on CI |
| tests that leave early and say nothing | 0 | 0 |

"The repository alone" is a simulation on this machine — the three corpus
variables naming a directory that does not exist, the 34 hard-coded
`D:\work\…` roots moved, the worktree's `symbols-full` junction taken away
— not a reading off a CI runner. On `396593c` the same two columns read 16
and 57 tests; 1 and 10 of them said nothing, and 30 of the 57 said it in
another spelling.

What is skipped everywhere, this machine included:

- **The thirteen DWG mirror gates** (`publish_dwg_mirror.rs` 4,
  `publish_dwg_generated_parity.rs` 4, `publish_meta_parity.rs` 2,
  `publish_rel_parity.rs` 3) want
  `test-file/backup-test/DWG-0202GP06-01_p/extracted/Export.mdf`. That file
  has never existed: the package's `Export.dmp` is an Oracle `exp` dump,
  which the MDF line cannot read, and no SQL Server copy of plant `P01` is
  on this machine. They have run nowhere since they were written.
- `interface_parity_on_a01_writer_matches_reference_superset_post_a22`
  passes over seven tags the A01 reference does not hold; only the DWG
  gates would compare them.
- `controlled_pid_diff_pairs_report_stream_level_evidence_when_available`
  wants drawing pairs under `test-file/controlled-diff`, which are
  gitignored by policy and exist nowhere.

The other thirty skip only where the corpora outside the repository are
absent: the share (`PID_PARSE_SELF_CHECK_DIR`), the 53-sheet backup package
(`PID_TEST_DATA_DIR`) and the acceptance sheets (`PID_ACCEPTANCE_CORPUS`).

When a test needs data the repository does not hold: look for it in a
committed archive first (the symbol library and the border templates are
there, and `refdata_archive` reads them without unpacking); otherwise print
the line, return, and say so in the table above. A walk that reads "what is
there" asserts how much it read, so that reading nothing is not a pass.

### 排字规则由谁守

文字怎么写进 PDF——转角、文字矩阵、字距、按文字种类切对象、记录的分段、借字的字型与步进——由两处守：
`pid-plot` 的库内用例（`write.rs`、`write/*.rs`、`fonts.rs` 的 `#[cfg(test)]`），和验收门禁
`pid-plot/tests/reference_plots_acceptance.rs`（每页的 `text_off`、两条字符类阈值、被切开的标签每一段的位置、
借字的字型族、交付的字节）。
**按墨读的门禁 `pid-plot/tests/reference_plots_ink.rs` 不读文字的位置，不守这些规则**：它过了不说明排字没坏。

两处守的不是同一批东西。验收门禁只读得到验收页上有的标签，而且读的是位置，不是算符：

| 规则 | 库内用例 | 验收门禁 |
|---|---|---|
| 转角：离直角不到 0.05° 的标签排在直角上 | `escapement.rs` 5 条、`write.rs` 3 条 | 读不到：验收页的标签离直角最远 1.2e-10° |
| 记录的分段：每段按自己的字高量；同一种样式的记录才各段取整起笔 | `write.rs` 3 条 | 只读得到 `SP:`（三张空冷器图的 `text_off`）；换样式的记录验收页上没有 |
| 每种字体各写自己的文字矩阵 | `write.rs` 3 条 | 读不到：验收页上没有旋转或压窄、又带第二种字体的标签 |
| 以 `℃` 结尾的标签，`℃` 之前的字逐字落步进 | `write.rs` 1 条 | `PIECE_SHIFT_MM` 与 `WholePage::pieces`（2026-11-12 起；此前读不到，把那次改动退回去两道门禁都过） |
| 字距 `Tc` 与「一个字串一个文字对象」 | `letter_spacing.rs` 5 条、`write.rs` 4 条 | 读不到：新旧两种写法的字最多差 0.008 mm，22 份页面报告里只有 185 个类平均宽差的叶子动了（最多 0.0008 个字高） |
| 文字矩阵的 256 分之几、按文字种类切对象、借字的字型与字号 | `text_matrix.rs`、`script_runs.rs`、`borrowed.rs`、`fonts.rs`、`write.rs` | 读得到：`text_off`、两条阈值、`borrowed_in_family` |
| 交付的字节就是 `pid_plot` 单张导出的字节 | — | `pid_plot_page` 的逐字节断言；墨迹门禁与 `write_five_drawings` 对自己读的、留在盘上的页做同一条（`exported_page.rs`） |

第六次审查在 `9e045e1` 上对前四行做了 14 种故意改坏：库内用例拦住 13 种（另一种是等价改动），验收门禁只拦住
1 种（`SP:` 不再取整起笔），墨迹门禁试了 3 种，一种都没拦住。前五行里验收门禁守得到的只有表里写的两处：第 2 行的
`SP:`，和第 4 行（2026-11-12 起，`PIECE_SHIFT_MM` 与 `WholePage::pieces`）。转角、每种字体的矩阵、字距这三行，
以及第 2 行里换样式的记录，**只有库内用例在守**，跑过验收门禁不等于验过它们。

**待定、没有见证、行为没有改的三件**（都不影响验收页与备份页上的位置）：

- 借来的字在压窄的标签里压多少：现在沿用所在那一段的压窄比。审查在本机 GDI（屏幕 DC、系统自己挑的关联字体，
  不是出图的环境）量到的读数指向「关联字体拿到同一个 `lfWidth`」。只涉及夹具上 27 个文字对象；参考图上借来
  的新宋体全在 `1 0 0 1` 下。
- 以 `℃` 结尾的标签，我方是一个 `TJ`（每个字后面带调整量）加 `℃` 的一个 `Tj`，参考图是每个字一个文字对象、
  各带自己的 `Tm`。字的位置逐个相同，算符不同。
- 两张 `DWG-0202GP06-01` 夹具左上角倒写的 `DWG-0202GP06-02`（恰好 180°）按正排的规矩写成字串加 `Tc`；参考图上
  旋转的字都是一个字一个对象，但没有 180° 的标签。

2026-11-11 的独立审查在 `1ad2df8` 上做过 30 种故意改坏（审查报告不在仓库里）：库内用例拦住 28 种，剩下的
两种一种是等价改动、一种是语料里没有的情形；拿去过两道门禁的 11 种里，验收门禁拦住 10 种（漏掉的是
「`Tc` 不按 256 分之几重述」，效果不到 0.02 mm，库内用例拦住了），按墨读的门禁 11 种全部放过。

**字型为谁备。** `pid_plot` 为它导出的那一张图备字。把页面留在盘上的门禁照它逐张备：验收门禁（交付
`<corpus>\plot\*.pdf`）、墨迹门禁（读同样 11 页）、`write_five_drawings`（把五张图写进 `<corpus>\plot\`，
`compare_five_drawings` 再读回去）。三道都断言自己写出的每一页与 `pid_plot` 作为进程对同一张图写出的文件逐字节
相同——验收门禁用自己的 `pid_plot_page`，另两道用 `tests/support/exported_page.rs`（2026-11-12 加；此前这两道
「与交付的同字节」「与 `pid_plot` 同字节」没有断言守着）。把哪一道改回成批备字，它都在第一页失败（`I井场`，
`pid_plot` 写出 430 508 字节：墨迹门禁成批备出 462 246 字节，`write_five_drawings` 453 051 字节，都是第 57 个字节
起不同；验收门禁的读数在它那一行）。另外四道——
`export_plant_backup`、`export_every_fixture`、`plot_compare`、`plot_text_sources`——仍然为整批图一起备字，
理由：它们不留页面，读回的是笔画、字符、位置与账本，这些在两种备法下相同（第六次审查把 71 页两种备法逐个
文字对象、逐条笔画比过，全部相同；规则由 `fonts.rs` / `write.rs` 里「单张与成批借同一个字型」那组用例和
`probe_does_a_sheet_letter_alone_as_in_a_batch` 守）；成批备出的页只会比单张的重（子集更大），所以
`export_plant_backup` 的页重上限对单张的页同样成立；备份门禁的语料描述头把这批图用到的字型记成一张表
（`font_faces`），与入库的账本基线逐项比，那张表说的本来就是一批图。成批与单张写出的文件**字节不同**
（审查量到 71 页 28.0 MB 对 22.1 MB），这四道门禁写出的字节不是交付。

验收门禁在 CI 上软跳过（语料不在仓库里，见下一节），所以 **CI 上守排字规则的只有库内用例**。改文字的
写法时：新规则要有不依赖语料的库内用例（先红后绿），并在有语料的机器上跑验收门禁
（`PID_ACCEPTANCE_DELIVER=0` 不动交付目录）；逐字形核对参考图用 `tools/pdf_text_objects.py`，逐个文字对象比 `Tc` 用
`tools/pdf_letter_spacing.py <参考图目录> <我方导出目录>`。

### CI 覆盖：pid-plot 的测试文件哪些真跑、哪些软跳过

`pid-plot/tests/` 下现在是 17 个测试文件（2026-11-12 在树上数的；`tests/support/` 里的不是测试文件）：13 个
既有的，本 mission 新增的 `paper_model_golden.rs`、`export_ledger_fixtures.rs` 与 `cli_pid_plot.rs`，和
2026-11-11 新增的 `reference_plots_ink.rs`。下表量的是前 16 个，第 17 个没有量过，照代码写在表后。下表是**实测**定论，不是推断（2026-11-10 改过的四行例外，见表后的说明）：用
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
| `paper_model_golden.rs` | **真跑** | 无任何 `skipping` 行；7 行 `… matches its golden` 逐张打印，末行 `paper model golden: 7 fixtures, <字节数> bytes (<MiB>)`——量这张表时是 `4162420 bytes (3.97 MiB)`，金样每重写一次这个数就变，见下面「金样可行性」。干净检出里 7.28 s 通过。2026-11-11 加的绘制次序那一条只读同样的入库文件，照代码也是到处真跑，没有在干净检出里重测；它多打一行 `paint order golden: 7 fixtures, 7732 primitives` |
| `refdata_archive_corpus.rs` | **真跑** | 4 个用例全 `ok`（0.77 s）；夹具 `test-file/backup-test/DWG-0202GP06-01_p` 已入库，`backup_dir()` 返回 `Some`，`skipping the archive corpus` 一次都没出现 |
| `border_template.rs` | **混合（真跑 A2 与 A01，A0/A1 软跳过）** | 4 个用例全 `ok`，（2026-11-11 起是 7 个：`an_a2_sheet_is_written_on_the_paper_its_sheet_states`、`an_a2_templates_inner_frame_is_painted_where_its_complex_string_is_listed` 与 `the_xa_template_paints_every_record_where_its_sheet_lists_it` 只读仓库里的夹具和模板，照代码是到处真跑的，没有在干净检出里重测）但只有 `an_a2_sheet_draws_its_linked_a2_template`（`D06.pid` + `CPECCHB-SQ-A2.igr`）与 `the_a01_sheets_embedded_frame_draws_the_a2_template_it_states`（`export-test/publish-data/A01/A01.pid` + `CPE-SQ-A2.igr`）的建模段真跑——两者的夹具都在仓库里。A0/A1 两条各打印一行 `skipping border template; C:\nonexistent-pid-corpus\I井场 注采阀组工艺及自控流程图.pid is absent` 与 `skipping border template; C:\nonexistent-pid-corpus\集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图.pid is absent`，因为 `corpus_dir()` 指向不存在的目录。两条 A2 用例的渲染段也打印 `skipping the page render; no font directory is present`，但建模段已过钉 |
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
| `cli_pid_plot.rs` | **混合（33 条真跑，D06 导出软跳过）** | 没有字体目录时 `d06_exports_the_page_the_library_lays_out` 打 `skipping the D06 export; no font directory is present (searched …)`；其余 33 条不需要字体（2026-11-12 第十一轮把解析线第六轮的两条页框被拒的用例并进来之后把 `PID_PARSE_SELF_CHECK_DIR` 指向不存在的目录重量过：34 条全过，一行 `skipping the D06 export`，另有 32 行 `<case>: no font directory is present`——七条断链、两条回退目录、两条模板、读不到显示次序那一条（它起两次程序，两行）、六条纸张不是页框的、`DWG-0201` 上的三条、符号文件的四条（六行：断链那条与模板那条各起两次程序）、页框被拒的两条（四行：各起两次程序）各走没有字体的那一支；剩下 6 条到不了要字体的地方）。两条回退目录的用例、两条模板用例与七条断链用例在没有字体时断言退出码 1（过了 corpus 检查、停在第一个缺的字型上），各打一行 `<case>: no font directory is present (searched …), so the run can only get as far as the first missing face`；模板那条与三条断链的照样断言 stderr 上的 `warning:`，断链的四条另断言页面模型与账本（都不要字体） |

**结论**：**真跑 3 个 + 混合 5 个 + 软跳过 8 个 = 表里的 16 个**测试文件；树上的第 17 个
`reference_plots_ink.rs` 不在这个数里，见下面那一段。

真跑的三个文件是 `paper_model_golden`（7 页整棵模型）、
`refdata_archive_corpus`（4 个用例）与 `export_ledger_fixtures`（7 页的
五层守恒 + 交叉核对 + 确定性与路径扫描 + 基线比对）。混合的五个文件各有一部分用例真跑：
`border_template` 真跑两条 A2 建模段（A0/A1 软跳过）、`write_five_drawings`
真跑三条纯单测（落盘那条跳过）、`reference_plots_acceptance` 真跑
`only_the_surveyed_sheets_carry_furniture_masks`（`eleven_reference_plots`
跳过）、`export_every_fixture` 真跑走查与模板表两条断言（导出段跳过）、
`cli_pid_plot` 真跑 33 条不需要字体的用例（D06 导出跳过）。其余 8 个文件在 CI 上
一条出图断言都不执行，因为它们读的语料（验收数据、备份语料、字体）都不在仓库里。

**2026-11-11 新增的第 17 个文件 `reference_plots_ink.rs` 不在上面的实测里**，下面是照代码
定的，没有在干净检出里量过：它是**混合**——`every_known_defect_is_one_reading_with_a_cause`
只读表里的常量，到处都跑；`eleven_reference_plots_print_the_ink_their_pins_say` 在
`PID_ACCEPTANCE_CORPUS` 的第一张参考图不在时打印 `skipping the ink gate; … is absent` 后
早退，一条页面断言都不跑；另三条读语料的用例（A2 图的盒装标签、标签与原点标记的先后、
放置从哪里画）同样在语料不在时各打印一行原因后早退。下次重测整张表时把它量进去，结论那一行届时是 17 个文件、
混合 6 个。

**2026-11-10 改过、没有在干净检出里重测的四行**：`cli_pid_plot`、
`export_every_fixture`、`compare_five_drawings` 与 `export_ledger_fixtures`
是照代码和本机「藏起字体目录」的模拟更新的，不是按上面的方法量的。下次动 CI
覆盖面时连同整张表重测。`cli_pid_plot` 那一行当天是 9 条，是同一天加了两条模板用例
之后在本机量的：`PID_PARSE_SELF_CHECK_DIR` 指向不存在的目录（本机没有
`test-file/fonts`，字体搜索因此落空），10 条全过，stderr 上四行
`no font directory is present` 加一行 `skipping the D06 export`。2026-11-11 加了
四条断链用例之后用同一个办法重量：14 条全过，八行 `<case>: no font directory is
present` 加一行 `skipping the D06 export`，表里的 13 条出自这一次。2026-11-12 合并
main `7f71586`（它带来两条断链用例）并加了三条用例之后，在合并树上再用同一个办法
重量：26 条全过，21 行 `<case>: no font directory is present` 加一行
`skipping the D06 export`，表里的 25 条出自那一次。同日第七轮加了一条（纸张只错在高度上）
之后再量：27 条全过，22 行 `<case>: no font directory is present` 加一行
`skipping the D06 export`，表里的 26 条出自那一次。同日第八轮加了三条符号文件的用例
之后再量：30 条全过，25 行 `<case>: no font directory is present` 加一行
`skipping the D06 export`，表里的 29 条出自那一次。同日第十轮加了两条（一个符号文件两条流都坏、
模板域的放置从坏的库符号画）之后再量：32 条全过，28 行 `<case>: no font directory is present` 加一行
`skipping the D06 export`，表里的 31 条出自那一次；解析线第六轮在自己的分支上加了两条页框被拒的
用例，那边量到的是 32 条全过、29 行。第十一轮把两边并到一起之后再量：34 条全过，32 行
`<case>: no font directory is present` 加一行
`skipping the D06 export`，表里现在的 33 条出自这一次。

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
5 MB 上限之内。同日页面改成图纸写的纸张（`page_mm`），图框自己的矩形另记为 `frame_mm`：
七份各多 4 行，图元一个不变，合计 4 190 258 字节（4.00 MiB）。

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
(16 rows: eleven of the thirteen below plus `igBspCurve2d`,
`igRectangle2d`, `igDimension`, `igSmartFrame2d` and `DependencyObject`.
Four rows were retired on 2026-11-11 — `GLine2d`, `JStyleOverride`,
`SubRecord0x0010`, `AttributeFragment` — for decoding nothing on any
chain; see the caveats).

| Slice | PSM Type | Decoder | DTO | Sigma Class |
|---|---|---|---|---|
| D-E | `0x3FE6` | — (retired; the matcher is on the test side) | — | `GLine2d` (**not** a record family: `0x3FE6` is not a type code, and what the scan matched was the top two bytes of a page frame's aspect ratio) |
| Phase 16 D | `0x0030` | — (retired; the matcher is on the test side) | — | `JStyleOverride` (RAD `style.dll`, **not** an arc; no sheet chain holds one — the class's records are in `StyleCluster`) |
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

Cross-fixture decoded geometry spans the igLine2d / polyline /
point / text / symbol families (Phase 16 reclassified `0x0030`
records as non-geometry `JStyleOverride`; Phase 17 removed the legacy
`PrimitiveArc` compatibility surface; the `GLine2d` lines Phase 14
counted beside them were never records).
All decoders are panic-safe (validated via
`tests/parser_panic_safety.rs` adversarial matrix) and bounds-checked.

Decoded records flow through `streams/cluster.rs` →
`model::SheetGeometry::decoded_{iglines, iglinestrings, igpoints,
igtextboxes, igsymbols, igarcs, igcircles, igboundaries, …}` →
`geometry::build_normalized_geometry`
to emit `PidGraphicEntity { confidence: Decoded, kind: Line | Arc |
Circle | Polyline | Point | Text | SymbolInstance, source:
PidGraphicProvenance { stream_path, byte_range, record_kind,
graphic_oid, note } }`. The `note` carries the byte-level evidence
chain. (A decoded `igBoundary2d` is emitted as the fill ring of its
loops — see the `0x0013` caveat below.)

**Key insight (Slice J discovery)**: Intergraph Sigma uses its IGDS
class tags directly as PSM type codes for standard primitives. All of
a sheet's geometry is standard IGDS records using IGDS class tags as
PSM type codes — the "`GLine2d` (`0x3FE6`)" SmartPlant extension
Phase 14 set beside them does not exist, see the caveats. This
unlocked Slices J–N without needing `radsrvitem.dll` decompilation.

**Caveats**:
- `0x3FE6` is **not a type code**, and `GLine2d` is not a record
  family. The native reader's class table has a slot for each code
  from `0x0000` to `0x0118` and nothing past it; a chain walk stops
  on a word past it. No chain of any stream of the corpora holds a
  record that states `0x3FE6`. What Phase 14 decoded — three "lines"
  across four fixtures, thirteen hits across all the corpora and
  their archives — was found
  by sliding a decoder over the stream a byte at a time: an
  `igSmartFrame2d` states its height over its width at payload
  `+148`, on an ISO A sheet that is `1/√2` give or take the frame's
  margins, every `f64` from 0.6875 up to 0.71875 has `E6 3F` for its
  top two bytes, and the bytes a page frame keeps behind them
  sometimes pass for a unit vector and an ordered pair. Every hit
  sits 160 bytes into a page frame. Gated on the
  record chain from 2026-08-10 (`e5ef8fb`) the family decoded nothing
  on any corpus; once the walk took the class table for its tripwire
  it could decode nothing on any stream. **The family is retired
  (2026-11-11)**: no registry row, no `SheetGeometry` field, no model
  DTO, no emitter, no `decode_primitive_lines`, and no
  `decode_primitive_line_at` in the library: the matcher lives in
  `tests/support/retired_matchers.rs`, for the tests that re-find
  the old hits. `GLine2d` is a real class of the vendor's geometry
  library (`GLine2d::Validate`, the six-double parametric layout);
  it is not a class the reader serializes a record of. See
  `docs/analysis/2026-08-10-gline2d-is-the-iso-page-ratio-not-a-record.md`
  and
  `docs/analysis/2026-11-10-what-the-sixty-unreferenced-pages-leave-undrawn.md`
  §18.
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
  chain the family decoded nothing on any sheet, and the `ProbeOnly`
  `PidGraphicKind::Unknown` entity each hit surfaced as is gone (the
  `Inferred` `Annotation` before it had been withdrawn in August, when
  `style.dll`'s serializer showed payload `+0..15` to be four separate
  four-byte reads). **The sheet family is retired (2026-11-11)**: no
  registry row, no `SheetGeometry` field, no emitter, no
  `decode_jstyle_overrides`, and since the matcher left the library
  no `decode_jstyle_override_at` either: it lives in
  `tests/support/retired_matchers.rs`, for the tests and probes that
  re-find the old hits. See
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
  carries 23 `igBspCurve2d` (the bowls of its wordmark) and two
  sheet-level `igEllipticalArc2d`, direction byte 1; `A0-W.igr` to
  `A3-W.igr`, inside the committed `RefData~4~682.zip`, carry four of
  each in their logo, the arcs with direction byte 0 (no page wears a
  `-W` template; `tests/elliptical_arcs.rs` holds both); both families
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
  2 158 boundaries on the sheet chains of the 86 documents that count
  was taken on (the plant backup, the acceptance sheets, the share, the
  fixture drawings, the two template directories), two on
  `XA-A1.igr`'s oids 5074 and 5076, whose second loop is the counter
  of a logo letter. The anchor is the point the fill was seeded at:
  inside the loop on every one-loop record of those documents that
  carries member references, between the two loops on those two. Not
  among the 86: the templates inside the committed
  `RefData~4~682.zip`, whose sheet chains keep 41 more — the 20 in
  the logos of `A0-W.igr` to `A3-W.igr` are refused by this grammar
  (no page wears them), and the 21 of `CPECCHBA2.igr` and
  `qsmcqtaz13A2.igr` decode with an anchor that is not inside their
  ring. The family emits — it was audit-only until the fills
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
  families decode on the record chain the two decoded nothing on any
  sheet, and **both are retired (2026-11-11)**: no registry rows, no
  `SheetGeometry::decoded_sub_records_0x0010` /
  `decoded_attribute_fragments`, no emitters, no stream entry points.
  What they could still do was make a record of whatever a walk read
  off the chain after a wrong length — a sub-type word `0x0010` passes
  for a type word — and they were the only families that would. The
  two matchers are no longer in the library either; they read at any
  offset from `tests/support/retired_matchers.rs`, which is how
  `examples/probe_what_the_chainless_families_matched.rs` and three
  tests of `parse_real_files.rs` re-find the old hits. Nothing was
  ever reverse-engineered for this
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
  shorter than its header, a type word no record opens with — the
  records the chain read before the stop decode as on any stream and
  nothing after it is read. The walk is not picked up
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
  raises a fault (`tests/sheet_chain_faults.rs`). The style tables
  (`StyleCluster`, the document's and each cache's) are held the same
  way since 2026-11-11, and a table is built from the records that
  walk read (`DocumentStyleTable::from_stylecluster_bytes` walks
  `SheetChain::walk`; it had a walk of its own with no tripwire, which
  read past the stop a warning named). A `.sym` body carries the
  faults of its file (`SymbolGeometry::chain_faults`), and a page says
  them where it draws a placement from that body: one `warning:` a
  file and stream on `pid_plot`'s stderr — the file, how many
  placements are drawn from it, the reader's sentence of the stream —
  and one suspect `symbol_file_chain_fault` item in the export ledger
  (`LayoutLedger::symbol_chain_faults`); the exit code does not
  change. A file no placement is drawn from is not said: a drawing
  caches the body of every symbol it places, the library is read only
  for a placement the document holds no body of, and none of the 71
  pages draws one from the library (the two ledger baselines pin that
  for 60 of them, `reference_plots_ink`'s `PLACEMENTS` for the
  eleven). What such a placement draws is whatever the symbol reader
  took, and that reader has a walk of its own (`read_sheet_chain`: a
  sheet whole or not at all, type words masked to their class, no
  class-table tripwire). Measured on the committed pressure gauge,
  whose `/Sheet6` is its normal view: a length of zero, stray bytes at
  the end or a wrong magic leave the sheet out, the body is then the
  file's other sheet, and the page draws the gauge's circle and a `PG`
  the sound file does not letter — other ink, not less; a type word
  flagged `0x4000` or a header count that is off is read through, and
  the body is the sound file's though the fault states a stop. So the
  line says the file is damaged and that placements are drawn from
  it, and claims neither missing ink nor none
  (`pid-plot/tests/cli_pid_plot.rs`, the five symbol-file cases). A
  file with two damaged streams is two lines and two items. A file
  that a border template's placement is drawn from is said as the
  template's ("in the border template, …") and filed among the
  template's items; it is named as the template names it, which for a
  title-block symbol is a path on the machine it was authored on — such
  a placement is found by its file name, and the library does not say
  which file answered. **One of the streams a body is read from is
  outside all of this: the root `/PSMcluster0` of the `.sym`**, which
  keeps the file's layers and which of them each sheet shows (the
  body is read from that stream, the sheets and the `StyleCluster`,
  and from no other). The parse does not report that stream's chain,
  so a fault in it is not among `SymbolGeometry::chain_faults`, and a
  page drawn from such a file says nothing and files nothing.
  Measured on the committed gauge
  (2026-11-12, the eleventh review's finding, read again here): with
  a length of zero in the middle of that stream, or with its last
  record one byte too long, the page draws two circles where the sound
  file draws one — no warning, no item, no suspect, exit 0; with a
  type word flagged `0x4000` the page is the sound file's, and still
  nothing is said. The chain walk does find all three
  (`StreamChainFault::of_stream`), and the review counted the root
  cluster of all 624 `.sym` files on hand as one sound chain, so the
  fault can be held; that is the parser's to do, in ticket #67, and
  once the fault is in `chain_faults` the warning and the item above
  follow with no change on this side. Until then a damaged symbol
  file is said only when the damage is in a sheet or in its
  `StyleCluster`.
  `sheet_record_starts` keeps the all-or-nothing
  reading for the publish line's row reader and the semantic join,
  which read nothing from such a stream. `pid_publish_xml` says so on
  stderr, for a `.pid` and for each drawing of a backup package: one
  read gives the drawing and the faults
  (`pid_load::read_publish_from_reader`), and each line says what the
  reader is left without in the chain's own numbers: how many live
  records the chain read before the stop, how many of them are of the
  kinds the publish reader keeps — none, for a style table, whose stop
  costs the XML nothing the chain read — and how many more the header
  states (`tests/publish_chain_faults.rs`). The line is the framing
  sentence and that consequence; "what the stream draws may be
  incomplete" is a sheet's consequence and is not said of a stream a
  publisher reads rows from. A package's `_Index.json` lists the same
  sentence among its `problems` — a list that already held scan and
  read failures; such an entry stands beside a drawing that is written
  and counted as written, so `failed` does not move, and the exit code,
  which is `failed` and the gate, does not either. An intact package's
  index is byte for byte what it was. `_Coverage.json` does not carry
  the faults.
- **A length that is wrong and still inside the stream** sends the
  walk on from an offset where no record starts, so "the records the
  chain read" are the file's only as far as the lengths were right.
  The walk stops on a type word no record opens with
  (`psm_type_word_opens_a_record`, `SheetChainStop::NoSuchClass`): a
  type code past the native reader's class table (`radsrvitem.dll`
  keeps 281 slots, `0x0000`–`0x0118`, `0x00F8` empty), the `0x4000`
  bit (`PSMSerializeIn` reports it as a stream offset mismatch), or
  type code zero. No record of the 499 289 on the corpora's chain
  streams fails it. Changing one length by ±1 … ±64 on every record
  of the corpora's sheets, 620 049 of 2 106 074 changes used to read a
  record from where none starts and 41 read as a sound chain; now
  35 074 and none, and on 98.3% the stop is the first offset past the
  changed record. The rule is the class table's, not the list of codes
  the corpora carry: that list catches more and would stop a sound
  drawing at its first `igBalloon`. A walk that lands back on the
  chain and reaches the end is given away by the header's count alone,
  and that fault names no place. The class table was read from one
  build, `radsrvitem.dll` 9.0.0.143 (its slots are not in CLSID order;
  what ends it is what follows slot `0x0118`, three doubles and
  pointers): a later build with a class past `0x0118` would write
  records the walk stops on, loudly.
- **What a walk reads off the chain is no page frame.** A walk that
  has left the record boundaries can still land on a word the class
  table has: `0x003D`, a page frame's type word, is also the character
  `=` in a label's text. With one length changed by every amount from
  −64 to +64 on every record (29 120 173 changes, 885 documents),
  walks read 1 371 448 records off the corpora's chains, and 102 of
  them were `igSmartFrame2d` by the decoder's first rules — payloads
  of 65 536 bytes and more, doubles that are denormals or zero. A
  frame states its extent twice, width and height at `+76` / `+84` and
  height over width at `+148`, and the decoder takes a record for a
  frame only when the width and the height are lengths — above zero
  and normal numbers, not the denormals a count reads as — whose
  quotient is a normal number too, and the stated ratio lies within a
  millionth (relative) of it. All 178 frames of the corpora's chains
  agree to 5e-16, and the smallest is a micrometre square; none of the
  102 does, nor any of the 179 places off a chain where a walk could
  read a frame's type word at all, whatever length led it there.
  (Until 2026-11-12 a width of zero skipped the comparison and a
  quotient of infinity passed it: six of the 102 still decoded. A
  frame whose own record states a side of zero is therefore refused
  as well, and listed with the refused graphic records.) The decoder
  says which part of the rule a refused record fails
  (`refused_smartframes`, `smartframe_refusal_at`), the document lists
  the refused frames of its sheets (`PidDocument::refused_smart_frames`),
  and a drawing whose own border frame is one is laid out as a sheet
  with no frame and says so: `pid_plot` prints one `warning:` — the
  stream, the record, the reason, and what the page is without it —
  and the page's account files one suspect `page_frame_refused`
  beside the refused record its parse layer counts
  (`pid-plot/tests/cli_pid_plot.rs`, the two border-frame cases).
  `examples/probe_what_passes_for_a_page_frame_off_the_chain.rs` is
  the measurement; `tests/sheet_chain_faults.rs` holds the same on
  every corpus in reach, the repository's files pinned.
- **Of the other families, one is known to decode such a record.** Put
  to all sixteen families, four of those 1 371 448 records decode as
  `igComplexString2d` and none as anything else. All four are on
  `G井场进站生产收球筒`'s `/Sheet6` in the plant backup: 28 bytes into
  each of two `DependencyObject` records the bytes spell a
  `ComplexString` of 256 members in a payload of 65 536 bytes, and two
  length changes reach each place. That decoder holds a record to a
  member count the payload has room for and a finite scale, which such
  a run meets; the family emits nothing either way. What would tell a
  string from the run is not decoded — a string's trailer is four
  bytes on a sheet and 16, 20 or 36 in a symbol cache — so the four
  stand, as a ceiling `tests/sheet_chain_faults.rs` holds; no record
  read off a chain of the repository's own files decodes as anything.
- `0x0021 ComplexString` and `0x007B Group` are **associations**: a
  member count, a double, and one 14-byte entry per member (oid, range
  box, a word). Neither states a coordinate and neither emits: each
  member is a live record of the same chain that names the association
  as its owner and draws through its own family. The double is the
  unit of the members' range boxes, `range_scale` (`head_double`
  until 2026-11-11): a box is in `range_scale / 32768` m — 1.0 on
  fourteen of the corpora's twenty records, 2.0 on six
  (`CPECCHB-SQ-A1.igr`'s outer frame, five groups of the A0 / A1 `-W`
  templates). **What sets the scale is how far the members reach**:
  it is 1.0 while the far corner of the box that holds them is less
  than a metre from the origin, and 2.0 from one metre to two. A
  sheet's own child list (`0x0114 JSheet`) states the same double by
  the same rule, and the 1 293 such records of the corpora — 1 273
  sheets, 8 strings, 12 groups, each distinct sheet stream once —
  hold it with none against: the far corner is at most 998.0 mm out
  under 1.0 and at least 1 015.9 mm under 2.0. It is not the
  template, the paper, the sheet's own scale, the layer, the owner or
  the size of what is listed: `A1-W.igr` keeps the group of its
  border at 2.0 and the group of its logo at 1.0 on one sheet, and
  one sheet on A1 paper states 1.0 where thirty-nine state 2.0. Nor
  is it whether an `i16` holds each coordinate: an A1 frame ends at
  841 mm and states 2.0 for its diagonal of 1 030. Read off the
  files, not off the vendor's code; a box past two metres has no
  witness. A reader takes the scale as stated, as before; a writer
  would have to set it by this
  (`docs/analysis/2026-11-12-a-range-scale-is-set-by-how-far-the-children-reach.md`,
  `examples/probe_what_decides_the_scale_of_a_range_box.rs`). A `ComplexString`'s curve members are `igLine2d` /
  `igArc2d` with its layer and style id, joined by `0x0019 KeyPoint
  Constraint` members, which have no decoder and need none; a Group
  lists the two `igRectangle2d` of a border or, in `A0-W.igr` to
  `A3-W.igr`, the 37 records of a logo. That a string prints its
  members and nothing else has a vendor plot behind it for three
  strings: the A2 acceptance sheet wears `CPECCHB-SQ-A2.igr`, whose
  frame and title-block borders are `ComplexString`s of four, three
  and two lines, and the vendor's plot draws each member once, in one
  pen width, with no other ink, fill or text within 0.4 mm of it
  (`pid-plot/examples/probe_what_the_reference_prints_where_a_complex_string_is.rs`).
  A string whose members are arcs — `D04`'s, the one the census pins —
  has no plot of its own. See
  `docs/analysis/2026-11-10-what-the-sixty-unreferenced-pages-leave-undrawn.md`
  §10.4 and §16, and
  `docs/analysis/2026-11-11-what-the-plant-backup-census-pins-and-why-none-of-it-draws.md`
  §2.

### Display order: the `0x0114 JSheet` child list

A sheet's records sit in `Sheet<N>` in the order their oids were handed
out, which is **not** the order the sheet draws them in. The drawing
order is the sheet's own list: the `0x0114 JSheet` record of the root
`PSMcluster0` names every record of the sheet stream whose parent is the
sheet, 14 bytes each (oid, a range box of four `i16` in
`scale × 2⁻¹⁵` m, a flag word), in the order the sheet draws them.
(The scale is 1.0 while the far corner of the children's boxes is
less than a metre from the origin and 2.0 beyond — the rule of the
association records, see the caveat above.)
`src/parsers/sheet_display_order.rs` decodes it and ranks every live
record of the stream (`PidDocument::sheet_display_order`, keyed by
`/Sheet<N>`; not serialized, so no JSON or schema output changes):

- a record the sheet owns takes its place in the list;
- a record another record owns is drawn with its owner: it takes the
  place of the listed object it hangs under, and each owner's records
  come behind the owner in the order the owner names them, each followed
  by what it owns itself. A `0x00FA DependencyObject` names its
  dependents among the words of its payload (a label's text boxes, a
  connector's line string, an arrow head); a `0x0021 ComplexString` and
  a `0x007B` Group name their members in the list that is their payload.
  `CPECCHB-SQ-A2.igr` lists its sheet outline first and its inner frame,
  a string of four lines, second, and the A2 reference plot strokes the
  outline, then those four lines as one path, then the title block;
- the border template is drawn where the list has the frame (`0x003D`):
  first in 58 of the 71 files on hand, second on `I井场` (behind one
  valve), 91 to 2 708 places down in seven.

On all 71 files on hand the list names exactly the records the sheet
owns (85 836 children), and nine of the eleven reference plots paint
98.3–99.3 % of their elements in its order (the other two, the ones
that print no border, were not made from the list we hold). `pid-plot`
paints by it: `LayoutLedger::paint_order` beside the model,
`PaperModel::paint_sequence`, `write::written_order`.

**Caveats**:
- Never take oid order or stream order for drawing order: over the
  10 928 records the nine plots paint, 10 429 first appear in the
  list's order, 1 989 in oid order and 1 990 in stream order.
- The list is taken only when it is the sheet's. A list every child of
  which is a live record of the stream whose parent is the sheet is
  taken first; on every stream of the corpora that is the whole list, no
  more and no fewer. A stream read short of its list — its chain stops
  (`SheetChain::walk`), or it was cut off at a record — holds fewer
  records than the list names: the list is then the sheet's when it
  names every record of the sheet's the walk did read, and it ranks
  those. A record before the stop stands where it stands on the whole
  stream; one whose owner lies past the stop keeps the owner's place,
  in stream order among the owner's records that were read
  (`tests/sheet_chain_faults.rs`: 179 of `DWG-0201GP06-01`'s 354 ranked
  records survive a break in the middle, 14 of them under an owner past
  it). A sheet with no such list keeps the old order (strokes, then
  labels, each in model order).
- The `DependencyObject` grammar is not decoded. Its dependents' order
  is read by scanning the payload for their oids at two-byte steps; 176
  of 1 201 groups on the reference plots come out in another order.
- The lists of the definition caches (`JSite*`) are not read: the order
  inside a symbol body is the body's own.
- The 164 bytes behind the entries state the sheet's paper (an `f64`
  height at `+102`, a size code at `+110`, an `f64` width at `+114`:
  0.42 × 0.594, 0.594 × 0.841 or 0.841 × 1.189 m on the 71 files);
  `decode_sheet_paper` reads it into `SheetChildTable::paper` and
  `SheetDisplayOrder::paper`. The eleven reference plots are printed on
  exactly that paper, the A2 sheet included, and **the paper is
  `pid-plot`'s page** (`PaperModel::page_mm`, since 2026-11-11). The
  rectangle of the border frame, which was the page until then, is kept
  beside it (`PaperModel::frame_mm`): it still picks the template, tells
  the drawing's own frame rail, and is what lettering is set against —
  the plotter's grid for a sheet whose frame is not its paper is not
  known, so no label moved. On 46 of the 71 pages on hand the two
  differ by a micrometre or less; on 20 a frame is 0.15 to 2.7 mm off
  its paper and puts its template past the paper's corner, so that the
  top and the right edge of the template's outline lie off the paper
  (and, on the `CPECCHB-SQ-A2` sheets, a line that restates the right
  edge). That ink stays in the model and is not printed: a segment
  whose whole pen is past one edge of the paper is
  `DropReason::SegmentBeyondThePaper`, an intentional drop, and the A2
  reference plot holds no ink there either — four of its 4 586 path
  coordinates are outside its page box, all in the outline's own path.
  That plot shows one template at one offset, and the ledger says which
  of this ink rests on it: `witness::PAPER_EDGE_WITNESSES` lists the
  template placements past a paper's corner that a reference plot shows
  (one row: `CPECCHB-SQ-A2` at (0.388407, 0.565648) mm), and a template
  placed any other way files the same segments under the intentional
  `segment_beyond_the_paper_no_plot_of_template`. On the plant backup
  that is 27 segments on the nine `CPECCHB-SQ-A2` sheets with the
  witness and 12 without it on six sheets — `A井场` on `CPE-SQ-A0` and
  the two `凝析油稳定塔` sheets on `CPE-SQ-A1`, templates whose plots
  show them only on the paper's corner, and `N井场`, `O井场`, `P井场` on
  `CPECCHB-SQ-A1`, which has no plot. A new plot of an offset template
  is a new row of that table, not a change to a list of sheets; the
  acceptance gate holds the table to its corpus both ways round.
  Evidence, the table of what each of the 20 files holds past its
  paper and the 60 unreferenced pages one by one are in
  `docs/analysis/2026-11-11-a-sheet-states-its-paper.md`; the probe is
  `pid-plot/examples/probe_what_a_sheet_holds_past_its_paper`.
- **A paper is the page only when it is the frame's paper.** Nothing in
  the file holds the two doubles of the paper to anything else, and a
  paper read wrong would cut the page to whatever it said. So
  `lay_out_drawing` takes a paper for the page when each side of the
  border frame's rectangle lies within `model::PAPER_FRAME_REACH_MM`
  (2.707 mm) of it. The limit is measured, not chosen: of the 65
  distinct drawings on hand 60 state both, 39 within a micrometre, and
  the farthest is `N井场`, whose frame is 2.456811 mm wider and
  2.706782 mm taller than its paper
  (`pid-plot/examples/probe_how_far_a_frame_is_from_its_paper`, which
  exits non-zero if the limit refuses a file it reads). What that is
  worth: the limit is the sample's largest reading with a fifth of a
  micrometre to spare, and the sample is 13 distinct placements, not 60
  drawings — sheets that wear one template the same way read alike to
  the last bit — four of them frames placed by hand. No reading
  witnesses 2.707 mm as a boundary; it stands at the bottom of the gap
  between the largest reading and the 87 mm of the next ISO size. So a
  sound drawing whose frame was placed a little further off is refused
  too, and gets the page it had before papers were read. A paper that is
  not the frame's is refused: the page is the frame's rectangle — the
  page every drawing had before papers were read, on which the drawing
  is whole as long as the frame holds the sheet (a frame smaller than
  its paper by more than the limit gives a page smaller than the sheet;
  none on hand does) — the ledger files one suspect
  `paper_unlike_the_frame` for the page, and `pid_plot` prints one
  `warning:` naming the drawing, the paper and the frame; the exit code
  does not change. A sheet whose `JSheet` record could not be read has
  no paper either, and its display-order warning says in the same line
  what the page is instead: the border frame's rectangle, or, where no
  frame was read, the size its template's name states. Where a stopped
  chain is why the record was not taken, that line is not started and
  the chain's own line ends with it (below). Five of the 65 drawings
  state a paper and no frame's rectangle, and nothing holds their paper
  to anything: there a stroke of the drawing's own whose whole pen is
  past the page is the suspect `drawing_segment_beyond_the_paper` — the
  writer states the one fact, `segment_beyond_the_paper`, and
  `ExportLedger::assemble` files it by whose segment it is — while a
  label is still never judged by its position. See §10 of the same
  analysis.
- **A template's name refuses no paper.** `PaperModel::frame_mm` is the
  geometry's `page_dimensions_mm`: the rectangle the drawing's border
  frames state when they state one between them, and otherwise the size
  the name of its template stands for (`A2` is 594 × 420) —
  `pid_parse::framed_page_dimensions_mm` is the first of the two and
  `None` for the second, and `ledger::SheetRectangle` keeps which it
  was. A paper is refused for a frame's rectangle only, one reading
  held to another. A name is a label, of the template the drawing was
  started from: a drawing with a paper and a name alone takes the paper
  for its page, and a paper that is not the size the name states — half
  a millimetre or more on a side, `model::PAPER_NAME_REACH_MM`, because
  a name states whole millimetres and the paper is read to the
  nanometre — is said: one suspect `paper_unlike_the_template_name` for
  the page (the 49th code) and one `warning:` with both sizes, that no
  frame was read, and that the page is the paper. **Nothing witnesses
  this path**: none of the 65 drawings states a paper beside a
  template's name alone (three carry a `Template` tag and each has a
  frame), so the rule is held by a copy of `DWG-0201GP06-01.pid` cut
  ahead of its one live frame, and by nothing a vendor printed. Between
  2026-11-12's fifth and seventh rounds such a paper was refused and
  filed as `paper_unlike_the_frame`. §12 of the same analysis.
- An `igTextBox` that states a box letters its line hung from the top
  of the box: `baseline = insertion + box height / 2 − ascent`. A box
  as tall as the line's own cell — 542 of the lettered boxes on hand —
  is the middle line a label was always lettered from; a taller one
  lifts the line by half the difference. `pid-plot` applies it to the
  one class the reference plots letter (`hang_in_the_box` in
  `model.rs`): anchor code `0x48`, one line, FangSong, a box no taller
  than 4.034 mm — 26 labels of three sheets, each 0.173 mm up. The 65
  lettered boxed records outside that class (taller boxes, other anchor
  codes, the fixtures' multi-line notes) are lettered as before and
  listed, drawing by drawing, in
  `docs/analysis/2026-11-11-a-boxed-label-hangs-from-the-top-of-its-box.md`
  for want of a plot that shows where their lines sit. Fifteen of those
  65 have the witnessed box height, 4.034 mm, and another anchor code
  (`0x37` on thirteen labels of `凝析油一级闪蒸分离器B气相出口改造`), so
  labels that look alike on like sheets are lettered one device unit
  apart by a code whose meaning is not known. That is the rule kept to
  what its four witnesses show, not an oversight, and whether it should
  reach them is undecided; §9 of the same analysis.

- A sheet a frame embeds is drawn where the drawing lists the frame. An
  `igSmartFrame2d` record names its site at payload `+156`, and the site's
  storage is `JSite<oid>` (68 of the 68 frame records on the top-level
  sheets of the 71 drawings name a storage of their own file;
  `sheet_display_order::smart_frame_sites`). A linked frame's storage —
  a border's — holds no sheet. An embedded frame's holds the sheet it
  draws, with a `PSMcluster0` of its own whose `JSheet` record lists that
  sheet's objects: `A01.pid`'s frame 64 embeds `/JSite204/Sheet6` (97
  objects, the border and title block) and `Q井场`'s frame 67755 embeds
  `/JSite68432/Sheet6` (eight lines). `PidDocument::
  embedded_sheet_display_order` holds each one's order and the rank of its
  frame, apart from `sheet_display_order` (an embedded sheet states a
  paper of its own), and `pid-plot` paints its primitives at the frame's
  rank in the embedded sheet's own order. **This is the template's rule
  carried over, not a reading**: no drawing with a vendor plot embeds a
  sheet. Until 2026-11-11 those two pages painted that ink last.
- A record that draws and takes no place in the order is said. Its
  primitives are painted after everything that has a place — strokes
  first and lettering after, when no sheet of the page states an order —
  and the page says so three ways: `LayoutLedger::unplaced` (domain,
  stream, reason, records, primitives), one suspect
  `record_without_display_place` item a stream in the export ledger (the
  45th reason code when it was added; the catalog is 51 now, 28 suspect
  and 23 intentional), and one `warning:` line a stream on `pid_plot`'s
  stderr. None of the 71 pages has one
  (`pid-plot/examples/probe_which_records_take_no_place_in_the_display_order`).
  The reason is the parser's: `sheet_display_order_or_why` says which way
  a sheet has no order (`NoDisplayOrder`: no cluster, no `JSheet` record
  of the sheet, no record of the sheet's own in the stream, a table that
  is not of the stream's records) and `PidDocument::
  sheet_without_display_order` keeps it by stream. A stream that holds no
  record of the sheet's own is `UnplacedWhy::NoRecordOfTheSheet`, apart
  from every way of there being no list, because that is what a chain
  leaves that stops ahead of the sheet's first own record: the list
  whole, and the records the walk read hanging under objects it never
  reached (a copy of `DWG-0201GP06-01.pid` whose first record is eight
  bytes too long: one record read, two primitives, the list's 160
  children untouched). **On a stream whose chain fault states a stop
  such records get no line of their own**: the chain's warning is the
  word on them and ends by saying so — how many records and primitives
  take no place, and, for the drawing's own sheet, what the page is for
  want of the paper the list's record states — and the account keeps
  both items. Any other reason is
  said beside a chain fault, and that one is said on a sound stream or
  beside a header whose count is off. See §11 of
  `docs/analysis/2026-11-11-a-sheet-draws-its-children-in-the-order-it-lists-them.md`.
- What CI holds of this line. The order: `tests/sheet_display_order_fixtures.rs`
  and the paint-order goldens of `paper_model_golden.rs` read the
  committed files. Colour, caps, pens and which of a white mark and its
  ink is painted first: `pid-plot`'s unit tests alone — the two
  reference-plot gates and `export_plant_backup` need corpora and soft-skip
  there. The fourth review's fifteen mutations were caught 15 of 15 by
  unit tests, 14 by `reference_plots_ink`, 1 by `reference_plots_acceptance`.
  Two things that review asks to be carried with every reading of the ink
  gate: its twelve "needs data from the site" lines on the A2 sheet rest
  on the plot being of a later revision, which it could not confirm
  independently for eleven of them, and the boxed-label rule rests on
  four labels of one sheet at two heights.
- A label and its origin mark are painted in the order the sheet lists
  them; there is no rule of their own. A label is an `igSymbol2d`
  whose body carries a mark in the paper's colour and an `igTextBox`
  owned by a `DependencyObject` that names the symbol (payload `+16`),
  and the sheet lists the symbol and that owner at places of their own.
  Of the 96 such labels on the eleven reference sheets the owner is
  listed right after the symbol for 49 and ahead of it for 47, and the
  plots paint 95 as listed; of the 90 places where a reference strokes
  in the paper's colour on ink and both sides are ours, the sheet's
  list is in the reference's order at 89. The 96 are every such label
  the drawing on hand and its plot both show. One more is not read:
  `SP=-8℃` on the A2 sheet, at whose mark the plot strokes nothing in
  the paper's colour — it letters the text 16.9 mm away, in the
  instrument cluster a later revision moved — so no order can be read
  for it. And 23 further owners that name a marked symbol hold a point
  and no text, each on the symbol of one of the 96: 120 owners in all.
  Nor do the numbers the records carry decide the order: the oids of a
  label's records, and their places in the stream, say what the plot
  paints for 48 and 56 of the 96. The one label against its
  list, `P70` on `一级闪蒸分离器A`, has a twin on `一级闪蒸分离器B` —
  the same records at the same two places — that is painted as listed,
  so no rule read off the records gives both plots, nothing is redrawn,
  and the ink gate's two lines for it stay "not settled". "The same"
  is to the byte but for numbers of each drawing's own: the label's
  three records differ between the two sheets in their oids, in style
  ids and in the references of the symbol's cached body, 20 bytes in
  all (`tools/display_order/label_records_apart.py`).
  `pid-plot/tests/reference_plots_ink.rs` pins the tallies
  (`MARKED_LABELS`, `AGAINST_ITS_LIST`, `ITS_TWIN`, `NO_LABEL`,
  `NOT_WHERE_ITS_PLOT_HAS_IT`); see
  `docs/analysis/2026-11-12-a-mark-and-its-ink-are-painted-in-the-order-the-sheet-lists-them.md`.

See `docs/analysis/2026-11-11-a-sheet-draws-its-children-in-the-order-it-lists-them.md`
and `examples/probe_what_order_a_sheet_lists_its_children_in.rs`.

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
what it reads: `examples/probe_<the question>.rs` (293 of the 301
tracked root examples carry that prefix; the other eight predate the
convention and keep their names).

`probe_is_the_unpacked_symbol_library_what_the_archives_hold` compares
`test-file/symbols-full`, on a machine that has that directory, with the two
committed `RefData~4~681.zip` the tests now read the library out of: the
members each archive holds, the names both hold and how many of those differ
(603, 12), and, file by file, the directory against the archives read in
order. It read 618 files on each side, the same names, every one identical,
and exits non-zero when they are not.

`probe_what_decides_the_scale_of_a_range_box` reads every record that lists
children with a range box — `JSheet`, `ComplexString`, Group — on every
distinct sheet stream of the corpora and their archives (885 documents, 1 293
records) and sets the scale each states beside what could decide it. It
prints the twenty strings and groups one by one, the sheets by paper and
scale, the records that share a document, a sheet, an owner and a layer and
state two scales (two places), and, for seven measures of where the children
lie, the largest value under 1.0 against the smallest under 2.0: only the
distance from the origin to the far corner separates them (998.0 mm against
1 015.9 mm). It exits non-zero when a record does not state the scale that
distance calls for; an argument lists the records of the documents it names.
About 70 seconds in a debug build.

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
none of `0x0010`), then repeats the old sliding walk through the three
matchers — included from `tests/support/retired_matchers.rs`, where they have
lived since the library stopped exporting them — and files each hit under the
chain record it sits in; it exits non-zero if a sheet chain holds a record of
either code, or a hit turns up that its host does not account for.
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

`probe_what_passes_for_a_page_frame_off_the_chain` asks what a walk that has
left the record boundaries takes for an `igSmartFrame2d`, or for a record of
any family. It changes one length by every amount from −64 to +64 on every
record of every distinct chain stream of the corpora and their archives (885
documents, 29 120 173 changes) and puts each of the 1 371 448 records the walks
read off the chain to all sixteen families and to the three sets of rules the
frame decoder has had: 102 were frames by the first rules, six by the ratio
rule of 2026-11-11, none by the decoder now; four decode as
`igComplexString2d`. It tries the same three at every offset where a walk could
read a frame's type word off a chain (179 places: 83, six, none), and keeps the
tallies of the fourteen changes ±1, ±2 … ±64 (3 337 379 changes, 49 058
records, ten frames by the first rules) and of the repository's own files
apart. Beside them it reads the 178 real frames for their payload lengths (228,
234, 238, 246), their narrowest side (a micrometre) and the ratio each states
against its own height over its width. It exits non-zero if a record read off
the chain decodes as a frame, a live frame is refused, or a frame's ratio is
not that quotient. Run it with `--release`: about two minutes.

`pid-plot/examples/probe_what_the_reference_prints_where_a_complex_string_is`
reads the vendor's plot of the A2 acceptance sheet along each curve member of
the three `ComplexString`s its template holds: the member's length against the
vendor ink that runs along it and the pen widths of that ink, the vendor ink in
the corridor that ours covers and does not, the fill outlines and text there,
and — for the frame, whose lines `read_plot` leaves out of both pages as sheet
edges — the strokes `read_plot_pens` lists. It reads each member drawn once
and nothing else in the corridors.

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
`WholePage::text_off`, every lettering class with its means, whole page and
masked, and each pair the two pages set in different faces, with both faces
and body sizes. It reads 24 pieces, 24 pairs with no label, no label past the
limits on any sheet and no pair in different faces (re-run on 2026-11-12;
the "2 labels" this paragraph kept until then was the count before the
boxed labels moved): the fourteen borrowed runs the gate
counts as `WholePage::borrowed` are NSimSun at 3.387 mm on both pages. Until
2026-11-11 ours were FangSong or SimSun at 3.588 mm and 64 labels were past
the limits; the three `SP:` of the air-cooler sheets left the count when each
run of a record took its own whole-pixel start, the 44 condensed SimSun
labels of the title blocks when a label's `Tm` took the scales the PDF driver
writes (`pid-plot/src/write/text_matrix.rs`), `J K井场`'s `1-1/2"`
beside `FC` when the page was painted in the sheet's display order: two
labels on one baseline painted one after the other read back as one run,
and the reference paints other objects between them; and the last two, the
A2 sheet's boxed `仪表风管线`, when a boxed line was hung from the top of
its box. For the same reason
the pairs with no label went from 12 to 24 — where the reference's order
does put two such labels next to each other, both pages now read them as
one run, which is no single label of the paper table. A machine with no
`nsimsun.ttf` borrows SimSun instead, so there the probe would list those
fourteen again, SimSun against NSimSun (not run on such a machine).

The ink line's probes (2026-11-10 and 11) each settle one question about
what a page prints, and print the numbers the two reference gates rest on.
Root crate: `probe_what_order_a_sheet_lists_its_children_in` (the
`JSheet` child list of every sheet stream given: listed against owned,
where the frame stands, the paper the record states; exits non-zero when
a list is not exactly the sheet's records),
`probe_which_strokes_state_the_paper_colour` (which strokes of a symbol, a
template or a drawing state `0x8000_0000` or white),
`probe_which_text_records_state_a_box` (which `igTextBox` records carry
the kind-2 tail and how tall the box is) and
`probe_does_the_database_place_a_sheet_where_the_pid_does` (an `exp` dump
against the `.pid`). `pid-plot`: `probe_which_paint_leaves_no_ink`,
`probe_which_primitives_paint_the_paper` (also against another build's
dump), `probe_which_strokes_the_reader_drops_as_sheet_edges`,
`probe_what_the_template_masks_hide`, `probe_what_a_page_reads_inside_a_window`,
`probe_is_a_library_gap_drawn_from_the_file`,
`probe_is_the_border_a_reference_lacks_the_one_its_sister_prints`,
`probe_which_device_grid_a_page_is_lettered_on`,
`probe_does_the_unrounded_start_place_the_colon_of_sp` (9 of 9),
`probe_does_a_sheet_print_alone_as_in_a_batch` and
`probe_does_a_page_read_the_same_twice` (the same two plots compared
again and again: one outcome and one fit, since the comparison stopped
summing in hash order). Each one's module docs give its run and what it
read.

Four probes and a kit carry the evidence of the fourth review's round
(2026-11-11). `examples/probe_which_storage_a_frame_names` prints every
`igSmartFrame2d` record, the storage it names at `+156` and what that storage
holds: over the 71 drawings, 68 of 68 top-level frames name a storage of their
file, two of them embedded, each embedding one listed sheet.
`pid-plot/examples/probe_which_records_take_no_place_in_the_display_order`
prints, a page a line, the primitives of the drawing's own sheets, of an
embedded sheet and of the template, and the records no list places, and exits
non-zero if a page has one: none of the 71 does.
`pid-plot/examples/probe_which_pen_a_declared_width_prints_on` reads the pen
rule off the reference plots — every straight piece our model strokes against
the reference stroke at its place, strokes along the sheet included: 19 393
pieces, nine declared widths, every piece on the width's whole device units.
`pid-plot/examples/probe_which_white_strokes_of_the_reference_ours_lacks`
lists the reference's white stroked pieces with no white of ours within a
reach (`PID_WHITE_NEAR_MM`, 0.3 mm): 462 on the references, 261 on ours; at
0.25 mm and over the only ones without are one mark of the A2 sheet's moved
cluster, and what a shorter reach adds is the one device unit the vendor's
driver strokes past the end of an origin mark's arm. `tools/display_order/`
holds the Python the display-order record's tables were read with
(`order_check.py` and the rest); `pid-plot/examples/dump_paper_model` writes
the page models they read.
`pid-plot/examples/probe_which_of_a_mark_and_its_ink_a_plot_paints_first`
lists, for the eleven reference sheets, every place a reference strokes in the
paper's colour on ink — what the ink is in our model (the label's own text, or
anything else), which of the two each page paints first and where the sheet
lists them — and every label whose symbol carries a mark, with the places of
its symbol and of its text's owner and the steps the plot paints them at: 91
places, 96 labels, 95 painted as listed, and the twin of the one that is not.
It also lists every owner of a marked symbol that gave no label, with why — 24:
23 hold a point and no text, one is a label the plot does not have where the
drawing puts it — and says for how many of the 96 labels the sheet's list, the
oids of the label's records and their places in the stream each say what the
plot paints (95, 48 and 56).
`tools/display_order/plot_and_file_dates.py` reads when each plot was made and
each drawing last saved (the plots are of 2026-10-12 and -14, the drawings of
2026-06-22 to 07-06; it needs `olefile`).
`tools/display_order/label_records_apart.py` prints where the three records of
one label — its symbol, the owner of its text, the text — differ between two
drawings that both hold it, byte for byte (`olefile` again, nothing of the
crate): for `P70` on the two flash-separator sheets, 20 bytes, every one part
of an oid, a style id or a reference of the cached body.

`pid-plot/examples/probe_what_a_sheet_holds_past_its_paper` prints, for each
drawing it is given, the paper its sheet states, the page the layout writes,
where the border frame puts the template, and every primitive that reaches
past the paper — whose it is, how far past which edge, and which of a stroke's
pieces lie wholly off it. It takes the corpus the drawings are laid out
against, as their gate does. Over the 71 pages it reads 20 files whose frame
is 0.15 to 2.7 mm off its paper; on 18 of them what lies past the paper is the
template's sheet outline (its top and right pieces) and, on the eleven that
wear `CPECCHB-SQ-A2`, the line that restates its right edge; the one primitive
of a drawing's own is `N井场`'s sideways label.

A `tmp_`-prefixed example is the same tool before its answer is worth
anything: `examples/tmp_<subject>.rs` is gitignored, nothing cites it,
and it disappears once the answer lands. When the numbers do turn
out to matter, rename it to `probe_<the question>.rs` and commit it —
that promotion is what lets a later session re-run the evidence.
`pid-plot/examples/` follows the same naming; the ignore pattern covers
only the root directory, so keep scratch there out of commits by hand.

## Common commands

```bash
cargo test                                        # the root crate: 1 874 pass here, 15 of them with a check unmade — see "Soft skips"
cargo test --test publish_xml_cli -- --nocapture   # CLI integration
cd vendor/oxidized-mdf && cargo test --lib         # vendored unit tests (37 tests)
```

Give every extra checkout of this repo (a `git worktree`, a scratch
copy) its own `CARGO_TARGET_DIR`. Cargo keys a workspace member by its
path relative to the workspace root, so checkouts sharing a target
directory share build fingerprints: a build in one checkout made after
the other's last source edit looks up to date to the other, whose tests
then run the first checkout's code without recompiling. If that has
already happened, update the modification time of `src/lib.rs` in the
checkout under test to force a rebuild.

Putting a file back the way it was can bring its old modification time
back with it: copying a saved copy over it (`Copy-Item`, `cp -p`) or
moving a backup into place does. Cargo decides whether to rebuild by
modification time, so it then finds every source older than its last
build and runs the executable it built from the *changed* file as up to
date — a test broken on purpose stays red after the file is restored, or
a result is reported for code the tree no longer holds. After restoring a
file, set its modification time to the present before building —
`(Get-Item <file>).LastWriteTime = Get-Date`, or `touch <file>` — or
restore it by writing it (an editor save, `git checkout -- <file>`), which
stamps it now.

A `cargo` that warns "ignoring the configured target directory … this
cargo always uses `./target`" (a fork, tagged as such by `--version`)
keeps every checkout's intermediate artifacts and test executables in
one directory under `CARGO_HOME\targets` regardless — the sharing above,
with no variable that opts out. Run the toolchain's own cargo there,
`rustup run nightly cargo …`, which honours `CARGO_TARGET_DIR`.

A `pid-plot` test that needs files of its own makes them in a `ScratchDir`
(`pid-plot/src/scratch.rs`; the integration tests include the same file by
path): a directory under the temporary directory named
`pid-plot-<name>-<process id>-<count>`, removed when the guard is dropped —
when the test ends, passed or failed. A whole `cargo test -p pid-plot` leaves
no `pid-plot-*` entry in `TEMP` (it left eight a run before 2026-11-11), so
do not join a fixed name onto `std::env::temp_dir()` in a new test.

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

`cargo fmt --all` hands rustfmt the root file of every target, as
`\\?\`-prefixed absolute paths, in one command line for each edition: 361
files for edition 2021 (the root crate's lib, 6 bins, 301 examples, 49
tests and a bench, and the three of `vendor/oxidized-mdf`) and 98 for
edition 2024 (`pid-plot`: its lib, 7 bins, 73 examples, 17 tests; counted
on the tree on 2026-11-12, after the parse line's sixth round and the
soft-skip round were merged). Under
Windows a command line may not pass 32 767 characters, and the 2021 line
grows by 361 characters for every character of the checkout's path, so
whether the command runs depends on where the checkout is. Measured with
rustfmt 1.10.0-stable on `e9ebba1` (the seventh review) and again on
`735de61`, after the merge of main (the ninth, 2026-11-12), when the line
held 357 files: a checkout whose path was 50 characters or shorter ran it
and from 51 on cargo-fmt failed before rustfmt read a file (`os error
206`). **Four files more and the limit is 49**: with two probes and two
tests added the same day, the command line `cargo fmt --all -v` prints is
33 375 characters in a 51-character checkout (measured on the merged tree,
where the command fails with `os error 206`; an independent review read
30 848 in a 44-character one, the same line less 361 a character), which
makes it 32 653 at 49 and 33 014 at 50 — 247 past the limit. (The 49 and
the 50 are reckoned, not run.) The main checkout (32 characters) runs it,
with 17 characters of path to spare — the line is about 26 500 characters
there — or some 60 more root-crate examples, each of which costs its
path, its name and about 15
characters. Worktrees under `pid-parse-wt\accept-evidence` (51) and
`pid-parse-wt\publish-export` (50) cannot run it; one under
`pid-parse-wt\lettering-8` (47) can. The `pid-plot` line (98 files, about 10 400 characters
even in a 51-character checkout) is nowhere
near it, so `cargo fmt -p pid-plot` runs anywhere. Linux, and so CI, has
no such limit. Where it bites, check the tree out under a shorter path,
or run rustfmt over the same roots in pieces: each package's `src/lib.rs`,
`src/bin/*.rs`, `examples/*.rs`, `tests/*.rs` and `benches/*.rs` with that
package's edition (2021 for the root crate and `vendor/oxidized-mdf`, 2024
for `pid-plot`), `rustfmt --check --edition <edition> <sixty files>`.

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

- `--workspace` does not reach the vendored `oxidized-mdf` crate's own
  tests: the root `Cargo.toml` lists `vendor/oxidized-mdf` under
  `[workspace] exclude`, so the commands above only compile it as a
  dependency of `pid-parse`. Run its unit tests on their own, from its
  directory: `cd vendor/oxidized-mdf && cargo test --lib`.
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
