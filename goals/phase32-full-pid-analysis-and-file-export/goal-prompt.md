# Goal Prompt: Phase 32 Full PID Analysis And File Export

> **[DRAFT — awaiting Plannotator gate]**

准备执行时，把下面 `/goal` 段落交给执行 agent：

```text
/goal 开始 Phase 32：制定并执行完整的 SmartPlant / Smart P&ID `.pid` 文件格式分析与文件化解析路线。目标是把当前 parser / IDA / byte-audit / writer / publish XML 能力收束成两个核心交付：PID format atlas 和 `.pid.bundle/` export contract，后续再实现 `pid_inspect --export-bundle`。

用 goals/phase32-full-pid-analysis-and-file-export/ 作为 durable source of truth：

- 读 brief.md：目标、背景、上下文、约束、非目标、Done Means
- 跟 plan.md：Solution Overview、Slice P0/A/B/C/D/E/F/G、bundle contract draft、风险表
- 跑 verification.md：planning checks、future implementation commands、bundle shape checks、completion signatures
- 遇 blockers.md 的 Stop-And-Ask 条件立即暂停，写 progress.jsonl，等用户

必读上下文：

- docs/plans/2026-06-16-phase32-full-pid-analysis-and-file-export-plan-cn.md
- docs/analysis/2026-06-12-phase30-ida-gated-next-actions.md
- docs/analysis/2026-06-13-phase31-olecrt-storage-entrypoints.md
- docs/prd-pid-parse-current-state.md
- docs/architecture-guide.md
- task_plan.md / findings.md / progress.md

执行顺序：

1. P0：确认当前 Phase 30/31 文档增量和 git status；未获用户授权不 commit。
2. Slice A：创建 `docs/analysis/2026-06-16-pid-format-atlas-cn.md`，列出 stream/storage confidence matrix（Decoded / TypedAudit / Probe / IdentifiedOnly / Unknown）。
3. Slice B：创建 `docs/pid-export-bundle-contract.md`，定义 `.pid.bundle/` 目录结构、manifest、raw/decoded/geometry/audit/writer/publish 子目录与 schema version。
4. Slice C：设计 `ExportBundlePlan` 和 `pid_inspect --export-bundle <dir>` 的实现切片；先写 tests 计划，不急着代码。
5. Slice D：对 `PSMspacemap`、StyleCluster、`0x0010`、GraphicGroup、JSitesList tail、`0x0089` 做 gated backlog 表；没有 direct evidence 就 negative/deferred。
6. Slice E：定义 writer/round-trip 可编辑边界；禁止从 probe/geometry JSON 反写 Sheet bytes。
7. Slice F：定义 publish XML opt-in 子树；MDF-backed only，并记录 MDF hash。
8. Slice G：补 README / schema golden / CI smoke 计划；执行实现时跑 verification.md gates。

不要做：

- 不把 Probe / TypedAudit 升级为 Decoded，除非有 byte-range + fixture + IDA/controlled fixture 证据。
- 不默认输出全部 raw stream bytes。
- 不从 geometry/probe JSON 写回 `.pid` Sheet bytes。
- 不把 MDF publish XML 当作 `.pid` raw decode 结果。
- 不继续 broad IDA。
- 不 commit / push，除非用户明确授权。

完成 planning-only 时 append：

{"type":"planning_complete","timestamp":"...","phase":"32","goal_package":"goals/phase32-full-pid-analysis-and-file-export","implementation_started":false}

Plannotator gate 通过后再进入 Slice A；若 gate 不可用，append `gate_deferred` 并暂停等用户。
```

## 启动检查清单

- [ ] `brief.md` / `plan.md` / `verification.md` / `blockers.md` 已读
- [ ] `progress.jsonl` 含 initial scaffold entry
- [ ] 已读 Phase 32 总计划文档
- [ ] 已确认 Plannotator gate 状态
- [ ] 已确认当前 git status，避免混入未授权 commit
- [ ] 首个执行动作是 format atlas，不是 parser promotion
