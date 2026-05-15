# Codex Goal Prompt: Phase 16 PSM 0x0030 真实归属与 decoder 重写

本目录下的 goal package 用于启动 Phase 16。准备执行时，把下面 `/goal`
段落粘到 Codex：

```text
/goal 继续 Phase 14 §6.1 触发的下一阶段：把 SmartPlant P&ID `.pid` Sheet 流中 PSM type code `0x0030` 的 decoder 从错误的 GArc2d 假设，改写为基于真实类型身份（`j2dsrv.dll` CLSID `{47FCC338-2D0F-11D0-A1FF-080036A1CF02}` 注册的 RAD 2D 复合 record）的 conservative typed decoder。

用 `goals/phase16-j2dsrv-record-decode/` 作为 durable source of truth：

- 读 `brief.md`：使命、背景、约束、非目标、Ask Before、Done Means
- 跟 `plan.md`：七层模板、Slice A-H、AC1-AC11、required evidence
- 跑 `verification.md`：IDA 反编译、parser tests、panic-safety、cross-fixture integration、5 道 gate、Phase 14/15 baselines
- 遇到 `blockers.md` 的 Stop-And-Ask 条件时立即暂停、写 `progress.jsonl`、等用户

执行顺序：

1. **Slice A**：让用户在 IDA 加载 `dlls/j2dsrv.dll`，等新 IDA instance（port ≥ 13347）出现在 `list_instances`。
2. **Slice B**：反编译 CLSID `47FCC338` 的 ClassFactory / Save / Load / Validate vtable slot；从 RTTI 字符串拿真实类名；把字段表写入 `docs/analysis/2026-05-1?-j2dsrv-47FCC338-fields.md`，与 `docs/analysis/2026-05-15-garc2d-packed-int-tail.md` §10 对账。
3. **Slice C**：与用户对齐 DTO / decoder 新名（候选见 plan.md §2），以及是否新增 `PidGraphicKind` variant。
4. **Slice D**：在 `src/parsers/sheet_records.rs` 重命名 + 重写 decoder，按真实字段表映射 64B payload + tail 中已锁定字段；未锁定字段保留为 raw bytes。先测试后实现：unit tests 覆盖 canonical、wrong type、short header、truncated payload、各字段 sanity rejection、panic-free random input。
5. **Slice E**：`src/model.rs` 重命名 DTO + 更新 `From` impl；`src/streams/cluster.rs` 字段映射；`src/schema.rs` ratchet。
6. **Slice F**：`src/geometry.rs` emission 路径按用户 Slice C 决策落地（新 variant 或 audit-only）。
7. **Slice G**：`tests/parse_real_files.rs` cross-fixture ratchet：4 fixtures decoded 总数 ∈ [90, 98]；Phase 14 其他 decoder 与 Phase 15 audit collection 计数全保。`tests/parser_panic_safety.rs` 加新 entry。
8. **Slice H**：跑 5 道 gate：`cargo build --locked --workspace --all-targets`、`cargo test --locked --workspace --all-targets`、`cargo clippy --locked --workspace --all-targets -- -D warnings`、`cargo fmt --all -- --check`、`bash .github/scripts/check-missing-docs.sh`（Windows 上用等价 `cargo rustdoc --lib --locked -- -W missing-docs`）。每个 AC 的命令 / artifact / 结果 append 到 `goals/phase16-j2dsrv-record-decode/progress.jsonl`。

不要做：

- 不在没拿到 IDA 反编译证据前就给字段命名（rotation / sweep_extent / axis_a / axis_b 等都是猜测）
- 不把 0x0030 record 在 stable schema 里命名为 "Arc" / "Circle"
- 不实现 J2DSrv 其他 12 个 type code (0x29..0x2F + 0x31..0x35) 的 decoder
- 不解析 `0x0010` sub-record family
- 不改 Phase 14 / Phase 15 已落地的 decoder
- 不提交 DLL / `.i64` / 私有 fixture
- 不 commit / push，除非用户明确授权

完成时 append：

```json
{"type":"goal_complete","timestamp":"...","decoded_type":"PSM 0x0030 (j2dsrv 47FCC338)","real_class_name":"<IDA 拿到的类名>","fixtures":4,"decoded_record_count":N,"phase14_baselines_preserved":true,"phase15_audit_preserved":true,"gates":"5/5 green"}
```

然后暂停等用户签收，不主动扩到 J2DSrv 其他 type code 或 0x0010 sub-record decoding。
```

## 启动检查清单

- [ ] `brief.md` / `plan.md` / `verification.md` / `blockers.md` 已读
- [ ] `progress.jsonl` 有 initial scaffold 条目
- [ ] 确认当前工作树中 Phase 15 的 351 行未提交改动叠加 Phase 16 改动
      是预期行为，且最终 commit 拆分由用户决定
- [ ] 已查 `docs/analysis/2026-05-15-garc2d-packed-int-tail.md` §10 与
      `examples/probe_garc2d_packed_bytes.rs`，理解触发证据
- [ ] 首个执行动作是让用户加载 `j2dsrv.dll` 到 IDA，不是直接动 decoder
      代码
