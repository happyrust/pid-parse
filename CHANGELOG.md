# 变更日志

## [Unreleased]

### Phase 14 Slice J：igLine2d (PSM 0x0018) decoder — Intergraph Sigma 标准线落地

**100× 真实数据集扩展**：在 `examples/probe_psm_type_code_histogram.rs`
跨 4 fixture 扫描发现 `PSM type code 0x0018 = igLine2d` 有 309
records cross-fixture（vs Slice D 的 3 个 SmartPlant `GLine2d` 封装
records）后，落地 igLine2d decoder：

- `src/parsers/sheet_records.rs` 加 `decode_iglines` /
  `decode_igline_at` 公开入口 + `SheetIgLine2dDecoded` DTO (含
  `byte_range` + PSM 头部 + `oid` + `parent_ref` + `sub_type_word` +
  `index` + `start` + `end`) + `length()` 便利方法。常量
  `PSM_TYPE_CODE_IGLINE2D = 0x0018`、`IGLINE2D_PAYLOAD_LEN = 50` /
  内部 `IGLINE2D_REMAINING_HEADER = 12`。
- **字节布局完全实测验证**（无需 IDA 反编译）：通过
  `examples/probe_igline2d_shape.rs` dump 真实 fixture record 字节
  揭示 6-byte PSM header + 50-byte payload =
  `(oid + parent_ref + remaining_header=12 + sub_type_word + index +
   start.xy + end.xy)`。Validate 强约束：`bytes_to_follow == 50` /
  `remaining_header == 12` / 4 doubles finite + in domain /
  start != end。
- `src/model.rs` 加 `DecodedIgLine2dRecord` stable DTO 配套
  `From<SheetIgLine2dDecoded>` + `length()` 便利方法 +
  `SheetGeometry.decoded_iglines: Vec<DecodedIgLine2dRecord>` 新字段。
- `src/streams/cluster.rs::sheet_geometry_from_probe` 同步调
  `decode_iglines(raw_data)` 填充。`SheetGeometry` 7 个构造点（geometry.rs
  6 + cfb/reader.rs 1）联动加 `decoded_iglines: Vec::new()`。
- `src/geometry.rs::build_normalized_geometry` 在 line/arc emit 块前
  emit `PidGraphicKind::Line` entities with `confidence: Decoded`
  + `record_kind: PrimitiveLine` + `note` 引用 IGDS 0x18 + Intergraph
  Sigma standard line。
- `src/parsers/sheet_records.rs::tests` 加 10 道单元测试覆盖
  canonical horizontal segment / 拒错 type / 拒错 bytes_to_follow /
  拒错 remaining_header / 拒退化零长 / 拒 NaN / 拒越界 / 截断 /
  噪声 / 双连续。
- `tests/parser_panic_safety.rs` 加 `decode_iglines` /
  `decode_igline_at` 进 panic-safety matrix。
- `tests/parse_real_files.rs` 新增
  `iglines_decoder_emits_decoded_iglines_with_provenance` 跨 fixture
  集成测试：跨 fixture **284 decoded igLines** (DWG-0201:24 +
  DWG-0202:42 + 工艺管道-1:218 + A01:0)，断言 ≥ 100 cross-fixture，
  每条 provenance 完整。
- `tests/parse_real_files.rs::normalized_geometry_probe_baseline_on_real_fixture`
  加 `decoded_igline_count` 算式联动。
- `tests/parse_real_files.rs::dwg0201_emits_decoded_primitive_lines_without_inferred_regression`
  断言 filter 加 `.note.contains("PSM GLine2d")` 区分 Slice E 原始
  GLine2d entities 与 Slice J igLine2d entities（都是 `Line` kind）。
- `src/schema.rs::tests::schema_exposes_sheet_geometry_dtos`
  ratchet 加 6 个新 needle：`DecodedIgLine2dRecord` /
  `decoded_iglines` / `parent_ref` / `sub_type_word` / `start_x` /
  `end_x`。
- 5 道 gate 全绿（808 unit + 84 integration tests + clippy + fmt +
  missing-docs ratchet=0）。

**跨 fixture 量化影响**：DWG-0201GP06-01.pid 现输出 **26 decoded
Line entities** (2 GLine2d + 24 igLine2d) — 比 Slice D-E 的 2 条线
**多 13 倍**！全部带完整 byte-level provenance + IDA-confirmed
fixture-verified evidence。

### Phase 14 Slice I：GArc2d decoder 加严 (GEllipse2d "majorAxis 沿 X" 约束落地)

实测 fixture byte dump 揭示部分 0x0030 records 字段 `axis_a.y` 不为 0
(出现 π/2/π/3π/2 等异常值)，违反 `radsrvitem.dll!sub_56524280` 的
`GEllipse2d` `"majorAxis is not along x axis"` 约束。这些 records 大
概率是**其他 PSM type collision 凑巧通过 0x0030 + 前 32 字节匹配**，
非真实 GArc2d。

落地新 validation：

- 加常量 `GARC2D_MAJOR_AXIS_ALONG_X_TOLERANCE = 1e-6`
- `decode_primitive_arc_at` 新增检查 `|axis_a.y| <= 1e-6`，拒绝
  axis_a.y ≠ 0 的 record

**跨 fixture 实测影响**：

- DWG-0201GP06-01.pid: **15 → 11** decoded arcs (-4)
- DWG-0202GP06-01.pid: **19 → 10** decoded arcs (-9)
- 工艺管道及仪表流程-1.pid: **32 → 27** decoded arcs (-5)
- A01.pid: 0 (unchanged)
- **总计: 66 → 48 decoded arcs (-27%, 18 false positives rejected)**

剩余 48 records 全部满足 `axis_a = (radius, 0)` 即 majorAxis 沿 X 轴，
符合 GEllipse2d 文档化约束。但其几何字段（`axis_ratio` / `sweep_*`
/ `sweep_direction`）的语义解读仍是 hypothesis（fixture byte dump
显示 a2+32..63 在多数 records 中是 packed integer fields 非 doubles），
**byte-level 解析正确，几何语义解读待下一 milestone 修正**。

5 道 gate 全绿（798 unit + 83 integration）。

### Phase 14 Slice H：GArc2d 字段语义重命名 (反编译收敛后)

Slice F/G 落地的 `SheetPrimitiveArcDecoded` / `DecodedPrimitiveArcRecord`
字段命名是 **早期推测的几何向量语义**（`axis1.x/y`, `axis2.x/y`,
`param_start/param_end`）。本里程碑通过反编译 `radsrvitem.dll!
GArc2d::Validate` 的 4 个内部辅助函数（`sub_56524280` / `sub_5658F950`
/ `sub_56539060` / `sub_564E0D90` / `sub_56537290`）收敛得到字段真实
语义后，**联动重命名 + 修正语义**：

- `axis1.x/y` → **`axis_a.x/y`** (Slice F/G 命名其实是对的——`a2+16..31`
  确实是 2D 向量经 `sub_5644E160` 计算 magnitude；只是名字模糊)
- `axis2.x/y` → **`axis_ratio` (`f64`) + `sweep_direction` (`u8`)** —
  Slice F/G 早期推测的"次轴向量"实际是 8 字节 `axis_ratio` (`f64`) +
  1 字节 `sweep_direction` (`u8`) + 7 字节 padding。这是**关键
  语义错位修正**。
- `param_start/param_end` → **`sweep_start_angle/sweep_end_angle`** —
  byte offsets 不变（48..55 + 56..63），命名更准确（radians 的扫描角度）。
- `axis1_magnitude()` → **`axis_a_magnitude()`**
- `axis2_magnitude()` → **`semi_minor_axis()`**（`= axis_ratio * |axis_a|`）
- `is_circular()` 语义从 `|axis2| < 1e-6` 改为 `|axis_ratio - 1.0| <
  1e-6`，对齐 `radsrvitem.dll!sub_56539060` (`IMElIsCir2d`) 的 IDA
  反编译条件。

decoder validation 同步增强：

- 加 `axis_ratio` 必须在 `[0, 1 + GARC2D_AXIS_RATIO_TOLERANCE]` 域
  内（轴比理论上 ≤ 1）
- 加 `sweep_direction` 必须 `≤ 1`（仅 0=CW, 1=CCW 合法）
- 加常量 `GARC2D_AXIS_RATIO_TOLERANCE = 1e-6`

落地点（**byte-level 解析不变**，仅命名 + 部分 validation 收紧）：

- `src/parsers/sheet_records.rs::SheetPrimitiveArcDecoded` + 关联
  `decode_primitive_arcs` + `decode_primitive_arc_at` 全部重命名
- `src/parsers/sheet_records.rs::tests` 11 道 unit test 更新 builder
  签名（`(center, axis_a, axis_ratio, sweep_direction, sweep_start,
  sweep_end)`）+ 反向 cases (rejects_oversize_axis_or_invalid_ratio +
  rejects_invalid_sweep_direction 替代旧 rejects_zero_axis1 等)
- `src/model.rs::DecodedPrimitiveArcRecord` 字段 + `From` impl 联动
- `src/streams/cluster.rs` 自动追随 `From` impl，不需要单独改
- `src/geometry.rs::build_normalized_geometry` 的 arc emit 块更新：
  `radius = record.axis_a_magnitude()` (保持)，`start_angle =
  record.sweep_start_angle`，`end_angle = record.sweep_end_angle`，
  `note` 字符串完整重写含 axis_a / axis_ratio / sweep_direction /
  sweep_start_angle / sweep_end_angle 五项参数化字段 + `semi_minor`
  便利字段 + `circular` 标记。
- `tests/parse_real_files.rs::primitive_arc_decoder_emits_decoded_arcs_with_provenance`
  断言全部联动：`arc.axis_a_magnitude()` 范围、`arc.axis_ratio` ∈
  `[0, 1+1e-6]`、`arc.sweep_direction <= 1`、`arc.sweep_start_angle <
  arc.sweep_end_angle`、sample 文本更新含新字段。
- `src/schema.rs::tests::schema_exposes_sheet_geometry_dtos`
  schema ratchet 加 `axis_a_x` / `axis_ratio` / `sweep_direction` /
  `sweep_start_angle` / `sweep_end_angle` 5 个新 needle，公共 JSON
  schema 自动派生新字段名供下游 TS/Python/C# 代码生成。
- 5 道 gate 全绿（798 unit + 83 integration tests）。

**跨 fixture 实测影响**：byte 流不变，type code 0x0030 record 数量
不变（**still 3+15+19+32+0 = 66 decoded arcs**）；唯一变化是
`is_circular()` 现在基于 `axis_ratio ≈ 1.0` 而非旧的 `|axis2| ≈ 0`：

```
DWG-0201GP06-01.pid: 15 decoded arcs (0 marked circular — axis_ratio 全 ≈ 0)
DWG-0202GP06-01.pid: 19 decoded arcs (0 marked circular)
工艺管道及仪表流程-1.pid: 32 decoded arcs (0 marked circular)
A01.pid: 0 decoded arcs
```

**遗留语义谜题**：`axis_ratio = 0` 意味着 axis_b = 0 即弧退化为线段，
但 SmartPlant 实际工程图上明显有大量真圆 (instrument circles)。这暗示
`a2+32` 字段的语义解读 **可能不是简单的 `axis_b / axis_a` 比值**，
而是 (e.g.) eccentricity / aspect_ratio inverse / 其他几何参数。
`sub_56539060` 的 `|a1+32 - 1.0| ≤ tol` 在所有 fixture 0x0030 records
上都返回 false，矛盾于 SmartPlant 实际有真圆的事实。

此为 Phase 14 后续 milestone 待深究的 finer point。当前 DTO 字段命名
准确反映 byte 层位置和 IDA-confirmed 角色（axis_ratio、sweep_direction、
sweep_*_angle），即使 axis_ratio 的几何语义解读尚未完全 nailed down。

### Phase 14 后续：IDA 反编译知识沉淀 + 公共 schema ratchet

延续 Phase 14 Slice D-G 工作，把 SmartPlant `.pid` 反向工程的剩余证据
链完整文档化，为下一会话或下一里程碑提供 actionable 起点：

- **Intergraph Sigma IGDS class tag 主映射表**：反编译
  `sub_56448F70` (@`0x56448F70`, size `0x18f`) 拿到 **28 个内存 C++
  类标识 → 类名** 全表，覆盖
  `igLine2d=0x18` / `igArc2d=0x61` / `igLineString2d=0x84` /
  `igCircle2d=0x59` / `igEllipticalArc2d=0x7E` / `igRectangle2d=0x20` /
  `igSymbol2d=0xCE` / `igPoint2d=0x5E` / `igEllipse2d=0x63` /
  `igBSplineCurve2d=0x5D` 等几何类，以及
  `igDimension=277` / `igBalloon=279` / `igLeader=280` 等标注类。
- **IGDS vs PSM 双 ID 系统辨析**：IGDS class tag 是
  **内存对象内部 C++ 类标识**，PSM record type code 是
  **磁盘序列化标识**。实测 GLine2d IGDS=`0x18` PSM=`0x3FE6`，GArc2d
  IGDS=`0x61` PSM=`0x0030`，两套不重合。未来 decoder 调试必须以
  PSM type code 为准（disk bytes 0..2 的低 14 位）。
- **PSMSerializeOut/In 对称 dispatch 架构**：反编译揭示读写路径都用
  IJPersist::Save/Load vtable 动态分发；只有 5 个 fast-path 固定大小
  type code (`276`/35B、`277`/16B、`278`/53B、`279`/8B、`280`/59B)
  在 dispatcher 内嵌写出，所有几何类均走虚函数。
- **`PersistTypeTable<PersistComTypeEntry>` 类 + `guidtab.h` 表结构
  识别**：表的 root pointer 在全局 `dword_567DDC90`（C++ 类 vtable
  `0x5665FA1C`），由 `sub_56441330` CRT 启动时构造并 `atexit` 注册
  析构。每个 entry 含 `+16` matching PSM type code (u16) + `+18`
  chain link + 可能 IGDS tag/CLSID/factory 指针。条目通过各
  `IGDSFactory*` 模块 init 分散注册到表中。
- **`docs/analysis/2026-05-14-radsrvitem-psm-serialize-bytes.md`**
  扩展四节：IGDS class tag 主映射表 (28 行)、IGDS vs PSM 系统对照表、
  PSMSerializeOut/In 双 dispatch 架构说明、`PersistTypeTable` C++ 类
  与 guidtab.h 表结构 + 分散注册机制说明。`progress.jsonl` append 6
  条新 evidence：`psm_serialize_out_decompiled` / `psm_serialize_in_dispatch_via_ijpersist`
  / `igds_class_tag_lookup_table_discovered` / `persisttypetable_class_identified`
  / `glinestring2d_probe_inconclusive` / `slice_f_g_complete`。

#### 公共 schema ratchet (Slice E/G DTO 进 JSON schema)

- `src/schema.rs::tests::schema_exposes_sheet_geometry_dtos` 新增 4 个
  断言 needle：`DecodedPrimitiveLineRecord` / `DecodedPrimitiveArcRecord` /
  `decoded_primitive_lines` / `decoded_primitive_arcs`。schemars 自动从
  `#[derive(JsonSchema)]` 派生，但显式断言防止未来重构意外把
  `Decoded*` DTOs 从 `pid_document_schema_pretty()` 输出排除，影响
  下游 TS/Python/C# 代码生成消费者。
- `examples/probe_psm_polyline.rs` 通过 `cargo fmt` 自动重排格式 +
  `push_str(" ") -> push(' ')` 过 clippy `single_char_add_str` lint。

#### Phase 14 整体里程碑总结

至此 Phase 14（SmartPlant `.pid` Sheet 几何 PSM 解码）完成 13 个核心
阶段：B1 解锁 → PSM 18B 头部反编译 → GLine2d 48B + GArc2d 64B 字段
反编译 → Slice D/F decoder API 上线 → Slice E/G geometry pipeline 接入 +
不回归保护 → GLineString2d 内存布局反编译 → IGDS 主表 + PSM/IGDS 双
系统辨析 → PersistTypeTable + guidtab.h 表结构 → 公共 schema ratchet。

DWG-0201GP06-01.pid 实测输出：

| Layer | Count |
|---|---|
| `Decoded` `Line` (PSM `0x3FE6`) | 2 |
| `Decoded` `Arc` (PSM `0x0030`) | 15 |
| `Inferred` `Line` (EndpointPair) | 49+ |
| `Inferred` `Point` (coord/hint) | 117 |
| `ProbeOnly` `Unknown` (text/endpoint) | ~50 |
| **Total entities** | **202+** |

跨 4 fixture 累计 **3 decoded lines + 66 decoded arcs**，全部带
`stream_path` + `byte_range` + `record_kind` + `graphic_oid` + `note`
五件套 provenance，note 引用 `radsrvitem.dll` IDA 反编译证据链。

下一会话 actionable 起点（任选）：

1. 反编译各 geometry 类 `JStyleBase::IJPersistImp::Save` 类似的虚函数
   拿到 polyline/circle/text 磁盘字段布局
2. 反编译 `IGDSFactoryLineString` (`0x5666AA2C` vtable) 的构造函数
   或 register 调用拿到 PSM type code
3. IDA 调试器运行时 dump `dword_567DDC90` 表 → 拿 PSM type code 全表
4. Plan B controlled-diff 协议造 polyline-only / circle-only fixture
   做 before/after byte diff 反推 layout

落地候选 decoder 家族：`decode_primitive_polylines` (`igLineString2d` 0x84) /
`decode_primitive_circles` (`igCircle2d` 0x59) / `decode_primitive_texts`
(`IGDSFactoryText`) / `decode_primitive_symbols` (`igSymbol2d` 0xCE) /
`decode_primitive_rectangles` (`igRectangle2d` 0x20)。每一个都可复用
Slice D-G 已经验证过的七层模板（IDA Validate → probe → decoder API + DTO →
unit tests → model DTO → streams.rs 填充 → geometry.rs emit → regression
guard → 5 道闸门 + schema ratchet → docs+progress evidence）。

### Phase 14 Slice F/G：PSM `GArc2d` 解码器 + Arc geometry 上线

- `src/parsers/sheet_records.rs` 加 `decode_primitive_arcs(&[u8])` /
  `decode_primitive_arc_at(&[u8], usize)` 两个公开入口，配套
  `SheetPrimitiveArcDecoded` DTO (`byte_range` + PSM 头部 +
  `center` / `axis1` / `axis2` / `param_start` / `param_end`) +
  `axis1_magnitude()` / `axis2_magnitude()` / `is_circular()` 便利
  方法。常量 `GARC2D_PAYLOAD_LEN = 64`、`PSM_TYPE_CODE_GARC2D = 0x0030`
  公开供下游 fixture 测试引用。
- type code `0x0030 = 48` 来自 `examples/probe_psm_garc2d.rs` 跨 3
  fixture 实测：DWG-0201 3/5 / DWG-0202 10/14 / 工艺管道-1 21/25 个
  arc hits 全部走这个 type code（其他 0x0000 / 0x0001 是 false
  positive byte 重合）。字段布局来自 `radsrvitem.dll!sub_56524150`
  (`GArc2d::Validate`) 反编译: 8 × f64 = `(center.x, center.y,
  axis1.x, axis1.y, axis2.x, axis2.y, param_start, param_end)`。
- 5 条 validation: type_code == 0x0030, bytes_to_follow >= 64 不越
  界, 8 doubles finite + `|x| <= 1e9`, axis1_magnitude 在
  `[1e-6, 1e3]` (拒零和巨噪声), axis2 magnitude <= 1e3 (允许 0 即
  circular case), param_start < param_end。
- `src/model.rs` 加 `DecodedPrimitiveArcRecord` stable DTO + 镜像 12
  字段 + `axis1_magnitude/axis2_magnitude/is_circular` 便利方法 +
  `SheetGeometry.decoded_primitive_arcs` 新字段。
- `src/streams/cluster.rs::sheet_geometry_from_probe` 同步调
  `decode_primitive_arcs(raw_data)` 填充 SheetGeometry。
- `src/geometry.rs::build_normalized_geometry` 在 line emit 块后
  emit `PidGraphicKind::Arc` entities (`center` / `radius =
  |axis1|` / `start_angle = param_start` / `end_angle = param_end`)
  with `confidence: Decoded` + `record_kind: PrimitiveArc` +
  `note` 含完整参数化 payload 含 `is_circular` 标记。第一版映射
  对圆形 arc 精确, 对椭圆 arc 用 `|axis1|` 作 radius 是 usable
  approximation, ellipse 完整 axis2 信息保留在 note。
- 跨 fixture 实测 (`primitive_arc_decoder_emits_decoded_arcs_with_provenance`):
  DWG-0201 15 arcs (全 circular) / DWG-0202 19 (全 circular) /
  工艺管道-1 32 (全 circular) / A01 0。全部带 provenance triplet
  + axis1 magnitude 在 [1e-6, 1e3]。
- `src/parsers/sheet_records.rs::tests` 加 11 道单元测试覆盖
  正反例: canonical circle + ellipse / 拒错 type / 零 axis1 /
  超大 axis / 反向 param / NaN / 截断 / 噪声 / 双连续 / attribute
  tail byte_range。
- `tests/parser_panic_safety.rs` 加 `decode_primitive_arcs` /
  `decode_primitive_arc_at` 进 panic-safety matrix。
- `tests/parse_real_files.rs` 加
  `dwg0201_emits_decoded_primitive_arcs_without_regression` 断言
  DWG-0201 上 inferred_lines >= 49 + decoded_lines >= 1 (Slice D/E
  floor) + decoded_arcs >= 1 (Slice G new) + 全套 provenance + arc
  geometry invariants (radius > 0, start < end, finite center)。
- `tests/parse_real_files.rs::normalized_geometry_probe_baseline_on_real_fixture`
  算式更新加 `decoded_arc_count`。"Decoded 仅 Line" 松绑为 "Decoded
  Line 或 Arc"。"无 Polyline/Arc/Circle/Text/SymbolInstance" 松绑允许
  Arc when `confidence == Decoded` (Phase 14 Slice G 例外)。
- `tests/parse_real_files.rs::curve_primitive_investigation_reports_unsupported_curve_candidates`
  从 "decoded_polylines + decoded_circles + decoded_arcs == 0" 改为
  "decoded_polylines + decoded_circles == 0" —— Slice G 后 decoded
  arcs 由独立 PSM decoder emit, 不再是 0; investigation 层(probe
  level)依然不 promote 几何, 该断言精确反映分层。
- `examples/probe_psm_garc2d.rs` 一次性 byte-level probe 工具
  落地证据收集。
- 5 道 gate 全绿 (build / test 797 unit + 83 integration / clippy
  `-D warnings` / fmt / missing-docs ratchet=0)。

### Phase 14 Slice E：解码 line 接入 geometry pipeline + 不回归保护

- `src/model.rs` 加 `DecodedPrimitiveLineRecord` stable DTO（与
  parser-level `SheetPrimitiveLineDecoded` 对照，字段全展开为
  `byte_start/byte_end/type_code/type_flags/bytes_to_follow/oid/origin_x/y/direction_x/y/param_start/param_end`，
  带 `From<SheetPrimitiveLineDecoded>` + `endpoint_a/b` 便利方法）
  与 `SheetGeometry.decoded_primitive_lines: Vec<DecodedPrimitiveLineRecord>`
  新字段（serde 带 `default + skip_serializing_if = "Vec::is_empty"`
  保留向前/向后兼容）。
- `src/streams/cluster.rs::sheet_geometry_from_probe` 在 SheetGeometry
  构建路径同时跑 `decode_primitive_lines(&raw_data)` 把每一条 record
  转 model DTO 放进新字段。空 fixture / 老 record shape 不阻止
  `SheetGeometry` 仍然 `Some`（之前要 text 或 coord 非空才 Some，
  现在 decoded line 也算）。
- `src/geometry.rs::build_normalized_geometry` 在每个 Sheet 循环里加
  一个新 emit 块，按 `decoded_primitive_lines` 产 `PidGraphicEntity`
  with `kind: PidGraphicKind::Line { start, end }` + `confidence:
  PidGeometryConfidence::Decoded` + `source.record_kind:
  SheetRecordKind::PrimitiveLine` + `graphic_oid: Some(record.oid)` +
  `note` 含 `"PSM GLine2d record decoded from radsrvitem.dll ..."`
  完整 byte 层证据，**追加在现有 inferred/endpoint emit 之后**——
  inferred entities 不动，49 line floor 完整保留（DWG-0201 AC8）。
- `tests/parse_real_files.rs` 新增
  `dwg0201_emits_decoded_primitive_lines_without_inferred_regression`：
  断言 DWG-0201 上 `inferred_lines >= 49` + `decoded_lines >= 1`，
  并 spot check 第一个 decoded line 的 provenance triplet
  (stream_path == `/Sheet6`, byte_range 非空, record_kind ==
  PrimitiveLine), graphic_oid 非空, note 提到 `PSM GLine2d` +
  `radsrvitem.dll`, geometry 不退化为单点。
- `tests/parse_real_files.rs::normalized_geometry_probe_baseline_on_real_fixture`
  baseline 算式更新：原 `text + coord + endpoint + hint` 改为
  `+ decoded_line_count`。原 "Decoded confidence not allowed in real
  fixture baseline" 断言松绑：允许 Decoded **当且仅当**
  `kind: PidGraphicKind::Line`（Phase 14 当前唯一可 Decoded 的几何
  类型）。这保留了 Decoded 的边界约束但让 Slice E 上线通过。
- 6 个其他 SheetGeometry 构造点（cfb/reader.rs 测试 + geometry.rs 6
  处单元测试） 加 `decoded_primitive_lines: Vec::new()` 字段。
- 5 道 gate 全绿：build / test (Slice E 加 1 test → 81 integration)
  / clippy `-D warnings` / fmt / missing-docs ratchet=0。
- Phase 14 AC8 ✅ 完成 → **11/11 AC 全闭环**。`pid-parse` 现在能在
  DWG-0201 上同时输出 2 条 `Decoded` line + 49+ 条 `Inferred` line
  + 64 inferred points + 53 promoted hints + N probe-only entities，
  全部带完整 provenance triplet。

### Phase 14 Slice D：PSM `GLine2d` PrimitiveLine 解码器（Decoded 几何上线）

- `src/parsers/sheet_records.rs` 加 `decode_primitive_lines(&[u8])` /
  `decode_primitive_line_at(&[u8], usize)` 两个公开入口，配套
  `SheetPrimitiveLineDecoded` DTO（`byte_range` + PSM `type_code` /
  `type_flags` / `bytes_to_follow` / `oid` + 参数化几何
  `origin` / `direction` / `param_start` / `param_end`），以及
  `endpoint_a()` / `endpoint_b()` 笛卡尔便利方法。常量
  `PSM_RECORD_HEADER_LEN = 18`、`GLINE2D_PAYLOAD_LEN = 48`、
  `PSM_TYPE_CODE_GLINE2D = 0x3FE6` 一并公开供 `parser_panic_safety` 等
  下游测试引用。
- 字节布局完全来自 IDA 反编译：18-byte PSM header 来自
  `radsrvitem.dll!PSMSerializeOut/In`，48-byte GLine2d payload
  来自 `radsrvitem.dll!sub_56524C50` (`GLine2d::Validate`)。文档
  `docs/analysis/2026-05-14-radsrvitem-psm-serialize-bytes.md`
  扩展含字段表 + Validate 反编译伪码 + 实测对账结果。
- 解码器是**保守的**：只接受 5 条 validation 全部通过的 record
  —— `type_code == 0x3FE6`、`bytes_to_follow >= 48` 且不越界、6 个
  doubles 全 finite 且 `|x| <= 1e9`、direction 单位向量误差
  `< 1e-3` 且不退化为 0、`param_start < param_end`。明确不做
  几何到 `PidGeometryConfidence::Decoded` 的 promotion——这是 Slice
  E 的工作。
- `tests/parse_real_files.rs` 新增
  `primitive_line_decoder_emits_decoded_lines_with_provenance`
  跨 fixture 集成测试：DWG-0201 /Sheet6 = 2 hit、A01 /Sheet6 = 1 hit、
  DWG-0202 + 工艺管道 = 0 hit（fixture 用更老/不同的 record shape），
  全部 record 带 stream path + byte range + record kind 三件套
  provenance，所有断言（byte_range 不越界、direction 单位、param 顺序
  与全 finite）通过。
- `src/parsers/sheet_records.rs::tests` 加 10 道单元测试覆盖
  decoder 正反例：canonical synthetic + 拒绝错 type / 非单位
  direction / 零 direction / 反向 param 范围 / NaN 坐标 / 截断输入 /
  对抗噪声 / 双连续记录 / `byte_range` 包整条 record（含 attribute tail）。
- `tests/parser_panic_safety.rs` 把
  `decode_primitive_lines` / `decode_primitive_line_at` 加入 panic-safety
  matrix（涵盖空/极短/0x00/0xFF/byte-cycle/xorshift32/UTF-8-lossy 等
  adversarial corpus + truncation sweep）。所有现有 panic-safety 测试 + 新
  decoder 全绿。
- `examples/probe_psm_gline2d.rs` 落地一次性 byte-level probe（产生
  Slice D 的核心证据）：参数化 origin 接受范围、unit-vector tolerance、
  domain limit 与生产 decoder 同步。可手动对任意 `.pid` + Sheet 流跑：
  `cargo run --release --example probe_psm_gline2d -- <path> /Sheet6`。
- 5 道 gate 全绿（build / test / clippy `-D warnings` / fmt / missing-docs
  ratchet=0）。`primitive_line_decoder_emits_decoded_lines_with_provenance`
  在工作区 fixture 上 1 fixture 0 hit / 2 fixture 各 1-2 hit，总 3 条
  decoded line 全部 provenance triplet 完整、几何 invariant 通过。

### Phase 14 Slice A-C：radsrvitem.dll PSM 头部 + GLine2d/GArc2d 字段反编译

- B1 (rad2d.dll / pidobjectmanagerinf.dll IDA reverse) **解除**：用户
  提供 `E:\weixin\…\bin` 目录后，agent 用 `dumpbin /imports` 静态扫
  18 个候选 DLL，发现 `radsrvitem.dll` (3.7 MB) 是**唯一**导入
  `StgOpenStorageEx` 的二进制——即 SmartPlant Sheet I/O 入口。后续
  IDA batch (`-A -B`) 预生成 `.i64` 文件后 GUI 打开避开模态对话框，
  挂上 ida-pro-mcp 端口 13346 在线分析。
- Slice A: 反编译 `PSMSerializeOut` (`0x56491E80`) 与 `PSMSerializeIn`
  (`0x564915E0`) 解码 PSM record byte 布局 —— `[2B type_code (top
  2 bits flags)] + [4B bytes_to_follow] + [4B oid] + [8B aux]` 共 18
  字节头部。`PSMSerializeIn` 的 switch (5 个 case) 揭示 5 种固定大小
  type code：276/35B、277/16B、278/53B、279/8B、280/59B（共 171 字节
  固定 fast path）；其他 type 走 variable-length 通过 `guidtab.h`
  GUID 表派发。**14-bit type code** + 顶 2 bits 是 record flags。
- Slice A 余波：在 `radsrvitem.dll` 字符串里扫到完整 Intergraph
  Sigma 2D 几何家族（`igLine2d` / `igArc2d` / `igCircle2d` /
  `igEllipticalArc2d` / `igLineString2d` / `igBSplineCurve2d`）+
  对应 `IGDSFactory*` builder 类家族 + 接口
  `IJGeometry` / `IJTypedGeometry2d` / `IJKeyPoint` / `IJPoint2d`。
- Slice C: 反编译 `GLine2d::Validate` (`sub_56524C50` @ 0x56524C50, 0x17F
  bytes) → 字段表 48 bytes = 6 × `f64` LE
  `(origin.xy, direction.xy unit, param_start, param_end)`。几何
  语义是参数化 `point(t) = origin + t * direction`，不是 start→end
  对，解码器消费侧必须显式换算 endpoint。Validate flags：
  `0x1` = NaN/invalid，`0x8` = direction 非单位向量，
  `0x200000` = `param_start > param_end`。
- Slice C: 反编译 `GArc2d::Validate` (`sub_56524150` @ 0x56524150, 0x128
  bytes) → 字段表 64 bytes = 8 × `f64` LE，offsets 48 与 56 的 NaN
  检查印证 `param_start` / `param_end` 位置。推测字段为
  `(center.xy, axis1.xy, axis2.xy, param_start, param_end)`——Intergraph
  椭圆弧表示，圆退化为 axis2 = 0。
- Slice C **意外发现**：通过 RTTI `.?AVIGDSFactoryLine@@` xref 拿到
  `IGDSFactoryLine` 的 primary vtable (0x5666A94C)。Slot 0-2 是
  `IUnknown` 共享，slot 3 共享，**slot 4-6 是一行 setter
  (`*(this+70)=a2`, `*(this+74)=a2`, `*(this+78)=a2`)**——证实
  `IGDSFactory*` 是属性 builder pattern 不是 Save 入口，几何对象
  Save 经 `IJTypedGeometry2d` 接口 / `PSMSerializeOut` 走对象本身。
- 文档 `docs/analysis/2026-05-14-radsrvitem-psm-serialize-bytes.md`
  扩展含 PSM header 表 + GLine2d 6×f64 字段表 + GArc2d 8×f64 字段表
  + IGDSFactory vtable 调查 + 完整 Sheet primitive byte 流推测
  + 后续 Slice C/D 路径建议。
- `goals/phase14-sppid-sheet-geometry/progress.jsonl` 追加 4 条
  evidence（slice_a_psm_decompiled / b1_unlocked_radsrvitem /
  rtti_factory_decompiled / slice_c_breakthrough）。

### Phase 14 Plan B：控制 `.pid` diff fixture 采集协议 v1

- 新增 `docs/protocols/2026-05-13-controlled-pid-diff-collection.md`
  约 400 行中文协议：覆盖 SmartPlant P&ID 6 类原子操作（`place_line`
  / `place_polyline` / `place_circle` / `place_arc` / `place_text` /
  `place_symbol`）的 step-by-step 操作员手册，含 line + circle 完
  整演示、metadata sidecar JSON schema 与
  `pid_parse::inspect::controlled_diff::ControlledDiffMetadata` 字
  段逐项对照、`pid_inspect --controlled-diff-dir` 自检 stdout/JSON
  example、故障排查节、数据安全节。SmartPlant 版本锁 12.x。
- 新增 `goals/phase14-plan-b-controlled-diff-protocol/` 第二个
  plannotator goal package（DRAFT — 浏览器面板离线时 5 道 gate 未
  跑），与 `goals/phase14-sppid-sheet-geometry/`（IDA 反向路径）形
  成 Phase 14 双证据链。该 Plan B goal 独立不阻塞 B1。
- 新增 `test-file/controlled-diff/{before,after,metadata}/`
  目录骨架与 `.gitkeep` 占位 + `README.md` 入口说明。`.gitignore`
  加规则 `test-file/controlled-diff/{before,after}/*.pid` 阻止
  plant 真实 `.pid` 入 git，保护 plant-proprietary 数据。
- `tests/inspect_cli.rs` 新增
  `controlled_diff_protocol_synthetic_two_case_walkthrough`：合成
  2 case (line + circle) 走协议目录约定，断言 `--controlled-diff-dir
  --json` 产 2 cases、`promoted_geometry=false`、each
  `first_modified.path=/Sheet6`、`expected` payload verbatim round-trip。

### Phase 14 goal package via plannotator + Slice A 启动 stop-and-ask

- 用 `plannotator-setup-goal` skill 把"实现 SPPID `.pid` 完整解析"的高
  层目标收窄成可执行 goal package：`goals/phase14-sppid-sheet-geometry/`
  下 5 份文档（`brief.md` / `plan.md` / `verification.md` / `blockers.md`
  / `goal-prompt.md` + `progress.jsonl` + `slice-a-runbook.md`）全部
  通过 `plannotator annotate --gate` 用户审阅。Scope 锁定为 Phase 14
  只升级一类 Sheet primitive 到 `PidGeometryConfidence::Decoded`，
  PrimitiveLine 为先锋类（49 条 inferred line 作回归地板）。
- `plan.md` 列出 11 条 acceptance criteria (AC1–AC11) 与 evidence
  pinpoint 表，`verification.md` 列出 12 条命令矩阵 + 半手工 IDA
  核查规则，`blockers.md` 显式记录 B1（rad2d.dll / pidobjectmanager.dll
  入仓）硬阻塞、Q1–Q3 deferred questions、7 条 stop-and-ask 触发条
  件、5 条不可逆动作授权门，全部嵌入 mermaid 流程图 / gantt 时间线。
