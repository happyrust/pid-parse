# Plan: Phase 20 PSM 0x0010 IDA-confirmed RAD class identity

## 1. Solution Overview

参考 Phase 16 跨 5 IDA instance 反向 `JStyleOverride` 的方法论，把 PSM
`0x0010` 钉到具体的 RAD class。**纯 reverse engineering + 文档**，不
改 src/ 代码，不改 test。

```
[radsrvitem.dll PSM dispatch table]  ← 入口点
   ↓ 找 PSM type code 0x0010 的派发分支
[PSM 0x0010 factory function]  ← 创建 sub-record instance
   ↓ 找 CLSID lookup / VTable
[RAD class CLSID + DLL]  ← AC1: 类身份
   ↓ 找 Read/IO 函数
[Read/IO sequence + sub-kind switch]  ← AC2: sub-kind discriminator
   ↓ 枚举 switch case 值
[Sub-kind value table]  ← AC3: sub-kind 枚举
   ↓ 与 Phase 19 leading_word 数字交叉验证
[Cross-fixture distribution per sub-kind]  ← AC4 §4
   ↓ 落盘 analysis doc
[docs/analysis/2026-05-17-phase20-psm-0x0010-rad-class.md]  ← AC4
```

## 2. Why This Approach（only this approach）

| 候选 | 优点 | 缺点 | 决策 |
|---|---|---|---|
| **A. IDA dispatch-table-first**（推荐）| 与 Phase 16 同 methodology；radsrvitem.dll 已 indexed；CLSID + factory pattern 已被 Phase 16 验证 | 5374 个 function 大部分未命名，需要耐心 | **本 phase 采用** |
| B. Byte-pattern reverse-engineering only | 不依赖 IDA | Phase 19 probe 已证明纯 byte 看不出全局 discriminator | 拒绝 |
| C. Hybrid byte + IDA | 平衡 | 增加复杂度；Phase 16 经验证明 IDA-first 效率更高 | 拒绝 |
| D. Skip Phase 20，直接 Phase 21 typed DTO | 落地快 | 没 IDA 证据就命名字段 = Phase 14 GArc2d 重蹈覆辙 | 拒绝（违反 Phase 18/19 audit-only 原则） |

## 3. How It Will Work

### 3.1 IDA instance roadmap

| Port | Binary | 用途 |
|---:|---|---|
| 13346 | `radsrvitem.dll` | **入口**：PSM type code dispatch 表 |
| 13341 | `sppid.dll` | SmartPlant 业务 class |
| 13347 | `J2DSrv.dll` | 2D Sheet 几何 |
| 13348 | `style.dll` | Phase 16 已 navigated（可能含 0x0010 sibling） |
| 13340 | `sppidautomation.dll` | SmartPlant 自动化 |
| 13342 | `smartplantpid.exe` | 主程序 |
| 13343 | `sppiddwgprocess.dll` | DWG export |
| 13344 | `sppidautomation.exe` | 自动化主程序 |
| 13345 | `llama.dll` | （可能 unrelated） |
| 13339 | `ipidobjectmanagerinf.dll` | object manager |
| 13338 | `sppidautomationwrap.dll` | 自动化 wrapper |
| 13337 | `core.dll` (D:\\AVEVA\\) | **可能** unrelated（AVEVA E3D，不是 SmartPlant）|

### 3.2 调查步骤

**Step 1 — radsrvitem.dll PSM dispatch table**：
- `search_text` 找 `"0x10"`, `"0x0010"`, `0x10` literal 在 .text 段
- 查 IDA 是否有 PSM dispatch table 的 cross-reference 模式
  （Phase 16 的 `JStyleOverride` 入口是 `style.dll!sub_1000F030`，
  从 `radsrvitem.dll` factory 拓扑反向找到）
- 找到 0x0010 的 case branch，记录 factory function 地址

**Step 2 — Factory function → CLSID + DLL**：
- 反编译 0x0010 factory function
- 寻找 CLSID lookup（`CoCreateInstance` / `CLSIDFromString` /
  CLSID literal 模式）
- 跨 DLL 跳转到目标 class 所在 DLL（用 `select_instance`）

**Step 3 — Class Read/IO sequence**：
- 在目标 DLL 里找 class 的 `Read` / `Load` / `IO` / `DoIO` 函数
- 反编译 Read 函数，列出 `IOContext::DoIO` 序列（按 byte offset
  + 字段类型）

**Step 4 — Sub-kind discriminator**：
- 在 Read 函数里找 switch / if-else 分支
- 识别 discriminator 字段的偏移 + 数据类型
- 枚举 switch case 值

**Step 5 — Cross-fixture validation**：
- 用 Phase 19 probe 结果反向验证：
  - leading_word == 0x0002 应该映射到某个 sub-kind
  - size 31 bucket (182 records) 应该映射到另一个 sub-kind
  - 验证数量分布与 IDA-derived sub-kind enumeration 一致

**Step 6 — Analysis doc**：
- 把所有发现写入
  `docs/analysis/2026-05-17-phase20-psm-0x0010-rad-class.md`
- 结构 mirror Phase 16
  `jstyleoverride-v3-fields.md`

**Step 7 — Gates**：
- 跑 5 道 pre-commit gate 确认无意外退化

## 4. Slices

