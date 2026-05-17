# Codex Goal Prompt: Phase 18 PSM 0x0010 sub-record family — audit-only decoder

本目录下的 goal package 用于启动 Phase 18。准备执行时，把下面 `/goal`
段落粘到 Codex：

```text
/goal 开始 Phase 18：为 PSM type code 0x0010（跨 4 fixture 638 hits，被 Phase 16 JStyleOverride 和 Phase 15 GraphicGroup tail 反复引用的 sub-record 家族）实现 conservative typed audit-only decoder，沿用 Phase 15 GraphicGroup 模板：暴露稳定 header + raw_payload + 完整 provenance，不命名 sub-kind 字段，不引入新 PidGraphicKind variant。

用 `goals/phase18-psm-0x0010-sub-record/` 作为 durable source of truth：

- 读 `brief.md`：使命、背景、约束、非目标、Ask Before、Done Means
- 跟 `plan.md`：solution overview、why audit-only、Slice A-H、AC1-AC11、required evidence
- 跑 `verification.md`：parser/schema/panic-safety/cross-fixture tests、5 道 gate
- 遇到 `blockers.md` 的 Stop-And-Ask 条件时立即暂停、写 `progress.jsonl`、等用户

执行顺序：

1. **Slice A**：用 `rg "decode_graphic_groups|DecodedGraphicGroupRecord|SheetGraphicGroupDecoded|decoded_graphic_groups"` 盘点 Phase 15 audit-only 模板的实现路径（parser/model/cluster/test/panic-safety/schema），把 5 个 mirror 点 append 到 `progress.jsonl`。
2. **Slice B**：在 `src/parsers/sheet_records.rs` 新增 `SheetSubRecord0x0010Decoded` parser DTO（`byte_range / type_code / type_flags / bytes_to_follow / raw_payload: Vec<u8>`，**无 `oid` 字段**——0x0010 是 6-byte header 形态）+ `decode_sub_records_0x0010(data: &[u8]) -> Vec<SheetSubRecord0x0010Decoded>` + `decode_sub_record_0x0010_at(data: &[u8], offset: usize) -> Option<SheetSubRecord0x0010Decoded>` + 公开 `PSM_TYPE_CODE_SUB_RECORD_0X0010 = 0x0010` 常量；写 parser unit tests 覆盖 canonical record / wrong type_code / oversized bytes_to_follow / truncated input / empty input / 全0 / 全0xFF。
3. **Slice C**：在 `src/model.rs` 新增 `DecodedSubRecord0x0010Record` stable DTO（mirror parser DTO 字段，`byte_start / byte_end / type_code / type_flags / bytes_to_follow / raw_payload`）+ `From<SheetSubRecord0x0010Decoded>` impl + `JsonSchema` derive + `SheetGeometry::decoded_sub_records_0x0010: Vec<DecodedSubRecord0x0010Record>` 字段。同步 `src/schema.rs` 默认 schema needles（`DecodedSubRecord0x0010Record` + `decoded_sub_records_0x0010`）。
4. **Slice D**：在 `src/streams/cluster.rs` 调用新 decoder 并 populate `decoded_sub_records_0x0010` collection；同步 empty-sheet 检查分支。在 `src/cfb/reader.rs` 测试 fixture 中初始化新字段。
5. **Slice E**：在 `tests/parse_real_files.rs` 新增 `sub_records_0x0010_decoder_emits_audit_records_with_provenance` cross-fixture ratchet test，跨 4 fixture 总计 **582**（DWG-0201=161, DWG-0202=104, 工艺管道-1=306, A01=11；probe non-advancing scan 输出 638 含 overlap，本 decoder 采用 advancing scan）；每条 assert stream_path + byte_range + type_code==0x0010 + non-empty raw_payload + raw_payload.len() == bytes_to_follow。
6. **Slice F**：在 `tests/parser_panic_safety.rs` adversarial matrix 加 `decode_sub_records_0x0010` + `decode_sub_record_0x0010_at`。
7. **Slice G**：在 `CHANGELOG.md` 写 Phase 18 入口（638 baseline + audit-only 设计 + 不引入 PidGraphicKind）。
8. **Slice H**：跑 5 道 gate：`cargo build --locked --workspace --all-targets`、`cargo test --locked --workspace --all-targets`、`cargo clippy --locked --workspace --all-targets -- -D warnings`、`cargo fmt --all -- --check`、`cargo rustdoc --lib --locked -- -W missing-docs`（Windows）或 `bash .github/scripts/check-missing-docs.sh`。每个 AC 的命令 / artifact / 结果 append 到 `goals/phase18-psm-0x0010-sub-record/progress.jsonl`。

不要做：

- 不在 `SheetSubRecord0x0010Decoded` / `DecodedSubRecord0x0010Record` 中命名 sub-kind 字段（如 `coord_a_x` / `referenced_type` / `tag_block`）。本 phase 严格 audit-only。
- 不引入新 `PidGraphicKind` variant。
- 不实现 cross-record reference resolver（JStyleOverride.referenced_oid → 0x0010 lookup）。
- 不反向 RAD `47FCC330..47FCC33E` siblings（Phase 19 工作）。
- 不提取 plant instrument tag from raw_attribute_tail。
- 不修改 Phase 14 / 15 / 16 / 17 既有 decoder 的输出 / 字段名。
- 不删除 Phase 14 / 15 / 16 / 17 analysis 文档。
- 不提交 `dlls/`、`.i64`、私有 fixture。
- 不 commit / push，除非用户明确授权。

完成时 append：

{"type":"goal_complete","timestamp":"...","phase":"18","decoded_type":"PSM 0x0010","class_name":"audit-only (not yet IDA-confirmed)","sub_record_count":582,"per_fixture":{"DWG-0201GP06-01.pid":161,"DWG-0202GP06-01.pid":104,"工艺管道及仪表流程-1.pid":306,"export-test/publish-data/A01/A01.pid":11},"phase14_baselines_preserved":true,"phase15_audit_preserved":true,"phase16_jstyle_preserved":true,"phase17_primitive_arc_removed":true,"normalized_geometry_unchanged":true,"gates":"5/5 green","pidgraphickind_new_variant":false}

然后暂停等用户签收，不主动扩到 sub-kind 字段反向 / reference resolver / Phase 19。
```

## 启动检查清单

- [ ] `brief.md` / `plan.md` / `verification.md` / `blockers.md` 已读
- [ ] `progress.jsonl` 含 initial scaffold + probe_evidence 条目
- [ ] 已读 `docs/plans/2026-05-14-phase15-graphic-group-final-summary.md`（audit-only 模板）
- [ ] 已读 `docs/plans/2026-05-16-phase16-jstyleoverride-final-summary.md` Future Work 段
- [ ] 已确认 Phase 17 已 commit + push，working tree 干净
- [ ] 首个执行动作是 Slice A inventory（mirror Phase 15），不是直接写 decoder
- [ ] Slice B 前已确认 DTO 命名（Q1 推荐：`SheetSubRecord0x0010Decoded` / `DecodedSubRecord0x0010Record`）
