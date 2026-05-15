# Codex Goal Prompt: Phase 15 PSM 0x00FA GraphicGroup Records

本目录下的 goal package 用于启动 Phase 15。准备执行时，把下面 `/goal`
段落粘到 Codex：

```text
/goal 继续 Phase 14 之后的下一阶段：为 SmartPlant P&ID `.pid` Sheet 流中的 PSM `0x00FA` GraphicGroup / GraphicPersist records 建立保守 typed decoder，并把 geometry OID 与 group / parent / child reference evidence 关联起来。

用 `goals/phase15-graphic-group-records/` 作为 durable source of truth：

- 读 `brief.md`：使命、背景、约束、非目标、Ask Before、Done Means
- 跟 `plan.md`：Phase 15 七层模板、Slice A-G、AC1-AC11、required evidence
- 跑 `verification.md`：probe、parser tests、panic-safety、cross-fixture integration、5 道 gate
- 遇到 `blockers.md` 的 Stop-And-Ask 条件时，立即暂停、写 `progress.jsonl`、等用户

执行顺序：

1. 先扩展 `examples/probe_psm_0x00fa_shape.rs`，输出 cross-fixture size buckets、payload words、candidate OID words、adjacent geometry context。
2. 写/更新 `docs/analysis/2026-05-14-psm-0x00fa-graphic-group-layout.md` 或同日期后续文档，记录稳定 header、sub_type variants、tail uncertainty 和 rejected interpretations。
3. 在 `src/parsers/sheet_records.rs` 中新增 `SheetGraphicGroupDecoded`、`decode_graphic_groups`、`decode_graphic_group_at`，全部 bounds checked，不 panic。
4. 先测试后实现：unit tests 覆盖 canonical、wrong type、short header、truncated payload、invalid size、OID sanity rejection；`tests/parser_panic_safety.rs` 加新 entry。
5. 接入 model / pipeline：新增 decoded group collection 或 audit-only collection；stable 字段只放已验证 header + raw reference payload。child OID list 只有 cross-fixture validation 充分时才能暴露。
6. 在 `tests/parse_real_files.rs` 加 `graphic_groups_decoder_emits_decoded_groups_with_provenance`，至少 2 个现有 fixture 输出 decoded groups，验证 byte_range / oid / parent_ref / sub_type_word。
7. 跑 5 道 gate：`cargo build --locked --workspace --all-targets`、`cargo test --locked --workspace --all-targets`、`cargo clippy --locked --workspace --all-targets -- -D warnings`、`cargo fmt --all -- --check`、`bash .github/scripts/check-missing-docs.sh`。
8. 每个 AC 的命令、artifact、结果 append 到 `goals/phase15-graphic-group-records/progress.jsonl`。

不要做：

- 不把 `0x0010` 纳入本 goal
- 不猜 style/color/layer 语义
- 不改写 Phase 14 decoded geometry confidence
- 不做 Sheet 编辑/写回
- 不提交 DLL、`.i64`、私有 fixture
- 不 commit / push，除非用户明确授权

完成时 append：

```json
{"type":"goal_complete","timestamp":"...","decoded_type":"PSM 0x00FA GraphicGroup","fixtures":N,"decoded_group_records":N,"child_oid_list_exposed":false,"gates":"5/5 green"}
```

然后暂停等用户签收，不主动扩大到 `0x0010` 或 style/color/layer decoding。
```

## 启动检查清单

- [ ] `brief.md` / `plan.md` / `verification.md` / `blockers.md` 已读
- [ ] `progress.jsonl` 有 initial scaffold 条目
- [ ] 确认当前工作树中大量 `dlls/` 和既有文档改动不是本 goal 范围
- [ ] 首个执行动作是扩展 probe，不是直接写 stable DTO
