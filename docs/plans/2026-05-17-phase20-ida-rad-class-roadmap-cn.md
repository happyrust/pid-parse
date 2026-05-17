# Phase 20 路线图：PSM 0x0010 IDA 反向 + RAD 类身份与 sub-kind discriminator

> 编写：2026-05-17
> 作者：Cursor 代理
> 受众：Codex / Cursor 后续会话、人类 reviewer
> 工作目录：`d:\work\plant-code\cad\pid-parse`

## 0. TL;DR

Phase 18 把 PSM `0x0010` sub-record family 落地为 audit-only 集合
（582 records），Phase 19 在其上加了一个 audit-only `leading_word:
Option<u16>` 字段并被 cross-fixture ratchet 锁定。两个 phase 都已
commit + push（`81daa20` / `6beb6f1`）。

下一阶段 **Phase 20** 的核心目标是**用 IDA-confirmed evidence 把
`0x0010` 钉到具体 RAD class**，回答三个问题：

1. PSM `0x0010` 真实属于哪个 RAD 类（CLSID + DLL + factory function
   地址）？
2. sub-kind discriminator 在哪个字节偏移、什么数据类型？
3. 共有多少种 sub-kind？每种 sub-kind 对应 Phase 18 audit collection
   里多少条 record？

**Phase 20 是纯 reverse engineering + 文档**，不改 `src/` 代码、不改
`tests/`。Typed sub-kind DTO 实现是 Phase 21 工作。

12 个 IDA instance 已 reachable（含 `radsrvitem.dll` / `style.dll` /
`J2DSrv.dll`），不需要装载新 instance。预期工作量 **2-5 session**，
本计划文档把 Phase 20 拆为 7 个 Slice（A-G）并给出每个 Slice 的
checkpoint 与 stop-and-ask 条件，避免单 session 吞掉太多上下文。

## 1. 起点：Phase 18/19 已完成什么

### 1.1 Phase 18 落盘（commit `81daa20`）

- `SheetSubRecord0x0010Decoded` parser DTO：6-byte PSM header +
  `raw_payload: Vec<u8>`，audit-only，无 sub-kind 字段命名。
- `DecodedSubRecord0x0010Record` model DTO mirror + `From` impl +
  `JsonSchema` derive + `SheetGeometry::decoded_sub_records_0x0010`
  字段。
- `decode_sub_records_0x0010` advancing scan decoder：跨 4 fixture
  共 **582** records（DWG-0201=161 / DWG-0202=104 / 工艺管道-1=306 /
  A01=11）。
- 12 个 parser unit test + 1 cross-fixture ratchet test。

### 1.2 Phase 19 落盘（commit `6beb6f1`）

- `SheetSubRecord0x0010Decoded.leading_word: Option<u16>` 字段：
  `payload[0..2]` little-endian u16。**字段名描述字节位置**，不
  描述语义（避免重蹈 Phase 14 GArc2d 错误命名）。
- 新 ratchet `sub_records_0x0010_leading_word_distribution_matches_phase19_probe`：
  - `leading_word == 0x0002` count = **164 (28.2%)**
  - `leading_word == 0x0003` count = **21 (3.6%)**
  - `leading_word == 0x0001` count = **18 (3.1%)**
  - `None` count = 0
  - total = 582（与 Phase 18 完全一致）
- 2 个 probe binary：
  - `examples/probe_rad_siblings_0x0029_0x0035.rs`：证伪 "CLSID 段
    47FCC330..47FCC33E 一一映射到 PSM 0x29..0x35" 的假设。
  - `examples/probe_psm_0x0010_sub_kind.rs`：per-size-bucket
    discriminator 直方图。
- 1 个 evidence breadcrumb：
  `docs/analysis/2026-05-17-phase19-rad-sibling-probe-null-result.md`。

### 1.3 Phase 18/19 留下的核心未解问题

