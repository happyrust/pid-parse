# Phase 20: PSM 0x0010 IDA-confirmed RAD class identity + sub-kind discriminator

## 目标产出

用 IDA-confirmed evidence 回答 Phase 18/19 留下的核心问题：

1. **PSM type code `0x0010` 真实属于哪个 RAD 类？**（CLSID + 所在 DLL +
   factory function 地址）
2. **0x0010 sub-kind discriminator 在哪个字节偏移？** Phase 19 probe
   证明 `+0..+1` 只覆盖 ~36% 记录，size 31 (182 records) / 70 / 13 /
   16 / 43 在 `+0` 异质。剩下的 discriminator 位置必须 IDA-confirmed。
3. **共有多少种 sub-kind？每种 sub-kind 的字段表是什么？**

完成时输出：

- `docs/analysis/2026-05-17-phase20-psm-0x0010-rad-class.md`（权威字段
  表 + IDA 地址索引 + 跨 DLL dispatch chain，参考 Phase 16
  `2026-05-16-jstyleoverride-v3-fields.md` 模板）
- **不**新增任何 typed sub-kind DTO（留给 Phase 21）
- **不**修改 Phase 14-19 任何 baseline
- Phase 19 `leading_word == 0x0002` (164 records) 应被 IDA 证据**解释**
  为某个具体 sub-kind class 的 type tag，但**不**需要 promote 为字段名

## 背景

Phase 16 reverse engineering 历时多 session，最终把 PSM `0x0030` 钉到
RAD `JStyleOverride` 类（`style.dll`，CLSID `{47FCC338-2D0F-11D0-A1FF-
080036A1CF02}`）。完整证据链跨 5 IDA instance（`radsrvitem.dll` →
`J2DSrv.dll` → `JUTIL.dll` → `style.dll`）。

Phase 19 RAD sibling probe 已证伪 "CLSID 段 47FCC330..47FCC33E 与 PSM
0x29..0x35 一一对应" 的假设：`/Sheet6` 上只有 0x0030 有命中，其它 12
个 type code 全 0。所以 0x0010 的 RAD 类**不在** CLSID 段 47FCC330 邻
近，需要在 radsrvitem.dll dispatch 表里查 0x0010 → CLSID 的真实映射。

Phase 19 leading-word probe 进一步确认 0x0010 是 polymorphic family：

- 28% 记录 `leading_word == 0x0002`（跨 ~40 个 size bucket）
- 3.6% `== 0x0003`，3.1% `== 0x0001`
- 但 size 31（182 records，最大 bucket）、70、13、16、43 在 `+0` 异质

这种 "一个 type code + 多种 payload shape" 的形态在 RAD 框架里通常对应：
- 一个 base class（共享 PSM type code）
- 多个 derived class（共享 base class 的 PSM `IO/Read/Write` 入口
  + 不同的字段表）
- discriminator 字段（可能在 base class header 或紧跟其后）

## 上下文（必读）

| 文档 / 文件 | 作用 |
|---|---|
| `docs/analysis/2026-05-16-jstyleoverride-v3-fields.md` | Phase 16 reverse engineering 模板（权威字段表 + IDA 地址索引 + schema 冲突 §11） |
| `docs/plans/2026-05-16-phase16-jstyleoverride-final-summary.md` | Phase 16 final summary（5 IDA instance 跨 DLL chain 方法论） |
| `docs/analysis/2026-05-15-garc2d-packed-int-tail.md` | Phase 16 反向证据章节（含 0x0010 reference chain hints） |
| `docs/analysis/2026-05-17-phase19-rad-sibling-probe-null-result.md` | Phase 19 null-result 证据 |
| `examples/probe_psm_0x0010_sub_kind.rs` | Phase 19 leading-word probe（discriminator 部分覆盖证据） |
| `examples/probe_psm_0x0010_shape.rs` | Phase 18 probe（size bucket 分布） |
| `goals/phase19-psm-0x0010-leading-word-audit/` | Phase 19 完整 goal package |
| `src/parsers/sheet_records.rs::decode_sub_records_0x0010` | 当前 audit-only decoder |
| `src/model.rs::DecodedSubRecord0x0010Record` | 当前 audit-only model DTO |
| IDA instance `radsrvitem.dll` (port 13346, pid 128320) | SmartPlant PSM type code 派发主入口 |
| IDA instance `J2DSrv.dll` (port 13347) | 2D Sheet 几何记录 |
| IDA instance `style.dll` (port 13348) | Phase 16 已 navigated |
| IDA instance `sppid.dll` (port 13341), `sppidautomation.dll` (port 13340), `smartplantpid.exe` (port 13342) | 上层 SmartPlant 应用 |

