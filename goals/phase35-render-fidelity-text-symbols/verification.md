# Verification: Phase 35 渲染保真

> “Complete”指 grill 验收口径（`goal-prompt.md`）在当前 6 fixture
> 范围内成立，不指 vendor 格式全量解码或像素级复刻。

## 验收矩阵（对应 grill 决策）

| # | 验收项 | 方法 | 期望 |
|---|---|---|---|
| 1 | 6 fixture 零错误加载 | OCS `examples/pid_probe.rs` 逐一运行 | 无 Err、实体计数 ≥ 既有 golden 下界 |
| 2 | 文字高度/旋转 | pid-parse 单测 + gongyi 截图 | 非占位 height/rotation 记录数 > 0；截图密集标注不再互相重叠 |
| 3 | 符号名称标签 | OCS 截图 + 实体断言 | 每个 crossref 命中的实例有可读名称标签 |
| 4 | 符号本体 | Circle.sym 对照 + 图纸截图 | Circle.sym 渲染为圆；库命中实例渲染本体几何 |
| 5 | 质量门 | `plan.md` §5 全套命令 | 全绿 |
| 6 | 截图可读 | `工艺管道及仪表流程-1.pid`、`A01.pid` DPI-aware 抓屏 | 人工判定“工程图可读” |
| 7 | 提交状态 | `git log`（两仓） | 语义化本地提交，无 push |

## Planning Verification

| Check | Command / Method | Expected |
|---|---|---|
| 目标包完整 | inspect `goals/phase35-render-fidelity-text-symbols/` | goal-prompt / brief / plan / verification / blockers / progress |
| 35-A 证据在案 | inspect `docs/analysis/2026-07-10-phase35a-0059-igcircle2d-layout-evidence.md` | 布局 admitted、语义门明确 |
| 格式 | `cargo fmt --all -- --check` | pass |
| 空白 | `git diff --check` | pass |

## Evidence Refresh Commands

```powershell
cargo run --quiet --example probe_igcircle2d_shape -- test-file
cargo run --quiet --example probe_psm_type_code_histogram
cargo run --quiet --example probe_curve_family_corpus_scan -- test-file
```

## Status

- 2026-07-26：目标包创建；35-A complete，其余 pending。