| 问题 | 当前状态 | Phase 20 要做的 |
|---|---|---|
| 0x0010 真实 RAD class 是什么？ | 仅推测为 `radsrvitem.dll` 家族 | IDA dispatch table → CLSID → DLL → factory address |
| sub-kind discriminator 在哪？ | `+0..+1` 部分覆盖 28%，size 31/70/13/16/43 异质 | 在目标 class Read/IO 函数里识别真实 discriminator 字段（可能不在 `+0`） |
| 共有多少 sub-kind？ | probe 见 ≥ 10 种 size bucket，但不等于 sub-kind 数 | Read 函数 switch / vtable 多态分支枚举 |
| `leading_word == 0x0002` (28%) 语义？ | 仅是 byte 位置统计 | IDA 证据要解释为某个具体 sub-kind tag |
| size 31 bucket (182 records) 是什么？ | `+0` 异质，可能含坐标 / OID | IDA 反编译 Read 函数确认 |

## 2. Phase 20 高层路线

```
   Phase 18/19 audit baseline (582 records, leading_word audit)
                        │
                        ▼
       Slice A: radsrvitem.dll dispatch table recon
                        │  (找 0x0010 派发分支)
                        ▼
       Slice B: factory function → CLSID + 目标 DLL
                        │  (跨 IDA instance 跳转)
                        ▼
       Slice C: 目标 class Read/IO 函数 + IO sequence
                        │  (字节偏移 + 类型表)
                        ▼
       Slice D: sub-kind discriminator 偏移 + 枚举值
                        │  (switch / vtable 反向)
                        ▼
       Slice E: cross-fixture validation
                        │  (Phase 19 leading_word 对得上)
                        ▼
       Slice F: authoritative analysis doc (8 节)
                        │
                        ▼
       Slice G: 5 道 pre-commit gate + goal_complete
                        │
                        ▼
       Phase 21 入口：typed sub-kind DTO + reference resolver
```

## 3. Slice 详细分解

### Slice A：`radsrvitem.dll` dispatch table 侦察

**目标**：在 5374 个 function（4867 个未命名）里找到处理 PSM type
code 0x0010 的派发分支或 factory function。

**输入**：
- IDA instance `radsrvitem.dll` (port 13346)
- Phase 16 `JStyleOverride` 反向方法论：从 `radsrvitem.dll` factory
  → CLSID lookup → 跨 DLL 跳转

**步骤**：
1. `select_instance(13346)`
2. `survey_binary` 已完成（progress.jsonl 已记录）：5374 functions,
   346 named, 1739 strings, exports `GetServerItemTransceiver` /
   `GetServerItemVersion`
3. `search_text` 找 `.text` 段里 literal `0x10` / `0x0010` 的出现
4. 检查每个 hit 是否在 dispatch-shaped function 里：
   - 大 switch / jump table
   - 多个 CLSID literal
   - `CoCreateInstance` 调用
5. 候选 factory function 写入 `progress.jsonl`

**checkpoint**：找到至少 1 个候选 factory function 地址，附 ≤ 50 行
反编译伪代码片段。

**Stop-And-Ask**：
- 4867 个未命名 function 里 0x10 出现位置太多无法收敛 → 切到 Slice
  A-alt：strings-based search（找 "0010" / "Type0x10" / "SubRecord"
  等 marker）
- IDA `analyze_function` 在大函数上超时 ≥ 3 次 → 写 blocker

### Slice B：factory function → CLSID + 目标 DLL

**目标**：从 factory function 拓扑反向，识别 PSM 0x0010 对应的 RAD
class CLSID 与所在 DLL。

**输入**：Slice A 找到的 factory function 地址。

**步骤**：
1. `analyze_function(factory_addr)` 反编译
2. 寻找 CLSID lookup 模式：
   - 直接 CLSID literal（GUID 字节序列）
   - `CLSIDFromString` / `CLSIDFromProgID` 调用
   - 间接 vtable lookup
3. 如果找到 CLSID，用 IDA `search_text` 在其它 instance（按 plan.md
   roadmap 的 IDA instance 表）找匹配 CLSID
4. 跨 instance 跳转：`select_instance(<目标 port>)`
5. 在目标 DLL 里 confirm class 定义（vtable 起点、构造函数）

**checkpoint**：拿到 CLSID（GUID 字符串）+ 目标 DLL 名 + class
构造函数地址。

**Stop-And-Ask**：
- factory function 用 indirect call / 多重 dispatch，CLSID 不易
  identify → 切到 Slice B-alt：从 `style.dll` 反查（Phase 16 经验
  表明 SmartPlant 大量 sub-record 在 `style.dll` 家族）
