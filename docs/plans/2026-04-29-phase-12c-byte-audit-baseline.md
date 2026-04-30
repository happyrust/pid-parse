# Phase 12c — Real Fixture Byte-Audit Baseline Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 把 `--byte-audit-baseline` 从可选工具升级为 CI 回归门：把至少 1 份真实 `.pid` fixture 的 byte-audit JSON 提交到 `docs/baselines/`，让 `.github/scripts/check-byte-audit-baselines.sh` 在 fixture 存在时产生 hard-fail 信号，缺 fixture 时仍 soft-skip。

**Architecture:** baseline JSON 与 fixture 路径按文件名一一对应；script 已经就位（v0.11.6 落地）；本 Phase 只在 `docs/baselines/` 添加 baseline + 解决中文文件名 fixture 的 ASCII slug 化 + 接通 `ci.yml`。零 parser / writer / lib API 改动。

**Tech Stack:** existing `pid_inspect --byte-audit --json` CLI, existing `.github/scripts/check-byte-audit-baselines.sh`, GitHub Actions, Markdown docs。

**Upstream:** `docs/plans/2026-04-29-pid-parse-roadmap.md` 阶段 A / Phase 12c。

---

## Pre-flight Checks

```bash
# 1. fixture 已就位（本地确认）
ls test-file/*.pid
# 预期输出包含：
#   test-file/DWG-0201GP06-01.pid
#   test-file/DWG-0202GP06-01.pid
#   test-file/工艺管道及仪表流程-1.pid

# 2. baseline 目录不存在（待 Task 2 创建）
ls docs/baselines 2>/dev/null || echo "missing (expected)"

# 3. runner script 已就位
test -f .github/scripts/check-byte-audit-baselines.sh && echo "ok"

# 4. CLI 支持 --byte-audit --json
cargo run --locked --bin pid_inspect -- --help | grep byte-audit
```

如果以上任何一项不符合，**先停下来**确认环境，再进入 Task 1。

---

## Task 1: 确认 runner 当前行为

### Files
- Existing: `.github/scripts/check-byte-audit-baselines.sh`

### Steps

1. 在缺 baseline 的当前状态下跑一次 runner，验证 soft-skip：
   ```bash
   bash .github/scripts/check-byte-audit-baselines.sh
   echo "exit=$?"
   ```
2. 预期：脚本打印 "no baselines"（或类似 skip 信息），`exit=0`。
3. 阅读脚本源码，确认：
   - 是否按 `docs/baselines/<name>.byte-audit.json` ↔ `test-file/<name>.pid` 的命名约定派生 fixture 路径。
   - fixture 缺失时是 skip 还是 fail。
   - baseline 缺失时是 skip 还是 fail。
4. 把脚本的命名约定记录到本 plan doc 的 Task 2 注释里（如果与下面假设不一致，调整 Task 2 命名）。

### Expected Result

- runner 在无 baseline 时 exit 0，并打印明确 skip 消息。
- 命名约定已确认。

---

## Task 2: 决定 baseline 命名（处理中文文件名）

### Background

本仓 fixture 之一是 `test-file/工艺管道及仪表流程-1.pid`（中文文件名）。直接派生
`docs/baselines/工艺管道及仪表流程-1.byte-audit.json` 会带来 3 个问题：

1. Windows / Linux / macOS 的文件名编码差异（NFC vs NFD on macOS）。
2. CI shell escaping 与 git pathspec 易踩坑。
3. 公开 baseline 文件名带项目相关中文，可能泄漏内部信息。

### Decision

**采用 ASCII slug + 元数据映射**：

- baseline 文件名只用 ASCII：
  - `docs/baselines/dwg-0201gp06-01.byte-audit.json`
  - `docs/baselines/dwg-0202gp06-01.byte-audit.json`
  - `docs/baselines/sample-cn-1.byte-audit.json`（中文 fixture 的脱敏 slug）
