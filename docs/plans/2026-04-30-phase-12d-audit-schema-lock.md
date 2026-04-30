# Phase 12d — Coverage / Byte-Audit JSON Schema Lock

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 把 `CoverageReport` 与 `ByteAuditReport` 的 JSON 输出契约从"靠 `derive(JsonSchema)` 默认行为"升级为"对外锁定 + CI 守门"。下游消费者（codegen tooling、TypeScript / Python / C# clients、`pid_writer_validate` 等）拿到的 schema 必须随仓内类型同步演进；任何字段重命名 / 删除 / 类型基础变更都会被 snapshot 测试 hard-fail，强制走"先调整 baseline → 再合并"路径，避免悄悄破坏下游。

**Architecture:**

- 现有 `src/schema.rs` 只暴露 `pid_document_schema()` / `pid_document_schema_pretty()`（针对 `PidDocument`），CLI 通过 `pid_inspect --schema` 输出。
- `CoverageReport`（`src/model.rs`）与 `ByteAuditReport`（`src/byte_audit/aggregate.rs`）已经 `#[derive(JsonSchema)]`，但**没有公开 schema 入口**，下游只能自己 `schemars::schema_for!` 重新生成，schema drift 没有 CI 守门。
- 本 Phase 的改动是**纯 additive**：
  1. `src/schema.rs` 新增 4 个公共函数（`coverage_report_schema` / `byte_audit_report_schema` 各两条，原始 + pretty）。
  2. `pid_inspect` 新增 `--coverage-schema` 与 `--audit-schema` flag（`--schema` 行为不动）。
  3. `tests/schema_snapshots.rs` + `tests/snapshots/*.json` 新增 3 份 schema 快照（document / coverage / audit）做 byte-equal 比较；`UPDATE_SNAPSHOTS=1` 环境变量驱动重写。
- 不动 `CoverageReport` / `ByteAuditReport` / `PidDocument` 任何字段；不引入 `insta` 等新 dev-dep。

**Tech Stack:** Rust 2021 + 现有 `schemars 0.8` + `serde_json` + 标准 `#[test]` 框架。零新依赖。

**Upstream:**

- `docs/plans/2026-04-29-pid-parse-roadmap.md` 阶段 A / Phase 12d
- `docs/plans/2026-04-29-phase-12c-byte-audit-baseline.md` 完成后的 forward link
- PRD `docs/prd-pid-parse-current-state.md` §9.1 "新 decoder 必须声明 confidence" 与本 Phase 的"schema 字段不能悄悄漂移"是同源约束的两端

---

## Pre-flight Checks

```bash
# 1. main 已转绿（fixture-drift fix 已落地）
cargo test --workspace --locked --all-targets

# 2. Phase 12c 产出已 commit（runner / baselines / docs）
git log --oneline | head -5  # 应当看到 Phase 12c 相关 commit
ls docs/baselines             # 应有 3 份 baseline + sidecar + README
git status                    # 应该是 working tree clean

# 3. 现有 schema CLI / API 工作正常
cargo run --locked --bin pid_inspect -- test-file/DWG-0201GP06-01.pid --schema | head -20

# 4. 现有 derive(JsonSchema) 类型清单
rg "derive.*JsonSchema" src --no-heading | head -30
```

如果以上任何一项不符合（尤其 main 仍 red、12c 未 commit），**先停下来**回到上游 plan doc 把阻塞解掉，再进入 Task 1。

---

## Task 1: 在 `src/schema.rs` 暴露 coverage / audit schema 入口

### Files

- Modify: `src/schema.rs`

### Steps

在 `pid_document_schema_pretty()` 之后追加 4 个公开函数 + module-level doc 更新，保持模块整体调性一致：

```rust
use crate::byte_audit::aggregate::ByteAuditReport;
use crate::model::{CoverageReport, PidDocument};

/// Produce the JSON Schema for a [`CoverageReport`].
///
/// Downstream CI / codegen consumers can pin the coverage report shape
/// against the snapshot under `tests/snapshots/coverage_report_schema.json`
/// to detect breaking field renames / removals before they hit a release.
pub fn coverage_report_schema() -> Schema {
    schemars::schema_for!(CoverageReport)
}

/// Pretty-printed variant of [`coverage_report_schema`].
pub fn coverage_report_schema_pretty() -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(&coverage_report_schema())
}

/// Produce the JSON Schema for a [`ByteAuditReport`].
///
/// Same drift-guard story as [`coverage_report_schema`]: pinned via a
/// snapshot at `tests/snapshots/byte_audit_report_schema.json` and
/// surfaced through `pid_inspect --audit-schema`.
pub fn byte_audit_report_schema() -> Schema {
    schemars::schema_for!(ByteAuditReport)
}

/// Pretty-printed variant of [`byte_audit_report_schema`].
pub fn byte_audit_report_schema_pretty() -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(&byte_audit_report_schema())
}
```