- CLSID 指向当前未 reachable 的 DLL → 写 blocker 等用户授权

### Slice C：目标 class Read/IO 函数 + IO sequence

**目标**：在目标 DLL 里找 class 的 `Read` / `Load` / `IO` / `DoIO`
函数，列出按字节偏移 + 字段类型的 IO sequence（对应 Phase 16
`style.dll!sub_1000F030` 的 13 个 `IOContext::DoIO` 调用）。

**输入**：Slice B 得到的目标 DLL + class 构造函数地址。

**步骤**：
1. `select_instance(<目标 port>)`
2. 从 class 构造函数顺 vtable 找 Read/Load/IO 函数
3. 反编译，列出 `IOContext::DoIO` / `Read` / `>>` 调用序列
4. 把每个调用映射到字节偏移 + 数据类型
5. 与 Phase 18/19 probe 看到的实际 payload bytes 交叉验证（用
   `raw_payload` sample）

**checkpoint**：拿到 IO sequence 表，至少覆盖 Phase 18 看到的最小
payload 13 字节。

**Stop-And-Ask**：
- Read 函数分散在多个虚函数 / RTTI 多继承 → 写 blocker
- IO sequence 长度与 probe 实际看到的 size bucket 对不上（如 IO
  sequence 说 64 字节但 probe 见 13 字节） → 可能找错 class，回 Slice B

### Slice D：sub-kind discriminator 偏移 + 枚举

**目标**：在 Read 函数（或其调用链里）识别 sub-kind discriminator
字段。

**输入**：Slice C 的 IO sequence 表。

**步骤**：
1. 在 Read 函数里找 switch / if-else 分支
2. 识别 discriminator 字段的偏移 + 数据类型（u8 / u16 / u32 / u64）
3. 枚举 switch case 值
4. 对每个 case 反编译，记录 sub-kind 名（如果 IDA 给出）+ 该 sub-kind
   的额外字段

**checkpoint**：拿到 discriminator offset + 数据类型 + 至少 3 个
sub-kind 枚举值。

**Stop-And-Ask**：
- 不存在单一 discriminator field（用 vtable 多态完成 dispatch） →
  调整 AC2 为 "vtable-based dispatch + per-class entry points"
- discriminator 是 bit-packed（type_flags 的高 2 bit 用作 sub-kind） →
  扩 AC2 描述 bit layout

### Slice E：cross-fixture validation

**目标**：用 Phase 19 leading_word 数字反向验证 IDA-derived sub-kind
enumeration 与实际 fixture 数据吻合。

**输入**：Slice D 的 discriminator + sub-kind 枚举。

**步骤**：
1. 写一个一次性 probe（或扩展 `probe_psm_0x0010_sub_kind.rs`）按
   IDA-derived discriminator offset 重新分桶 582 records
2. 对每个 sub-kind 值，统计跨 fixture record count
3. 与 Phase 19 数字交叉对比：
   - leading_word=0x0002 (164 records) 应映射到某个具体 sub-kind
   - size=31 bucket (182 records) 应映射到另一个 sub-kind
4. 写一个 reconciliation 表

**checkpoint**：至少 80% 的 record 落到 IDA-derived sub-kind 桶内
（保留 ~20% noise budget）。

**Stop-And-Ask**：
- IDA-derived discriminator 与实际 fixture 数据完全对不上（< 50%
  bucket coverage） → 可能找错 class 或 discriminator 偏移，回 Slice
  B/C 重审
- 出现新 marker pattern（如 Phase 11 发现的 `FA 00`、`CE 00`） →
  写 `[discovery]` entry 但不扩 scope

### Slice F：authoritative analysis 文档

**目标**：把 Slice A-E 的所有发现写入
`docs/analysis/2026-05-17-phase20-psm-0x0010-rad-class.md`，结构 mirror
Phase 16 `2026-05-16-jstyleoverride-v3-fields.md`。

**章节结构**：

1. **Class identity** — class 名 / CLSID / DLL / factory address /
   vtable
2. **IO sequence** — 按 byte offset 列字段类型 + 用途
3. **Sub-kind discriminator** — offset + 数据类型 + 反编译伪代码
   片段 + 枚举值表
