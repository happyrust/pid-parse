# Phase 11a-2b — Safe Commit Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to execute this plan step-by-step. Do not commit or push until the user explicitly authorizes it.

**Date:** 2026-04-30  
**Parent plan:** `docs/plans/2026-04-30-phase-11a-2b-next-steps.md`  
**Goal:** 在当前 mixed working tree 中，只拆出 Phase 11a-2b closeout，避免把 W1 / 12c / 12d baseline 与 docs 改动混进同一个 commit。  
**Current checkpoint:** changelog closeout 已完成，focused verification 全绿，剩余动作是安全 stage、commit、push。  
**Estimate:** 20-40 min after explicit commit authorization.

---

## 0. Why This Plan Exists

当前工作区不是干净的 11a-2b 单一 diff：

- 11a-2b 相关：
  - `src/crossref.rs`
  - `src/inspect/report.rs`
  - `src/inspect/coverage.rs`
  - `CHANGELOG.md` 中的 `Phase 11a-2b` 段
  - 可选：`docs/plans/2026-04-30-phase-11a-2b-next-steps.md`
  - 可选：`docs/plans/2026-04-30-phase-11a-2b-safe-commit.md`
- unrelated / later sprint 相关：
  - `.github/scripts/check-byte-audit-baselines.sh`
  - `docs/byte-audit-guide.md`
  - `docs/baselines/`
  - `docs/plans/2026-04-29-*`
  - `docs/plans/2026-04-30-phase-12d-audit-schema-lock.md`
  - `docs/plans/2026-04-30-w1-unblock-and-ship-sprint.md`

`CHANGELOG.md` 也混合了 11a-2b 与 12c/W1 内容，所以不能简单
`git add CHANGELOG.md`。

---

## 1. Commit Boundary

### Commit A — 11a-2b code closeout

Include:

- `src/crossref.rs`
- `src/inspect/report.rs`
- `src/inspect/coverage.rs`
- only the `CHANGELOG.md` hunk for:
  - `### inspect：PSMclustertable consistency guardrail 收口（Phase 11a-2b）`

Exclude:

- all baseline runner / docs / roadmap files.
- `CHANGELOG.md` 里 `Phase 12c`、fixture drift、roadmap 相关段落。
- safe-commit planning docs, unless user asks to include docs in same commit.

Suggested message:

```text
feat(parser): surface PSM cluster consistency guardrails
```

### Commit B — optional planning docs

Only if user wants plan files committed now:

- `docs/plans/2026-05-07-phase-11a-2b-consistency-closeout.md`
- `docs/plans/2026-04-30-phase-11a-2b-next-steps.md`
- `docs/plans/2026-04-30-phase-11a-2b-safe-commit.md`

Suggested message:

```text
docs(plan): add Phase 11a-2b closeout handoff
```

If the user wants one commit only, prefer Commit A and leave planning docs untracked.

---

## 2. Pre-flight Before Staging

Run:

```bash
git status --short
cargo fmt --all -- --check
cargo test --locked --lib crossref::tests::psm_cluster_decoded_consistency
cargo test --locked --lib inspect::report::tests::report_shows_psm_cluster
cargo test --locked --lib inspect::coverage
cargo test --locked --test parse_real_files psm_cluster -- --nocapture
git diff --check
```

Expected:

- all commands exit 0.
- only line-ending warnings may appear from Git on Windows.
- no generated files changed as a side effect.

If any command fails, stop before staging.

---

## 3. Safe Staging Strategy

Interactive `git add -p` is not suitable for this environment. Use explicit staging:

```bash
git add src/crossref.rs src/inspect/report.rs src/inspect/coverage.rs
```

For `CHANGELOG.md`, stage only the 11a-2b hunk. Recommended method:

1. Generate a temporary patch containing only the 11a-2b changelog hunk.
2. Apply it to the index with `git apply --cached`.
3. Delete the temporary patch.

The patch must contain only this section:

```markdown
### inspect：`PSMclustertable` consistency guardrail 收口（Phase 11a-2b）

- 补齐 decoded candidate consistency 的 warning / missing path 测试，锁住名称、
  sheet marker 与 decoded records 缺失时的诊断行为。
- `pid_inspect` 文本 report 在 `PSMclustertable` 段输出 decoded consistency
  summary，让 consistency guardrail 不只停留在 crossref API。
- coverage policy 测试明确 `PSMclustertable` 继续保持 `PartiallyDecoded`；
  decoded record candidates 只是工程候选视图，不宣称 SmartPlant 字段语义已
  fully decoded。
```

Do not use:

```bash
git add CHANGELOG.md
```

unless the user explicitly wants to include 12c/W1 changelog content in the same commit.

---

## 4. Index Verification

Before committing, inspect exactly what is staged:

```bash
git diff --cached --name-status
git diff --cached --stat
git diff --cached -- CHANGELOG.md
git diff --cached -- src/crossref.rs src/inspect/report.rs src/inspect/coverage.rs
```

Must match:

- staged files are only `src/crossref.rs`, `src/inspect/report.rs`, `src/inspect/coverage.rs`, and the partial `CHANGELOG.md` hunk.
- `CHANGELOG.md` staged diff contains `Phase 11a-2b` and does not contain:
  - `Phase 12c`
  - `fix-real-file-fixture-drift`
  - `docs/baselines`
  - `12 周战略路线图`
- no `.github/scripts/*`, `docs/baselines/*`, or `docs/byte-audit-guide.md` staged.

If staged diff is wrong:

```bash
git restore --staged .
```

Then repeat staging more narrowly. Do not use destructive working-tree restore.

---

## 5. Commit And Push

Only after explicit user authorization:

```bash
git commit -m "$(cat <<'EOF'
feat(parser): surface PSM cluster consistency guardrails

EOF
)"
git status --short
git push origin HEAD
```

After push:

```bash
gh run list --limit 1
```

Report:

- commit hash.
- verification commands already passed.
- CI run state.
- remaining unrelated working-tree files.

---

## 6. Stop Conditions

Stop and ask before committing if:

- user has not explicitly authorized commit/push.
- staged diff contains W1 / 12c / 12d files.
- staged `CHANGELOG.md` includes unrelated sections.
- verification fails after staging.
- pre-commit hook modifies files in a way that mixes unrelated changes.

---

## 7. Done State

This plan is done when:

- 11a-2b code closeout is committed and pushed, or user chooses to stop before commit.
- unrelated W1 / 12c / 12d changes remain unstaged.
- Phase 11b handoff remains `docs/plans/2026-05-07-phase-11b-psmsegmenttable-records.md`.