模块顶部 doc comment 增加一行交代两条新入口对应的 CLI flag。

### Expected Result

- `pid_parse::schema::coverage_report_schema()` / `byte_audit_report_schema()`
  返回 valid `schemars::Schema`。
- `cargo build --workspace` 通过；新 API 暴露在 `pub mod schema` 之下。
- 13-deny rustdoc gate 仍绿（必要时用绝对路径 intra-doc link，如
  `[`crate::byte_audit::aggregate::ByteAuditReport`]`）。

---

## Task 2: 在 `src/schema.rs` 单元测试覆盖新入口

### Files

- Modify: `src/schema.rs` (`#[cfg(test)] mod tests`)

### Steps

照 `schema_is_serializable_and_well_formed` 的模式，加 2 个并行测试：

```rust
#[test]
fn coverage_report_schema_is_well_formed_and_mentions_status_buckets() {
    let text = coverage_report_schema_pretty().expect("pretty JSON");
    let parsed: serde_json::Value = serde_json::from_str(&text)
        .expect("re-parse generated coverage schema");
    assert!(parsed.is_object(), "root coverage schema is an object");
    for needle in [
        "CoverageReport",
        "CoverageEntry",
        "ParseCoverageStatus",
        "FullyDecoded",
        "PartiallyDecoded",
        "IdentifiedOnly",
        "Unknown",
    ] {
        assert!(
            text.contains(needle),
            "coverage schema must mention `{needle}`; got prefix:\n{}",
            &text[..text.len().min(500)]
        );
    }
}

#[test]
fn byte_audit_report_schema_is_well_formed_and_mentions_core_fields() {
    let text = byte_audit_report_schema_pretty().expect("pretty JSON");
    let parsed: serde_json::Value = serde_json::from_str(&text)
        .expect("re-parse generated audit schema");
    assert!(parsed.is_object(), "root audit schema is an object");
    for needle in [
        "ByteAuditReport",
        "StreamAuditSummary",
        "ParserTrace",
        "TraceConfidence",
        "ByteRange",
        "overall_coverage_ratio",
        "unregistered_paths",
    ] {
        assert!(
            text.contains(needle),
            "audit schema must mention `{needle}`; got prefix:\n{}",
            &text[..text.len().min(500)]
        );
    }
}
```

### Expected Result

- 单元测试为新公开 API 提供 happy-path 自检，与既有 `schema_is_serializable_and_well_formed` 同级别。
- 这两条断言**不**承担 byte-equal 守门职责（这是 Task 5 snapshot 测试的工作），只防"完全删除核心字段名"这一类灾难性 regression。

---

## Task 3: `pid_inspect` 加 `--coverage-schema` / `--audit-schema`

### Files

- Modify: `src/bin/pid_inspect.rs`

### Steps

1. 在 `--schema` 处理块附近增加两个 flag：

   ```rust
   let coverage_schema_mode = args.iter().any(|a| a == "--coverage-schema");
   let audit_schema_mode = args.iter().any(|a| a == "--audit-schema");
   ```

2. 在 `if schema_mode { ... }` 之后并列追加（仍然在文件路径解析之前，避免 schema 模式还要 mock 一个 path）：

   ```rust
   if coverage_schema_mode {
       match pid_parse::schema::coverage_report_schema_pretty() {
           Ok(s) => println!("{s}"),
           Err(e) => {
               eprintln!("Coverage schema serialization error: {e}");
               std::process::exit(1);
           }
       }
       return;
   }
   if audit_schema_mode {
       match pid_parse::schema::byte_audit_report_schema_pretty() {
           Ok(s) => println!("{s}"),
           Err(e) => {
               eprintln!("Audit schema serialization error: {e}");
               std::process::exit(1);
           }
       }
       return;
   }
   ```