4. **Cross-fixture distribution** — 每个 sub-kind 在 4 fixture 的
   record count + 与 Phase 19 leading_word 数字的关系
5. **IDA 地址索引** — 每个引用地址列基址相对偏移 + IDA port
6. **与 Phase 16 JStyleOverride reference chain 的关系** — 是否
   sibling class、是否被 JStyleOverride `+38..41` / `+56..59`
   referenced
7. **Known unknowns** — 仍 audit-only 的字段、未识别的 sub-kind
8. **Phase 21 implementation prerequisites** — typed sub-kind DTO
   字段表草图、迁移路径、对 Phase 18/19 既有 DTO 的影响

**checkpoint**：文档 ≤ 300 行；伪代码 + 关键 byte 模式优先，不放
大块连续反汇编。

### Slice G：5 道 pre-commit gate + goal_complete

**目标**：确认本 phase 没有意外改动 `src/` 代码导致 baseline 退化。

**命令**（本 phase 不改代码，应该自动绿）：

```powershell
cargo build --locked --workspace --all-targets
cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo rustdoc --lib --locked -- -W missing-docs
```

**完成签名**：append goal_complete entry 到 progress.jsonl，包含
`rad_class` / `clsid` / `dll` / `factory_address` /
`sub_kind_discriminator_offset` / `sub_kind_enumeration` 等字段。

## 4. 备选方案与决策矩阵

如果 Phase 20 IDA 反向遇到不可逾越的障碍，下面是备选角度：

| 角度 | 工作量 | 风险 | 何时选 |
|---|---|---|---|
| **20-main（本文档主线）** Phase 20 IDA RAD class | 2-5 session | IDA 反向不确定性 | 默认推荐；IDA 已 reachable，Phase 16 经验可复用 |
| **20-B** JStyleOverride/GraphicGroup → 0x0010 reference resolver | 0.5-1 session | 低 | Slice A/B 反复卡壳后的退路；纯跨记录簿记官，不需 IDA |
| **20-C** size 31 bucket (182 records) 专项反向 | 1-2 session | 中 | Slice D 失败、单一 discriminator 不存在时；用 byte pattern + Phase 19 leading_word filter 缩小搜索空间 |
| **20-D** 多 Sheet* 流未知 type code inventory | 0.5 session | 极低 | 临时切到 inventory 收集更多 unknown 形态，作为 Phase 22+ 的优先级输入 |
| **20-E** 装载新 IDA instance（如 sigma SDK / SmartPlant SDK） | 高（需用户授权） | 高 | 当前 12 instance 不足时才考虑；写 blocker 等用户 |

**决策规则**：

- Slice A 找到 ≥ 1 个候选 factory function → 继续 main 路径
- Slice A 反复 ≥ 3 次仍找不到 → 切 Slice A-alt（strings search）
- Slice A-alt 失败 → 切 20-B 作为后退路径（保住 phase 至少有 partial
  AC 完成）
- 任何 Slice 出现 unauthorized scope 扩张诱因（typed DTO / reference
  resolver / 新 IDA instance） → 立即 Stop-And-Ask

## 5. 多 session checkpoint 策略

Phase 20 是 polymorphic family 反向，预期 2-5 session，所以必须**每
Slice 一个 progress.jsonl checkpoint** + **每 2 Slice 一个跨 session
recap**，避免后续会话 lost context。

### 5.1 单 Slice checkpoint

每完成一个 Slice，append 一条 progress.jsonl entry：

```json
{"type":"slice_X_complete","timestamp":"...","ac":["ACn"],"ida_port":13346,"key_addresses":["0x564XXXXX"],"key_findings":["..."],"next_slice_input":"..."}
```

### 5.2 跨 session recap

每开新 session 前，agent 必须：

1. 读 `goals/phase20-psm-0x0010-ida-class-identity/progress.jsonl` 全文
2. 读 `findings.md` 最近一节
3. 调用 `list_instances` 确认 IDA 仍 reachable
4. 调用 `list_sessions`（在 best-mcp-sqlite 里）或 `load_progress`
   恢复 best-mcp 进度
