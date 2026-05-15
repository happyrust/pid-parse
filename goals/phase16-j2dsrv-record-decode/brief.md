# Phase 16: PSM 0x0030 真实归属 — `j2dsrv.dll` `47FCC338` 复合 record 字段重定义

## 目标产出

把 Phase 14 落地的 `decode_primitive_arcs` / `SheetPrimitiveArcDecoded` 从
"基于 GArc2d 错误假设的 8-double payload decoder" 改写为 **基于真实类型
身份的 J2DSrv 复合 record decoder**：

1. 反编译 `j2dsrv.dll` 中 CLSID `{47FCC338-2D0F-11D0-A1FF-080036A1CF02}`
   的 `Save` / `Load` / `Validate` vtable slot，**拿到真实字段名 + 字段
   类型 + 字段顺序**。
2. 把 `SheetPrimitiveArcDecoded` 重命名 + 字段重定义为对应真实类的
   conservative typed DTO（命名建议见 plan.md §2）。
3. 重写 `decode_primitive_arcs` 验证规则：去掉错误的 `axis_a.y ≈ 0`
   过滤，正确解析 packed reference 链 + plant tag + companion coord +
   tail 中实际存在的字段。
4. 联动更新 `model.rs` / `geometry.rs` / `schema.rs` / 跨 fixture
   integration test 的 baseline。
5. 保留 audit-only 路径：未能锁定语义的 tail / packed 字段以 raw bytes
   形式暴露，不命名虚假语义。

完成后，下游消费者拿到的不再是 "假装是 GArc2d 但字段全错" 的 48 条
record，而是 **真实类名 + 真实字段语义 + 完整 provenance 的 98+ 条
J2DSrv 复合 record**。

## 背景

Phase 14 §6.1 把 GArc2d 字段语义修正列为 future-slice。Phase 16 触发
该 slice 后的三轮 probe + IDA 反编译收敛出**完全出乎预料**的事实：

- PSM type code `0x0030` 对应的 CLSID 是 `{47FCC338-2D0F-11D0-A1FF-
  080036A1CF02}`，**不是 IGDS `GArc2d`**（IGDS CLSID 后缀是
  `08003601B44A`，`47FCC338` 后缀是 `080036A1CF02`，属于
  `j2dsrv.dll` "RAD 2D Service" 注册的复合 record 家族）。
- 该家族（`47FCC330..47FCC33E`）共 13 个连续 IID，对应 PSM type code
  `0x29..0x35`，全部 a5=`0x40` / a6=`0x03`（与 IGDS 标准类 a5=`0xC0` /
  a6=`0x01` 模式截然不同）。
- 当前 decoder 用 8-double 假设跨 fixture 输出 48 条 "decoded arc"，但
  真实 0x0030 hit 总数是 **98 条**，丢失 ≈ 51% 的真实 record。
- 64 字节 payload 内部 +0..15（center.xy）位置碰巧对，+16..23 在 39%
  的 record 上字节级 ≡ center.x；+24..31 取值集中在 {0, π/2, 3π/2,
  2π}（看起来像 rotation，但也可能是 sweep_extent）；+32..63 实际是
  **packed PSM-style reference**（含 igLine2d / igTextBox / igSymbol2d
  / GraphicGroup / 0x0010 sub-record 等被引用类型的 PSM type code）。
- attribute tail 含 **length-prefixed UTF-16LE plant instrument tag**
  （如 DWG-0202 oid=1 的 `"A3-FA060201"`）、companion coords、`1.0`
  常量 marker 和更多 PSM reference 链。

证据链落档：

- `docs/analysis/2026-05-15-garc2d-packed-int-tail.md` §1-§10
- `examples/probe_garc2d_packed_bytes.rs`（3 轮迭代后的最终版）

## 上下文（必读）

| 文档 / 文件 | 作用 |
|---|---|
| `docs/analysis/2026-05-15-garc2d-packed-int-tail.md` | 本阶段触发证据；§10 列出 IDA 反编译定位过程与 a5/a6 分类 |
| `docs/analysis/2026-05-14-radsrvitem-psm-serialize-bytes.md` | Phase 14 IDA 反编译记录；GArc2d 错误假设的来源 |
| `docs/plans/2026-05-14-phase14-decoder-suite-final-summary.md` | Phase 14 final summary §6.1（本阶段触发点） |
| `examples/probe_garc2d_packed_bytes.rs` | 跨 4 fixture 的 0x0030 字节扫描 probe（含 packed-int / tail / referenced_type 桶分析） |
| `src/parsers/sheet_records.rs::decode_primitive_arcs` | 待修正的 decoder（第 2180-2440 行附近） |
| `src/model.rs::DecodedArcRecord` | 待重命名的 stable DTO |
| `src/streams/cluster.rs` | decoder → SheetGeometry 接入点 |
| `src/geometry.rs::build_normalized_geometry` | `PidGraphicEntity` emission with `radius = axis_a_magnitude()` 错误映射 |
| `tests/parse_real_files.rs::primitive_arc_decoder_emits_decoded_arcs_with_provenance` | cross-fixture baseline 待 ratchet 更新 |
| `tests/parser_panic_safety.rs` | 新 decoder entry 必须加入 adversarial matrix |
| `dlls/j2dsrv.dll` | 真实类实现所在（需新加载到 IDA） |
| `dlls/radsrvitem.dll` | PSM type table 所在；GUID lookup 已确认 |

