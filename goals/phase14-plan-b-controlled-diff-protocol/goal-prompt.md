# Codex Goal Prompt: Phase 14 Plan B 控制 diff fixture 采集协议

> **[DRAFT — awaiting plannotator gate]**：5 道 gate 未跑（用户未
> 在浏览器面板）。当 gate 全过后再粘下面 `/goal` 段到 Codex。

```text
/goal 把 Phase 14 的"控制 .pid 编辑 diff 采集协议"做成可被任何有 SmartPlant P&ID 访问权限的操作员照做的离线协议文档。协议产出后，操作员能产生 before / after `.pid` 文件对 + metadata sidecar 三件套，直接被 pid_parse::inspect::controlled_diff 消费。

用 `goals/phase14-plan-b-controlled-diff-protocol/` 作为本 goal 的 durable source of truth：

- 读 brief.md：使命 / 上下文 / 限制 / 非目标 / Ask Before 规则 / 完成判据
- 跟 plan.md：方案叙事、Slice 1–6 任务表、AC1–AC8 acceptance criteria + 证据表
- 跑 verification.md 里命令矩阵 + 手工核查；每条 evidence append 进 progress.jsonl
- 任何 blockers.md 里的 Stop-And-Ask 触发条件出现，立刻暂停 + 写 evidence + 等用户

执行风格：

1. **B2 优先**：5 道 plannotator gate 必须先全部 approved，才能驱动本 goal 真正进入 Slice 1。否则文档停在 DRAFT
2. **协议先文档化、再自检**：Slice 3 写完协议后 Slice 5 用合成 CFB fixture 走完整目录约定，证明协议可消费
3. **不**碰 inspect::controlled_diff 的类型不变式（promoted_geometry = false 硬编码）
4. **不**入任何真实 plant `.pid` fixture 进 git；只协议文档 + 占位 .gitkeep
5. **commit / push 必须明确授权**
6. **5 道 pre-commit gate 永远先跑**

不要做的事：

- 不实现 typed decoder（那是另一个 goal）
- 不承诺单次 fixture 立刻升级 PidGeometryConfidence
- 不写 SmartPlant 自动化脚本（VBA / AutoIt）
- 不引导 SmartPlant P&ID 安装

完成判据（AC1–AC8 全过）一旦满足，写最后一条 progress.jsonl：

```json
{"type":"goal_complete","timestamp":"...","protocol_doc":"docs/protocols/2026-05-XX-controlled-pid-diff-collection.md","case_count":6,"ci_run_url":"..."}
```

并暂停等用户最终签收。
```

## 启动检查清单

- [ ] 5 份关键文档全 plannotator approved（**当前 0/5**，因 B2 用户未在浏览器）
- [ ] `progress.jsonl` 已有 scaffold 条目
- [ ] 用户已读 brief.md + blockers.md
- [ ] 用户明确何时把 plannotator 浏览器面板打开做 gate

未 gate 完之前，本 goal-prompt 不要粘到 Codex /goal。