5. 写 `[session_resume]` entry 摘要：当前已完成 Slice X，下一 Slice
   Y 的输入是什么

### 5.3 Phase 20 终止条件

- AC1-AC7 全部达成 → goal_complete
- Slice B/C 在 2 个 session 里仍找不到 RAD class → 接受 partial AC，
  写 `[partial_complete]` + 切到 20-B 退路
- 5 道 gate 退化（由 Phase 19 之外的意外因素引起） → 立即 abort，
  写 blocker，先修 gate 再决定是否继续 Phase 20

## 6. 风险登记表

| 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|
| `radsrvitem.dll` PSM dispatch 是 indirect call table，难 trace | 高 | 中 | 切 Slice A-alt strings search；用 Phase 16 已知的 style.dll factory 反查 |
| factory function inline 在大函数里（> 1000 行反编译） | 中 | 高 | 用 IDA `analyze_component` 拆解；分段反编译 |
| sub-kind 用 vtable 多态而非单一 discriminator | 高 | 中 | 调整 AC2 描述为 vtable-based dispatch；列每个 vtable entry 而非 switch case |
| IDA instance 反编译超时 | 中 | 中 | 单 function 反编译 ≥ 1 小时 stop and ask；切其它 function 或 strings search |
| 找到的 class 与 0x0010 实际不匹配（IO sequence 长度对不上） | 中 | 高 | 回 Slice B 重审；用 probe payload sample 字节比对 |
| Phase 19 commit `6beb6f1` 在本地未触发的副作用导致 gate 退化 | 低 | 中 | Slice G 跑全 gate 验证；若退化，先修 gate 再继续 |
| 5 个 fixture 不足以验证 sub-kind 枚举完整性 | 中 | 低 | 接受 partial AC3；明确标注哪些 sub-kind 是 IDA-only 证据 |

## 7. 验证命令汇总

### 7.1 IDA 调查命令

```python
# 1. 确认 IDA reachable
list_instances()

# 2. 切到 radsrvitem.dll
select_instance(port=13346)

# 3. 起手 survey（已完成）
survey_binary(detail_level="minimal")

# 4. 搜 0x0010 literal
search_text(pattern="10 00 00 00", segment=".text")  # u32 LE
search_text(pattern="10 00", segment=".text")        # u16 LE

# 5. 反编译候选 factory function
analyze_function(addr=0xXXXXXXXX)

# 6. 追 xref
find_xref_signatures(addr=0xXXXXXXXX)

# 7. 跨 instance 跳转
select_instance(port=<目标 port>)
```

### 7.2 cross-fixture validation

```powershell
cargo run --release --example probe_psm_0x0010_sub_kind  # Phase 19 probe
cargo test --locked -j 4 --test parse_real_files sub_records_0x0010_leading_word_distribution_matches_phase19_probe -- --nocapture
cargo test --locked -j 4 --test parse_real_files sub_records_0x0010_decoder_emits_audit_records_with_provenance -- --nocapture
```

### 7.3 收口 gate

```powershell
cargo build --locked --workspace --all-targets
cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo rustdoc --lib --locked -- -W missing-docs
```

## 8. 启动检查清单

执行 Phase 20 之前确认：

- [ ] Phase 19 已 commit + push (`6beb6f1`)，working tree 干净
- [ ] 12 个 IDA instance 全 reachable（特别是 port 13346 `radsrvitem.dll`）
- [ ] 读 `goals/phase20-psm-0x0010-ida-class-identity/{brief, plan,
      verification, blockers, goal-prompt}.md` 全部
- [ ] 读 `docs/analysis/2026-05-16-jstyleoverride-v3-fields.md`
      （Phase 16 authoritative analysis 模板）
- [ ] 读 `docs/plans/2026-05-16-phase16-jstyleoverride-final-summary.md`
      （Phase 16 跨 5 IDA instance 反向方法论）
- [ ] 读 `docs/analysis/2026-05-15-garc2d-packed-int-tail.md` §11
      （0x0010 reference chain hints）
- [ ] best-mcp-sqlite `load_progress` 恢复跨 session 进度
- [ ] 首个执行动作是 Slice A IDA dispatch table recon，不是直接写
      analysis doc

## 9. 与既有计划文件的关系