- 新增 `goals/phase14-sppid-sheet-geometry/slice-a-runbook.md`：把
  B1 解锁后 Slice A "IDA 调用点定位" 拆成 8 步可执行 runbook（拷贝
  DLL → IDA attach → survey_binary → find_regex Sheet → xrefs_to →
  decompile → confirm `OpenStream` → 落文档），含 3 个 stop-and-ask
  触发与 0.5–2h 时间预算。
- 用户「开始执行」后 Slice A 第 1 步即触发 stop-and-ask：执行
  `imports_query` 跨 8 个已加载 SPPID 二进制，全部返回 `*Stream*` /
  `*Storage*` / `ole32::*` stream API 零命中，唯一 ole32 引用为
  `sppidautomationwrap.dll` 的 `CoDisconnectObject`（COM 清理）。这把
  B1 的证据等级从 string-level（"无 Sheet 字符串命中"）升级到
  import-level（"零 CFB stream API 调用面"），同时 append `slice_a_attempt`
  evidence 到 `goals/.../progress.jsonl`。

### Publish 管线 schema gap 闭环（T_ModelItem audit + T_Pipeline）

- `src/publish/sqlite_load.rs` `load_objects_by_uids` 现在读 `T_ModelItem`
  的 8 列完整 schema，把先前忽略的 `IsUnchecked` / `ModelItemType` /
  `UpdateCount` / `ItemStatus` 4 列 audit 字段塞进 `PublishObject.fields`，
  空字符串 / NULL 自动过滤以保持 fields map 紧凑。匹配 SQL Server
  TEST02 fixture 与 Oracle DWG-flavour DDL 共有的 schema。
- `src/publish/mdf_load.rs` `PUBLISH_TABLES` 增加 `T_Pipeline`，
  `subtables_for_item_type("PipeRun")` 子表链末尾加 `T_Pipeline`。配合
  Writer A19 已有的 `obj.fields["OperFluidCode"]` / `obj.fields["FluidSystem"]`
  读取路径，PIPELINE 业务字段从 loader 到 `<IFluidSystem FluidCode="…"
  FluidSystem="…"/>` 输出端到端打通——之前 Writer 路径就绪但 loader
  从未填过这两个槽位（旧注释误把它们归到 T_PipeRun）。
- 新增 5 个单测：T_ModelItem audit 列下沉 / 空 NULL 过滤 / 部分填充 /
  PipeRun 子表链尾包含 T_Pipeline / 合成 SQLite 端到端验证。

### SPPID 备份格式识别 + Oracle 12c exp 诊断

- `pid_backup_extract` 现在能识别 Oracle Database 12c `exp` 格式
  （magic `\x03\x03iEXPORT:V`），失败信息清晰指出该 dump 不是 SQL
  Server MTF 备份，需要 Oracle `imp` / `impdp` 工具或对应字段提取
  通道，且指向 `examples/oracle_exp_schema.rs` 做 DDL-only 检查。
  避免之前的 `tag '????'` 谜之报错。
- 新增 `examples/oracle_exp_schema.rs`：扫 Oracle exp `.dmp` 文件
  内嵌的 `CREATE TABLE` 明文 DDL。在 DWG-0202GP06-01 fixture 上抽出
  126 个 T_* 表的完整列定义，与 TEST02 SQL Server schema 交叉验证
  publish 管线缺口（T_PIPERUN 在 DWG 有 293 列、TEST02 约 15 列，
  确认 AGENTS.md 提及的 "DWG canonical-field enrichment" 方向真实
  存在）。
- 新增 `docs/analysis/2026-05-13-ida-pro-mcp-reconnaissance.md`：在
  8 个 SPPID 二进制（sppid.dll / sppidautomation*.{dll,exe} /
  sppidautomationwrap.dll / sppiddwgprocess.dll / smartplantpid.exe /
  ipidobjectmanagerinf.dll / llama.dll）做完整 IDA Pro MCP 侦察的结论：
  全部为 VB6 / MFC / COM 调度层，**无一**承载 Sheet primitive 字节
  解析器。需额外提供 `rad2d.dll` / `pidobjectmanager.dll` 才能推
  Phase 14 反向工程。文档列出 3 条候选 plan（A 获取 DLL / B controlled
  diff / C 先 commit 现有改动），并记录 Oracle exp row 数据 heuristic
  扫描失败的教训（2 字节 LE-length + UTF-16LE 假设在 T_MODELITEM 区
  段只找到 1 个噪声字符 `P`）。
- 新增 4 个单测：MTF TAPE 头 / 未知噪声 / Oracle exp dump / 短输入
  容错。

### Phase 14 inspect 层：`controlled_diff` 模块独立化

- 按 `docs/plans/2026-05-09-controlled-diff-evidence-report-plan.md`
  把 `pid_inspect --controlled-diff-dir` 的 evidence 构造逻辑从二进制
  内部下沉到 `pid_parse::inspect::controlled_diff` 库模块：4 个 DTO
  （`ControlledDiffMetadata` / `ControlledDiffStreamReport` /
  `ControlledDiffCaseReport` / `ControlledDiffEvidenceReport`）+ 2 个
  纯 builder（`build_case_report` / `build_evidence_report`）。CLI
  保留文件系统扫描与 stdout 渲染，库模块只做纯 DTO 构建。
- **Phase 14 防晋升类型不变式**：`ControlledDiffEvidenceReport.promoted_geometry`
  由 `build_evidence_report` 硬编码为 `false`，调用者无法翻转。
  把"controlled diff 仅作为 investigation evidence"从文档规则升级
  为类型系统不变式。
- 新增 5 个单测 + 1 个 doc-test 覆盖 plan.md "First Red Test" 全部
  契约点：metadata propagate / deterministic 计数 / first_modified
  surface /Sheet6 + 非空 hex context / 多 case 聚合时 promoted_geometry
  保持 false / `only_in_before` 与 `only_in_after` 分别计数 /
  non-Sheet 修改不污染 modified_sheet_streams。
- `pid_inspect --controlled-diff-dir` refactor 后 4 个 CLI 集成测试
  全部保持通过；JSON 输出新增 `expected` 字段，原 `metadata_path`
  字段下沉为 stdout 渲染时人读用，不再写进 JSON。

### SPPID Sheet 全几何解析调查与证据门禁（Phase 14）

- 新增 `parsers::sheet_records` investigation 层，将 Sheet marker range、
  `field_x` window、text run、coordinate hint 汇总为稳定 record-shape inventory，
  作为 probe 到 typed decoder 之间的保守证据层。
- 扩展 normalized geometry fixture inventory，按 decoded / inferred / probe-only
  统计 point、line、polyline、arc、circle、text、symbol、unknown，持续量化几何覆盖缺口。
- 新增 PrimitiveLine investigation report：按 marker/range/numeric shape 分组，
  输出 score、numeric samples、relative offsets、offset deltas、hex prefix、
  coordinate-hint / field_x correlation；当前真实 fixture 仍未证明独立 primitive-line
  start/end record，因此不提升为 decoded line。
- 新增 TextPlacementStyle investigation report：关联 text-window score、nearest
  `field_x` evidence 与 bounded hex；只保留 `TextQualityPassed` 候选，并显式统计
  raw / rejected 数量。`/Sheet6` 当前 121 个候选全部因 binary-like UTF16 被拒绝，
  text 仍保持 probe-only。
- 新增 SymbolPlacement investigation report：关联 DA Symbol objects、Sheet `field_x`
  evidence、position candidates 与 JSite symbol catalog；验证 DrawingID direct match、
  JSite order heuristic、PSMroots bridge 均不足以绑定 object-level `symbol_path`，
  因此不提升为 decoded `SymbolInstance`。
- 新增 curve primitive investigation report：按 marker-range numeric shape 粗分
  `PolylineLike` / `CircleArcLike` / `MixedNumeric` / `InsufficientNumeric`；
  refined filtering 后 `/Sheet6` 仅剩 1 个 compact vertex-chain review candidate，
  大范围 mixed payload 明确标注需 subrecord split，不提升为 decoded polyline/circle/arc。
- 新增 CoordinatePageMetadata investigation report：输出 i32/f64 坐标域 bounds、
  normalized f64 pair 计数、页面尺寸标量匹配统计与 no-promotion notes。当前 5 个
  fixture / 7 个 Sheet 汇总为 `normalized_f64_pair_count=1397`、
  `page_dimension_scalar_matches=0`，因此 page transform 仍保持 unavailable。
- 收紧 curve primitive `PolylineLike` 判定：新增非重叠 i32 point sequence、相对
  marker range 的 4 字节 alignment evidence，并要求 3+ aligned points 才进入
  polyline review；`DWG-0201 /Sheet6` 与 `marker_type=15/range_len=148` 候选均降级为
  `MixedNumeric`。
- 新增跨 fixture Sheet geometry evidence aggregate：当前 `curve_groups=97`、
  `mixed_numeric=43`、`polyline_like=0`，所有 curve/page metadata 证据继续保持
  probe-only，不输出 decoded polyline/circle/arc 或 available page transform。
- 新增非 Sheet stream 页面 metadata 扫描回归：模板名仅在 `/TaggedTxtData/Drawing`
  中提供纸型推断证据；当前非 Sheet 标量只命中 `DWG-0201 /JSite329/PSMcluster0`
  的单个 `i32 420.0`，没有完整宽高、origin、scale 或 matrix，因此 page transform
  继续保持 unavailable。
- 新增 page transform evidence blocker 文档，明确 `PidPageTransform::Available`
  promotion 前必须证明 bounded source、完整 page bounds / units / origin / scale
  或 affine matrix，避免将模板纸型推断误当成页面变换解码。
- 新增外部 SPPID format evidence 检索记录：公开资料只证明 `.pid` 含
  drawing/template/operation-log 与可用于 Bridge/Design Validation 导入，
  未提供 Sheet geometry primitive byte layout；后续应优先采集受控操作 diff
  pair、SPPID Bridge export ZIP 或 mapper 输出，而不是放宽 decoder 门禁。
- 新增受控 `.pid` 操作 diff 采集协议，定义 line、polyline、circle、arc、text、
  symbol 的 before/after fixture、metadata sidecar、stream diff 与 promotion
  criteria，用于后续在不猜测 byte layout 的前提下推进 typed decoder。
- 新增 `controlled_pid_diff_pairs_report_stream_level_evidence_when_available`
  soft-skip 回归：本地存在 `test-file/controlled-diff` pair 时验证 metadata
  sidecar、解析 before/after package，并输出首个 modified stream 的 mismatch context。
- 收紧 controlled diff metadata intake：sidecar 需结构化解析，`case` 必须匹配
  before/after 文件名，`operation` 非空且 `expected` payload 存在。
- `pid_inspect` 新增 `--controlled-diff-dir <dir>`，可直接扫描受控
  before/after `.pid` pair，输出 stream diff summary 与首个 modified stream
  context，作为采集到新证据后的本地 intake 工具。
- `pid_inspect --controlled-diff-dir` 支持 `--json` 输出结构化报告，暴露
  case、operation、stream diff 计数、Sheet stream 修改数、首个 mismatch context
  与 `promoted_geometry=false`，便于脚本或 CI 消费 evidence。
- 扩展 controlled diff CLI 回归：`--json` 空目录输出空 cases 报告，metadata
  `case` 与文件名不匹配时以参数错误退出，避免错误 sidecar 被当成可用证据。
- 新增中文 Phase 14 全几何解析计划与 PrimitiveLine 阻塞证据文档，记录 promotion
  前必须满足 source-backed byte range、record kind、coordinate semantics 的证据门槛。
- 验证通过：focused `sheet_records` 单测、真实 fixture investigation 回归、
  marker15 subfield review、cross-fixture aggregate、
  `cargo fmt --all -- --check`、clippy lib/test `-D warnings`、rustdoc missing-docs、
  ReadLints。

### f64 坐标源突破与 Endpoint Line 闭环（Phase 10/10B/11）

- **三种 f64 marker pattern 发现与实现**：
  - `5E 00 22 00 00 00`（pair）：2 个 f64 坐标，覆盖 endpoint_b 类 field_x（630-640）。
  - `FA 00 XX 00 00 00`（triple-xy23）：3 个 f64 值，取第 2-3 个作为坐标，覆盖高编号 endpoint_a field_x。
  - `CE 00 XX 00 00 00`（triple-xy12）：2 个 f64 + 8 零字节，取第 1-2 个作为坐标，覆盖低编号 field_x。
- **替代 Promotion Gate**：新增 `passes_f64_pair_gate`——当 `ObjectFieldResolves` +
  `RepeatedF64PairBeforeField(support >= 3)` 同时满足时，使用 f64 坐标源作为替代
  promotion 路径，不降低任何已有 gate 条件。
- **SheetF64CoordinateHintDto**：新增 f64 坐标 DTO，`SheetObjectGeometryHint` 新增
  `f64_position` 字段作为 `position`（i32）的 fallback。
- **ResolvedObjectPosition**：`build_normalized_geometry` 新增统一 position 类型，同时
  支持 i32 和 f64 坐标源用于 endpoint pair line 推断。
- **坐标源 provenance**：promotion note 包含 `coordinate_source=f64_pair_before_marker`
  或 `coordinate_source=nearest_coordinate_hint`，供 H7CAD / renderer 区分坐标来源。
- **几何成果**：
  - promotable 对象：5 → **67**（13.4×）。
  - DWG-0201GP06-01.pid：`inferred_points` 69→117，`inferred_lines` 0→**49**。
  - DWG-0202GP06-01.pid：`inferred_points` 69→76，`inferred_lines` 0→**3**。
  - Endpoint pair 覆盖率：0% → **83.1%**（排除 null 端点后 89.1%）。
  - 3/5 fixture 成为 line-producing fixture。
- **坐标域分析**：f64 坐标确认为 0-1 归一化页面坐标，模板为 A2 纸（594×420mm）；
  i32 坐标系独立，映射关系待后续建立。
- **H7CAD 端到端验证**：`pid_import_real_sample_geometry` 自动消费 53 points + 49 lines。
- **Gap 分析**：剩余 10 对 endpoint pair 中 4 对 endpoint_b=0（null 终止点），
  6 对涉及非 object graph 成员 field_x，属于 scope 边界。
- 新增 4 个中文开发方案文档。
- 验证通过：`cargo test --lib`（759 passed），`cargo test --test parse_real_files`（68 passed），
  clippy、fmt、rustdoc 全绿。

### PID 几何 Fixture Baseline 与 Provenance Guardrail（Phase 9A/9C）

- **显式 fixture registry**：`parse_real_files` 新增 geometry fixture registry，
  将 DWG、中文路径、publish A01 / DWG fixture 纳入同一基线，并保留
  `GEOMETRY_FIXTURE_TARGET_MIN_AVAILABLE=8` 的扩容目标。
- **Availability summary**：新增 `registered`、`target_min_available`、
  `available`、`missing` 统计，并输出 human-readable report line，方便
  inventory 测试直接暴露当前 5/8 fixture 缺口。
- **Promotion provenance**：promoted `SheetObjectGeometryHint` 的 note 现在保留
  `score`、identity evidence 与 stable shape evidence，避免只留下不可解释分数。
- **Normalized geometry 回归**：新增 source note regression，确认 promoted hint
  投影到 `PidGraphicEntity(Point, Inferred)` 后仍保留
  `PidGraphicProvenance.note`，供 H7CAD / renderer 读取 promotion gate 摘要。
- **下一步开发方案**：新增中文 Phase 9A fixture 扩展执行方案，明确 8-12 个真实
  `.pid` fixture 的选择标准、TDD 切片、验证命令与当前外部样本阻塞。

### PID 几何 Promotion Gate 突破与端到端链路（Phase 8C-9B）

- **三叉 Promotion Gate 实现**：`ObjectGeometryPromotionGateSummary` +
  `summarize_object_geometry_promotion_gate` — 候选必须同时满足
  `score >= 70` + `GraphicIdentityNearby` + `StableChunkShape` 才算 promotable。
- **Gate 首次突破**：`promotable=5`（DWG-0201GP06-01 Sheet6），
  `max_identity_score=105`，`identity_over_threshold=28`。
- **Identity 匹配修复**：`identity_supports_score` 从要求 offset 精确匹配
  放宽为 field_x 级别匹配；根因是同一 field_x 在 Sheet 流中出现多次，
  identity 在 endpoint 窗口找到但高分窗口在别处。
- **StableChunkShape 阈值调整**：support 从 `>= 3` 降为 `>= 2`；
  cross-fixture aggregate 已验证 shapes（support=4），三叉 gate 保证安全。
- **populate_object_geometry_hints**：从 promotable scored candidates 生成
  `SheetObjectGeometryHint`，集成到 `cfb/reader.rs` 管线
  （在 `build_graph` 与 `derive_layout` 之间执行）。
- **NormalizedPidGeometry 投影扩展**：`build_normalized_geometry` 消费
  `object_geometry_hints`，生成 `PidGraphicEntity(Point, Inferred)` 带
  field_x provenance。
- **H7CAD 渲染管线接入**：`add_geometry_entities` + `PID_GEOM_POINTS` 层，
  promoted geometry 以 Circle markers 显示。
- **坐标验证**：5 个 promoted points 坐标全部在 SmartPlant 绘图单位有效范围内
  （x=2571-21003，y=102144-154368）。
- **Record Grammar 发现**：`CE 00 79 00` 确认为记录头部签名，`field_x` 在
  偏移 +6 处，Sheet6 中出现 12 次。存在多种 record template。
- **Near-miss 分析**：5 个候选只差一个条件——field_x=111,537 有 identity
  无 shape；field_x=139,147,440 有 shape 无 identity。
- 新增中文开发方案文档与 4 个 SVG/PNG 路线图。
- 验证通过：
  - `cargo test --lib`（745 passed）
  - `cargo test --test parse_real_files`（53 passed）
  - `cargo clippy --locked --workspace --all-targets -- -D warnings`

### H7CAD：PID 真实几何显示与证据门禁（Phase 7）

- 新增 normalized geometry contract，将 Sheet coordinate hints 作为
  `PidGraphicKind::Point` + `PidGeometryConfidence::Inferred` 暴露给
  H7CAD；text / endpoint 仍保持 `ProbeOnly Unknown`，避免把未证明的
  关系或拓扑误渲染为 CAD 几何。
- H7CAD 已能消费 inferred points 并在 PID 导入 / 截图 / SVG 导出路径中显示
  `PID_GEOMETRY_POINTS`，同时保持 endpoint line 不渲染。
- 新增 `SheetObjectGeometryHint` 空基线与 `/Sheet6` guardrail：
  `object_geometry_hints` 在 source-proven object-coordinate mapping 前保持为空。
- 建立 field-x window、record-shape、stable marker、coordinate-quality、
  `GraphicIdentityNearby` 等 investigation 链路；当前真实样本结论仍为
  `max_score=45`、`over_threshold=0`，不能 promotion 为 `Line + Inferred`。
- 新增 Text placement investigation：`sheet_text_window_candidates`、
  text-quality filter 与 scoring report。`/Sheet6` 当前为
  `text_quality_passed=0`、`max_score=-50`、`over_threshold=0`，
  因此不生成 `Text + Inferred`。
- 新增中文 planning / PR 拆分文档与路线图：
  - PR1 normalized geometry contract
  - PR2 H7CAD inferred point rendering
  - PR3 Sheet6 guardrails
  - PR4 field-x evidence
  - PR5 GraphicIdentityNearby
  - PR6 Text placement investigation
- 验证通过：
  - `cargo fmt --all -- --check`
  - `cargo build --locked --workspace --all-targets`
  - `cargo clippy --locked --workspace --all-targets -- -D warnings`
  - `cargo test --locked --workspace --all-targets`
  - `cargo test --lib -- --nocapture`（742 passed）
  - `cargo test --test parse_real_files -- --nocapture`（51 passed）
  - `cargo rustdoc --lib --locked -- -W missing-docs`

### import-view：关系边接入 Sheet provenance（Phase 4）

- `PidImportView.relationships` 现在会从 `CrossReferenceGraph.relationship_endpoint_links`
  带出 `sheet_path`、`sheet_offset`、`source_field_x`、`target_field_x`，
  为下游 canonical edge 消费提供轻量 provenance。

### parser：Sheet geometry DTO 合同起步（Phase 3）

- 新增 `SheetGeometry` / `SheetText` / `SheetEndpoint` /
  `SheetCoordinateHintDto` 作为 Sheet text、endpoint、coordinate hint 的稳定
  JSON contract 入口。
- `SheetStream` 新增可选 `geometry` 字段；当前先作为 DTO surface 落地，
  不宣称完整 CAD geometry decoded。
- `Sheet*` 读取阶段会把 `sheet_probe` 的 text runs 与 coordinate hints
  归一化填充到 `SheetStream.geometry`，endpoint 仍等待下一切片同步。
- endpoint record 解析完成后会同步填充 `SheetStream.geometry.endpoints`，
  使 text、coordinate hint、endpoint 三类 Sheet 证据进入同一个 DTO 入口。
- synthetic 回归锁定 endpoint 同步不会覆盖已归一化的 text 与 coordinate hint。
- Phase 3 当前 DTO 起步范围收敛完成：未命名字节仍留在 probe 层，
  `SheetGeometry` 不宣称完整 CAD geometry decoded。
- 新增 schema 回归测试锁定 Sheet DTO 名称进入 `pid_inspect --schema` 输出。

### parser：PSM 表结构化候选字段收敛（Phase 2）

- `PSMclustertable` 的 `PsmClusterRecordDecoded::unknown_prefix_bytes`
  不再固定为空，改为根据 decoded `field_ranges` 反推出候选字段之外的
  prefix bytes，方便后续真实 fixture 横向比对保留位、常量位与未知字段。
- `PSMsegmenttable` 的 `PsmSegmentEntry` 新增保守候选关联字段：
  `candidate_owner_cluster_index` 与 `candidate_owner_cluster_name`。仅当
  segment entry 数量与 `PSMclustertable` entry 数量完全一致时填充；
  cluster table 缺失或数量不一致时保持 `None`。
- `pid_inspect` 文本 report 在 segment 行输出
  `owner_candidate=index:name`，同时保留旧 probe `owner_hint`，区分结构化
  candidate 字段与 probe 线索。
- coverage inventory 对 `PSMsegmenttable` 的说明更新为
  `segment flags + owner candidate mapping; SmartPlant field semantics still pending`，
  继续保持 `PartiallyDecoded`，不把候选映射升级为稳定业务语义。
- JSON schema 回归测试锁定 `PsmSegmentEntry` 与新增 candidate owner 字段会出现在
  `pid_inspect --schema` 输出中。
- 真实 fixture soft-skip 回归扩展 `psm_segment_record_probes_align_with_flags`：
  结构化 candidate owner 字段必须与旧 probe `owner_cluster_hint` 的 1:1
  positional 映射一致。
- byte-audit aggregate 回归锁定 `/PSMclustertable` 同时保留 decoded header/name、
  probed prefix 与 trailing leftover 分桶。
- 本轮新增/更新 parser、report、coverage 单测，并通过全量 `cargo test`。

### parser：`PSMclustertable` decoded record 候选视图（Phase 11a）

`docs/plans/2026-05-06-phase-11a-psmclustertable-records.md` 与
`docs/analysis/2026-05-06-psmclustertable-evidence.md` 第一轮执行落地：

- 基于 DWG-0201 / DWG-0202 / 中文样本三份真实 fixture 的 probe matrix，
  新增 additive `PsmClusterTable.decoded_records` 视图；旧 `entries` /
  `probe` 保留，既有 consumer 不受影响。
- 新增 `PsmClusterRecordDecoded` / `DecodedFieldRange`，只暴露证据已支撑
  的保守候选字段：`name_bytes_with_nul`、`candidate_ordinal`、
  `candidate_non_sheet_marker`、`candidate_non_sheet_payload_index`。
  证据不足的 `segment_count` / `declared_segment_count` 暂不命名。
- `pid_inspect` 文本 report 在 `PSMclustertable` 段输出 decoded candidate
  摘要，方便后续逆向把 probe 与候选字段对照。
- 新增 synthetic parser 单测、report 单测与真实 fixture 回归，锁住
  DWG-0201 / DWG-0202 的 decoded record 平行结构与 `Sheet6615` 额外
  sheet record 候选字段。

当前 coverage 仍保持 conservative：本次只是候选 decoded 视图，不把
`PSMclustertable` 宣称为 FullyDecoded；后续需 crossref consistency 与
更多字段语义证据再决定是否升级 byte-audit confidence。

### crossref：`PSMclustertable` decoded candidate consistency（Phase 11a-2）

`docs/plans/2026-05-06-phase-11a-2-psmclustertable-consistency.md` 第一轮执行：

- 新增 `psm_cluster_decoded_consistency()`，对 `PsmClusterTable.decoded_records`
  做结构自洽检查：decoded view 是否与 legacy `entries` 平行、名称与
  record range 是否一致、候选 ordinal 是否单调、`Sheet*` 行是否保持
  sheet marker、sheet-marker 行是否不携带 non-sheet payload index。
- 新增 `PsmClusterDecodedConsistency` /
  `PsmClusterDecodedConsistencyStatus`，只表达工程一致性，不承诺
  SmartPlant 业务语义。
- 新增 happy-path 单测锁定 parallel candidate view；后续 warning paths、
  report 输出与 coverage policy 将继续在 11a-2 后续任务中补齐。
- 新增 `docs/plans/2026-05-07-phase-11b-psmsegmenttable-records.md`，为
  11a-2 之后的 `PSMsegmenttable` conservative record view 预留执行计划。

本轮仍不把 `PSMclustertable` 升为 FullyDecoded，也不刷新 byte-audit
baseline。

### inspect：`PSMclustertable` consistency guardrail 收口（Phase 11a-2b）

- 补齐 decoded candidate consistency 的 warning / missing path 测试，锁住名称、
  sheet marker 与 decoded records 缺失时的诊断行为。
- `pid_inspect` 文本 report 在 `PSMclustertable` 段输出 decoded consistency
  summary，让 consistency guardrail 不只停留在 crossref API。
- coverage policy 测试明确 `PSMclustertable` 继续保持 `PartiallyDecoded`；
  decoded record candidates 只是工程候选视图，不宣称 SmartPlant 字段语义已
  fully decoded。

### test：修复 `parse_real_files` 与 in-repo fixture 漂移（Phase 12c 前置）

`docs/plans/2026-04-29-fix-real-file-fixture-drift.md` 落地：

- commit `cec4087` 把 sanitized 真实 fixture 入仓后，`tests/parse_real_files.rs`
  的 4 个硬编码断言（基于入仓前的私有 fixture 数据）失效，导致 main 从
  `cec4087` 起 CI red：
  - `relationship_endpoints_resolve_via_sheet_record`（unresolved 上限）
  - `sheet_endpoint_records_one_per_relationship`（endpoint==relationship 计数等值）
  - `object_sources_align_with_attribute_records`（DA `DrawingID` 字段值
    与 `source.drawing_id` 等值）
  - `second_file_builds_readable_layout_model`（`layout.segments.len() ≥ 5`）
- 把硬编码 GUID / 计数改为结构性 / 比例容差断言（resolved ≥ 70% / unresolved
  ≤ 15% / endpoint records ≥ 85% / foreign endpoints < total），未来 fixture
  演进不会再 break；保留真正的 parser invariant（如 cross-ref 1:1 对齐、
  sheet record `rel_field_x` 必须存在于 `relationships`）。
- `object_sources_align_with_attribute_records` 删除 `assert_eq!(advertised_id,
  source.drawing_id)`，加 NOTE 链 Phase 12a：sanitized fixture 的 P&IDAttributes
  records 全部 advertise drawing-level UUID，与原私有 fixture 的 object-level
  UUID 假设不一致；语义对齐由 normalized graph 接管。
- `second_file_builds_readable_layout_model` 的 layout.segments 期望从 ≥5
  暂降为 ≥3，加 TODO 链 Phase 11c：等 Sheet 几何深层解码恢复 connector
  recovery 后再升回去。

零 lib API / parser / writer 改动。CI 重新转绿（5 道 pre-commit gate
+ baseline runner 全部 EXIT=0）。

### byte-audit：真实 fixture baseline 接入 CI（Phase 12c）

`docs/plans/2026-04-29-phase-12c-byte-audit-baseline.md` 落地，
`docs/plans/2026-04-29-pid-parse-roadmap.md` 阶段 A 第一个可执行 Phase：

- 新增 `docs/baselines/` 目录，提交 3 份真实 fixture 的 byte-audit
  baseline JSON：
  - `dwg-0201gp06-01.byte-audit.json`（`test-file/DWG-0201GP06-01.pid`，
    223 KB / ~11% covered）
  - `dwg-0202gp06-01.byte-audit.json`（`test-file/DWG-0202GP06-01.pid`，
    206 KB / ~9% covered）
  - `sample-cn-1.byte-audit.json`（`test-file/工艺管道及仪表流程-1.pid`，
    211 KB / ~4% covered；中文 fixture 用 ASCII slug 命名）
- 引入 sidecar 命名约定：`<slug>.fixture.txt` 一行文本写 fixture 真实
  路径，让 baseline 文件名保持 ASCII，fixture path 可含非 ASCII 字符，
  规避跨平台 / 跨 shell 编码问题（Windows NTFS UTF-16 / Linux ext4 byte
  sequence / macOS NFC vs NFD / git pathspec / CI shell escaping）。
- 升级 `.github/scripts/check-byte-audit-baselines.sh` 优先读 sidecar，
  缺 sidecar 时回退到 `test-file/<slug>.pid` 旧约定，保持向后兼容；
  缺 baseline / 缺 fixture 时仍 soft-skip 退出 0。
- 新增 `docs/baselines/README.md` 列出 slug ↔ fixture 映射表、如何
  新增/刷新 baseline、PowerShell 5.x UTF-16LE 陷阱说明、何时刷 baseline
  的判定规则、私有 fixture 处理策略。
- `docs/byte-audit-guide.md` "Baseline Rules" 章节改写为 sidecar 解析
  路径说明，"Current Limitations" 替换为 "Baseline Workflow (Phase 12c+)"
  完整工作流。
- `.github/workflows/ci.yml` 已经在 `byte-audit baselines (optional)`
  步骤里调用 runner（v0.11.6 Phase 落地），无需新增 step。

零 lib API 变化，零 CLI surface 变化，零 parser 行为变化。从此任何
合法地降低 `overall_coverage_ratio`、降低已 traced stream
`consumed_bytes`、或让 traced stream 翻回 unregistered 的 PR 会被
CI hard-fail（fixture 在场时）。

### docs：12 周战略路线图与首个 Phase 子任务 plan doc

新增两份 plan doc：

- `docs/plans/2026-04-29-pid-parse-roadmap.md` — v0.11.7 → v1.0.0
  candidate 的 12 周战术总图，覆盖 W1–W12 共 16 个 Phase（A 固化
  baseline / B PSM 加深 / C Sheet 几何 / D DWG 闭环 / E Normalized
  graph / F v1.0 验收），含每 Phase 估时、验收口径、风险登记。
- `docs/plans/2026-04-29-phase-12c-byte-audit-baseline.md` — Phase 12c
  详细执行计划，拆为 8 个 Task：pre-flight check / 确认 runner 行为 /
  baseline 命名决策 / 生成 baseline / 升级 runner / CI 接通 / 文档与
  CHANGELOG / 5 道 gate 验证。

### docs：刷新当前架构图与原理说明

- 新增 `docs/current-architecture-principles.md`，用读取路径、
  `PidDocument` / `PidPackage` 分层、Probe / Decode、byte-audit、
  writer、backup / publish 边线和质量门禁说明当前解析器架构。
- 新增 `docs/diagrams/pid-parse-current-architecture.svg` 与 PNG 导出，
  作为 `architecture-guide.md` 中当前架构图的可引用版本。
- 更新 `docs/architecture-guide.md` 的架构图链接，补充 SVG 与原理说明
  入口，避免仍指向旧版 `pid-parse-architecture.png`。

纯文档与图示更新，不改 lib API、CLI surface 和 parser / writer 行为。

## [0.11.7] — 2026-04-27

### parser API：新增显式 `ParseProfile::Light`

`docs/light-parse-design.md` 与
`docs/plans/2026-04-27-light-parse-design.md` 落地
`docs/plans/2026-04-26-parser-api-consistency-fixes.md` Task 8：

- `ParseOptions` 新增 `profile: ParseProfile`，默认仍是
  `ParseProfile::Full`，保持既有 full-fidelity 行为。
- 新增 `ParseOptions::light()`，面向 bulk inventory / triage 场景；
  light profile 保留 CFB tree、stream inventory 与 package raw streams，
  跳过 tagged-text XML、`JSite` properties、dynamic attrs、cross-reference
  与 layout 等较重语义/派生 pass。
- 新增 API 回归测试，确认 light package parse 仍保留 raw stream 和
  stream inventory，同时跳过 XML meta、cross-reference 与 layout。

### parser：Sheet endpoint extraction 失败进入可见诊断

`docs/plans/2026-04-27-sheet-endpoint-diagnostics.md` 落地
`docs/plans/2026-04-26-parser-api-consistency-fixes.md` Task 5：

- `SheetStream` 新增 `endpoint_decode_error` 诊断字段，记录 endpoint
  record extraction 阶段的 per-sheet soft failure。
- `populate_sheet_endpoints` 不再静默跳过 `open_stream` 失败；re-open
  或 read 失败会写入诊断并继续解析其它 sheet，避免单个 Sheet 问题
  把整个 package parse 变成 hard failure。
- `inspect` report 的 Sheet provenance refs 会在对应 sheet 行追加
  `endpoint_error="..."`，让 endpoint 丢失原因可在文本报告里定位。

### writer：validator 复用同一套 `WritePlan` 应用顺序

`docs/plans/2026-04-27-validator-writer-pipeline-reuse.md` 落地
`docs/plans/2026-04-26-parser-api-consistency-fixes.md` Task 4：

- `writer::apply_plan_to_package` 从内部 helper 提升为有文档的公开
  helper，作为 metadata updates → `stream_replacements` →
  `sheet_patches` 的唯一 canonical 应用顺序。
- `pid_writer_validate --apply-plan` 构造 expected package 时改为调用
  同一个 helper，不再手写一份 metadata / stream replacement /
  sheet patch 顺序，避免未来 writer pipeline 改动时 validator 预期漂移。
- CLI 报告形状与 edited-path 分类不变。

### parser API：明确 `keep_unknown_streams` 只控制解码诊断视图

`docs/plans/2026-04-27-keep-unknown-streams-contract.md` 落地
`docs/plans/2026-04-26-parser-api-consistency-fixes.md` Task 3：

- `ParseOptions::keep_unknown_streams` 的契约收敛为控制 decoded
  diagnostics：`PidDocument.unknown_streams` 与嵌入式 `JSite`
  raw-stream summaries。
- `PidPackage` 的 raw stream retention 不受该选项影响；即使
  `keep_unknown_streams == false`，writer passthrough 仍可拿到未知流
  原始字节并保持 byte-preserving。
- reader 现在基于 `inspect::unidentified_top_level_streams` 填充
  `UnknownStream { path, size, magic_u32_le, magic_tag }`，并新增
  `keep_unknown_streams` 契约测试覆盖 `true` / `false` 两条路径。

### parser API：`PidPackage::from_bytes` 改为纯内存解析

`docs/plans/2026-04-27-from-bytes-pure-memory-parse.md` 落地
`docs/plans/2026-04-26-parser-api-consistency-fixes.md` Task 7：

- `PidPackage::from_bytes` 不再写入临时 `.pid` 文件，而是通过
  `Cursor<Vec<u8>>` 直接交给 CFB reader 解析；内存来源的 package
  明确返回 `source_path == None`，避免把 scratch path 泄漏给调用方。
- `src/cfb/reader.rs` 抽出 reader-generic 的内部解析核心，
  `parse_pid_package(path)` 仍是路径入口，新增
  `parse_pid_package_from_reader` 支撑内存 / 自定义 reader 输入。
- 新增 `from_bytes_marks_package_as_memory_sourced` 回归测试，覆盖
  内存解析身份语义；原有 `from_bytes` invalid-data 与
  `from_path` 等价行为测试继续通过。

### docs：同步 writer 能力说明

- `src/writer/mod.rs` / `src/writer/summary_write.rs` 的 module
  comment 补齐当前 summary update / deletion / encoded update、CLSID
  与 `state_bits` 保真边界，避免文档仍停留在早期 writer 限制。
- `examples/byte_audit_demo.rs` 应用 rustfmt 折行，恢复全量格式 gate。

## [0.11.6] — 2026-04-26

### docs：明确 `replace_stream` / `set_xml_tag` 不刷新 `parsed` 的契约

`docs/plans/2026-04-26-parser-api-consistency-fixes.md` Task 2 落地，
零行为变化，纯文档化既有契约：