| Slice | Purpose | IDA instances | Done when | Risks |
|---|---|---|---|---|
| A | `radsrvitem.dll` dispatch-table recon: 找 PSM 0x0010 分发点 | 13346 | 找到至少 1 个 cross-reference 指向 0x0010 factory，写入 progress.jsonl | dispatch 是 indirect call table，需要 trace runtime / vtable |
| B | Factory function reverse + CLSID lookup | 13346 + 目标 DLL | 拿到 CLSID + factory address | factory 可能 inline 在大函数里 |
| C | 切到目标 DLL，找 class Read/Load 函数 | 目标 DLL | 反编译 Read 函数，识别 IO sequence | class 可能用 RTTI / multiple inheritance，IO 分散 |
| D | Sub-kind discriminator 识别 + 枚举 | 目标 DLL | 拿到 discriminator offset + 至少 3 个 sub-kind 值 | sub-kind 可能不是单一 switch，而是多态 vtable |
| E | Cross-fixture validation（Phase 19 leading_word vs IDA sub-kind） | — | 数字交叉对得上（至少 0x0002 = 164 records 对应某个 sub-kind） | IDA-derived discriminator offset ≠ leading_word 位置时需要重新分桶 |
| F | Analysis doc (`docs/analysis/2026-05-17-phase20-psm-0x0010-rad-class.md`) | — | 8 节全部写完 + IDA 地址索引完整 | 文档篇幅大；尽量保持 ≤ 300 行 |
| G | 5 道 pre-commit gate + progress.jsonl goal_complete | — | gates 全绿 + AC1-AC7 evidence | 无（不改代码） |

## 5. Sequencing

A → B → C → D → E → F → G 顺序执行。

**重要**：如果 Slice A 找不到 dispatch table 入口（5374 个 function
里 0x0010 literal 出现位置太多），立即写 `progress.jsonl` blocker，
切换到 strings-based search（找 "0010" / "Type0x10" 等 marker）。

## 6. Phase Boundaries

最低可交付：
- AC1-AC7 全部达成
- 5 道 gate 绿（不改代码，应该自动绿）
- Phase 14/15/16/17/18/19 baseline 全部保持

不在本 phase 做：
- typed sub-kind DTO 实现（推迟 Phase 21）
- reference resolver
- 其它 PSM type code 反向
- src/ 任何代码改动

## 7. Steering Notes

- **节奏**：Phase 16 单 type code 反向用了多个 session；Phase 20 是
  polymorphic family，预期更长。如果 Slice C 反编译 > 1 小时仍未
  identify Read 函数，写 blocker 暂停。
- **不要顺手解决** Phase 18 `bytes_to_follow = 0` 退化、Phase 19
  heterogeneous size 31 bucket 等议题；本 phase 专注 IDA class
  identity + sub-kind discriminator。
- **优先用 `search_text` / `survey_binary`**，避免 `py_exec_file`
  跑 IDA Python 长脚本（可能 timeout）。
- **每个 IDA tool call 后** 写一条 progress.jsonl entry，方便回看。

## 8. Acceptance Criteria

- [ ] **AC1**：PSM 0x0010 真实 RAD class 已 identify：class 名 +
      CLSID + DLL + factory function 地址（IDA 端口 + 函数偏移）。
- [ ] **AC2**：sub-kind discriminator 字段已 identify：字节偏移 +
      数据类型（u8 / u16 / u32 / u64）+ 反编译伪代码片段（≤ 30 行）。
- [ ] **AC3**：至少 3 个 sub-kind 枚举值 + 每个 sub-kind 在 Phase 18
      audit collection 里的 record count（用 leading_word 或 size
      bucket 反向匹配）。
- [ ] **AC4**：`docs/analysis/2026-05-17-phase20-psm-0x0010-rad-class.md`
      8 节全部写完，结构 mirror Phase 16
      `jstyleoverride-v3-fields.md`。
- [ ] **AC5**：5 道 pre-commit gate 全绿（不改代码应该自动绿）。
- [ ] **AC6**：`progress.jsonl` 每个 IDA 调查动作都有 entry。
- [ ] **AC7**：所有 IDA 调查只用现有 12 个 reachable instance（不
      装载新 instance）。

## 9. Required Evidence

| Requirement | Evidence to inspect | Where recorded |
|---|---|---|
| AC1 | analysis doc §1 + progress.jsonl factory function 地址 | analysis doc + progress.jsonl |
| AC2 | analysis doc §3 + progress.jsonl Read 函数反编译片段 | analysis doc + progress.jsonl |
| AC3 | analysis doc §4 cross-fixture distribution 表 | analysis doc |
| AC4 | analysis doc 8 节完整 | analysis doc |
| AC5 | 5 道 gate 输出 | progress.jsonl |
| AC6 | progress.jsonl 本身 | progress.jsonl |
| AC7 | IDA `list_instances` 输出对比初始 12 instance | progress.jsonl |

## 10. Completion Audit

声明完成前逐项对照 AC1-AC7。任何 typed sub-kind DTO 落地（src/ 代码
改动 / 新 schema needle / 新 ratchet test）都必须**回退**——本 phase
是纯 IDA + 文档，不改代码。Typed DTO 是 Phase 21 工作。

如 IDA reverse engineering 在 Slice C/D 卡住超过 2 个 session 仍未
identify Read 函数，写 progress.jsonl `[blocker]` entry 等用户确认
是否：
- 切换 IDA instance / 加载新 instance（需用户授权）
- 接受 partial AC（identified RAD class 但 sub-kind discriminator
  推迟 Phase 21）
- 完全 abort，留 0x0010 audit-only
