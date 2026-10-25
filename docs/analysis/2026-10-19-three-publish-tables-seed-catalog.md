# 三张（实为五处）publish 表对账 → PublishItemCatalog 种子行集

工单：pid-parse #16（#14 地图的研究单，AFK 纯阅读）。日期 2026-10-19，HEAD `3722c2f`。

工单描述的三张表是 7 月的图景；10 月 `.pid` 发布线（N0–N12）落地后，同一类事实现在分散在**两条线、五处映射**里。catalog 若按 7 月的三表对账，会把 `.pid` 线整个漏掉。本页按现状对账。

## 0. 现状的五处映射

| # | 位置 | 线 | 事实 |
|---|---|---|---|
| 1 | `sqlite_load::subtables_for_item_type`（`sqlite_load.rs:470`） | DB（MDF/exp） | item_type → SQLite 子表链 |
| 2 | `xml_writer::write_business_objects` dispatch + `a01_object_rank`（`xml_writer.rs:165`/`:306`） | DB | item_type → writer 臂 + A01 排序档 |
| 3 | `diff::supported_pid_tags`（`diff.rs:261`） | DB | 写出器会发的 tag 清单（coverage_against_reference 用，只有 tests + `tests/common` 漂移守卫消费） |
| 4 | `pid_load::CLASS_OF_ITEM_TYPE` + `UNCLASSED_ITEM_TYPES` + `INSTRUMENT_CLASSES` + `EQUIPMENT_CLASSES`（`pid_load.rs:62`/`:78`/`:79`/`:85`） | `.pid` | ModelItemType → 发布类 |
| 5 | `pid_load::branch_point_class`（`pid_load.rs:1001`） | `.pid` | PipeRun/SignalRun → 派生分支点类 |

`.pid` 线的发出器是 `pid_writer.rs`（`write_item` 按 `item.class` 字符串发 tag，`pid_writer.rs:414`）；`oracle_exp.rs` 给 `.pid` 线加端口（`PIDPipingPort`/`PIDSignalPort`，`:782`/`:791`）与 OPC/注释/连通关系；`pipeline.rs` 从 `T_PIPERUN` + datamap 合成 `PIDPipeline`（N9-2）；`pid_writer.rs:191` 发 `<PIDPipeline>`。

## 1. 主表：item_type 全集对账

「supported」列 = 发出的 tag 是否落在 `supported_pid_tags` 15 项内。`—` = 该线不产此 item_type。

