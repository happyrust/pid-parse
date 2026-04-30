# Phase 11a-2b — Next Steps Ship Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to execute this plan task-by-task.

**Date:** 2026-04-30  
**Parent plan:** `docs/plans/2026-05-07-phase-11a-2b-consistency-closeout.md`  
**Current saved task:** 执行 Phase 11a-2b coverage policy  
**Goal:** 把已经完成的 11a-2b 工程 guardrail 收尾成一个可提交、可推送、可交接 Phase 11b 的小闭环。  
**Estimate:** 45-75 min

---

## 0. Current State

已完成：

- `src/crossref.rs`：`PSMclustertable` decoded candidate consistency 的 warning / missing paths 已补齐。
- `src/inspect/report.rs`：文本 report 已暴露 decoded consistency summary。
- `src/inspect/coverage.rs`：新增 coverage policy test，锁定 `PSMclustertable` 即使存在 decoded record candidates 仍保持 `PartiallyDecoded`。
- 已验证：
  - `cargo test --locked --lib inspect::coverage::tests::coverage_keeps_psm_cluster_table_partial_with_candidate_decoded_records`
  - `cargo fmt --all -- --check`
  - linter check

仍缺：

- `CHANGELOG.md` 中文说明。
- 11a-2b 组合验证。
- 提交并推送 11a-2b closeout。

---

## 1. Scope

### In Scope

- 更新 `CHANGELOG.md` 的 `[Unreleased]`，补 `Phase 11a-2b` 收尾段。
- 跑 focused verification，覆盖 crossref / report / coverage 三个可见面。
- 审查 diff，确保只包含 11a-2b 相关改动。
- 提交并推送。
- 在最终回复中明确 Phase 11b 可以从哪个 plan 继续。

### Out of Scope

- 不把 `PSMclustertable` 升为 `FullyDecoded`。
- 不刷新 byte-audit baseline。
- 不命名 SmartPlant 业务语义字段，例如 `cluster_id`、`declared_segment_count`。
- 不把当前工作区里 W1 baseline / runner / roadmap 的未提交改动混入 11a-2b commit。
- 不修改 `PSMsegmenttable` parser；Phase 11b 单独执行。

---

## 2. Pre-flight

先确认工作区和上游状态：

```bash
git status --short
git log --oneline -5
git diff -- src/crossref.rs src/inspect/report.rs src/inspect/coverage.rs CHANGELOG.md
```

预期：

- 11a-2b 相关文件至少包含：
  - `src/crossref.rs`
  - `src/inspect/report.rs`
  - `src/inspect/coverage.rs`
  - `CHANGELOG.md`
- 工作区可能同时存在 W1 / 12c / 12d 相关未提交文件；这些文件只 review，不 stage 到 11a-2b commit。
- 若 `CHANGELOG.md` 已含 11a-2b 段，先合并为一段，不追加重复标题。

---

## 3. Task Plan

### Task 1 — Changelog closeout

**File:** `CHANGELOG.md`

在 `[Unreleased]` 顶部的 `Phase 11a-2` 段之后追加 `Phase 11a-2b` 小节，表达三件事：

- consistency warning paths 已补测试。
- `pid_inspect` 文本 report 现在展示 decoded consistency summary。
- coverage policy 明确保持 conservative：`PSMclustertable` 仍是 `PartiallyDecoded`，decoded records 只是候选视图。

建议措辞：

```markdown
### inspect：`PSMclustertable` consistency guardrail 收口（Phase 11a-2b）

- 补齐 decoded candidate consistency 的 warning / missing path 测试，锁住名称、
  sheet marker 与 decoded records 缺失时的诊断行为。
- `pid_inspect` 文本 report 在 `PSMclustertable` 段输出 decoded consistency
  summary，让 consistency guardrail 不只停留在 crossref API。
- coverage policy 测试明确 `PSMclustertable` 继续保持 `PartiallyDecoded`；
  decoded record candidates 只是工程候选视图，不宣称 SmartPlant 字段语义已 fully decoded。
```

Acceptance:

- 不出现 `FullyDecoded` 宣称。
- 不声称已完成 Phase 11b。
- 不刷新 release version header。

### Task 2 — Focused verification

按从快到慢顺序运行：

```bash
cargo fmt --all -- --check
cargo test --locked --lib crossref::tests::psm_cluster_decoded_consistency
cargo test --locked --lib inspect::report::tests::report_shows_psm_cluster
cargo test --locked --lib inspect::coverage
cargo test --locked --test parse_real_files psm_cluster -- --nocapture
git diff --check
```

If any command fails:

- 先记录失败命令与首个失败断言。
- 只修与 11a-2b 相关的断言或实现。
- 不借机改 baseline / fixture / unrelated parser。
- 修完从失败命令开始重跑，再补跑后续命令。

### Task 3 — Diff scope review

审查：

```bash
git diff --stat
git diff -- src/crossref.rs src/inspect/report.rs src/inspect/coverage.rs CHANGELOG.md
git status --short
```

必须满足：

- 11a-2b commit 只 stage 这四类内容：
  - `src/crossref.rs`
  - `src/inspect/report.rs`
  - `src/inspect/coverage.rs`
  - `CHANGELOG.md`
- `docs/baselines/*`、`.github/scripts/check-byte-audit-baselines.sh`、`docs/byte-audit-guide.md`、W1 / 12c / 12d plans 不进入本 commit。
- 如果本文件需要提交，单独作为 planning/doc commit，或跟随用户要求处理；默认不混入 11a-2b code closeout commit。

### Task 4 — Commit and push

建议 commit message：

```text
feat(parser): surface PSM cluster consistency guardrails
```

执行：

```bash
git add src/crossref.rs src/inspect/report.rs src/inspect/coverage.rs CHANGELOG.md
git commit -m "feat(parser): surface PSM cluster consistency guardrails"
git status --short
git push origin HEAD
```

如果 pre-commit hook 修改文件：

- 重新 `git status --short` 和 `git diff`。
- 若修改属于 formatter 结果，stage 后创建新的 commit 或按当前对话的 git policy 处理。
- 不用 `--no-verify`。

### Task 5 — Post-push handoff

推送后确认：

```bash
gh run list --limit 1
```

最终状态说明应包含：

- commit hash。
- 本地 focused verification 结果。
- 最新 CI run 当前状态。
- Phase 11b 下一步入口：`docs/plans/2026-05-07-phase-11b-psmsegmenttable-records.md`。

---

## 4. Acceptance Criteria

- [ ] `CHANGELOG.md` 有 11a-2b 中文 closeout 段。
- [ ] focused verification 全部通过，或失败原因已明确且未继续提交。
- [ ] commit diff 不包含 W1 / 12c / 12d unrelated files。
- [ ] 11a-2b commit 已推送。
- [ ] Phase 11b handoff plan 已在最终回复中点明。

---

## 5. Risk Register

| Risk | Impact | Mitigation |
|---|---|---|
| 工作区已有 W1 / 12c / 12d 未提交文件 | 容易把 unrelated docs / baseline 混进 11a-2b commit | 只显式 `git add` 11a-2b 文件 |
| `CHANGELOG.md` 顶部已有多个 PSM 段 | 重复标题导致 release note 难读 | 合并到 Phase 11a-2 后面，保持 chronological narrative |
| coverage test 被误解为升级 decoded confidence | 下游以为 `PSMclustertable` FullyDecoded | changelog 和 test name 都强调 `PartiallyDecoded` |
| `parse_real_files psm_cluster` 受 fixture drift 影响失败 | 收尾被 W1 fixture 工作阻断 | 只接受和 11a-2b 相关修复；fixture drift 走 W1 plan |

---

## 6. Done State

11a-2b 完成后，仓库状态应满足：

- `PSMclustertable` decoded candidate view 有 consistency guardrails。
- report / coverage / crossref 三个可见面均有测试锁定。
- `PSMclustertable` 仍保守标记为 `PartiallyDecoded`。
- 后续可以进入 Phase 11b：`PSMsegmenttable` conservative record view。