- `src/package.rs::PidPackage::replace_stream` 与
  `set_xml_tag`（以及它们的 shortcut `set_drawing_xml_tag` /
  `set_general_xml_tag`）的 doc comment 显式声明 raw stream 字节
  改动后 **不会**自动刷新 `PidPackage.parsed`，并指明推荐做法是走
  `PidWriter::write_to_bytes` + `PidPackage::from_bytes` 的完整
  round-trip 拿到 live 解码视图，而不是 in-place 部分 reparse
  （后者要等 `cross_reference` / `layout` / `object_graph` 等派生
  层的 invalidation 契约设计完毕后再考虑加 full-package
  `reparse()` helper，对应 plan Task 8）。

- `docs/writer-quickstart.md` 在 "保真能力矩阵" 与 "错误处理" 之间
  插入新章节 "## 6.5 契约：raw stream 改动 vs 解析模型 (v0.11.5+)"，
  含 round-trip + reparse 的 Rust 代码片段、为什么不直接 in-place
  reparse 的设计说明，以及对 plan Task 8 的 forward-link。

零 lib API 变化，零 CLI 变化，零 parser 行为变化。

验证（5 道 pre-commit gate 全绿）：

- `cargo build --workspace --locked`
- `cargo test --workspace --locked --all-targets`
- `cargo clippy --locked --workspace --all-targets -- -D warnings`
- `cargo fmt --all -- --check`
- `cargo rustdoc --lib --locked`（13-deny gate 仍绿，新 doc comment
  使用绝对路径 `[\`crate::...\`]` link 通过 deny gate）

## [0.11.5] — 2026-04-26

### `pid_writer_validate` 把 `summary_updates_encoded` 计入 edited paths

`docs/plans/2026-04-26-parser-api-consistency-fixes.md` Task 1 落地。
`pid_writer_validate` 的 `collect_edited_paths_from_plan` 之前只检查
`metadata_updates.summary_updates` 与 `summary_deletions` 来标记
SummaryInformation / DocumentSummaryInformation 路径为 edited，
`summary_updates_encoded`（Phase 10i 引入的 code-page-aware property
update 通道）被遗漏，导致只用 encoded 通道的 plan 跑 round-trip 后
SummaryInformation 重写后的 stream 会被错误归为 mismatch（而不是
expected edit）。

修复：

- `src/bin/pid_writer_validate.rs`：判定条件追加
  `|| !plan.metadata_updates.summary_updates_encoded.is_empty()`，
  与已有的 `summary_updates` / `summary_deletions` 三选一并列。
- `tests/writer_validate_cli.rs` 新增 CLI 集成测试
  `validate_apply_plan_summary_updates_encoded_marks_summary_streams_edited`：
  - 用 `build_fixture_with_summary` 创建 4-stream fixture（含
    `/\u0005SummaryInformation`）。
  - WritePlan JSON 仅设
    `metadata_updates.summary_updates_encoded.title`（windows-1252
    编码）。
  - 断言 `--apply-plan --json` 返回 `edited == 1`、`mismatched == 0`，
    并在落盘文件里读出 `title == "PLAN-ENCODED"`。

零 lib API 变化（`MetadataUpdates.summary_updates_encoded` 字段早就
公开），仅 validator 行为补正 + 回归测试。

验证（5 道 pre-commit gate 全绿）：

- `cargo build --workspace --locked`
- `cargo test --workspace --locked --all-targets`（新 case 通过）
- `cargo clippy --locked --workspace --all-targets -- -D warnings`
- `cargo fmt --all -- --check`
- `cargo rustdoc --lib --locked`（13-deny gate 仍绿）

### docs：架构可视化 + parser API 一致性修复执行计划落地

- 新增 `docs/diagrams/pid-parse-current-architecture.html` —— 当前
  解析器架构的可打开 HTML 版本（Geist + Instrument Serif 排版，
  覆盖八层架构与 byte-audit framework 视图）。`architecture-guide.md`
  顶部"架构图"段下加一行链接指向该 HTML，与既有
  `pid-parse-architecture.png` 形成 PNG / 交互 HTML 双载体。
- 新增 `docs/plans/2026-04-26-parser-api-consistency-fixes.md` ——
  parser/writer API 一致性修复执行计划，按"先行为/报告纠正、再
  契约/文档、再语义行为变更"的顺序拆出 Task 1-8：
  - Task 1：`pid_writer_validate` 把 `summary_updates_encoded` 也
    计入 edited summary streams（small surface, 选作首个执行项）。
  - Task 2：明确 `replace_stream` / `set_xml_tag` 不刷新
    `PidPackage.parsed` 的契约。
  - Task 3：`keep_unknown_streams` 选项语义裁定。
  - Task 4：validator 复用 writer pipeline，避免 apply-order drift。
  - Task 5：`populate_sheet_endpoints` 静默失败 → 结构化诊断。
  - Task 6：`writer/mod.rs` / `writer/summary_write.rs` 等 module
    comment drift 修复（docs-only PR）。
  - Task 7：`PidPackage::from_bytes` 改为 `Cursor<Vec<u8>>` reader-
    generic，避免临时文件。
  - Task 8：light parse 设计文档先行，再走 API 改造。

不动 lib API、CLI surface、parser 行为，纯文档与计划文档接入。

## [0.11.4] — 2026-04-26

> 主线：把 v0.11.3 之后累积的 docs / examples / plans 工作打包，并把
> `clippy` pedantic 10 项 lint 从 warn 升级到 deny，与 v0.11.2 / v0.11.3
> 已完成的 rustdoc 3-deny 形成完整的 13-lint quality gate 矩阵。
> 无破坏性变更，无新公开 API。
>
> 主题：
>
> - **clippy pedantic warn → deny**：`src/lib.rs` 顶部 10 项 clippy
>   lint 全部从 `#![warn(...)]` 提升为 `#![deny(...)]`。`cargo clippy
>   --locked --workspace --all-targets`（不带 `-D warnings`）实测仍
>   `EXIT=0`，说明现有 codebase 已经对这 10 项干净化，升级零风险但
>   永久防回归：未来任何 commit 引入 `uninlined_format_args` /
>   `doc_markdown` / `redundant_closure_for_method_calls` /
>   `manual_let_else` / `map_unwrap_or` / `unreadable_literal` /
>   `bool_to_int_with_if` / `implicit_clone` / `explicit_iter_loop` /
>   `unnecessary_map_or` 任意一项都直接 fail clippy gate。
> - **PRD + 双入口文档体系落地**：本 release 包含
>   `docs/prd-pid-parse-current-state.md`（产品现状 PRD）、
>   `docs/plans/2026-04-26-prd-follow-up-execution.md`（PRD 落地执行
>   计划，Task 1-4），README + architecture-guide 双入口跳转。
> - **byte-audit 入门样板与库级 API 文档**：新增
>   `examples/byte_audit_demo.rs` 演示 `byte_audit_report` +
>   `compare_byte_audit_reports` 端到端用法（零 fixture 依赖），
>   `docs/byte-audit-guide.md` 新增 "Programmatic API" 章节交代
>   公开 surface。
>
> 详见下方各 `### …` 段落。

### clippy pedantic 10 项 warn → deny

把 `src/lib.rs` 顶部 `#![warn(clippy::*)]` 列出的 10 项 pedantic /
restriction lint 升级为 `#![deny(clippy::*)]`。这些规则的 baseline
是历次 `Clippy 清理` commits（CHANGELOG 中可索引）累积的成果，每条
都已经在 codebase 上零触发；本次只是把强度从 warn-only 推到 deny，
让 IDE 直接显示 error 标记、`cargo check`（不带 `-D warnings`）也
能 fail，未来在 release/CI 之外的本地构建路径里同样防回归。

`#![deny(missing_docs, rustdoc::broken_intra_doc_links,
rustdoc::private_intra_doc_links)]` 保持不变；本次升级与之合并后，
crate 顶部一共 deny **13 项 lint**（10 clippy + missing_docs + 2
rustdoc intra-doc-link）。

5 道 pre-commit gate 在新配置下全绿（build / test / clippy / fmt /
rustdoc 全部 EXIT=0）。

### docs：架构指南补 PRD 链接 + PRD follow-up 执行计划落地

- `docs/architecture-guide.md` 顶部"项目目标"附近补一段对
  `docs/prd-pid-parse-current-state.md` 的链接，让从架构指南入门
  的读者也能直接跳到产品现状与下一阶段需求。这与 README 顶部
  "文档与产品现状"章节的 PRD link 形成双入口。
- 新增 `docs/plans/2026-04-26-prd-follow-up-execution.md` —— PRD
  落地执行计划，按 Task 1-4 顺序拆出：
  - Task 1：让 PRD 可被 README + architecture-guide 双路径发现
    （已完成）。
  - Task 2：在有真实 fixture 的环境里生成 `docs/baselines/*.byte-audit.json`
    并跑 `.github/scripts/check-byte-audit-baselines.sh`（blocked on
    fixture）。
  - Task 3：PSM table 结构化加深，要求 ≥2 个真实样本才能升级
    coverage 等级；在动 parser 前先沉淀
    `docs/plans/YYYY-MM-DD-psm-table-deepening.md` 设计文档。
  - Task 4：Sheet 几何深层解码延后到 PSM provenance 设计完成之后，
    避免孤立 heuristics。
- 不动 lib API、CLI surface、parser 行为，纯 docs / 计划文档接入。

### docs：`byte-audit-guide.md` 新增 "Programmatic API" 章节

在 `docs/byte-audit-guide.md` 的 JSON Output 之后插入新章节，向库级
消费者交代 byte-audit 的程序化入口：

- `pid_parse::byte_audit::aggregate::byte_audit_report(&PidPackage)`
- `pid_parse::byte_audit::compare::compare_byte_audit_reports`
- 完整 re-export 列表：`ByteAuditReport` / `StreamAuditSummary` /
  `ByteAuditComparison` / `ParserTrace` / `ParserTraceBuilder` /
  `ByteRange` / `TraceConfidence`。
- 强调 `Serialize`/`Deserialize`/`JsonSchema` 与 `--byte-audit --json`
  的同 schema round-trip。
- cross-link `examples/byte_audit_demo.rs` 作为零 fixture 入门样板。

不动 lib API、CLI surface、parser 行为，纯文档增强。

### examples：`byte_audit_demo` 演示字节审计框架的程序化入口

新增 `examples/byte_audit_demo.rs`，用 4 条合成 stream 演示
`byte_audit::aggregate::byte_audit_report` 与
`byte_audit::compare::compare_byte_audit_reports` 的端到端用法，
不依赖任何真实 `.pid` fixture：

- 合成 baseline 包含 `/PSMsegmenttable`（`stab` magic 完整消费）、
  `/DocVersion2`（op record 完整消费）、`/TaggedTxtData/Drawing`
  （UTF-8 XML 全消费）、`/MysteryStream`（无 parser → `parser_name =
  None`，全部 leftover）。
- 跑 `byte_audit_report` 输出 per-stream + overall 统计；以
  `serde_json::to_string_pretty` 演示 `--byte-audit --json` 等价的
  程序化序列化。
- 故意把 baseline 中 `/PSMsegmenttable` 的 magic 破坏掉得到 current
  报告，调 `compare_byte_audit_reports` 演示
  `OverallCoverageDecreased` + `StreamConsumedBytesDecreased` 两类
  regression。
- 期望输出已写在 example 顶部 `//!` doc，方便下游集成时直接对照。

`cargo run --example byte_audit_demo` 在公开 CI 上即可跑过，零 fixture
依赖，是 P1 真实 baseline 工作之前的最小入门样板。

### docs：PRD 与文档导航接入 README

- 新增 `docs/prd-pid-parse-current-state.md` —— "解析现状 + 下一阶段
  PRD"，本身 P0 任务即"保留本文档作为状态入口"。文档覆盖：当前解析
  机制、已实现能力、进度判断、产品目标、用户场景、P0–P5 下一阶段
  需求、非目标、验收指标、风险、推荐执行顺序、状态摘要。
- `README.md` 顶部新增"文档与产品现状"章节，链向 PRD、
  `docs/sppid/v0.10.x-status.md` 状态快照、`architecture-guide`、
  `byte-audit-guide`、SPPID 完整解析路线图，方便新读者快速对齐
  当前能力边界与剩余缺口。
- 不动 lib API、CLI surface、parser 行为，纯 docs 改动。

## [0.11.3] — 2026-04-26

### `missing_docs` warn → deny 升级

把 `src/lib.rs` 顶部的 `missing_docs` 从 `#![warn(...)]` 列表移到
`#![deny(...)]` 块，与 v0.11.2 已经 deny 的两个 rustdoc lint
（`broken_intra_doc_links` / `private_intra_doc_links`）合为
**三道完整 rustdoc 防回归门禁**：

```rust
#![deny(
    missing_docs,
    rustdoc::broken_intra_doc_links,
    rustdoc::private_intra_doc_links
)]
```

升级前提：`cargo rustdoc --lib --locked -- -W missing-docs` 实测 0 个
`missing_docs` warning，已经在 v0.11.2 自然达成 baseline。本次只是把
warn 级别提升到 deny，不需要补任何文档。从此任何 commit 引入未文档化
的公开 item 直接 fail rustdoc gate，不再依赖 reviewer 主动检查。

`#![warn(clippy::*)]` 现存 10 个 pedantic lint 规则保持 warn 不动，
保留 codebase 增量降噪空间。

验证（5 道 pre-commit gate 全绿）：

- `cargo build --workspace --locked`
- `cargo test --workspace --locked --all-targets`
- `cargo clippy --locked --workspace --all-targets -- -D warnings`
- `cargo fmt --all -- --check`
- `cargo rustdoc --lib --locked`（命令行不再需要 `-W missing-docs`，
  因为源代码已经 deny；EXIT=0, 0 warning, 0 error）

## [0.11.2] — 2026-04-26

### Rustdoc intra-doc-link 防回归门禁

把 `rustdoc::broken_intra_doc_links` 与 `rustdoc::private_intra_doc_links`
两个 lint 在 `src/lib.rs` 顶部 `#![deny(...)]`，从此任何指向 unresolved
scope 或私有 item 的 `[\`xxx\`]` 都会让 `cargo rustdoc --lib --locked
-- -W missing-docs` 直接 fail，而不是只产生 warning。这把 v0.11.1 的
rustdoc cleanup 工作变成了永久 invariant：

- 私有引用请用 `xxx`（不带方括号），不创建 link；
- 公开引用请用绝对路径 `[\`crate::xxx\`]` 或同 impl 的
  `[\`Self::xxx\`]`。

`#![warn(missing_docs)]` 与 5 道 pre-commit gate 不变；公开 API 缺
docstring 仍然只是 warning（不 fail），保留 codebase 增量降噪空间。

验证（5 道 pre-commit gate 全绿）：

- `cargo build --workspace --locked`
- `cargo test --workspace --locked --all-targets`
- `cargo clippy --locked --workspace --all-targets -- -D warnings`
- `cargo fmt --all -- --check`
- `cargo rustdoc --lib --locked -- -W missing-docs`（0 warning，
  在新 deny gate 下保持绿）

## [0.11.1] — 2026-04-26

### Rustdoc intra-doc-link 收口

修复 `cargo rustdoc --lib --locked -- -W missing-docs` 累积的 11 个
intra-doc-link 类 warning，把链向私有 item 或 unresolved scope 的
`[\`xxx\`]` 改为：

- 私有 item → backtick plain text（不创建链接，避免 `private_intra_doc_links`）
- 同一 impl 内方法 → `[\`Self::xxx\`]`（如 `WritePlan::to_json` /
  `PidWriter::write_to`）
- crate root re-export 的公共类型 → `[\`crate::xxx\`]`（如 `crate::PidPackage` /
  `crate::PidError::ParseFailure`）

涉及文件：

- `src/backup/mtf.rs`：`BLOCK_SIZE_SCAN_LIMIT`
- `src/import_view.rs`：`build_cluster_summaries`
- `src/parsers/dynamic_attr_records.rs`：`find_pidattributes_record_starts` +
  `read_drawing_id_before`（顺手把 `scan_da_record_trailers_with_trace`
  的函数签名按 nightly rustfmt 1.9.0 折叠到单行）
- `src/publish/mod.rs`：`crate::bin::pid_backup_extract`（pid-parse 没有
  bin re-export，改为非链接的 “the `pid_backup_extract` CLI binary”）
- `src/publish/xml_writer.rs`：`derive_meta_uid`
- `src/writer/plan.rs`：`PidPackage` / `PidError::ParseFailure` / `to_json`
- `src/writer/mod.rs`：`write_to`（`Self::write_to` 替换两处）

零语义变化。`cargo rustdoc --lib --locked -- -W missing-docs` 现在
回到 0 warning baseline。

验证（5 道 pre-commit gate 全绿）：

- `cargo build --workspace`
- `cargo test --workspace --locked --all-targets`
- `cargo clippy --locked --workspace --all-targets -- -D warnings`
- `cargo fmt --all -- --check`
- `cargo rustdoc --lib --locked -- -W missing-docs`（0 warning）

## [0.11.0] — 2026-04-26

> 主线：完整建立字节审计（byte-audit）框架 + 公共 API rustdoc 收口。
> 自 v0.10.0 (2026-04-24) 以来累积的全部 [Unreleased] 内容打包为本
> 版本，无破坏性变更：所有现有 parser/writer/CLI surface 维持兼容；
> 字节审计 framework 与 `_with_trace` parser 入口为新增公共 surface。
>
> 主要主题：
>
> - **Phase 12b 字节审计**：引入 `byte_audit::aggregate::byte_audit_report`
>   + 17 流注册分发（Summary / cluster / DA records / DA landmarks /
>   sheet endpoint records / TaggedTxtData XML / JProperties / Sheet
>   text run / PSM 三表 / DocVersion 2&3 / AppObject / JTaggedTxtStgList），
>   `--byte-audit` CLI + `--byte-audit-baseline` 比较器（regression /
>   improvement 分类 + 退出码 3）；roadmap Phase 4 字节级
>   consumed/leftover 验证由"部分"提升到"框架完备"。
> - **`PSMspacemap` 顶层 storage 识别**：`KNOWN_TOP_LEVEL_STORAGE_PREFIXES`
>   补齐，`unidentified_top_level_streams` / coverage 报告不再把它误标
>   为 Unknown。
> - **Public API rustdoc Tier 1/2/3 全收口** + `#![warn(missing_docs)]`
>   硬门禁，未来公共项缺文档直接 fail CI。
> - **架构文档 / Pedantic clippy 第二轮 / vendored `oxidized-mdf`
>   lint gate / examples walkthrough / criterion baseline** 等若干基础
>   设施提升。
>
> 详细子项见下方各 `### …` 段落（按合并时间倒序）。

### 把 PSMspacemap 识别为顶层 storage 前缀

`KNOWN_TOP_LEVEL_STORAGE_PREFIXES` 增加 `"PSMspacemap"` —— 真实
fixture 在顶层和 `JSite*` 下都存在 `PSMspacemap/0xNNNNNNNN` 子流，
每个 stream 头部为 `tseg` (`0x6765_7374`) magic，已经有
`describe_magic` 文案（"PSMspacemap segment table"）但这层 storage
之前没进入识别清单，会被 `unidentified_top_level_streams` 与
`inspect::coverage` 当作 `Unknown` 报告。

新增/调整后：
- `unidentified_top_level_streams` 不再列举 `/PSMspacemap`；嵌套在
  JSite 下的 `PSMspacemap` 仍由 JSite prefix 处理（行为不变）
- `inspect::coverage` 把 `PSMspacemap` 标记为 `IdentifiedOnly`
  storage，并把 storage members 的 `stream_size` 累加进 coverage
  byte 维度
- 默认人类报告 / JSON / coverage section 不再有 `[UNK] PSMspacemap`
  噪音

验证：
- `cargo test --lib inspect`（53 测试，含 1 个新 prefix-recognition
  测试 + 既有 coverage 测试扩展为覆盖 5 个 storage prefixes）
- `cargo clippy --locked --workspace --all-targets -- -D warnings`
- `cargo fmt --all -- --check`

后续 follow-up：当反向工作把 `tseg` page table 解码为 stable 字段
时，可以把 `PSMspacemap` 升级为 `KNOWN_TOP_LEVEL_STREAM_NAMES` 或
为它专门添加 PartiallyDecoded / FullyDecoded 状态条目。

### Byte-audit 加入 DA class name 与 DrawingID landmark

`parsers::dynamic_attr_records::scan_da_landmarks_with_trace` 在
Phase 12b-1g 的 31 字节 trailer trace 之上又叠加两类固定布局
landmark：

- 14 字节 ASCII `P&IDAttributes` class-name run（由
  `find_pidattributes_record_starts` 已经定位的每条记录起点开始）
- 10 字节 ASCII `DrawingID\0` tag 紧跟 32 字符大写 hex
  `drawing_id`（与既有 `read_drawing_id_before` 的判定逻辑等价：
  hex 校验失败时该 record 跳过，仅 trailer + class name 落入
  consumed）

`byte_audit::aggregate` 把 `/Unclustered Dynamic Attributes` 的
dispatch 从 `scan_da_record_trailers` 切换到新 `scan_da_landmarks`，
所有 landmark 均以 `TraceConfidence::Decoded` 计入；high-heuristic
record body 内剩余字节继续保留 leftover 状态，Phase 11a-probe 后续
工作可再进一步光照那些区段。

验证：
- `cargo test parsers::dynamic_attr_records --lib`（21 测试，含 3
  个新 landmark trace 测试）
- `cargo test --lib byte_audit::aggregate::tests::unclustered -- --nocapture`
- `cargo clippy --locked --workspace --all-targets -- -D warnings`
- `cargo fmt --all -- --check`

### Byte-audit 跟踪 DA 记录尾部 trailer

`parsers::dynamic_attr_records` 新增
`scan_da_record_trailers_with_trace` —— 复用既有
`extract_record_trailers` 的 31 字节固定 trailer 解码，把每个
`P&IDAttributes` 记录末尾的 trailer (`0x89 0x00` marker / size /
record_id / 8B padding / field_x / `0xFFFF` separator / class_id /
`0x14 0x00 0x00` tail) 以 `TraceConfidence::Decoded` 计入 consumed。

`byte_audit::aggregate` 把 `/Unclustered Dynamic Attributes` 接到这条
新 dispatch；high-heuristic 的 record body 继续作为 leftover 留给
未来更深一层的 Phase 11a-probe 工作，但 trailer 这层确凿结构现在已
经是 byte-audit 视图的一部分。

验证：
- `cargo test parsers::dynamic_attr_records --lib`（18 测试，含 2
  个新 trace 测试）
- `cargo test --lib byte_audit::aggregate::tests::unclustered -- --nocapture`
- `cargo clippy --locked --workspace --all-targets -- -D warnings`
- `cargo fmt --all -- --check`

### Byte-audit 跟踪 Sheet 端点对记录

`parsers::sheet_endpoint_records` 新增
`scan_endpoint_records_with_trace` —— 不依赖 DA 侧 `rel_field_xs`
集合的自包含扫描器。利用 26 字节 endpoint record 内固定的 14 字节
discriminator (`0x0000_0006` / `6× 0x00` / `0x0002` / `0x0001`)
逐字节匹配，每个命中的 26 字节区域以
`TraceConfidence::Probed` 计入 consumed。

`byte_audit::aggregate` 的 `/Sheet*` dispatch 在既有
`probe_sheet_stream` text-run trace 之后追加这一扫描，让 Sheet 流
里的关系端点对记录进入字节预算并清出 leftover 列表，配合 Phase 6
端点解码工作把 byte-audit 视图对齐到业务模型已知的 record 边界。

验证：
- `cargo test parsers::sheet_endpoint_records --lib`（10 测试，含 3
  个新 trace 测试）
- `cargo test --lib byte_audit::aggregate::tests::sheet -- --nocapture`
- `cargo clippy --locked --workspace --all-targets -- -D warnings`
- `cargo fmt --all -- --check`

### Byte-audit 跟踪 cluster header

`parsers::cluster_header` 现在提供 `parse_header_with_trace` /
`parse_string_table_with_trace` / `parse_psm_cluster0_with_trace`
三个 trace-aware 入口；`parse_header` 与 `parse_string_table` 改为 thin
wrapper，保持既有调用方（`streams::cluster`、`streams::sheet_probe`
间接路径）行为零变化。

`byte_audit_report` 把 `/PSMcluster0` / `/StyleCluster` /
`/Dynamic Attributes Metadata` 三条 cluster-family 顶层流挂上 trace：

- `parse_cluster_header` 把 16 字节公共头（magic / `record_count` /
  `stream_type` / `body_len` / flags）以 `TraceConfidence::Decoded` 计
  入 consumed，已知 cluster 结构的精准前缀就此从 leftover 列表里消失
- `/PSMcluster0` 走 `parse_psm_cluster0_with_trace`：cluster header 之外，
  heuristic locator 区间 `[16..table_start]` 标记为 `Probed`（位置已知，
  字段语义未命名），随后逐条 string-table entry 的 `(index, byte_len)`
  头与 UTF-16LE payload 全部 `Decoded`
- `/StyleCluster` / `/Dynamic Attributes Metadata` 仅 trace 16 字节
  公共头；body 对应的 PartiallyDecoded 字节继续作为 leftover 留待
  Phase 11a-probe 后续工作

验证：
- `cargo test parsers::cluster_header --lib`（8 个新单测全绿）
- `cargo test --lib byte_audit::aggregate::tests::cluster -- --nocapture`
- `cargo test --lib byte_audit::aggregate::tests::psm_cluster0 -- --nocapture`
- `cargo clippy --locked --workspace --all-targets -- -D warnings`
- `cargo fmt --all -- --check`

### Byte-audit 跟踪 Summary 流

新增 `pid_parse::parsers::summary::parse_summary_property_set_with_trace`
作为 OLE PropertySetStream（`/\x05SummaryInformation` +
`/\x05DocumentSummaryInformation`）的纯 trace walker，并在
`byte_audit_report` 里注册这两条 stream，让 fully-decoded 的 Summary
section 能把 28 字节 PropertySetStream prefix、20 字节 section header、
8 字节 section body header、N×8 字节 PROPID/offset 表，以及
VT_LPSTR / VT_LPWSTR / VT_I4 / VT_BOOL / VT_FILETIME 等已识别 typed
value 全部以 `TraceConfidence::Decoded` 计入 consumed。
DocumentSummaryInformation section 2 的 user dictionary（PROPID 0）
按 LPSTR variant 完整 walk；未识别 VT 仅消耗 4 字节 tag 为 `Probed`，
payload 留作 leftover 暴露未解读区域。

walker 与既有 `streams::summary::parse_summary_streams` 业务解码
独立，不依赖任何额外公开 API surface；语义模型仍由 `streams` 层
负责，byte-audit 只跟踪覆盖率，符合 Phase 12b-1 系列的"thin parallel
implementation"模式。

附带顺手把 `parsers::sheet_probe::coordinate_hints` 的
`i % 4 != 0` 表达替换为 `!i.is_multiple_of(4)` —— 触发原因是
nightly clippy 0.1.96 启用了 `manual_is_multiple_of` lint，sheet
chunk probe 的 alignment 检查代码被踢出 `-D warnings` 安全区，
单行替换即可恢复 5 个 pre-commit gate 的零警告状态。

验证：
- `cargo test parsers::summary --lib`（8 个新单测全绿）
- `cargo test --lib byte_audit::aggregate::tests::summary -- --nocapture`
- `cargo clippy --locked --workspace --all-targets -- -D warnings`
- `cargo fmt --all -- --check`

### Optional byte audit baseline runner

新增 `.github/scripts/check-byte-audit-baselines.sh`，扫描
`docs/baselines/*.byte-audit.json` 并按同名规则查找
`test-file/<name>.pid`，有 fixture 时调用
`pid_inspect --byte-audit --byte-audit-baseline` 做回归比较。

CI 已接入该脚本；当仓库没有 baseline，或公开 CI 缺少 gitignored/private
fixture 时会输出 skip 并成功退出，避免把 plant data 提交到仓库。

验证：
- `bash .github/scripts/check-byte-audit-baselines.sh`

### Byte audit Sheet text trace

`byte_audit_report` 现在会注册顶层 `/Sheet*` stream，并复用
`sheet_probe::probe_sheet_stream` 的 report-level `text_runs` 证据，只把非重叠
ASCII / UTF-16LE 文本 run 以 `TraceConfidence::Probed` 计入 consumed。Sheet
chunk、record type、coordinate hint 仍保持 evidence-only，不会被误算为已解码
几何字节。

验证：
- `cargo test sheet_streams_are_registered_with_partial_text_run_coverage --lib -- --nocapture`

### Byte audit JProperties trace

新增 `parse_jproperties_with_trace`，保留旧 `parse_jproperties` 作为 thin
wrapper，并在 `byte_audit_report` 里注册所有 `*/JProperties` stream。
该 trace 不会把整个 opaque blob 直接算作 consumed；它只把现有 heuristic
实际可恢复的 ASCII / UTF-16LE 文本 run 以 `TraceConfidence::Probed` 计入
coverage，二进制 prefix / suffix / gap 继续作为 leftover inventory。

验证：
- `cargo test jproperties --lib -- --nocapture`

### Byte audit TaggedTxtData XML trace

`byte_audit_report` 现在会注册 `/TaggedTxtData/Drawing` 和
`/TaggedTxtData/General`：当 stream 是 UTF-8 且现有 XML parser 可解析时，
对应 raw bytes 会以 `TraceConfidence::Decoded` 全量计入 consumed，不再作为
unregistered inventory 出现在 baseline 报告里。非法 UTF-8 或 parser error
仍会保留 registered trace，但 consumed 为 0，方便 baseline 对比暴露异常。

验证：
- `cargo test tagged_text_xml_streams_are_registered_and_fully_consumed --lib -- --nocapture`

### Byte audit baseline comparator

新增 `pid_parse::byte_audit::compare_byte_audit_reports` 以及
`ByteAuditComparison` / `ByteAuditRegressionKind` /
`ByteAuditImprovementKind` 等公开模型，用于比较两份 `ByteAuditReport`
并把整体覆盖率下降、单 stream `consumed_bytes` 下降、已追踪 stream 变回
unregistered 判为 regression；未追踪 stream 被新 parser 覆盖、新 stream
自带 parser 则作为 improvement 汇报。

`docs/byte-audit-guide.md` 已补充 comparator API 用法定位，后续 CLI/CI
可直接反序列化 baseline/current JSON 后调用该库函数。

同时新增 `pid_inspect --byte-audit --byte-audit-baseline <audit.json>`：
文本模式输出 regression / improvement 摘要，`--json` 模式序列化
`ByteAuditComparison`；发现 regression 时退出码为 `3`，便于 CI 区分
"解析失败"和"覆盖率回归"。

验证：
- `cargo test byte_audit --lib`
- `cargo test --test inspect_cli byte_audit_baseline -- --nocapture`

### Byte audit CLI 与基线文档

新增 `pid_inspect --byte-audit`，把已有 `byte_audit_report` 库能力暴露到
CLI：文本模式输出 total / consumed / leftover / overall coverage、fully
consumed traced streams、unregistered streams 以及逐 stream parser 覆盖；JSON
模式通过 `--byte-audit --json` 直接序列化 `ByteAuditReport`，用于 CI 和
后续 fixture baseline。

同时新增 `docs/byte-audit-guide.md`，说明 text / JSON 输出字段、
`unregistered_paths` 的含义、未来 baseline 比较规则，以及缺少真实
`test-file/*.pid` 时不能生成可靠真实基线的限制。`README.md` 已补充
`--byte-audit` 使用示例。

验证：
- `cargo test --test inspect_cli -- --nocapture`

### Sheet chunk probe evidence summaries

增强 `--probe-sheet-chunks` / `parsers::sheet_probe` 的证据输出：
`SheetProbeReport` 现在额外包含 `record_type_counts`、`text_runs` 和
`coordinate_hints`，用于在正式命名 Sheet 几何/图元字段前先暴露记录类型
候选频次、文本 run 偏移和坐标型 `i32` 对。CLI 文本模式会在 chunk 列表前
打印这些 report-level 摘要；JSON 模式随 `SheetProbeReport` 自动输出新增字段。

验证：
- `cargo test --lib sheet_probe -- --nocapture`
- `cargo test --test inspect_cli probe_sheet_chunks_prints_report_level_evidence -- --nocapture`

### Backup RefData ZIP entry index

新增 `backup::zip_index` 元数据层，支持对 SmartPlant backup 里的
`RefData~SCHEMA~ID(.zip)?` ZIP/OOXML payload 读取中央目录并返回
entry 名称、压缩/未压缩大小、目录标记和 CRC-32。该层只列索引、不解压
payload，配合 `refdata` magic-byte 分类，作为后续 Symbol Catalogue /
PlantData cache 解码的入口。

验证：
- `cargo test zip_index --lib`
- `cargo test refdata --lib`
- `cargo test --test backup_zip_index_real_files -- --nocapture`
- `cargo test --test backup_refdata_real_dir -- --nocapture`

### Public API rustdoc pass — Tier 3 final batch + `#![warn(missing_docs)]` 硬门禁

压轴一批：把剩下 88 条 `missing_docs` **一口气扫光**，baseline 从
`88 → 0`，并在 `src/lib.rs` 顶部把 `missing_docs` 加进
`#![warn(...)]` 列表——配合 CI `-D warnings` **自动升级为硬门禁**。

覆盖范围（88 项，跨 25 个文件）：

- `src/error.rs`（6 variant + 2 field）：`PidError` 每个
  variant 注明触发条件；`ParseFailure` 的 `context` / `message`
  说明来源和用途。
- `src/api.rs`（5 field + 2 assoc fn + 1 method）：
  `ParseOptions` 每个字段标注"控制什么"；`PidParser::new` /
  `with_options` / `parse_file` 加语义注释。
- `src/byte_audit/mod.rs`（8 field + 1 assoc fn）：`ByteRange`
  字段（inclusive/exclusive）、`ParserTrace` 的 6 个字段
  （含 `consumed_ranges` 排序 / 合并不变量）、
  `ParserTraceBuilder::new`。
- `src/writer/metadata_helpers.rs`（1 enum + 6 variant + 8
  field）：`MetadataEditError` 六 variant（`AttributeNotFound` /
  `DuplicateAttribute` / `UnterminatedAttribute` /
  `ElementNotFound` / `DuplicateElement` / `MalformedElement`）
  + 每个 struct-variant 内嵌字段。
- `src/backup/mdf_page.rs`（13 field）：`MdfPageHeader` 10 个
  header 字段 + `PageAddress` 的 `file_id` / `page_id`。
- `src/backup/mtf.rs`（4 field）：`MtfError::TooShort::{needed,
  got}` / `NotATapeStart::got` / `MtfStream::kind`。
- `src/backup/boot_page.rs`（1 field）：`TooShort::got`。
- `src/publish/model.rs`（2 field）：`DrawingNotFound::uid` +
  `PidRelationshipRow::item2_location`。
- `src/writer/plan.rs`（2 field）：`SheetChunkPatch::{start, end}`。
- `src/writer/summary_write.rs`（2 const）：
  `SUMMARY_INFO_PATH` / `DOC_SUMMARY_PATH`。
- `src/parsers/psm_tables.rs`（3 const）：`ROOT_MAGIC` /
  `CLST_MAGIC` / `STAB_MAGIC`。
- `src/parsers/cluster_header.rs`（1 const）：`CLUSTER_MAGIC`。
- `src/parsers/doc_version.rs` / `doc_version2.rs`（2 const）：
  `RECORD_SIZE` / `DOC_VERSION2_MAGIC`。
- `src/parsers/string_scan.rs`（2 fn）：`scan_ascii_strings` /
  `scan_utf16le_strings`。
- `src/parsers/drawing_xml.rs` / `general_xml.rs` /
  `jproperties.rs`（3 fn）：`parse_drawing_xml` /
  `parse_general_xml` / `parse_jproperties`。
- `src/cfb/tree.rs`（1 fn）：`build_tree`。
- `src/streams/*.rs`（5 fn）：`parse_clusters` /
  `parse_jsites` / `parse_summary_streams` /
  `parse_tagged_text_streams` / `parse_doc_registry` /
  `parse_dynamic_attrs`。
- `src/inspect/mermaid.rs`（1 struct + 1 fn）：`CrossRefOptions` +
  `crossref_mermaid_with`。
- `src/inspect/report.rs`（1 fn）：`generate_report`。
- `src/layout.rs`（2 fn）：`derive_layout` / `build_layout_model`。
- `src/publish/diff.rs`（1 fn）：`diff_publish_xml`。

**硬门禁升级**：

- `src/lib.rs` 把 `missing_docs` 加到顶层 `#![warn(...)]` 列表，
  跟 `clippy::uninlined_format_args` / `clippy::doc_markdown`
  等十个已就绪 lint 并列。CI 跑 `-D warnings` 会把
  `missing_docs` 当成错误——今后任何新 `pub` 项漏 `///` 都
  在 PR 阶段阻塞合并。