| item_type | DB 子表链 | DB writer 臂 → tag | a01 rank | `.pid` 线 class → tag | supported | 备注 |
|---|---|---|---|---|---|---|
| Vessel | PlantItem→Equipment→ProcessEquipment→Vessel | `write_process_vessel` → `PIDProcessVessel` | 0 | `PIDProcessVessel` | ✓ | 两线一致 |
| Nozzle | PlantItem→EquipComponent→Nozzle | `write_nozzle` → `PIDNozzle` | 1 | `PIDNozzle` | ✓ | 两线一致 |
| PipeRun | PlantItem→Connector→PipeRun→Pipeline | `write_pipeline`+`write_piping_connector`+派生 → `PIDPipeline`+`PIDPipingConnector`+`PIDPipingPort`×2+`PIDProcessPoint` | 2 | `PIDPipingConnector`（+`.PPT` 派生 +`PIDPipingBranchPoint` 派生；`PIDPipeline` 由 `pipeline.rs` 从 `T_PIPERUN` 另行合成） | ✓ | **同一行 DB 发 5 个对象，`.pid` 发 1+派生**；pipeline 的出处两线不同 |
| PipingPoint | T_PipingPoint | `write_piping_port` → `PIDPipingPort`（**死臂**：loader A13 起不再注入） | 9 | —（端口来自 exp，非 item_type） | ✓ | 子表链与 writer 臂同为前向兼容死代码 |
| PipingComp | PlantItem→InlineComp→PipingComp | `write_piping_component` → `PIDPipingComponent` | 9 | `PIDPipingComponent` | ✓ | 两线一致 |
| Instrument | 与 InstrFunction 同臂：PlantItem→Instrument→InstrFunction | `write_control_system_function`+派生 → `PIDControlSystemFunction`+`PIDSignalPort`×8 | 9 | 由 `instrument_class` 三分 → `PIDInstrument`/`PIDControlSystemFunction`/`PIDInlineInstrument` | ⚠ `PIDInstrument`/`PIDInlineInstrument` **未登记** | 两线同名不同类：DB 恒 `PIDControlSystemFunction`，`.pid` 按符号库目录三分 |
| InstrFunction | 同 Instrument 臂 | 同上 | 9 | —（`.pid` ModelItemType 只有 `Instrument`） | ✓ | DB 专有 item_type 名 |
| Note | 与 ItemNote 同臂：T_ItemNote | `write_item_note` → `PIDNote` | 9 | —（`.pid` 用 `ItemNote` 名） | ✓ | DB 双名共臂 |
| ItemNote | 同 Note 臂 | 同上 | 9 | `PIDNote` | ✓ | 两线一致（名不同） |
| Exchanger | PlantItem→Equipment→Exchanger | **无专臂 → `write_generic_object` → `PIDItem`** | 9 | `PIDHeatTransferEquipment` | ✗（`PIDItem`、`PIDHeatTransferEquipment` 均未登记） | **两线发出的类不同**（TODO A12+） |
| Mechanical | PlantItem→Equipment→Mechanical | **无专臂 → `PIDItem`** | 9 | `PIDMechanicalEquipment` | ✗ | 同上 |
| SignalRun | PlantItem→Connector→SignalRun | `write_signal_connector` → `PIDSignalConnector` | 9 | `PIDSignalConnector`（+`PIDSignalBranchPoint` 派生） | ⚠ `PIDSignalBranchPoint` **未登记** | `.pid` 线多一个派生 tag |
| BranchPoint | T_PlantItem（**暂定**，等 DWG 镜像确认） | `write_pid_branch_point` → `PIDBranchPoint` | 9 | —（`.pid` 的 `PIDBranchPoint` 来自 `OPC` item_type） | ✓ | DB 暂定行 |
| PipingBranchPoint | T_PlantItem（**暂定**） | `write_piping_branch_point` → `PIDPipingBranchPoint` | 9 | —（由 PipeRun 表示派生） | ✓ | DB 暂定行；`.pid` 侧是派生非 item_type |
| EquipmentOther | — | — | — | `PIDProcessEquipment` | ✗ 未登记 | `.pid` 专有 item_type |
| OPC | — | — | — | `PIDBranchPoint`（无 exp 时 class 空，见 `UNCLASSED_ITEM_TYPES`） | ✓ | `.pid` 专有 item_type |

## 2. 派生与文档级 tag（无 item_type 但发出）

| tag | DB 线出处 | `.pid` 线出处 | supported |
|---|---|---|---|
| `PIDDrawing` | `write_pid_drawing`（:2270） | `pid_writer` 图纸信封 | ✓ |
| `PIDRepresentation` | `write_representations`（:2310） | `pid_writer` 表示 | ✓ |
| `PIDPipeline` | PipeRun 臂 `write_pipeline`（:1072），UID 承 PipeRun | `pipeline.rs` 合成 + `pid_writer.rs:191`，UUID v5(plant,name) | ✓ |
| `PIDPipingPort` | PipeRun 派生×2（:1369）；PipingPoint 死臂 | `oracle_exp` 端口（:782） | ✓ |
| `PIDProcessPoint` | PipeRun 派生（:1391） | `.PPT` 派生（`pid_writer.rs:97`） | ✓ |
| `PIDSignalPort` | Instrument 派生×8（:1445） | `oracle_exp` 端口（:791） | ✓ |
| `PIDPipingBranchPoint` | PipingBranchPoint item 臂 | PipeRun 表示派生 | ✓ |
| `PIDSignalBranchPoint` | —（DB 线不发） | SignalRun 表示派生 | **✗ 未登记** |
| `PIDItem` | `write_generic_object` 兜底（:1995） | — | **✗ 未登记** |

## 3. 对账发现（按严重程度）

