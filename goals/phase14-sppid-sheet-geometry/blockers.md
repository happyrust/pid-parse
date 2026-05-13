# 阻塞清单：Phase 14 SPPID Sheet 几何 primitive 解码器

## 当前 KNOWN 阻塞（开 goal 时必须先解的）

### B1 — rad2d.dll / pidobjectmanager.dll 入仓 [BLOCKER]

| 项 | 内容 |
|---|---|
| **现状** | 8 个已加载 IDA binary 都被证实是上层 COM 调度层，**不是** Sheet 字节解析器 |
| **证据** | `docs/analysis/2026-05-13-ida-pro-mcp-reconnaissance.md` |
| **解锁动作** | 用户从 SmartPlant P&ID 工作站安装目录复制 `rad2d.dll` + `pidobjectmanager.dll`（以及 `sigma2d.dll` / `igrgcdt.dll` 等同目录依赖）到 `E:\weixin\xwechat_files\happydpc_b2ec\msg\file\2026-05\bin` 或直接 `dlls/` |
| **owner** | 用户 |
| **next action** | 用户通知"DLL 入仓完成"，agent 拷贝 + 开 IDA 实例 + `list_instances` 验证 |
| **解锁判据** | `CallMcpTool user-ida-pro-mcp list_instances` 包含 `rad2d.dll.i64` 一个 reachable=true 实例 |

**未解锁前 goal 0 进度**。每天 daily check 一次。超过 7 天未解锁触发
stop-and-ask（见下 §"Stop And Ask"）。

## Open Questions

### Q1 — RAD2D Sheet 结构是 RAD2D 原生还是 SPPID 自定义？

| 项 | 内容 |
|---|---|
| **背景** | `Interop.RAD2D.dll` 是 .NET interop wrapper；真正的 RAD2D 实现可能是 Intergraph 通用 2D CAD 引擎（多个产品共用），也可能是 SPPID 在 RAD2D 上的扩展 |
| **决定影响** | 决定 IDA 反向工作量 —— 通用 RAD2D 有公开 API 文档（Plant Design Web），SPPID 扩展只能从 binary 推断 |
| **解法** | Slice A 拿到 DLL 后，先用 `survey_binary` 看 export 列表 / 字符串密度，5 分钟可初判 |
| **owner** | agent（Slice A 内回答） |

### Q2 — Sheet 流的 chunk 内是否有 endian / 版本号字段？

| 项 | 内容 |
|---|---|
| **背景** | 现行 `sheet_probe.rs` 看到 `0x89` marker 但没看到稳定的 version field。如果 RAD2D 内部用 endian-mark 来 disambiguate，那现行 marker 解释可能有偏差 |
| **决定影响** | 影响 Slice C 字节布局表的稳定性 |
| **解法** | Slice C 内交叉验证 3 条 record；不一致就回 Slice B 加 dispatcher 二次分析 |
| **owner** | agent（Slice C 内回答） |

### Q3 — Coverage 升级是否需要先动 `IdentifiedOnly → PartiallyDecoded` 的 ratchet？

| 项 | 内容 |
|---|---|
| **背景** | `Sheet*` coverage 当前是 `IdentifiedOnly`。Phase 14 任务表（task 14-05）明说升级是 audit gate |
| **决定影响** | 升级影响下游 inspect 报告语义 |
| **解法** | Slice E 内 ASK BEFORE：在升级前给用户看 decoded fixture inventory，让用户判断升级到 `PartiallyDecoded` 还是停在 `IdentifiedOnly` 但补 supported categories 子分类 |
| **owner** | 用户（Slice E 内决策） |

## Stop And Ask（必须暂停去问用户的条件）

任一条件成立立即停手、写 `progress.jsonl` 解释，等用户回复：

1. **B1 入仓超过 7 天未解锁**：用户没拿到 DLL；问是否换 Plan B
   （`docs/plans/2026-05-09-controlled-diff-evidence-report-plan.md`）
2. **Slice C 字节布局推断在 3 个 fixture 上不一致**：可能 DLL 与
   fixture 版本错配；问是否需要先用 SmartPlant 自己重存 fixture
3. **decoded line 数量 ≥ 1 但 < 5 时**：`Decoded` confidence 是否足
   够强？询问用户是否升级 Coverage 分级或继续保持 `Inferred` 退路
4. **Coverage 分级升级请求**（Q3）：升级 `Sheet*` 从 `IdentifiedOnly`
   到 `PartiallyDecoded`，audit gate 必须用户确认
5. **`SheetRecordKind` enum 需要加新 variant**：`pub` schema，下游
   契约，加变体前必须用户签
6. **CI flaky / failing 超过 3 次连续**：怀疑 main 上别的改动冲突；
   暂停 goal，拉用户排查
7. **fixture 不足以判断**（registry 还在 5/8+，且 decoded line 在
   仅 1 个 fixture 上出现）：问用户是否需要补 fixture 才能签收

## 高风险 / 不可逆动作（必须先得授权）

- **任何对 `main` 分支的 push 或 force-push**：必须用户明确说 `push`。
  本 goal 期间不接受 implicit `继续` 作为 push 授权（已学到的教训）
- **commit 任何二进制 DLL 进 git**：dlls/ 53.5 MB 严禁入 git；如确
  要分享，走 git-lfs 并明确确认
- **修改 `pid_parse::inspect::controlled_diff::ControlledDiffEvidenceReport.promoted_geometry`
  字段**：硬编码 false 是 phase 14 type-system invariant，改了就破
  防晋升保险
- **改写 `.i64` IDA 数据库**：必须先把分析结论 export 成 docs/analysis/
  下的文档；`.i64` 是工具 state，会随版本丢失
- **引入新主 crate 依赖**：尤其涉及 GPL-3.0 / LGPL 的 crate，影响
  本仓 license（主 crate MIT/Apache-2.0；vendored oxidized-mdf 是
  GPL-3.0 隔离在 `vendor/`）

## 当前已知 blocker 状态总表

| ID | 类型 | 状态 | next action | owner |
|---|---|---|---|---|
| B1 | hard | OPEN | 用户提供 DLL | 用户 |
| Q1 | question | DEFERRED | Slice A 内回答 | agent |
| Q2 | question | DEFERRED | Slice C 内回答 | agent |
| Q3 | question | DEFERRED | Slice E 内 ask | 用户 |

**所有 hard blocker 解锁前，本 goal 进度记 0。**
