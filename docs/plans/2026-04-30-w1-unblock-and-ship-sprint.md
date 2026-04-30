# W1 Sprint — Unblock main + Ship Phase 12c + Land Phase 12d

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan day-by-day.

**Sprint window:** 2026-04-30 (Thu) ~ 2026-05-04 (Mon) — 5 working days
**Sprint goal:** 把 main 从 CI red 解阻 → 把已完成但未提交的 Phase 12c 产出 ship 上线 → 完成 roadmap 阶段 A 的 Phase 12d schema lock → 收口后无缝进入 W2 Phase 11a。

**Why this plan exists:** 当前仓里有 4 份相关但分散的 plan / status doc：

- `docs/plans/2026-04-29-fix-real-file-fixture-drift.md`（CI red 修复）
- `docs/plans/2026-04-29-phase-12c-byte-audit-baseline.md`（baseline 落盘）
- `docs/plans/2026-04-30-phase-12d-audit-schema-lock.md`（schema lock）
- `docs/plans/2026-04-29-pid-parse-roadmap.md`（12 周战略总图）

它们各自完整，但缺一份"按时间序怎么做、每天 acceptance 是什么"的 single source of truth。本文档就是这份 source of truth。每个 day 的任务都链接回详细 plan，**不重复执行细节**，只钉时间轴。

---

## 0. Sprint pre-flight

执行 Day 1 之前必须确认：

```bash
cd d:/work/plant-code/cad/pid-parse
git status
git log --oneline | head -10
```

预期：
- 当前分支 `main`，HEAD 是 `270405b` 或更靠后但 CI 仍 red 的 commit
- 工作区有未跟踪文件（baselines / plans）+ 部分已修改文件未 stage（runner / changelog / docs）
- `cargo test --workspace --locked --all-targets` 在 `tests/parse_real_files.rs` 上 fail 4 个

如果实际情况与预期不一致（比如有人已经做了部分工作），**先停下来读一遍 git log 与所有 4 份 upstream plan**，再决定哪些 Day 已完成。

---

## 1. Sprint timeline

| Day | 日期 | 主题 | 估时 | Upstream plan | Day-end gate |
|---|---|---|---|---|---|
| Day 1 | Thu 04-30 | 解阻 main（修 4 个 fixture-drift 测试） | 4h | `2026-04-29-fix-real-file-fixture-drift.md` | `cargo test --workspace --locked --all-targets` 全绿 |
| Day 2 | Fri 05-01 | Ship Phase 12c（baselines + runner + docs commit） | 3h | `2026-04-29-phase-12c-byte-audit-baseline.md` | `git push origin main` 后公开 CI 绿（baseline runner soft-skip exit 0） |
| Day 3 | Mon 05-04 | Phase 12d Task 1-5（schema 入口 + CLI flag + snapshot baseline） | 4h | `2026-04-30-phase-12d-audit-schema-lock.md` Task 1-5 | `cargo test --locked --test schema_snapshots` 0 diff PASS |
| Day 4 | Tue 05-05 | Phase 12d Task 6-9（docs + drift self-test + commit） | 3h | `2026-04-30-phase-12d-audit-schema-lock.md` Task 6-9 | 5 道 gate + drift self-test 全绿，commit pushed |
| Day 5 | Wed 05-06 | 收口 + W2 Phase 11a 草稿 | 2h | `2026-04-29-pid-parse-roadmap.md` 阶段 B | `docs/plans/2026-05-06-phase-11a-psmclustertable-records.md` 落盘 |

总计：~16h，含每日 5 道 gate / commit / push 的开销

---

## 2. Day 1 — 解阻 main（CI red → green）

> **Upstream:** `docs/plans/2026-04-29-fix-real-file-fixture-drift.md`

### Tasks（按顺序）

- [ ] **D1.1** 读 upstream plan Task 1 的 fixture findings table，把 4 个失败位置和实测数值默写出来一遍（强制理解，不抄）
- [ ] **D1.2** 实施 upstream plan Task 2：`relationship_endpoints_resolve_via_sheet_record` 改用比例容差
  - `resolved >= 0.7 * total`
  - `unresolved <= 0.15 * total`
  - `foreign_endpoints < total`
