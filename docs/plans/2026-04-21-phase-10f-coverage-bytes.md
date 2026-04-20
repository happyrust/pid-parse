# 开发计划：Phase 10f — coverage 加 bytes 维度

> 起稿：2026-04-21
> 背景：roadmap Phase 4 的最终目标是"字节级 consumed/leftover 验证
> 框架"，让 coverage 报告能说"多少 bytes 被解释、哪些流有未解释区域
> 集中"。Phase 10a-10e 完成了 *结构* 维度（name/status/kind）但还没
> 有 *尺寸* 维度。本轮在 `CoverageEntry` 上加 `stream_size`，让
> consumers 至少能按字节排序，优先处理"大而未解析"的目标。
> 目标：v0.6.5 patch ship。

## 动机

- Phase 10e 输出了 JSON coverage，但 CI / dashboard 要做"按字节加权
  排序"就没抓手：所有 entry 看上去同等重要。
- 实际工程优先级极大依赖流尺寸：一个 Unknown 10 MB 流比 Unknown 100
  byte 流应该**先**看；静态分类看不出这个。
- 本轮不引入真正的 consumed/leftover 框架（那是 Phase 10g+），只
  先把"流大小"这个免费信号接进来。

## 非目标

- 不做 consumed-bytes tracking
- 不改 parser
- 不加 byte-level leftover 报告

## 范围

| 文件 | 改动 | 行数 |
|---|---|---|
| `src/model.rs` | `CoverageEntry` 加 `stream_size: Option<u64>` 字段 + `CoverageReport::total_bytes_by_status` helper | +40 |
| `src/inspect/coverage.rs` | 生成 coverage 时聚合 stream 大小到 entry；顶层 stream 取自己，storage 取 children 总和 | +50 |
| `src/inspect/coverage.rs::tests` | +3 测试（single stream size / storage aggregation / bytes_by_status）| +90 |
| `src/inspect/report.rs` | coverage section 显示 entry size（"28 KB" 或 raw bytes）| +20 |
| `src/inspect/report.rs::tests` | 更新 existing Phase 10a 断言 + 1 新测试 | +25 |
| `tests/inspect_cli.rs` | 1 新 CLI 测试验证 JSON 含 stream_size | +30 |
| `CHANGELOG.md` | `[0.6.5]` | +30 |
| `Cargo.toml` | 0.6.4 → 0.6.5 | ±1 |
| **本 plan** | | +本文件 |

~290 行。additive。

## 关键决策

### A. size 来源：`PidDocument.streams`

`StreamEntry` 已有 `size: u64`。我们在构造 coverage entry 时直接
lookup：

```rust
fn size_for_top_level(doc: &PidDocument, top_level_name: &str) -> Option<u64> {
    let mut total = 0u64;
    let mut found_any = false;
    for stream in &doc.streams {
        let trimmed = stream.path.trim_start_matches('/');
        let head = trimmed.split_once('/').map(|(h, _)| h).unwrap_or(trimmed);
        if head == top_level_name {
            total = total.saturating_add(stream.size);
            found_any = true;
        }
    }
    found_any.then_some(total)
}
```

- 顶层 stream (`"/DocVersion3"` → head="DocVersion3") 就一个 entry
- 顶层 storage (`"/Sheet1/Foo"` → head="Sheet1") 聚合所有 children

### B. `CoverageEntry.stream_size` 为 `Option<u64>`

- `Some(N)` = 已知大小
- `None` = 未知（当前 entry 来自命名推断，对应的 stream 不在
  `doc.streams` 里 —— 理论上不应发生，但 parser 容错下是可能的）

JSON 序列化 skip if None：

```rust
#[serde(skip_serializing_if = "Option::is_none")]
pub stream_size: Option<u64>,
```

### C. report section 的字节显示

```text
--- Coverage ---
  Fully decoded:     1
  Partially decoded: 1
  Identified only:   1
  Unknown:           1
  [FULL] DocVersion3 -> version_history              (48 B)
  [PART] PSMsegmenttable -> psm_segment_table         (220 B)
  [ID]   Sheet1 -> sheet_streams                      (12.3 KB)
  [UNK]  GhostStream                                  (123 B)
```

单位：B / KB / MB（按 1024 进制；保留 1 位小数）。

### D. `status_counts` 补兄弟：`total_bytes_by_status`

```rust
impl CoverageReport {
    pub fn total_bytes_by_status(&self) -> [u64; 4] {
        let mut totals = [0u64; 4];
        for entry in &self.entries {
            if let Some(sz) = entry.stream_size {
                let idx = match entry.status { ... };
                totals[idx] = totals[idx].saturating_add(sz);
            }
        }
        totals
    }
}
```

## 实施步骤

### W1 — model + coverage 构造

- `CoverageEntry` 加 field
- `coverage::classify` 接到 `size_for_top_level(doc, name)`
- 填入 entry

### W2 — 单测

1. `coverage_entry_carries_stream_size_for_single_stream`
2. `coverage_entry_aggregates_sizes_across_storage_children`
3. `coverage_report_total_bytes_by_status_matches_entries`

### W3 — report / CLI 升级

- report section 显示 "(X KB)" 后缀
- 更新 Phase 10a 3 个 report 测试的 assertion
- inspect_cli: 新 CLI 测试验证 `--coverage --json` 输出的
  entries 含 `stream_size` 字段

### W4 — ship

bump 0.6.4 → 0.6.5 + CHANGELOG + commit + tag。

## 预计工时

- W1 30 min
- W2 30 min
- W3 40 min
- W4 10 min
- **合计 ~2 hr**

## 验证清单

- [ ] fmt/clippy/test 全 0
- [ ] test count 329 → 334+
- [ ] Cargo.toml 0.6.5 + tag

## Next 候选

- **Phase 10g**：consumed-bytes tracking — 每个 parser 报告自己
  消费的 byte range；coverage 升级到 "解释了 N/M bytes" 的真字节率
- **Phase 10h**：leftover-range 报告 — 把未消费区间高亮显示
