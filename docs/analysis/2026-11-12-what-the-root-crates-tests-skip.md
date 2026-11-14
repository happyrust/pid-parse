# 根 crate 的测试跳过了什么

日期：2026-11-12　分支：`tests-skips-20261112`（基点 `396593c`）　工单：#54

根 crate 有一批测试在数据不在时打印一行就通过（软跳过）。`cargo test` 不显示通过的测试打印的东西，所以
一次全绿的运行说不出其中有多少条其实没有断言。这一篇把它们逐条列清：哪条测试、缺什么、本机别处有没有；
靠现有数据能真跑的改成真跑；剩下的列给 #62。

## 结论

| | `396593c` | 现在 |
|---|---:|---:|
| **本机**（有两份 `pid-test-data`、共享语料、解包的 `symbols-full`）：没有做完断言就通过的测试 | 16 | 15 |
| 　其中一个字都不说的 | 1 | 0 |
| 　以 `skipping: ` 开头的行 | 14 | 21 |
| **只有仓库**（模拟 CI，见第一节）：没有做完断言就通过的测试 | 57 | 45 |
| 　其中打印 `skipping: ` 的 | 17 | 45 |
| 　其中换了别的写法的（`skip: …`、`skipping …`、`not on this machine: …`） | 30 | 0 |
| 　其中一个字都不说的 | 10 | 0 |
| 　以 `skipping: ` 开头的行 | 17 | 55（真正的 CI 上是 53，见 1.3） |

1. **12 条测试改成了真跑**（第三节）。它们读的是厂里的整套符号库，原来从 `test-file/symbols-full` 读——
   这个目录被 gitignore，只有解包过的机器才有——不在时 10 条一个字不说就通过。整套库其实在入库的两个
   压缩包里，逐字节相同；现在从压缩包读，哪里检出都真跑。
2. **其中一条从来没有跑过，而且它的断言不成立**（第四节）。`a_symbol_carries_the_colour_and_width_its_own_style_table_states`
   找的文件不在它找的目录里，在任何机器上都是静默通过；改成真跑的第一次就红在第一条笔画上。文件里的事实
   与它断言的不同，已经按文件改正并加严。
3. **剩下的 45 条全是缺仓库里没有的数据**，靠现有的东西改不了（第五、六节，#62 的清单）。其中 13 条
   （DWG 镜像门禁）在本机也跑不了：它们要的那个 MDF 从来没有存在过。
4. **跳过只有一种写法，数得出来。** 一行，以 `skipping: ` 开头。别的两种写法 58 处都改了，
   `tests/soft_skip_spelling.rs` 守着；`python tools/soft_skips.py --run` 带输出跑根 crate 的测试并数出来。
   没有放宽任何断言，没有一条从「跳过」变成「静默通过」。

## 一、怎么量的

### 1.1 带输出跑，数行

`cargo test -p pid-parse --lib --bins --tests --examples -- --nocapture`，数以 `skipping: ` 开头的行。
这只数得到「说了的」。

### 1.2 找不出声的：让每个提前退出都说话

