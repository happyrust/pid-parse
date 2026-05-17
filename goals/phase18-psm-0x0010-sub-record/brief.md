# Phase 18: PSM 0x0010 sub-record family — audit-only typed decoder

## 目标产出

把 PSM type code `0x0010` 这个跨 4 fixture **582 decoded** /
probe scan 638 hits、size 高度多形态、被 Phase 16 `JStyleOverride`
和 Phase 15 `GraphicGroup` tail 反复引用的 sub-record 家族，从
probe-only 提升为 **conservative typed audit-only decoder**，沿用
Phase 15 `GraphicGroup` 的「stable header + raw variable tail +
全 provenance」模式。

probe（非 advancing scan）输出 638 包含重叠 hit；本 phase 的
advancing-scan decoder（Phase 15 GraphicGroup 模板）跨 fixture
输出 **582**：DWG-0201=161, DWG-0202=104, 工艺管道-1=306, A01=11。

1. 暴露稳定 metadata header（`byte_range / type_code / type_flags /
   bytes_to_follow`），不命名 sub-kind payload 字段，直到 IDA 反编译
   能锁定。**注意：0x0010 沿用 Phase 15 GraphicGroup 的 6-byte
   PSM header（type_word + bytes_to_follow），不是 IGDS 标准的
   18-byte header（type_word + bytes_to_follow + oid + padding）。**
   因此 DTO **没有** `oid` 字段，与 Phase 14 typed primitives 不同。
2. payload 整体作为 `raw_payload: Vec<u8>` audit 字段暴露，长度 =
   `bytes_to_follow`；下游消费者可按 fixture-specific size bucket
   自行解读；后续 phase 再按 sub-kind 升级为 typed schema。
3. 跨 fixture decoded 数量 = **582**（advancing-scan baseline；
   probe 的 638 是非 advancing scan 含 overlap 的数字）。
4. 不引入新 `PidGraphicKind` variant（0x0010 是 sub-record，不
   独立呈现几何）。
5. 提供 `SheetGeometry::decoded_sub_records_0x0010` audit collection
   + parser-level cross-fixture sanity test，作为 Phase 19 / 后续
   reference-chain 解析的稳定基线。

完成后，下游消费者拿到的是：**638 条带完整 provenance 的 0x0010
audit record，每条都能被引用方（JStyleOverride.referenced_oid_a /
referenced_oid_c）反向定位**，但不假装解读了 payload 语义。

## 背景

`AGENTS.md` 已经记录 0x0010 (638 hits) 是 "embedded sub-records /
attribute fragments inside other record types, not a standalone
geometry type"。Phase 14 §6.3 把它列入 future work；Phase 15
GraphicGroup audit collection 已实现同款 audit-only 模板（352 条
cross-fixture，header + raw variable tail，无 PidGraphicKind 提升）。

Phase 16 `JStyleOverride` 反向工程的副产物（见
`docs/analysis/2026-05-15-garc2d-packed-int-tail.md` §11）
进一步证明 0x0010 是 JStyleOverride 复合 record 的 reference 目标：

- JStyleOverride `+38..41` (referenced_oid_a) 常指向 0x0010 sub-record
- JStyleOverride `+56..59` (referenced_oid_c) 常指向 0x0010 sub-record
- GraphicGroup tail 中也出现 0x0010 引用

Phase 18 即按 Phase 15 模板把 0x0010 落地为 audit-only collection，
为 Phase 19 reference-chain 真正的语义解析铺路。

probe 跑分结果（`cargo run --release --example
probe_psm_0x0010_shape`）：

| Fixture | Hits | Top size buckets |
|---|---:|---|
| DWG-0201 | 183 | 70 (53), 76 (24), 74 (11), 86 (11), 99 (9), 16 (8), 43 (7), 94 (7) |
| DWG-0202 | 133 | 13 (22), 43 (16), 45 (8), 18 (6), 25 (6), 17 (5), 21 (5) |
| 工艺管道-1 | 315 | 31 (182), 46 (23), 41 (16), 16 (14), 36 (11), 50 (11) |
| A01 | 7 | 28 (2), 15/16/24/30/34 各 1 |
| **总计** | **638** | size 高度多形态，无单一主导布局 |

样本 payload 显示存在多种 sub-kind：

- 13B、16B、21B：紧凑型，含小整型 + 少量 PSM type code 引用
- 28B / 31B：内含 IEEE 754 doubles（坐标对）+ PSM type code 后缀
- 70B / 76B / 86B / 99B：长 payload，含多个引用 + 可能 UTF-16 文本片段

这种多形态特征强烈暗示 0x0010 是带 sub-kind discriminator 的
polymorphic record，不适合在 Phase 18 强行命名 sub-kind 字段。

## 上下文（必读）

| 文档 / 文件 | 作用 |
|---|---|
| `AGENTS.md` Phase 14 §6.3 | 0x0010 = "embedded sub-records / attribute fragments" 的最初定性 |
| `docs/plans/2026-05-14-phase14-decoder-suite-final-summary.md` §6.3 | Phase 14 把 0x0010 留为 future 的原因 |
| `docs/plans/2026-05-14-phase15-graphic-group-final-summary.md` | Phase 15 GraphicGroup audit-only 模板，本 phase 沿用 |
| `docs/plans/2026-05-16-phase16-jstyleoverride-final-summary.md` Future Work | "Decode 0x0010 sub-record family (638 cross-fixture hits) — Phase 18" |
| `docs/analysis/2026-05-15-garc2d-packed-int-tail.md` §11 | JStyleOverride → 0x0010 reference chain 证据 |
| `examples/probe_psm_0x0010_shape.rs` | 现有 probe（本次跑出 638 hits + size 分布） |
| `src/parsers/sheet_records.rs::decode_graphic_groups` | Phase 15 audit-only decoder 模板，本 phase 仿照 |
| `src/model.rs::DecodedGraphicGroupRecord` | Phase 15 stable DTO，本 phase 仿照 |
| `src/streams/cluster.rs` | decoder → SheetGeometry 接入点（Phase 15 已示例） |
| `tests/parse_real_files.rs::graphic_group_decoder_ratchets_fixture_counts_and_header_fields` | Phase 15 cross-fixture ratchet test 模板 |
| `tests/parser_panic_safety.rs` | 新 parser entry 必须加入 adversarial matrix |

