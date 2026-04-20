# 开发计划：Phase 10h — session 成果归档 + 卫生 pass

> 起稿：2026-04-21
> 背景：本 session（2026-04-21）连续 ship 了 12 个 commit（v0.4.2 →
> v0.7.0），跨两个大周期（Writer 建设 + SPPID full-parse）。按 Phase
> 9d 方法论"连续新功能后强制插入卫生 pass"，这轮不加 feature，只做
> 归档 + 轻量清理。
> 目标：v0.7.1 patch ship。

## 动机

- **检索困难**：Phase 9k/9l/9m/9n/9o + 10a/10b/10c/10d/10e/10f/10g
  共 12 轮，每轮 dev plan 各一份，CHANGELOG 条目各一段。后来者想
  回答"v0.5.x 做了什么"/"coverage 怎么演化的"要跑一堆 grep。
- **docs 导航缺失**：`docs/phase8-9h-summary.md` 是 Phase 9h 之前的
  总结；之后 12 个 Phase 没有对应 summary 文档。
- **轻量清理窗口**：12 轮 feature 后做一次"停下来扫地"很健康
  （Phase 9d / 9i / 9k 都是这种节奏）；本轮把 session 成果封存
  在一份可 grep 的文档里。

## 非目标

- 不加新 API
- 不改 parser
- 不动 coverage / Writer 行为
- 不做新 Phase 的 plan（Phase 10i / 11a 各自需要独立 session）

## 范围

| 文件 | 改动 | 行数 |
|---|---|---|
| `docs/phase10-coverage-series-summary.md` **新增** | 归档 Phase 9k-9o 和 10a-10g 的 12 个 commit 里程碑 | +200 |
| `docs/sppid/v0.7.x-status.md` **新增** | SPPID 解析能力现状表（stream / decode level / writer coverage / roadmap Phase）| +100 |
| `CHANGELOG.md` | `[0.7.1]` | +25 |
| `Cargo.toml` | 0.7.0 → 0.7.1 | ±1 |
| 可选：移除明显过时的 TODO 注释或 docstring | ±10 |
| **本 plan** | | +本文件 |

~350 行，零代码逻辑改动。

## 具体内容

### A. `docs/phase10-coverage-series-summary.md`

一份风格对齐 `phase8-9h-summary.md` 的总结：

- 起点与终点（v0.4.1 vs v0.7.0 对照表：test count / 模块数 /
  Phase 数 / CHANGELOG 行数 / plan 文件数）
- 阶段轨迹（9k → 10g 每轮一段：动机 / 核心产出 / 方法论点评）
- 方法论沉淀（可列入：coverage 静态→动态的两段式策略、SemVer minor
  与 patch 的判定案例、doc 先于代码的 plan 文化）

### B. `docs/sppid/v0.7.x-status.md`

SPPID 解析能力快照表，一页纸形式：

- 每个顶层流当前 coverage 状态
- 每个 Writer 能力的可用方法签名
- 每个 CLI flag 的"谁引入的 Phase" + "什么时候该用"
- roadmap Phase 1-5 的完成度图

### C. CHANGELOG

`[0.7.1]` 写作 "docs archive; no behavior change" 类型的 patch
条目。

## 实施步骤

### W1 — 写 `phase10-coverage-series-summary.md`

### W2 — 写 `v0.7.x-status.md`

### W3 — 扫清明显过时的 doc 注释（grep TODO / FIXME / "parked"）

### W4 — ship v0.7.1

## 预计工时

- W1 ~45 min
- W2 ~30 min
- W3 ~15 min（如果没有 obvious 过时注释就 skip）
- W4 ~10 min
- **合计 ~1.5 hr**

## 验证清单

- [ ] fmt/clippy/test 全 0（代码零改动，但确认）
- [ ] test count 332（不变）
- [ ] Cargo.toml 0.7.1 + tag

## Next 候选

- **Phase 10i**: CP1252 / code page fallback for VT_LPSTR
- **Phase 10j**: DocumentSummaryInformation section 2 编辑
- **Phase 11a**: 规范化语义图层（roadmap Phase 3，大 Phase，需要
  独立 session 设计）
