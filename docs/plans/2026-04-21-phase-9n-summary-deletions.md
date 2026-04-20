# 开发计划：Phase 9n — `summary_deletions` + CRUD 收尾

> 起稿：2026-04-21
> 背景：Phase 9l / 9m 已给 `MetadataUpdates.summary_updates` 装好了
> CREATE + UPDATE 两条语义（新 prop 追加 / 已有 prop 覆写）。Phase 9l
> plan 里把 DELETE 标为 "future minor bump"；现在补齐它，让
> SummaryInformation / DocumentSummaryInformation 的 property-set 写
> 路径支持完整 CRUD。
> 目标：v0.5.2 patch ship。

## 动机

- 完形补图：CRUD 少了 D，API 就不全。CI / 自动化场景下用户经常需要
  "从模板继承，但删掉 `comments` / `keywords`" 这类操作。目前 Phase
  9l/9m 下用户只能 UPDATE 到空字符串（"title": ""）——这会留一个
  VT_LPSTR 0x001E prop 占位，而不是真的删掉。两种语义不同。
- 工作量小、风险低：逻辑完全对称于 UPDATE 路径，只是替换 prop 的
  section 级操作从 "set" 改成 "remove"。

## 非目标

- 不做整条 stream 删除（`remove_stream`）—— stream 层面的操作应走
  `stream_replacements` 手写 base64 空 blob（虽然 OLE property set
  stream 不允许完全空，所以这实际是无意义操作；此处只是划清边界）。
- 不做 `DocumentSummaryInformation` 第二个 section 的 prop 删除
  （section 2 仍未纳入 Phase 9l 的修改范围）。
- 不做 key-level 同时 update+delete（互斥：同一 key 不能既在
  `summary_updates` 又在 `summary_deletions` 里，否则 reject）。

## 范围

| 文件 | 改动类型 | 行数估计 |
|---|---|---|
| `src/writer/plan.rs` | 新增 `MetadataUpdates.summary_deletions: Vec<String>` 字段 + `#[serde(default)]` | +10 |
| `src/writer/summary_write.rs` | `apply_summary_deletions` fn + `SummarySection::remove` 方法 | +80 |
| `src/writer/metadata_write.rs` | 先 delete，再 update（guard 与 ordering） | +10 |
| `src/bin/pid_writer_validate.rs` | `--delete-summary KEY` flag + usage 更新 | +40 |
| `tests/writer_validate_cli.rs` | +3 CLI 集成测试 | +120 |
| `src/writer/summary_write.rs` tests | +3 unit：删单 prop / 删多 prop / 删不存在 prop（no-op 或 error？）| +80 |
| `tests/writer_real_files.rs` | +1 条件性 real-file 测试（如果 fixture 有 comments，删它） | +60 |
| `docs/writer-quickstart.md` | 5.6 节 append delete 例子 | +20 |
| `CHANGELOG.md` | `[Unreleased]` → `[0.5.2] - 2026-04-21` | +30 |
| `Cargo.toml` | 0.5.1 → 0.5.2 | ±1 |
| **本 plan** | | +本文件 |

合计 ~460 行。

## 关键设计决策

### A. "删不存在的 prop" 的语义

三种可选：
1. **Silent no-op**（删除不存在的 key 不 fail）
2. **Strict error**（`DeleteTargetMissing { key }`）
3. **Warn-but-continue**（日志里标注，但成功返回）

**选 1（silent no-op）**：与 `stream_replacements`、`metadata_updates.drawing_xml` 这些已有字段的惯例一致（目标不存在不会 fail）。用户通常不关心"已经没有"和"现在删掉"的区别，只关心结束态。
如果未来有严格场景，可以加 `strict: bool` 选项。

### B. update + delete 同 key 冲突处理

`summary_updates` 里包含 key `"title"` + `summary_deletions` 里也包含 `"title"` →
**reject** (`ConflictingOperation { key }`)。两个语义互斥，API consumer 一定写错了。

实施：`apply_summary_updates` 入口做 pre-check，遍历所有 `summary_updates` key，若也在 `summary_deletions` 里则 `return Err(...)`。

### C. 顺序：先 delete 还是先 update？

选 **先 delete 后 update**。这样 rare edge case（先 delete 再 update 同 key）如果冲突校验没拦住，也会在最后 update 路径上把 prop 回加回来，语义是"最终状态有该 prop"，对用户更符合直觉。

但由于 B 已经在前置 reject 了冲突，实际上 ordering 对正确行为没影响。为了可读性，选择 "先 delete 后 update"。

### D. CLI 参数设计

```
pid_writer_validate input.pid --delete-summary comments --out output.pid
```

- 支持多次：`--delete-summary title --delete-summary author`
- key 与 `--set-summary` 共享同一 symbolic 表（writer 层做 resolve）
- 未知 key 报 `UnknownKey`（错误走 writer 路径）
- 与 `--set-summary` 可共存（只要无同 key 冲突）
- 与 `--apply-plan` 互斥（同 Phase 9b/9m 规则）

### E. `apply_metadata_updates` 内部顺序

Phase 9l 已有：
1. drawing XML
2. general XML
3. summary_updates (apply)

Phase 9n 插入：
1. drawing XML
2. general XML
3. summary_deletions (先删)
4. summary_updates (再增/改)

## 实施步骤

### W1 — WritePlan 新字段

`src/writer/plan.rs`：

```rust
pub struct MetadataUpdates {
    // ... existing ...
    #[serde(default)]
    pub summary_deletions: Vec<String>,
}
```

`is_passthrough` 判空扩 `summary_deletions.is_empty()`。

### W2 — summary_write 核心