## 关键约束

- **审计优先**：本 phase 只暴露稳定 header + raw payload + provenance；
  不在 stable DTO 命名 sub-kind 字段（如 `coord_a_x` / `referenced_type`），
  避免重蹈 Phase 14 GArc2d 错误命名的覆辙。
- **不引入新 `PidGraphicKind` variant**：0x0010 是 sub-record，
  通过 `SheetGeometry::decoded_sub_records_0x0010` 集合暴露，不
  emit `PidGraphicEntity`。
- **不解析 reference chain**：本 phase 不实现 "JStyleOverride
  references 0x0010 by oid" 的解析；那是 Phase 19 / future 工作。
- **panic-safe**：所有 public parser entry 必须在
  `tests/parser_panic_safety.rs` adversarial matrix 覆盖；
  size 验证规则要在 `[1, 100_000]` 之类的保守范围（避开
  `bytes_to_follow = 0` 退化）。
- **不退化 Phase 14/15/16/17 任何 baseline**：
  - Phase 14: GLine2d=3, igLine2d=284, igLineString2d=119, igPoint2d=146,
    igTextBox=142, igSymbol2d=27
  - Phase 15: GraphicGroup audit=352
  - Phase 16: JStyleOverride=98, Annotation Decoded emission
  - Phase 17: PrimitiveArc surface fully removed
- 5 道 pre-commit gate 必须保持：build / test --workspace --all-targets /
  clippy -D warnings / fmt --check / missing-docs ratchet (baseline=0)。

## 非目标

- **不**反向 0x0010 的每个 sub-kind payload 字段。本 phase 只是 audit。
- **不**新增 `PidGraphicKind` variant。
- **不**实现 cross-record reference resolver（JStyleOverride.ref → 0x0010）。
- **不**反向 RAD `47FCC330..47FCC33E` siblings（Phase 19 工作）。
- **不**提取 plant instrument tag from raw_attribute_tail。
- **不**做 Sheet geometry 写回。
- **不**引入新 fixture。
- **不**提交 `dlls/`、`.i64`、私有 fixture。
- **不**修改 Phase 14/15/16/17 任何已落地 decoder 的输出。
- **不**commit / push，除非用户明确授权。

## Ask Before（要先问）

- 偏离 audit-only 模板，例如在 stable DTO 命名 sub-kind 字段、或
  emit `PidGraphicKind` entity。
- 新增 IDA instance 加载（如要确认 0x0010 真实类身份，需用户确认
  加载 `radsrvitem.dll` 或 sibling DLL 到 IDA MCP）。
- 任何 commit / push / 删除已存在测试前。
- 把 638 hit ratchet 数字向下调整（说明 decoder validation 太严格，
  需要审视是否过度保守）。
- 把 `SheetGeometry::decoded_sub_records_0x0010` audit collection 升级
  为 typed sub-kind 集合。

## Done Means（完成判据）

同时满足：

1. `src/parsers/sheet_records.rs` 新增 `SheetSubRecord0x0010Decoded`
   conservative DTO + `decode_sub_records_0x0010` /
   `decode_sub_record_0x0010_at` + 公开 `PSM_TYPE_CODE_SUB_RECORD_0X0010`
   常量；parser unit tests 覆盖 canonical + 每条 validation rejection
   + panic safety。
2. `src/model.rs` 新增 `DecodedSubRecord0x0010Record` stable DTO（含
   `byte_start / byte_end / type_code / type_flags / bytes_to_follow /
   raw_payload`，**无 `oid` 字段**——0x0010 是 6-byte header 形态）
   + `From` impl + `SheetGeometry::decoded_sub_records_0x0010` 字段
   + schema ratchet（新 needles）。
3. `src/streams/cluster.rs` 在 sheet pipeline 调用新 decoder 并填充集合。
4. `src/cfb/reader.rs` 同步 `SheetGeometry` 测试 fixture 初始化。
5. `tests/parse_real_files.rs` 新 cross-fixture ratchet test
   `sub_records_0x0010_decoder_emits_audit_records_with_provenance`，
   断言：跨 fixture 总数 = **582**（DWG-0201=161, DWG-0202=104,
   工艺管道-1=306, A01=11）；每条带稳定 stream_path / byte_range /
   type_code / non-empty raw_payload + raw_payload.len() ==
   bytes_to_follow（**无 oid 断言**）。
6. `tests/parser_panic_safety.rs` adversarial matrix 加入新 parser entry。
7. `CHANGELOG.md` 写明 Phase 18 audit-only collection 落地，包括 638 条
   baseline 和不引入 PidGraphicKind 的设计选择。
8. 5 道 pre-commit gate 通过，`missing_docs` baseline 不上升（=0）。
9. Phase 14/15/16/17 全部既有 baseline 保持。
10. `progress.jsonl` 对每个 AC 都有命令 / artifact / 输出摘要。

停止条件全部写入 `blockers.md`。