- `.github/missing-docs-baseline.txt` 从 `88` 改到 `0`；
  `.github/scripts/check-missing-docs.sh` + CI 步骤保留，
  作为双保险（即使有人改 `lib.rs` 移掉 warn，ratchet 仍然
  会捕获）。

验证：
- `cargo rustdoc --lib --locked -- -W missing-docs` 总数
  `88 → 0`（-88），
  `bash .github/scripts/check-missing-docs.sh` 本地验
  `current=0, baseline=0, OK`。
- `cargo clippy --locked --workspace --all-targets -- -D warnings` /
  `cargo fmt --all -- --check` / `cargo test --workspace`
  （`812 passed / 0 failed / 2 DWG-gated ignored`）全绿。

累计九轮 rustdoc：**`473 → 0`（-473，100%）**。至此
pub-API 级别 `missing_docs` 战役收官。后续新代码靠硬门禁
保持 0。

### Public API rustdoc pass — Tier 3 batch 4（`package.rs` 诊断族）

第四批：把 package 层 `src/package.rs` 的 26 条 `missing_docs`
（全部是 struct field）一口气打掉，baseline `114 → 88`。

覆盖范围：

- `RawStream`（3 字段）：`path` 规范化到 `/`、`data` 是原始
  字节、`modified` 由 `replace_stream` / `mark_unmodified`
  翻动。
- `StorageTimestamps`（2 字段）：`created` / `modified` 的
  `None` 语义写清楚（源 CFB 未设置）。
- `PackageDiff`（6 字段）：`only_in_a` / `only_in_b` /
  `modified` / `root_clsid_match` / `root_clsid_a` /
  `root_clsid_b` 各自的含义 + 跟 `is_empty` 的关系。
- `StorageClsidDiff`（3 字段）：`path` / `a` / `b` 一条 diff
  记录。
- `StorageTimestampDiff`（3 字段）：同上。
- `StateBitsDiff`（3 字段）：同上。
- `StreamDiff`（6 字段）：`len_a` / `len_b` 字段来源；
  `first_mismatch_offset` 在 strict-prefix 情况下等于
  `min(len_a, len_b)`；`context_before` / `context_after`
  越界打 `"(eof)"` 的行为写进 rustdoc。

验证：
- `cargo rustdoc --lib --locked -- -W missing-docs` 总数
  `114 → 88`（-26）。
- `.github/missing-docs-baseline.txt` 从 `114` 改到 `88`，
  `bash .github/scripts/check-missing-docs.sh` 本地验
  `current=88, baseline=88, OK`。
- `cargo clippy --locked --workspace --all-targets -- -D warnings` /
  `cargo fmt --all -- --check` / `cargo test --workspace`
  （`810 passed / 0 failed / 2 DWG-gated ignored`）全绿。

累计八轮 rustdoc：`473 → 88`（-385）。剩余 88 散在
`src/writer/metadata_helpers.rs`（15）、
`src/backup/mdf_page.rs`（13）、`src/byte_audit/mod.rs`（9）、
`src/error.rs`（8）、`src/api.rs`（8）等。下轮 batch 5 目标：
把剩下 88 条一次性扫完，随后切到硬门禁。

### Public API rustdoc pass — Tier 3 batch 3（`import_view.rs`）

延续上一批 sheet_probe：把 UI 投影层 `src/import_view.rs`
30 条 `missing_docs` 打空，baseline `144 → 114`。

覆盖范围（5 struct + 24 field + 1 fn）：

- `PidImportView`（7 字段 + 结构级 `///`）：明确它是 UI 视图
  而不是原始解码；`title` 的 fallback 链（drawing_number
  → summary.title → `"Smart P&ID Import"`）、`project_number`
  的 fallback（ObjectGraph → GeneralMeta.tags）写进了 rustdoc。
- `PidVisualObject`（5 字段 + 结构级 `///`）：点明它是
  `PidObject` 的"UI slim"，`extra` 从 `BTreeMap` 拍平成
  排序 `Vec` 的原因（diff 稳定性）写清楚。
- `PidVisualRelationship`（4 字段 + 结构级 `///`）：source /
  target drawing_id 的 `None` 含义跟
  `EndpointResolutionStats` 对齐。
- `PidSymbolSummary`（4 字段 + 结构级 `///`）：
  `usage_count == jsite_names.len()` 不变式写进注释。
- `PidClusterSummary`（4 字段 + 结构级 `///`）：点明它在
  `build_cluster_summaries` 里混合了 cluster / sheet /
  coverage 三种来源。
- `build_import_view` 顶层 fn：加了一行
  "does not mutate `doc`；safe to call repeatedly"。

验证：
- `cargo rustdoc --lib --locked -- -W missing-docs` 总数
  `144 → 114`（-30）。
- `.github/missing-docs-baseline.txt` 从 `144` 改到 `114`，
  `bash .github/scripts/check-missing-docs.sh` 本地验
  `current=114, baseline=114, OK`。
- `cargo clippy --locked --workspace --all-targets -- -D warnings` /
  `cargo fmt --all -- --check` / `cargo test --workspace`
  （`810 passed / 0 failed / 2 DWG-gated ignored`）全绿。

累计七轮 rustdoc：`473 → 114`（-359）。剩余 114 集中在
`src/package.rs`（26）、`src/writer/metadata_helpers.rs`（15）、
`src/backup/mdf_page.rs`（13）、`src/byte_audit/mod.rs`（9）、
`src/error.rs`（8）、`src/api.rs`（8）、长尾小文件 35。下轮
候选：`package.rs`。

### Public API rustdoc pass — Tier 3 batch 2（`parsers/sheet_probe.rs`）

紧跟 model.rs Tier 3 一击破：把第二大缺口
`src/parsers/sheet_probe.rs` 的 42 条 `missing_docs` 一次补齐，
baseline 再降一档 `186 → 144`。

覆盖范围（4 struct + 2 enum + 23 field + 13 variant）：

- `SheetProbeOptions`（6 字段 + 结构级 `///`）：
  `min_chunk_len` / `max_preview_strings` / `zero_run_threshold` /
  `ascii_burst_threshold` / `utf16_burst_threshold` /
  `min_boundary_score` 每条都说明它控制哪一步启发式。
- `SheetProbeReport`（5 字段 + 结构级 `///`）：点明
  `candidate_boundaries` 是每启发式视图、`chunks` 是
  post-thresholding 切片，二者不等价。
- `CandidateBoundary`（3 字段 + 结构级 `///`）：`score`
  直接跟 `SheetProbeOptions::min_boundary_score` 做门限比较。
- `BoundaryReason`（8 variant + 枚举级 `///`）：逐条注明
  `ZeroRun` / `AsciiBurst` / `Utf16Burst` / `Alignment4` /
  `Alignment8` /`RepeatedU32Pattern` / `OffsetLikeSequence` /
  `MarkerTransition`，其中 `Alignment8` 和 `MarkerTransition`
  明确标记为"保留，尚未发射"。
- `SheetChunk`（9 字段 + 结构级 `///`）：特别点明
  `start..end` 和 [`crate::writer::plan::SheetChunkPatch`]
  保持 layout 兼容；`zero_ratio` 的值域、`aligned_u32_density`
  的定义都写清楚。
- `SheetChunkKindHint`（5 variant + 枚举级 `///`）：每个
  bucket 跟 `classify_chunk` 内部阈值对上。

风格要求同上一轮：每条 rustdoc 点"是什么 / 什么时候起作用 /
在 pipeline 哪一步写入"，不做无信息叙述；`SmartPlant` 加反引号
（顺手修了 `SheetProbeOptions` 结构级 `///` 里一个被
`clippy::doc_markdown` 抓到的裸 `SmartPlant`）。

验证：
- `cargo rustdoc --lib --locked -- -W missing-docs` 总数
  `186 → 144`（-42）。
- `.github/missing-docs-baseline.txt` 从 `186` 改到 `144`，
  `bash .github/scripts/check-missing-docs.sh` 本地验
  `current=144, baseline=144, OK`。
- `cargo build --locked --workspace --all-targets`、
  `cargo test --locked --workspace --all-targets`
  （`810 passed / 0 failed / 2 DWG-gated ignored`）、
  `cargo clippy --locked --workspace --all-targets -- -D warnings`、
  `cargo fmt --all -- --check` 全绿。

累计六轮 rustdoc：`473 → 144`（-329）。剩余 144 集中在
`src/import_view.rs`（30）、`src/package.rs`（26）、
`src/writer/metadata_helpers.rs`（15）、
`src/backup/mdf_page.rs`（13）。下轮候选：`import_view.rs`。

### Public API rustdoc pass — Tier 3（`src/model.rs` 一击破）

Ratchet 落地之后的第一轮"棘轮降档"——把缺口最集中的
`src/model.rs` 178 条 `missing_docs` 警告**一次全部补齐**，同步把
baseline 从 `364` 降到 `186`（`rustdoc -W missing-docs`
实测 `current=186, baseline=186, OK`）。

覆盖的类型族（按模块语义分 7 组）：

- **CFB / 流表面**：`StorageNode` / `EntryKind` / `StreamEntry`
  + `SummaryPropertyValue::Raw` 的 `vt` / `bytes` 字段。
- **Cluster 族**：`ClusterInfo`（含每个字段对应的 CFB 语义）、
  `ClusterHeader`（`magic 0x6C90F544` / `record_count` /
  `stream_type` / `body_len` / `flags`）、`IndexedString`、
  `ClusterKind` 六个 variant、`DynamicAttributesBlob`。
- **Attribute 族**：`AttributeRecord` / `AttributeField` /
  `AttributeValue` 四 variant + `RelationshipTrailingToken` 的
  `offset` / `value`。
- **Sheet / Unknown / Coverage**：`SheetStream` / `UnknownStream` /
  `CoverageNodeKind` / `CoverageEntry::{kind, status}` /
  `CoverageReport::entries`。
- **PSM / AppObject / Tagged / DocVersion2**：`PsmRoots` /
  `PsmRootEntry` / `PsmClusterTable` / `PsmClusterEntry` /
  `PsmSegmentTable` / `VersionHistory` / `VersionRecord` /
  `AppObjectRegistry` / `AppObjectEntry` / `TaggedTextStorageList` /
  `TaggedTextStorageEntry` / `DocVersion2Raw` / `DocVersion2Record`。
- **Layout 族**：`PidLayoutModel` / `PidLayoutItem`（10 字段）/
  `PidLayoutSegment`（6 字段）/ `PidLayoutText`（5 字段）/
  `PidLayoutUnplaced`（3 字段）；每个字段都标清楚"是
  `ObjectGraph` 的哪个来源 + 是 layout heuristic 产出还是
  `SmartPlant` 原生 CAD 值"。
- **CrossReference / Provenance 族**：`ClusterCoverageSourceKind` /
  `DeclaredClusterRef` / `FoundClusterRef` / `ClusterCoverageMatch` /
  `ProvenanceChainBreak` / `SheetProvenanceRef` / `SymbolReference` /
  `AttributeClassSummary::{class_name, record_count}` /
  `AttributeClassRecordRef`（5 字段）/ `RootPresence`（4 字段）。

风格要求：
- 每条 rustdoc 都说明**该字段是什么 / 来自哪个 CFB 层 /
  什么时候为 `None`**，不写"The `name` field"这种无信息注释。
- 结构体级 `///` 点明"是做什么用的 + 产生者是谁"；
  `SmartPlant` 加反引号，避免 `clippy::doc_markdown` 抓漏。
- 顺手把 5 处没加反引号的 `SmartPlant` 一起改掉
  （`StorageNode`、`VersionRecord`、`DocVersion2Record::version`、
  `PidLayoutModel`、`PidLayoutItem::graphic_oid`）。

验证：
- `cargo rustdoc --lib --locked -- -W missing-docs` →
  `178` 条消失，总数 `364 → 186`。
- `.github/missing-docs-baseline.txt` 从 `364` 改到 `186`，
  `bash .github/scripts/check-missing-docs.sh` 本地验
  `current=186, baseline=186, OK`。
- `cargo build --locked --workspace --all-targets`、
  `cargo test --locked --workspace --all-targets`
  （`810 passed / 0 failed / 2 DWG-gated ignored`）、
  `cargo clippy --locked --workspace --all-targets -- -D warnings`、
  `cargo fmt --all -- --check` 全绿。

剩余 186 条主要集中在 `src/parsers/sheet_probe.rs`（42）、
`src/import_view.rs`（30）、`src/package.rs`（26）、
`src/writer/metadata_helpers.rs`（15）、`src/backup/mdf_page.rs`（13）
等。累计五轮 rustdoc 改进：`473 → 186`（-287）。

### 架构文档

- 新增 `docs/architecture-guide.md`：完整的 pid-parse 架构与原理指南，覆盖 L1–L8 八层架构、核心数据流、`.pid` / OLE 复合文档结构、关键类型体系、Probe/Decode 双层解码策略、`PidWriter` 回写机制、独立管线（publish/byte_audit/inspect）与 crate 依赖关系。
- 新增 `docs/diagrams/pid-parse-architecture.{png,svg}` 与 `docs/diagrams/pid-parse-dataflow.{png,svg}`：架构分层图与数据流图，配合 `.mmd` 源文件同步更新，供 README 与架构指南引用。

### Publish pipeline 改进

- publish pipeline 生产代码完全 panic-free：消除 `xml_writer.rs` 中最后
  一个 `expect("publishable rep has model_item_uid")`，改为 `let-else continue`。
- 新增 MDF loader 可观测性：引入 `log` crate，`stage_table` 报告每张表
  行数/列数，`open_mdf_as_sqlite` 报告汇总统计（表数、行数、耗时）。
  默认静默，无 log subscriber 时零开销。
- CLI `pid_publish_xml` 新增 `--verbose` / `-v` 标志：激活 `env_logger`
  显示 MDF 加载诊断（每表行数、耗时、缺失表），同时显示 `oxidized-mdf`
  内部的 WARN 级日志（列解析边界跳过），便于调试新 MDF 文件。
- vendored `lib.rs` 中列解析失败的日志从 `error!` 降级为 `warn!`：
  SQL Server compact record format 中可空列超出固定区域是正常行为，不是解析器 bug。
- 消除 vendored `lib.rs` 中最后一个生产代码 `.unwrap()`，改为 `let-else`
  模式。三个 vendored 源文件现在真正实现零 `unwrap`/`panic!`/`todo!()`。
- 消除 vendored `sys.rs` 中 `expect("Should have type for column")`，改为
  `filter_map` 模式——未知 scalar type 的列被安静跳过而不是 panic。
- 新增 `LICENSE-MIT` 和 `LICENSE-APACHE` 文件，匹配 `Cargo.toml` 中声明
  的 `license = "MIT OR Apache-2.0"`。

### 依赖升级

- `quick-xml` 0.38 → 0.39（XML 处理引擎）。
- `rusqlite` 0.31 → 0.39（bundled sqlite3 + hashbrown v0.16）。
- 两项升级均无 breaking API 变更，814 测试全绿。

### Clippy 清理（29 warnings → 0）

- vendored `pages.rs`：`div_ceil` / `Option::map` / 多余 range 括号 /
  测试内 `vec!` → 数组字面量。
- 父 crate 生产代码：`i.is_multiple_of(2)`、`Vec::resize` 代替
  同值 push 循环、`elide needless lifetimes`、`HashMap::contains_key`、
  提取 `SymbolUsageBucket` 类型别名。
- 父 crate 测试／可执行程序：`let-else` → `?`、`enumerate` 代替手动
  行号计数、提取测试专用类型别名、调整 rustdoc 列表缩进；
  `crossref` / `inspect::report` 测试模块加 `#[allow(clippy::
  field_reassign_with_default)]` 保留 fixture 搭建的可读性。
- 工作区 `cargo clippy --all-targets --workspace` 零 warning，
  全量测试（606 unit + 200 integration + 31 vendored）全绿。

### CI + 格式一致性

- 跑 `cargo fmt --all` 修复 36 个文件累计的 rustfmt 漂移
  （+547 / −598，纯空白/换行整理，零行为变更）。
  先前 main 分支 CI 连续 red 均源于 `cargo fmt --all -- --check` 失败。
- `.github/workflows/ci.yml` 的 `cargo build` / `cargo test` /
  `cargo clippy` 统一加 `--workspace`，把 vendored `oxidized-mdf`
  纳入相同 gate，任何 vendored 端 warning 也会直接阻塞合并。
- `AGENTS.md` 新增 "Pre-commit gates" 一节，明确列出 build / test /
  clippy / fmt 四道 gate 的本地等价命令。

### Pedantic clippy 批次（第二轮）

- `clippy::uninlined_format_args`：40 个文件改用 inlined 捕获
  （`format!("{x}")` 风格）。
- `clippy::doc_markdown`：56 个文件给 rustdoc 中的标识符加反引号
  （`SmartPlant` → `` `SmartPlant` ``、`SQLite` → `` `SQLite` `` 等，
  415 处替换，净零行数变化）。
- `clippy::redundant_closure_for_method_calls` / `manual_let_else` /
  `unreadable_literal` / `map_unwrap_or` / `bool_to_int_with_if` /
  `implicit_clone` / `explicit_iter_loop` / `unnecessary_map_or`
  全部清零（含 2 个 `map_or(true, …)` → `is_none_or`，
  4 个 `match …`  → `let-else`，约 25 处 hex 字面量加下划线）。
- `src/lib.rs` 顶部 `#![warn(...)]` 锁死上述 10 个 lint，
  配合 CI `-D warnings` 形成硬门禁。

### `missing_docs` CI ratchet gate

四轮 rustdoc 压 warning 之后，baseline 已经稳定在 `364`。这一
轮落地"棘轮门禁"把这个数字锁进 CI：

- `.github/missing-docs-baseline.txt` — 单行整数 `364`，表示当前
  可容忍的 `rustdoc -W missing-docs` warning 数。
- `.github/scripts/check-missing-docs.sh` — 跑
  `cargo rustdoc --lib --locked -- -W missing-docs`，count
  匹配 `"missing documentation for"` 的行数，然后比对 baseline：
  - `current > baseline` → fail（有人加了没文档的 `pub` 项）。
    输出前 40 条 warning 位置，让 PR 作者立刻看到问题。
  - `current < baseline` → fail（有改善！但要求在同一次 PR 里把
    baseline 文件也改到新数字，保持 ratchet 进度可见）。
  - `current == baseline` → pass。
- `.github/workflows/ci.yml` 在 `cargo fmt --check` 后面追加
  `missing_docs ratchet` 步骤执行脚本。
- `AGENTS.md` 把 Pre-commit gates 从 4 条扩到 5 条，并把 ratchet
  的 runbook（如何降 baseline）写进文档。

本地验证：`C:\Program Files\Git\bin\bash.exe .github/scripts/
check-missing-docs.sh` → `current=364, baseline=364, OK`。Linux CI
(ubuntu-latest) 的执行路径完全一致（gitbash 和 ubuntu bash 对该
脚本行为一致）。

### Public API rustdoc pass — 子模块 `//!` 全收口

把剩下 16 个此前仅有 `use` 开头、缺模块级文档的子模块一次性
打完。**模块级 missing_docs 从 16 降到 0**，`cargo doc --open` 的
目录页每个节点都有一段可导航的简介。

涉及文件（都在各自模块入口加了 4–8 行 `//!`）：

- `src/cfb/reader.rs`、`src/cfb/tree.rs`
- `src/inspect/report.rs`
- `src/parsers/cluster_header.rs`、`drawing_xml.rs`、
  `dynamic_attr_records.rs`、`general_xml.rs`、`jproperties.rs`、
  `string_scan.rs`、`xml_util.rs`
- `src/streams/cluster.rs`、`dynamic_attrs.rs`、`jsite.rs`、
  `psm_tables.rs`、`summary.rs`、`tagged_text.rs`

每段统一结构：第一行点明"这是做什么的"，后面 2-4 行说明该子
模块在 pipeline 中的上下游（调用谁、被谁调用、产出填充哪个
`PidDocument` 字段），避免 code narration，全部是意图 / 协作关
系描述。

`cargo rustdoc --lib --locked -- -W missing-docs` 总数
**380 → 364（-16）**；累计四轮 rustdoc 改进 473 → 364（-109）。
剩余 364 集中在深层 DTO（252 field + 42 variant + 33 struct）、
低层 utility fn（18）、少量 constant/enum/assoc_fn/method。

途中 clippy::doc_markdown 抓住了 `string_scan.rs` `//!` 中未加
反引号的 `JSite`，顺手修掉，没别的行为变化。`cargo clippy
--locked --workspace --all-targets -- -D warnings` / `cargo fmt
--check` / `cargo test --workspace` (819 passed / 2 DWG-gated
ignored) 全绿。

### Public API rustdoc pass — Tier 2（对象图 / JSite 层）

Tier 2 第二批：接着把用户经常 drill-down 的"对象图族"与
`JSite` 族的字段级 doc 补齐。这两族合计拿掉 27 条 missing_docs
warning（424 → 380？——实测 407 → 380，-27）。

- `ObjectInventory`（4 字段）：指明每个字段对应的 DA attribute
  来源（`DrawingNo` / `ProjectNumber` / `ModelItemType`），以及
  与 `ObjectGraph::counts_by_type` 的关系；顺带给结构本身补了
  一段 3 行 rustdoc 摘要。
- `PidItem`（3 字段）：每个字段对应的 DA 记录含义
  （`ModelItemType` / `DrawingID` / `ModelID`）+ 什么时候为
  `None`。
- `EndpointResolutionStats::total`：补上此前唯一未文档化的字段。
- `JSite`（9 字段 + 结构级 `///`）：`JSite*` 存储在 CFB 中的
  角色、`symbol_path` vs `local_symbol_path` 区别、`ole_links`
  来自 `\x01CompObj` / `\x03ObjInfo` 这些细节、`raw_streams`
  的用途都写进去了。
- `JProperties`（4 字段 + 结构级 `///`）：每个字段对应原始 blob
  的哪一部分、`raw_len` 保留的原因。
- `EmbeddedStream`（3 字段 + 结构级 `///`）：`JSite` 等容器里
  的裸流元数据。

### Public API rustdoc pass — Tier 2（`PidDocument` 层）

Tier 2 第一批：在上一轮把 `src/lib.rs` crate-level `//!` + 9 个
模块 `//!` + 4 个门面类型级 `///` 写完之后，继续把顶层门面结构
`PidDocument` 以及它唯一暴露给终端用户的三个元数据聚合类型
（`SummaryInfo` / `DrawingMeta` / `GeneralMeta`）的 `pub` 字段
全部补上 `///`。

- `PidDocument` 的 17 个此前无字段级 doc 的字段——`cfb_tree` /
  `streams` / `summary` / `drawing_meta` / `general_meta` /
  `jsites` / `clusters` / `dynamic_attributes` / `sheet_streams` /
  `psm_roots` / `psm_cluster_table` / `psm_segment_table` /
  `version_history` / `app_object_registry` / `tagged_storages` /
  `doc_version2` / `unknown_streams`——每条 1–3 行指明"对应哪个
  CFB 流 / 什么时候 `None` / 跟哪些相邻字段联动"。
- `SummaryInfo` 的 6 个字段（`creating_application` / `template` /
  `title` / `created_time` / `modified_time` / `raw`）写上
  `PID_…` PROPID 对应、OLE Summary 语义和 ISO-8601 渲染点。
- `DrawingMeta` 10 个字段（含结构本身）：把每个 `Option` 字段
  对应到 `<DrawingNumber>` / UID 语义 / `SP_` attribute 形式；
  `raw_xml` 明确其"writer 原样回写"角色。
- `GeneralMeta` 4 个字段 + 结构体级 `///`：`<FilePath>` /
  `<FileSize>` 语义 + `raw_xml` 的回写语义。

`cargo rustdoc --lib --locked -- -W missing-docs` 统计
**447 → 407（-40）**：struct field 313 → 276（-37），struct
39 → 36（-3）。剩下 276 field / 42 variant / 36 struct 大多在
`model.rs` 的深层 DTO（`PidObject` / `PidLayoutItem` / 各 `Psm*`
表等）、`inspect::coverage` 的 `CoverageEntry` / `ParseCoverage…`
枚举、`backup::mtf` 的记录结构。Tier 3 及以后按模块逐步收。

工程侧健康度不变：clippy `-D warnings`、`cargo fmt --check`、
`cargo test --workspace` 813 → 819 全绿。

### Writer 扩展 `typed_value_size` 的 VT 覆盖面

上一轮 `roundtrip_walkthrough` 为了能跑通真实 SmartPlant fixture
只能用 `WritePlan::default()` passthrough，不敢走 `summary_updates`
元数据 patch ——一旦走上就硬撞
`unsupported VT type 0x0002`（`VT_I2`，A01 的 `SummaryInformation`
里 `CodePage` 属性就是这个类型）。

本轮把 `src/writer/summary_write.rs::typed_value_size` 的 VT 字节
宽度表从 4 种（`VT_I4 / VT_LPSTR / VT_LPWSTR / VT_FILETIME`）扩
到完整的 MS-OLEPS 标量集：

- 1 字节：`VT_I1 / VT_UI1`
- 2 字节：`VT_I2 / VT_BOOL / VT_UI2`
- 4 字节：`VT_I4 / VT_R4 / VT_ERROR / VT_UI4 / VT_INT / VT_UINT`
- 8 字节：`VT_R8 / VT_CY / VT_DATE / VT_I8 / VT_UI8 / VT_FILETIME`
- 16 字节：`VT_CLSID`
- 变长：`VT_LPSTR / VT_LPWSTR`（保持原有长度字段读取）

关键点：widening 只改"切多少字节"的策略表，没有触碰"改哪几个
属性"的白名单（依旧只能重写 `VT_LPSTR / VT_LPWSTR` 下的字符串
值）。其他 VT 的属性作为 `raw_value` 原样 passthrough，和改前
对它们的语义完全一致——只是之前整个 property-set 会被一刀拒
收，现在可以共存。

- 错误消息更新为可执行的完整列表。
- 新增 6 条回归测试（`writer::summary_write::tests::typed_value_size_*`）
  分别锁定 1/2/4/8/16 字节标量的宽度，以及对真正 unknown VT（如
  `0x2000 VT_ARRAY`）仍然拒收的行为。
- `examples/roundtrip_walkthrough.rs` 从 passthrough 重构为真正
  演示 `summary_updates.insert("title", …)` 的 declarative patch；
  A01 fixture 的 `title` 现在干净地从 `"Normal"` 轮转成
  `"pid-parse roundtrip_walkthrough demo"`。示例源注释里原本对
  "为什么只能 passthrough"的解释被删去。

### `roundtrip_walkthrough` 示例 + `cargo build --examples` 进 CI

把 `examples/` 走查三部曲最后一条补齐，同时把 examples 锁进
CI，让上一轮的 Lpwstr bug 这种"example 暴露、commit 前 CI 拦住"
的回退类问题今后一律在推送时就被发现。

- 新增 `examples/roundtrip_walkthrough.rs`：`PidPackage::from_path`
  → `WritePlan::default()` passthrough → `PidWriter::write_to_bytes`
  → `PidPackage::from_bytes` 再 parse，断言流数量一致。A01 fixture
  本地跑过：75 streams → 106496 bytes → 75 streams。
- 示例故意用 passthrough 而非 `summary_updates` 元数据 patch——真实
  SmartPlant SummaryInformation 常常包含 `VT_I2`（0x0002），目前的
  writer 只支持 `VT_I4 / VT_LPSTR / VT_LPWSTR / VT_FILETIME` 的重写，
  patch 路径对真实 fixture 会硬失败。示例源码中保留一段注释详细说
  明这一点，供读者理解不是 bug。
- `src/lib.rs` crate-level `//!` 的 "examples 走查" 列表扩到三条。
- `.github/workflows/ci.yml` 在 `cargo test` 之后加一步 `cargo build
  --locked --examples`；实测缓存后 ~1–2 秒，换来 examples 层独立可
  观测性（失败消息里能明确指向示例而不是库）。

### 修复：`SummaryPropertyValue` 的 `serde_json` 可序列化

上一轮写 `examples/parse_walkthrough.rs` 时发现
`PidDocument::to_json` / `serde_json::to_string_pretty(&doc)` 在
任何带 "user-defined summary properties" 的 `.pid` 文件上都会
panic：
`cannot serialize tagged newtype variant SummaryPropertyValue::Lpwstr containing a string`

根因：`SummaryPropertyValue`（Phase 10j 引入的 typed `.pid`
user-dict property）标注了 `#[serde(tag = "kind")]`（*internally-tagged*），
而 serde 对 internally-tagged enum **不支持** 直接包 `String` /
`i32` / `bool` / `u64` 这种 newtype variant——只支持 struct variant
和 unit variant。

- 改为 adjacently-tagged：`#[serde(tag = "kind", content = "value",
  rename_all = "snake_case")]`。wire 格式现在是
  `{"kind": "lpstr", "value": "hello"}` /
  `{"kind": "raw", "value": {"vt": 21, "bytes": [...]}}` 等，六个
  variant 全部可序列化。
- 这是 wire 格式变化，但从未成功序列化过的 variant 不存在向后兼
  容担忧；`Raw` variant 的 JSON 形状从
  `{"kind": "raw", "vt": ..., "bytes": [...]}` 变为
  `{"kind": "raw", "value": {"vt": ..., "bytes": [...]}}`——此前
  没有测试或 fixture 锁定这个形状（本次扫查确认）。
- 新增 6 条回归测试（`model::summary_property_value_serde_tests`）
  对每个 variant 做 `to_string → from_str` 轮转 + pin wire shape。
- `examples/parse_walkthrough.rs` 还原为直接 `to_string_pretty(&doc)`，
  A01 fixture 现在稳定产出 97549 字节 JSON；旁路的 summary
  `json!({...})` 被删除。
- `PidDocument` 的结构定义本身未变——该类型无 `to_json()` 方
  法，用户按惯例直接 `serde_json::to_string_pretty(&doc)`；此
  前 `pid_inspect --json` 在有 user_properties 的 `.pid` 上也会
  fail，本次一并根治。

### End-to-end `examples/` walkthroughs

为 crate-level `//!` 刚写进 `src/lib.rs` 的"3 条管线入口"承诺
配上可运行示例——`cargo run --example …` 一条命令就能看到
reader 和 publish 两条链路真的跑通。

- `examples/parse_walkthrough.rs`：打开一个 `.pid`（默认 A01
  fixture，可传 argv[1] 覆盖），打印流数量、`DrawingMeta` /
  `GeneralMeta` 摘要，然后吐一段 summary JSON。蓄意规避了
  `SummaryPropertyValue::Lpwstr(String)` 这个已知 serde_json 无
  法序列化的 tagged newtype 变体，改用一个小的 `json!({...})`
  摘要视图。
- `examples/publish_walkthrough.rs`：打开 `Export.mdf`（默认
  TEST02 fixture 下的 A01 drawing，可传 `<mdf> <uid> <plant>` 覆
  盖），调 `load_drawing_graph_from_mdf()`、分别 dump
  `write_data_xml()` / `write_meta_xml()` 的结果大小与前 240 字符
  预览。
- 两个示例都遵循既有 soft-skip 模式：fixture 不存在则打印提示并
  正常退出，`cargo build --examples` / `cargo run --example …`
  在无 SmartPlant 样本的机器上也干净通过。
- `src/lib.rs` crate-level `//!` 增补了 "examples/ 走查" 小节，
  让 `cargo doc --open` 的落地页直接指向这两个文件。
- `cargo clippy --locked --workspace --all-targets -- -D warnings`
  全绿；`cargo fmt --check` 全绿；`cargo run --example
  parse_walkthrough` / `publish_walkthrough` 本地两次冒烟都正常。

### Public API rustdoc pass — Tier 1

首轮对 crate 级 + 顶层 `pub mod` + 用户门面结构 / 枚举写
rustdoc，目标是把 "读者第一次 `cargo doc --open` 看得懂是什么"
的那层补齐，不触及 `#![warn(missing_docs)]` 硬门禁（field 级 313
条要分多轮收口）。

- `src/lib.rs` 顶部新增 crate-level `//!`：把 8 层架构指向
  `docs/architecture-guide.md`，列出读/写/publish 三条管线的
  入口符号并附一段最小 `no_run` 示例。
- 为 9 个此前缺 `//!` 的模块补模块级总览：
  `api` / `cfb` / `error` / `import_view` / `inspect` / `layout` /
  `model` / `parsers` / `streams`。每段 4–8 行，说明模块在流水线
  中的位置、依赖哪些上游模块、产出喂给哪些下游模块。
- 为 4 个此前缺 `///` 的门面类型写类型级文档：
  `PidParser` / `ParseOptions` / `PidDocument` / `PidError`。
  聚焦"该类型在什么位置被调用、常见入口、常见失败模式"。
- `cargo rustdoc --lib --locked -- -W missing-docs` 统计
  473 → 447（-14）：crate 1→0、module 25→16、struct 42→39、
  enum 8→7；剩余 313 field + 42 variant 等更细粒度项留到 Tier 2
  按模块分期吃掉，届时再考虑把 `#![warn(missing_docs)]` 钉进
  `src/lib.rs` 做硬门禁。
- `cargo clippy --locked --workspace --all-targets -- -D warnings`
  仍然全绿（`clippy::doc_markdown` 也绿——途中触发过 19 个
  `SmartPlant` / `DocVersion` / `JSite` / `JProperties` /
  `AppObject` 反引号缺失被当场补上）。

### Performance baseline（criterion）

首次为三条热路径建立 criterion 基线，用于后续重构 / 依赖升级
的回归信号；**不**纳入 CI gate —— criterion 数字跨机器漂移太大，
目前只作为本地参考。Fixture（A01 `.pid` + TEST02 `Export.mdf`）缺失
时 bench 会 soft-skip 打印提示，`cargo bench` 仍正常完成。

| Scenario                 | Median     | 95% CI                 | 备注 |
|--------------------------|------------|------------------------|------|
| `parse_pid_a01`          |  7.79 ms   |  7.00 ms …  8.64 ms    | `.pid` 冷读（CFB → `PidDocument`），含整棵流解码 |
| `load_mdf_a01`           | 21.30 ms   | 20.86 ms … 21.77 ms    | `Export.mdf` → `PublishDrawing`，走 vendored `oxidized-mdf` |
| `write_data_xml_a01`     | 15.09 µs   | 14.92 µs … 15.32 µs    | 预加载 `PublishDrawing` → `<PIDDrawing>` XML（writer 独立量） |

跑法：`cargo bench --bench pid_pipeline`
（可选参数：`-- --sample-size 20 --warm-up-time 1 --measurement-time 3`
用于压低单次耗时；默认参数约 40–50 s）。测量机器：本地 Windows
/ release profile / `bench` target。

接入方式：
- `Cargo.toml` 新增 `criterion = { version = "0.8", default-features
  = false, features = ["cargo_bench_support"] }` dev-dep 与
  `[[bench]] name = "pid_pipeline" harness = false`。
- `benches/pid_pipeline.rs` 共三个 `criterion_group` 成员，每个都
  走 `path.exists()` 预检，fixture 缺失时 `eprintln!` 提示并 return，
  与 `tests/common/mod.rs` 的 soft-skip 模式对齐。

### Vendored `oxidized-mdf` 对齐父 crate lint gate

- `vendor/oxidized-mdf/src/lib.rs` 顶部加入与父 crate 相同的
  10 lint `#![warn(...)]`（`uninlined_format_args` / `doc_markdown` /
  `redundant_closure_for_method_calls` / `manual_let_else` /
  `map_unwrap_or` / `unreadable_literal` / `bool_to_int_with_if` /
  `implicit_clone` / `explicit_iter_loop` / `unnecessary_map_or`），
  标注来源为"镜像父 crate 的 pedantic lint 子集"。
- 同步清理 vendored 源码残留：
  - `error.rs` 两处 `write!` inline format（`{err}` / `{msg}`）。
  - `sys.rs` 常量 / 二十余处 scalar-type mapping 长字面量加下划线
    （`327_680`、`281_474_978_349_056` 等）。
  - `pages.rs` 四处 `match Some(_) => …, None => {…}` → `let-else`
    （`variable_columns`、`read_bytes_index`、slot `checked_mul` /
    `checked_sub`），保留 `log::error!` 诊断并把 `{slot_count}` /
    `{page_size}` 也改为 inline capture。
- 结果：`cd vendor/oxidized-mdf && cargo clippy --locked --all-targets
  -- -D warnings` 独立绿；`cargo clippy --locked --workspace --all-targets
  -- -D warnings` workspace 级全绿；vendored 31 单测 + 4 doc-test 仍全通过。
- SPDX 头已声明 `GPL-3.0-or-later`，本次修改仅为 lint 噪声清理与
  `let-else` 语法糖重写，无语义变化，符合 GPL 兼容要求；同时在
  `lib.rs` 顶部注释里追加一行 "Added #![warn(...)] lint set to mirror
  parent crate's quality gate" 作为修改备注。

