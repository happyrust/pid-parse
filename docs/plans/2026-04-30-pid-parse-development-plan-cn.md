# pid-parse 下一阶段开发方案

## 目标
把 `pid-parse` 从“可解析、可报告、可回写部分元数据”推进到“解析深度可证明、下游导入契约稳定、关键未知区有优先级闭环”的阶段。

## 当前判断
当前项目已经具备成熟的 `.pid` 容器读取、元数据解析、对象/关系基础模型、writer passthrough、byte-audit 和 publish XML 主链。短板集中在三处：

1. `PSMclustertable` / `PSMsegmenttable` 仍是 partial decode。
2. `Sheet*` 仍以 probe 为主，缺完整几何/文本/端点 DTO。
3. object、relationship、endpoint、symbol、cluster、sheet provenance 尚未统一成一个 canonical semantic graph。

## 推荐路线

### W1：PSM table 结构化闭环
- 为 `PSMclustertable` 增加 per-record 字段命名与 trace。
- 为 `PSMsegmenttable` 增加 segment record 解码与 cluster 关联。
- 将新增字段接入 coverage、byte-audit、report、JSON schema。
- 验收：已有 fixture 不退化，新增 parser trace 能减少 leftover 或提升 decoded/probed 覆盖。

### W2：Sheet 几何最小稳定模型
- 定义 `SheetTextRun`、`SheetEndpointRecord`、`SheetCoordinateHint` 的稳定 DTO。
- 将现有 text run / endpoint / coordinate hint 从报告逻辑下沉到模型层。
- 不承诺完整图元，只交付“可追溯、可回归、可供下游显示”的最小模型。
- 验收：`pid_inspect --json` 能稳定输出 sheet text / endpoint evidence；byte-audit 标明哪些字节仍是 leftover。

### W3：Canonical Import Graph
- 在 `import_view` 或新模块中统一对象、关系、端点、符号、cluster、sheet provenance。
- 保留 source/provenance 字段，明确 `Decoded`、`Probed`、`Inferred`。
- 面向 H7CAD / CAD 导入方定义稳定 JSON contract。
- 验收：新增 contract 测试，schema/golden 输出变更可审查。

### W4：Publish XML DWG 闭环
- 保持 A01 publish fidelity gates 作为 hard gate。
- 对 DWG 侧补齐 fixture availability、loader enrichment、branch-point parity。
- 输出合规说明：`oxidized-mdf` GPL-3.0 对二进制分发的影响。
- 验收：DWG fixture 存在时执行 hard parity；缺失时 soft-skip 但报告未验证项。

## 验收矩阵
| 能力 | 验收方式 | Gate |
|---|---|---|
| PSM table decode | unit + real fixture + byte-audit diff | hard when fixture exists |
| Sheet DTO | JSON schema + fixture snapshot + byte-audit trace | hard for synthetic, fixture soft/hard 分层 |
| Canonical graph | schema/golden + import view tests | hard |
| Writer 安全 | round-trip + diff_packages | hard |
| Publish A01 | SemanticDiff / interface / attribute / rel parity | hard |
| Publish DWG | DWG mirror tests | fixture 存在时 hard，否则 soft-skip |

## 不做范围
- 不承诺短期完整解析所有 Sheet 图元。
- 不把 Probe 结果升级为稳定语义字段，除非 byte evidence 和 fixture 回归足够。
- 不把 `.pid` 深层语义编辑作为本阶段目标；writer 继续保持 passthrough-first。

## 交付物
- `task_plan.md`
- `findings.md`
- `progress.md`
- `docs/plans/2026-04-30-pid-parse-development-plan-cn.md`
- `docs/diagrams/pid-parse-development-roadmap.svg`