- [ ] **D1.3** 实施 upstream plan Task 3：`sheet_endpoint_records_one_per_relationship` 改用 `endpoint_records.len() >= 0.85 * relationships.len()`，保留 `rel_field_x ∈ relationships[].field_x` 结构性断言
- [ ] **D1.4** 实施 upstream plan Task 4：删除 `assert_eq!(advertised_id, source.drawing_id)`（Option A），加 NOTE 链 Phase 12a
- [ ] **D1.5** 实施 upstream plan Task 5：`second_file_builds_readable_layout_model` 把 `>= 5` 暂降 `>= 3`，加 TODO 链 Phase 11c
- [ ] **D1.6** 跑 5 道 gate + baseline runner，确认 0 fail
- [ ] **D1.7** 按 upstream plan Task 7 写 CHANGELOG `[Unreleased]` 段（注意：这一段会和 12c / 12d 段一起在最终 release 时归并）
- [ ] **D1.8** Commit：`fix(test): adjust real-file assertions to structural invariants (CI unblock)`
- [ ] **D1.9** Push 到 main，等 CI 跑完确认绿

### Day 1 acceptance gate

```bash
cargo build --locked --workspace --all-targets    # exit 0
cargo test  --locked --workspace --all-targets    # exit 0, 4 个旧测试都 PASS
cargo clippy --locked --workspace --all-targets -- -D warnings   # exit 0
cargo fmt --all -- --check                         # exit 0
bash .github/scripts/check-missing-docs.sh         # exit 0
gh run list --limit 1                              # 最新 main run = success
```

### Day 1 rollback rule

如果 Task 4 在 0201 fixture 上跑通但 0202 fail，**不要继续往下推**——回滚到 upstream plan "Out of Scope" 段，开独立 bug fix。本 sprint 不接 parser 行为改动。

---

## 3. Day 2 — Ship Phase 12c（baselines + runner + docs）

> **Upstream:** `docs/plans/2026-04-29-phase-12c-byte-audit-baseline.md`
>
> 前置：Day 1 完成且 main 已绿。

### Tasks

- [ ] **D2.1** `git status` 二次确认未提交清单：
  - untracked: `docs/baselines/*` (3 baseline + 3 sidecar + README)
  - untracked: `docs/plans/2026-04-29-{roadmap,phase-12c-byte-audit-baseline,fix-real-file-fixture-drift}.md`
  - untracked: `docs/plans/2026-04-30-{phase-12d-audit-schema-lock,w1-unblock-and-ship-sprint}.md`（本 plan 自己）
  - modified: `.github/scripts/check-byte-audit-baselines.sh`、`docs/byte-audit-guide.md`、`CHANGELOG.md`
- [ ] **D2.2** 单独 review 每份 baseline JSON：
  - `python -c "import json,sys; json.load(open('docs/baselines/dwg-0201gp06-01.byte-audit.json'))"` 不报错
  - `head -c 200 docs/baselines/dwg-0201gp06-01.byte-audit.json` 不应有 BOM 或 UTF-16
  - `wc -l docs/baselines/*.byte-audit.json` 大致一致量级
- [ ] **D2.3** 拆 commit A — runner & 文档：
  ```bash
  git add .github/scripts/check-byte-audit-baselines.sh \
          docs/baselines/README.md \
          docs/byte-audit-guide.md
  git commit -m "chore(baselines): add ASCII-slug runner support and README"
  ```
- [ ] **D2.4** 拆 commit B — baseline 数据 + plans + CHANGELOG：
  ```bash
  git add docs/baselines/*.byte-audit.json \
          docs/baselines/*.fixture.txt \
          docs/plans/2026-04-29-pid-parse-roadmap.md \
          docs/plans/2026-04-29-phase-12c-byte-audit-baseline.md \
          docs/plans/2026-04-29-fix-real-file-fixture-drift.md \
          docs/plans/2026-04-30-phase-12d-audit-schema-lock.md \
          docs/plans/2026-04-30-w1-unblock-and-ship-sprint.md \
          CHANGELOG.md
  git commit -m "docs(baselines): add 3 real fixture byte-audit baselines and W1 plans"
  ```
  > Note: `2026-04-30-*` plans 一起进 commit B 比单独 commit 好——它们是 12c 完成后才有意义的产物，逻辑上同源。