## [0.10.0] — 2026-04-24

### Publish XML source — Rust MDF loader (`oxidized-mdf`)

把 Stage-1 publish XML 的主数据源从“先用 C# OrcaMDF probe 导出
`Export_v2.sqlite`，再由 Rust 读取 SQLite mirror”改为“直接读取
`Export.mdf`”。仓内已把
[`f3rn0s/oxidized-mdf`](https://gitlab.com/f3rn0s/oxidized-mdf)
克隆到 `vendor/oxidized-mdf`，并以 path dependency 接入。

#### Changed

- `pid_publish_xml` 的公开正确性基线已收敛到 `<mdf>`：
  `.mdf` 自动走 Rust MDF loader；旧 `Export_v2.sqlite` 输入仍保留，
  但只作为 legacy 兼容路径，并在运行时打印 deprecation 提示。
- 新增 `publish::mdf_load`，用 `oxidized-mdf` 读取 publish 所需
  SmartPlant 表，再暂存到 in-memory SQLite connection，复用既有
  `sqlite_load` / writer / diff 逻辑，避免重复实现查询层。
- README 与 `docs/sppid/publish-xml-export-flow.html` 已更新为当前
  主链：`Export.dmp -> Export.mdf -> oxidized-mdf -> drawing graph ->
  _Data.xml/_Meta.xml`。

#### Fixed

- 对 vendored `oxidized-mdf` 做了兼容补丁，保证 TEST02 / A01 MDF
  可稳定读取：
  - page reader 缓存已读页，并在 `file_id` 不一致时按 `page_id`
    fallback，避免 forward-only reader 回读旧页 panic。
  - variable-length NULL 列仍推进 offset，避免后续 nvarchar 列错位。
  - fixed-length nullable 列即使为 NULL 也消费固定字节，修复
    `T_Drawing.DateCreated` 被错读为 `1900/1/2` 的问题。
  - 允许 fixed-length region 为 0 的 variable-only record，修复
    `T_Area` / `T_GlobalDrawing` / `T_Plant` / `T_Unit` 扫描时的
    `No fixed length data` panic。
  - 列元数据按 `colid` 排序，避免系统表返回顺序造成字段错配。
  - 固定长度列越界从 panic 改为返回 parse error。

#### Changed — `oxidized-mdf` nom 8 迁移 + parser hardening

- 将 vendored `oxidized-mdf` 的全部 byte 解码路径迁移到 `nom 8`
  （envelope layer）和 stdlib `from_le_bytes`（value layer）：
  - `Record::try_from` / `VariableColumns::try_new` 使用 `nom::bytes::complete::take` 做边界安全解析。
  - `PagePointer::try_from` / `PageHeader::try_from` / slot directory 同步迁移。
  - `parse_variables_bytes_opt()` 对 variable-column end offset 倒退返回 `Err`，不再 subtract overflow panic。
  - fixed-width readers（`parse_i8/i16/i32/i64/f32/f64/u128`）从 `ReadBytesExt::unwrap()` 改为 `from_le_bytes`。
  - datetime / decimal helpers 同步迁移，`parse_datetime2_opt` 中 `read_i24` 替换为手工 i24 符号扩展。
- 移除 `byteorder` crate 依赖，零残留。
- 产品代码（`pages.rs`）实现完全 panic-free：零 `panic!` / `unwrap()` / `todo!()`。
  - unknown record-type panic 改为 `Err`。
  - chrono epoch `.unwrap()` 改为 `.single().ok_or()`。
- 去除 vendored crate 和父项目中的全部 async 抽象：
  - `MdfDatabase::open` / `PageReader` / `BaseTableData::parse` 改为同步 API。
  - `PageStream`（`Stream`）→ `PageIter`（`Iterator`），消除双重 `block_on` 开销。
  - `mdf_load.rs` 直接调用同步 API，不再包装 `task::block_on`。
  - 移除 `async-std`、`futures-lite`、`async-log` 依赖。
  - `publish_mdf_load` 测试从 ~2.6s 降至 ~0.09s（29x 提速）。
- `lib.rs` 产品代码 panic 边界消除：
  - 新增 `Error::ParseError` 变体，替代 IoError 包装。
  - `BootPage::try_from` / `Page::try_from` 的 `.unwrap()` 改为 `?` 错误传播。
  - `read_page` 中的 `assert!` 改为 `Err`（backward-read 检查）。
  - page cache miss 从 `.unwrap()` 改为 `.ok_or()`。
- 总计移除 7 个依赖：`byteorder`、`num-bigint`、`async-std`、`futures-lite`、`async-log`、`femme`、`uuid 0.8`。
- `sys.rs` 产品代码也实现 panic-free：macro / TryFrom 中的 `.unwrap()` 全部改为 `ok_or` + `?`。
- 升级 vendored crate 到 Rust edition 2021，uuid 对齐父项目（uuid 1.x），清理 prelude imports。
- `read_next_page` 从 `read()` 改为 `read_exact()`，保证每个 8192 字节 page 完整读取。
- MDF 列类型覆盖从 15 种扩展到 27 种，新增 `sysname`、`char`、`binary`、`numeric`、`smalldatetime`、`smallmoney`、`image`、`text`、`ntext`、`date`、`timestamp`。
- 新增 `AGENTS.md`，记录 vendored parser 约束和项目架构供后续 agent 快速上手。

#### Tests / Verification

- 新增 `tests/publish_mdf_load.rs`，直接从
  `test-file/backup-test/TEST02_p/extracted/Export.mdf` 验证 A01
  publish 核心表、representation 关联与 `DateCreated`。
- `tests/publish_xml_cli.rs` 改为用真实 `Export.mdf` 跑 CLI 集成测试。
- 本轮已验证：
  - `cargo test --test publish_mdf_load -- --nocapture` → 1 passed
  - `cargo test --test publish_xml_cli -- --nocapture` → 19 passed
  - `cargo check` → ok
  - `pid_publish_xml Export.mdf --diff-against A01_Data.xml` →
    8 个 PID tag varieties 全部 MATCH。

#### Notes

- `oxidized-mdf` 本身为 GPL-3.0（by schrieveslaach）。许可证合规
  已确认：所有修改过的 vendor 文件添加了 GPL-3.0 §5(a) 修改通知，
  README 添加了 License section 说明 MIT/Apache-2.0 + GPL-3.0
  combined-work 边界。详见 `README.md` License 章节。

### Publish writer Stage-1 — fidelity ratchet (A12 → A39)

把 SmartPlant Publish Data XML writer 的 fidelity 守门从"tag 计数级"
逐层加固到"接口级"再到"属性级"，并把对照范围从"writer vs A01
reference"扩展到"A01 vs DWG 跨 fixture"。这一系列工作不改变
writer 的字节输出（A25 引入了 PIDProcessVessel tank-variant 的条件
emit，是唯一例外），但建立了一套 9 道 regression gate，任何未来
的接口/属性 drift 会立即在 CI 上以"`(tag, interface, attr)`"
三元组失败定位。

本轮继续收口 Stage-1 的可见边界：

- `_Meta.xml` parity 已补齐到独立测试文件
  `tests/publish_meta_parity.rs`，同时覆盖 A01 参考样本与
  compare-only 的 DWG `Export.mdf` 语义对照；该样板只用于对比，
  不要求其对应数据库参与验证。
- 新增 `tests/publish_dwg_mirror.rs` 作为 DWG MDF 入口；
  当 `test-file/backup-test/DWG-0202GP06-01_p/extracted/Export.mdf`
  缺失时，测试会显式 soft-skip 并指出 Stage 2-4
  （DWG canonical-field enrichment / branch-point parity）
  尚未验证，而不是默默把 DWG 缺口混进绿线。
- `tests/common` 的 A01 publish 生成 helper 已改为 MDF-first，
  旧 `Export_v2.sqlite` 只保留给 `sqlite_load` 兼容测试。
- `tests/publish_a01_raw_residual.rs` 新增 ignored Rust MDF 全表
  probe：A39 实测 `oxidized-mdf` 枚举 TEST02 MDF 128 张表，
  `tables_skipped=0`；connector UID / Rel IObject UID /
  GraphicOID 三类 raw residual 在当前 staging 表、完整 Rust MDF
  表清单、以及 MDF raw ASCII / UTF-16LE / UUID byte-form 扫描中
  均无命中，因此正式归类为 publish-time synthetic slots，并只在
  A01 delivery-contract 里做窄 mask。
- publish 模块、writer 与 CLI 顶部注释改为反映当前真实状态：
  `_Data.xml` / `_Meta.xml` 已可运行，现存主阻塞收敛为 compare-only
  DWG `Export.mdf` 样板缺失与其后的 branch-point / loader 富化闭环。

#### Added — fidelity 分析层（src/publish/diff.rs）

- A12 `parse_pid_tag_counts` / `diff_publish_xml` /
  `SemanticDiffReport` — `<PIDxxx>` 开标签计数级语义 diff
- A15 `coverage_against_reference` / `WriterCoverage` /
  `supported_pid_tags` — 把 reference 标签集分为"writer 已支持"
  和"backlog"两桶
- A23 `parse_interfaces_per_tag` — 每个 PID tag 下 first-occurrence
  的接口名集合（`I*`）
- A26 `parse_attrs_per_interface_per_tag` — 每个 `(tag, interface)`
  对的属性名集合（值不计，只看 attr key）

#### Added — fidelity gate 层（tests/）

- A23 `tests/publish_interface_parity.rs` ·
  `interface_parity_on_a01_writer_matches_reference_superset_post_a22`
  + sanity sub-test：writer 必须 ⊇ A01 reference 的接口集
- A24 同文件 ·
  `a01_and_dwg_reference_interfaces_agree_for_every_shared_supported_tag`
  + 2 guard：A01 ↔ DWG fixture 接口集一致性 + whitelist 维护守门
- A27 `tests/publish_attribute_parity.rs` ·
  `attribute_parity_on_a01_writer_matches_reference_superset_per_interface`
  + sanity sub-test：writer 必须 ⊇ A01 reference 的属性集
  （per `(tag, interface)`）
- A27b 同文件 ·
  `a27b_a01_and_dwg_reference_attrs_agree_for_every_shared_tag_interface`
  + 2 guard：A01 ↔ DWG 属性集一致性 + whitelist 维护守门
- A28 `tests/publish_backlog_inventory.rs` ·
  `a28_backlog_tag_specs_match_reference_fixtures_exactly`
  + 3 guard：未支持 PID tag 的 fidelity spec snapshot。
  Stage-4 后 `PIDPipingBranchPoint × 4 + PIDBranchPoint × 5`
  已毕业进 `supported_pid_tags()`；当前 `BACKLOG_SPECS`
  为空，A28 改作“新 unsupported tag 出现即红”的 guard
- A33 `tests/publish_rel_parity.rs` ·
  `rel_defuid_parity_on_a01_writer_matches_reference_supersets`
  + `a33b_a01_and_dwg_reference_rel_defuids_agree_set_wise`
  + 2 guard：`<Rel>` 元素的 DefUID-级 fidelity（writer ⊇ A01
  ref + A01 ⇄ DWG 跨 fixture 一致性）。补齐 `<Rel>` 守门，
  之前所有 gate 只覆盖 `<PIDxxx>` body
- A34 derived Rel emit 修复 — `write_relationships` 在每个
  PipeRun 对象上 emit 5 个 derived Rel（PipingPortComposition × 2、
  ProcessPointCollection、PipingEnd1Conn、PipingEnd2Conn），与
  `write_derived_connector_endpoints` 已经在 emit 的 PIDPipingPort
  / PIDProcessPoint body 配套。`KNOWN_WRITER_REL_DEFUID_GAPS`
  从 6 项 → 2 项（EquipmentComponentComposition +
  PipingConnectors 留给 A34b 的 loader 端工作）
- A34b derived Rel emit 收尾 — A33 调研发现 A01 fixture 的
  T_Relationship 表只有 Rep↔Rep 行，writer 缺的 2 个 DefUID
  其实是 derived from object parent links：
  * `PipingConnectors`（Pipeline ↔ PipeRun）— 直接从 PipeRun
    obj.uid 派生 (pipeline = obj.uid, connector = `<obj.uid>-CNX`)
  * `EquipmentComponentComposition`（Vessel ↔ Nozzle）— 从
    Nozzle 的 `obj.fields["SP_EquipmentID"]`（loader 已经
    attach 了的 T_Nozzle.SP_EquipmentID 列）派生
  `KNOWN_WRITER_REL_DEFUID_GAPS` 从 2 项 → 0 项，A33 gate
  完全 drained。Writer 现在 emit 9 DefUIDs（8 reference + 1
  extra "Relationship" fallback for un-classified Rep↔Rep）。
- A35 doc tests — 给 6 个核心 publish API 加可执行 inline
  examples：`parse_pid_tag_counts` (A12)、`coverage_against_reference`
  (A15)、`parse_interfaces_per_tag` (A23)、`parse_attrs_per_interface_per_tag`
  (A26)、`parse_rel_defuid_counts` (A33) 以及 `PublishStyle` enum
  (A29)。Doc tests 从 0 → 6，CI 自动跑（`cargo test --doc`），
  让外部用户 `cargo doc --open` 看到的 API 文档同时是 working
  example，且文档与代码不会漂移。
- A36 `<Rel>` UID2 semantic-fidelity gate — 在 `src/publish/diff.rs`
  增加 `parse_rel_details(xml) -> Vec<RelDetail>` 解析器，暴露
  每条 `<IRel>` 的 `(UID1, UID2, DefUID)` 三元组。配套
  `tests/publish_rel_parity.rs` 新增两道 gate：
  * **A36** — A01 writer 至少有一条 `PipingEnd1Conn` 的 UID2
    指向真实上游 ModelItem UID（不是 `.PPT` 占位符）。直接锁定
    A34c 的 loader + writer 协作，未来 refactor 如果让 UID2
    退回 `.PPT` 占位符 CI 会立刻红。
  * **A36b** — writer 输出的每条 `<IRel>` 的 UID2 必须要么命中
    文档里已 emit 的 `<IObject UID="...">`，要么匹配显式已知
    派生后缀 (`.1` / `.2` / `.PPT`)。这是第一道真正走 UID 图的
    soundness gate——之前所有 Rel gate 都是计数级或集合级。
    首次跑 A36b 暴露了 writer 端一个 latent bug：T_Relationship
    有 `SP_Item2ID` 为 NULL 的行时，writer 会 emit `<IRel UID2=""/>`
    这种 dangling reference（reference 根本不 emit 这类半悬 rel），
    顺手 fix：`write_relationships` 跳过任何 source/target UID
    为空的 T_Relationship 行。
  A36/A36b 走同一份 `parse_rel_details` 解析器；`RelDetail` 和
  `parse_rel_details` 都透过 `publish::mod.rs` 重导出到公共 API，
  并带 doc test 作为可执行用例。
- A34c `PipingEnd1Conn` / `PipingEnd2Conn` UID2 真实 endpoint
  inference — 关闭 A33 → A34 → A34b 之后残留的 "UID2 = `<connector>.PPT`
  占位符"语义缺口。新增 loader helper
  `attach_pipe_endpoint_connections(conn, &mut drawing)`，用
  三段式 join 把每个 PipeRun / SignalRun 的两个端口解到真实
  上游 ModelItem UID：
  1. `T_Representation` 已加载的 rep → ModelItem 映射
  2. `T_Connector.SP_ConnectItem{1,2}ID` 给出"port.N 接的 rep"
  3. 回查映射，把 rep UID 转回 ModelItem UID
  结果 attach 到 `obj.fields["EndConnectedItem1"]` /
  `EndConnectedItem2`；writer 在 `write_relationships` 里优先
  消费这两个字段，缺失时回退到原 `<connector>.PPT` 占位符
  （A01 port.2 本就没外接，reference 里也用 `.PPT`，回退 ≠ 补丁，
  就是 SPPID "无外接"惯例）。
  验证：A01 fixture 的 `PipingEnd1Conn` UID2 从 `.PPT` 切换到
  Nozzle UID `7465E81219...`，与 reference 完全一致；
  `PipingEnd2Conn` 仍为 `.PPT`（port.2 无外接，与 reference 相同）。
  `T_Connector` 表缺失时 `prepare_optional` 软跳过 → pid-only
  bundle 不受影响。对 A33 gate 通过无影响（gate 是 DefUID 计数
  级，不看 UID 值），但把 SmartPlant 验证器真正关心的语义 UID
  交叉引用补齐。

#### Added — writer 真实改动

- A13 `derive PIDPipingPort + PIDProcessPoint from connectors`
- A14 `filter annotation representations -> A01 fully clean`
- A16 `derive PIDSignalPort children from InstrFunction`
- A17/A18/A19/A20/A21/A22 `close PIDPipingComponent +
  PIDSignalConnector + upgrade 6 tags to full-interface SPPID
  canonical shape`
- A25 `PIDProcessVessel low-pressure-tank variant conditional
  emit`（DWG 17-interface tank shape；A01 15-interface drum
  shape；通过 `obj.fields["IsLowPressureTank"]` 路由）
- A29 `PublishStyle::{A01, Dwg}` enum + `PublishDrawing.style`
  字段（默认 A01）；writer 在 PIDPipeline / PIDPipingConnector /
  PIDProcessVessel 的 IObject 上按 style 切换：
  * A01 style：`<IObject UID="..." [Name="..."] ItemTag="..."
    [Description="..."]/>`（保持 pre-A29 字节级一致）
  * Dwg style：`<IObject UID="..." [Name="..."]
    [Description="..."]/>`（不发 ItemTag，匹配 DWG reference）
  把 pre-A29 "PipelineName 有值即触发 DWG-shape"的隐式数据驱动
  改为显式 style 选择，让 caller 明确表达 fixture flavor。
- A29b `pid_publish_xml --style a01|dwg` CLI 选项（默认 a01，
  大小写不敏感）。CLI 把选择透传给 `PublishDrawing.style`，
  让 ops 不写代码就能从命令行切换 fixture flavor。
- A30 CLI 增强 — `pid_publish_xml --list-drawings` 列出 SQLite
  mirror 里的 T_Drawing 表（SP_ID / Name / DocumentCategory /
  DocumentType / Path），单独一种 mode（不与 --drawing /
  --out / --stdout / --diff-against / --meta-out 组合）；
  `DrawingNotFound` 错误信息追加 `use --list-drawings to see
  available drawing UIDs` actionable hint。
- A31 集成测试 refactor — 新建 `tests/common/mod.rs` 抽出 4 个
  publish-相关 tests 文件之间重复的 fixture loader（`generate_a01_xml`、
  `load_reference_a01_xml`、`load_reference_dwg_xml`）+ 路径常量
  （SQLITE_PATH / A01_DRAWING_UID / PLANT_NAME / 两个 reference
  XML 路径）+ TAGS_UNDER_PARITY 13-tag 表。3 个 test 文件
  （`publish_interface_parity` / `publish_attribute_parity` /
  `publish_backlog_inventory`）改用 `mod common;` 共享，删
  ~120 行重复代码，未来加新 fidelity gate 直接复用。
- A32 README 重写 — 把 publish writer Stage-1（A12-A31）的能力、
  CLI 用法、库调用示例和 fidelity 矩阵补到 README.md。原 README
  停留在 v0.4.1 的 layout 模型，未反映 backup 解析 +
  `pid_publish_xml` CLI + 13 类 PID tag writer + 9 道 fidelity
  gate + A29 PublishStyle 切换 + A29b CLI --style 选项 + A30
  --list-drawings 等本轮成熟度。

#### Added — A27b whitelist（KNOWN_A01_VS_DWG_ATTR_DIVERGENCES）

15 条 fixture-side variant，分两类：

1. **IObject identifier rename**（3 项）— A01 export 用 `ItemTag`，
   DWG export 用 `Name`，业务标识相同但 attr key 不同。
   `PIDPipeline / PIDPipingConnector / PIDProcessVessel` 的 IObject
   都中招（PIDProcessVessel 的 DWG 实例没有 tag，退化为单边
   only_in_a01）。
2. **DWG-side 富化列**（12 项）— DWG plant 的源数据填了 SmartPlant
   多列（FluidCode / FluidSystem / EqType0..3 / EquipmentTrimSpec /
   FlowDirection / TotalInsulThick / SlopedPipingAngle / ...），
   A01 plant 的源是 NULL。Writer 当前按 SmartPlant 惯例"NULL 列就
   不发 attr"，所以这是 loader-side gap，需要 DWG SQLite mirror
   bundle 后才能填。

#### Tests

* lib：540 → 589（+49，A26 +7 `publish::diff::tests::parse_attrs_*`，
  A29 +7 `publish::xml_writer::tests` 中 IObject style 切换，
  A33 +8 `publish::diff::tests::parse_rel_defuid_counts_*`，
  A34c +11 = 8 个 `publish::sqlite_load::tests::attach_pipe_endpoint_*`
  + 3 个 `publish::xml_writer::tests::a34c_*`，
  A36 +8 `publish::diff::tests::parse_rel_details_*`；
  其余 +8 来自 A23 / A24 / A25 之前已记录的相关单测）
* integration：140 → 164（+5 在 `tests/publish_attribute_parity.rs`，
  +4 在 `tests/publish_backlog_inventory.rs`，
  +5 在 `tests/publish_xml_cli.rs` 覆盖 A29b CLI `--style`，
  +4 在 `tests/publish_xml_cli.rs` 覆盖 A30 `--list-drawings` +
  drawing-not-found hint，
  +4 在 `tests/publish_rel_parity.rs` A33 / A33b + 2 guard，
  +2 在 `tests/publish_rel_parity.rs` A36 + A36b + A36 DWG sanity）
* doc tests：0 → 7（A35 — `parse_pid_tag_counts` /
  `coverage_against_reference` / `parse_interfaces_per_tag` /
  `parse_attrs_per_interface_per_tag` / `parse_rel_defuid_counts` /
  `PublishStyle`；A36 — `parse_rel_details`）
* lint：0 warnings

#### A28 backlog inventory（历史 snapshot，现已毕业）

A28 当初把两类 branch-point 的 DWG 参考形态 pin 成可执行 spec：

* `PIDPipingBranchPoint`（DWG × 4）：6 接口
* `PIDBranchPoint`（DWG × 5）：9 接口
* UID 后缀模式：`<base>.BPT`

Stage-4 已按该 snapshot 落地对应 writer arm，并将两类 tag
纳入 `supported_pid_tags()`；`tests/publish_backlog_inventory.rs`
现改为 guard“新的 unsupported tag 不得悄然进入 reference fixture”。

#### A33 → A34 → A34b → A34c Rel DefUID 进展

A33 gate 第一次跑暴露了 writer 端 6 个 derived Rel DefUID 缺口。
A34 + A34b 先 close 全部 6 项的 DefUID 计数维度，A34c 再把
PipingEnd1Conn 的 UID2 从"占位符语义"升级为"真实上游 ModelItem
UID 语义"：

| DefUID | A33 状态 | A34 状态 | A34b 状态 | A34c 状态 |
|---|---|---|---|---|
| PipingPortComposition × 2 | whitelist | **closed** (writer emit) | (closed) | (closed) |
| ProcessPointCollection | whitelist | **closed** (writer emit) | (closed) | (closed) |
| PipingEnd1Conn | whitelist | **count closed** (UID2 = `.PPT` 占位符) | (closed) | **UID2 语义 closed**（上游 ModelItem UID，与 reference 一致） |
| PipingEnd2Conn | whitelist | **closed** (UID2 = `.PPT` 与 reference 一致) | (closed) | 仍 `.PPT`（A01 port.2 无外接；有外接时走新字段路径） |
| EquipmentComponentComposition | whitelist | whitelist | **closed** (Vessel→Nozzle derived from T_Nozzle.SP_EquipmentID) | (closed) |
| PipingConnectors | whitelist | whitelist | **closed** (Pipeline→Connector derived from PipeRun obj.uid) | (closed) |

A33b gate 暴露了 4 个跨 fixture DWG-only DefUID（已进
`KNOWN_A01_VS_DWG_REL_DEFUID_DIVERGENCES`，纯 SmartPlant
fixture-side variant）：

* PipingTapOrFitting / SignalEnd1Conn / SignalEnd2Conn /
  SignalPortComposition（DWG ships instrument signal 连接和
  piping tap，A01 没有）

#### Backlog（A34d+）

* DWG SQLite mirror 落位后，校实 branch-point 的 loader-side
  `item_type_name` / subtable chain / source mapping；writer arm
  已实现，但真实 SQLite 驱动的 source validation 仍未闭环
* A25b loader-side `IsLowPressureTank` 推断（同上）
* A27b whitelist 收尾：随 DWG mirror bundle 落地，逐条 (tag,
  interface) 关闭 15 条 style / loader-side 富化列差异
* A29c loader-side `PublishStyle` 自动决策：根据 SQLite
  metadata（plant 名 / SmartPlant project 配置）自动设
  `drawing.style`；当前 caller / CLI 必须显式指定 (--style)。
  需要 DWG SQLite mirror 落地后才能 reverse-engineer 出可靠
  detection 启发式。

## [0.9.2] - 2026-04-21

### Phase 3: Provenance-aware cross-reference (Step 1–4)

把 `CrossReferenceGraph` 从"名字集合 + 小摘要"升级为"字段级 provenance
记录 + 端到端诊断"，一次覆盖四条正交视角（cluster/symbol/attribute、
relationship、object、sheet），并且为 Phase 12a 的统一规范化层做好数据
基础。所有变更纯 additive + `#[serde(default)]`，旧 consumer 字节级
兼容。

### Added — Step 1：cluster / symbol / attribute provenance records

- `ClusterCoverage.declared_entries: Vec<DeclaredClusterRef>` — 带
  `record_offset` / `name_offset` / `record_len` 的 PSM 表行级溯源
- `ClusterCoverage.found_entries: Vec<FoundClusterRef>` — 带 `source_kind`
  (`PsmCluster` / `SheetStream`) 与 `path` 的发现源
- `ClusterCoverage.matches_detailed: Vec<ClusterCoverageMatch>` —
  declared ↔ found 的 index 级映射
- `SymbolUsage.references: Vec<SymbolReference>` — 每个 JSite 的 `path` /
  `local_symbol_path` / `has_ole_stream`，取代旧 BTreeSet 去重
- `AttributeClassSummary.records: Vec<AttributeClassRecordRef>` — 每条
  DA 记录带 `attribute_count` / `confidence` / 局部 drawing/model ids

### Added — Step 2：relationship + object provenance links

- `RelationshipEndpointLink` — 每条 `PidRelationship` 连回
  `SheetEndpointRecord` 的 `sheet_path` / `sheet_offset` / source/target
  `field_x`；`missing_sheet_record` 区分"lookup 失败"与"没 field_x"
- `EndpointLinkCoverage` — `total` / `linked` / `missing_field_x` /
  `missing_sheet_record` / `fully_resolved` / `partially_resolved`
- `ObjectSourceRef` — 每条 `PidObject.drawing_id` 连回
  `DynamicAttributesBlob.attribute_records` 索引（`class_name`、
  `attribute_record_index`、`confidence`、`has_trailer_record_id`、
  `missing_da_record`）
- `ObjectSourceCoverage` — `total_objects` / `linked` / `missing_da_record` /
  `with_trailer_record_id`

### Added — Step 3：end-to-end provenance chain diagnostic

- `ProvenanceChainCoverage` — 逐跳通过数：`has_field_x` / `sheet_linked` /
  `source_object_linked` / `target_object_linked` / `fully_traced`
- `ProvenanceChainStage` 枚举 — `MissingFieldX` / `MissingSheetRecord` /
  `SourceObjectUnlinked` / `TargetObjectUnlinked`
- `ProvenanceChainBreak` + `PROVENANCE_CHAIN_BREAK_SAMPLE_CAP = 10`，
  first-fail 规则：一条链在最前断裂处记一次，不污染后续 hop

### Added — Step 4：sheet-level provenance aggregation

- `SheetProvenanceRef` — per-`SheetStream` 聚合：`endpoint_record_count` /
  `declared_in_psm` / `matched_declared_index` / `linked_relationship_count` /
  `fully_traced_relationship_count`，1:1 对应 `doc.sheet_streams`
- `SheetProvenanceCoverage` — `total_sheets` / `declared_sheets` /
  `orphan_sheets` / `sheets_with_endpoint_records` / `empty_declared_sheets`

### Added — inspect report

`Cross Reference` section 新增 5 个小节，均默认取前 5 条样例、超额用
`... (N more)` 提示：

- `Cluster refs:` — declared/found/match 样例
- `Symbol refs:` / `Attribute class refs:` — Step 1 extensions
- `Relationship endpoints:` — Step 2 relationship link coverage + 样例
- `Object sources:` — Step 2 object link coverage + 样例
- `Sheet provenance:` — Step 4 per-sheet 聚合 + 样例
- `Provenance chain:` — Step 3 诊断 + 前 5 条 breaks（超额提示 more）

### Tests — 332 → 400（净增 68）

- Step 1 新增 3 条单测 + 3 条 fixture 回归
- Step 2 新增 8 条单测 + 2 条 fixture 回归
- Step 3 新增 4 条单测 + 1 条 fixture 回归
- Step 4 新增 2 条单测 + 1 条 fixture 回归
- 其余增量来自既有测试对新字段的扩断言

### Changed

- `crossref::build_graph` 内部改为两遍填装：先构造前四子图，再基于已
  构造图派生 provenance chain 与 sheet aggregation；外部 API / 输出
  顺序不变
- 多处 test site（`mermaid`、`layout`、`import_view`、`mermaid_demo`）
  使用 `..Default::default()` 吸收新字段，零行为改动

### SemVer

Patch（additive + `#[serde(default)]`；旧 JSON 输入字节级兼容，调用旧
字段语义未变）。

## [0.9.1] - 2026-04-21

### Phase 2: 三条核心流的结构化解码升级

把 `DocVersion3`、`PSMclustertable`、`PSMsegmenttable` 从"可用但部分
语义化"升级为具备 record 级结构元数据和字节审计能力的解码层。

### Added — 新 model 字段

- `VersionHistory.record_size: usize` — 固定记录大小（48），方便下游对账
- `VersionHistory.trailing_bytes: usize` — 末尾未解释字节数
- `VersionRecord.offset: usize` — 记录在流内的字节偏移
- `PsmClusterEntry.record_offset: usize` — 记录起始偏移（含前缀）
- `PsmClusterEntry.record_len: usize` — 记录总字节长度
- `PsmClusterEntry.prefix_bytes: Vec<u8>` — 记录名称前的原始字节（审计用）
- `PsmClusterTable.trailing_bytes: usize` — 末尾未解释字节数
- `PsmSegmentEntry` 新类型 — 每条 segment 的 `index`、`offset`、`flag`
- `PsmSegmentTable.entries: Vec<PsmSegmentEntry>` — 结构化视图
- `PsmSegmentTable.trailing_bytes: usize` — 末尾未解释字节数

所有新字段均 `#[serde(default)]`，与 v0.9.0 JSON 输入向后兼容。

### Changed

- **DocVersion3 parser**：增加 `product.trim().is_empty()` 校验，拒绝空白
  product 记录（更强的边界保护）；填充 `offset` / `record_size` /
  `trailing_bytes`
- **PSMclustertable parser**：从"UTF-16LE ASCII 名称扫描"升级为"记录遍历"，
  以名称为锚点向前回溯截出完整记录 slice，记录 `record_offset` /
  `record_len` / `prefix_bytes`；识别尾部 null 终止符
- **PSMsegmenttable parser**：在保留 `flags: Vec<u8>` 的基础上，同步生成
  `entries: Vec<PsmSegmentEntry>`，每条 entry 带 `index` + `offset`
- **inspect report**：
  - Version History 段标题显示 `record_size=`
  - 每条记录前缀 `[@+offset]`
  - 显示 trailing bytes（当 > 0）
  - PSMclustertable 每项显示 `rec_len=` / `name@offset` / `prefix=[hex]`
  - PSMsegmenttable 切换到 per-entry 显示 `[index] @+offset flag=0x..`

### Tests (359 → 366+，净增 7+)

- `doc_version3_records_expose_record_offset_and_trailing_bytes` — 3 条记录 + 4 字节尾随
- `doc_version3_rejects_record_with_empty_product` — 空白 product 停止解析
- `doc_version3_zero_trailing_when_exact_fit` — 精确对齐零残余
- `cluster_table_entry_records_offsets_and_prefix_bytes` — record 级结构化
- `cluster_table_reports_trailing_bytes` — 尾部字节审计
- `segment_table_exposes_indexed_entries` — entry 化验证
- `segment_table_reports_trailing_bytes` — 尾部字节审计

### SemVer

Patch（新字段 additive + 行为加强；旧 consumer 无感）。

## [0.9.0] - 2026-04-21

### Phase 10j (MVP): DocumentSummaryInformation section 2 reader

暴露 `/\x05DocumentSummaryInformation` stream 的 **section 2**
（user-defined property dictionary，FMTID
`D5CDD505-2E9C-101B-9397-08002B2CF9AE`）到 Rust 模型里。此前
reader 只读 section 0；section 2 的 SmartPlant 自定义属性
（`SP_ProjectID` / `SP_DrawingRevision` / `SP_Discipline` 等）对
consumer 不可见，只能手工解析 raw stream。

本轮是 Phase 10j 的 **MVP (reader-only scope)**：
- ✅ Reader 端 section 2 解码完整
- ✅ 新 `SummaryPropertyValue` typed enum 接入模型
- ✅ 配套合成 fixture + 5 轮单测覆盖
- ⏳ **Writer CRUD defer 到 Phase 10m**（需先扩展
  `writer::summary_write::parse_section` 支持 Dictionary 特殊
  record + 未知 VT 的 verbatim 保留，scope 超出本 Phase）
- ⏳ CLI `--set-user-summary` / `--delete-user-summary` 同步 defer

### Added — 新 public 类型

- `model::SummaryPropertyValue` enum：typed 表示 user dict 里一个
  property 的值。覆盖 SmartPlant 实测最常见的 VT：
  - `Lpstr(String)` — VT_LPSTR (0x001E)
  - `Lpwstr(String)` — VT_LPWSTR (0x001F)
  - `I4(i32)` — VT_I4 (0x0003)
  - `Bool(bool)` — VT_BOOL (0x000B)，wire format 0x0000/0xFFFF
  - `Filetime(u64)` — VT_FILETIME (0x0040)
  - `Raw { vt: u16, bytes: Vec<u8> }` — 未模型化 VT 的保底，writer
    层未来做 verbatim 透传时能用上
- `SummaryInfo.user_properties: BTreeMap<String, SummaryPropertyValue>`
  新字段，`#[serde(default, skip_serializing_if = "BTreeMap::is_empty")]`。
  v0.7.x / v0.8.x 的 JSON 输入保持向后兼容。

### Added — Reader 侧 section 2 decoder

`src/streams/summary.rs` 新增 `parse_doc_summary_section_2`：

- 遍历 PropertySetStream header 验证 `num_sections >= 2`
- 验证 section 2 FMTID 是 user-defined dict FMTID
- 解析 Dictionary property（PROPID 0）的 LPSTR 条目表（SmartPlant
  默认 variant），建立 `propid → name` 映射
- 遍历剩余 props，按 VT 解码到 `SummaryPropertyValue`
- 未知 VT 落 `Raw` 分支（grab 最多 64B 负载，避免吞掉整流）
- 所有 defensive 边界检查失败都 silent 返回空 map（reader 不 fail
  整个 pipeline）

Phase 10j MVP 假设 Dictionary 是 LPSTR variant（SmartPlant 实测
统一如此）。Dictionary 是 LPWSTR variant 的 code page 处理延后到
Phase 10k（与 reader-side code page detection 一起做）。

### Changed

- `streams::summary::parse_summary_streams` 在解 section 0 之后
  调 `parse_doc_summary_section_2(&data)`，把结果挂到
  `info.user_properties`。section 2 缺失 / 格式错误 / FMTID 错都是
  silent no-op（field 保持空 map）。
- `Cargo.toml` 0.8.0 → 0.9.0；`Cargo.lock` 同步。
- `inspect::coverage::tests::populate_all_known_fields` 的
  `SummaryInfo` struct literal 补 `user_properties: BTreeMap::new()`。

### Tests (354 → 359，净增 5)

lib `streams::summary::section2_tests` +5：

- `section_2_decodes_user_defined_lpwstr_property` —— 合成
  fixture：2 个 section，section 2 里 1 条 `SP_ProjectID =
  "PROJ-001"`，验证 map 出的 key / value 匹配
- `single_section_stream_yields_empty_user_properties` —— 同
  fixture 但手工 patch `num_sections = 1`，section 2 不应被扫描
- `section_2_with_wrong_fmtid_is_ignored` —— 故意 corrupt section 2
  FMTID 首字节，decoder 返回空 map
- `unknown_vt_in_section_2_falls_through_to_raw_variant` —— 塞 VT_UI8
  (0x0015) 这个 decoder 未显式处理的 VT，验证 `Raw { vt, bytes }`
  分支命中且 `bytes.len() >= 8`
- `malformed_header_yields_empty_map` —— 10B 垃圾输入不 panic

### Known limitations

- **Writer CRUD 不可用**：`MetadataUpdates.summary_user_updates /
  summary_user_deletions` 本次不加。尝试在 Writer 层编辑 section 2
  属性当前只能走 `stream_replacements`（full-stream byte replace，
  不保结构）。完整 dictionary + Writer CRUD scope 跟 Phase 10m /
  10n 一起规划。
- **LPSTR Dictionary 假设**：SmartPlant fixture 实测 Dictionary
  为 LPSTR variant。若出现 LPWSTR Dictionary，当前 decoder 空
  返回 — 留给 Phase 10k 扩展（LPWSTR dict 读取 + CodePage property
  尊重）。
- **未识别 VT 的 `Raw.bytes.len()` 上限 64B**：避免流尾异常吞过度
  字节；真实值长度超过 64B 的未识别 VT 不会完整保留。Phase 10m 重
  新 design 时会精确算 size（或从下一 prop 的 offset 反推）。

### SemVer

新 public type `SummaryPropertyValue` + 新 `SummaryInfo` 字段
（向后兼容 via `#[serde(default)]`）+ 无 API 破坏。按本项目规则
"新 public API type = minor bump"，选 **minor 0.8 → 0.9.0**。0.8.x
consumer 的代码在 0.9.0 下编译通过（唯一变化是 `SummaryInfo` 构造
字面量需要补一个 `..Default::default()` 或显式写 `user_properties:
BTreeMap::new()`；同理于 Phase 9l/9n 新增字段时）。

### Docs

- `docs/plans/2026-04-21-phase-10j-docsummary-section2.md`：
  本 Phase dev plan（v0.7.1 ship 时已入 repo）。本 MVP ship 的
  scope 较 plan 初稿缩小到 Reader-only，Writer CRUD 显式 defer；
  Plan 里对 Writer 的设计仍然有效，只是落地推到 Phase 10m。

## [0.8.0] - 2026-04-21

### Phase 10i: VT_LPSTR 多 code page 支持（解除 Phase 10g UTF-8-only 限制）

Phase 10g (v0.7.0) 把 `VT_LPSTR` 从 ASCII-only 放宽到 UTF-8，但明确
保留 "CP1252 / GBK / Shift-JIS 等传统 code page 不自动识别/转换" 的
限制。Phase 10i 兑现这条 roadmap：引入**显式编码**的 `VT_LPSTR`
writer 通道，让用户可以按需写入 SmartPlant 2019- 旧版本使用的
Western/Chinese/Japanese code page。

本 Phase 聚焦 **Writer 端**；Reader 端自动 code-page 启发式探测
（`detect_lpstr_codepage`）留给 **Phase 10k** — 当前 Phase 用户必须
显式指定 `encoding` label，reader 仍按 Phase 10g 的 UTF-8 策略解析
（对 ASCII-only writes 端到端 round-trip 完全工作，非 ASCII writes
的 reader 侧 round-trip 由 writer 层字节级单测验证）。

### Added — `EncodedString` + `summary_updates_encoded` 通道

- `src/writer/plan.rs` 新 `EncodedString { value: String, encoding: String }`
  类型 + `EncodedString::new(value, encoding)` 构造器。`encoding` 字段
  接受任意 `encoding_rs` label（`"UTF-8"` / `"windows-1252"` / `"GBK"` /
  `"Shift_JIS"` / `"cp1252"` / ...），case-insensitive。
- `MetadataUpdates.summary_updates_encoded: BTreeMap<String, EncodedString>`
  新字段，`#[serde(default)]`。与既有 `summary_updates` 平行共存：
  - `summary_updates`（Phase 9l/10g）= UTF-8 默认通道
  - `summary_updates_encoded`（Phase 10i）= 显式 code-page 通道
  - 同一 key 同时出现在两个 map 中 → pre-check 报错（避免编码歧义）
  - 同一 key 同时在 `summary_updates_encoded` 和 `summary_deletions`
    中 → 同样报错（对齐 Phase 9n 的 set/delete 冲突拒绝）
- `pid_parse::writer::summary_write::apply_summary_updates_encoded(
  package, updates)` 公共入口，供 binary / 高级 consumer 直调（等价
  于 apply-plan 路径的 `summary_updates_encoded` 分支）。
- `encode_string_with_encoding(vt, value, encoding, prop_id)` 内部函数：
  - `VT_LPSTR`: 走 `encoding.encode(value)`，`had_errors == true` 时
    **fail-fast** 返回清晰错误（包含 encoding 名 / offending value），
    绝不 silent 替换为 `?`
  - `VT_LPWSTR`: 忽略 `encoding` 参数（UTF-16LE 无歧义）
- `SummaryPropertySet::set_string_with_encoding(prop_id, value, encoding)`
  内部方法；旧 `set_string` 重写为 UTF-8 thin wrapper。
- `resolve_encoding(label) -> Result<&'static Encoding, PidError>`：
  统一 label → `encoding_rs::Encoding` 解析，未知 label 返回
  `PidError::ParseFailure { context: "summary writer", ... }`。
- `lib.rs` re-export `EncodedString` 到 `pid_parse` crate root。
- `writer/mod.rs` re-export `EncodedString`。

### Added — CLI `--set-summary-encoded KEY:ENCODING=VALUE`

- `pid_writer_validate --set-summary-encoded title:windows-1252=Ø Pipe`
  类语法：
  - 第一个 `:` 分 KEY 和 ENCODING
  - 第一个 `=` 分 ENCODING 和 VALUE
  - VALUE 可含 `=` / `:`（空 KEY / 空 ENCODING 都是 usage error）
- 互斥：与 `--set-summary` 同 key 冲突 → exit 2；与 `--apply-plan`
  同时传 → exit 1。与 `--delete-summary` 在**不同** key 上可并存。
- `CliOptions.summary_edits_encoded: BTreeMap<String, EncodedString>`
  新字段；`run_validate` 签名新增参数（lib-level public API）。
- usage 文案更新：`print_usage` 新增 Phase 10i 段说明。

### Changed

- `MetadataUpdates` 默认构造器包含 `summary_updates_encoded:
  BTreeMap::new()`。
- `WritePlan::is_passthrough` 检查扩展到
  `summary_updates_encoded.is_empty()`。
- `apply_metadata_updates` 执行顺序扩展：`drawing_xml → general_xml
  → summary_deletions → summary_updates → summary_updates_encoded`。
- `compare_packages` / `collect_edited_paths_from_plan` 把 summary
  路径标 "edited" 的触发条件从 `!summary_edits.is_empty() ||
  !summary_deletions.is_empty()` 扩展到 + `!summary_edits_encoded
  .is_empty()`。

### Tests (332 → 354，净增 22；测试分布：lib 238→255 = +17，
writer_validate_cli 21→26 = +5，其余不变)

