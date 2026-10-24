# N9-3：备份包目录一次发出 53 张图（2026-10-13，2026-10-14 补管线合成计数）

> `pid_publish_xml <目录> --out-dir`。Oracle `exp` 的 `Export.dmp` 与 `RefData~4~804.zip`（`SPPIDDataMap.xml`）各读一次；对非空图按 `PipeLinePublisher` 口径合成 `PIDPipeline` 与 `PipingConnectors`（N9-2，`7f1d891`，规则见 `docs/analysis/n6-pipeline-rows.md`「合成」）；`_Meta.xml` 写出 `derive_meta_uid` 派生的版本 / 修订 / 文件三件，`_Index.json` 标记 `pipeline_uids`（`4866b81`）。

## 成员

`D:\work\cad\pid-test-data` 今晚在盘上的是一整包：`Manifest.txt`、`Export.dmp`（74,489,856 字节，头 `EXPORT:V12.01.00`）、`PlantData~2~711.zip`（60 个条目，53 个 `.pid`）、`PlantData~2~722`（`SPEMDataMap.xml`，不是 zip，不参与发布）、`RefData~4~680` 到 `809`（其中 `RefData~4~804.zip` 含 `SPPIDDataMap.xml`）、`PlantConfig.xml`、`Export.log`。

## 写出

`pid_publish_xml D:\work\cad\pid-test-data --out-dir D:\work\cad\pid-test-data\publish --plant P01` 退出 0，53 / 53，失败 0。`_Index.json`（含 N9-2 管线合成后的盘上实测值，括号内为未挂管线时的 N8 口径）：

| 项 | 数 |
|---|---:|
| `pid_entries` / `drawing_rows` / `paired` | 53 / 53 / 53 |
| `failed` | 0 |
| 没有 `DrawingNo` 的空图（`shells`） | 2 |
| 对象（`objects`，不含那 2 张空图） | 80 085（未挂管线 77 690） |
| 值（`values`） | 87 419（未挂管线 82 726） |
| 关系（`relationships`） | 106 705（未挂管线 103 577） |
| 管道端口（`piping_ports`） | 9 557 |
| 信号端口（`signal_ports`） | 22 021 |
| 连通关系（`connectivity_rels`） | 19 371 |
| OPC 分支点（`opc_branch_points`） | 476 |
| 设备↔管口关系（`equipment_composition_rels`） | 372 |
| 注释关系（`item_note_rels`） | 114 |
| 管线（`pipelines`） | 2 395 |
| 管段↔管线关系（`piping_connectors`） | 3 128 |
| 管线 UID 口径（`pipeline_uids`） | `"synthesized"` |

空图是 `test/U01/D03.pid` 和 `test/U01/D04.pid`。zip 路径对上 `T_DRAWING.PATH`，图名是 `D03` / `D04`，层级是 `SQPlant \ test \ U01`，`uid_from` 是 `T_DRAWING.PATH`，不挂管线（`pipelines` / `piping_connectors` 为 0），各写一张只有 `PIDDrawing` 的 `_Data.xml`。其余 51 张合计多出的 2 395 个对象、4 693 个值（2 395 个管线 `Name` + 2 298 个 `FluidSystem`）和 3 128 条关系，全部来自 `attach_pipelines`。

I井场三件（对象 3 093、值 3 346、关系 4 107、管线 79、`PipingConnectors` 106）与同一份备份包的单张 `--oracle-exp Export.dmp --datamap RefData~4~804.zip --plant P01` 逐字节相同；`_Coverage.json` 比较时去掉 `source`。

初次 `cargo run` 从启动到退出 0 是 53 秒（含增量编译）；挂上管线后的 `backup_directory_publishes_the_whole_package` 整包 + 单张对照实跑约 46.8–50.2 秒。计划里逐张重读 dump 的一轮是 431 秒。

## 验证

- `cargo test --locked -p pid-parse --test publish_from_pid`：8 过 / 0 败（整包一组钉 80 085 / 106 705 / 2 395 / 3 128 / `"synthesized"` + 无 dump 的最小目录一组钉 `"not synthesized"` + I井场 `oracle_exp` 钉 3 093 / 3 346 / 4 107 + `.pid` 模式 5 组），测试套件耗时 46.75 秒（2026-10-14，`HEAD = 4866b81`）。
- `cargo test --locked -p pid-parse --lib plant_group_path_joins` 与 `publish::backup_package` 各 1 过。
- `cargo clippy --locked -p pid-parse --lib --bins --tests -- -D warnings` 零告警。
- `cargo clippy --locked -p pid-parse --lib --no-default-features -- -D warnings` 零告警。
- `cargo fmt --all -- --check` 干净。
- `cargo check --locked -p pid-parse --lib --no-default-features --target wasm32-unknown-unknown` 过。
- 全库 `--lib` 1 200 条、`--workspace`、`--all-targets` 这次没跑。
