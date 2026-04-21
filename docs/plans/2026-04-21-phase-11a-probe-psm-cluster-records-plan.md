# Phase 11a-probe — PSMclustertable Per-Record Probe Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** roadmap 的 Phase 11a 要把 `PSMclustertable` 从 PartiallyDecoded 升级到 FullyDecoded，但硬约束是 ≥ 2 份 fixture。当前只有 1 份，所以本步先做 **probe 层**（roadmap 要求的"raw + decoded + audit/probe 三层"里的 probe 层）：每条 record 提供字节探针，既不承诺字段名字，也不 block 后续 decoded 层。等另一份 fixture 到位时，再在 probe 的基础上命名字段。

**Architecture:** 保持 `parse_psm_cluster_table` 主流程不变。新增 `PsmClusterRecordProbe` 并把它挂到 `PsmClusterEntry` 的一个可选字段上。探针纯计算字节，没有语义。`inspect/report` 能把前几条 probe 打出来便于人看。

**Tech Stack:** Rust 现有 `model.rs` / `parsers/psm_tables.rs` / `inspect/report.rs`、单测 + fixture 断言。

---

### Task 1: 定义 `PsmClusterRecordProbe` 模型

**Files:**
- Modify: `src/model.rs`
- Test: `src/parsers/psm_tables.rs`

**Step 1: 写失败测试**

在 `parse_psm_cluster_table` 的测试模块里新增：

```rust
#[test]
fn cluster_table_entry_probes_expose_byte_level_summary() {}
```

至少断言：

- 每条 `PsmClusterEntry.probe` 是 `Some`
- probe 里暴露：
  - `first_u32_le: Option<u32>`（prefix 前 4 字节解读为 LE u32，若 prefix < 4 则 None）
  - `last_u32_le: Option<u32>`（record 最后 4 字节解读为 LE u32，若 record_len < 4 则 None）
  - `prefix_hex: String`（prefix 字节的两位大写 hex，空格分隔）
  - `trailer_hex: String`（record 尾部 8 字节（若 record_len < 8 则全部）两位大写 hex）
  - `name_char_count: usize`（name 字符数）

**Step 2: 扩展模型**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Default)]
pub struct PsmClusterRecordProbe {
    pub first_u32_le: Option<u32>,
    pub last_u32_le: Option<u32>,
    pub prefix_hex: String,
    pub trailer_hex: String,
    pub name_char_count: usize,
}
```

并在 `PsmClusterEntry` 下新增：

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub probe: Option<PsmClusterRecordProbe>,
```

`#[serde(default)]` + `skip_if_none`，旧 JSON 兼容。

**Step 3: 跑编译**

Run: `cargo build --all-targets`

Expected: 编译通过；测试仍 FAIL（probe 未填）。

**Step 4: Commit**

```bash
git add src/model.rs
git commit -m "feat(model): add PSM cluster record probe type"
```

### Task 2: 填充 probe 字段

**Files:**
- Modify: `src/parsers/psm_tables.rs`

**Step 1: 在 `parse_psm_cluster_table` 里生成 probe**

在 push `PsmClusterEntry` 之前构造 probe：

```rust
let record_slice = &data[record_start..i];
let probe = build_record_probe(record_slice, &prefix, &name);
```

实现 `fn build_record_probe(record: &[u8], prefix: &[u8], name: &str) -> PsmClusterRecordProbe`：

- `first_u32_le = (prefix.len() >= 4).then(|| read_u32_le(prefix, 0)?)`
  - 注意：`prefix` 可能 < 4 字节（第一条记录前面只有 magic+count，prefix 通常为 8 字节起步；但保险检查）
- `last_u32_le`：看 `record` 最后 4 字节
- `prefix_hex`：`prefix.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ")`
- `trailer_hex`：`record[record.len().saturating_sub(8)..]` 的 hex
- `name_char_count = name.chars().count()`

**Step 2: 跑测试**