3. 同时支持"无 file 路径"调用：当任意 schema flag 出现时，`args.len() < 2` 不应再 hard-fail。把 usage 检查改成"如果没有 schema flag 才要求位置参数"。例如：

   ```rust
   let any_schema_flag = args.iter().any(|a| {
       a == "--schema" || a == "--coverage-schema" || a == "--audit-schema"
   });
   if args.len() < 2 || (!any_schema_flag && !args.iter().any(|a| !a.starts_with("--"))) {
       // existing usage block
   }
   ```

   注意保留对 `pid_inspect drawing.pid --schema` 这种"既有路径也有 flag"的调用方式的兼容。

4. 更新 usage 文本：

   ```
   pid_inspect <file.pid> [--json] [--schema] [--coverage-schema] [--audit-schema]
   ```

### Expected Result

- `cargo run --locked --bin pid_inspect -- --coverage-schema | head -5` 输出 JSON Schema 文档头。
- `cargo run --locked --bin pid_inspect -- --audit-schema | head -5` 输出 JSON Schema 文档头。
- 既有 `pid_inspect drawing.pid --schema` 行为不变。
- `pid_inspect` 在 schema-only 模式下不再要求位置参数（无需 `dummy.pid`）。

---

## Task 4: CLI 集成测试覆盖新 flag

### Files

- Modify: `tests/inspect_cli.rs`

### Steps

照该文件已有的 CLI 集成测试模式（`assert_cmd` / `Command::cargo_bin` 风格，按现存测试为准）加：

```rust
#[test]
fn coverage_schema_flag_emits_valid_json_without_path() {
    let out = run_pid_inspect(&["--coverage-schema"]);
    assert!(out.status.success(), "stderr:\n{}", out.stderr);
    let text = String::from_utf8(out.stdout).expect("utf8 schema");
    let _v: serde_json::Value = serde_json::from_str(&text)
        .expect("coverage schema is valid JSON");
    assert!(text.contains("CoverageReport"));
}

#[test]
fn audit_schema_flag_emits_valid_json_without_path() {
    let out = run_pid_inspect(&["--audit-schema"]);
    assert!(out.status.success(), "stderr:\n{}", out.stderr);
    let text = String::from_utf8(out.stdout).expect("utf8 schema");
    let _v: serde_json::Value = serde_json::from_str(&text)
        .expect("audit schema is valid JSON");
    assert!(text.contains("ByteAuditReport"));
}
```

> 如果 `inspect_cli.rs` 现有 helper 名字不同，按现存命名（`run_inspect` / `pid_inspect_cmd` 等）调整。

### Expected Result

- 两个 CLI 集成测试 PASS。
- 无 fixture 依赖（schema 模式不读 `.pid`），CI 公开运行也能跑过。

---

## Task 5: 落地 schema snapshot 守门

### Files

- Create: `tests/schema_snapshots.rs`
- Create: `tests/snapshots/.gitignore`（确保不忽略 `*.json`）
- Create: `tests/snapshots/pid_document_schema.json`
- Create: `tests/snapshots/coverage_report_schema.json`
- Create: `tests/snapshots/byte_audit_report_schema.json`

### Steps

#### 5.1 生成 baseline snapshot

首次运行（在落 snapshot 文件之前），用 `UPDATE_SNAPSHOTS=1` 让 Task 5 测试代码同时承担"刷新基线"职责：

```bash
mkdir -p tests/snapshots
UPDATE_SNAPSHOTS=1 cargo test --locked --test schema_snapshots
```

预期结果：3 份 `tests/snapshots/*.json` 写入磁盘。再次运行（不带 `UPDATE_SNAPSHOTS=1`）应当 0 diff PASS。

#### 5.2 测试代码（`tests/schema_snapshots.rs`）

