# Phase 15: PSM 0x00FA GraphicGroup Records

## 目标产出

把 Phase 14 发现但未完整解码的 PSM `0x00FA`
GraphicGroup / GraphicPersist records 从 probe-only 提升为保守 typed
decoder，并建立 geometry OID 与 group / parent / child OID list 的可审计
provenance 关系。

完成后，`Sheet*` 流不只说明“有哪些 decoded geometry primitives”，还要能
解释一部分 geometry primitive 属于哪个 graphic group，以及 group record
在原始字节中的来源。

## 背景

Phase 14 已完成 8 个 PSM type family 的 Sheet geometry decoder，跨 fixture
输出 769 个 `PidGeometryConfidence::Decoded` entity，并通过
`pid_inspect --geometry-summary` 暴露给用户。

Phase 14 Slice Q 进一步确认：

- geometry records 是密集打包的，没有 inline attribute tail
- PSM `0x00FA` 的 Phase 14 legacy discovery count 是 250；Phase 15
  broad probe 可复现为 353，保守 decoder ratchet 为 352
- `0x00FA` 记录疑似 GraphicGroup / GraphicPersist
- payload 形状含 `oid`、`parent_ref`、sub-type / version，以及 variable OID
  references list
- PSM `0x0010` 有 638 hits，但更像嵌入式 sub-record fragment，不适合单独优先
  解码

## 上下文（必读）

| 文档 / 文件 | 作用 |
|---|---|
| `docs/plans/2026-05-14-phase14-decoder-suite-final-summary.md` | Phase 14 完成态与 Future Slices |
| `examples/probe_psm_0x00fa_shape.rs` | `0x00FA` 现有 cross-fixture byte dump 探针 |
| `examples/probe_igline2d_attribute_tail.rs` | 证明 geometry records 后没有 inline attribute tail |
| `src/parsers/sheet_records.rs` | Phase 14 decoder API 与七层模板 |
| `src/streams/cluster.rs` | Sheet raw stream -> `SheetGeometry` DTO 的 decoder 集成点 |
| `src/model.rs` | decoded record DTO 与 schema-facing model |
| `src/geometry.rs` | `PidGraphicEntity` provenance emission |
| `tests/parser_panic_safety.rs` | 新 byte-level decoder 必须加入 adversarial matrix |
| `tests/parse_real_files.rs` | cross-fixture integration guard |

## 关键约束

- `0x00FA` decoder 必须保守：结构不确定的 tail 保留 raw，不猜字段语义。
- 不把 `0x0010` 当独立 geometry 或 group record 解码；只允许作为后续线索记录。
- 不改变 Phase 14 geometry primitive decoder 的输出数量和 confidence。
- 不把 group association 伪装成几何坐标、样式、颜色或业务对象关系。
- 每条 decoded group record 必须有 bounded `byte_range`、`type_code`、
  `oid`、`parent_ref`、`sub_type_word` 和 raw reference payload。
- 只有在 OID list 结构有 cross-fixture validation 后，才把 child references 暴露
  到 stable DTO。
- 5 道 pre-commit gate 必须保持：build / test / clippy -D warnings / fmt --check /
  missing_docs ratchet。

## 非目标

- 不做 line style / color / layer 语义承诺，除非 `0x00FA` payload 里有清楚、
  多 fixture 验证的字段证据。
- 不解析 `0x0010` 子记录家族。
- 不做 Sheet geometry 编辑或写回。
- 不新增真实私有 fixture。
- 不 commit `dlls/` 里的任何二进制素材。
- 不升级 public coverage 分级，除非用户单独确认。

## Ask Before（要先问）

- 需要把 `SheetGeometry` 或 public schema 加字段时，先给出字段名和兼容性影响。
  本阶段采用 audit-only `SheetGeometry::decoded_graphic_groups`，只暴露稳定头部
  和 raw payload，不暴露 `child_oids`。
- 需要把 `0x00FA` 记录输出为 normalized graph relationship 时，先确认 model 边界。
- 需要引入新主 crate dependency 时，先确认 license / CI 成本。
- 需要修改 coverage tier、confidence enum、`PidGraphicKind` public contract 时，先确认。
- 需要提交 / push / 改写已有 Phase 14 文档时，先确认。

## Done Means（完成判据）

同时满足：

1. `decode_graphic_groups` / `decode_graphic_group_at` 这类 typed decoder 落地，
   panic-safe，并覆盖 `0x00FA` canonical + rejection tests。
2. 至少 2 个现有 registry fixture 上输出 `0x00FA` decoded group records，
   cross-fixture 总数与 probe 数量可解释。
3. Stable DTO 至少暴露 group `oid`、`parent_ref`、`sub_type_word`、byte range 和
   raw reference payload；child OID list 只有在验证充分时暴露。
4. Pipeline 能把 decoded group records 挂进 `SheetGeometry` 或等价 audit model，
   并保留完整 provenance。
5. `tests/parser_panic_safety.rs` 和 `tests/parse_real_files.rs` 有回归保护。
6. 5 道 gate 通过，`progress.jsonl` 含每个 AC 的具体证据。

停止条件全部写入 `blockers.md`。