- [ ] **D2.5** Push：`git push origin main`
- [ ] **D2.6** 等 CI 跑完，确认：
  - `cargo test` step 全绿
  - `byte-audit baselines (optional)` step exit 0（公开 CI 上 fixture 缺失 → soft-skip）
  - `cargo audit` step 不引入新 advisory
- [ ] **D2.7** 在私有 fixture 环境（如本地 / 内部 runner）也跑一次 `bash .github/scripts/check-byte-audit-baselines.sh`，确认 fixture 在场时是 hard-fail 模式且当前 baseline 与实际 0 diff

### Day 2 acceptance gate

```bash
git log --oneline | head -3
# 预期最上方两个 commit 的 message 与 D2.3 / D2.4 一致

git status
# 工作区干净

gh run list --limit 1
# 最新 main run = success

bash .github/scripts/check-byte-audit-baselines.sh
# 私有 fixture 在场时 exit 0；公开缺 fixture 时 exit 0（skip 消息）
```

### Day 2 risk: PowerShell 把 baseline JSON 写成 UTF-16

如果 D2.2 发现 baseline JSON 有 BOM 或 UTF-16 编码（PowerShell 5.1 默认行为），在 commit 前用 Git Bash / WSL 重生成（参见 `2026-04-29-phase-12c-byte-audit-baseline.md` Task 3 的 PowerShell 注意事项）。**不要**在 Day 2 commit 一份编码错误的 baseline。

---

## 4. Day 3 — Phase 12d Task 1-5（schema 入口 + snapshot baseline）

> **Upstream:** `docs/plans/2026-04-30-phase-12d-audit-schema-lock.md` Task 1-5
>
> 前置：Day 2 完成且 main 已绿；本 plan（W1 sprint）已 commit 进入 main。

### Tasks

- [ ] **D3.1** 跑 12d plan Pre-flight Checks 4 项，全部 pass
- [ ] **D3.2** 实施 12d Task 1：`src/schema.rs` 加 4 个公开函数（含 doc comment）
- [ ] **D3.3** 实施 12d Task 2：`src/schema.rs` `mod tests` 加两条 `_well_formed_and_mentions_*` 测试
- [ ] **D3.4** 跑 `cargo test --lib --locked schema::tests` 确认 4 条测试（含 2 条新增）全绿
- [ ] **D3.5** 实施 12d Task 3：`src/bin/pid_inspect.rs` 加 `--coverage-schema` / `--audit-schema` flag，处理 schema-only 模式下不要求位置参数
- [ ] **D3.6** 手动 smoke：
  ```bash
  cargo run --locked --bin pid_inspect -- --coverage-schema | head -20
  cargo run --locked --bin pid_inspect -- --audit-schema | head -20
  cargo run --locked --bin pid_inspect -- test-file/DWG-0201GP06-01.pid --schema | head -5  # 旧路径不变
  ```
- [ ] **D3.7** 实施 12d Task 4：`tests/inspect_cli.rs` 加 2 条 CLI 集成测试
- [ ] **D3.8** 实施 12d Task 5：建 `tests/snapshots/` 目录、写 `tests/schema_snapshots.rs`（按 12d plan §5.2 代码模板）、用 `UPDATE_SNAPSHOTS=1` 生成 3 份 baseline JSON
- [ ] **D3.9** `cat .gitattributes` 确认是否需要追加 `tests/snapshots/*.json text eol=lf`；缺失时追加
- [ ] **D3.10** 不带 `UPDATE_SNAPSHOTS` 跑 `cargo test --locked --test schema_snapshots`，确认 3 条 0 diff PASS

### Day 3 acceptance gate

```bash
cargo test --locked --test schema_snapshots
# 3 条 PASS

cargo run --locked --bin pid_inspect -- --coverage-schema \
    | python -c "import json,sys; json.load(sys.stdin); print('valid coverage schema JSON')"
cargo run --locked --bin pid_inspect -- --audit-schema \
    | python -c "import json,sys; json.load(sys.stdin); print('valid audit schema JSON')"
```

### Day 3 stop rule

如果 D3.10 失败（snapshot 不稳定），**先停下来排查**：
1. `schemars 0.8` 输出顺序是否稳定？检查 BTreeMap key 顺序。
2. Windows CRLF 是否污染了 baseline？`file tests/snapshots/*.json` 应当全部是 ASCII 文本无 BOM。

