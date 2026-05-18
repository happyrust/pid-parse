# Blockers: Phase 20 PSM 0x0010 IDA-confirmed RAD class identity

## Open Questions

### Q1 — 切换 IDA instance 的顺序 [OPEN]

候选切换路径：

- `radsrvitem.dll` → CLSID lookup → 目标 DLL（**推荐**，与 Phase 16
  方法论一致）
- `style.dll` 先看（Phase 16 已 navigated，可能含 0x0010 sibling
  class）
- `J2DSrv.dll` 先看（2D Sheet 几何记录，可能直接含 0x0010）

执行时按推荐路径，若 radsrvitem.dll dispatch table 难找，再切到
style.dll/J2DSrv.dll。

### Q2 — Analysis doc 是否包含完整反汇编 [OPEN]

候选：

- 只保留反编译伪代码 + 关键 byte 模式（**推荐**，文档紧凑）
- 同时保留反汇编 + 反编译（更全面，但文档膨胀）

执行时按推荐 only-pseudocode + key bytes。

### Q3 — 若 IDA 反编译伪代码与 Phase 19 leading_word 数字对不上 [DEFERRED]

可能原因：
- discriminator 偏移不是 `+0..+1`
- discriminator 是 bit-packed（如 type_flags 用了高 2 bit）
- 不同 sub-kind 走不同 vtable，无单一 discriminator 字段

**决定**：写 progress.jsonl `[discrepancy]` entry，accepting partial
AC3（即使 sub-kind enumeration 不完整，仍认为 AC1/AC2 达成即可）。

### Q4 — 如发现 0x0010 真实是嵌入 fragment 而非独立 record [DEFERRED]

Phase 14 §6.3 + AGENTS.md 都说 0x0010 是 "embedded sub-records /
attribute fragments inside other record types"。如果 IDA 证据强烈
表明它是 fragment（无独立 Read 函数，只在其他 record Read 函数里被
解码），则：

- AC1 改为 "0x0010 是某个 host class 的内嵌 fragment + host class
  identity"
- AC2 改为 "fragment discriminator 在 host class 的解码逻辑里"
- analysis doc §1 标题改为 "embedded fragment identity"

**决定**：本 phase 容忍这种结论；写 progress.jsonl `[discovery]`
entry，调整 AC 描述。

### Q5 — concrete class name / Read-DoIO 未恢复 [PARTIAL-AC ACCEPTED]

Phase 20 Slice B 已确认 `0x0010` 的 persisted GUID / type-table identity：

- `entry[0x0010] @ radsrvitem.dll .data:0x5667B1A8`
- GUID `1D1928C0-0000-0000-C000-000000000046`
- `tail16=0x40`, `tail17=0x06`, `parent=0x0115`
- `entry[0x0115]` 复用同一 GUID，作为 root alias

同时已形成负证据：

- `style.dll!DllGetClassObject` 没有该 GUID 的 direct CLSID branch；
- `style.dll .rdata:0x10068F44` 只有 raw GUID hit，无 static xref；
- `sub_5647CE40` / `sub_5647CA50` 是 default `E_NOTIMPL` stub；
- `sub_56468B30` 路径是 packed OID → existing record slot →
  SerialCluster lazy-load，不是普通 COM class factory。

**决定**：接受 partial AC。Phase 20 当前收口为：

- AC1 partial pass：GUID + persisted type-table identity confirmed；
  human persisted type name / concrete class name deferred。
- AC2/AC3 deferred：Read/DoIO 与 sub-kind discriminator 未恢复，禁止
  命名 sub_kind 或实现 typed DTO。
- AC4 partial pass：analysis doc 已创建并明确 partial 状态。

后续若要继续恢复 human type name，应另开小 phase，优先查外部 metadata /
RTTI / local types，而不是继续 blind factory tracing。

## Stop And Ask

任一条件成立立即停手，写 `progress.jsonl`，等用户回复：

1. Phase 14 / 15 / 16 / 17 / 18 / 19 任一 baseline 退化（5 道 gate
   失败原因不是简单错误）。
2. 出现要装载新 IDA instance 的诱因（当前 12 个 reachable 不够用）。
3. 出现要修改 src/ 代码的诱因（本 phase 是纯 reverse engineering + 文档）。
4. 出现要落地 typed sub-kind DTO 的诱因（属于 Phase 21）。
5. 出现要实现 reference resolver 的诱因（属于 Phase 21+）。
6. IDA `analyze_function` / `py_exec_file` 超时 ≥ 3 次（说明 IDA
   instance 不稳定，需要重启）。
7. 反向工程在单个 IDA function 上耗时 > 1 小时仍未识别关键字段，
   切换 strategy（搜 string / 跨 DLL）或换函数。
8. `missing_docs` ratchet 上升（current > baseline=0）。

## Dangerous Or High-Risk Actions

必须先获得用户授权：

- 装载新 IDA instance（当前 12 个不够用时）
- 修改任何 src/ 代码
- 修改任何 test 文件
- 修改 Phase 14-19 任何 stable DTO 既有字段
- 提交 `dlls/`、`.i64`、私有 fixture
- 把 Phase 20 scope 扩到 typed DTO 实现、reference resolver、
  其它 PSM type code
- 在 analysis 文档中包含大块连续反汇编（> 100 行）

## Known Blockers

| ID | 类型 | 状态 | next action | owner |
|---|---|---|---|---|
| B1 | scope | RESOLVED BY DEFAULT | IDA reverse engineering scope adopted; 12 instances reachable | user + agent |
| Q1 | question | OPEN | 执行时按 radsrvitem.dll-first 推荐路径 | agent |
| Q2 | question | OPEN | 执行时按 pseudocode-only 推荐 | agent |
| Q3 | question | DEFERRED | partial AC3 accepted if discriminator 与 leading_word 对不上 | agent |
| Q4 | question | DEFERRED | 容忍 "embedded fragment" 结论，调整 AC 描述 | agent |
| Q5 | partial-ac | ACCEPTED | GUID/table identity 收口；class name / Read-DoIO / sub-kind deferred | user + agent |
| Q6 | metadata-recon | ACCEPTED NEGATIVE | external metadata / RTTI / local type / registry search did not recover human type name | agent |
| Q7 | readonly-tracing | ACCEPTED PARTIAL | SerialCluster traced to storage accessors; style IJPersist DoIO recovered for JStyleBase control GUID, not 1D1928C0 | agent |

## 当前状态总表

IDA-confirmed reverse engineering 路径 + Phase 16 同款方法论 = 中等
风险（IDA 反向工程本身不确定性高）。B1 默认解锁；Q1-Q4 不阻塞 Slice
启动。

整体工作量预估：**Phase 16 单 type code 反向用了多个 session**，
Phase 20 是 polymorphic family，预期更长（2-5 sessions）。建议
按 Slice A → B → C → D 分别 checkpoint 等用户 sign-off，避免一次
session 内吞掉太多上下文。

不改 src/ 代码 = 5 道 gate 不会因本 phase 而失败（除非 Phase 19
commit 有意外副作用）。