1. **`.pid` 线整个不在 `supported_pid_tags` 里**。`PIDProcessEquipment`/`PIDMechanicalEquipment`/`PIDHeatTransferEquipment`/`PIDInstrument`/`PIDInlineInstrument`/`PIDSignalBranchPoint` 六个 `.pid` 线实际发出的 tag 未登记。`supported_pid_tags` 的 docstring 自称「the executable counterpart of the dispatch matrix」，但它只覆盖 DB 线。catalog 必须把「哪条线」建成行的维度，否则同表异义。
2. **同一 item_type 两线发不同类**：Exchanger（DB `PIDItem` vs `.pid` `PIDHeatTransferEquipment`）、Mechanical（`PIDItem` vs `PIDMechanicalEquipment`）、Instrument（`PIDControlSystemFunction` vs 三分）。catalog 的 `writer`/`class` 列需要按线取不同值——单行单 writer 的草案装不下。
3. **item_type 集合本身不一致**：`EquipmentOther`/`OPC` 只有 `.pid` 线有；`InstrFunction`/`Note`/`PipingPoint`/`BranchPoint`/`PipingBranchPoint`（item_type 身份）只有 DB 线有（`.pid` 侧分支点是派生对象，不是 item_type）。种子行集是两线 item_type 的**并集**，多数行有一列为空。
4. **共享臂**：`Instrument|InstrFunction`、`Note|ItemNote` 在 DB 侧是两名共一链一臂——catalog 行要么收 `aliases: &[&str]`，要么拆两行同指针。
5. **死臂**：`PipingPoint` 的 DB 子表链与 writer 臂自 A13 起均不可达（loader 不注入独立 PipingPoint），catalog 应标 dead/forward-compat 而非按活项登记。
6. **暂定行**：`BranchPoint`/`PipingBranchPoint` 的 DB 子表链标着「provisional until DWG mirror」——catalog 需要 `provisional` 标志，或直接收进 fallback 语义。
7. **`a01_object_rank` 只有三档实名**（Vessel 0/Nozzle 1/PipeRun 2，余皆 9）且仅 `PublishStyle::A01` 用；是独立的 per-item-type 事实，可归并进 catalog 行也可以留给 writer——#15 拍板项。
8. **`PIDItem` 兜底 tag 未登记**：`write_generic_object` 实际会发 `<PIDItem>`（Exchanger/Mechanical 今天就走它），但 `supported_pid_tags` 不收——是有意（占位不算「会发」）还是漏，#15 拍板项。若 reference 里出现 `PIDItem`，coverage 会把它算成 backlog，方向就反了。
9. **关系侧未在本对账范围**：`supported_pid_tags` 只管对象 tag；`Rel`/`DefUID` 的连通六类、EQC/ITN/PCN/DrawingItems 在 `write_relationships`/`pid_writer`/`oracle_exp`/`pipeline.rs` 里另有映射，catalog 若要管关系需另起一张表。

## 4. 给 #15 的种子建议

行字段（在工单草案上按现状修正）：

```text
{ item_type, aliases: &[&str],
  db_subtables: Option<&[&str]>,           // DB 线子表链；.pid 专有空
  db_writer: Option<fn/臂>,               // DB 线 writer 臂；generic 兜底可单列行
  pid_class: Option<&str>,                // .pid 线发布类；未分类空
  derived: &[DerivedEmit],                // 该 item_type 派生的额外对象（端口/工艺点/分支点/管线）
  a01_rank: u8, provisional: bool, dead: bool }
```

- `PIDItem` 兜底、`PIDDrawing`/`PIDRepresentation` 文档级对象建议各占一个**特殊行**（`item_type = "*"` / `"$document"`），否则 fallback 语义无处登记。
- `supported_pid_tags` 之后若改成 `catalog.tags(line)` 派生，则 #16 发现 1 的六个漏登 tag 与 `PIDItem` 的取舍在 catalog 层一次说清。
- 本表未对 `InstrFunction` 的 `.pid` 侧定名——`.pid` 文件里 Instrument 的 ModelItemType 恒为 `Instrument`，`InstrFunction` 是 DB 表名；catalog 的 `pid_class` 对 `InstrFunction` 行应为 `None`。

## 5. 复核路径

- DB 三表：`src/publish/sqlite_load.rs:470-509`、`src/publish/xml_writer.rs:165-313`、`src/publish/diff.rs:261-279`
- `.pid` 两表：`src/publish/pid_load.rs:62-89,1001-1007`；发出面 `src/publish/pid_writer.rs:92-99,191,297-415`
- exp 端口/管线：`src/publish/oracle_exp.rs:782,791`、`src/publish/pipeline.rs`
- 消费方：`coverage_against_reference`（`diff.rs:375`）→ `tests/publish_writer_coverage.rs`、`tests/publish_attribute_parity.rs`、`tests/publish_backlog_inventory.rs`、`tests/common/mod.rs:116` 漂移守卫
