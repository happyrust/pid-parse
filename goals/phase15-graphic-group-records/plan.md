# Plan: Phase 15 PSM 0x00FA GraphicGroup Records

## 方案总览

Phase 15 把 Phase 14 的成果从“图元 decoded”推进到“图元分组关系可解释”。
核心目标是为 PSM `0x00FA` records 建立一个保守、可回归、可审计的 typed
decoder。

执行路线沿用 Phase 14 已验证的七层模板：

1. 扩展 probe：让 `examples/probe_psm_0x00fa_shape.rs` 输出更多 fixture、offset、
   size、payload words、候选 OID list 和相邻 geometry OID。
2. Layout discovery：从真实 byte dump 归纳 `0x00FA` payload 的稳定头部和 variable
   tail 结构。
3. Decoder API：在 `src/parsers/sheet_records.rs` 新增 `SheetGraphicGroupDecoded`
   DTO、`decode_graphic_groups`、`decode_graphic_group_at` 和常量。
4. Validation rules：严格校验 type code、payload bounds、size domain、OID sanity、
   parent_ref domain；不确定字段只保留 raw bytes。
5. Unit tests：canonical record builder + 每个 validation rejection + truncation /
   random input panic safety。
6. Model DTO：新增 stable/audit DTO，接入 `SheetGeometry` 或等价 decoded group
   collection，并同步 schema ratchet。
7. Pipeline：`streams/cluster.rs` 调 decoder；如 child OID list 可靠，则在
   `geometry.rs` 或 audit path 中建立 group provenance association。

## 为什么先做 0x00FA

Phase 14 已经把几何 primitive 本身打开：line、arc、polyline、point、text、
symbol 都能 decoded 输出。但用户下一步最需要的不是再猜一个低频 primitive，
而是解释这些 primitive 的关系和组织方式。

`0x00FA` 是最合适的下一刀：

- hit count 足够高：当前 broad probe 为 353，保守 decoder ratchet 为 352；
  Phase 14 的 250 是 legacy discovery count，未保存过滤条件
- 与 geometry OID 关系密切：现有 probe 显示 group oid 常与临近 geometry 相关
- 结构比 `0x0010` 更像 standalone record：有稳定 type header、OID、parent_ref 和
  variable payload
- 风险可控：即便 child list 暂不稳定，也可以先输出 typed header + raw reference
  payload，不污染 geometry semantics

`0x0010` 暂不优先，因为 Phase 14 总结判断它更像 embedded sub-record /
attribute fragment，脱离宿主 record 解码容易过拟合。

## Slice 表

| Slice | 目的 | 主要文件 | Done when | 风险 |
|---|---|---|---|---|
| A | 扩展 `0x00FA` probe，收集结构证据 | `examples/probe_psm_0x00fa_shape.rs` | 输出 per-fixture hit count、size distribution、前 N 条 payload dump、候选 OID words | dump 不足以区分 references vs flags |
| B | 建立 layout hypothesis | `docs/analysis/2026-05-14-psm-0x00fa-graphic-group-layout.md` 或新日期文档 | 写出 header / tail / OID list 假设和反例 | 多版本 sub_type 混在一起 |
| C | Parser DTO + decoder API | `src/parsers/sheet_records.rs` | `decode_graphic_groups` 和 `decode_graphic_group_at` 单测通过 | validation 太宽导致 false positive |
| D | Model + pipeline 接入 | `src/model.rs`, `src/streams/cluster.rs`, `src/schema.rs` | `SheetGeometry` 或 audit model 包含 decoded groups，schema ratchet 通过 | public DTO 名称/字段需要用户确认 |
| E | Cross-fixture integration guard | `tests/parse_real_files.rs`, `tests/parser_panic_safety.rs` | 至少 2 fixtures 输出 decoded groups，panic-safety matrix 覆盖新入口 | 私有 fixture 不足 |
| F | Optional association | `src/geometry.rs` 或 inspect/audit path | 只有 child OID list 稳定时，关联 group -> child geometry provenance | 关系语义不够稳，需停在 audit-only |
| G | 收口 | `progress.jsonl`, verification docs | 5 道 gate 绿，每个 AC 有 evidence | CI 时间长或 unrelated dirty tree |

## Phase Boundaries

Phase 15 的最低可交付版本是：typed `0x00FA` record decoder + DTO + tests +
pipeline collection。OID list association 是条件性交付，必须建立在 Slice B/E 的
证据之上。

如果 OID list 结构不稳定，Phase 15 仍可完成为 “decoded group header + raw reference
payload inventory”。这比把未验证的 child list 放进 stable model 更好。

## Acceptance Criteria

- [x] **AC1**：扩展后的 `probe_psm_0x00fa_shape` 能输出 cross-fixture size
      distribution、sample payload words、candidate OID words 和相邻 geometry
      context。
- [x] **AC2**：新增/更新 analysis 文档记录 `0x00FA` byte layout，至少覆盖
      canonical header、known sub_type variants、tail uncertainty 和 rejected
      interpretations。
- [x] **AC3**：`src/parsers/sheet_records.rs` 新增 `SheetGraphicGroupDecoded`
      DTO、`decode_graphic_groups`、`decode_graphic_group_at`，全部 bounds checked。
- [x] **AC4**：unit tests 覆盖 canonical decode、wrong type、short header、
      truncated payload、invalid size、OID sanity rejection 和 panic-free random
      input。
- [x] **AC5**：`tests/parser_panic_safety.rs` 覆盖新 public decoder entry。
- [x] **AC6**：`tests/parse_real_files.rs` 新增 cross-fixture guard，至少 2 个
      fixture 输出 decoded `0x00FA` records，并验证 byte range / oid /
      parent_ref / sub_type_word。
- [x] **AC7**：model / schema 暴露 decoded group records，或明确 audit-only
      collection；字段语义与 `brief.md` 约束一致。
- [x] **AC8**：如果暴露 child OID list，必须有 cross-fixture validation；否则只
      暴露 raw reference payload。
- [x] **AC9**：Phase 14 现有 decoded geometry tests 不退化。
- [x] **AC10**：5 道 gate 通过：build、test、clippy、fmt、missing_docs ratchet。
- [x] **AC11**：`progress.jsonl` 对 AC1-AC10 都有具体命令 / artifact / 输出摘要。

## Required Evidence

| Requirement | Evidence to inspect | Where recorded |
|---|---|---|
| AC1 | `cargo run --release --example probe_psm_0x00fa_shape` 输出 | `progress.jsonl` |
| AC2 | analysis 文档含 byte table + uncertainty | git diff + `progress.jsonl` |
| AC3-AC4 | `cargo test --locked --lib parsers::sheet_records::tests::graphic_group` 或等价过滤 | `progress.jsonl` |
| AC5 | `cargo test --locked -j 1 --test parser_panic_safety` | `progress.jsonl` |
| AC6-AC9 | `cargo test --locked --workspace --all-targets` plus `cargo test --test parse_real_files graphic_group_decoder_ratchets_fixture_counts_and_header_fields -- --nocapture` | `progress.jsonl` |
| AC10 | 5 道 pre-commit gate 输出 | `progress.jsonl` |

## Completion Audit

声明完成前，逐项对照 AC1-AC11。任何 stable DTO 字段都必须能追溯到真实 byte
layout 证据；任何不能证明的 tail 字段必须留在 raw / audit 层。