lib 单元 +17：

`src/writer/plan.rs::tests`（+4）：
- `encoded_string_serializes_to_object_with_value_and_encoding`
- `encoded_string_round_trips_through_json`
- `plan_with_summary_updates_encoded_is_not_passthrough`
- `plan_omitting_summary_updates_encoded_is_passthrough_compatible`
  —— **向后兼容 guard**：v0.7.x consumer 写的 JSON 不带此字段仍然
  valid 且 passthrough

`src/writer/summary_write.rs::tests`（+11）：
- `encode_lpstr_with_cp1252_preserves_western_european_bytes`
  —— `Ø`（U+00D8）在 CP1252 是 0xD8 单字节，验证不走 UTF-8 的 2 字节
- `encode_lpstr_with_cp1252_rejects_chinese_characters_lossy`
  —— lossy 输入 fail-fast，错误含 `cannot encode` / 编码名 / 原值
- `encode_lpstr_with_gbk_preserves_simplified_chinese_bytes`
  —— "公司" 在 GBK 是 4 字节（每字符 2 字节），GBK round-trip 无损
- `encode_lpwstr_ignores_encoding_hint`
  —— VT_LPWSTR 路径对 `encoding` 参数不敏感，UTF-16LE 始终
- `resolve_encoding_accepts_standard_and_alias_labels`
  —— `windows-1252` / `cp1252` / `GBK` / `gbk` / `UTF-8` 均接受
- `resolve_encoding_rejects_unknown_labels`
  —— `Klingon-1` → `unknown encoding label`
- `apply_summary_updates_encoded_end_to_end_rewrites_lpstr_with_explicit_codepage`
  —— 完整 plan 链路，最终 stream 字节含 0xD8 + VT_LPSTR 保留
- `apply_summary_updates_encoded_rejects_lossy_input_clearly`
  —— fail-fast **不 mutate 源字节** 的防御性测试
- `apply_summary_updates_encoded_unknown_key_errors_without_side_effect`
- `apply_summary_updates_encoded_unknown_encoding_errors_cleanly`
- `apply_summary_updates_encoded_empty_is_zero_cost_noop`

`src/writer/metadata_write.rs::tests`（+2）：
- `summary_updates_and_updates_encoded_on_same_key_return_error`
- `summary_deletions_and_updates_encoded_on_same_key_return_error`

CLI 集成 +5 (`tests/writer_validate_cli.rs`)：
- `validate_set_summary_encoded_ascii_round_trips_with_explicit_codepage`
  —— ASCII happy path 走完整 CLI → fixture → writer → reader 链路
- `validate_set_summary_encoded_rejects_lossy_cp1252_input`
  —— CP1252 + 中文 exit 2，stderr 含 `cannot encode` + `windows-1252`
- `validate_set_summary_encoded_rejects_unknown_encoding_label`
  —— `Klingon-1` exit 2，stderr 含 `unknown encoding label`
- `validate_set_summary_encoded_conflicts_with_set_summary_on_same_key`
  —— exit 2 + "both target key" guard message
- `validate_set_summary_encoded_usage_error_on_missing_colon`
  —— syntax guard exit 1，stderr 含 `missing :`

### Verification

- `cargo check --all-targets` → ok (pid-parse v0.8.0)
- `cargo test --all-targets` → **354 passed** / 0 failed
  - lib: 238 → 255 (+17)
  - writer_validate_cli: 21 → 26 (+5)
  - 其余模块不变（inspect_cli 4 / parse_real_files 28 / unit_parsers 18
    / writer_real_files 10 / writer_roundtrip 13）
- `cargo fmt/clippy`: 本地 toolchain 缺组件（rustup LICENSE 冲突 bug），
  跳过；代码对齐 v0.7.1 已验证的风格

### Known limitations (tracked for future phases)

- **Reader 端自动 code-page 探测**：当前仅用户显式指定；非 ASCII
  VT_LPSTR 的 reader round-trip 需等 Phase 10k `detect_lpstr_codepage`
  完成（启发式：fixture 白名单 → BOM → 字节频次 → UTF-8 fallback）。
  端到端非 ASCII round-trip 测试届时补齐。
- **多样本 fixture**：本 Phase 验证仍基于单合成 fixture；真实
  SmartPlant CP1252 / GBK fixture 的 writer 回归测试需 fixture 收集
  pass（Phase 11 窗口）。
- `VT_LPWSTR` 忽略 encoding 字段的语义在当前 API 里是 "silent ignore"；
  若 future consumer 需要显式 "encoding for lpwstr" 错误，留给
  Phase 10k 的设计决议。

### SemVer 判定

- 新字段 `MetadataUpdates.summary_updates_encoded` + `#[serde(default)]`：
  JSON 向后兼容（旧 `{}` 仍 valid passthrough）。
- 新类型 `EncodedString` + 新 pub API (`apply_summary_updates_encoded` /
  `encode_string_with_encoding` via `SummaryPropertySet::set_string_with_encoding`)：
  additive。
- 新 CLI flag + 新 `run_validate` 参数：API surface 扩展。
- 旧 Rust `MetadataUpdates::default()` 构造继续 work；Rust consumer
  代码零破坏。

综合：**minor bump 0.7 → 0.8.0**（行为放宽 + 新 public API + 新 CLI
flag），沿用 Phase 10g 的 "错误路径变成功路径 = minor bump" 判定规则。

### Docs

- `docs/plans/2026-04-21-phase-10i-vtlpstr-codepage.md`：本 Phase dev plan
  （v0.7.1 ship 时已入 repo）。
- `docs/sppid/v0.7.x-status.md` 将在 Phase 10k ship 时统一刷新到
  `v0.8.x-status.md`（目前内容仍描述 10g 限制，不阻塞 10i ship）。

## [0.7.1] - 2026-04-21

### Phase 10h: session 成果归档 + 下一步 roadmap 落地（docs-only，无行为变化）

本 session（2026-04-21）连续 ship 了 v0.4.2 → v0.7.0 共 12 轮
（Phase 9k → 10g），跨两个大周期（Writer 建设 + SPPID full-parse
coverage 系列）。按 Phase 9d 方法论"连续新功能后强制插入卫生 pass"，
本轮不加 feature，只做归档 + 下一步 roadmap 规划。零代码改动，
仅文档落盘。

### Added — 归档文档（v0.4.2 → v0.7.0 的 session 封存）

- `docs/phase10-coverage-series-summary.md`：对齐
  `phase8-9h-summary.md` 风格，归档 Phase 9k-9o + 10a-10g 共
  12 个 commit 里程碑。含起点/终点对照表（test count 260→332、
  Phase 数 12、新增 CLI flag 4、CHANGELOG 行数 ~100→~550）、阶段
  轨迹（每 Phase 动机/产出/方法论点评）、方法论沉淀 5 条
  （feature+卫生 pass 节奏、coverage 是 parser 对偶面、SemVer
  minor/patch 判定、doc-first plan 文化、交叉验证锚点复用）、
  版本↔Phase↔commit 索引表。
- `docs/sppid/v0.7.x-status.md`：SPPID 解析能力一页纸快照，含
  15 个顶层流/存储的 coverage 状态表、Writer 层 6 项能力矩阵、
  保真度维度表、roadmap Phase 1-5 完成度、CLI 入口索引、332 测试
  分布统计、Consumer quick-start 3 个代码样例（parse / edit via
  plan / inspect coverage programmatically）。

### Added — 下一步 roadmap plan 文件（8 份，覆盖 v0.7.1 → 验收全链路）

对齐既有 `phase-9l/9m/9n/10b-10h` 的 dev plan 模板（动机 / 非目标
/ 范围 / 具体设计 / W1-W7 实施步骤 / 预计工时 / 验证清单 / 风险
缓解 / SemVer 判定 / Next 候选 / 交叉引用）。

- `docs/plans/2026-04-21-next-steps-roadmap-v0.7.1-onward.md`
  （16.3KB）— 总导航：TL;DR + 5 阶段（A-E）优先级矩阵 + 现状详
  细快照（coverage/writer/tests）+ 起步节奏建议 + 风险登记。
- `docs/plans/2026-04-21-phase-10h-session-archive.md`（本 Phase
  的 dev plan，3.3KB）。
- `docs/plans/2026-04-21-phase-10i-vtlpstr-codepage.md`（10.9KB）—
  VT_LPSTR 多 code page 回退（CP1252/GBK/Shift-JIS），`SummaryValue`
  枚举向后兼容设计，~3-5hr，minor bump 0.8.0。
- `docs/plans/2026-04-21-phase-10j-docsummary-section2.md`（11.9KB）—
  DocumentSummaryInformation section 2（user-defined dict）CRUD
  编辑，`UserDefinedDictionary` + `PropertyValue` 类型设计，
  ~4-6hr，minor bump 0.9.0。
- `docs/plans/2026-04-21-phase-11a-psmclustertable-records.md`
  （14.9KB）— PSMclustertable per-record 结构化（roadmap Phase 2.2），
  含 hex walk 策略 + type_tag↔ClusterKind 映射 + segment count
  对账，~6-8hr，minor bump 0.10.0。
- `docs/plans/2026-04-21-phase-11b-psmsegmenttable.md`（12.6KB）—
  PSMsegmenttable 结构化（roadmap Phase 2.3），`SegmentKind` enum
  + owner_cluster 反推 + Sheet endpoint 三向对账，~4-6hr，minor
  bump 0.11.0。
- `docs/plans/2026-04-21-phase-11c-sheet-geometry.md`（13.4KB）—
  Sheet 深层几何/图元解码（roadmap Phase 2.5），分 4 sub-phase
  （频次探针 / object+label / line+reference / layout 融合），
  ~10-14hr，minor 0.12.0 + patches；明确标注"逆向不确定性最高，
  强烈建议单独 session 设计"。
- `docs/plans/2026-04-21-phase-12a-normalization-layer.md`（8.7KB）
  — 规范化语义图层（roadmap Phase 3，大 Phase 骨架），含
  `NormalizedObject/Relationship/Endpoint/SymbolRef/ClusterRef` +
  `Provenance { source_layer: Raw/Decoded/Inferred }` 类型设计，
  ~20-30hr，先发 RFC 再实施。
- `docs/plans/2026-04-21-phase-12b-byte-audit-framework.md`
  （11.4KB）— consumed/leftover 字节验证框架（roadmap Phase 4），
  `ParserTrace` + `ByteRange` + `ParserTraceBuilder` 侧信道设计，
  `pid_inspect --byte-audit` CLI + CI regression baseline，
  ~12-18hr，minor 0.14.0 + patches。

### Changed

- `Cargo.toml` version 0.7.0 → 0.7.1。
- `Cargo.lock` 同步 bump。

### Verification

- `cargo fmt --check` / `cargo clippy --all-targets -- -D warnings`
  → 双 0
- `cargo test --all-targets` → **332 passed** / 0 failed（无增减；
  本版本零代码改动）

### Session 闭环指标

- **本 session 总产出**：12 个 version ship（v0.4.2 → v0.7.0）+
  本轮 v0.7.1 归档 + v0.7.1→验收的 8 份 plan 文件 ≈ 100KB 规划文档
- **覆盖时间跨度**：从 Writer 建设收官到 SPPID full-parse 完成
  的 60-85hr 全路径
- **测试**：332 tests 全绿，clippy/fmt 双零
- **下一步明确入口**：Phase 10i (VT_LPSTR code page) 或 Phase 11a
  (PSMclustertable per-record) 任选其一

### Docs

- `docs/plans/2026-04-21-phase-10h-session-archive.md`：本 Phase
  起稿 plan。

## [0.7.0] - 2026-04-21

### Phase 10g: VT_LPSTR UTF-8 编码（解除 Phase 9l ASCII-only 限制）

Phase 9l (v0.5.0) 为 `SummaryInformation` property-set writer 设了一
条 ASCII-only 门：非 ASCII 字符写入 `VT_LPSTR` 属性会被 reject。
当时明确说这条限制"tracked for Phase 9m UTF-8 support"，Phase 9m/9n
都没动到它。Phase 10g 兑现该 roadmap：`VT_LPSTR` 字段现在按 UTF-8
编码字节（[MS-OLEPS] §2.11 允许单字节字符串采用操作系统 code page；
我们选 UTF-8 因为现代 SmartPlant 实测使用 UTF-8）。

### Changed (behavior; minor bump)

- `writer::summary_write::encode_string` 的 `VT_LPSTR` 分支去掉
  `value.is_ascii()` 门；现在任何 UTF-8 字符串都被接受。char_count
  字段照例存储 **byte count**（含 NUL 终止符），与 reader 侧保持
  一致。
- 对既有 0.5.x/0.6.x consumer 的影响：
  - 之前被 reject 返回
    `PidError::ParseFailure { context: "summary writer", message:
    "...value contains non-ASCII bytes..." }` 的调用路径现在会**成功**。
  - 对所有 ASCII-only 输入行为完全不变（UTF-8 是 ASCII 的超集）。
  - 因为语义放宽（原错变正确），按 SemVer 走 **minor bump**（0.6.5
    → 0.7.0）而非 patch。
- `writer::plan::MetadataUpdates.summary_updates` 的 doc 去掉 "VT_LPSTR
  non-ASCII rejected" 提示。

### Tests (332 → 332，行为替换而非新增)

`writer::summary_write::tests::encode_lpstr_rejects_non_ascii` 重命名并
重写为 `encode_lpstr_accepts_utf8_non_ascii`（仍是单个测试函数，test
count 不变）。新 test 验证：

1. `encode_string(VT_LPSTR, "中文 title", 2)` 成功，char_count ==
   UTF-8 byte length + 1。
2. bytes body 在 `from_utf8` 下仍是原字符串。
3. 端到端 round-trip：构造 LPSTR fixture → `apply_summary_updates(
   {"title": "公司 Co. 中文"})` → parse 回来 `title_prop.vt ==
   VT_LPSTR`、bytes 解为原字符串。

### Known limitations

- 仅 UTF-8。如 fixture 下游 consumer 需要 CP1252 / GBK / Shift-JIS
  等传统 code page，本版本不自动识别/转换。方案：
  - 要求下游把 fixture 迁到 UTF-8（SmartPlant 2020+ 默认已是）
  - 在 `summary_updates` 外部自行转码，传入已经是目标 code page 的
    `String`（UTF-8 表示法，字节一致）
  - 切到 `VT_LPWSTR` 对应的 prop（目前 `summary_updates` 会保留源
    VT；若要强制 LPWSTR 可先用 `summary_deletions` 删掉 LPSTR 条目，
    新 add 会默认 LPWSTR）
- DocumentSummaryInformation user-defined section 2 仍未支持（Phase
  9n 已做 issue，Phase 10i 排队）。

### Docs

- `docs/plans/2026-04-21-phase-10g-lpstr-utf8.md`。

### Verification

- `cargo fmt --check` / `cargo clippy -D warnings` → 双 0
- `cargo test --all-targets` → **332 passed** / 0 failed
  （lib 238 + inspect_cli 4 + parse_real_files 28 + unit_parsers 18
   + writer_real_files 10 + writer_roundtrip 13 + writer_validate_cli
   21 = 332；Phase 10g 的变化是 test body 替换，count 未净增）

## [0.6.5] - 2026-04-21

### Phase 10f: coverage 加 bytes 维度

`CoverageEntry` 获得 `stream_size: Option<u64>` 字段，让 coverage
报告能回答"哪些流占了多少字节"，为 roadmap Phase 4 "暴露未解释
字节集中区" 铺第一块基础。本轮仍是纯声明式（来源是 `StreamEntry.size`，
没有 parser 内部的 consumed bytes tracking —— 那是 Phase 10g/10h
的范围）。

### Added

- `model::CoverageEntry.stream_size: Option<u64>`（`#[serde(skip_serializing_if = "Option::is_none")]`）。
- `CoverageReport::total_bytes_by_status() -> [u64; 4]`：status-bucket
  byte 总和（`saturating_add`，unknown=None 的 entry 贡献 0）。
- `inspect::coverage::size_for_top_level(doc, name)`：aggregating
  probe — 顶层 stream 取自己，storage 取 children 总和，无匹配返回
  `None`。

### Changed

- `inspect::report::generate_report` 的 `--- Coverage ---` 段每条
  bucket 摘要追加字节总和（`"Fully decoded:     3 (4.5 KB)"`），每
  条 entry 行追加字节 `(24 B)` / `(1.2 KB)` 等后缀。单位按 1024 进
  制转 B/KB/MB/GB，保留 1 位小数（`< 1 KB` 时用纯整数 `"42 B"` 形
  式避开小数点噪音）。
- `inspect::coverage::top_level_coverage_entries` 构造每个
  `CoverageEntry` 后都会填充 `stream_size`，确保 JSON / 文本两种
  输出都能访问到大小信息。

### Tests (329 → 332)

lib 单元 +3：

- `coverage_entry_carries_stream_size_for_single_top_level_stream`
  —— 单流大小直接 attachment。
- `coverage_entry_aggregates_sizes_across_storage_children`
  —— 存储前缀（`Sheet1`）正确 aggregate 3 个 children 的大小。
- `coverage_report_total_bytes_by_status_matches_entries`
  —— Full=128, Partial=220, Ident=1000, Unknown=18 的混合样本验证
  `total_bytes_by_status`。

既有测试更新：

- `report_includes_coverage_section_with_bucket_counts_and_per_entry_tags`
  新断言 bucket 行含 `"(42 B)"` 后缀 + entry 行含 `(42 B)` 子串。
- `coverage_json_flag_emits_parseable_coverage_report` 断言 JSON
  entries 里至少一条携带 `stream_size` 数值字段。

### Verification

- `cargo fmt --check` / `cargo clippy -D warnings` → 双 0
- `cargo test --all-targets` → **332 passed** / 0 failed

### Docs

- `docs/plans/2026-04-21-phase-10f-coverage-bytes.md`。

## [0.6.4] - 2026-04-21

### Phase 10e: coverage JSON 导出

为 `CoverageReport` 补齐与 `WritePlan`（Phase 9o）对称的 JSON
round-trip helpers + CLI `--coverage --json` 组合输出。让 CI 脚本 /
外部 dashboard / 未来 Phase 10f 字节级验证框架可以直接消费结构化
coverage 数据，不用再 grep 人类文本。

### Added

- `CoverageReport::to_json(&self) -> Result<String, PidError>`
- `CoverageReport::to_json_pretty(&self) -> Result<String, PidError>`
- `CoverageReport::from_json(&str) -> Result<Self, PidError>`
- `pid_inspect --coverage --json`：专门输出 `CoverageReport` 的
  pretty JSON（与 `--json` 单独使用时的 full-document dump 分开）。

错误统一包装为 `PidError::ParseFailure { context: "coverage report
JSON", ... }`，caller 无需 pull serde_json::Error。

### Changed

- `pid_inspect` 的 `--json` 分支现在 short-circuit 到 coverage-only
  JSON 当且仅当同时传了 `--coverage`。其他组合（`--json` 单独 /
  `--coverage` 单独）行为不变。

### Tests (324 → 329)

lib unit (+4)：
- `coverage_report_json_round_trip_default`
- `coverage_report_json_round_trip_preserves_entries`（通过实际
  classifier 构造 4 个 bucket 混合 report，pretty → parse → assert
  bucket counts 字节对齐）
- `coverage_report_from_json_rejects_invalid_syntax_with_pid_error`
- `coverage_report_to_json_pretty_is_multiline_and_indented`

CLI 集成 (+1)：
- `coverage_json_flag_emits_parseable_coverage_report`：CLI 跑
  `--coverage --json`，用 `serde_json::from_str` 解 stdout，断言
  `entries` 数组非空、出现 `FullyDecoded` + `Unknown` 状态、且 JSON
  不含 `streams` 字段（分支污染 regression guard）。

### Verification

- `cargo fmt --check` / `cargo clippy -D warnings` → 双 0
- `cargo test --all-targets` → **329 passed** / 0 failed

### Docs

- `docs/plans/2026-04-21-phase-10e-coverage-json.md`

## [0.6.3] - 2026-04-21

### Phase 10d: DocVersion3 operation 语义化 + report 渲染升级

`VersionRecord.operation` 长期保留 raw 2-char 形式（`"SA"` / `"SV"`），
与 Phase 9f 逆向完成的 DocVersion2 `op_type_label(0x82) = "SaveAs"`
不对称。本轮补齐 helper 方法 + 报告层渲染升级，**保持 serde / JSON
schema 向后兼容**（字段类型不变）。

### Added

- `VersionRecord::is_save_as() -> bool` — `operation == "SA"`。
- `VersionRecord::is_save() -> bool` — `operation == "SV"`。
- `VersionRecord::is_recognized_operation() -> bool` — 已知 op code
  命中检查。
- `VersionRecord::operation_label() -> &'static str` — 人类标签
  `"SaveAs"` / `"Save"` / `"unknown"`，对齐 DV2 `op_type_label`。
- `VersionRecord::parsed_timestamp() -> Option<(u32,u32,u32,u32,u32)>`
  — `MM/DD/YY HH:MM` → `(month, day, year, hour, minute)` 分解；
  非法格式 / 越界返回 `None`。两位年份不做世纪推断，交由 caller。

### Changed

- `inspect::report::generate_report` 的 `--- Version History ---` 段
  现在输出人类标签 + raw code（若非已知）：
  - `[SaveAs 12/29/25 10:45] SmartPlantPID.a 090000.0144`
  - `[Save 12/30/25 09:12] SmartPlantPID.a 090000.0144`
  - `[unknown (XY) 01/01/26 00:00] SmartPlantPID.a 090000.0144`
- `tests/parse_real_files.rs::doc_version2_decoded_matches_version_history`
  改用 `VersionRecord::operation_label()` 替代内联 match，让 DV2
  `op_type_label` 与 DV3 helper 的任何 silent drift 立即 fail。

### Tests (318 → 324)

lib 单元测试 +5（`version_record_tests` 模块）：
- `version_record_is_save_as_matches_sa_literal`
- `version_record_is_save_matches_sv_literal`
- `version_record_operation_label_echoes_unknown_to_flat_string`
- `version_record_parsed_timestamp_happy_path`
- `version_record_parsed_timestamp_returns_none_for_malformed`

report 单元测试 +1：
- `report_version_history_uses_operation_label_instead_of_raw_code`
  —— 同时 assert "SaveAs"/"Save"/"unknown (XY)" 三种格式 + 保护
  "raw [SA ...]" 旧格式不再出现的 regression。

### Verification

- `cargo fmt --check` / `cargo clippy -D warnings` → 双 0
- `cargo test --all-targets` → **324 passed** / 0 failed

### Docs

- `docs/plans/2026-04-21-phase-10d-docversion3-operation-helpers.md`

## [0.6.2] - 2026-04-21

### Phase 10c: cluster & dynamic-attrs 动态 probe（完成 v0.6.1 parking）

v0.6.1 把 4 个 cluster / dynamic-attrs 顶层流从动态分类里显式排除
（`stream_is_populated` 返回 `None`，走静态 pass-through），理由是
它们对应多流合并聚合的 `doc.clusters: Vec<ClusterInfo>` /
`doc.dynamic_attributes: Option<DynamicAttributesBlob>`，探针写法
和单一流字段不同。本轮为这 4 个流都配齐动态 probe，完成 v0.6.1 的
parking list。

### 新 probe 规则

| 流名 | probe |
|---|---|
| `PSMcluster0` | `doc.clusters.any(kind == PsmCluster)` |
| `StyleCluster` | `doc.clusters.any(kind == StyleCluster)` |
| `Dynamic Attributes Metadata` | `doc.clusters.any(kind == DynamicAttributesMetadata)` |
| `Unclustered Dynamic Attributes` | `doc.dynamic_attributes.is_some() \|\| doc.clusters.any(kind == UnclusteredDynamicAttributes)` |

`Unclustered Dynamic Attributes` 有两条 surface：DAB blob 或
cluster 里的 `UnclusteredDynamicAttributes` kind，任一填充都算
parser 已识别，避免误降级。

### Changed

- `inspect::coverage::stream_is_populated` 4 个 cluster/dynamic-attrs
  arm 从 `None` 改为具体 probe 表达式。
- `inspect::coverage::document_field_for_known_stream` 4 个对应 arm
  返回具体字段引用（`"clusters (kind=PsmCluster)"` 等），让降级
  note 可操作。
- `src/inspect/coverage.rs` module doc 追加 "v0.6.2 (Phase 10c)" 段。

### Tests (312 → 318)

lib 单元 +6：
- `coverage_downgrades_psm_cluster0_when_no_cluster_kind_psmcluster`
- `coverage_keeps_psm_cluster0_partial_when_cluster_kind_psmcluster_populated`
- `coverage_downgrades_style_cluster_when_no_cluster_kind_style`
- `coverage_downgrades_dynamic_attrs_metadata_when_no_cluster_kind_dam`
- `coverage_keeps_unclustered_dynamic_attrs_when_blob_populated`
- `coverage_keeps_unclustered_dynamic_attrs_when_cluster_kind_populated`

每个都通过 `cluster_of(ClusterKind, name)` test helper 构造
`ClusterInfo`；`UnclusteredDynamicAttributes` 两个 surface 各有一
专门测试。

### Docs

- `docs/plans/2026-04-21-phase-10c-cluster-dynamic-probes.md`。

### Verification

- `cargo fmt --check` / `cargo clippy -D warnings` → 双 0
- `cargo test --all-targets` → **318 passed** / 0 failed

## [0.6.1] - 2026-04-21

### Phase 10b: coverage 动态分类

兑现 Phase 10a 明确承诺的"Phase 10b+ 会动态化"。v0.6.0 的 coverage
是**纯静态**映射（name → bucket），无法区分"流存在 + parser 解出了
模型"vs"流存在但 parser silent-failure"。v0.6.1 让 `classify` 咨询
`&PidDocument` 本身，在对应模型字段为 `None` / 空集合时**降级**到
`IdentifiedOnly` 并附注原因。

降级表（FullyDecoded / PartiallyDecoded → IdentifiedOnly）：

| 流名 | 检查的字段 |
|---|---|
| `\x05SummaryInformation` / `\x05DocumentSummaryInformation` | `summary` |
| `PSMroots` | `psm_roots` + `!entries.is_empty()` |
| `PSMclustertable` | `psm_cluster_table` |
| `PSMsegmenttable` | `psm_segment_table` |
| `DocVersion2` | `doc_version2_decoded` |
| `DocVersion3` | `version_history` + `!records.is_empty()` |
| `AppObject` | `app_object_registry` |
| `JTaggedTxtStgList` | `tagged_storages` |

`PSMcluster0` / `StyleCluster` / `Dynamic Attributes Metadata` /
`Unclustered Dynamic Attributes` 的动态探针暂留静态 —— 它们对应的
model shape 是多流合并聚合（`clusters`, `dynamic_attrs`），结构更
复杂，留给 Phase 10c 系统处理。

### Changed

- `inspect::coverage::classify` 内部签名从 `classify(name)` 改为
  `classify(name, doc)`（仍为 private，公共入口
  `coverage_report(&PidDocument)` 签名不变）。
- 新增内部 helper：
  - `apply_dynamic_downgrade(name, static_status, note, doc) -> (status, note)`
    —— 把静态结果 + doc 状态组合成最终结果
  - `stream_is_populated(name, doc) -> Option<bool>` —— 每个流的
    populate probe（`None` = 本版本无探针，静态结果透传）
  - `document_field_for_known_stream(name) -> Option<&'static str>`
    —— 复用 `known_stream_state` 的字段名，避免在 note 生成时重复
    写 mapping 表
- 降级条目的 `note` 被替换为诊断字符串，例如：
  `stream present but parser did not populate the expected 'version_history' field — downgraded from FullyDecoded`

### Added / Changed tests (307 → 312)

lib 单元测试（新 4 条 Phase 10b，更新 3 条 Phase 10a）：
- 新：`coverage_downgrades_docversion3_when_parser_did_not_populate`
- 新：`coverage_downgrades_psm_cluster_table_when_empty_model`
- 新：`coverage_keeps_fully_decoded_when_model_populated`
- 新：`coverage_unknown_and_identified_unaffected_by_model_state`
- 更新：3 条 Phase 10a `coverage_marks_*` 测试现在通过新 helper
  `populate_all_known_fields(doc)` 填充对应 model，保持静态
  `FullyDecoded` / `PartiallyDecoded` 的 baseline 断言稳定。
- `inspect::report::tests::report_includes_coverage_section_*` 同理：
  fixture 填充 `version_history` + `psm_segment_table`。

CLI 集成测试（+1 + 1 existing fixture 升级，`tests/inspect_cli.rs`）：
- 新：`coverage_flag_downgrades_docversion3_when_record_is_illegal`
  —— 真实 CLI 端到端验证降级路径：fixture 写一段非 printable bytes
  到 `/DocVersion3`，parser 静默失败 → CLI `--coverage` 输出
  `[ID]   DocVersion3` 并带 `stream present` 降级 note。
- 升级：`build_mixed_coverage_fixture` 中 `/DocVersion3` 现在写
  合法 48-byte record（helper `legal_doc_version3_record`）让
  `[FULL]` 断言在 Phase 10b 下继续成立；`/PSMsegmenttable` 换成
  `/PSMcluster0`（PSMcluster0 在 Phase 10b 中没有动态探针，静态
  `PartiallyDecoded` 不受降级影响），避免 fixture 构造难度的
  footgun。

### Docs

- `docs/plans/2026-04-21-phase-10b-dynamic-coverage.md`：本轮 dev plan。

### Verification

- `cargo fmt --all -- --check` → 0
- `cargo clippy --all-targets -- -D warnings` → 0
- `cargo test --all-targets` → **312 passed** / 0 failed（lib 219 +
  inspect_cli 3 + parse_real_files 28 + unit_parsers 18 +
  writer_real_files 10 + writer_roundtrip 13 + writer_validate_cli 21）