一次性的做法（脚本在 `target\tickets-54-55\scripts\`，不入库）：把测试代码里每个裸的 `return;`、
`=> return,`，以及 `let … else {` 里的、紧跟在 `.exists()` / `.is_file()` / `.is_dir()` 之后的 `continue;`
改写成先打印一行记号再退出（`396593c` 上 361 个 `return`、245 个 `continue`，行号不变），单线程带输出跑，这样每行输出
都归得到一条测试。同一份插桩跑两遍：保留本机的语料，和拿掉它们（1.3）。**只在拿掉语料时才触发的退出**
就是因为缺数据而没做完的断言；那条测试没有打印 `skipping: ` 行，就是不出声或者换了写法。数据循环里
正常的 `continue`（一条记录不是要找的那种）两遍都触发，不算。

`396593c` 上这样量出来：32 条测试只在缺语料时提前退出、又不打印 `skipping: ` 行（24 条换了写法，8 条
不出声），另有 5 条没有提前退出、只是少读了东西而换了写法。改过之后同样量：**0 条**。

量不到的是「条件体」——`if path.exists() { 断言 }`，没有 `else`。这种逐个看过
（`rg "\.(exists|is_file|is_dir)\(\)"` 的 214 处里不带取反的 43 处），是跳过的只有一处：`parse_real_files.rs` 里读
`arrester breather valve(RD).sym` 的那一段，也已改成真跑。

### 1.3 「只有仓库」是模拟的

没有新开检出（磁盘紧），在这个 worktree 上模拟：三个环境变量（`PID_TEST_DATA_DIR`、
`PID_PARSE_SELF_CHECK_DIR`、`PID_ACCEPTANCE_CORPUS`）指向不存在的目录；测试代码里写死的 34 处 `D:\work\…`
临时改名；这个 worktree 自己的 `test-file\symbols-full` 联接临时拿掉（worktree 里别的都是入库文件）。
跑完全部还原。**没有在 GitHub 的 runner 上量过**；Linux 上那些 `D:\…` 路径同样不存在，走的应当是同一些
分支。模拟时设了环境变量，`sheet_chain_faults` 因此多打两行（它把环境变量点名的目录也当成一处语料）；
真正的 CI 不设这些变量，所以是 53 行而不是 55 行——这是推算，不是读数。

## 二、本机也跳过的 15 条

| 测试 | 缺什么 | 本机别处有没有 |
|---|---|---|
| `publish_dwg_generated_parity.rs`：`generated_dwg_interface_parity_matches_reference_superset_when_mirror_available`、`generated_dwg_attribute_parity_…`、`generated_dwg_pid_branch_point_shape_…`、`generated_dwg_piping_branch_point_shape_…` | `test-file/backup-test/DWG-0202GP06-01_p/extracted/Export.mdf`：P01 厂（`DWG-0202GP06-01` 所在的厂）的 SQL Server 数据文件 | **没有，而且做不出来**（下） |
| `publish_dwg_mirror.rs`：`dwg_mirror_end_to_end_meta_xml_agrees_with_reference_identifiers`、`dwg_mirror_end_to_end_data_xml_follows_dwg_style_on_pipeline`、`dwg_mirror_emits_pid_branch_point_matching_reference_count`、`dwg_mirror_emits_piping_branch_point_matching_reference_count` | 同上 | 同上 |
| `publish_meta_parity.rs`：`generated_dwg_meta_matches_reference_summary_when_mirror_available`、`generated_dwg_meta_is_byte_stable_across_repeated_emits` | 同上 | 同上 |
| `publish_rel_parity.rs`：`rel_defuid_parity_on_dwg_writer_matches_reference_supersets_when_mirror_available`、`a36_piping_end1_conn_uid2_is_real_upstream_on_dwg_when_available`、`a36b_every_rel_uid2_resolves_within_the_document_on_dwg_when_available` | 同上 | 同上 |
| `parse_real_files.rs`：`controlled_pid_diff_pairs_report_stream_level_evidence_when_available` | `test-file/controlled-diff/{before,after}/<case>.pid` 与 `metadata/<case>.json`：按采集协议做出来的「只改一处」的图纸对 | 没有。三个目录在这里和主检出里都只有 `.gitkeep`；`.pid` 按约定不入库（厂里的数据），协议是 `docs/protocols/2026-05-13-controlled-pid-diff-collection.md` |
| `publish_interface_parity.rs`：`interface_parity_on_a01_writer_matches_reference_superset_post_a22`（**部分**：7 个标签不比） | A01 的参考 XML 里没有 `PIDBranchPoint`、`PIDControlSystemFunction`、`PIDNote`、`PIDPipingBranchPoint`、`PIDPipingComponent`、`PIDSignalConnector`、`PIDSignalPort` 这 7 种标签；它们在 DWG 的参考 XML 里 | 参考 XML 在（`test-file/export-test/publish-data/DWG-0202GP06-01/`），能生成 DWG 那一侧的只有上面那 13 条，所以这 7 种标签现在没有一条测试在比 |

**DWG 的 MDF 为什么做不出来。** 入库的 `DWG-0202GP06-01_p/Export.dmp` 开头是 `03 03 69`——Oracle `exp`
导出，不是 SQL Server 的 MTF 磁带；它从 2026-04-28 入库起就是这一个文件，`extracted/Export.mdf` 在 git
历史里从来没有出现过。数据库这条线（`mdf_load` → `sqlite_load`）只读 MDF / SQLite。在本机 `D:\work\cad` 与
`D:\work\plant-code\cad` 下把每个 `*.mdf`、`*.bak`、`Export.dmp` 都打开找过（1 709 个）：含有这张图的 UID
`9D1F3F232C47409D8C54EFB22FC3682A` 的只有 19 份 Oracle 导出（同一个文件在各个检出里的副本），没有一个 MDF
或 MTF。所以这 13 条从写下来那天起没有在任何地方跑过（`tests/common/mod.rs`：「when the MDF fixture
lands…」）。

`#[ignore]` 的 3 条不算跳过，是有意不随门禁跑的：`the_extended_batch_never_panics_hangs_or_overallocates`
（加长的随机批）、`connector_uid_reverse_engineering_probe_logs_current_candidates_against_reference` 与
`raw_residual_source_probe_scans_full_mdf_and_confirms_synthetic_slots_are_absent`（两条是探针）。

## 三、改成真跑的 12 条

### 3.1 数据在哪里

`test-file/symbols-full` 是厂里的符号库解包出来的目录，618 个 `.sym`，被 gitignore。同一套库入了库，是包着的：

| 来源 | `.sym` | 与 `symbols-full` 的关系 |
|---|---:|---|
| `test-file/backup-test/DWG-0202GP06-01_p/RefData~4~681.zip` | 616 | 616 个名字都在，**616 个逐字节相同** |
| `test-file/backup-test/TEST02_p/RefData~4~681.zip` | 605 | 603 个名字与上一个包重合（其中 12 个内容不同，`symbols-full` 里是上一个包的那一份）；另外 2 个只有它有：`sewage pool.sym`、`Cone Roof Parametric Tank.sym`，与 `symbols-full` 里的逐字节相同 |
| 两个包，先前一个、名字不在才用后一个 | 618 | **就是 `symbols-full`**：名字相同，618 个逐字节相同 |

`examples/probe_is_the_unpacked_symbol_library_what_the_archives_hold.rs` 逐文件比过（有这个目录的机器上
退出码 0；两边不一致非零）。夹具 `D06.pid` 放了 `Cone Roof Parametric Tank`，所以第二个包不能省。

读法在 `tests/support/library_symbols.rs`（集成测试）和 `src/symbol_library.rs` 测试模块的
`library_symbol`（单测，只用得到第一个包）：有 `refdata-archive` 特性时读压缩包；没有这个特性的构建
（`--no-default-features`）打不开压缩包，退回去读解包的目录，目录也没有就打印 `skipping: ` 行。

### 3.2 先后对比

「改前」是 `396593c` 的测试代码，「改后」是现在的；都在没有 `symbols-full` 的状态下跑（拿掉联接）。
第四、五列是把每条测试里的一个钉值故意改坏之后的结果（12 处，两边改的是同一行）：改坏了还通过，说明
断言根本没有执行。

| 测试 | 改前，没有 `symbols-full` | 改后 | 改坏钉值，改前 | 改坏钉值，改后 |
|---|---|---|---|---|
| `symbol_library::tests::an_arc_carries_the_direction_its_trailing_byte_states` | 不出声，提前返回 | 真跑 | 通过 | 失败 |
| `…::a_symbol_carries_the_colour_and_width_its_own_style_table_states` | 不出声，提前返回（**有这个目录也一样**，见第四节） | 真跑 | 通过 | 失败 |
| `…::a_symbol_carries_the_dash_its_own_style_table_states` | 不出声，提前返回 | 真跑 | 通过 | 失败 |
| `…::a_symbols_lettering_comes_off_its_own_style_table` | 不出声，提前返回 | 真跑 | 通过 | 失败 |
| `…::a_symbols_override_switches_off_the_dash_its_base_names` | 不出声，提前返回 | 真跑 | 通过 | 失败 |
| `symbol_library_text.rs::labels_the_reader_used_to_refuse_read_off_the_bundled_library` | 不出声，4 个标签全部跳过 | 真跑，4 个标签 | 通过 | 失败 |
| `symbol_library_text.rs::the_bundled_library_refuses_no_text_record_but_the_empty_ones` | 不出声，提前返回 | 真跑：618 个符号、1 265 条文字记录、拒收 0 | 通过 | 失败 |
| `symbol_library_sheet_pick.rs::a_solenoid_valve_draws_its_body_beside_its_s` | 不出声，4 个阀全部跳过 | 真跑，4 个 | 通过 | 失败 |
| `symbol_library_sheet_pick.rs::a_slope_label_keeps_its_lettering_beside_its_strokes` | 不出声，4 个标签全部跳过 | 真跑，4 个 | 通过 | 失败 |
| `parse_real_files.rs::rectangles_own_their_edges_and_the_bspline_reaches_its_body` | 后一半（库里的那条 B 样条）不出声地不做 | 整条真跑 | 通过 | 失败 |
| `parse_real_files.rs::a_cached_body_carries_the_stroke_styles_its_own_storage_states` | 第 5 项（对照库的样式表）不做，打印 `…: skipping the library comparison: no test-file/symbols-full` 5 行 | 真跑：5 张夹具分别对照 17 / 11 / 6 / 4 / 2 个放置的符号 | 通过 | 失败 |
| `parse_real_files.rs::a_cached_arc_sweeps_clockwise_from_its_start_angle_to_its_end_angle` | `Remarks` 云线那一段不做，打印 `skipping the Remarks cloud: …` | 整条真跑 | 通过 | 失败 |

改后那一列在有、没有 `symbols-full` 两种状态下读数相同。本机有这个目录时，改前除第二条外的 11 条是真跑的；
它们读到的字节与现在从压缩包读到的相同（3.1），所以断言没有变，变的只是哪里能跑。

### 3.3 顺带加严的两处

- `the_bundled_library_refuses_no_text_record_but_the_empty_ones` 原来只断言「拒收的文字记录不超过 2 条」：
  走查一个文件都没找到、或者某个文件读不出来（原来是 `continue`），它照样通过。现在另外断言读到的正好是
  618 个符号、没有一个读不出来，并打印读数。上限 2 没有动（现在的读数是 0）。
- 改正后的 `ElecTraceLine` 那一条，见下。

## 四、一条从没跑过、断言也不成立的测试

`a_symbol_carries_the_colour_and_width_its_own_style_table_states`（`e2f01d7`，2026-08-24）说：
`ElecTraceLine.sym` 的样式表只定义一种线型，`#0000FF`、0.5 mm，它的十条笔画都指向这一种，「所以伴热那一行
画成蓝色」。它从 `test-file/symbols/Design/Annotation/Graphics/` 找这个文件——那是入库的三个样本符号所在的
目录，从来没有过这个文件——找不到就不出声地返回。所以它在任何机器上都没有执行过一条断言。

从压缩包里读到这个文件之后，它第一次运行就红了：第一条笔画是黑色 0.13 mm
（`target\tickets-54-55\logs\elec-first-run-fails.log`）。本机能找到的这个文件只有一个版本（两个入库的包、
`symbols-full`、别的检出里的副本，哈希相同），所以不是文件换过。文件里是这样的：

| `StyleCluster` 的记录 | 内容 |
|---|---|
| 样式 1，`JStyleSimpleLine`，名字 `Normal` | 0.35 mm，黑 |
| 样式 7，`JStyleSimpleLine`，名字 `As Drawn` | 0.5 mm，`#0000FF` |
| 样式 8，`JStyleOverride`（被取代的旧副本，类型字 `0x8030`） | 基样式 1，掩码 `0x02`（只说线宽），0.01 mm |
| 样式 9，`JStyleOverride` | 基样式 1，掩码 `0x02`，**0.13 mm** |

两张图纸流上的 12 条笔画记录（去重后 10 条：8 条线、2 条弧）的样式号全是 **9**：`Normal` 的一个只改线宽的
覆盖。所以这个符号体是**黑色、0.13 mm**。`As Drawn`（蓝、0.5 mm）在表里，没有一条笔画指向它——整套库的
616 个符号里有 586 个带着同样的这一条，没有一个符号有活的记录指向它：它是库的样板样式，不是哪个符号的笔。

「伴热那一行画成蓝色」本身是对的，原因不在这里：`DWG-0202GP06-01` 的纸面金样里这个符号的笔画是
`rgb [0, 0, 255]`、`width_mm 0.13`——蓝色来自**放置记录**点名的样式（图纸自己的 `As Drawn`，
`pid-plot/src/model.rs::body_stroke`：放置的样式给整个体上色；它是 `As Drawn` 时不给笔，体自己的线宽留着），
0.13 mm 才是符号体自己说的。这条测试写下之后五个小时，「放置记录不点名样式」的说法就被
`docs/analysis/2026-08-24-placement-names-the-body-style.md`（`9b7a896`）推翻了，`PrimitiveStyle` 的文档也是按后者写的；
只有这条从没跑过的测试还留着旧说法。

改正后的测试断言文件里的事实，比原来多：样式表里恰好这两种线型（按名字、颜色、线宽）；样式 9 是 `Normal`
的覆盖、只说线宽；十条笔画的笔正是样式 9 解析出来的那一支（黑、0.13 mm、实线、不是纸色），并且表里没有
别的样式解析出同一支笔。原来的三个断言（10 条、颜色、线宽、实线）一个没少，颜色与线宽的期望值从
（蓝，0.5）改成了（黑，0.13）——这不是放宽，是原来的值从来没有被任何一次运行检验过，而文件说的是另一个值。

整套库照现在的读法扫了一遍作旁证（616 个符号、7 062 条笔画）：457 个符号有黑色 0.13 mm 的笔画，是最常见
的一种；5 221 条记录直接指向线型，6 070 条经过覆盖。

## 五、只有仓库时才跳过的另外 30 条

本机都跑。按缺的数据分：

### 5.1 共享语料 `D:\work\cad\PID自研软件验证`（`PID_PARSE_SELF_CHECK_DIR`）

5 张图（`.pid` 8.6 MB）、各自的 `_Data.xml` / `_Meta.xml`、`publish\`（2.2 MB）、`Ref\`（650 个文件，
32.8 MB）。

| 测试 | 具体缺什么 |
|---|---|
| `publish_from_pid.rs::the_pid_alone_export_matches_the_reference_wherever_it_says_anything`、`the_cli_delivers_a_drawings_three_files_into_out_dir`、`diff_against_a_pid_exits_zero_when_nothing_emitted_is_wrong` | 一张 `<名字>.pid` 和它旁边的 `<名字>_Data.xml` |
| `publish_from_pid.rs::ijingchang_oracle_exp_matches_the_pinned_reference`、`diff_against_oracle_exp_exits_zero_for_ijingchang` | `I井场 注采阀组工艺及自控流程图.pid` 与参考 XML，加 5.2 的 `Export.dmp` 和 `RefData~4~804.zip` |
| `publish_diff_accepted_baseline.rs::the_ijingchang_run_never_leaves_the_registered_item`、`the_ijingchang_alias_stem_reproduces_the_registered_item` | `I井场 ….pid`，加 5.2 的 `Export.dmp` |
| `publish::oracle_exp::tests::ijingchang_ports_match_the_reference`、`ijingchang_connectivity_ends_match_the_reference_pairs`、`ijingchang_nozzle_and_note_rows_match_the_reference`、`ijingchang_opc_points_match_the_reference` | `I井场 ….pid`，加 5.2 的 `Export.dmp` |
| `publish::oracle_exp::tests::equipment_and_instrument_rows_are_read_from_the_dump_only` | `publish\I井场 …_Data.xml`，加 5.2 的 `Export.dmp` |
| `symbol_library::tests::a_wellhead_tree_keeps_its_body_off_the_single_stroke_stub` | `Ref\Symbols\Equipment\Other Equipment\Wellhead Christmas Tree 02.sym`（81 920 字节）。入库的两个符号包里都没有这个文件 |

### 5.2 备份包 `D:\work\cad\pid-test-data`（`PID_TEST_DATA_DIR`）

53 张图的厂。`Export.dmp`（Oracle `exp`，71.0 MB）、`PlantData~2~711.zip`（20.4 MB，53 张图）、
`RefData~*.zip`；`D:\work\plant-code\cad\pid-test-data` 是同一个包，另有解包出来的
`backup-plant-data\`（53 个 `.pid`，97.2 MB）。

| 测试 | 具体缺什么 |
|---|---|
| `publish::oracle_exp::tests::exp_tables_reads_the_backup_dump_once` | `Export.dmp` |
| `publish_from_pid.rs::backup_directory_publishes_the_whole_package` | 整个包目录 |
| `publish_diff_accepted_baseline.rs::a_drawing_absent_from_the_dump_is_refused_not_silently_published` | `Export.dmp` 与 `RefData~4~804.zip` |
| `publish::catalog_index::tests::real_refdata_zips_parse_with_expected_counts`、`real_refdata_zips_cross_check_against_the_publish_catalog`（**部分**：两个包里少读一个，入库的 `TEST02_p` 那个照读） | 这个包的 `RefData~4~681.zip`（5.3 MB；与入库的两个 `~681` 都不同：581 个条目对 595 个） |
| `parse_real_files.rs::d04_igsymbol2d_placements_are_no_longer_refused` | `backup-plant-data\…\D04.pid` |
| `plant_backup_census.rs::the_backup_census_is_its_chains_and_the_ef_dot_fills` | `backup-plant-data\` 的 53 张图 |
| `render_gap_census.rs::every_refused_graphic_record_is_named_in_a_warning`、`a_refusal_and_a_missing_decoder_do_not_read_alike`（**部分**：夹具照读，少的是那一张备份图） | `backup-plant-data\…\A井场 注采阀组工艺及自控流程图.pid` |
| `sym_elliptical_arc_matches_cached_body.rs::sym_dome_matches_the_drawings_own_cached_dome` | `backup-plant-data\00\00\I井场 ….pid` 与同一个根下的 `RefData~4~681.zip` |

这个包里有一个文件其实在仓库里：`RefData~4~804.zip`（数据映射，4 357 748 字节）与入库的
`DWG-0202GP06-01_p/RefData~4~804.zip` 逐字节相同。但是用它的测试同时都要 `Export.dmp`，单有它跑不起来。

### 5.3 验收语料 `D:\work\cad\pid-test-data\验收数据`（`PID_ACCEPTANCE_CORPUS`）

11 张图（`.pid` 25.4 MB）。

| 测试 | 具体缺什么 |
|---|---|
| `publish_diff_accepted_baseline.rs::the_jk_well_site_prints_exactly_the_registered_differences`、`the_lm_well_site_…`、`the_light_hydrocarbon_separator_…`、`the_air_cooler_a_…`、`the_air_cooler_b_…`、`the_air_cooler_c_…` | 各自的那张 `.pid`，加 5.2 的 `Export.dmp` 与 `RefData~4~804.zip` |

### 5.4 读得少了、但没有哪条断言不做的

`sheet_chain_faults.rs::no_intact_stream_of_any_corpus_raises_a_chain_fault` 总是读仓库自己的文件（钉了
下限：675 个文档），仓库外的三处语料（两份 `pid-test-data`、共享语料）有就读。不在的每一处现在打印一行
`skipping: … is not on this machine, so the corpus under it is not read`（原来是 `not on this machine: …`）。

合计：5.1 的 13 条 + 5.2 的 10 条 + 5.3 的 6 条 + 5.4 的 1 条 = 30 条；加上第二节的 15 条，是「只有仓库」
时的 45 条。

## 六、给 #62 的清单：缺的测试数据

按「补上之后能让多少条测试真跑」排：

| # | 缺的数据 | 现在在哪里 | 体量 | 补上之后 | 说明 |
|---|---|---|---|---|---|
| 1 | P01 厂的 SQL Server 数据文件（`Export.mdf`），放到 `test-file/backup-test/DWG-0202GP06-01_p/extracted/` | **哪里都没有**。入库的是这个厂的 Oracle 导出 | 未知 | 13 条 DWG 镜像门禁第一次真跑；A23 那 7 种标签才有测试在比 | 两条路：从原厂环境要一份这个厂的 SQL Server 备份；或者让数据库这条线能从 Oracle 导出进暂存库（`ExpTables` 已经读得了这个 `Export.dmp`，缺的是「表 → SQLite 暂存」这一段）。后一条是功能，不是数据 |
| 2 | 53 张图的备份包的 `Export.dmp`（Oracle `exp`） | `D:\work\cad\pid-test-data\`（两份相同） | 71.0 MB | 18 条要用到它：`oracle_exp` 6 条、`publish_from_pid` 3 条（其中一条是整包发布）、`publish_diff_accepted_baseline` 9 条 | 体量大，且是整厂的数据库 |
| 3 | 共享语料的 5 张图与参考 XML | `D:\work\cad\PID自研软件验证\` | `.pid` 8.6 MB，`publish\` 2.2 MB | 与第 2 项一起：上面 18 条里的 9 条（`oracle_exp` 5、`publish_from_pid` 2、`publish_diff_accepted_baseline` 2）；单独：`publish_from_pid` 的 3 条 | 那 9 条点名要的都是 `I井场` 一张图 |
| 4 | 验收语料的 6 张图 | `D:\work\cad\pid-test-data\验收数据\` | 11 张共 25.4 MB | 与第 2 项一起：6 条 | |
| 5 | 备份的 53 张图（`backup-plant-data\`，或直接读 `PlantData~2~711.zip`） | `D:\work\plant-code\cad\pid-test-data\` | 97.2 MB 解包 / 20.4 MB 压缩 | `plant_backup_census` 1 条、`d04_…` 1 条（只要 `D04.pid`）、`render_gap_census` 2 条的那一张（只要 `A井场`）、`sym_elliptical_arc…` 1 条（要 `I井场` 加第 6 项） | 只要其中 3 张（`D04`、`A井场`、`I井场`）就能让 4 条真跑；普查那一条要全部 53 张 |
| 6 | 这个包的 `RefData~4~681.zip` | 同上 | 5.3 MB | `catalog_index` 2 条读全、`sym_elliptical_arc…` 1 条 | 与入库的两个 `~681` 是不同的文件 |
| 7 | `Wellhead Christmas Tree 02.sym` | 共享语料的 `Ref\Symbols\Equipment\Other Equipment\` | 81 920 字节 | 1 条 | 最小的一项：一个文件 |
| 8 | 受控差异的图纸对 | **哪里都没有** | — | 1 条 | 要在原厂软件里按协议做出来；`.pid` 按约定不入库，`metadata/*.json` 可以 |

另外，`parse_real_files.rs` 每次运行打印一行
`geometry fixture availability: registered=6, target_min_available=8, available=6, missing=[]; target gap=2`：
登记的几何夹具 6 个都在，目标是 8 个，差的 2 个从来没有登记过。这不是跳过（没有哪条断言因此不做），列在
这里是因为它也是「缺数据」。

**能从入库的压缩包里取、还没有用上的，没有了。** 逐项查过：`Wellhead Christmas Tree 02.sym` 不在两个入库
的符号包里；`D04`、`A井场`、`I井场` 不在入库的两个 `PlantData~2~711.zip` 里（那是另外两个厂）；入库的
`RefData~4~804.zip` 与备份包的相同，但没有 `Export.dmp` 用不上。

## 七、怎么保持

- **写法。** 测试因为缺数据而少做了断言，就打印一行，以 `skipping: ` 开头，说缺的是什么，然后通过。循环里
  每跳过一样东西打一行。`tests/soft_skip_spelling.rs` 扫测试代码（`tests/` 下的文件，和 `src/` 里
  每个文件从 `#[cfg(test)]` 模块起的部分，共 130 个文件、200 处 `skipping: `），遇到 `"skip: …"` 或不带冒号的
  `"skipping …"` 就失败并点名。它守不住的是不出声的跳过和别的说法，那要靠 1.2 的办法再量。
- **数。** `python tools/soft_skips.py --run`（本机：`--cargo "rustup run stable cargo"`）带输出跑根 crate 的
  测试，数出行数并逐行列出；`--by-test` 单线程跑，把每一行归到测试。也可以直接给它一份 `--nocapture` 的
  日志。门禁脚本里根 crate 那一组带输出跑、把这个数写进摘要即可；CI 的 `cargo test` 不带 `--nocapture`，
  CI 日志里看不到这些行，这一点没有变。
- **改过的写法**：`skip: …` 50 处（7 个文件）、不带冒号的 `skipping …` 8 处（5 个文件）、`skip {name}: …` 1 处、
  `…: skipping the library comparison…` 1 处、`[A23] … ; skipping` 1 处、`not on this machine: …` 1 处；原来不出声的现在说话的：`src/symbol_library.rs`
  测试模块里样本目录的两个守卫（`fixture`、`sample_library_is_here`）、`geometry_golden_snapshot.rs` 里缺的
  夹具（原来只进汇总行）。`src/inspect/commands.rs` 里那句 `skipping controlled diff case …` 是 `pid_inspect`
  的输出，不是测试的跳过，没有动。
