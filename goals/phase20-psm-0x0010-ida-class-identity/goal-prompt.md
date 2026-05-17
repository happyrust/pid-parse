# Codex Goal Prompt: Phase 20 PSM 0x0010 IDA-confirmed RAD class identity

本目录下的 goal package 用于启动 Phase 20。准备执行时，把下面 `/goal`
段落粘到 Codex：

```text
/goal 开始 Phase 20：用 IDA-confirmed evidence 回答 Phase 18/19 留下的核心问题——PSM type code 0x0010 真实属于哪个 RAD 类、sub-kind discriminator 在哪个字节偏移、共有多少种 sub-kind。**本 phase 是纯 reverse engineering + 文档，不改 src/ 代码、不改 test**。完成时输出 docs/analysis/2026-05-17-phase20-psm-0x0010-rad-class.md（结构 mirror Phase 16 jstyleoverride-v3-fields.md）。

用 `goals/phase20-psm-0x0010-ida-class-identity/` 作为 durable source of truth：

- 读 `brief.md`：使命、背景、约束、非目标、Ask Before、Done Means
- 跟 `plan.md`：IDA instance roadmap、调查 7 步、Slice A-G、AC1-AC7
- 跑 `verification.md`：5 道 pre-commit gate（不改代码应该自动绿）
- 遇到 `blockers.md` 的 Stop-And-Ask 条件时立即暂停、写 `progress.jsonl`、等用户

执行顺序（IDA-side）：

1. **Slice A**：IDA `list_instances` → `select_instance(13346)` → `survey_binary` (`radsrvitem.dll`) → 用 `search_text` 找 PSM 0x0010 dispatch 入口（factory function 候选）。每个发现 append progress.jsonl `ida_recon` entry。
2. **Slice B**：反编译 factory function (`analyze_function`)，找 CLSID lookup + 跨 DLL 跳转点。识别目标 RAD class CLSID + 所在 DLL。
3. **Slice C**：`select_instance` 切到目标 DLL，找 class 的 Read/Load/IO 函数，反编译，列出 IOContext::DoIO sequence（按 byte offset + 字段类型）。
4. **Slice D**：在 Read 函数里识别 sub-kind discriminator——字段偏移 + 数据类型 + switch 分支枚举值。
5. **Slice E**：cross-fixture validation：用 Phase 19 leading_word 数字（0x0002=164, 0x0003=21, 0x0001=18, 总计 582）反向验证 IDA-derived sub-kind enumeration 与实际数据吻合。
6. **Slice F**：写 docs/analysis/2026-05-17-phase20-psm-0x0010-rad-class.md，8 节结构：(1) Class identity, (2) IO sequence, (3) Sub-kind discriminator, (4) Cross-fixture distribution, (5) IDA 地址索引, (6) 与 Phase 16 JStyleOverride reference chain 关系, (7) Known unknowns, (8) Phase 21 implementation prerequisites（typed DTO 字段表草图）。
7. **Slice G**：跑 5 道 pre-commit gate 确认无意外退化：cargo build --locked --workspace --all-targets / cargo test --locked --workspace --all-targets / cargo clippy --locked --workspace --all-targets -- -D warnings / cargo fmt --all -- --check / cargo rustdoc --lib --locked -- -W missing-docs。每个 IDA tool call + AC 完成都 append 到 goals/phase20-psm-0x0010-ida-class-identity/progress.jsonl。

不要做：

- 不改任何 src/ 代码、不改任何 test、不改任何 schema。本 phase 严格 reverse engineering + documentation only。
- 不落地 typed sub-kind DTO（推迟 Phase 21）。
- 不实现 cross-record reference resolver。
- 不引入新 PidGraphicKind variant。
- 不装载新 IDA instance（当前 12 个 reachable instance 足够）。若不够用，写 blocker 暂停。
- 不修改 Phase 14-19 任何 stable DTO 既有字段。
- 不在 analysis 文档中包含大块连续反汇编（≤ 100 行；优先伪代码 + 关键 byte 模式）。
- 不提交 dlls/、.i64、私有 fixture。
- 不 commit / push，除非用户明确授权。

完成时 append：

{"type":"goal_complete","timestamp":"...","phase":"20","work_type":"reverse_engineering_only","rad_class":"<class name>","clsid":"<CLSID>","dll":"<dll name>","factory_address":"<addr>","sub_kind_discriminator_offset":<offset>,"sub_kind_discriminator_type":"<u8/u16/u32/u64>","sub_kind_enumeration":[{"value":"<v>","record_count":<n>}],"analysis_doc":"docs/analysis/2026-05-17-phase20-psm-0x0010-rad-class.md","phase14_baselines_preserved":true,"phase15_audit_preserved":true,"phase16_jstyle_preserved":true,"phase17_primitive_arc_removed":true,"phase18_audit_preserved":true,"phase19_leading_word_preserved":true,"normalized_geometry_unchanged":true,"gates":"5/5 green","src_code_changes":false,"new_ida_instances":false}

然后暂停等用户签收。Phase 21 typed DTO 实现需要单独 /goal 启动。
```

## 启动检查清单

- [ ] `brief.md` / `plan.md` / `verification.md` / `blockers.md` 已读
- [ ] `progress.jsonl` 含 initial scaffold + IDA instance roadmap entry
- [ ] 已读 `docs/plans/2026-05-16-phase16-jstyleoverride-final-summary.md`（Phase 16 跨 5 IDA instance 反向方法论）
- [ ] 已读 `docs/analysis/2026-05-16-jstyleoverride-v3-fields.md`（authoritative analysis doc 模板）
- [ ] 已读 `docs/analysis/2026-05-15-garc2d-packed-int-tail.md` §11（0x0010 reference chain hints）
- [ ] 已读 `goals/phase18-psm-0x0010-sub-record/brief.md`（Phase 18 audit-only 设计）
- [ ] 已读 `goals/phase19-psm-0x0010-leading-word-audit/brief.md`（Phase 19 leading-word 部分覆盖证据）
- [ ] 已确认 Phase 19 已 commit + push (`6beb6f1`)，working tree 干净（除 dlls/ / .superdesign/ 外）
- [ ] 已运行 IDA `list_instances` 确认 12 个 reachable instance（特别是 radsrvitem.dll port 13346）
- [ ] 首个执行动作是 Slice A IDA dispatch table recon，不是直接写 analysis doc
