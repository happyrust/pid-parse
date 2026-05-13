# Codex Goal Prompt: Phase 14 SPPID Sheet 几何 primitive 解码器

本目录下 5 份关键文档（`brief.md` / `plan.md` / `verification.md` /
`blockers.md` / 本 `goal-prompt.md`）已全部通过 plannotator gate。
准备好后把下面 `/goal` 段落粘到 Codex 即可启动：

```text
/goal 让 SPPID `.pid` 文件的 Sheet 流里**至少一类**几何 primitive（PrimitiveLine / Polyline / Circle / Arc / TextPlacementStyle / SymbolPlacement / CoordinatePageMetadata）输出 PidGeometryConfidence::Decoded，并且证据链来自 IDA Pro 对 SPPID 运行时核心 DLL（rad2d.dll / pidobjectmanager.dll）的逆向分析，而不是单纯的字节差异归纳。

用 `goals/phase14-sppid-sheet-geometry/` 作为本 goal 的 durable source of truth：

- 读 `brief.md`：使命 / 上下文 / 限制 / 非目标 / Ask Before 规则 / 完成判据
- 跟 `plan.md`：方案叙事、为何选 IDA 反向、Slice A–F 任务表与依赖、AC1–AC11 acceptance criteria + 证据表
- 跑 `verification.md` 里命令矩阵 + 半手工核查；每条 evidence append 进 `progress.jsonl`
- 任何 `blockers.md` 里的 Stop-And-Ask 触发条件出现，立刻暂停 + 写 evidence + 等用户

执行风格：

1. **B1 阻塞先解**：等用户 commit `rad2d.dll` / `pidobjectmanager.dll` 等到 `dlls/` 之后再开干。期间每天 daily check 一次 IDA `list_instances`，进度记 0
2. **TDD red-green**：每个 Slice D 子任务先在 `tests/parser_panic_safety.rs` 加 adversarial smoke + `tests/parse_real_files.rs` 加 red unit test，再实现 decoder
3. **IDA 结论必须 export**：所有反向工程发现写进 `docs/analysis/2026-05-XX-rad2d-*.md`（callsites / dispatcher / primitive-line-layout 至少三份），不能只留在 `.i64` 数据库里
4. **provenance 三件套硬约束**：每个 decoded entity 必须同时填 `stream_path` / `byte_range` / `record_kind`；任一空就退回 `Inferred`
5. **commit / push 必须明确授权**：本会话学到的教训是 "按推荐方案继续" 不算 push 授权，必须用户明文说 `commit` 或 `push`
6. **5 道 pre-commit gate 永远先跑**：build / test --workspace --all-targets / clippy -D warnings / fmt --check / missing_docs ratchet (0=0)。任一失败立即停手

不要做的事（详见 `brief.md` 非目标 + `blockers.md` 高风险动作）：

- 不实现编辑 / 写回路径
- 不把现有 `EndpointPair + Inferred` line 重标为 `Decoded`
- 不解 Oracle exp 行数据（DWG fixture 是 Oracle，行反向是另外的 goal）
- 不动 `pid_parse::inspect::controlled_diff::promoted_geometry` 硬编码 false 不变式
- 不 commit 任何二进制 DLL 进 git
- 不在没用户授权下做 `git push` / `git push --force` / `force` reset

完成判据（AC1–AC11 全过）一旦满足，写最后一条 `progress.jsonl`：

```json
{"type":"goal_complete","timestamp":"...","decoded_class":"PrimitiveLine","fixture":"DWG-0201GP06-01.pid","decoded_line_count":N,"inferred_line_count":M,"ci_run_url":"..."}
```

并暂停等用户最终签收。不主动开新 goal，不扩展到其他 primitive 类（每类一个独立 goal）。
```

## 启动检查清单

- [ ] 5 份关键文档全 plannotator approved
- [ ] `goals/phase14-sppid-sheet-geometry/progress.jsonl` 已有 scaffold 条目
- [ ] 用户已读 `brief.md` + `blockers.md`，理解 B1 是硬阻塞
- [ ] 用户决定何时把 `rad2d.dll` 入仓 → 触发 goal 真正开跑

`/goal` 段粘贴前如果用户还在收集 DLL，可以先粘贴并标 `状态=blocked-on-B1`，
让 Codex daily check IDA list_instances 而不是空跑。