## 关键约束

- **不允许**在没拿到 IDA 反编译证据前就给字段命名（rotation / sweep_extent
  / axis_a / axis_b / chord_length / arc_length 等都是猜测）。
- **不允许**保留 `decode_primitive_arcs` 这个错误名（Phase 14 用 "Arc" /
  "Primitive" 表达了对几何类型的承诺，但实际不是 arc）。重命名要在同
  一 PR 内完成。
- 任何已被 IDA / probe 双重证实的字段才能进 stable DTO；其余 tail 字节
  保留为 raw payload。
- 当 IDA 反编译显示真实字段类型与现有 f64 假设冲突时，**字节位置以
  IDA 为准**，不能为了保持兼容性掩盖 bug。
- 必须保留 `byte_range / oid / type_code / type_flags / bytes_to_follow`
  五个 metadata 字段（这些已被 Phase 14 验证）。
- 5 道 pre-commit gate 必须保持：build / test / clippy -D warnings /
  fmt --check / missing_docs ratchet（baseline 不上升）。
- Phase 14 其他 decoder（igLine2d / igLineString2d / igPoint2d /
  igTextBox / igSymbol2d / GLine2d）的 cross-fixture 计数 baseline
  不能退化。Phase 15 `SheetGeometry::decoded_graphic_groups` audit
  collection 也不能退化。

## 非目标

- 不实现 J2DSrv 其他 12 个 type code (0x29..0x2F + 0x31..0x35)。本阶段
  只锁定 0x0030 一个，其它在后续 phase。
- 不解析 `0x0010` sub-record family（即便 0x0030 reference 链里指向它）。
- 不做 Sheet geometry 编辑 / 写回。
- 不把 J2DSrv record 投射为 `PidGraphicKind::Arc`，除非 IDA 确认它真的
  描述弧线。可能要新增 `PidGraphicKind::Annotation` / `Instrument` /
  `J2DComposite` 之类的 variant，但**必须先与用户确认**。
- 不引入新 fixture（继续用 4 个 registry fixture）。
- 不把 `dlls/` 任何二进制 commit 进 git。

## Ask Before（要先问）

- 加载新 IDA instance（`j2dsrv.dll`，port ≥ 13347）前先确认。
- `SheetPrimitiveArcDecoded` 重命名为具体什么名（取决于 IDA 拿到的真实
  类名）。
- 是否新增 `PidGraphicKind` variant 来承载 J2DSrv 复合 record（影响
  public schema 与下游契约）。
- 是否把 0x0030 record 输出降级为 `PidGeometryConfidence::ProbeOnly`
  或 audit-only，作为重写期间的过渡（避免 stale Decoded 字段污染）。
- 任何 commit / push / 改写已有 Phase 14 / Phase 15 文档前。
- 是否在 reference 链中暴露 referenced `oid` 与 referenced PSM type
  code 进 stable DTO（vs audit-only）。

## Done Means（完成判据）

同时满足：

1. `j2dsrv.dll` 已加载到 IDA MCP，CLSID `47FCC338` 的真实类名 + Save /
   Load / Validate vtable slot 已反编译，字段表已写入新 analysis 文档
   `docs/analysis/2026-05-1?-j2dsrv-47FCC338-fields.md`。
2. `SheetPrimitiveArcDecoded` 已重命名为反映真实身份的 DTO（最终命名见
   plan.md §2），字段语义全部有 IDA / probe 双重证据。
3. `decode_primitive_arcs` 已重命名 + 验证规则重写：去掉 `axis_a.y ≈ 0`
   过滤，cross-fixture decoded 数量 ≥ 90（接近 probe 上限 98）。
4. `tests/parse_real_files.rs` 更新 baseline；Phase 14 其他 decoder
   计数 baseline 与 Phase 15 audit collection 计数全部保持。
5. `geometry.rs::build_normalized_geometry` 的 emission 路径不再把
   0x0030 当 Arc/Circle 处理；可选新增 `PidGraphicKind` variant 经用户
   确认后落地。
6. 5 道 gate 通过；`missing_docs` baseline 不上升。
7. `progress.jsonl` 对每个 AC 都有 IDA / probe / test command + artifact。
8. 完成后 commit 必须包含：`docs/analysis/2026-05-1?-j2dsrv-47FCC338-fields.md`、
   `examples/probe_garc2d_packed_bytes.rs`（原始证据，名字保留以追溯
   过程）、`docs/analysis/2026-05-15-garc2d-packed-int-tail.md`（触发
   证据）、`src/` 改动、`tests/` 改动、`CHANGELOG.md` slice 条目。

停止条件全部写入 `blockers.md`。