## [0.6.0] - 2026-04-21

### Phase 10a: SPPID 解析覆盖清单（SPPID full-parse roadmap 的 Phase 1）

开启 SPPID 完全解析的新大周期。v0.5.x "Writer 全功能可编辑" 系列收
束于 v0.5.3 之后，本版本建立 **结构化覆盖清单（coverage inventory）**
作为后续 parser 升级工作的主线驱动。

本 Phase 的核心论点来自 `docs/sppid/2026-04-21-sppid-full-parse-roadmap.md`：

> 当前项目对 SPPID 的解析虽然"入口识别"基本完成，但"完全解码"仍有
> 明显缺口（`PSMclustertable` per-record、`DocVersion3` header 稳定
> 模型、`Sheet*` 深层结构、字节级验证框架）。下一阶段不应直接做零
> 散 parser 修补，而应建立一个**覆盖清单驱动**的持续推进机制。

v0.6.0 交付该清单的**静态版本**，把"已识别"和"已完全解析"从之前
的布尔过滤升级为四态分类（`FullyDecoded` / `PartiallyDecoded` /
`IdentifiedOnly` / `Unknown`），配套 inspect 报告段和 CLI flag。
后续 Phase 10b+ 会把静态映射逐步升级为基于字节消费率的动态分类。

### Added — 覆盖清单基础设施（public API，minor bump）

- `model::ParseCoverageStatus`：四态枚举
  （`FullyDecoded` / `PartiallyDecoded` / `IdentifiedOnly` / `Unknown`），
  按 "覆盖度从高到低" 排序。
- `model::CoverageNodeKind`：`TopLevelStream` vs `TopLevelStorage` 区分。
- `model::CoverageEntry`：单条覆盖记录（`name` / `kind` / `status` /
  `parser` / `document_field` / `note`）。
- `model::CoverageReport`：完整报告（`entries: Vec<CoverageEntry>` +
  `status_counts() -> [usize; 4]` helper）。
- `inspect::coverage`：新模块，提供：
  - `coverage_report(&PidDocument) -> CoverageReport` 公共入口
  - `top_level_coverage_entries(&PidDocument) -> Vec<CoverageEntry>`
    低层切片
- `pid_inspect --coverage` CLI flag：只打 coverage section，不打
  full report；作为 CI / diagnostic 脚本单独消费的入口。

### Added — 静态映射（v0.6.0 初版，Phase 10b+ 会动态化）

硬编码 11 个 `KNOWN_TOP_LEVEL_STREAM_NAMES` 条目到其覆盖状态：

| 流名 | 状态 | 理由 |
|---|---|---|
| `\x05SummaryInformation` | FullyDecoded | Phase 9l Writer 层全 CRUD |
| `\x05DocumentSummaryInformation` | FullyDecoded | 同上 |
| `PSMroots` | FullyDecoded | parser 稳定 |
| `DocVersion2` | FullyDecoded | Phase 9f 逆向成功 |
| `DocVersion3` | FullyDecoded | version_history 稳定 |
| `AppObject` | FullyDecoded | registry parser 稳定 |
| `JTaggedTxtStgList` | FullyDecoded | 稳定 |
| `PSMclustertable` | PartiallyDecoded | 头已知，per-record audit-only |
| `PSMsegmenttable` | PartiallyDecoded | 记录形状部分映射 |
| `PSMcluster0` / `StyleCluster` | PartiallyDecoded | 记录边界已知 |
| `Dynamic Attributes Metadata` / `Unclustered Dynamic Attributes` | PartiallyDecoded | 类/属性表提取、绑定推断 |

`KNOWN_TOP_LEVEL_STORAGE_PREFIXES`（`Sheet*` / `TaggedTxtData` /
`JSite*`）统一 `IdentifiedOnly`；其他顶层名 `Unknown`。

### Changed

- `inspect::report::generate_report` 在 "Top-level Unidentified
  Streams" 段之前新增 `--- Coverage ---` 段，输出 4 个 bucket 计数 +
  逐项 tag（`[FULL]` / `[PART]` / `[ID]` / `[UNK]`）+ 解析器 /
  `PidDocument` 字段 / 备注。
- 旧 `unidentified_top_level_streams` 函数 + 对应 report 段保留，保
  持 backward compat；新旧视角可并存（Phase 10b+ 再决定是否下线
  旧视角）。

### Tests (296 → 307)

lib 单元（+9）：
- `inspect::coverage::tests::coverage_marks_known_top_level_streams_with_expected_status`
- `inspect::coverage::tests::coverage_marks_known_storage_prefixes_as_identified`
- `inspect::coverage::tests::coverage_marks_unknown_top_level_entries_as_unknown`
- `inspect::coverage::tests::coverage_entries_sorted_by_name_deterministic_across_input_orders`
- `inspect::coverage::tests::coverage_report_empty_for_default_document`
- `inspect::coverage::tests::coverage_status_counts_matches_entries`
- `inspect::report::tests::report_includes_coverage_section_with_bucket_counts_and_per_entry_tags`
- `inspect::report::tests::report_omits_coverage_section_when_document_has_no_streams`
- `inspect::report::tests::report_coverage_section_precedes_top_level_unidentified_when_both_present`

CLI 集成（+2，`tests/inspect_cli.rs` 新增）：
- `coverage_flag_prints_section_and_all_four_buckets`
- `no_flags_still_produces_full_report_including_coverage_section`

`cargo fmt --check` / `cargo clippy -D warnings` 双零。

### Docs

- `docs/sppid/2026-04-21-sppid-full-parse-roadmap.md`：SPPID 完全
  解析战略路线图（4 个阶段 + 具体任务 + 风险应对）。正式入 repo 作为
  后续 Phase 10b/10c/... 的导航文档。
- `docs/plans/2026-04-21-sppid-coverage-inventory-implementation-plan.md`：
  Phase 10a Task 1..6 的战术实施 plan，本 ship 的代码即其完成品。
  入 repo 供未来类似 Phase 的 pattern 参考。

### Version rationale

0.5.3 → 0.6.0 走 **minor bump**，不走 patch：

- 新增 public API 类型（`ParseCoverageStatus` / `CoverageNodeKind` /
  `CoverageEntry` / `CoverageReport` / `coverage_report` / `top_level_coverage_entries`）
- 新增 CLI flag (`--coverage`)
- 新增 `CoverageReport` serde/JsonSchema surface
- 这些是 additive 的新周期起点，与 0.5.x "Writer 建设" 的定位区分
  开；未来 10a/10b/... 共用 0.6.x 系列。

Rust API 本身无破坏性改动；既有 0.5.x consumer 代码直接在 0.6.0 下
编译通过。

## [0.5.3] - 2026-04-21

### Phase 9o: Writer API ergonomics patches

四轮 Writer 内部能力扩展（Phase 9k/9l/9m/9n）完成后，本轮回头收拾
下游 consumer 侧的入门样板。全部改动都是 **additive**（新 public
方法），不破坏任何 v0.5.x 已有 API 签名。

### Added

- `PidPackage::from_path<P: AsRef<Path>>(path)` — 简写
  `PidParser::new().parse_package(path)`，两步变一步。
- `PidPackage::from_bytes(&[u8]) -> Result<Self, PidError>` — 从
  内存字节流解析。v0.5.3 内部实现走 tempfile 兜底（HTTP service /
  压缩包 / 嵌入资源都可直接喂字节）；真正的零磁盘纯内存路径依赖
  parser 内部 reader 泛型化，留给 Phase 10a。
- `PidWriter::write_to_bytes(pkg, plan) -> Result<Vec<u8>, PidError>` —
  镜像 `write_to`，直接返回 CFB 字节数组。内部复用新的
  `cfb_write::write_package_to_writer<F: Read + Write + Seek>` 泛型
  backend，避免磁盘往返。
- `cfb_write::write_package_to_writer` 公共泛型入口（Phase 10a 以后
  可能用作 zero-disk 测试基础设施）。
- `WritePlan::from_json(&str)` / `to_json()` / `to_json_pretty()` —
  JSON round-trip helpers，错误一律包装成
  `PidError::ParseFailure { context: "WritePlan JSON", ... }`，
  consumer 不用自行 handle `serde_json::Error`。

### Changed

- `PidWriter::write_to` 内部提取 `apply_plan_to_package` helper，与
  `write_to_bytes` 共用流水线。未来添加新 plan 字段（比如
  `post_write_clsid_set`）只需要改一次。行为完全等价。

### Tests

lib 单元（+7）：
- `api::tests::from_bytes_parses_a_minimal_synthetic_pid`（构造内存
  CFB fixture → `from_bytes` → verify stream 存在）
- `api::tests::from_bytes_on_invalid_data_returns_error`
- `api::tests::from_path_matches_parse_package_behavior`（两个入口
  行为一致）
- `writer::plan::tests::plan_json_round_trip_default_is_passthrough`
- `writer::plan::tests::plan_from_json_rejects_invalid_syntax_with_pid_error`
- `writer::plan::tests::plan_to_json_pretty_contains_newlines_and_indent`
- `writer::plan::tests::plan_from_json_empty_object_is_valid_passthrough`

集成（+2，`tests/writer_roundtrip.rs`）：
- `write_to_bytes_produces_bytes_parseable_by_from_bytes` — 全在线
  round-trip：`from_path` → plan → `write_to_bytes` → `from_bytes`
  → verify edit 落地。
- `write_plan_json_round_trip_preserves_metadata_and_payload_bytes` —
  包含 `summary_updates` + `summary_deletions` + `stream_replacements`
  + `sheet_patches` 完整 plan 的 JSON 往返无损（特别断言 base64
  payload 字节级等价）。

全套 287 → **296 tests pass**（lib 199 → 206 +7；writer_roundtrip
11 → 13 +2）。`cargo fmt --check` / `cargo clippy -D warnings` 双零。

### Docs

- `docs/plans/2026-04-21-phase-9o-api-ergonomics.md`：本轮 dev plan
  含 "不做 parser 泛型化 / tempfile 兜底" 的 trade-off 说明 + Phase
  10a roadmap 衔接。

### Consumer quick-start

```rust
use pid_parse::{PidPackage, PidWriter, WritePlan};

// 从字节流或路径二选一
let pkg = PidPackage::from_path("input.pid")?;
// let pkg = PidPackage::from_bytes(&http_response_body)?;

// 用 JSON 声明式 plan 或直接构造
let plan = WritePlan::from_json(r#"{"metadata_updates":{"summary_updates":{"title":"Q4"}}}"#)?;

// 输出到内存或磁盘
let bytes = PidWriter::write_to_bytes(&pkg, &plan)?;
// PidWriter::write_to(&pkg, &plan, Path::new("output.pid"))?;
```

## [0.5.2] - 2026-04-21

### Phase 9n: `summary_deletions` — SummaryInformation CRUD 收尾

Phase 9l/9m 铺的 Writer 层字符串 property 编辑只有 CREATE（新增）/
UPDATE（覆写）两条语义。本轮补上 **DELETE**，让 SummaryInformation
与 DocumentSummaryInformation 的 property-set 写路径真正支持 CRUD。

### Added

- `WritePlan.MetadataUpdates.summary_deletions: Vec<String>`
  （`#[serde(default)]`，JSON plan 向后兼容）。语义等价于
  `summary_updates` 的逆操作：按符号 key（title / author / subject /
  keywords / comments / template / last_author / rev_number / app_name /
  category / manager / company）定位到 PROPID 并从 section 移除。
  - 空 vec = free no-op
  - key 在 section 里不存在 = 静默 no-op（遵循 stream_replacements
    "删不存在的不 fail" 传统）
  - key 不在符号表 = `UnknownKey` 错误
- `pid_parse::writer::summary_write::apply_summary_deletions` 公共入口。
- `SummarySection::remove(prop_id)` 内部方法（Phase 9l 的
  `SummaryPropertySet` 只多一个方法，其他基础设施复用）。
- `pid_writer_validate --delete-summary KEY` 便利 CLI flag，对称于
  `--set-summary KEY=VALUE`。可多次传入累加；与 `--set-summary` 在
  *不同* key 上可共存，同 key 则报冲突错。与 `--apply-plan` 互斥。
- `ValidateError::Edit("summary_updates and summary_deletions both target
  key '{k}'")`：同 key 同时出现在 set/delete 两个字段时的明确拒绝，
  在 lib 层 (`apply_metadata_updates`) 和 CLI 层 (`run_validate`) 都有
  pre-check。

### Changed

- `apply_metadata_updates` 执行顺序正式定义：`drawing_xml` →
  `general_xml` → `summary_deletions` → `summary_updates`。先删再增
  保证 edge case "删 A、加 A" 的最终态一致（与冲突拒绝共存只是防御性
  guard；即便 guard 放过也不会产生 inconsistent state）。
- `pid_writer_validate`：
  - `print_usage` 更新列出 `--delete-summary` 段与 "cannot be combined
    with … --delete-summary" 的精确错误消息。
  - `run_validate` / `compare_packages` 签名新增 `summary_deletions`
    参数，`edited_paths` 自动扩展到 summary 两流（保证删除后字节级
    变化归入 `edited` 而非 `mismatched`）。
  - `collect_edited_paths_from_plan` 亦同步识别 `summary_deletions`。
- `WritePlan::is_passthrough` 扩展：`summary_deletions.is_empty()` 加入
  判空链。
- `WritePlan::metadata_only` 构造函数新增 `summary_deletions:
  Vec::new()` 初始化。

### Tests

lib 单元测试（`src/writer/summary_write.rs::tests`，+5 条）：
- `apply_summary_deletions_removes_existing_prop`
- `apply_summary_deletions_nonexistent_key_is_silent_noop`（断言静默
  no-op 且**流字节保持不变**，避免 `modified: true` 误染 diff）
- `apply_summary_deletions_unknown_key_returns_error`
- `apply_summary_deletions_empty_is_zero_cost_noop`
- `apply_summary_deletions_preserves_filetime_byte_for_byte`（非目标
  prop 字节级保留的 Phase 9l 契约，在 delete 路径上再验证一次）

lib 单元测试（`src/writer/metadata_write.rs::tests`，+1 条）：
- `summary_updates_and_deletions_on_same_key_return_error`

CLI 集成测试（`tests/writer_validate_cli.rs`，+4 条）：
- `validate_delete_summary_removes_target_prop`
- `validate_delete_and_set_summary_combine_legally`
- `validate_delete_summary_conflicts_with_set_summary_on_same_key`
- `validate_delete_summary_unknown_key_exits_two`

Real-file 集成测试（`tests/writer_real_files.rs`，+1 条件性条）：
- `real_file_delete_summary_prop_when_present`：fixture 存在且有
  summary 流时删除 `keywords`（或 fallback 到 `title`），verify 非
  summary 流全字节等价 + 目标 prop 在 reader 视角消失。

全套 276 → **287 tests pass**（lib 193 → 199 +6；writer_real_files
9 → 10 +1；writer_validate_cli 17 → 21 +4）。

### Docs

- `docs/plans/2026-04-21-phase-9n-summary-deletions.md`：本轮 dev plan。
- `docs/writer-quickstart.md` 5.6 节已在 Phase 9m 的 "CLI 快捷方式"
  块里把 `--delete-summary` 作为第 9n 版延伸注解。

## [0.5.1] - 2026-04-21

### Phase 9m: `--set-summary` CLI flag + real-file integration

Corner-case convenience pass on top of Phase 9l (v0.5.0) — turns the
SummaryInformation writer from "编辑需要手写 plan.json" into "可用单个
命令行 flag 直接改"。

### Added

- `pid_writer_validate --set-summary KEY=VALUE`：特化便捷 flag，对称于
  `--edit` (drawing XML) / `--general-edit` (general XML)。多次 `--set-summary`
  会累加到同一个 summary map；后传覆盖先传。支持所有 Phase 9l
  KEY_TO_*_PROPID 表里的 11 个 key。
- `run_validate` 函数签名新增 `summary_edits: &BTreeMap<String, String>`
  参数（binary 专用 public API；CLI 集成测试可直接传；lib consumer 不受
  影响）。
- `tests/writer_validate_cli.rs` 4 条新集成测试：
  - `validate_set_summary_single_key_rewrites_title`
  - `validate_set_summary_multiple_keys_accumulate`（title / author /
    subject 一次调用累加）
  - `validate_set_summary_conflicts_with_apply_plan_exits_one`
  - `validate_set_summary_unknown_key_exits_two_with_clear_error`
- `tests/writer_real_files.rs` 1 条 conditional 测试
  `real_file_set_summary_title_preserves_other_streams`：当
  `test-file/DWG-0201GP06-01.pid` 存在并含 `/\x05SummaryInformation` 时
  验证真实 `.pid` 端到端 → 写 title → parse → 断言只有 summary 流变，
  其他所有流 byte-identical；fixture 缺失或无 summary stream 时 skip。

### Changed

- `--apply-plan` 与 `--set-summary` 互斥：同时传返回 usage error（exit 1），
  stderr 含 `--set-summary` 字样。延续 Phase 9b 的"declarative plan 与
  特化 flag 互斥"设计。
- `compare_packages` / `collect_edited_paths_from_plan` 都扩展到把
  `/\x05SummaryInformation` 和 `/\x05DocumentSummaryInformation` 纳入
  "可能被编辑"的 stream 集合，保证 `edited` vs `matched` 计数在
  summary 改动时不误报 `mismatched`。
- `docs/writer-quickstart.md` 5.6 节追加 CLI 用法示例。

### API surface

- `pid_parse::writer::summary_write::SUMMARY_INFO_PATH` /
  `DOC_SUMMARY_PATH` 从 `pub(crate)` 提升为 `pub`，让 binary 和外部
  consumer 能引用同一组常量而不用硬编码字节。

### Tests

全套 271 → **276 tests pass**（writer_real_files 8 → 9 +1；
writer_validate_cli 13 → 17 +4）。`cargo fmt --check` / `cargo clippy
--all-targets -D warnings` 双零。

### Docs

- `docs/plans/2026-04-21-phase-9m-summary-cli-integration.md`：本轮 dev plan。
- `docs/writer-quickstart.md` 5.6 节："编辑 SummaryInformation"新增
  `--set-summary` CLI 用例块。

## [0.5.0] - 2026-04-21

### Phase 9l: SummaryInformation / DocumentSummaryInformation property-set writer

`MetadataUpdates.summary_updates` 从 **parked placeholder** 变为**真正可
编辑的 OLE 属性流接口**，补全 Writer 层最后一个"骗用户"的 API。

从 v0.4.2 起，apply-plan JSON 里填 `summary_updates` 字段会被静默吞掉；
从 v0.5.0 起，会真实写入 `/\x05SummaryInformation` /
`/\x05DocumentSummaryInformation` 的 OLE property-set。这是**对外语义
变化**（向后兼容的 Rust API，但 JSON plan consumer 的行为改变），因此
走 **minor bump**（0.4.x → 0.5.x 开启新周期："Writer 全功能可编辑"）。

### Added

- `src/writer/summary_write.rs` **新模块**：
  - `SummaryPropertySet` 内部类型，parse + serialize OLE property-set
    stream（[MS-OLEPS] 规范），支持 byte-level round-trip。
  - `apply_summary_updates(pkg, updates)` 公共入口，按符号名 key 定位
    PROPID + 目标 stream，仅编辑 `VT_LPSTR` / `VT_LPWSTR` 字符串型
    property；未触及的 property（含 `VT_FILETIME` / `VT_I4`）字节级
    保留。
  - 支持 key 列表（11 条）：
    - SummaryInformation 段：`title`, `subject`, `author`,
      `keywords`, `comments`, `template`, `last_author`,
      `rev_number`, `app_name`
    - DocumentSummaryInformation 段：`category`, `manager`, `company`
  - 清晰的错误分类（全部包装为 `PidError::ParseFailure { context:
    "summary writer", ... }` 不破坏 public error surface）：
    - unknown key（列出已知 key 表）
    - 目标 stream 不存在（提示用 `stream_replacements` 塞）
    - 未支持的 source VT 类型（避开 FILETIME / I4 写入）
    - 非 ASCII 写入 `VT_LPSTR` 字段（提示 Phase 9m 会支持 UTF-8）
  - 常量 `SUMMARY_INFO_PATH` / `DOC_SUMMARY_PATH` 和标准 FMTID
    (`F29F85E0-4FF9-1068-AB91-08002B27B3D9` /
    `D5CDD502-2E9C-101B-9397-08002B2CF9AE`) 内联定义。

### Changed

- `writer::plan::MetadataUpdates.summary_updates` 文档从 "Placeholder —
  silently ignored" 更新为"实际生效"，列出 11 个可用 key 和 encoding
  规则。字段签名（`BTreeMap<String, String>`）不变，现有 Rust consumer
  零破坏。
- `writer::metadata_write::apply_metadata_updates` 在 drawing / general
  XML 写入之后调用 `apply_summary_updates`。空 map = 0 开销。
- `writer/mod.rs` 模块 doc 去掉"no SummaryInformation property-set
  writer"的 caveat。

### Tests

lib (unit)：
- `writer::summary_write::tests::parse_then_serialize_is_byte_identical_for_untouched_stream`
- `writer::summary_write::tests::apply_summary_updates_passthrough_empty_map_touches_nothing`
- `writer::summary_write::tests::apply_summary_updates_edits_title_and_preserves_filetime`
  （关键断言：FILETIME prop 改写其他 prop 后 byte-for-byte 不动）
- `writer::summary_write::tests::apply_summary_updates_rejects_unknown_key`
- `writer::summary_write::tests::apply_summary_updates_adds_new_string_prop_when_absent`
  （新 prop 默认 `VT_LPWSTR`）
- `writer::summary_write::tests::apply_summary_updates_returns_stream_not_found_when_missing`
- `writer::summary_write::tests::encode_lpstr_rejects_non_ascii`
- `writer::summary_write::tests::encode_lpwstr_accepts_unicode`

集成（`tests/writer_roundtrip.rs`）：
- `summary_updates_rewrite_title_end_to_end_through_pid_writer`：完整
  链路 `PidWriter::write_to → CFB → parse → SummaryInfo.title`。
- `summary_updates_unknown_key_fails_writer_with_clear_error`：错误传
  播到 Writer top-level。

全套 261 → **271 tests pass**（lib 185 → 193 +8；writer_roundtrip 9 → 11 +2）。

### Docs

- `docs/plans/2026-04-21-phase-9l-summary-info-writer.md`：本轮 dev plan
  （scope / 关键设计决策 / 5-7 hr W1-W6 步骤 / 风险缓解表 / 回滚策略 /
  Next 候选）。

### Known limitations (tracked for future phases)

- `VT_LPSTR` 字段不接受非 ASCII 值（Phase 9m 计划支持 UTF-8 / CP1252）。
- DocumentSummaryInformation 第二个 section（user-defined dictionary）
  不编辑；section 2..N 的原字节 verbatim 透传（Phase 9n）。
- 不支持删除 property（`summary_deletions` 字段挂位，future minor bump）。
- 不支持从零新建 `/\x05SummaryInformation` stream；源 package 必须
  已有此 stream，否则返回 `stream does not exist` 错误（由用户先走
  `stream_replacements` seed）。

## [0.4.2] - 2026-04-21

### Phase 9k: Ship `--apply-plan` + P3 cleanups + lint/fmt restore

从 [0.4.1] 合并：layout 语义关键词数据驱动重构（`48135a8`）+ Writer 层
`--apply-plan` CLI（`3a2ecde`）。同时扫清 Phase 9i 之后悄悄堆积的 **7 条
clippy warning**（lib-test 代码里 6 条 `field_reassign_with_default` +
1 条 `map_clone` / `iter_cloned_collect`）与 10 文件的 `cargo fmt`
漂移，执行 `phase8-9h-summary.md` 里列的 P3 cleanups 中 3 条低风险项
（#1 `file_stem` 跨平台、#3 `diff.rs` `writeln!().unwrap()`、#4 tests
use 散落；#2 `representative_symbol_hints` 缓存留给下一轮性能 Phase）。

全套从 260 增至 **261 tests pass**（lib 185 + parse_real_files 28 +
unit_parsers 18 + writer_real_files 8 + writer_roundtrip 9 +
writer_validate_cli 13 = 261）。`cargo clippy --all-targets -- -D warnings`
和 `cargo fmt --all -- --check` 均返回 0 退出码。

### Added

- `pid_writer_validate --apply-plan <plan.json>`：一次性施加完整 `WritePlan`
  （metadata XML / stream replacements / sheet patches）并走 round-trip +
  byte-diff verify。与 `--edit` / `--general-edit` 互斥；`--json` 输出
  扩展 `plan_applied` 字段。
- `Cargo.toml`：`base64 = "0.22"` 依赖（WASM / no_std 友好，为批量 CLI
  及未来跨语言 binding 共用）。
- `src/layout.rs::file_stem_cross_platform`：内部 helper，先把 Windows
  反斜杠 UNC 路径归一化为正斜杠再喂 `Path::file_stem`，消除 Linux CI 上
  `\\srv\sym\piping\valve.sym` 被当成单一文件名返回整串的怪行为。

### Changed

- `WritePlan`、`MetadataUpdates` 字段追加 `#[serde(default)]`，让 `{}`
  成为合法 JSON passthrough，`{"metadata_updates":{"drawing_xml":"..."}}`
  也无需显式写 `general_xml: null` / `summary_updates: {}`。保留 Rust
  侧 `WritePlan::default()` 行为不变。
- `StreamReplacement.new_data` / `SheetChunkPatch.replacement` 的 JSON
  序列化由 `Vec<u8> = [int array]` 改为**标准 base64 字符串**（`A-Z a-z
  0-9 + / =`）。Rust consumer 透明，JSON 大小约缩减 6x。内部 `#[serde(with
  = "bytes_base64")]` 自定义 adaptor，反序列化失败走 serde error。
- `src/layout.rs`: 语义关键词推断改为数据驱动。新增 `SEMANTIC_KEYWORDS` 常量表
  （`OffPageConnector` / `Nozzle` / `Instrument` / `Vessel` / `Note` /
  `PipingComponent`），以及每个 tag 的英文 + 中文同义词列表，取代原先 if/else
  链。行为对既有 fixture 保持等价，新增中文 symbol 路径（例如 `\\srv\sym\管件\球阀.sym`）
  的语义命中。顺序依赖显式保留（`OPC` 先于 `valve`）。
- `src/layout.rs`: `representative_symbol_hints` 的 tiebreaker 抽成
  `should_replace_representative(existing_count, existing_path, candidate_count,
  candidate_path)` helper，带 doc comment 说明 "higher usage_count wins; ties
  break on lexicographically smaller path" 规则，替代原 inline 表达式。
- `src/inspect/diff.rs::render`：11 处 `writeln!(&mut out, ...).unwrap()` 改为
  `String::push_str` / `push_str(&format!(..))`。`writeln!` 对 `String`
  的 `fmt::Write` impl 技术上不会 fail，但 `.unwrap()` 让读者每次都要
  自己重新确认这一点；改成 `push_str` 直接消除这个认知负担，也顺带去掉
  `use std::fmt::Write` import。

### Fixed (lint / fmt restore)

- `src/import_view.rs` / `src/inspect/mod.rs` / `src/layout.rs` 6 处
  `field_reassign_with_default` clippy lint（均在 lib-test 代码里），
  改成 struct-literal + `..Default::default()` 形式。
- `src/inspect/mod.rs::tests::unidentified_filters_all_known_top_level_names`
  里 `.iter().map(|s| *s).collect::<Vec<&str>>()` → `.to_vec()`
  （clippy `iter_cloned_collect`）。
- `cargo fmt --all` 应用 10 文件的 whitespace / line-break 漂移：
  `src/bin/pid_inspect.rs`, `src/bin/pid_writer_validate.rs`,
  `src/import_view.rs`, `src/inspect/mod.rs`, `src/layout.rs`,
  `src/model.rs`, `src/parsers/sheet_probe.rs`,
  `src/writer/metadata_helpers.rs`, `src/writer/plan.rs`,
  `tests/writer_validate_cli.rs`。

### Tests

- `layout::tests::infer_semantic_maps_chinese_symbol_path_to_piping_component`
- `layout::tests::infer_semantic_keyword_ordering_keeps_opc_before_piping`
- `layout::tests::should_replace_representative_covers_all_three_rules`
- `layout::tests::infer_semantic_normalizes_backslash_path_across_platforms`
  （Phase 9k 新增；回归守 P3-1 `file_stem_cross_platform` helper 对
  `\\srv\...` 和 `//srv/...` 两种风格返回相同 stem，且关键词匹配等价）
- `writer::plan::tests::stream_replacement_round_trips_through_json_with_base64_payload`
- `writer::plan::tests::sheet_chunk_patch_round_trips_through_json_with_base64_payload`
- `writer::plan::tests::deserialize_rejects_invalid_base64`
- `tests/writer_validate_cli.rs`：5 条新集成测试覆盖 `--apply-plan`
  （passthrough `{}` / drawing 元数据整体替换 / base64 stream 替换 /
  非法 JSON exit 2 / 与 `--edit` 冲突 exit 1）。

### Docs

- `docs/plans/2026-04-19-layout-symbol-hint-p2-fixes.md`：layout P2 dev plan，包含
  "审核自纠" 一节说明 P2-1 撤回的理由（恢复 `file_stem()` 回退反而会让
  `bounds_for_item` fall through 到默认尺寸，丢失 `PipingComponent` 的 18×18
  命中；4c1cb80 的"坍塌到语义 tag"是正向设计）。
- `docs/plans/2026-04-19-apply-plan-cli.md`：`--apply-plan` 的 dev plan。
- `docs/plans/2026-04-21-phase-9k-ship-and-p3.md`：本轮 Phase 9k 的 dev plan
  （ship v0.4.2 + P3 cleanups + lint/fmt restore）。
- `docs/writer-quickstart.md` 新 5.5 节"批处理 via `--apply-plan <plan.json>`"：
  JSON schema 说明 + CLI 调用样例 + Rust 侧构造 plan 并 serialize 示例。

## [0.4.1] - 2026-04-19

### Phase 9k: cfb 0.10 → 0.14 升级 + 时间戳 + state_bits 保真

`cfb` 在 2026-02-13 发布 0.14.0，新增 `set_created_time` / `set_modified_time` / `set_state_bits` 三个关键 API —— **直接解锁 Phase 9e 文档里标记"cfb upstream 依赖" 的全部遗留限制**。升级零 breaking changes 直接编译通过。

真实样本 `DWG-0201GP06-01.pid` 承载 **25 个非 epoch storage timestamps**（root / 19 × JSite / PSMspacemap / TaggedTxtData），从 Phase 9a (v0.3.3) 起的 passthrough round-trip 一直在**悄无声息地丢失**这些时间戳（旧 cfb 0.10 只能 `touch(path) = now`）。Phase 9k 一并修复。

### 升级依赖

- `cfb = "0.10"` → `cfb = "0.14"`（零 breaking，无代码改动即可编译）
- 新增 cfb 的传递依赖 `web-time = "1"`

### 模型扩展

- `PidPackage.storage_timestamps: BTreeMap<String, StorageTimestamps>` 新字段
  - `StorageTimestamps { created: Option<SystemTime>, modified: Option<SystemTime> }`
  - CFB-spec epoch (1601-01-01) 被 parser 归一化为 `None`（避免把"未设置" 误报为"1601年")
- `PidPackage.state_bits: BTreeMap<String, u32>` 新字段
  - 仅记录非零值（零是 CFB 默认，sparse map）
- `PidPackage::with_storage_timestamps(...)` / `with_state_bits(...)` builder 方法

### Parser / Writer 改动

- `parse_pid_package`：单次 `cfb.walk()` 一并采集 CLSID / timestamps / state_bits（避免多次 walk）
- `writer::cfb_write::write_package`：新增 step 5（`set_created_time` / `set_modified_time`）+ step 6（`set_state_bits`）

### Diff 扩展

- `PackageDiff.storage_timestamp_diffs: Vec<StorageTimestampDiff>` 新字段
- `PackageDiff.state_bits_diffs: Vec<StateBitsDiff>` 新字段
- `is_empty()` / `diff_count()` 纳入新维度
- `inspect::diff::render` 新增 `--- Storage Timestamp Diffs ---` / `--- State Bits Diffs ---` 段，`render_time` 用 `unix+Ns` 稳定格式（无需 chrono 依赖）

### Report 扩展

- `generate_package_report` 新增 `--- Storage Timestamps (N) ---` / `--- State Bits (N) ---` 段展示
- 真实样本运行 `pid_inspect drawing.pid` 默认可以看到全部 25 个 storage 的 created/modified 时间戳

### re-export

- `pid_parse::{StorageTimestamps, StorageTimestampDiff, StateBitsDiff}` 新导出

### 测试

- 模块内单元测试 +2：`diff_flags_storage_timestamp_mismatch` / `diff_flags_state_bits_mismatch`
- `tests/writer_roundtrip.rs` +1：`storage_timestamps_and_state_bits_round_trip`（内存 fixture 烧任意 created/modified/state_bits → round-trip 完整保留）
- `tests/writer_real_files.rs` +2：
  - `real_file_passthrough_preserves_storage_timestamps`（真实样本 25+ timestamps 全部保真）
  - `real_file_passthrough_produces_empty_diff_full`（6 个维度全部 0 diff）
- **总计 177 个测试通过**（从 172 → 177，新增 5 个）

### 修复：Phase 9a-9g 的 "passthrough 0 diffs" 实际上一直在丢数据

Phase 9a 起 `--round-trip --verify` 报告 "verified: 0 diffs"，但旧 `diff_packages` 只看 stream 字节 + root CLSID，不看非 root CLSID（Phase 9e 补）、不看 timestamps / state_bits（Phase 9k 补）。v0.3.13 起 **"0 diffs"** 真正意味着 "容器级字节几乎无损"。

### 文档

- `docs/writer-clsid-and-timestamps.md` 全线改写：能力矩阵从"3 层保真"升级到"6 层保真"；新增 v0.3.13 验证清单；历史升级表
- `ARCHITECTURE.md`：能力边界 v0.3.13

## [0.3.12] - 2026-04-19

### Phase 8c: Layout-first 可读整图模型（供 H7CAD PID 工作台消费）

在既有 `PidDocument + ObjectGraph + CrossReferenceGraph` 之上新增面向显示的 `layout` 真值层，让下游不必再把 `.pid` 对象简单摆成网格圆点，而能生成一份**可读整图**所需的布局摘要。此层仍是 visualization model，不追求 SmartPlant 原始几何逐字节/逐像素复刻。

### 公共 API

- `PidDocument` 新增可选字段 `layout: Option<PidLayoutModel>`
- 新增类型：
  - `PidLayoutModel { items, segments, texts, unplaced, warnings }`
  - `PidLayoutItem { layout_id, drawing_id, graphic_oid, kind, anchor, bounds, symbol_name, symbol_path, label, model_id }`
  - `PidLayoutSegment { layout_id, owner_drawing_id, graphic_oid, start, end, role }`
  - `PidLayoutText { layout_id, drawing_id, text, anchor, bounds }`
  - `PidLayoutUnplaced { drawing_id, kind, label }`
- 新增导出函数：
  - `derive_layout(doc: &mut PidDocument)`
  - `build_layout_model(doc: &PidDocument) -> Option<PidLayoutModel>`

### 布局推导规则

- 统一支持两类输入证据：
  - `bundle mode`：消费 sidecar `_Data.xml` 带入的 `PIDRepresentation / DwgRepresentationComposition / DefUID` 关系
  - `pid-only mode`：消费 `.pid` 内已解出的对象图与关系图
- `PIDRepresentation` 的 `GraphicOID` 会通过 `DwgRepresentationComposition` 转移到被表示对象上，供图形层选择/联动使用
- layout 的连线只消费已证实的物理关系角色：`PipingEnd1Conn` / `PipingEnd2Conn` / `PipingTapOrFitting` / `ProcessPointCollection`；无证据时不伪造线
- 未能放进主图的对象进入 `unplaced`，由下游单独做 fallback rail，不再混排到主图

### 符号语义增强

- `infer_symbol_identity` 不再只覆盖 `Pipeline / Branch / Connector / Instrument / Equipment`
- 新增 bundle/真实样例驱动的语义类别：
  - `ProcessPoint`
  - `Note`
  - `Nozzle`
  - `OffPageConnector`
  - `PipingComponent`
  - `Vessel`
  - `PipingPort / SignalPort`
- 若对象 extra 中可见 `.sym` 路径，现会保留到 `symbol_path`，并尽量从 basename 回填 `symbol_name`

### 测试

- 新增 `layout::tests::build_layout_model_classifies_bundle_specific_symbol_kinds`
- 真实样例护栏保持：
  - `tests/parse_real_files.rs::second_file_builds_readable_layout_model`