```rust
//! Schema snapshot drift guard.
//!
//! Locks the JSON Schema for [`PidDocument`], [`CoverageReport`] and
//! [`ByteAuditReport`] against committed baselines under
//! `tests/snapshots/`. Any field rename / removal / type-shape change
//! flips the byte-level comparison and fails CI.
//!
//! Refresh path:
//!     UPDATE_SNAPSHOTS=1 cargo test --test schema_snapshots
//! …then review the diff and commit the new baseline together with the
//! type change in the same PR.

use std::fs;
use std::path::Path;

const SNAPSHOT_DIR: &str = "tests/snapshots";

fn snapshot_path(name: &str) -> std::path::PathBuf {
    Path::new(SNAPSHOT_DIR).join(format!("{name}.json"))
}

fn check_or_update(name: &str, current: &str) {
    let path = snapshot_path(name);
    let update = std::env::var_os("UPDATE_SNAPSHOTS").is_some();
    if update {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create snapshot dir");
        }
        fs::write(&path, current).expect("write snapshot");
        return;
    }
    let baseline = fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "missing baseline {}; rerun with UPDATE_SNAPSHOTS=1 to seed it ({e})",
            path.display()
        )
    });
    if baseline != current {
        let preview = current.lines().take(40).collect::<Vec<_>>().join("\n");
        panic!(
            "schema drift detected for `{name}`. \n\
             Baseline path: {}\n\
             First 40 lines of current schema:\n{preview}\n\
             ---\n\
             If this is intentional, re-run:\n    \
             UPDATE_SNAPSHOTS=1 cargo test --test schema_snapshots\n\
             …then commit the updated snapshot in the SAME PR as the type change.",
            path.display()
        );
    }
}

#[test]
fn pid_document_schema_matches_snapshot() {
    let current = pid_parse::schema::pid_document_schema_pretty()
        .expect("pid document schema pretty");
    check_or_update("pid_document_schema", &current);
}

#[test]
fn coverage_report_schema_matches_snapshot() {
    let current = pid_parse::schema::coverage_report_schema_pretty()
        .expect("coverage schema pretty");
    check_or_update("coverage_report_schema", &current);
}

#[test]
fn byte_audit_report_schema_matches_snapshot() {
    let current = pid_parse::schema::byte_audit_report_schema_pretty()
        .expect("audit schema pretty");
    check_or_update("byte_audit_report_schema", &current);
}
```

#### 5.3 路径与平台

- `tests/snapshots/*.json` 必须 commit；`.gitattributes` 已经把 `*.json` 视为 text，但要确认 line ending 一致：在 snapshot 写入时使用 `to_string_pretty`（输出 `\n`），不要再做平台转换。
- 不要使用 `\r\n`：snapshot 为 text，CRLF 会让 Linux CI 挂掉。如果 Windows 本地写入产生 CRLF，加一行配置：
  - `.gitattributes` 增加 `tests/snapshots/*.json text eol=lf`（如缺失）。

### Expected Result

- 3 份 snapshot baseline 文件落盘并 commit。
- `cargo test --locked --test schema_snapshots` 0 diff PASS。
- 故意改字段名后再跑测试会 PASS-failure，错误信息明确指出 snapshot 路径与 update 命令。

---

## Task 6: 文档与 contributor 工作流

### Files

- Modify: `AGENTS.md`
- Modify: `CONTRIBUTING.md`
- Modify: `docs/byte-audit-guide.md`
- Modify: `README.md`

### Steps

1. **`AGENTS.md`** "Pre-commit gates" 部分末尾追加：

   ```markdown
   - `cargo test --locked --test schema_snapshots`
     防止 `PidDocument` / `CoverageReport` / `ByteAuditReport` schema
     字段悄悄漂移；如果你**有意**改字段名 / 类型，按测试错误信息
     用 `UPDATE_SNAPSHOTS=1` 重新生成，并把新 baseline 与类型改动
     放在同一个 PR。
   ```