| 文件 | 与本计划的关系 |
|---|---|
| `task_plan.md`（项目根目录） | 本计划在 §1 Phase 13-19 列出实际工作；本 phase 加 Phase 20 entry |
| `findings.md`（项目根目录） | 本计划在 §1.2 / §1.3 落盘 Phase 19 关键 finding；本 phase 添加 Phase 20 IDA roadmap 备忘 |
| `progress.md`（项目根目录） | 本计划新增 2026-05-17 session entry，记录 Phase 19 commit + Phase 20 package 与本计划诞生 |
| `goals/phase20-psm-0x0010-ida-class-identity/brief.md` | 简短的 mission 表述（≤ 400 行）；本计划是它的**详细配套路线图** |
| `goals/phase20-psm-0x0010-ida-class-identity/plan.md` | Slice A-G 的紧凑版；本计划是它的**展开版**带 stop-and-ask 与备选方案 |
| `goals/phase20-psm-0x0010-ida-class-identity/blockers.md` | 阻塞条件列表；本计划在 §3 / §6 引用 |
| `goals/phase20-psm-0x0010-ida-class-identity/goal-prompt.md` | `/goal` 启动文案；本计划是被引用的上下文 |
| `goals/phase20-psm-0x0010-ida-class-identity/progress.jsonl` | 执行时的 evidence trail；本计划在 §5.1 / §5.2 定义 entry 模式 |
| `docs/analysis/2026-05-16-jstyleoverride-v3-fields.md` | Phase 16 模板，Slice F 的 analysis doc 必须 mirror 它 |
| `docs/plans/2026-05-16-phase16-jstyleoverride-final-summary.md` | Phase 16 final summary，Slice A-E 方法论的参考 |
| `docs/analysis/2026-05-17-phase19-rad-sibling-probe-null-result.md` | Phase 19 null-result 证据，解释为什么本 phase 不走 RAD sibling sweep |

## 10. 完成定义（Definition of Done）

Phase 20 完成时必须同时满足：

- ✅ AC1：PSM 0x0010 真实 RAD class identified（CLSID + DLL +
  factory address）
- ✅ AC2：sub-kind discriminator 偏移 + 数据类型 identified
- ✅ AC3：至少 3 个 sub-kind 枚举值 + cross-fixture record count
- ✅ AC4：`docs/analysis/2026-05-17-phase20-psm-0x0010-rad-class.md`
  8 节全部写完
- ✅ AC5：5 道 pre-commit gate 全绿（不改代码应自动绿）
- ✅ AC6：`goals/phase20-…/progress.jsonl` 每个 Slice 都有 entry +
  goal_complete
- ✅ AC7：未装载新 IDA instance（保持 12 个 reachable instance）

不在本 phase 做：

- 任何 typed sub-kind DTO 实现（推迟 Phase 21）
- reference resolver
- 任何 `src/` 代码改动
- 任何 test 改动
- 其它 PSM type code 反向

## 11. 后续阶段预告（Phase 21+）

Phase 20 完成后，Phase 21 的 mission 草图：

- **Phase 21A** typed sub-kind DTO：把 0x0010 audit collection 升级
  为 `enum SheetSubRecord0x0010Kind { KindA(KindAFields), KindB(...),
  ... }`，按 Phase 20 IDA 证据命名字段
- **Phase 21B** reference resolver：`JStyleOverride.referenced_oid_a/c`
  + `GraphicGroup.tail_oids` → `SheetSubRecord0x0010Decoded.oid` 的
  lookup（如果 0x0010 record 真的有 oid 字段，Phase 20 §1 应该
  identify 它）
- **Phase 21C** normalized geometry 升级（如果 Phase 20 §1 证明
  0x0010 sub-kind 含 coordinate / text，可能 promote 为
  `PidGraphicKind::Annotation` 的 sibling variant）

但 Phase 21 草图只有 Phase 20 完成后才能落实，本文档不展开。

---

**本路线图编写日期**：2026-05-17
**预期 Phase 20 启动**：用户 `/goal` 授权后下一会话
**预期 Phase 20 完成**：2-5 session 后
**当前 commit baseline**：`6beb6f1` (Phase 19 leading-word audit)