- 加一份 `docs/baselines/README.md` 列出 slug ↔ fixture 真实路径的映射表。
- runner 脚本（Task 4 调整）：从 `<slug>.byte-audit.json` 读 `__fixture_path` 元字段而不是直接派生 `test-file/<slug>.pid`。
  - 元字段方案保留 ASCII baseline 文件名，但 fixture 路径仍可指向中文文件。
  - 如果 baseline JSON schema 不接受额外顶层字段，则在 baseline 同名 sidecar 文件 `<slug>.fixture.txt` 里写 fixture 相对路径。

### Files

- Create: `docs/baselines/README.md`
- Create: `docs/baselines/<slug>.fixture.txt`（每个 baseline 一个 sidecar）
- Modify (Task 4): `.github/scripts/check-byte-audit-baselines.sh`

### Steps

1. 创建 `docs/baselines/` 目录。
2. 写 `docs/baselines/README.md`，模板：
   ```markdown
   # byte-audit baselines

   每份 baseline JSON 都对应一份真实 `.pid` fixture。文件名只用
   ASCII slug，避免跨平台 / 跨 shell 的编码问题；fixture 真实路径写在
   同名 `.fixture.txt` sidecar 里。

   | slug | fixture path | 说明 |
   |---|---|---|
   | dwg-0201gp06-01 | test-file/DWG-0201GP06-01.pid | DWG-style A01 |
   | dwg-0202gp06-01 | test-file/DWG-0202GP06-01.pid | DWG-style |
   | sample-cn-1 | test-file/工艺管道及仪表流程-1.pid | 中文 fixture（私有） |

   ## 如何新增 / 更新 baseline

   1. 选 ASCII slug。
   2. 生成 baseline：
      ```bash
      cargo run --locked --bin pid_inspect -- "$FIXTURE" \
          --byte-audit --json > docs/baselines/$SLUG.byte-audit.json
      ```
   3. 写 sidecar：`echo "$FIXTURE" > docs/baselines/$SLUG.fixture.txt`
   4. 跑 `bash .github/scripts/check-byte-audit-baselines.sh` 确认 0 diffs。
   5. 提交 commit，message 用 `docs(baselines): add $SLUG byte-audit baseline`。
   ```

### Expected Result

- `docs/baselines/` 已建，README 解释 slug 约定。
- 后续 Task 3 可按该约定生成 baseline。

---

## Task 3: 生成 3 份真实 fixture 的 baseline

### Files

- Create: `docs/baselines/dwg-0201gp06-01.byte-audit.json`
- Create: `docs/baselines/dwg-0201gp06-01.fixture.txt`
- Create: `docs/baselines/dwg-0202gp06-01.byte-audit.json`
- Create: `docs/baselines/dwg-0202gp06-01.fixture.txt`
- Create: `docs/baselines/sample-cn-1.byte-audit.json`
- Create: `docs/baselines/sample-cn-1.fixture.txt`

### Steps

```bash
# DWG-0201GP06-01
cargo run --locked --bin pid_inspect -- \
    test-file/DWG-0201GP06-01.pid --byte-audit --json \
    > docs/baselines/dwg-0201gp06-01.byte-audit.json
echo "test-file/DWG-0201GP06-01.pid" > docs/baselines/dwg-0201gp06-01.fixture.txt

# DWG-0202GP06-01
cargo run --locked --bin pid_inspect -- \
    test-file/DWG-0202GP06-01.pid --byte-audit --json \
    > docs/baselines/dwg-0202gp06-01.byte-audit.json
echo "test-file/DWG-0202GP06-01.pid" > docs/baselines/dwg-0202gp06-01.fixture.txt

# Chinese-named fixture（用 PowerShell `Out-File -Encoding utf8` 在 Windows 上更稳）
cargo run --locked --bin pid_inspect -- \
    "test-file/工艺管道及仪表流程-1.pid" --byte-audit --json \
    > docs/baselines/sample-cn-1.byte-audit.json
echo "test-file/工艺管道及仪表流程-1.pid" > docs/baselines/sample-cn-1.fixture.txt
```