2. **`CONTRIBUTING.md`** 加一节 "## Schema snapshot workflow"：

   ```markdown
   ## Schema snapshot workflow

   `tests/snapshots/*.json` 锁定下游消费的三套 JSON Schema。任何修改
   `PidDocument` / `CoverageReport` / `ByteAuditReport` 形状的 PR 必须：

   1. 先跑 `cargo test --locked --test schema_snapshots`，看 diff。
   2. 如果 diff 是预期的（字段重命名、类型升级、文档表更新），用：
      ```bash
      UPDATE_SNAPSHOTS=1 cargo test --locked --test schema_snapshots
      ```
   3. 把新 baseline 与本次类型修改放进同一个 commit。
   4. PR description 里说明"为什么 schema 必须变"，给下游 codegen
      消费者预警窗口。
   ```

3. **`docs/byte-audit-guide.md`** 在 "Programmatic API" 章节追加一段：

   ```markdown
   ### JSON Schema 锁定

   `pid_parse::schema::byte_audit_report_schema()` 直接给到下游
   codegen 工具一个稳定的 schema 入口；同时仓内 `tests/snapshots/byte_audit_report_schema.json`
   做 byte-equal 守门，杜绝默认 `derive(JsonSchema)` 行为悄悄漂移。
   CLI 等价：`pid_inspect --audit-schema`。
   ```

4. **`README.md`** "JSON Schema 导出" 段落补两行 bullet：

   ```markdown
   - `pid_inspect --coverage-schema`：输出 [`CoverageReport`] 的 schema。
   - `pid_inspect --audit-schema`：输出 [`ByteAuditReport`] 的 schema。
   ```

### Expected Result

- 任何按 `AGENTS.md` 跑 5 道 gate 的开发者都会在工作流里加上 `schema_snapshots`。
- 下游消费者通过 `CONTRIBUTING.md` 知道"schema 改动有窗口期"。

---

## Task 7: CHANGELOG

### Files

- Modify: `CHANGELOG.md`

### Steps

`[Unreleased]` 顶部追加段（紧跟在 Phase 12c 段之后）：

```markdown
### schema lock：CoverageReport / ByteAuditReport JSON Schema 入口 + snapshot 守门（Phase 12d）

`docs/plans/2026-04-30-phase-12d-audit-schema-lock.md` 落地：

- `src/schema.rs` 暴露 4 个新公开函数：`coverage_report_schema()` /
  `coverage_report_schema_pretty()` / `byte_audit_report_schema()` /
  `byte_audit_report_schema_pretty()`。`PidDocument` 的旧 API 行为不变。
- `pid_inspect` 加 `--coverage-schema` / `--audit-schema` flag，在
  schema-only 模式下不再要求位置参数；既有 `--schema` 行为不变。
- 新增 `tests/schema_snapshots.rs` + `tests/snapshots/*.json` 三份
  byte-equal 守门 snapshot（document / coverage / audit）；任何
  字段重命名 / 删除 / 类型形状变化都会被 hard-fail。
  `UPDATE_SNAPSHOTS=1 cargo test --test schema_snapshots` 是唯一
  refresh 入口；新 baseline 必须与对应类型修改进同一个 PR。
- `AGENTS.md` / `CONTRIBUTING.md` / `docs/byte-audit-guide.md` /
  `README.md` 同步说明新 CLI flag 与 contributor 工作流。

零 parser / writer / publish 行为变化；新增 4 个公开 lib API 与 2 个
CLI flag，纯 additive。
```

### Expected Result

- `[Unreleased]` 段记录两个 Phase（12c + 12d），按时间序排列。
- 提交后 CI 运行 5 道 gate + baseline runner + 新 schema snapshot test 全绿。

---

## Task 8: 5 道 pre-commit gate 验证 + 故意制造一次 drift 自检

### Steps

#### 8.1 5 道 gate

```bash
cargo build  --locked --workspace --all-targets
cargo test   --locked --workspace --all-targets    # 含 schema_snapshots
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt    --all -- --check
bash .github/scripts/check-missing-docs.sh
bash .github/scripts/check-byte-audit-baselines.sh
```

全部 EXIT=0 才能进 commit。

#### 8.2 Drift self-test（不进 commit）

为了证明守门确实工作，**临时**在 `src/byte_audit/aggregate.rs` 把
`StreamAuditSummary.consumed_bytes` 改名为 `consumed`，跑：

```bash
cargo test --locked --test schema_snapshots
```

预期：`byte_audit_report_schema_matches_snapshot` FAIL，错误信息精确指出
snapshot 路径与 update 命令。**确认 fail 后 `git restore` 恢复**，再次
跑测试 PASS。drift self-test 结果记到 commit message 或 PR description。

### Expected Result

- 5 道 gate 全绿。
- drift self-test 成功 fail → restore → 再次绿。

---

## Task 9: Commit

### Steps

按 12c plan 同样的"功能/文档分拆"风格拆 2 个 commit：

1. ```
   feat(api): add coverage / byte-audit schema entrypoints (Phase 12d)

   - src/schema.rs: 4 public functions for CoverageReport /
     ByteAuditReport schema access (raw + pretty).
   - bin/pid_inspect: --coverage-schema / --audit-schema flags;
     schema-only modes no longer require a positional path.
   - inspect_cli tests cover both new CLI flags.
   ```

2. ```
   docs(snapshots): lock document / coverage / audit JSON schemas

   - tests/schema_snapshots.rs + tests/snapshots/*.json byte-equal
     drift guard for the 3 schemas.
   - UPDATE_SNAPSHOTS=1 documented in CONTRIBUTING.md, AGENTS.md and
     byte-audit-guide.md.
   - CHANGELOG [Unreleased] records Phase 12d.
   ```

也可以单 commit 合并；按维护者偏好。如果合并，commit message 用
`feat(schema): lock coverage / audit JSON schemas (Phase 12d)`。

### Expected Result

- `git log --oneline | head -3` 能看到 12d 两/一个新 commit。
- `git status` 干净。

---

## Acceptance Criteria

- [ ] `pid_parse::schema` 对外暴露 `coverage_report_schema` /
      `byte_audit_report_schema` 及对应 `_pretty` 入口
- [ ] `pid_inspect --coverage-schema` / `--audit-schema` 输出有效 JSON Schema
- [ ] `pid_inspect` 在仅 schema 模式下不要求位置参数
- [ ] `tests/schema_snapshots.rs` + `tests/snapshots/*.json` 三份 baseline 入仓
- [ ] `UPDATE_SNAPSHOTS=1` 能正确重写 baseline，恢复后 0 diff
- [ ] 故意制造的 drift 能被 snapshot 测试 hard-fail
- [ ] 5 道 pre-commit gate + baseline runner + schema snapshot test 全绿
- [ ] `AGENTS.md` / `CONTRIBUTING.md` / `docs/byte-audit-guide.md` / `README.md`
      同步更新
- [ ] `CHANGELOG.md [Unreleased]` 记录 Phase 12d

---

## Out of Scope

- 不重构 `CoverageReport` / `ByteAuditReport` / `PidDocument` 字段
  （任何字段调整在独立 PR 进行，并同时刷新 snapshot）。
- 不引入 `insta` / `cargo-insta` 等 snapshot 框架（保持零新 dep）。
- 不写 `NormalizedGraph` 相关 schema（留 Phase 12a-design）。
- 不动 `byte_audit` framework 的实际计算逻辑（schema 只观察现有形状）。
- 不修改 `derive(JsonSchema)` 在哪些类型上 derive；本 Phase 只暴露入口。
- 不为 publish XML pipeline (`PublishDrawing`, `PublishStyle`) 加 schema lock
  （独立 phase；publish 已有 fidelity gate 系统）。

---

## Risks

| 风险 | 影响 | 缓解 |
|---|---|---|
| `schemars 0.8` 输出顺序不稳定 | snapshot 误 fail | 用 `to_string_pretty` 已会按字段声明顺序稳定输出；如出现 BTreeMap key 顺序漂移，落地时记下 `serde(rename = "...")` 排序对照 |
| Windows CRLF 让 snapshot 文件污染 | CI hard-fail | `.gitattributes` 强制 `text eol=lf`（Task 5.3） |
| 新 CLI flag 与既有 usage 冲突 | 开发者打字错挂 | 新 flag 前缀 `--coverage-schema` / `--audit-schema` 精确匹配；解析时 `args.iter().any(|a| a == ...)` 不会 partial match |
| schema 入口被下游硬编码后 schemars 升级要 minor bump | breaking API | 升级 schemars 时与 12d snapshot 一同刷新；按 SemVer 走 minor |
| snapshot 文件过大（PidDocument schema 几十 KB） | review 噪声 | 先看 `tests/snapshots/pid_document_schema.json` 实际大小；> 100 KB 时再考虑改为 hash-only 守门（暂不做） |

---

## Forward Links

- 完成后的下一个 Phase：`docs/plans/2026-05-XX-phase-11a-psmclustertable-records.md`
  （PSM record 字段命名，Phase 11a；roadmap 阶段 B）
- 战略上下文：`docs/plans/2026-04-29-pid-parse-roadmap.md` 阶段 A 收官
- 相关解决方法论：见 `docs/plans/2026-04-29-phase-12c-byte-audit-baseline.md`
  （Phase 12c byte-audit baseline drift guard 是本 Phase 的"运行期"对应物，
  本 Phase 是其"类型期"对应物）