Run: `cargo test parse_psm_cluster -- --nocapture`

Expected: PASS

**Step 3: 增补小测试**

- prefix 0 字节 → `first_u32_le = None`、`prefix_hex = ""`
- record 全长 ≤ 8 字节 → `trailer_hex` 包含整段记录
- name 含多字节 → `name_char_count` 准确

**Step 4: Commit**

```bash
git add src/parsers/psm_tables.rs
git commit -m "feat(parser): emit byte-level probe for each PSM cluster record"
```

### Task 3: inspect report 展示 probe 样本

**Files:**
- Modify: `src/inspect/report.rs`

**Step 1: 写失败测试**

```rust
#[test]
fn report_shows_psm_cluster_record_probe_sample() {}
```

断言：

- PSMclustertable 段内首 3 条 entry 显示 `probe:` 样本
- 样本里含 `first_u32_le=` / `trailer=` / `chars=` 等关键字

**Step 2: 最小实现**

在现有 PSMclustertable entry 打印下面新增一行：

```text
    probe: first_u32_le=0x.. last_u32_le=0x.. chars=N trailer=[hex...]
```

仅前 3 条样本（保持输出紧凑）。

**Step 3: 跑测试**

Run: `cargo test inspect::report::tests::report_shows_psm_cluster -- --nocapture`

Expected: PASS

**Step 4: Commit**

```bash
git add src/inspect/report.rs
git commit -m "feat(report): show PSM cluster record probe samples"
```

### Task 4: 真实样本回归

**Files:**
- Modify: `tests/parse_real_files.rs`

**Step 1: 写测试**

```rust
#[test]
fn psm_cluster_record_probes_match_entry_slice() {}
```

断言：

- 每条 `PsmClusterEntry.probe` 是 `Some`
- 当 `prefix_bytes.len() >= 4` 时 `probe.first_u32_le` 与直接读的一致
- 当 `record_len >= 4` 时 `probe.last_u32_le` 与 entry 最后 4 字节一致
- `probe.name_char_count == entry.name.chars().count()`
- `probe.prefix_hex.len() / 3 <= prefix_bytes.len()`（确保没越界）

**Step 2: 运行**

Run: `cargo test psm_cluster_record_probes_match_entry_slice -- --nocapture`

Expected: PASS（缺 fixture 则 skip）

**Step 3: Commit**

```bash
git add tests/parse_real_files.rs
git commit -m "test(parser): verify PSM cluster record probes against fixture"
```

### Task 5: 全量验证

Run:

```bash
cargo test --all-targets
cargo fmt --check
```

Expected: 全绿 + fmt 干净。

若有 fmt 增量则单独一次 `chore(fmt): ...` 提交。

## 设计约束

### 为什么只做 probe 不做 decoded 层

- roadmap 硬约束 = ≥ 2 fixture 才能 claim FullyDecoded
- 单 fixture 可能过拟合，命名字段会误导
- probe 层提供可观察数据，让后续 fixture 到位后能快速确认命名
- 与 Phase 9k 的 Dynamic Attributes probe 路数一致

### 不要做的事

- 不要给 probe 字段起语义化名字（如 cluster_id / flags / type_tag）
- 不要修改 `PsmClusterEntry` 已有字段
- 不要在 layout / import_view 里消费 probe
- 不要在此步升级 `psm_cluster_table` 的 coverage 分类

### SemVer

Patch（新增 Option 字段，默认 None；旧 JSON 兼容）。

## 完成定义

- `PsmClusterEntry.probe: Option<PsmClusterRecordProbe>` 就绪并填充
- report 展示前 3 条 probe 样本
- fixture 回归断言 probe ↔ record slice 一致
- 全量测试持续绿

后续：当第 2 份 fixture 到位时，按 Phase 11a 完整 plan
(`docs/plans/2026-04-21-phase-11a-psmclustertable-records.md`) 在 probe 基础上
命名字段 → coverage 升级到 FullyDecoded（confidence=high）。
