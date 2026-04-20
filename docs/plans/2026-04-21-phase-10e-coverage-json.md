# 开发计划：Phase 10e — coverage JSON 导出

> 起稿：2026-04-21
> 背景：Phase 10a (`--coverage`) / 10b (dynamic) / 10c (cluster probes)
> 已把 coverage 基础设施建完，但输出只有人类文本格式。CI 脚本、
> 外部 dashboard、甚至 Phase 10f 的字节级验证框架都需要机器可读的
> JSON。本轮补上这条出口。
> 目标：v0.6.4 patch ship。

## 动机

- `CoverageReport` 本身已 `#[derive(Serialize, Deserialize, JsonSchema)]`
  （Phase 10a 做的），但没有 ergonomic 入口。
- `pid_inspect --coverage` 现在只能输出 4-bucket 人类文本；下游若
  想自动 trip CI 阈值（e.g. "unknown 条数 > 0 fail"），需要 grep
  输出而不是 parse JSON。
- 与 Phase 9o 给 `WritePlan::{from_json, to_json, to_json_pretty}`
  的 pattern 对称：让 `CoverageReport` 也有同样 helpers + CLI 入口
  加 `--json` 支持。

## 非目标

- 不改变 `CoverageReport` 的字段
- 不改变 `coverage_report(&PidDocument)` 签名
- 不加新的 JSON schema validator 工具

## 范围

| 文件 | 改动 | 行数 |
|---|---|---|
| `src/model.rs`（`impl CoverageReport`）| `to_json` / `to_json_pretty` / `from_json` helpers | +50 |
| `src/inspect/coverage.rs` tests | +3 JSON round-trip 单测 | +60 |
| `src/bin/pid_inspect.rs` | `--coverage` + `--json` 组合输出 JSON | +25 |
| `tests/inspect_cli.rs` | +1 CLI 测试 | +35 |
| `CHANGELOG.md` | `[0.6.4]` | +20 |
| `Cargo.toml` | 0.6.3 → 0.6.4 | ±1 |
| **本 plan** | | +本文件 |

~200 行。additive only。

## 关键决策

### A. helper 方法而非独立 impl module

直接在 `CoverageReport` 上 `impl` 加 3 个方法，对齐 Phase 9o 的
`WritePlan::from_json` 设计。错误一律包装为 `PidError::ParseFailure
{ context: "coverage report JSON", ... }`。

### B. `pid_inspect --coverage --json` 行为

- `--coverage` 单独：打 human 文本（现状，Phase 10a 起）
- `--coverage --json`：打 `CoverageReport::to_json_pretty()` 结果
- `--json` 单独（不带 `--coverage`）：不变（dump 整个 PidDocument）

原因：`--json` 是既有"输出格式"flag，`--coverage` 是"section
selector" flag，两者组合语义天然明确。

### C. 字段 `kind` / `status` 的 JSON 表达

既有 serde 默认把 enum 变体渲染为 `"FullyDecoded"` / `"TopLevelStream"`
等字符串，consumer 侧无负担。不加 rename。

## 实施步骤

### W1 — `CoverageReport` JSON helpers + 单测

`src/model.rs` 在既有 `status_counts` 方法旁加：

```rust
impl CoverageReport {
    pub fn to_json(&self) -> Result<String, crate::error::PidError> { ... }
    pub fn to_json_pretty(&self) -> Result<String, crate::error::PidError> { ... }
    pub fn from_json(s: &str) -> Result<Self, crate::error::PidError> { ... }
}
```

测试放 `src/inspect/coverage.rs::tests`：

1. `coverage_report_json_round_trip_default`
2. `coverage_report_from_json_rejects_invalid_syntax_with_pid_error`
3. `coverage_report_to_json_pretty_contains_newlines`

### W2 — CLI --coverage --json

在 `print_coverage` 函数（Phase 10a 加的）里加一个 json 分支；传 flag
从 main 读。

### W3 — 1 条 CLI 集成测试

`tests/inspect_cli.rs` 加 `coverage_flag_with_json_emits_parseable_json`。

### W4 — ship

bump 0.6.3 → 0.6.4；CHANGELOG 加 `[0.6.4]`；commit + tag。

## 预计工时

- W1 ~25 min
- W2 ~15 min
- W3 ~20 min
- W4 ~10 min
- **合计 ~1 hr**

## 验证清单

- [ ] fmt/clippy/test 全 0
- [ ] test count 324 → 328+
- [ ] Cargo.toml 0.6.4 + tag

## Next 候选

- **Phase 10f**：字节级 consumed/leftover 验证框架（roadmap Phase 4）
  —— coverage 从 "模型字段填充" 升级为 "实际消费字节率"
- **Phase 10g**：PSMclustertable per-record 精确映射（需交叉验证
  锚点）
