# Phase 32: Full PID Analysis And File Export Plan

> **[DRAFT — awaiting Plannotator gate]**

## 目标产出

制定并执行一条完整路线：继续分析 SmartPlant / Smart P&ID `.pid` 文件格式，同时把解析结果交付为可审计、可复现、可被下游消费的文件化 bundle。

本 phase 先交付计划与合同，不直接扩大 parser promotion 面：

1. PID format atlas：把 stream/storage、parser、model、confidence、evidence、blocker 统一成可维护索引。
2. PID export bundle contract：定义 `.pid.bundle/` 目录结构、manifest、raw/decoded/geometry/audit/publish 子目录。
3. 后续 implementation plan：`pid_inspect --export-bundle`、round-trip/writer 边界、publish XML 分线、CI gates。
4. Plannotator gate：五件套通过后才进入代码执行。

## 背景

项目已经越过“能不能读出东西”的阶段：

- CFB/OLE、metadata、object graph、cross-reference、layout、writer passthrough 已成工程骨架。
- Sheet/PSM 已落地多类 typed decoder。
- `0x0030` 已经由 `style.dll` IDA 证据纠正为 `JStyleOverride`，不是 arc。
- `OLESITE.dll` 确认 `JSitesList` / `JSite` writer-reader 证据。
- `OLECRT.dll` 确认 `DocVersion2` version probe 与 `SymbolInformationCluster` embedded OLE/symbol 路径。

但剩余 byte-layout 深水区仍不能靠 broad search 或猜测推进：

- `PSMspacemap` raw page
- StyleCluster prefix
- `0x0010` discriminator
- GraphicGroup payload
- JSitesList stale tail
- `0x0089` / DA head semantic naming

## 上下文（必读）

| 文档 / 文件 | 作用 |
|---|---|
| `docs/plans/2026-06-16-phase32-full-pid-analysis-and-file-export-plan-cn.md` | Phase 32 总体路线 |
| `task_plan.md` | 当前 Phase status 入口 |
| `findings.md` | 已确认格式事实与决策 |
| `progress.md` | 最新 Phase 30/31 进度 |
| `docs/analysis/2026-06-12-phase30-ida-gated-next-actions.md` | IDA-gated backlog |
| `docs/analysis/2026-06-13-phase31-olecrt-storage-entrypoints.md` | OLECRT storage evidence |
| `docs/prd-pid-parse-current-state.md` | 产品现状与能力边界 |
| `docs/architecture-guide.md` | 8 层架构与读写路径 |

## 关键约束

- **Probe / Decode 分层不动摇**：没有 byte-range + fixture + IDA/controlled fixture 证据，不做 `Decoded` promotion。
- **文件化导出先定合同再写代码**：先 review bundle shape，避免实现后反复迁移。
- **raw bytes 可追溯**：bundle 可以选择输出 raw stream bytes，但默认应控制体积和隐私风险。
- **writer 保持 passthrough-first**：只写已证明可写面，不从 geometry/probe JSON 反写 Sheet bytes。
- **publish XML 分线**：MDF publish 是 parallel pipeline，不把 MDF 语义硬塞进 `.pid` raw decode。
- **不继续 broad IDA**：只追 direct stream name / `IOContext::DoIO` / persist-manager clue。

## 非目标

- 不在本 phase 直接实现所有 parser。
- 不把 `0x0010`、GraphicGroup、StyleCluster 等剩余未知字段强行命名。
- 不引入新的 `PidGraphicKind` promotion。
- 不默认输出全部 raw stream bytes。
- 不 commit / push，除非用户明确授权。

## Done Means

1. `docs/plans/2026-06-16-phase32-full-pid-analysis-and-file-export-plan-cn.md` 存在并覆盖全链路。
2. 本目录五件套存在：`brief.md` / `plan.md` / `verification.md` / `blockers.md` / `goal-prompt.md`。
3. `progress.jsonl` 有 `goal_package_created` entry。
4. `task_plan.md` / `findings.md` / `progress.md` 更新 Phase 32 入口。
5. Plannotator gate 通过，或明确记录 gate 未完成原因。
6. 若进入执行阶段，首个代码切片必须是 format atlas / export bundle contract，不是 parser promotion。
