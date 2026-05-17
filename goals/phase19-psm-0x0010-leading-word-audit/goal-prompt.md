# Codex Goal Prompt: Phase 19 PSM 0x0010 leading-word audit

本目录下的 goal package 用于启动 Phase 19。准备执行时，把下面 `/goal`
段落粘到 Codex：

```text
/goal 开始 Phase 19：在 Phase 18 已落地的 PSM 0x0010 audit-only 集合上，加一个稳定 audit-only `leading_word: Option<u16>` 字段（= `payload[0..2]` little-endian u16；payload 长度 < 2 时为 None）。这是 Phase 19 probe 在 4 fixture 578 records 上确认的部分 sub-kind discriminator（top: 0x0002=164/28%, 0x0003=21/3.6%, 0x0001=18/3.1%）。本 phase 严格 audit-only：不命名 sub-kind 字段（不叫 `sub_kind` / `record_kind` / `family_tag` 等），不引入 PidGraphicKind variant，不解析 reference chain，不退化 Phase 14-18 任何 baseline。

用 `goals/phase19-psm-0x0010-leading-word-audit/` 作为 durable source of truth：

- 读 `brief.md`：使命、背景（含 Phase 19 probe 完整直方图）、约束、非目标、Ask Before、Done Means
- 跟 `plan.md`：solution overview、why audit-only、Slice A-G、AC1-AC11、required evidence
- 跑 `verification.md`：parser/schema/ratchet/panic-safety/Phase 18 regression、5 道 gate
- 遇到 `blockers.md` 的 Stop-And-Ask 条件时立即暂停、写 `progress.jsonl`、等用户

执行顺序：

1. **Slice A**：用 `rg "leading_word|decoded_sub_records_0x0010|SheetSubRecord0x0010Decoded"` 盘点 Phase 18 mirror 点（parser DTO / model DTO / schema / ratchet test），确认 cluster.rs / cfb/reader.rs / geometry.rs 不需改，写入 `progress.jsonl`。
2. **Slice B**：在 `src/parsers/sheet_records.rs::SheetSubRecord0x0010Decoded` 新增 `leading_word: Option<u16>` 字段；`decode_sub_records_0x0010` / `decode_sub_record_0x0010_at` 在 `payload.len() >= 2` 时填充 `Some(u16::from_le_bytes([payload[0], payload[1]]))`，否则 `None`。Doc comment 必须说明 "= `payload[0..2]` as little-endian u16; `None` if `payload.len() < 2`"，禁止说 "sub-kind discriminator"。写 ≥ 2 个 unit test：canonical decode 填充 `leading_word == Some(expected_word)`；短 payload 返回 `None`。Phase 18 既有 12 unit test 全绿。
3. **Slice C**：在 `src/model.rs::DecodedSubRecord0x0010Record` mirror 新字段 + `From<SheetSubRecord0x0010Decoded>` impl 同步；`JsonSchema` derive 自动覆盖。在 `src/schema.rs` 默认 schema 加 `leading_word` needle 到 ratchet test。
4. **Slice D**：在 `tests/parse_real_files.rs` 新增 `sub_records_0x0010_leading_word_distribution_matches_phase19_probe` cross-fixture ratchet test，跨 4 fixture 断言 `leading_word == Some(0x0002)` count = 164、`Some(0x0003)` = 21、`Some(0x0001)` = 18、`None` = 0。**先**跑一次 decoder-side 计数，若与 probe 数字差 ±5%（即 0x0002 ∉ 156..172），写 progress.jsonl 解释 advancing-scan vs probe overlap-scan 差异并停手等用户；否则更新 ratchet 期望值到 decoder ground truth。
5. **Slice E**：跑 Phase 18 既有 `sub_records_0x0010_decoder_emits_audit_records_with_provenance` ratchet test，确认仍输出 582（不退化）。同时确认 Phase 14/15/16/17 cross-fixture decoded counts 不变。
6. **Slice F**：在 `CHANGELOG.md` 写 Phase 19 入口：`leading_word` 字段 + top distribution (0x0002=164/28%, 0x0003=21/3.6%, 0x0001=18/3.1%) + audit-only 设计选择 + 与 Phase 18 additive 关系 + 不命名 sub-kind 的解释 + Phase 18 ratchet 仍是 582。可选更新 `AGENTS.md` 0x0010 段落补一句 `leading_word` audit 信息。
7. **Slice G**：跑 5 道 gate：`cargo build --locked --workspace --all-targets`、`cargo test --locked --workspace --all-targets`、`cargo clippy --locked --workspace --all-targets -- -D warnings`、`cargo fmt --all -- --check`、`cargo rustdoc --lib --locked -- -W missing-docs`（Windows）或 `bash .github/scripts/check-missing-docs.sh`。每个 AC 的命令 / artifact / 结果 append 到 `goals/phase19-psm-0x0010-leading-word-audit/progress.jsonl`。

不要做：

- 不在 `SheetSubRecord0x0010Decoded` / `DecodedSubRecord0x0010Record` 中加任何带语义的字段名（如 `sub_kind` / `record_kind` / `family_tag` / `payload_kind` / `discriminator` / `type_tag`）。本 phase 严格 audit-only `leading_word`。
- 不引入新 `PidGraphicKind` variant。
- 不实现 cross-record reference resolver（JStyleOverride.referenced_oid → 0x0010 lookup）。
- 不为 size 31 / 70 / 13 / 16 / 43 bucket 做 deeper discriminator 反向（推迟 Phase 20）。
- 不修改 Phase 14 / 15 / 16 / 17 / 18 既有 decoder 的输出 / 字段名。
- 不删除 Phase 14 / 15 / 16 / 17 / 18 analysis 文档。
- 不提交 `dlls/`、`.i64`、私有 fixture。
- 不 commit / push，除非用户明确授权。

完成时 append：

{"type":"goal_complete","timestamp":"...","phase":"19","decoded_type":"PSM 0x0010 leading_word","class_name":"audit-only (still not IDA-confirmed)","sub_record_count":582,"new_typed_field":"leading_word: Option<u16>","leading_word_distribution":{"0x0002":164,"0x0003":21,"0x0001":18,"None":0},"phase14_baselines_preserved":true,"phase15_audit_preserved":true,"phase16_jstyle_preserved":true,"phase17_primitive_arc_removed":true,"phase18_audit_preserved":true,"normalized_geometry_unchanged":true,"gates":"5/5 green","pidgraphickind_new_variant":false,"sub_kind_field_naming":false}

然后暂停等用户签收，不主动扩到 sub-kind 字段反向 / reference resolver / Phase 20。
```

## 启动检查清单

- [ ] `brief.md` / `plan.md` / `verification.md` / `blockers.md` 已读
- [ ] `progress.jsonl` 含 initial scaffold + Phase 19 probe evidence 条目
- [ ] 已读 `goals/phase18-psm-0x0010-sub-record/brief.md`（audit-only 模板）
- [ ] 已读 `docs/analysis/2026-05-17-phase19-rad-sibling-probe-null-result.md`（RAD sibling-sweep 被证伪，证明 leading-word 是更可行角度）
- [ ] 已确认 Phase 18 已 commit + push (`81daa20`)，working tree 干净（除 `dlls/`、`.superdesign/`、本 phase 新增 `examples/probe_psm_0x0010_sub_kind.rs` + `goals/phase19-…/` 外）
- [ ] 首个执行动作是 Slice A inventory，不是直接写 decoder
- [ ] Slice B 前已确认字段命名（Q1 推荐：`leading_word`），并打算把 doc comment 写成纯字节位置描述