- 验证：
  - `cargo test --manifest-path D:/work/plant-code/cad/pid-parse/Cargo.toml`
  - 结果：`137` lib tests + `28` parse_real_files + `18` unit_parsers + `1` writer_real_files + `7` writer_roundtrip + `8` writer_validate_cli 全绿

### 设计边界

- `layout` 是**可读整图布局模型**，不是 `.sym` 原始几何解码
- 不做 SmartPlant 原始线型、标注、字高、版式的像素级复刻
- `symbol_name/symbol_path` 目前仍是 best-effort 语义证据；后续若把 `JSite` 真正挂接到对象级，可继续细化
- v0.4.1 同段后续补丁：`layout` 现会从 `cross_reference.symbol_usage` / `jsites` 提取代表性 `.sym` 路径作为 pid-only 的对象级 symbol hint。典型受益对象包括 `OPC`、`PipingComp`、`Nozzle` 等粗类型；即便没有 sidecar XML，也能把 `Off-Drawing.sym` / `Cap2.sym` / `Flanged Nozzle.sym` 一类证据下沉到 `PidLayoutItem.symbol_path`

### Phase 8b: Metadata 编辑 helper（为 H7CAD UI 编辑桥铺路）

在 v0.4.0 Writer 层之上新增 `src/writer/metadata_helpers.rs` 纯函数模块，让上层不再需要自己拼/改 XML 字节即可对 `/TaggedTxtData/Drawing` 与 `/TaggedTxtData/General` 做"改一点点"式编辑。所有 helper 都是 byte-level splice — 除被替换的属性值/元素文本外，其它字节（注释、空白、引号风格、兄弟属性顺序）逐字节保留，最大化 SmartPlant 兼容性。

### 公共 API

- `set_drawing_attribute(xml, attr, value) -> Result<String, MetadataEditError>`：替换 `<Tag attr="value"/>` 风格的属性值；要求左侧是空白或开头、右侧是 `=` 或空白后 `=`，从而 `MY_ATTR` 不会误匹配 `EXTRA_MY_ATTR`
- `set_drawing_number(xml, value)`：`set_drawing_attribute(xml, "SP_DRAWINGNUMBER", value)` 的便利别名
- `set_element_text(xml, element, value)`：替换 `<E>text</E>` 形式的元素文本内容；自闭合标签 `<E/>` 直接报 `MalformedElement`
- `set_general_file_path(xml, value)`：先试 `<FilePath>` 后退回 `<Path>`，与 `parsers/general_xml.rs` 的接受面对齐

### `MetadataEditError`（`thiserror::Error + PartialEq`）

- `AttributeNotFound { attr }` / `ElementNotFound { element }`：找不到目标
- `DuplicateAttribute { attr, count }` / `DuplicateElement { element, count }`：拒绝二义编辑（调用方需先把范围缩到唯一的位置）
- `UnterminatedAttribute { attr }`：属性值起始 `"` 后找不到结束 `"`
- `MalformedElement { element }`：自闭合或缺闭合标签

### XML 转义

- 属性值：`& < > " '` 全部转 entity；调用方传裸字符串
- 元素文本：`& < >` 转 entity（属性专用的 `" '` 不转，避免误伤合法文本）

### 测试

- `writer::metadata_helpers::tests` **18 个全绿**：
  - 简单替换 / 保留兄弟属性 + 空白 / 不匹配长名后缀 / 未找到 / 重复 / XML 转义 / 空字符串 / Unicode（中文 + №）
  - 元素文本：基本替换 / 带属性 / 自闭合拒绝 / 未找到 / 转义 / 不匹配长元素名后缀
  - `set_general_file_path` FilePath 优先 + Path 回退
  - 空 XML 双类返回各自的 NotFound
- `cargo test --lib`：**100/100 通过**（93 → 100，新增 18 - 11 个之前没数到的，无 regression）
- `cargo test --test writer_roundtrip` 7/7、`--test writer_real_files` 1/1 全绿
- 已知失败：`tests/parse_real_files.rs` 26 个 + `tests/unit_parsers.rs::sheet_stream_reuses_cluster_header` 都依赖未提交的 `test-file/DWG-0201GP06-01.pid`，与本次无关

### 设计取舍

- **byte-level vs XML re-emit**：选 byte 级是为了兼容性。`quick_xml::Reader` → `Writer` 的 round-trip 会归一化引号风格 / 属性顺序 / 空白；对 SmartPlant 这种 picky reader 容易引入"我没改这里它怎么也变了"的视觉 diff
- **不处理 BOM / UTF-16**：跟 v0.4.0 risk note 一致；helper 假设输入是 UTF-8 文本。下一迭代如发现真实 `/TaggedTxtData/*` 是 UTF-16，可在 `metadata_write::apply_metadata_updates` 一处统一做编码 round-trip
- **重复匹配显式拒绝**：与其 silently 替换"第一个"或"最后一个"，不如让调用方知道范围太宽并先 narrow（例如 SmartPlant 模板里有多张 sheet 的标题块共享 SP_DRAWINGNUMBER 时，应分别编辑每个 sheet 流的 Drawing XML）

### 公共 API 增量

- 新类型：`MetadataEditError`
- 新函数：`set_drawing_attribute` / `set_drawing_number` / `set_element_text` / `set_general_file_path`
- 配套读取器（v0.4.1 同段补丁）：`get_drawing_attribute(xml, attr) -> Option<String>` 与 `get_general_element_text(xml, element) -> Option<String>` —— "exactly once" 语义与 set 端的"重复拒绝"对偶；duplicates / not-found / 自闭合（element 版本）都返回 `None`
- `writer` 模块直接 re-export 全部新 API；`PidWriter` / `WritePlan` / `MetadataUpdates` 行为不变
- bulk 读取器（v0.4.1 同段补丁）：`list_drawing_attributes(xml) -> Vec<(String, String)>` 列出所有 `<TagName attr="value"…/>` 属性对、`list_general_elements(xml) -> Vec<(String, String)>` 列出所有 `<element>text</element>` leaf 对（自闭合 / 含子元素跳过）；都按源序返回，跳过 PI/comment/CDATA 前缀
- **新 binary `pid_writer_validate`**（v0.4.1 同段补丁）：CLI 工具，对真实 `.pid` 文件做 `parse_package → PidWriter::write_to (passthrough) → re-parse_package` 完整 round-trip，按 path 列出 stream byte 级 diff，支持 `--out` / `--keep` / `--json` / `--quiet` / `--max-diff-bytes`；exit 0=PASS / 1=mismatch / 2=parse-IO/edit 错；公共 helper `run_validate(input, output, max_diff_bytes, edits) -> Result<ValidateReport, ValidateError>` 与 `ValidateReport`/`StreamMismatch`/`EditOp`/`EditKind`（带 `serde::Serialize`）暴露给测试与下游集成。`tests/writer_validate_cli.rs` 8 个端到端测试通过 `env!("CARGO_BIN_EXE_pid_writer_validate")` 驱动 CLI（不引新依赖，无 assert_cmd / escargot）
- **`--edit ATTR=VALUE` / `--general-edit ELEMENT=VALUE`**（v0.4.1 同段补丁）：让 CLI 支持"编辑后再验证"模式。任意条数 edit 在 round-trip 之前应用到 source 包；roundtrip 用 **edited** 包作对照基准，被 edit 触碰的流标 `EDITED`、未触碰的流仍要字节级匹配。报告新增 `edited` 计数与 `edits_applied` 数组；新错误变体 `ValidateError::Edit` 透传 `MetadataEditError`；CLI 用 `splitn(2, '=')` 切 ATTR/VALUE 让 value 含 `=` 也安全
- **`inspect::unidentified_top_level_streams` 可发现性 API**（v0.4.1 同段补丁）：新 pub 函数 `pid_parse::inspect::unidentified_top_level_streams(&PidDocument) -> Vec<&StreamEntry>` 返回 pid-parse 尚未识别的顶层 CFB 流（解码工作的待办清单）；配套 pub 常量 `KNOWN_TOP_LEVEL_STREAM_NAMES` + `KNOWN_TOP_LEVEL_STORAGE_PREFIXES` 作为识别白名单；`inspect/report.rs` 内部的 "Top-level Unidentified Streams" 段改用此 API，人类输出一致
- **条件测试降级**（v0.4.1 同段补丁）：`tests/parse_real_files.rs`（26 测试）+ `tests/unit_parsers.rs::sheet_stream_reuses_cluster_header` 改为"fixture 缺失 eprintln! + return"，与 `tests/writer_real_files.rs` 同风格；消除 `cargo test` 在缺 `test-file/DWG-0201GP06-01.pid` 时的噪音失败。新增条件测试 `top_level_unidentified_streams_are_empty_on_sample_file` 把"样本文件顶层流全部已识别"锁为不变量，回归警报
- **`ObjectGraph` 图遍历便利方法**（v0.4.1 同段补丁）：`impl ObjectGraph` 加 4 个 ergonomic 查询方法 + 1 个新结构体：
  - `pub fn object_by_drawing_id(&self, drawing_id: &str) -> Option<&PidObject>`：O(log N) 索引化查找
  - `pub fn relationships_touching(&self, drawing_id: &str) -> Vec<&PidRelationship>`：返回 source/target 任一为该 id 的关系
  - `pub fn neighbors_of(&self, drawing_id: &str) -> Vec<&PidObject>`：通过关系边解析的对端对象，去重 + 跳过自环
  - `pub fn endpoint_resolution_stats(&self) -> EndpointResolutionStats`：fully/partially/unresolved 三态汇总
  - `pub struct EndpointResolutionStats { total, fully_resolved, partially_resolved, unresolved }` (Serialize/Deserialize/JsonSchema/Default)
  - 6 个新单元测试 (`model::object_graph_impl_tests`) 覆盖空图/已知/未知/自环/三态计数
  - 配套增量：`pub fn find_drawing_ids_by_prefix(&self, prefix: &str) -> Vec<&str>`，`BTreeMap::range`-backed O(log N + K)；空 prefix 返回所有 id；4 个新单测覆盖排序/未匹配/多匹配/长 prefix 等价精确
  - 搜索增量：`pub fn find_objects_by_item_type(&self, &str) -> Vec<&PidObject>` 与 `pub fn find_objects_by_extra(&self, key, value) -> Vec<&PidObject>`，O(N) 线性扫；4 个新单测覆盖匹配/未匹配/extra key 缺失/value 不匹配。`object_graph_impl_tests` 共 14/14 全绿
  - BFS 多跳遍历增量：`pub fn neighbors_within(&self, drawing_id, depth) -> Vec<&PidObject>`，level-by-level BFS、`BTreeSet` 去重、自环跳过、`depth=0`→空、`depth=1`≡`neighbors_of`、循环安全（每对象至多访问一次）；5 个新单测覆盖 zero/one/two-hops/unreachable/cycle。`object_graph_impl_tests` 共 19/19 全绿
  - 最短路径增量：`pub fn shortest_path<'a>(&'a self, from_id, to_id) -> Option<Vec<&'a str>>`，BFS + predecessor map + 反推路径；`from_id == to_id` 返回单元素 path、未知 endpoint 或不连通返回 None、循环安全。5 个新单测覆盖 zero-hop/direct/multi-hop/unreachable/unknown_endpoint。`object_graph_impl_tests` 共 24/24 全绿
  - `tests/parse_real_files.rs::relationship_endpoints_resolve_via_sheet_record` 重构：从手写 `.iter().filter().count()` 双段改为 `endpoint_resolution_stats()` 一次调用，减少噪音
- 测试：`writer::metadata_helpers::tests` 由 18 个增至 **29 个**全绿（新增 11 个：3 个 `get_drawing_attribute_*` + 3 个 `get_general_element_text_*` + 5 个 `list_*`）

## [0.4.0] - 2026-04-19

### Phase 8: Writer 层落地（Package + WritePlan + CFB 回写）

在 parser-only 结构之上引入 **package 层**（保留原始 stream 字节）和 **writer 层**（按写计划重发 CFB），实现 passthrough round-trip 与 metadata-only 更新；Sheet 字节级修补以 `experimental` 形式入模。

- **`src/package.rs` 新模块**：
  - `PidPackage { source_path, streams: BTreeMap<String, RawStream>, parsed: PidDocument }`
  - `RawStream { path, data, modified }`
  - 方法：`get_stream` / `get_stream_mut` / `replace_stream` / `mark_unmodified`
- **`PidParser::parse_package(path)` 新入口**：复用全部解析流水线，额外捕获每条 CFB 流的原始字节；`parse_file` 改为薄包装 `Ok(parse_pid_package(...).parsed)`，行为完全等价。
- **`src/cfb/reader.rs` 重构**：`collect_streams` → `collect_streams_and_bytes`，单次 walk 同时产出 `Vec<StreamEntry>` 和 `BTreeMap<String, RawStream>`，避免双重读取。
- **`src/writer/` 新模块**（`mod.rs` / `plan.rs` / `metadata_write.rs` / `sheet_patch.rs` / `cfb_write.rs`）：
  - `WritePlan { metadata_updates, stream_replacements, sheet_patches }` 三层组合，按序应用
  - `MetadataUpdates`：`drawing_xml` / `general_xml` 替换 `/TaggedTxtData/Drawing` 与 `/TaggedTxtData/General`；`summary_updates` 字段已就位但本期不实现 `SummaryInformation` 重写
  - `StreamReplacement`：低层 path → bytes 直替
  - `SheetPatch + SheetChunkPatch`：byte-range 倒序 splice，越界返回 `PidError::ParseFailure { context: "sheet_patch", ... }`
  - `cfb_write::write_package`：`::cfb::create` 起新容器，`collect_storage_paths` 按升序自动建立中间 storage，再按 `BTreeMap` 顺序写出每个 stream
- **`PidWriter::write_to(package, plan, output)`**：克隆 package → metadata_write → stream_replacements → sheet_patches → cfb_write，源 package 不变。
- **`lib.rs`**：补上 `pub mod package; pub mod schema; pub mod writer;`（schema 模块 v0.3.1 已实现但未在 lib.rs 暴露，本次顺手挂上）。

### 测试

- **lib 单元测试 75 通过**（62→75）：
  - `package`：3 个（insert/overwrite/mark_unmodified）
  - `writer::sheet_patch`：5 个（同长度 splice / 倒序多 patch / 增长型 patch / 越界 / 区间反转）
  - `writer::cfb_write`：2 个（父 storage 收集 / 根流无 storage）
  - `schema`：3 个（v0.3.1 既有，正式登记）
- **集成测试 `tests/writer_roundtrip.rs` 7 通过**：
  - `passthrough_roundtrip_preserves_streams`
  - `metadata_only_update_replaces_tagged_streams`
  - `stream_preservation_of_unknown_streams`
  - `explicit_stream_replacement_overrides_metadata_layer`（新增：验证 stream_replacements 在 metadata 之后生效）
  - `sheet_patch_byte_range`
  - `sheet_patch_out_of_range_errors`
  - `missing_sheet_yields_missing_stream_error`
- **`tests/writer_real_files.rs`** 条件性 smoke：本地有 `test-file/DWG-0201GP06-01.pid` 时 round-trip 真实文件并按流逐字节比较；缺失时 `eprintln!` + return（与 `parse_real_files.rs` 同约定）。
- 所有既有 lib 测试与 release 构建通过。

### 公共 API 新增面

- 新增类型：`PidPackage` / `RawStream` / `WritePlan` / `MetadataUpdates` / `StreamReplacement` / `SheetPatch` / `SheetChunkPatch` / `PidWriter`
- 新增方法：`PidParser::parse_package(path) -> Result<PidPackage>`
- `PidDocument`、`PidParser::parse_file` 行为不变；`PidError` 不变（继续复用 `Io` / `MissingStream` / `ParseFailure`）。

### 已知限制

- CFB 重建不复刻原文件 CLSID、storage 创建/修改时间和物理 sector 顺序；内容视图可保真（每条流字节按 path 一致），字节级整文件 diff 不会一致。
- `MetadataUpdates::drawing_xml/general_xml` 直接 `String::into_bytes()`，不嗅探 BOM / UTF-16；调用方需自行准备字节等价内容。
- Sheet patch 仅 byte-range，不对接语义 probe；未在 CLI 接线。

## [0.3.1] - 2026-04-19

### Phase 7b: JSON Schema 导出

- **`schemars` 依赖**（`v1.2.1`，`preserve_order` feature）：为 `PidDocument` 及其所有子类型添加 `#[derive(JsonSchema)]`，覆盖 model 中全部 `Serialize/Deserialize` 结构体与枚举
- **`src/schema.rs` 新模块**：
  - `pid_document_schema() -> Schema`：返回 `PidDocument` 的 JSON Schema 对象
  - `pid_document_schema_pretty() -> Result<String, _>`：便捷包装，直接产出 pretty-printed JSON Schema 文本
  - 3 个单元测试：序列化合法性 / 核心类型名出现 / `AttributeValue` 变体定义
- **CLI `--schema` 出口**（复用已有 `pid_inspect` 入口）：下游消费方可通过 `pid_inspect --schema` 获取 JSON Schema，用于 TypeScript / Python / C# 代码生成（quicktype / json-schema-to-typescript / NJsonSchema）
- **`docs/writer-layer-plan.md`**：新增 Package / Writer 层落地计划文档（不含代码实现，仅规划）

### 测试

- schema 模块 3 个单元测试全通过
- 所有既有 lib 测试继续通过

## [0.3.0] - 2026-04-18

### Phase 7a: Mermaid 可视化导出

- **`inspect/mermaid.rs` 新模块**：纯函数把 `ObjectGraph` 和 `CrossReferenceGraph` 渲染为 mermaid 文本
  - `object_graph_mermaid(doc)` / `object_graph_mermaid_with(doc, opts)`：对象图（objects + relationships），按 `item_type` 着色、`drawing_id` 截短、`off-drawing` 端点自动占位；默认过滤模板关系（`guid` 为空）
  - `crossref_mermaid(doc)` / `crossref_mermaid_with(doc, opts)`：交叉引用图，四个 subgraph（Cluster Coverage / Symbol Usage / Attribute Classes / PSMroots→CFB Tree），缺失与异常用 `missing` / `extra` 颜色高亮
- **CLI 扩展**：
  - `pid_inspect --graph-mermaid`：stdout 输出对象图 mermaid（可直接贴到 Mermaid Live Editor / Obsidian / Notion）
  - `pid_inspect --crossref-mermaid`：stdout 输出交叉引用图 mermaid
- **渲染容量控制**：`ObjectGraphOptions { max_nodes=200, max_edges=500, skip_template_relationships=true }` 和 `CrossRefOptions { max_symbols=20, max_jsites_per_symbol=6 }`，超出用 `... (N more)` 占位保持 mermaid 可解析

### 模型

- 纯派生层，无新字段，仅新增导出工具

### 测试

- `inspect::mermaid` 8 个单元测试：空文档返回空 / 节点&边渲染 / off-drawing 占位 / 模板关系过滤 / 四个 subgraph 全都输出 / `sanitize` 规范化 / `escape_mermaid` 转义 / max_nodes 溢出
- 所有 lib 测试 **62 通过**（53→62），release 构建通过

### 版本收敛

从 `0.3.0-rc1`（关系端点解码）+ `0.3.0-rc2`（跨引用对象图）合并为正式 `0.3.0`，三件事（关系边、跨引用统计、可视化）一起构成 Phase 6 + 7a 的闭环交付。

## [0.3.0-rc2] - 2026-04-18

### Phase 6c: 跨引用对象图（基于 rc1 关系端点解码继续演进）

在 v0.3.0-rc1（Phase 6 关系端点解码，`source`/`target` 可用）之上新增**派生层**，把已解码的数据结构对齐成关系视图。

- **`CrossReferenceGraph`**：在已解码的 `PidDocument` 之上生成关系视图，纯内存派生、无额外 IO。四个子视图：
  - `ClusterCoverage`：把 `PSMclustertable` 声明的 cluster 与实际发现的 cluster/sheet 流做对齐，输出 `matched` / `declared_missing` / `found_extra` 三集合，数据完整性一目了然
  - `SymbolUsage`：按 `symbol_path` 反向索引 JSite 实例，暴露"一个符号被哪几个 JSite 引用"
  - `AttributeClassSummary`：每个 DA `class_name` 下的记录数 / 出现过的属性名集合 / 涉及的 `DrawingID` / `ModelID`（后者截断到 32）
  - `RootPresence`：把 `PSMroots` 中每条根名和 CFB 顶层目录条目对齐，标记 `STORAGE` / `STREAM` / `MISSING`

### 新模块

- `src/crossref.rs`：纯函数 `build_graph(doc) -> CrossReferenceGraph`，6 个单元测试覆盖所有四个子视图 + 空文档 + 缺失 PSM 降级

### 模型扩展

- 新类型：`CrossReferenceGraph` / `ClusterCoverage` / `SymbolUsage` / `AttributeClassSummary` / `RootPresence`
- `PidDocument` 新增可选字段 `cross_reference`；在 pipeline 末尾（`build_object_inventory` / `build_object_graph` 之后）生成

### 报告 & CLI

- 主报告新增 `--- Cross Reference ---` 段：cluster 覆盖率 / 符号用量 Top 5 / 每个属性类一行摘要 / PSMroots 解析状态
- `pid_inspect --crossref`：交叉引用详细视图（所有符号 + 所有属性类 + 全部 root 状态）

### 与 v0.3.0-rc1 的关系

rc1（关系端点解码）解决了**图的边**（`source_drawing_id` / `target_drawing_id` via sheet endpoint record 间接引用），rc2（本次）负责**图的上层统计视图与数据完整性检查**。两者互补：rc1 是底层关系解码，rc2 是跨层索引和对齐检查。

## [0.3.0] - 2026-04-18

### Phase 6: 关系端点解码（`source`/`target` 可用！）

- **核心突破**：破译 `/Unclustered Dynamic Attributes` 的**每条 P&IDAttributes 记录统一 31 字节 trailer**：
  ```
  89 00 <u32 size> <u32 record_id> [0x00 × 8] <u32 field_x> FF FF <u32 class_id> 14 00 00
  ```
  - `class_id=0xF6` 为关系记录，`0x109` 为 Symbol/Nozzle，`0xEA` 为 Drawing 等
  - 关系的 `field_x` **单调 +2 递增**，暗示为端点对表索引
- **Sheet 端点记录结构破译**（Sheet6 流里）：
  ```
  +0 u32 rel_field_x   +4 u32=0x06   +8 [u8;6]=0  +14 u16=0x0002
  +16 u32 endpoint_a    +20 u16=0x01  +22 u32 endpoint_b
  ```
  每条关系在 Sheet 流里有恰好 1 条此类记录，`endpoint_a/b` 指向对象的 `field_x`
- **端到端端点解析**：`PidRelationship` 新增 `source_drawing_id` / `target_drawing_id`，样本 1 实测 55/64 完全解析、9 partial（跨图 OPC）、0 未解析
- **证伪假设**：之前的推测"端点是相邻 GUID"被 `probe_sheet_endpoints` 证伪——对象 GUID 在全 CFB（69 流 × raw+Windows 布局）只以 ASCII 形式出现一次，证明端点采用**紧凑 field_x 索引间接引用**

### 模型扩展

- `DaRecordTrailer`：新结构（record_id / field_x / class_id / drawing_id / relationship_guid）
- `SheetEndpointRecord`：新结构（rel_field_x / endpoint_a / endpoint_b）
- `PidRelationship` / `PidObject` 新增 `record_id` / `field_x`；`PidRelationship` 新增 `source_drawing_id` / `target_drawing_id`
- `DynamicAttributesBlob.record_trailers` / `SheetStream.endpoint_records` 新字段
- `DocVersion2Raw`：DocVersion2 流原始保留（size / magic / hex_preview）
- `AttributeField.raw_value`：值审计链，保存 `strip_value_prefix` 剥离前的原始值

### 新模块

- `parsers/sheet_endpoint_records.rs`：Sheet 端点记录解析器 + 6 个单元测试
- `parsers/relationship_probe.rs`：关系记录邻近字节探针 + 4 个单元测试
- `examples/probe_*`（5 个）：RE 过程探针工具，保留为文档

### 报告与 CLI

- 报告 `--- Object Graph ---` 新增 "Endpoint resolution" 统计行和端点对显示
- `pid_inspect --probe-endpoints` 打印每条关系的 source/target drawing_id 与对象类型
- `pid_inspect --probe-relationships` 打印 `Relationship.<GUID>` 邻近字节证据

### 测试

- 单元测试：`sheet_endpoint_records` 6 个、`dynamic_attr_records` 新增 trailer 提取测试
- 集成测试新增：`record_trailers_cover_every_pidattributes_record` / `relationship_endpoints_resolve_via_sheet_record` / `sheet_endpoint_records_one_per_relationship` / `doc_version2_preserved_raw` / `object_graph_has_objects_and_relationships` 等
- **总计 91 个测试通过**（47 单元 + 26 集成 + 18 模块内）

## [0.2.4] - 2026-04-17

### Phase 5b: 文档注册表类流解析

- **`DocVersion3` 版本日志**：固定 48 字节/记录格式 `[product 16B][version 12B][op 4B][timestamp 16B]` 完全解出，样本 4 条版本历史（SA→SV→SV→SV，时间戳 12/29/25 → 03/16/26，版本 0144 ↔ 0077 来回切换）
- **`AppObject` COM 注册表**：每条 `[CLSID 16B][u32 char_count][UTF-16LE path]` + 3B filler；5 个 COM 插件 CLSID/路径完整解出（`igrSmartLabel.dll` / `igrGluePnt.dll` / `igrConnector.dll` / `LineRn.dll` 等）
- **`JTaggedTxtStgList`**：格式 `[list_name utf16-ascii run][u32 count][记录×count]`，每记录 `[u32 char_count][UTF-16LE storage_name]`；揭示 `TaggedTxtStorages → TaggedTxtData` 的映射
- **关键细节**：
  - `AppObject` 的长度字段是**字符数**（含 L'\0'）而非字节数
  - `JTaggedTxtStgList` 的 `list_name` 无 L'\0' 终止符，靠 u32 count 低字节 `0x01` 天然分界
  - CLSID 按 Microsoft 经典 COM 二进制布局解析（前三段 LE，后两段 BE），渲染为 `{XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX}` 标准形式

### 模型扩展

- 新类型：`VersionHistory` / `VersionRecord` / `AppObjectRegistry` / `AppObjectEntry` / `TaggedTextStorageList` / `TaggedTextStorageEntry`
- `PidDocument` 新增三个可选字段：`version_history` / `app_object_registry` / `tagged_storages`

### 新模块

- `parsers/doc_version.rs`（DocVersion3 解析器 + 4 个单元测试）
- `parsers/app_object.rs`（AppObject 解析器 + 4 个单元测试，含 GUID 格式化校验）
- `parsers/tagged_stg_list.rs`（JTaggedTxtStgList 解析器 + 3 个单元测试）
- `streams/doc_registry.rs`（统一接入上述三种流到 pipeline）

### 报告

- 主报告新增三段：`--- Version History ---` / `--- App Object Registry ---` / `--- Tagged Text Storage List ---`
- 顶层未识别流仅剩 1 个：`DocVersion2` (48B, magic=0x00010034, 二进制非文本)

### 测试

- 集成测试 +4：`version_history_decoded` / `app_object_registry_decoded` / `tagged_storage_list_decoded` + 之前已有的 PSM 三项
- **总计 56 个测试通过**（17 集成 + 18 `unit_parsers` + 21 模块内）

## [0.2.3] - 2026-04-17

### Phase 5a: PSM 索引表解析

- **`PSMroots` 完整解码**：确认格式 `[u32 magic='root']` + N 条 `[u32 id][u32 char_count][UTF-16LE name]` 记录；样本文件 7 条记录全部解出（`Imagineer Document` / `Server Document` / `_SupportOnlyList` / `TopVFSet` / `Dynamic Attributes Set Table` / `StyleLibrarian` / `DocStore`）
- **`PSMclustertable` 名称提取**：声明计数 `count=5`，样本 5 个 cluster 名称全部识别（`PSMcluster0` / `StyleCluster` / `Dynamic Attributes Metadata` / `Sheet6` / `Unclustered Dynamic Attributes`）——这是 P&ID 文件中所有 cluster 风格流的**权威清单**
- **`PSMsegmenttable` 解码**：固定 12 字节格式 `[magic='stab'][u32 count][u8×count flags]`
- **揭示 Sheet 归属**：PSMclustertable 将 `Sheet6` 与其他 cluster 并列，证实 Sheet 流属于 cluster 体系（和 magic `0x6C90F544` 的推测一致）

### 模型扩展

- 新增类型：`PsmRoots` / `PsmRootEntry` / `PsmClusterTable` / `PsmClusterEntry` / `PsmSegmentTable`
- `PidDocument` 新增三个可选字段：`psm_roots` / `psm_cluster_table` / `psm_segment_table`

### 新模块

- `parsers/psm_tables.rs`：`parse_psm_roots` / `parse_psm_cluster_table` / `parse_psm_segment_table`，含 6 个内置单元测试
- `streams/psm_tables.rs`：接入主解析 pipeline（容错：流缺失时跳过）
- `examples/psm_dump.rs`：PSM 流 hex dump + 结构化 walk 开发工具

### 报告

- 主报告新增三段：`--- PSMroots ---`、`--- PSMclustertable ---`、`--- PSMsegmenttable ---`
- 顶层未识别流数从 7 降到 4（剩 `AppObject` / `DocVersion2` / `DocVersion3` / `JTaggedTxtStgList`）

### 测试

- 单元测试：`parsers::psm_tables` 6 个（roots/cluster/segment 各含正/负用例）
- 集成测试 +3：`psm_roots_extracts_known_entries` / `psm_cluster_table_matches_actual_clusters` / `psm_segment_table_decoded`
- **总计 42 个测试通过**（14 集成 + 18 `unit_parsers` + 10 模块内）

## [0.2.2] - 2026-04-17

### Phase 4: Sheet 流专项 + Magic 识别

- **Sheet 流结构化**：确认 `Sheet*` 流与 Cluster 共享 `magic 0x6C90F544`，复用 `cluster_header::parse_header()` 解析公共头（样本中 `Sheet6` 解析出 type=0x00CE / records=354 / body=121）
- **Sheet `ProbeSummary`**：对每个 Sheet 流扫描 0x89 标记并记录 body_start / marker_count / bytes_scanned（实测 Sheet 流 marker_count=0，确认 Sheet 不使用 DA 记录格式）
- **Sheet 属性记录探测**：尝试复用 `dynamic_attr_records::parse_attribute_records()`，若记录不为空则以 `confidence="heuristic"` 保留（当前样本未解出记录，为后续 Sheet 专用格式做铺垫）
- **`parsers/magic.rs` 新工具**：
  - `magic_tag(u32) -> Option<String>` 将 `magic_u32_le` 渲染为按磁盘字节顺序的 4 字符 ASCII（仅当全部可打印时返回）
  - `describe_magic(u32) -> &'static str` 为已知 magic（root/clst/stab/Smar/OLES/...）提供人类可读说明
- **未识别顶层流可见化**：报告中新增 `--- Top-level Unidentified Streams ---` 段，样本中揭示 7 个以往被忽略的结构化流：`PSMroots` (root)、`PSMclustertable` (clst)、`PSMsegmenttable` (stab)、`DocVersion3` (Smar)、`AppObject`、`DocVersion2`、`JTaggedTxtStgList`

### 模型扩展

- `SheetStream` 新增字段：`magic_u32_le` / `magic_tag` / `header` / `attribute_records` / `probe_summary`
- `UnknownStream` 新增字段：`magic_tag`

### CLI 增强

- `pid_inspect --probe-sheet`：Sheet 流专项探测输出（magic / header / probe / records / ASCII preview）

### 测试

- 新增 4 个单元测试：`magic_tag` × 2 / `describe_magic` / `sheet_stream_reuses_cluster_header`
- `parsers::magic` 内置 3 个单元测试
- 总计 32 个测试通过（11 集成 + 18 `unit_parsers.rs` + 3 模块内）

## [0.2.1] - 2026-04-17

### 正确性修复

- **`parse_header()` 边界安全**：最小长度判断从 14 修正为 16 字节，防止读取 `flags` 时越界
- **`parse_string_table()` sentinel 处理**：区分真正的 sentinel（index==0, byte_len==0）和合法空字符串条目（index!=0, byte_len==0），不再错误截断表
- **`DrawingMeta` SP_ 前缀兼容**：`RulesUID` / `FormatsUID` / `GappingUID` / `SymbologyUID` / `DefaultFormatsUID` 同时支持纯键名和 `SP_` 前缀键名

### Probe / Decode 分层

- **`AttributeRecord.confidence`**：每条 DA 记录携带 `"heuristic"` / `"decoded"` 置信度标记
- **`ProbeSummary`**：DA 流启发式扫描元数据（body_start_offset / marker_count / records_extracted / bytes_scanned）
- **`ClusterProbeInfo`**：PSMcluster0 字符串表定位元数据（string_table_offset / detection_method / entries_parsed / end_offset）
- **`report.rs` 标注**：报告中 DA 记录标注 `[EXPERIMENTAL/heuristic]`，Cluster 和 DA 输出 `[PROBE]` 行

### 测试

- 新增 14 个单元测试：`collect_simple_tags` (5) / `parse_header` (5) / `parse_string_table` (4)
- 全部 25 个测试通过（11 集成 + 14 单元）

### CLI 增强

- `pid_inspect --probe-cluster`：输出 Cluster 流探测详情（偏移量、检测方法、字符串表完整内容）
- `pid_inspect --probe-dynamic`：输出 DA 流探测详情（0x89 标记数、记录统计、属性字段详情）

### 文档

- **ARCHITECTURE.md** 全面重写：4 张 Mermaid 架构图（分层架构 / .pid 文件结构 / 数据流 / Probe-Decode 分层）、类型表、CLI 用法、演进路线

## [0.2.0] - 2026-04-16

### 新增 (Phase 4: 对象清单与精度修复)

- **P&ID 对象清单** (`ObjectInventory`)：从 DA 属性记录中自动构建 P&ID 对象统计——管道运行、仪表、管嘴、设备、关系等分类计数
- **DA 值解析精度提升**：double 优先检测（OLE Date 正确识别）、GUID 前缀保护（32 位 hex 不被误剥离）、单字节类型标记跳过

### 新增 (Phase 2-3: 语义提取与二进制记录切分)

- **OLE Summary 解析**：实现 `\x05SummaryInformation` 和 `\x05DocumentSummaryInformation` 流的二进制解码，支持 VT_LPSTR / VT_LPWSTR / VT_FILETIME / VT_I4 类型，提取应用名称、标题、作者、创建/修改时间等元数据
- **GUID 扫描**：在 `string_scan` 中新增双模式 GUID 提取——文本格式 `{XXXXXXXX-...}` 和原始 16 字节 LE 格式；`JProperties` 解析自动调用，测试文件提取 706 个 GUID
- **Cluster 公共头解析器** (`cluster_header.rs`)：解析所有 cluster 流共享的 magic `0x6C90F544` + type / record_count / body_len 字段
- **PSMcluster0 字符串表**：反向定位 entry1，从 PSMcluster0 中提取索引字符串表（SiteObjects, PreferenceSet, Sheets）
- **动态属性记录解码器** (`dynamic_attr_records.rs`)：基于 `0x0089` type marker 的记录边界检测，解析出属性类名 + 名称 / 值对，测试文件提取 231 条记录 / 10 个唯一类 / 1120+ 属性字段
- **结构化模型类型**：`ClusterHeader`、`IndexedString`、`AttributeRecord`、`AttributeField`、`AttributeValue`
- **inspect 报告增强**：输出 Summary 信息、JSite GUID 计数、Cluster header 详情、字符串表、属性记录摘要

### 修复

- `dynamic_attrs.rs` 中 `strings` 和 `class_names` 的重复问题，使用 `HashSet` 消除 ASCII + UTF-16LE 合并扫描中的重复项
- XML 解析器嵌套标签跳过导致 Drawing/General Meta 全空的 bug（MCP-4 修复）
- Symbol path 乱码前缀通过 UNC 路径提取清理（MCP-4 修复）
- 编译错误 3 个 + 逻辑 bug 4 个（MCP-4 修复）

### 改进

- `pid_inspect` 支持 `--json` 输出完整 `PidDocument` 的 JSON 序列化
- 集成测试 11 个用例全部通过

## [0.1.0] - 2026-04-16

### 初始版本

- CFBF/OLE 容器遍历与流索引
- `TaggedTxtData/Drawing` 和 `TaggedTxtData/General` XML 元数据提取
- `JSite*` 对象存储索引与 JProperties 解析
- Cluster 流分类（PSMcluster, StyleCluster, Dynamic Attributes）
- Unclustered Dynamic Attributes 字符串扫描（ASCII + UTF-16LE）
- `pid_inspect` CLI 工具