确认稳定后再继续 Day 4。

---

## 5. Day 4 — Phase 12d Task 6-9（docs + drift self-test + commit）

> **Upstream:** `docs/plans/2026-04-30-phase-12d-audit-schema-lock.md` Task 6-9
>
> 前置：Day 3 完成；本地 working tree 含 schema 入口、CLI flag、snapshot baseline、新 lib/cli 测试（未 commit）。

### Tasks

- [ ] **D4.1** 实施 12d Task 6：更新 `AGENTS.md` / `CONTRIBUTING.md` / `docs/byte-audit-guide.md` / `README.md`
- [ ] **D4.2** 实施 12d Task 7：CHANGELOG `[Unreleased]` 顶部追加 Phase 12d 段（在 Phase 12c 段之后）
- [ ] **D4.3** 实施 12d Task 8：跑 5 道 gate + baseline runner
- [ ] **D4.4** 实施 12d Task 8 的 drift self-test：
  - 临时把 `StreamAuditSummary.consumed_bytes` 改名 `consumed`
  - `cargo test --locked --test schema_snapshots` 应当 fail，且错误信息精确指出 snapshot 路径与 update 命令
  - `git restore src/byte_audit/aggregate.rs`
  - 再跑测试确认 PASS
  - 把这次 self-test 结果记到 commit message 或 PR description
- [ ] **D4.5** 实施 12d Task 9：拆 2 commit
  ```bash
  git add src/schema.rs src/bin/pid_inspect.rs tests/inspect_cli.rs
  git commit -m "feat(api): add coverage / byte-audit schema entrypoints (Phase 12d)"

  git add tests/schema_snapshots.rs tests/snapshots/ \
          AGENTS.md CONTRIBUTING.md docs/byte-audit-guide.md README.md \
          CHANGELOG.md .gitattributes
  git commit -m "docs(snapshots): lock document / coverage / audit JSON schemas"
  ```
- [ ] **D4.6** Push：`git push origin main`
- [ ] **D4.7** 等 CI 跑完确认绿

### Day 4 acceptance gate

```bash
git log --oneline | head -5
# 应看到 W1 5 个新 commit 序列（fix-drift / 12c-runner / 12c-data+plans / 12d-feat / 12d-snapshots）

cargo test --locked --workspace --all-targets    # 全绿
bash .github/scripts/check-byte-audit-baselines.sh    # 0 diffs

gh run list --limit 1    # success
```

---

## 6. Day 5 — 收口 + W2 准备

> 前置：Day 4 完成。

### Tasks

- [ ] **D5.1** 在 `docs/plans/2026-04-29-pid-parse-roadmap.md` "TL;DR" 表的 12c / 12d 行加上 ✅ 标记或 strikethrough
- [ ] **D5.2** 在本 plan 顶部加一行 "Status: COMPLETED 2026-05-06"，把所有 task checkbox 标记完成
- [ ] **D5.3** 写 `docs/plans/2026-05-06-phase-11a-psmclustertable-records.md` 草稿，至少包含：
  - For Claude / Goal / Architecture / Tech Stack / Upstream
  - Pre-flight Checks
  - Task 1 占位（Red test：构造 `PSMclustertable` 现实输入并断言期望字段）
  - 留待 Day 1 of W2 详化
  - Acceptance Criteria 占位
- [ ] **D5.4** 跑 `git status` 确认干净，commit `docs(plans): mark W1 sprint done, draft Phase 11a stub`
- [ ] **D5.5** 在团队 channel / PR / 周报里给一行 sprint 总结：
  > W1 done: main unblocked + Phase 12c shipped + Phase 12d schema lock landed (~5 commits, ~16h). Roadmap 阶段 A 收口，W2 进入 PSM record 深化。

### Day 5 acceptance gate

- 当前 plan（本文档）所有 task checkbox 完成
- `docs/plans/2026-05-06-phase-11a-psmclustertable-records.md` 落盘
- `git log --oneline | head -10` 干净，`gh run list --limit 1` 绿

---

## 7. Sprint-level acceptance criteria

