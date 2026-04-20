# 开发计划：Phase 9m — `--set-summary` CLI flag + real-file integration

> 起稿：2026-04-21
> 背景：Phase 9l（v0.5.0）刚 ship SummaryInformation property-set writer，
> lib API + apply-plan JSON 路径已齐全。但 `pid_writer_validate` CLI 还
> 缺一个与 `--edit` / `--general-edit` 对称的 `--set-summary KEY=VALUE`
> 便利 flag，用户想改个 title 仍然要手写 plan.json。本 Phase 补齐这条
> 便利路径 + 扩 CLI 集成测试到 summary_updates 路径 + 加真实 `.pid`
> fixture 条件性端到端测试。
> 目标：v0.5.1 patch ship。

## 动机

- 对称性：`--edit ATTR=VALUE` 和 `--general-edit ELEMENT=VALUE` 分别
  对应 `/TaggedTxtData/Drawing` 和 `/TaggedTxtData/General` 的单项
  编辑。Summary 层现在需要 `--set-summary KEY=VALUE` 走同一风格。
- 低学习曲线：Phase 9b 的经验（"先特化再泛化"）告诉我们用户期望
  先有便捷命令，再有通用版。`--apply-plan` 是通用版，`--set-summary`
  是特化快捷方式。
- **真实 fixture 验证**：Phase 9l 的 lib-level 测试是手写 property-set
  fixture，虽然覆盖正确性，但不 prove 真实 SmartPlant `.pid` 的
  `/\x05SummaryInformation` 能被 round-trip。Phase 9m 加一条条件性
  real-file 测试，填补这个缺口。

## 非目标

- 不扩 Summary writer 的类型支持（VT_LPSTR UTF-8 / DocumentSummary
  section 2 / deletion 都保留给 Phase 9n+）
- 不改 Rust lib API（只是在 CLI 层转发到既有 `apply_summary_updates`）
- 不引入新依赖

## 范围

| 文件 | 改动类型 | 行数估计 |
|---|---|---|
| `src/bin/pid_writer_validate.rs` | 新增 `--set-summary KEY=VALUE` flag + 累加到 `summary_edits` map | +60 |
| `tests/writer_validate_cli.rs` | +3 CLI 集成测试：`--set-summary` 单条 / 多条 / 与 `--apply-plan` 冲突 | +130 |
| `tests/writer_real_files.rs` | +1 条件性 real-file 测试（修 title，parse 回来断言） | +50 |
| `docs/writer-quickstart.md` | 5.6 节 append CLI 例子 | +25 |
| `CHANGELOG.md` | `[Unreleased]` 扩 `[0.5.1] - 2026-04-21` | +25 |
| `Cargo.toml` | version `0.5.0` → `0.5.1` | ±1 |
| **本 plan** | | +本文件 |

合计 ~300 行改动。

## 关键决策

### A. `--set-summary` 的 KEY 语义跟 lib 完全对齐

```
pid_writer_validate input.pid --set-summary title="New Title" --out output.pid
```

- `title` 必须是 Phase 9l `KEY_TO_SUMMARY_PROPID` /
  `KEY_TO_DOC_SUMMARY_PROPID` 里定义的 11 个 key 之一。CLI 层**不再次
  做 key validation**——直接把 `(key, value)` 塞到 `summary_updates`
  map，由 `apply_summary_updates` 做校验并返回 `UnknownKey` 错误。
  这样 CLI 的 help 文本不用维护一份 key 表副本。
- 支持多次传入：`--set-summary title="X" --set-summary author="Y"`
  会累加到同一个 map。
- 重复 key 的行为：后一次覆盖前一次（标准 BTreeMap insert 语义）。

### B. 与既有 flag 的互斥 / 共存规则

- `--set-summary` 可以与 `--edit` / `--general-edit` 共存（三者都是
  单项特化编辑，只是作用于不同 stream）
- `--set-summary` **不能**与 `--apply-plan` 共存（同 Phase 9b 规则：
  declarative plan 与特化 flag 互斥）
- 错误消息统一："`--apply-plan` cannot be combined with `--edit` /
  `--general-edit` / `--set-summary` (they describe mutually
  exclusive edit semantics)"

### C. CLI 层内部数据结构

新增字段到 `CliOptions`：

```rust
struct CliOptions {
    ...
    summary_edits: BTreeMap<String, String>,
}
```

在 `build_write_plan` 里把 `summary_edits` 挪到 `plan.metadata_updates.summary_updates`。

### D. 真实文件测试的 fixture 选择

选用已经在 Phase 9c / Phase 9e 作为 reference 的 `DWG-0201GP06-01.pid`
（当 `test-file/` 目录存在时）。流程：

