# Phase 12b-1b — Trace-migrate `parse_psm_roots` + `parse_psm_cluster_table`

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 验证 Phase 12b-1-minimal 建立的 trace API 在两种**非**平凡
parser 形态上也成立：

1. **PSMroots** — 定长 record + 显式 sentinel 终止 + 变长 UTF-16LE name
2. **PSMclustertable** — 变长 prefix + UTF-16LE ASCII scan-based record
   识别

这两种形态比 `parse_psm_segment_table`（纯 header + flag 字节数组）复杂得多，
如果 trace API 能干净描述它们，12b-2 的全 parser 迁移就有了稳定模板。

**Architecture:** 沿用 12b-1 的 thin-wrapper 模式 —
每个 parser 都新增 `_with_trace` 变体，原 `pub fn` 改为调用该变体并丢弃 trace。
单测覆盖：header+record+trailing 三段的 consumed/leftover 计算正确；
旧 API byte-identical 回归。

**Tech Stack:** 仅 Rust 现有 `src/parsers/psm_tables.rs` + 新单测。
零 fixture 依赖；零 lib.rs 改动（pub re-export 已在 12b-1 Task 4 完成）。

---

## Task 1: `parse_psm_roots_with_trace`

**Trace schema:**
- `[0..4]` — `root` magic — `Decoded`
- 每条 entry:
  - `[pos..pos+4]` — id — `Decoded`
  - `[pos+4..pos+8]` — char_count — `Decoded`
  - `[name_start..name_start+cc*2]` — UTF-16LE name — `Decoded`
- sentinel `{id=0, cc=0}` (8 字节) — `Decoded`（明确的终止标记）
- implausible `cc > 512` / `read_utf16le_name` 失败 — **不 consume**，退出循环
- `trailing_bytes` 从 `pos`（最后 entry 或 sentinel 后）开始 — 进 leftover

**Step 1: 新增单测**

- `trace_aware_roots_parser_covers_header_and_all_entries`
- `trace_aware_roots_parser_consumes_sentinel_and_leaves_trailing_garbage_as_leftover`
- `back_compat_parse_psm_roots_matches_trace_variant_byte_for_byte`

**Step 2: 实现 `_with_trace` + 旧 API 改 thin wrapper**

**Step 3: Commit**

```
feat(parser): trace-aware parse_psm_roots (Phase 12b-1b)
```

## Task 2: `parse_psm_cluster_table_with_trace`

**Trace schema:**
- `[0..4]` — `clst` magic — `Decoded`
- `[4..8]` — count — `Decoded`
- 每条 record:
  - prefix `[record_start..name_start]` — `Probed`（内部字段未命名）
  - name run `[name_start..name_start + name_byte_len]` — `Decoded`
  - null terminator（若存在，2 字节）— `Decoded`
- `trailing_bytes` 从最后 `record_start` 开始 — 进 leftover
- scan 跳过的非 ASCII 字节 — **不 consume**，自然进 leftover

**注意**：`parse_psm_cluster_table` 目前是 byte-by-byte scan，遇 ASCII 才
尝试构造 name。如果 name 长度 `< 4` 就放弃当前 run，record_start 不前移。
trace 策略要与 **实际落盘到 entries 里的 record** 对齐 —— 被 reject 的
short run 不 consume（否则会 double-count）。

**Step 1: 新增单测**

- `trace_aware_cluster_parser_covers_header_prefix_name_per_record`
- `trace_aware_cluster_parser_marks_prefix_probed_and_name_decoded`
- `trace_aware_cluster_parser_leaves_trailing_bytes_as_leftover`
- `back_compat_parse_psm_cluster_table_matches_trace_variant_byte_for_byte`

**Step 2: 实现**

**Step 3: Commit**

```
feat(parser): trace-aware parse_psm_cluster_table (Phase 12b-1b)
```

## Task 3: 全量验证

Run:

```bash
cargo build --all-targets
cargo test --all-targets
```

Expected: 全绿；新增测试数 ≥ 7；既有测试零回归；lib.rs 不动。

## 设计约束

### 为什么这两个 parser 很有代表性

- **PSMroots** — 定长 record + sentinel 终止：12b-2 的 DocVersion2/3 迁移
  会复用相同结构（fixed-size record + optional trailing sentinel）
- **PSMclustertable** — scan-based 变长 + prefix/name 混合 confidence：12b-2
  里 DynamicAttributes / Sheet endpoint records 都是类似变长 record，会复
  用相同的 "prefix Probed + named-field Decoded" 分桶逻辑

### 不要做的事

- 不要改 parser 的解析结果（entries / offsets / names / probes 全部保持
  byte-identical）
- 不要在 trace 里加 record-index 维度（Phase 12b-2 的 ByteAuditReport 才
  需要，本 phase 聚焦 per-parser trace）
- 不要动 `src/streams/psm_tables.rs`（dispatcher 暂时不收集 trace）
- 不要改 lib.rs pub re-export（12b-1 已经暴露了所有需要的类型）
- 不要 bump 版本号

### SemVer

Patch（新 pub API `parse_psm_roots_with_trace` /
`parse_psm_cluster_table_with_trace`，零破坏）。

## 完成定义

- 两个 parser 都有 `_with_trace` 变体
- 旧 API 改 thin wrapper 且行为 byte-identical
- 每个 parser 至少 3 个新单测（覆盖 / 分桶 / back-compat）
- 总测试数 ≥ 7 新增；现有测试零回归

## 交叉引用

- 前置：`docs/plans/2026-04-22-phase-12b-1-minimal-byte-audit-scaffold-plan.md`
- 后续：Phase 12b-2 全 parser 迁移 + ByteAuditReport