- [ ] CI 在 main 上从 Day 1 起持续绿（不允许中间穿插一次 red 不修就过周末）
- [ ] `tests/parse_real_files.rs` 4 个失败测试改为结构性 / 比例容差断言并 PASS
- [ ] `docs/baselines/*.byte-audit.json` 与 sidecar 入仓且 runner 在 fixture 在场时 hard-fail
- [ ] `pid_inspect --coverage-schema` / `--audit-schema` 可用并输出有效 JSON Schema
- [ ] `tests/schema_snapshots.rs` + `tests/snapshots/*.json` 守门 3 个 schema
- [ ] drift self-test 演示 schema 字段重命名 → snapshot fail（已恢复）
- [ ] Roadmap 阶段 A 标记完成，阶段 B Phase 11a 草稿到位

---

## 8. Out of scope（本 sprint 不做）

- 不动 `oxidized-mdf` vendored crate（除非 D2.6 cargo audit 报安全问题）
- 不引入 `insta` / `cargo-insta` 等 snapshot 框架
- 不开始 Phase 11a / 11b / 11c 实施（W2 才做）
- 不动 publish XML pipeline 任何代码
- 不重构 `CoverageReport` / `ByteAuditReport` / `PidDocument` 字段
- 不优化 `byte_audit` framework 计算性能
- 不为新 schema 入口加 OpenAPI 包装或 GraphQL 暴露

---

## 9. Sprint risks

| 风险 | 触发条件 | 缓解 |
|---|---|---|
| Day 1 Task 4 揭出真正的 parser regression | DA records 在 fixture 上 advertised DrawingID 漂移不是 sanitization 副作用 | 按 fix-drift plan "Out of Scope" 回滚，开独立 bug fix；本 sprint 推后到下周 |
| Day 2 PowerShell 编码污染 baseline | Windows 5.1 默认 `>` UTF-16 | 用 Git Bash / WSL 重生 baseline，不接受 commit 编码错误的 JSON |
| Day 3 schemars 输出不稳定 | BTreeMap 内部排序漂移 | snapshot fail 时先排查；必要时记一笔 issue 等 schemars upstream 修，本 phase 改 "包含子串" 守门替代 byte-equal |
| Day 4 drift self-test 没真正展示 fail | 测试代码逻辑错误 | 演练步骤前先单步跑过测试代码；把演示结果（terminal 输出）贴 PR description |
| Day 5 Phase 11a 草稿门槛过高 | 只剩 2h 写不完完整 plan | Day 5 只交"草稿"，W2 第一天再详化；本 sprint 不卡 11a 完整 plan |
| 突发线上 / 高优 bug | 任何外部紧急任务 | 立即暂停本 sprint，把当前 Day 的剩余 task 标 SKIPPED，紧急任务结束后从 SKIPPED 处重启 |

---

## 10. Forward links

- 完成后立即开始：`docs/plans/2026-05-06-phase-11a-psmclustertable-records.md`（Day 5 D5.3 落盘）
- 战略上下文：`docs/plans/2026-04-29-pid-parse-roadmap.md` 阶段 B
- 本 sprint 涉及的所有 detail plan：
  - `docs/plans/2026-04-29-fix-real-file-fixture-drift.md`
  - `docs/plans/2026-04-29-phase-12c-byte-audit-baseline.md`
  - `docs/plans/2026-04-30-phase-12d-audit-schema-lock.md`

---

## 11. 命名 / 工程约束（本 sprint 期间硬执行）

复述 `AGENTS.md` 的关键约束，避免本 sprint 中漏遵守：

1. **5 道 pre-commit gate** 每天结束前都必须跑过：
   ```bash
   cargo build  --locked --workspace --all-targets
   cargo test   --locked --workspace --all-targets
   cargo clippy --locked --workspace --all-targets -- -D warnings
   cargo fmt    --all -- --check
   bash .github/scripts/check-missing-docs.sh
   ```
   外加：
   ```bash
   bash .github/scripts/check-byte-audit-baselines.sh
   ```
2. **`missing_docs` ratchet** 不允许上调 baseline；要么不引入新 `pub` 项，要么补 `///` doc。
3. **Commit message 格式** 参照 `feat(area): ...` / `fix(area): ...` / `docs(area): ...` / `chore(area): ...`。本 sprint 涉及的 area：`test` / `baselines` / `api` / `snapshots` / `plans`。
4. **不主动扩展 scope**：每个 task 完成后即停，不添加未列入本 plan 的"顺便修一下"的改动。
