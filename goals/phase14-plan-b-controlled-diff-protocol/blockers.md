# 阻塞清单：Phase 14 Plan B 控制 diff fixture 采集协议

> **[DRAFT — awaiting plannotator gate]**

## 当前 KNOWN 阻塞

### B1（**本 goal 无关 Phase 14 B1**）

本 goal **不需要**等 Phase 14 主 goal 的 B1（rad2d.dll 入仓）解
锁。本 goal 是 Phase 14 主 goal **的备选证据链**，独立可执行。

### B2 — plannotator 浏览器面板不在线 [当前实时 BLOCKER]

| 项 | 内容 |
|---|---|
| **现状** | 本 goal package 5 份文档 (`brief.md` / `plan.md` / `verification.md` / `blockers.md` / `goal-prompt.md`) 都标 `[DRAFT — awaiting plannotator gate]`。Agent 已尝试 2 次 plannotator gate，浏览器面板均超时 15+/2+ 分钟无响应 |
| **证据** | 本会话 shell 任务 164346 + 848961 都被 kill |
| **解锁动作** | 用户打开 plannotator 浏览器面板 → 按文档显示 → approve 或写 review |
| **owner** | 用户 |
| **next action** | 用户回浏览器后，agent 重跑 `plannotator annotate <doc> --gate` 五次 |
| **解锁判据** | 五道 gate 全部 `The user approved.` 返回 |

未解 B2 前本 goal 文档全部停在 DRAFT 状态，不能驱动 Codex `/goal`
启动。

## Open Questions

### Q1 — 协议是否需要支持非 SmartPlant 12.x

| 项 | 内容 |
|---|---|
| **背景** | DWG fixture 用的是 Oracle 12c，对应 SmartPlant 12.x。但 TEST02 是 SQL Server (likely 较旧版本) |
| **决定影响** | 协议是单版本还是跨版本 |
| **解法** | Slice 3 内决定。建议先锁 SmartPlant 12.x；其他版本后续 goal |
| **owner** | agent（Slice 3 内回答） |

### Q2 — 协议是否包含 "如何安装 SmartPlant P&ID"

| 项 | 内容 |
|---|---|
| **背景** | 用户已有 license 才能采集 fixture。但协议是否引导工作站初始化？ |
| **决定影响** | 协议体量 + 隐含 license assumption |
| **解法** | Slice 3 内：协议**不**做 SmartPlant 安装指南，假设操作员已会用 |
| **owner** | agent（Slice 3 内回答） |

### Q3 — 协议要不要覆盖 6 类还是只 2 类

| 项 | 内容 |
|---|---|
| **背景** | `brief.md` 目标说"6 case 最理想 / 2 case 最小可接受" |
| **决定影响** | 协议完整度 + 写作时间 |
| **解法** | Slice 2 内：先写完 6 类骨架，2 类做完整示例，4 类标 TODO 后续补 |
| **owner** | agent（Slice 2 内回答） |

## Stop And Ask

1. **B2 超过 3 天未解锁**：用户不在线 → 是否切到 "skip-gate
   DRAFT-commit" 模式？需用户授权
2. **Slice 3 写协议时发现 SmartPlant UI 不支持某类原子操作**（例如
   不能单独"放 polyline"必须放 2+ 段）：协议覆盖 5 类还是仍尝试
   covered 6 类用变通方法？
3. **Slice 5 合成 fixture 自检测试反复 fail**：协议的合成数据假设
   与真实 SmartPlant 输出不匹配？停手让用户提供 ≥ 1 个真实 case 做
   对照
4. **`ControlledDiffMetadata` schema 想增加新字段**：影响下游
   `inspect::controlled_diff` 库 API，必须用户签
5. **协议建议提交 plant 真实 fixture 进 git**：plant 数据敏感，绝
   对不可未授权 commit；停手 ask
6. **跨 SmartPlant 版本字节兼容性出现矛盾**：例如 12.0 和 12.1 同
   操作生成不同 Sheet bytes：是否限制协议到单一 minor version？
7. **CI flaky / failing 超过 3 次连续**：怀疑 main 上别的改动冲突；
   暂停 goal

## 高风险 / 不可逆动作（必须先得授权）

- **commit 任何 `.pid` 真实 fixture 进 git**：plant 数据敏感，必须
  操作员 + 用户双方明确签
- **改写 `ControlledDiffMetadata` schema**：影响下游消费者
- **跨 SmartPlant 版本测试**：协议默认 12.x；扩展需明确签
- **修改 inspect::controlled_diff 库 API**：上 goal 已经 commit 的
  类型不变式（`promoted_geometry = false` 硬编码）必须保持

## 当前已知 blocker 状态总表

| ID | 类型 | 状态 | next action | owner |
|---|---|---|---|---|
| B1 | not-applicable | N/A | - | 本 goal 无关 |
| B2 | hard (实时) | OPEN | 用户打 plannotator 浏览器面板 | 用户 |
| Q1 | question | DEFERRED | Slice 3 | agent |
| Q2 | question | DEFERRED | Slice 3 | agent |
| Q3 | question | DEFERRED | Slice 2 | agent |