> Windows PowerShell 注意事项：`>` 在 PowerShell 5.1 默认输出 UTF-16LE，会把 JSON 文件搞坏。务必用：
>
> ```powershell
> cargo run --locked --bin pid_inspect -- test-file/DWG-0201GP06-01.pid --byte-audit --json `
>     | Out-File -Encoding utf8NoBOM docs/baselines/dwg-0201gp06-01.byte-audit.json
> ```
>
> 或在 Git Bash / WSL / cmd.exe 下用 `>`。

### Sanity Checks

- 每个 baseline JSON 第一行是 `{`。
- `python -c "import json; json.load(open('docs/baselines/dwg-0201gp06-01.byte-audit.json'))"` 不报错。
- `overall_consumed + overall_leftover == total_file_bytes`。
- 至少有一个 stream `parser_name != null`。

### Privacy / License Review

- 中文 fixture 是否私有数据？如果是：
  - **不要**把 fixture 本身 commit 到公开仓（已在 `.gitignore` 控制；确认）。
  - **可以**把 baseline JSON commit，因为只含字节偏移数值与 stream 路径名（路径名是 SmartPlant 标准命名空间，不含业务数据）。
  - 若 baseline 中含可识别业务信息（如 stream path 含 drawing number），脱敏或换 fixture。
- 写 commit message 时不要包含 fixture 内容预览。

### Expected Result

- 3 份 baseline JSON 与 3 份 sidecar txt 在 `docs/baselines/`。
- 文件大小预期：每份 baseline 约 5–50 KB。

---

## Task 4: 调整 runner 支持 sidecar fixture path

### Files

- Modify: `.github/scripts/check-byte-audit-baselines.sh`

### Current Behavior（推测，待 Task 1 确认）

旧 runner 大概率是按 baseline 文件名 stem 派生 `test-file/<stem>.pid`：

```bash
for baseline in docs/baselines/*.byte-audit.json; do
    stem="$(basename "$baseline" .byte-audit.json)"
    fixture="test-file/$stem.pid"
    ...
done
```

### Target Behavior

优先读 sidecar `<stem>.fixture.txt`，回退到旧逻辑保持向后兼容：

```bash
for baseline in docs/baselines/*.byte-audit.json; do
    stem="$(basename "$baseline" .byte-audit.json)"
    sidecar="docs/baselines/$stem.fixture.txt"
    if [ -f "$sidecar" ]; then
        fixture="$(head -n 1 "$sidecar")"
    else
        fixture="test-file/$stem.pid"
    fi
    if [ ! -f "$fixture" ]; then
        echo "skip: $stem (fixture $fixture not found)"
        continue
    fi
    cargo run --locked --bin pid_inspect -- \
        "$fixture" --byte-audit --byte-audit-baseline "$baseline" \
        || exit 1
done
```

### Steps

1. 按 Task 1 的实际脚本结构改动；保持 `set -euo pipefail` 与现有日志风格。
2. 加一行 echo `"checking $stem against $fixture"` 帮助 CI 排错。
3. 跑：
   ```bash
   bash .github/scripts/check-byte-audit-baselines.sh
   ```
4. 预期 3 个 baseline 全部 PASS（因为 baseline 是刚生成的当前状态）。

### Expected Result

- runner 能从 sidecar 解析出中文 fixture path 并跑通比较。
- 无 baseline 时仍 soft-skip exit 0。

---

## Task 5: 接通 CI

### Files

- Modify: `.github/workflows/ci.yml`

### Steps

1. 在 `cargo test` 与 `cargo clippy` 之后、`check-missing-docs.sh` 旁边加一步：
   ```yaml
   - name: byte-audit baselines
     run: bash .github/scripts/check-byte-audit-baselines.sh
   ```
2. 公开 CI 上 fixture 通常缺失（`.gitignore` 排除），runner 会 soft-skip。
3. 如果项目有私有 CI runner 持有 fixture，可以在该 runner 上跑同一脚本。

### Expected Result

- `.github/workflows/ci.yml` 含 byte-audit baseline 步骤。
- 公开 CI 通过（soft-skip）。

---

## Task 6: 文档与 CHANGELOG

### Files

- Modify: `docs/byte-audit-guide.md`
- Modify: `CHANGELOG.md`

### Steps

1. `docs/byte-audit-guide.md` 末尾追加章节"Baseline workflow"：
   - slug 命名约定。
   - sidecar `.fixture.txt` 用法。
   - 3 类 regression（overall ratio / stream consumed / traced→unregistered）。
   - 公开 vs 私有 fixture 分流。
2. `CHANGELOG.md` 在 `## [Unreleased]` 顶部加段：
   ```markdown
   ### byte-audit：真实 fixture baseline 接入 CI（Phase 12c）

   `docs/plans/2026-04-29-phase-12c-byte-audit-baseline.md` 落地：

   - `docs/baselines/` 提交 3 份真实 fixture 的 byte-audit baseline
     JSON（DWG-0201GP06-01 / DWG-0202GP06-01 / sample-cn-1），用 ASCII
     slug + sidecar `.fixture.txt` 处理中文 fixture 命名。
   - `.github/scripts/check-byte-audit-baselines.sh` 升级支持 sidecar
     fixture path；保持无 baseline / 无 fixture 时 soft-skip 退出 0。
   - CI 接通 baseline runner，fixture 在场时任何 `overall_coverage_ratio`
     下降 / traced stream `consumed_bytes` 下降 / traced stream 翻回
     unregistered 都会 hard-fail。
   - `docs/byte-audit-guide.md` 新增 "Baseline workflow" 章节交代 slug
     命名约定与 fixture 私有性分流。

   零 lib API 变化、零 CLI surface 变化、零 parser 行为变化。
   ```

---

## Task 7: 5 道 pre-commit gate 验证

```bash
cargo build --workspace --locked --all-targets
cargo test --workspace --locked --all-targets
cargo clippy --workspace --locked --all-targets -- -D warnings
cargo fmt --all -- --check
bash .github/scripts/check-missing-docs.sh
bash .github/scripts/check-byte-audit-baselines.sh
```

全部 EXIT=0 才能进 commit。

---

## Task 8: Commit

### Commit message

```
docs(baselines): add real fixture byte-audit baselines (Phase 12c)
```

或拆 2 个 commit：

1. `chore(baselines): add ASCII-slug runner support and README`
   （Task 1, 2, 4）
2. `docs(baselines): add 3 real fixture byte-audit baselines and wire CI`
   （Task 3, 5, 6）

---

## Acceptance Criteria

- [ ] `docs/baselines/` 含 3 份 baseline JSON + 3 份 sidecar txt + 1 份 README
- [ ] `.github/scripts/check-byte-audit-baselines.sh` 支持 sidecar fixture path
- [ ] `.github/workflows/ci.yml` 调用 baseline runner
- [ ] `docs/byte-audit-guide.md` 含 baseline workflow 章节
- [ ] `CHANGELOG.md` `[Unreleased]` 段记录 Phase 12c
- [ ] 5 道 pre-commit gate + baseline runner 全部 EXIT=0
- [ ] 公开 CI 在无 fixture 时 soft-skip

---

## Out of Scope

- 不动 parser 行为（PSM/Sheet 等深层解码留给 Phase 11x）
- 不改 byte-audit framework 的 JSON schema（schema lock 留给 Phase 12d）
- 不引入新的 CLI flag

---

## Forward Links

- 完成后下一个 Phase：`docs/plans/2026-04-XX-phase-12d-audit-schema-lock.md`（待写）
- 战略上下文：`docs/plans/2026-04-29-pid-parse-roadmap.md` 阶段 A