## 关键约束

- **本 phase 是纯 reverse engineering + 文档**，不改任何 src/ 代码、
  不改任何 test。Phase 21 才会基于本 phase 的 IDA 证据落地 typed DTO。
- **不修改 Phase 14-19 任何 baseline**。
- **不引入新 `PidGraphicKind` variant**。
- **不实现 reference resolver**（仍是 Phase 21+ 工作）。
- **不提交 `dlls/`、`.i64`、私有 fixture**。Analysis 文档只引用 IDA
  地址（基址相对偏移）+ 反编译伪代码片段，**不**包含原始 binary。
- 5 道 pre-commit gate 保持绿（本 phase 不改代码，gate 应该不会失败；
  但仍要跑确认）。

## 非目标

- **不**落地 typed sub-kind DTO（推迟 Phase 21）。
- **不**修改 Phase 18 `decoded_sub_records_0x0010` audit collection
  的 shape。
- **不**修改 Phase 19 `leading_word` 字段的 shape 或命名。
- **不**实现 reference resolver。
- **不**反向 Phase 18 之外的 PSM type code。
- **不**装载新 IDA instance（12 个已 reachable 的 instance 足够）。

## Ask Before（要先问）

- 加载任何**新** IDA instance（当前 12 个 instance 之外）。
- 修改 Phase 14-19 任何 stable DTO 既有字段。
- 在 analysis 文档中暴露具体 IDA 地址段大块反汇编（> 100 行连续）。
- 把 Phase 20 scope 扩到 typed DTO 实现（属于 Phase 21）。
- 把 Phase 20 scope 扩到其它 PSM type code 反向。
- commit / push（本 phase 完成时再问）。

## Done Means（完成判据）

同时满足：

1. **AC1（RAD class identity）**：identified PSM 0x0010 的 RAD class
   全名 + CLSID + 所在 DLL + factory function 地址。证据形态参考
   Phase 16 `JStyleOverride` 三选一：
   - `radsrvitem.dll` PSM dispatch table 行索引 → factory function
   - CLSID-keyed COM factory call site
   - vtable layout 匹配 PSM IO sequence
2. **AC2（sub-kind discriminator location）**：identified sub-kind
   discriminator 字段的字节偏移 + 数据类型（u8 / u16 / u32 / u64）。
   证据形态：sub-kind branch 在 `Read` / `IO` 函数里的 switch / 多态
   dispatch + reads at offset N 的伪代码片段。
3. **AC3（sub-kind enumeration）**：枚举所有 IDA-confirmed sub-kind
   值 + 每种 sub-kind 在 Phase 18 audit collection 里对应的 record
   范围（用 leading_word 或 size bucket 关联）。
4. **AC4（authoritative analysis doc）**：写
   `docs/analysis/2026-05-17-phase20-psm-0x0010-rad-class.md`，结构
   mirror Phase 16 `jstyleoverride-v3-fields.md`：
   - §1 Class identity（CLSID + DLL + factory + vtable）
   - §2 IO sequence（按 byte offset 列字段类型 + 用途）
   - §3 Sub-kind discriminator（offset + 枚举值）
   - §4 Cross-fixture distribution（用 Phase 19 leading_word 数字
     反推每个 sub-kind 的 record count）
   - §5 IDA 地址索引（每个引用地址列出基址相对偏移 + IDA instance
     端口）
   - §6 与 Phase 16 JStyleOverride reference chain 的关系
   - §7 Known unknowns（明确哪些字段 / sub-kind 仍 audit-only）
   - §8 Phase 21 implementation prerequisites（typed DTO 字段表草图）
5. **AC5（Phase 14-19 baseline 不退化）**：5 道 pre-commit gate 全绿
   （不改代码，应该全绿；仍要跑确认）。
6. **AC6（progress.jsonl 完整 evidence trail）**：每个 IDA 调查动作
   都 append 一条 entry，包含：
   - 工具调用（IDA MCP tool + arguments）
   - 输入（function address / pattern）
   - 输出（核心发现摘要）
   - 推论（这条证据如何支持 AC1 / AC2 / AC3）
7. **AC7（不超出 IDA 已 load 的 12 个 instance）**：所有 IDA 调查只
   用现有 instance（list_instances 已确认）。若需要新 instance，写
   blocker 暂停。

停止条件全部写入 `blockers.md`。