新增 `SummarySection::remove(&mut self, prop_id: u32) -> bool`（返回是否删成功）。

新增 `pub fn apply_summary_deletions(pkg: &mut PidPackage, deletions: &[String]) -> Result<(), PidError>`：
- 空输入 → no-op 返回 Ok
- 按 key resolve 到 (stream, PROPID)
- 按 stream 分组，每个 stream 解析 → 对每个 PROPID 调 `remove`（silent no-op on miss）
- 序列化回 stream

新增 `apply_summary_updates_and_deletions(pkg, updates, deletions)` 门面，或扩 `apply_summary_updates` 签名（选后者更简）。

为简洁，复用既有 `apply_summary_updates` 路径：
- 如果有 deletions，先在 section 里 remove 目标 prop
- 然后再走 update 的 set_string 路径

### W3 — apply_metadata_updates 接入

`writer/metadata_write.rs`：

```rust
pub fn apply_metadata_updates(package, updates) -> Result<...> {
    // ... xml writes ...
    // conflict check
    for key in &updates.summary_deletions {
        if updates.summary_updates.contains_key(key) {
            return Err(PidError::ParseFailure { context: "summary writer",
                message: format!("key '{key}' appears in both summary_updates and summary_deletions; at most one must be specified per key") });
        }
    }
    summary_write::apply_summary_deletions(package, &updates.summary_deletions)?;
    summary_write::apply_summary_updates(package, &updates.summary_updates)?;
    Ok(())
}
```

### W4 — CLI `--delete-summary`

扩 `CliOptions` 加 `summary_deletions: Vec<String>`。

argv parse 加：
```rust
"--delete-summary" => {
    let value = args.get(i+1).ok_or_else(|| "--delete-summary requires KEY".to_string())?;
    if value.is_empty() { return Err("--delete-summary KEY must be non-empty".into()); }
    summary_deletions.push(value.clone());
    i += 2;
}
```

互斥检查扩到 summary_deletions。`run_validate` 签名加 `summary_deletions: &[String]`。

### W5 — 单元测试（src/writer/summary_write.rs::tests）

- `apply_summary_deletions_removes_existing_prop`
- `apply_summary_deletions_nonexistent_key_is_noop` (silent)
- `apply_summary_deletions_unknown_key_returns_error` (key not in table)
- `apply_summary_deletions_then_updates_produces_expected_state`
- `apply_metadata_updates_rejects_key_in_both_deletions_and_updates`

### W6 — CLI 集成测试（tests/writer_validate_cli.rs）

- `validate_delete_summary_removes_target_prop`（先 fixture 有 title，CLI 跑 `--delete-summary title`，reparse 断言 title is None）
- `validate_delete_summary_multiple_keys`
- `validate_delete_and_set_summary_combine_legally`（`--set-summary title=X --delete-summary author`）
- `validate_delete_summary_conflicts_with_set_summary_on_same_key`
  （`--set-summary title=X --delete-summary title` → exit 2 + stderr）

### W7 — real-file 条件性测试

`tests/writer_real_files.rs` `real_file_delete_summary_comments_when_present`：
- 如果 fixture 存在且 `summary.raw` 里有 `"Comments"` → 删之 → reparse 断言没有

### W8 — docs + CHANGELOG + ship v0.5.2

## 预计工时

| 步骤 | 估时 |
|---|---|
| W1 plan 字段 | 10 min |
| W2 summary_write 核心 | 45 min |
| W3 metadata_write 接入 | 15 min |
| W4 CLI flag | 30 min |
| W5 单元测试 | 30 min |
| W6 CLI 集成测试 | 45 min |
| W7 real-file 测试 | 20 min |
| W8 docs + ship | 25 min |
| **合计** | **~3.5 hr** |

## 验证清单

- [ ] `cargo fmt --check` / `cargo clippy -D warnings` / `cargo test --all-targets` 全 0
- [ ] test count 276 → 285+（+4 CLI + 1 real-file + 4 unit = 9 条）
- [ ] `pid_writer_validate --help` 能看到 `--delete-summary`
- [ ] `Cargo.toml` version = "0.5.2"
- [ ] `CHANGELOG.md` 含 `## [0.5.2] - 2026-04-21`
- [ ] `git tag --list v0.5.2` 有输出

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| section 删 prop 后 id/offset 表重排错 | `SummarySection::serialize_body` 在 Phase 9l 就已按 `self.props` 的当前顺序逐个 assign offset，**删除是 `Vec::retain` 行为，offset 自动重算**。W2 单测覆盖。 |
| silent no-op 让用户以为删成功但其实没（拼错 key） | unknown key 依然报错（key 不在 symbolic 表里）；只有 "key 在表里但 section 里当前不存在" 才 silent。这跟 "update 不存在的 key 会 append" 行为对称 |
| 新 `summary_deletions` 字段让现有 plan.json 解析 fail | `#[serde(default)]` 保证 v0.5.0 / v0.5.1 生成的 plan.json 继续兼容（缺省视为空 Vec） |
| update + delete 同 key 互斥在 CLI 层漏检测 | 统一到 lib 层 `apply_metadata_updates` 入口 precheck；CLI 零维护 |

## 回滚

新字段 + 新模块方法都是增量，无破坏性。单个 commit revert 即可。

## Next 候选（跟进）

- **Phase 9o**：VT_LPSTR UTF-8 / CP1252 encoding 支持
- **Phase 9p**：DocumentSummaryInformation section 2 (user-defined)
- **Phase 10a**：layout performance pass（P3-2 缓存）
- **PSMclustertable per-record 逆向**（待交叉验证锚点确认）