1. parse → 记住原 title（`summary.title`）
2. 构造 `summary_updates = {"title": "ROUND-TRIP-9M-<random>"}`
3. PidWriter::write_to → dst
4. parse dst → assert `summary.title == "ROUND-TRIP-9M-..."`
5. diff_packages(src, dst) → assert 只有
   `/\x05SummaryInformation` 出现在 modified 列表（其他流零差异）
6. 清理 dst

若 fixture 不存在（CI 默认情况）→ eprintln + return，与其他
`real_file_*` 测试保持一致。

## 实施步骤

### W1 — CLI `--set-summary` flag

1. 扩 `CliOptions` 新增 `summary_edits: BTreeMap<String, String>`
2. argv parse 里加分支匹配 `"--set-summary"`，调已有 `parse_edit_op`
   (value 格式 `KEY=VALUE`)；解析结果插到 `summary_edits`
3. 互斥检查：`plan_path.is_some() && !summary_edits.is_empty()` →
   返回 combined 错误
4. 在构造 `WritePlan` 的路径（非 `--apply-plan` 分支）里把
   `summary_edits` 灌到 `plan.metadata_updates.summary_updates`
5. help 文本里加一行说明

### W2 — CLI 集成测试

`tests/writer_validate_cli.rs` 新增：

1. `validate_set_summary_single_key_writes_title`：fixture CFB 含最小
   property-set，CLI 跑 `--set-summary title="X" --out ...`，dst parse
   回来 title == "X"
2. `validate_set_summary_multiple_keys_accumulate`：多次传入
   `--set-summary`，全部生效
3. `validate_set_summary_conflicts_with_apply_plan`：同时传
   `--set-summary` 和 `--apply-plan` → exit 1 + stderr 含 "mutually
   exclusive"
4. `validate_set_summary_unknown_key_exits_with_clear_error`：跑
   `--set-summary madeup_key="x"`，exit 非 0 + stderr 含 "unknown key"

### W3 — 真实文件条件性测试

`tests/writer_real_files.rs` 新增 `real_file_set_summary_title_via_plan`
（注：走 Rust API 而非 CLI，保持 writer_real_files.rs 的 API-only
风格）：

```rust
#[test]
fn real_file_set_summary_title_via_plan() {
    let fixture = fixture_path("DWG-0201GP06-01.pid");
    if !fixture.exists() { return; }
    // ... parse → edit title → verify
}
```

### W4 — docs + ship

- `docs/writer-quickstart.md` 5.6 节追加 CLI 用例
- `CHANGELOG.md` 新 `[0.5.1] - 2026-04-21` 段
- `Cargo.toml` 0.5.0 → 0.5.1
- commit + tag v0.5.1

## 预计工时

| 步骤 | 估时 |
|---|---|
| W1 CLI flag | 30 min |
| W2 CLI 集成测试 | 60 min |
| W3 real-file 测试 | 30 min |
| W4 ship | 20 min |
| **合计** | **~2.5 hr** |

## 验证清单

- [ ] `cargo fmt --all -- --check` 退出 0
- [ ] `cargo clippy --all-targets -- -D warnings` 退出 0
- [ ] `cargo test --all-targets` 全绿，test count 271 → 275+（+4
      CLI + 条件性 +1）
- [ ] `pid_writer_validate --help` 能看到 `--set-summary` 一行
- [ ] 手工：`pid_writer_validate fixture.pid --set-summary
      title="Hello" --out /tmp/out.pid` → diff_packages 只报
      `/\x05SummaryInformation` 改动
- [ ] `Cargo.toml` version = "0.5.1"
- [ ] `CHANGELOG.md` 含 `## [0.5.1] - 2026-04-21` 段
- [ ] `git tag --list v0.5.1` 有输出

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| `--set-summary` 和 `--edit` 共存语义意外：两个 flag 各自改同一流？ | 物理上不可能（`--edit` 改 Drawing XML，`--set-summary` 改 SummaryInformation property-set），设计上安全 |
| real-file fixture 缺失时 CI 红 | W3 走与既有 `real_file_*` 一致的 `if !fixture.exists() { return; }` 模式 |
| KEY=VALUE 的 VALUE 含 `=` 被错误分割 | 复用 `parse_edit_op` 的 `split_once('=')`（与 --edit 对齐），VALUE 里有 `=` OK |
| 用户在 shell 里 quote 不当导致 VALUE 空格丢失 | 已知 CLI 通用问题，与 `--edit` 无差异；docs 里给 quote 示例 |

## 回滚

改动面小、高内聚：
- `src/bin/pid_writer_validate.rs` 新增 ~60 行
- 3 个测试文件新增 tests
- 文档追加段落
单个 commit 一键 `git revert`。

## Next 候选（跟进）

- **Phase 9n**：VT_LPSTR UTF-8 / CP1252 encoding 支持
- **Phase 9o**：DocumentSummaryInformation user-defined section 2 读写
- **Phase 9p**：`summary_deletions` 字段（删除 prop）
- **Phase 10a**：layout performance pass（P3-2 cache + hot-path 优化）
